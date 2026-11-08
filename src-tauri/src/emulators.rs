use super::*;
use std::io::Read;

#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Launcher {
    #[default]
    Executable,
    Appimage,
    Flatpak,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlatpakApp {
    pub app_id: String,
    pub name: String,
    pub installation: String,
}
fn parse_flatpaks(text: &str) -> Vec<FlatpakApp> {
    let mut apps = BTreeMap::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 {
            continue;
        }
        let (id, name, installation) = (fields[0].trim(), fields[1].trim(), fields[2].trim());
        if id.len() > 255
            || id.split('.').any(|p| p.is_empty())
            || id.split('.').count() < 3
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || installation.is_empty()
        {
            continue;
        }
        apps.insert(
            (
                name.to_lowercase(),
                id.to_string(),
                installation.to_string(),
            ),
            FlatpakApp {
                app_id: id.into(),
                name: name.into(),
                installation: installation.into(),
            },
        );
    }
    apps.into_values().collect()
}
pub(crate) fn list_flatpaks() -> Result<Vec<FlatpakApp>, String> {
    let output = launch_command("flatpak").args(["list","--app","--columns=application,name,installation"]).env("LC_ALL","C").output().map_err(|_|"Flatpak was not found. Install your emulator through Software first, or choose an AppImage / executable.")?;
    if !output.status.success() {
        return Err("Could not read installed Flatpak apps. Check that Flatpak works in Software, then try Reload apps.".into());
    }
    Ok(parse_flatpaks(&String::from_utf8_lossy(&output.stdout)))
}
fn installed(profile: &EmulatorProfile) -> Result<(), String> {
    if list_flatpaks()?.iter().any(|app| {
        app.app_id == profile.flatpak_id && app.installation == profile.flatpak_installation
    }) {
        Ok(())
    } else {
        Err("This Flatpak is no longer installed in the selected installation. Choose it again in emulator setup.".into())
    }
}
fn file_path_error(path: &str, error: std::io::Error) -> String {
    let detail = match error.kind() {
        std::io::ErrorKind::NotFound => "The selected emulator file is no longer available. It may have moved or its drive may be disconnected.",
        std::io::ErrorKind::PermissionDenied => "Matrix Play cannot access the selected emulator file. Check access to the file and its containing folders.",
        _ => "Matrix Play could not resolve the selected emulator file.",
    };
    format!("{detail} Selected path: {path}. Use Browse to choose the current file, or Refresh detected to scan again. ({error})")
}
fn validate_file(path: &str, appimage: bool, allow_execute: bool) -> Result<String, String> {
    let path = Path::new(path)
        .canonicalize()
        .map_err(|error| file_path_error(path, error))?;
    if !path.is_file() {
        return Err("Choose an emulator file, not a folder.".into());
    }
    if appimage {
        let mut header = [0; 11];
        fs::File::open(&path)
            .and_then(|mut f| f.read_exact(&mut header))
            .map_err(|_| "This file is not a recognizable AppImage.")?;
        if !header.starts_with(b"\x7fELF")
            || !(&header[8..11] == b"AI\x01" || &header[8..11] == b"AI\x02")
        {
            return Err(
                "This file is not a recognizable AppImage. Choose the downloaded .AppImage file."
                    .into(),
            );
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&path)
            .map_err(|e| e.to_string())?
            .permissions();
        if permissions.mode() & 0o111 == 0 {
            if appimage && allow_execute {
                permissions.set_mode(permissions.mode() | 0o100);
                fs::set_permissions(&path,permissions).map_err(|_|"Cannot allow this AppImage to run. Choose a file you own, or change its permissions in Files.")?;
            } else {
                return Err("This file is not executable. For an AppImage, enable ‘Allow this AppImage to run’.".into());
            }
        }
    }
    Ok(path.to_string_lossy().into_owned())
}
pub(crate) fn prepare(
    mut profile: EmulatorProfile,
    allow_execute: bool,
) -> Result<EmulatorProfile, String> {
    if profile.emulator_name.trim().is_empty() {
        return Err("Enter an emulator name.".into());
    }
    match profile.launcher {
        Launcher::Flatpak => {
            installed(&profile)?;
            profile.emulator_path.clear();
            profile.extract_appimage = false;
        }
        _ => {
            profile.emulator_path = validate_file(
                &profile.emulator_path,
                profile.launcher == Launcher::Appimage,
                allow_execute,
            )?;
            profile.flatpak_id.clear();
            profile.flatpak_installation.clear();
        }
    }
    Ok(profile)
}
// These identified engines need ROM-adjacent save writes by default. Do not grant
// write access for an arbitrary display name or for Open emulator/setup.
fn flatpak_game_access(
    profile: &EmulatorProfile,
    id: &str,
    folder: &Path,
    game: Option<&Path>,
) -> Result<Vec<String>, String> {
    let folder = folder.canonicalize().map_err(|e| e.to_string())?;
    let mut args = vec![format!("--filesystem={}:ro", folder.display())];
    let sidecar_saves = matches!(
        (profile.flatpak_id.as_str(), id),
        ("io.github.sameboy.SameBoy", "gb")
            | ("io.mgba.mGBA", "gb" | "gba")
            | ("net.kuribo64.melonDS", "nds")
    );
    if let Some(game) = game.filter(|_| sidecar_saves) {
        let game = game.canonicalize().map_err(|e| e.to_string())?;
        let parent = game.parent().ok_or("Invalid game path")?;
        if !game.is_file() || !parent.starts_with(&folder) {
            return Err("Game is outside this console's library".into());
        }
        // Flatpak needs directory writes to create/rename save and state files.
        // Grant just this game's containing directory, never the full Matrix root.
        if parent == folder {
            args.clear(); // Do not pass conflicting modes for the same mount.
        }
        args.push(format!("--filesystem={}:rw", parent.display()));
    }
    Ok(args)
}
pub(crate) fn command(
    profile: &EmulatorProfile,
    id: &str,
    game: Option<&Path>,
) -> Result<Command, String> {
    immersive::validate_profile_paths(profile, id)?;
    match profile.launcher {
        Launcher::Flatpak => {
            installed(profile)?;
            let mut c = launch_command("flatpak");
            c.arg("run");
            match profile.flatpak_installation.as_str() {
                "user" => {
                    c.arg("--user");
                }
                "system" => {
                    c.arg("--system");
                }
                other => {
                    c.arg(format!("--installation={other}"));
                }
            }
            let folder = match game {
                Some(path) => libraries::game_root(id, path)?,
                None => games_dir(id)?,
            };
            c.args(flatpak_game_access(profile, id, &folder, game)?);
            if profile
                .flatpak_runtime_fonts
                .unwrap_or(profile.flatpak_id == "net.pcsx2.PCSX2")
            {
                let root = dirs::data_local_dir()
                    .ok_or("Cannot locate Matrix Play data folder")?
                    .join("matrix/compat");
                fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                let path = root.join("runtime-fonts-v1.conf");
                let config = include_str!("runtime-fonts.conf");
                if fs::read_to_string(&path).ok().as_deref() != Some(config) {
                    fs::write(&path, config).map_err(|e| e.to_string())?;
                }
                c.arg(format!("--filesystem={}:ro", path.display()))
                    .arg(format!("--env=FONTCONFIG_FILE={}", path.display()));
            }
            c.arg(&profile.flatpak_id);
            Ok(c)
        }
        _ => {
            let path = validate_file(
                &profile.emulator_path,
                profile.launcher == Launcher::Appimage,
                false,
            )?;
            let mut c = launch_command(path);
            if profile.launcher == Launcher::Appimage && profile.extract_appimage {
                c.env("APPIMAGE_EXTRACT_AND_RUN", "1");
            }
            Ok(c)
        }
    }
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) async fn browse_emulator(
    app: tauri::AppHandle,
    appimage: bool,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        let mut picker = app.dialog().file().set_title("Choose emulator");
        if appimage {
            picker = picker.add_filter("AppImage", &["AppImage", "appimage"]);
        }
        picker
            .blocking_pick_file()
            .map(|file| {
                file.into_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|e| e.to_string())
            })
            .transpose()
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidecar_save_access_is_limited_to_recognized_game_parent() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("GB Games");
        let nested = folder.join("One Game");
        fs::create_dir_all(&nested).unwrap();
        let game = nested.join("Game.gbc");
        fs::write(&game, b"fixture").unwrap();
        let mut profile = EmulatorProfile {
            flatpak_id: "io.github.sameboy.SameBoy".into(),
            ..Default::default()
        };
        let readonly = format!("--filesystem={}:ro", folder.display());
        assert_eq!(
            flatpak_game_access(&profile, "gb", &folder, Some(&game)).unwrap(),
            [
                readonly.clone(),
                format!("--filesystem={}:rw", nested.display())
            ]
        );
        // Setup gets no save write access; unrelated engines/consoles stay read-only.
        assert_eq!(
            flatpak_game_access(&profile, "gb", &folder, None).unwrap(),
            [readonly.clone()]
        );
        assert_eq!(
            flatpak_game_access(&profile, "ps2", &folder, Some(&game)).unwrap(),
            [readonly.clone()]
        );
        profile.flatpak_id = "org.example.Custom".into();
        assert_eq!(
            flatpak_game_access(&profile, "gb", &folder, Some(&game)).unwrap(),
            [readonly.clone()]
        );
        profile.flatpak_id = "io.mgba.mGBA".into();
        let direct = folder.join("Direct.gba");
        fs::write(&direct, b"fixture").unwrap();
        assert_eq!(
            flatpak_game_access(&profile, "gba", &folder, Some(&direct)).unwrap(),
            [format!("--filesystem={}:rw", folder.display())]
        );
        let outside = root.path().join("Outside.gba");
        fs::write(&outside, b"fixture").unwrap();
        assert!(flatpak_game_access(&profile, "gba", &folder, Some(&outside)).is_err());
        assert!(flatpak_game_access(&profile, "gba", &folder, Some(&nested)).is_err());
        std::os::unix::fs::symlink(&outside, folder.join("Escape.gba")).unwrap();
        assert!(
            flatpak_game_access(&profile, "gba", &folder, Some(&folder.join("Escape.gba")))
                .is_err()
        );
    }
    #[test]
    fn melonds_gets_scoped_save_access_only_for_nds_game_launches() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("NDS Games");
        let nested = folder.join("One Game");
        fs::create_dir_all(&nested).unwrap();
        let game = nested.join("Game.nds");
        fs::write(&game, b"fixture").unwrap();
        let profile = EmulatorProfile {
            flatpak_id: "net.kuribo64.melonDS".into(),
            ..Default::default()
        };
        let readonly = format!("--filesystem={}:ro", folder.display());
        assert_eq!(
            flatpak_game_access(&profile, "nds", &folder, Some(&game)).unwrap(),
            [
                readonly.clone(),
                format!("--filesystem={}:rw", nested.display())
            ]
        );
        assert_eq!(
            flatpak_game_access(&profile, "nds", &folder, None).unwrap(),
            [readonly.clone()]
        );
        assert_eq!(
            flatpak_game_access(&profile, "gba", &folder, Some(&game)).unwrap(),
            [readonly]
        );
        let direct = folder.join("Direct.nds");
        fs::write(&direct, b"fixture").unwrap();
        assert_eq!(
            flatpak_game_access(&profile, "nds", &folder, Some(&direct)).unwrap(),
            [format!("--filesystem={}:rw", folder.display())]
        );
        let outside = root.path().join("Outside.nds");
        fs::write(&outside, b"fixture").unwrap();
        assert!(flatpak_game_access(&profile, "nds", &folder, Some(&outside)).is_err());
        std::os::unix::fs::symlink(&outside, folder.join("Escape.nds")).unwrap();
        assert!(
            flatpak_game_access(&profile, "nds", &folder, Some(&folder.join("Escape.nds")))
                .is_err()
        );
    }
    #[test]
    fn discovery_keeps_installations_and_rejects_bad_ids() {
        let apps=parse_flatpaks("org.example.Emu\tEmulator\tuser\norg.example.Emu\tEmulator\tsystem\n--command=evil\tBad\tuser\nbroken\n");
        assert_eq!(apps.len(), 2);
        assert_ne!(apps[0].installation, apps[1].installation);
    }
    #[test]
    fn appimage_permission_requires_explicit_consent_and_valid_header() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("An Emulator.AppImage");
        fs::write(&image, b"\x7fELF0000AI\x02synthetic image").unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(validate_file(image.to_str().unwrap(), true, false).is_err());
        validate_file(image.to_str().unwrap(), true, true).unwrap();
        assert_eq!(
            fs::metadata(&image).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::write(&image, b"#!/bin/sh\nnot an AppImage").unwrap();
        assert!(validate_file(image.to_str().unwrap(), true, true).is_err());
    }
    #[test]
    fn inaccessible_emulator_path_is_not_reported_as_a_missing_installation() {
        let denied = file_path_error(
            "/private/shadPS4.AppImage",
            std::io::ErrorKind::PermissionDenied.into(),
        );
        assert!(denied.contains("cannot access"));
        assert!(denied.contains("/private/shadPS4.AppImage"));
        assert!(!denied.contains("no longer available"));
        let other = file_path_error("/apps/emulator", std::io::ErrorKind::InvalidInput.into());
        assert!(other.contains("could not resolve"));
        assert!(other.contains("Browse"));
    }

    #[test]
    fn legacy_profile_stays_executable() {
        let profile:EmulatorProfile=serde_json::from_str(r#"{"emulatorName":"Old emulator","emulatorPath":"/usr/bin/emu","launchArgs":["{game}"]}"#).unwrap();
        assert!(profile.launcher == Launcher::Executable);
    }
}
