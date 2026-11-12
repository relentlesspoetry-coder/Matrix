//! Opt-in library conveniences. Game files are never copied or modified.
use super::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct GameOptions {
    pub args: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Recent {
    pub kind: String,
    pub uid: String,
    pub title: String,
    pub played_at: u64,
}
pub(crate) fn record(game: &Game) -> Result<(), String> {
    update_settings(|s| {
        s.recent
            .retain(|r| r.kind != game.kind || r.uid != game.uid);
        s.recent.insert(
            0,
            Recent {
                kind: game.kind.clone(),
                uid: game.uid.clone(),
                title: game.title.clone(),
                played_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            },
        );
        s.recent.truncate(20);
        Ok(())
    })
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn save_game_options(
    kind: String,
    uid: String,
    args: Vec<String>,
) -> Result<(), String> {
    validate_game(&kind, &uid)?;
    if args.len() > 64 || args.iter().any(|a| a.len() > 4096 || a.contains('\0')) {
        return Err(
            "Use up to 64 arguments, each at most 4096 bytes, without NUL characters".into(),
        );
    }
    let key = cover_key(&kind, &uid);
    update_settings(|s| {
        if args.is_empty() {
            s.game_options.remove(&key);
        } else {
            s.game_options.insert(key, GameOptions { args });
        }
        Ok(())
    })
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn game_options(kind: String, uid: String) -> Result<GameOptions, String> {
    validate_game(&kind, &uid)?;
    Ok(load_settings()?
        .game_options
        .get(&cover_key(&kind, &uid))
        .cloned()
        .unwrap_or_default())
}
#[derive(Serialize, Deserialize)]
struct Cover {
    kind: String,
    name: String,
    bytes: String,
}
#[derive(Serialize, Deserialize)]
struct Backup {
    format: u32,
    settings: Settings,
    covers: Vec<Cover>,
}
const MAX: usize = 64 * 1024 * 1024;
fn export() -> Result<Vec<u8>, String> {
    let mut settings = load_settings()?;
    settings.steamgriddb_api_key.clear();
    let mut covers = Vec::new();
    let mut total = 0;
    for spec in SPECS {
        for dir in [console_covers_dir(spec.id)?, artwork_root()?.join(spec.id)] {
            if !dir.is_dir() {
                continue;
            }
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                    continue;
                }
                let p = entry.path();
                if !p.extension().and_then(|s| s.to_str()).is_some_and(|s| {
                    ["png", "jpg", "jpeg", "webp"].contains(&s.to_ascii_lowercase().as_str())
                }) {
                    continue;
                }
                let len = entry.metadata().map_err(|e| e.to_string())?.len() as usize;
                if len > MAX_COVER_BYTES {
                    return Err("A cover exceeds the 4 MB backup limit".into());
                }
                total += len;
                if total > MAX / 2 {
                    return Err("Cover collection is too large for a single 64 MB backup".into());
                }
                let bytes = fs::read(p).map_err(|e| e.to_string())?;
                if cover_format(&bytes).is_none() {
                    continue;
                }
                covers.push(Cover {
                    kind: spec.id.into(),
                    name: entry
                        .file_name()
                        .into_string()
                        .map_err(|_| "Cover filename is not UTF-8")?,
                    bytes: BASE64.encode(bytes),
                });
            }
        }
    }
    let bytes = serde_json::to_vec(&Backup {
        format: 1,
        settings,
        covers,
    })
    .map_err(|e| e.to_string())?;
    if bytes.len() > MAX {
        return Err("Backup exceeds 64 MB".into());
    }
    Ok(bytes)
}
fn restore(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > MAX {
        return Err("Backup exceeds 64 MB".into());
    }
    let mut backup: Backup =
        serde_json::from_slice(bytes).map_err(|_| "Invalid Matrix Play backup")?;
    if backup.format != 1 {
        return Err("Unsupported backup version".into());
    }
    if backup
        .settings
        .game_options
        .values()
        .any(|o| o.args.len() > 64 || o.args.iter().any(|a| a.len() > 4096 || a.contains('\0')))
    {
        return Err("Invalid launch arguments in backup".into());
    }
    backup.settings.recent.truncate(20);
    let mut decoded = Vec::new();
    for cover in backup.covers {
        let name = Path::new(&cover.name);
        if name.components().count() != 1
            || !matches!(
                name.components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err("Unsafe cover filename in backup".into());
        }
        let extension = name
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or("Missing cover extension")?
            .to_ascii_lowercase();
        let target = console_covers_dir(&cover.kind)?
            .join(&cover.name)
            .with_extension(extension);
        let data = BASE64
            .decode(cover.bytes)
            .map_err(|_| "Invalid cover data")?;
        if data.len() > MAX_COVER_BYTES || cover_format(&data).is_none() {
            return Err("Invalid cover image in backup".into());
        }
        decoded.push((target, data));
    }
    // Preserve an automatic safety snapshot before applying anything.
    let safety = config_file()?.with_file_name("before-restore.json");
    let previous = export()?;
    let mut file = tempfile::NamedTempFile::new_in(safety.parent().ok_or("Invalid settings path")?)
        .map_err(|e| e.to_string())?;
    use std::io::Write;
    file.write_all(&previous).map_err(|e| e.to_string())?;
    file.persist(&safety).map_err(|e| e.to_string())?;
    // Existing covers win: restoring never destroys artwork chosen since the backup.
    let mut added = Vec::new();
    let result = (|| -> Result<(), String> {
        for (target, data) in decoded {
            if artwork::save_without_replacing(&target, &data)? {
                added.push(target);
            }
        }
        update_settings(|current| {
            backup.settings.steamgriddb_api_key = current.steamgriddb_api_key.clone();
            *current = backup.settings;
            Ok(())
        })?;
        Ok(())
    })();
    if let Err(e) = result {
        for path in added {
            let _ = fs::remove_file(path);
        }
        return Err(e);
    }
    Ok(format!("Restored settings, favorites and {} missing covers. Existing covers were kept. A safety backup is saved at {}.",added.len(),safety.display()))
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) async fn backup_library(
    app: tauri::AppHandle,
    restore_backup: bool,
) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        let picker = app
            .dialog()
            .file()
            .add_filter("Matrix Play backup", &["json"]);
        let selected = if restore_backup {
            picker
                .set_title("Restore a trusted Matrix Play backup")
                .blocking_pick_file()
        } else {
            picker
                .set_file_name("matrix-play-backup.json")
                .blocking_save_file()
        };
        let Some(selected) = selected else {
            return Ok("Cancelled".into());
        };
        let path = selected.into_path().map_err(|e| e.to_string())?;
        if !restore_backup
            && (path.extension().and_then(|s| s.to_str()) != Some("json") || path == config_file()?)
        {
            return Err(
                "Choose a separate .json backup file, not the application's settings file".into(),
            );
        }
        artwork::run_cover_operation(|| {
            if restore_backup {
                use std::io::Read;
                let mut bytes = Vec::new();
                fs::File::open(&path)
                    .map_err(|e| e.to_string())?
                    .take((MAX + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                restore(&bytes)
            } else {
                let bytes = export()?;
                let mut f =
                    tempfile::NamedTempFile::new_in(path.parent().ok_or("Invalid backup path")?)
                        .map_err(|e| e.to_string())?;
                use std::io::Write;
                f.write_all(&bytes).map_err(|e| e.to_string())?;
                f.persist(path).map_err(|e| e.to_string())?;
                Ok("Backup saved. API keys and game files are excluded.".into())
            }
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_tools_in_isolated_home() {
        let temp = tempfile::tempdir().unwrap();
        let name = format!("{}::fixture", module_path!().split_once("::").unwrap().1);
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env("MATRIX_LIBRARY_TOOLS_TEST", "1")
            .env("HOME", temp.path())
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .env("XDG_DATA_HOME", temp.path().join("data"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn fixture() {
        if std::env::var_os("MATRIX_LIBRARY_TOOLS_TEST").is_none() {
            return;
        }
        create_layout().unwrap();
        let path = games_dir("ps2").unwrap().join("Game.iso");
        fs::write(&path, b"synthetic game data").unwrap();
        let uid = path.to_str().unwrap().to_string();
        let game = validate_game("ps2", &uid).unwrap();
        update_settings(|s| {
            s.steamgriddb_api_key = "secret-that-must-not-be-exported".into();
            Ok(())
        })
        .unwrap();
        record(&game).unwrap();
        record(&game).unwrap();
        assert_eq!(load_settings().unwrap().recent.len(), 1);
        save_game_options(
            "ps2".into(),
            uid.clone(),
            vec!["--custom".into(), "{game}".into()],
        )
        .unwrap();
        assert_eq!(
            game_options("ps2".into(), uid.clone()).unwrap().args.len(),
            2
        );
        assert!(
            save_game_options("ps2".into(), uid.clone(), vec!["bad\0argument".into()]).is_err()
        );
        let cover = console_covers_dir("ps2").unwrap().join("Game.png");
        let png = include_bytes!("../../tests/fixtures/demo-cover.png");
        fs::write(&cover, png).unwrap();
        let bytes = export().unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("secret-that-must-not-be-exported"));
        let bad = Backup {
            format: 1,
            settings: Settings::default(),
            covers: vec![Cover {
                kind: "ps2".into(),
                name: "../escape.png".into(),
                bytes: BASE64.encode(png),
            }],
        };
        assert!(restore(&serde_json::to_vec(&bad).unwrap()).is_err());
        fs::remove_file(&cover).unwrap();
        update_settings(|s| {
            s.recent.clear();
            s.game_options.clear();
            Ok(())
        })
        .unwrap();
        restore(&bytes).unwrap();
        assert_eq!(fs::read(&cover).unwrap(), png);
        assert_eq!(load_settings().unwrap().recent.len(), 1);
        assert_eq!(
            load_settings().unwrap().steamgriddb_api_key,
            "secret-that-must-not-be-exported"
        );
        let mut replacement = png.to_vec();
        replacement.extend(b"newer manual image");
        fs::write(&cover, &replacement).unwrap();
        restore(&bytes).unwrap();
        assert_eq!(fs::read(&cover).unwrap(), replacement);
        fs::remove_file(&cover).unwrap();
        let newer_format = cover.with_extension("jpg");
        fs::write(&newer_format, &replacement).unwrap();
        restore(&bytes).unwrap();
        assert!(
            !cover.exists(),
            "A restored PNG must not displace a newer JPG"
        );
        assert_eq!(fs::read(&newer_format).unwrap(), replacement);
        assert_eq!(fs::read(&path).unwrap(), b"synthetic game data");
    }
}
