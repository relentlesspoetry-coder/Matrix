//! Explicit, bounded save-folder backups. Restore previews are held in memory.
use super::*;
use std::io::{Read, Write};
use std::sync::{LazyLock, Mutex};
const LIMIT: usize = 192 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
struct SaveFile {
    name: String,
    bytes: String,
}
#[derive(Serialize, Deserialize)]
struct Archive {
    format: u32,
    kind: String,
    files: Vec<SaveFile>,
}
struct Plan {
    token: String,
    root: PathBuf,
    archive: Archive,
}
static PENDING: LazyLock<Mutex<Option<Plan>>> = LazyLock::new(|| Mutex::new(None));
fn supported(id: &str) -> Result<(), String> {
    spec(id)?;
    let s = load_settings()?;
    let p = s.profiles.get(id).ok_or("Configure an emulator first")?;
    let name = format!("{} {} {}", p.emulator_name, p.emulator_path, p.flatpak_id).to_lowercase();
    if !["pcsx2", "dolphin", "ppsspp"]
        .iter()
        .any(|n| name.contains(n))
    {
        return Err("Save backups currently support PCSX2, Dolphin and PPSSPP profiles".into());
    }
    Ok(())
}
fn root(path: &Path) -> Result<PathBuf, String> {
    if path
        .symlink_metadata()
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Choose the real save folder, not a symlink".into());
    }
    let p = path.canonicalize().map_err(|e| e.to_string())?;
    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !p.is_dir()
        || ![
            "memcards",
            "sstates",
            "gc",
            "statesaves",
            "savedata",
            "ppsspp_state",
        ]
        .contains(&name.as_str())
    {
        return Err(
            "Choose a save folder: memcards, sstates, GC, StateSaves, SAVEDATA or PPSSPP_STATE"
                .into(),
        );
    }
    Ok(p)
}
fn capture(kind: &str, root: &Path) -> Result<Archive, String> {
    let mut files = Vec::new();
    let mut total = 0u64;
    for item in WalkDir::new(root).follow_links(false).max_depth(20) {
        let e = item.map_err(|e| e.to_string())?;
        if e.file_type().is_dir() {
            continue;
        }
        if !e.file_type().is_file() {
            return Err(
                "Save folder contains a symlink or special file; choose a regular save folder"
                    .into(),
            );
        }
        if files.len() >= 10000 {
            return Err("Save folder exceeds 10,000 files".into());
        }
        let len = e.metadata().map_err(|e| e.to_string())?.len();
        if len > 32 * 1024 * 1024 || total > 128 * 1024 * 1024 {
            return Err("Save backup limit: 128 MB total and 32 MB per file".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(e.path())
            .map_err(|e| e.to_string())?
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        total += bytes.len() as u64 + e.path().as_os_str().len() as u64;
        if bytes.len() > 32 * 1024 * 1024 || total > 128 * 1024 * 1024 {
            return Err("Save files grew beyond backup limits".into());
        }
        files.push(SaveFile {
            name: e
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .ok_or("Save filename is not UTF-8")?
                .into(),
            bytes: BASE64.encode(bytes),
        });
    }
    Ok(Archive {
        format: 1,
        kind: kind.into(),
        files,
    })
}
fn validated(archive: &Archive) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    if archive.format != 1 || archive.files.len() > 10000 {
        return Err("Invalid save backup".into());
    }
    spec(&archive.kind)?;
    let mut total = 0;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for f in &archive.files {
        let p = PathBuf::from(&f.name);
        if p.as_os_str().is_empty()
            || p.components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || !seen.insert(p.clone())
        {
            return Err("Unsafe or duplicate save filename".into());
        }
        let bytes = BASE64.decode(&f.bytes).map_err(|_| "Invalid save bytes")?;
        total += bytes.len();
        if bytes.len() > 32 * 1024 * 1024 || total > 128 * 1024 * 1024 {
            return Err("Save backup exceeds size limits".into());
        }
        out.push((p, bytes));
    }
    Ok(out)
}
fn target(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut p = root.to_owned();
    for c in relative.components() {
        p.push(c);
        match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err("Restore path contains a symlink".into())
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(p)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid save path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut f = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    f.write_all(bytes).map_err(|e| e.to_string())?;
    f.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn idle(kind: &str) -> Result<(), String> {
    if launches::get_launch_status(kind.into())?
        .is_some_and(|i| ["running", "starting"].contains(&i.phase.as_str()))
    {
        return Err("Close this console's game before backing up or restoring saves".into());
    }
    Ok(())
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) async fn save_data_dialog(
    app: tauri::AppHandle,
    id: String,
    restore: bool,
) -> Result<serde_json::Value, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move||{supported(&id)?;idle(&id)?;
 let folder=app.dialog().file().set_title("Close your emulator, then choose its save folder").blocking_pick_folder();let Some(folder)=folder else{return Ok(serde_json::json!({"cancelled":true}));};let root=root(&folder.into_path().map_err(|e|e.to_string())?)?;
 if restore{let file=app.dialog().file().add_filter("Matrix save backup",&["json"]).blocking_pick_file();let Some(file)=file else{return Ok(serde_json::json!({"cancelled":true}));};let mut bytes=Vec::new();fs::File::open(file.into_path().map_err(|e|e.to_string())?).map_err(|e|e.to_string())?.take((LIMIT+1)as u64).read_to_end(&mut bytes).map_err(|e|e.to_string())?;if bytes.len()>LIMIT{return Err("Save backup exceeds 192 MB".into());}let archive:Archive=serde_json::from_slice(&bytes).map_err(|_|"Invalid save backup")?;if archive.kind!=id{return Err("This backup belongs to a different console".into());}let files=validated(&archive)?;for(p,_)in &files{target(&root,p)?;}
 let token=format!("{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());let result=serde_json::json!({"token":token,"root":root.to_string_lossy(),"files":files.iter().map(|(p,b)|serde_json::json!({"name":p.to_string_lossy(),"bytes":b.len(),"replaces":root.join(p).exists()})).collect::<Vec<_>>()});*PENDING.lock().map_err(|_|"Restore state unavailable")?=Some(Plan{token,root,archive});Ok(result)
 }else{let archive=capture(&id,&root)?;let file=app.dialog().file().set_file_name("matrix-save-backup.json").add_filter("Matrix save backup",&["json"]).blocking_save_file();let Some(file)=file else{return Ok(serde_json::json!({"cancelled":true}));};let path=file.into_path().map_err(|e|e.to_string())?;if path.starts_with(&root)||path==config_file()?||path.extension().and_then(|s|s.to_str())!=Some("json"){return Err("Choose a separate .json file outside the save folder".into());}atomic(&path,&serde_json::to_vec(&archive).map_err(|e|e.to_string())?)?;Ok(serde_json::json!({"message":format!("Backed up {} save files from {}",archive.files.len(),root.display())}))}
 }).await.map_err(|e|e.to_string())?
}
pub(crate) fn apply_restore_impl(token: String) -> Result<String, String> {
    let mut pending = PENDING.lock().map_err(|_| "Restore state unavailable")?;
    let plan = pending
        .as_ref()
        .filter(|p| p.token == token)
        .ok_or("Restore preview expired; choose the backup again")?;
    supported(&plan.archive.kind)?;
    idle(&plan.archive.kind)?;
    let checked_root = root(&plan.root)?;
    let files = validated(&plan.archive)?;
    for (p, _) in &files {
        target(&checked_root, p)?;
    }
    let previous = capture(&plan.archive.kind, &checked_root)?;
    let safety = config_file()?.parent().unwrap().join("save-backups");
    fs::create_dir_all(&safety).map_err(|e| e.to_string())?;
    let mut backup = tempfile::Builder::new()
        .prefix("before-restore-")
        .suffix(".json")
        .tempfile_in(&safety)
        .map_err(|e| e.to_string())?;
    backup
        .write_all(&serde_json::to_vec(&previous).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let (_, safety) = backup.keep().map_err(|e| e.to_string())?;
    let mut written: Vec<(PathBuf, Option<Vec<u8>>)> = Vec::new();
    for (p, b) in files {
        let path = target(&checked_root, &p)?;
        if let Err(error) = atomic(&path, &b) {
            let mut errors = Vec::new();
            for (p, old) in written.into_iter().rev() {
                let result = if let Some(old) = old {
                    atomic(&p, &old)
                } else {
                    fs::remove_file(p).map_err(|e| e.to_string())
                };
                if let Err(e) = result {
                    errors.push(e);
                }
            }
            return Err(format!(
                "Restore failed: {error}. Rollback errors: {errors:?}. Safety backup: {}",
                safety.display()
            ));
        }
        written.push((
            path,
            previous
                .files
                .iter()
                .find(|f| Path::new(&f.name) == p)
                .map(|f| BASE64.decode(&f.bytes).unwrap()),
        ));
    }
    *pending = None;
    Ok(format!(
        "Saves restored. Previous saves are backed up at {}",
        safety.display()
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsafe_archives_are_rejected() {
        for name in ["../escape", "/absolute", "a/../../escape"] {
            assert!(validated(&Archive {
                format: 1,
                kind: "ps2".into(),
                files: vec![SaveFile {
                    name: name.into(),
                    bytes: BASE64.encode(b"test")
                }]
            })
            .is_err());
        }
    }
    #[test]
    fn capture_and_validate_preserves_bytes() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("card.ps2"), b"synthetic save").unwrap();
        let a = capture("ps2", d.path()).unwrap();
        assert_eq!(validated(&a).unwrap()[0].1, b"synthetic save");
    }
    #[test]
    fn restore_rejects_symlink_parents() {
        use std::os::unix::fs::symlink;
        let d = tempfile::tempdir().unwrap();
        symlink("/tmp", d.path().join("escape")).unwrap();
        assert!(target(d.path(), Path::new("escape/save")).is_err());
    }
}

#[cfg(test)]
pub(super) fn test_restore_roundtrip() {
    update_settings(|s| {
        s.profiles.insert(
            "psp".into(),
            EmulatorProfile {
                emulator_name: "PPSSPP".into(),
                ..Default::default()
            },
        );
        Ok(())
    })
    .unwrap();
    let save = dirs::home_dir().unwrap().join("SAVEDATA");
    fs::create_dir(&save).unwrap();
    fs::write(save.join("game.bin"), b"older save").unwrap();
    let archive = capture("psp", &save).unwrap();
    fs::write(save.join("game.bin"), b"newer save").unwrap();
    fs::write(save.join("unlisted.bin"), b"keep").unwrap();
    *PENDING.lock().unwrap() = Some(Plan {
        token: "fixture".into(),
        root: save.clone(),
        archive,
    });
    let result = apply_restore_impl("fixture".into()).unwrap();
    assert!(result.contains("backed up"));
    assert_eq!(fs::read(save.join("game.bin")).unwrap(), b"older save");
    assert_eq!(fs::read(save.join("unlisted.bin")).unwrap(), b"keep");
    assert!(apply_restore_impl("fixture".into()).is_err());
    fs::create_dir(save.join("occupied")).unwrap();
    *PENDING.lock().unwrap() = Some(Plan {
        token: "failure".into(),
        root: save.clone(),
        archive: Archive {
            format: 1,
            kind: "psp".into(),
            files: vec![
                SaveFile {
                    name: "game.bin".into(),
                    bytes: BASE64.encode(b"replacement"),
                },
                SaveFile {
                    name: "occupied".into(),
                    bytes: BASE64.encode(b"cannot replace directory"),
                },
            ],
        },
    });
    assert!(apply_restore_impl("failure".into()).is_err());
    assert_eq!(fs::read(save.join("game.bin")).unwrap(), b"older save");
    let backups = config_file()
        .unwrap()
        .parent()
        .unwrap()
        .join("save-backups");
    assert!(fs::read_dir(backups).unwrap().any(|entry| {
        let before: Archive =
            serde_json::from_slice(&fs::read(entry.unwrap().path()).unwrap()).unwrap();
        validated(&before)
            .unwrap()
            .iter()
            .any(|(p, b)| p == Path::new("game.bin") && b == b"newer save")
    }));
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) async fn apply_save_restore(token: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || apply_restore_impl(token))
        .await
        .map_err(|e| e.to_string())?
}
