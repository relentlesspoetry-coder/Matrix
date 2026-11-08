//! Explicit Steam roots retain their launcher even when a drive is disconnected.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RegisteredLibrary {
    pub path: String,
    pub launcher: SteamLauncher,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryStatus {
    path: String,
    launcher: SteamLauncher,
    available: bool,
}
fn statuses(entries: Vec<RegisteredLibrary>) -> Vec<LibraryStatus> {
    entries
        .into_iter()
        .map(|entry| LibraryStatus {
            available: Path::new(&entry.path).is_dir(),
            path: entry.path,
            launcher: entry.launcher,
        })
        .collect()
}
const MAX_CUSTOM_ROOTS: usize = 32;
pub(crate) const MAX_LIBRARIES: usize = 128;

fn normalize_root(path: &Path) -> Result<PathBuf, String> {
    let selected_steamapps = path.file_name().is_some_and(|name| name == "steamapps");
    let path = path
        .canonicalize()
        .map_err(|_| "Choose an existing Steam library folder.")?;
    let root = if selected_steamapps || path.file_name().is_some_and(|name| name == "steamapps") {
        path
    } else {
        path.join("steamapps")
    };
    if !root.is_dir() {
        return Err("Choose a steamapps folder or its parent Steam library folder.".into());
    }
    root.canonicalize().map_err(|e| e.to_string())
}
fn add_to_settings(
    settings: &mut Settings,
    path: &Path,
    launcher: SteamLauncher,
) -> Result<(), String> {
    let path = actionable_path(&normalize_root(path)?)?;
    if let Some(existing) = settings
        .steam_libraries
        .iter_mut()
        .find(|entry| entry.path == path)
    {
        existing.launcher = launcher;
    } else {
        if settings.steam_libraries.len() >= MAX_CUSTOM_ROOTS {
            return Err("At most 32 custom Steam libraries can be registered.".into());
        }
        settings
            .steam_libraries
            .push(RegisteredLibrary { path, launcher });
    }
    Ok(())
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn list_steam_libraries() -> Result<Vec<LibraryStatus>, String> {
    Ok(statuses(load_settings()?.steam_libraries))
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn add_steam_library(
    path: String,
    launcher: SteamLauncher,
) -> Result<Vec<LibraryStatus>, String> {
    update_settings(|settings| {
        add_to_settings(settings, Path::new(&path), launcher)?;
        Ok(statuses(settings.steam_libraries.clone()))
    })
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn remove_steam_library(path: String) -> Result<Vec<LibraryStatus>, String> {
    update_settings(|settings| {
        // Match the saved string: removal must work while the drive is offline.
        settings.steam_libraries.retain(|entry| entry.path != path);
        Ok(statuses(settings.steam_libraries.clone()))
    })
}
pub(crate) fn bounded_text(path: &Path) -> Option<String> {
    bounded_io::read_text(path, 1_048_576)
}

// Snap aliases named `steam` are often symlinks to the snap multiplexer.
// A native registration must not silently dispatch through that other client.
pub(crate) fn native_executable_in(paths: &[PathBuf]) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    paths
        .iter()
        .map(|directory| directory.join("steam"))
        .find(|path| {
            let Ok(target) = path.canonicalize() else {
                return false;
            };
            !path.starts_with("/snap")
                && !target.starts_with("/snap")
                && target.file_name().is_none_or(|name| name != "snap")
                && fs::metadata(path).is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
        })
}
pub(crate) fn command_for_launch(uid: &str, launcher: SteamLauncher) -> Result<Command, String> {
    if launcher != SteamLauncher::Native {
        return Ok(steam_command(uid, launcher));
    }
    let paths = std::env::var_os("PATH").unwrap_or_default();
    let executable = native_executable_in(&std::env::split_paths(&paths).collect::<Vec<_>>())
        .ok_or("Native Steam was not found. Install native Steam or select the correct Flatpak/Snap client for this library.")?;
    let mut command = launch_command(executable);
    command.args(["-applaunch", uid]);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_rejects_non_utf8_parent_and_symlink_targets() {
        use std::os::unix::ffi::OsStringExt;
        let temp = tempfile::tempdir().unwrap();
        let invalid = temp.path().join(std::ffi::OsString::from_vec(b"Steam-\xfe".to_vec()));
        let lookalike = temp.path().join("Steam-\u{fffd}");
        fs::create_dir_all(invalid.join("steamapps")).unwrap();
        fs::create_dir_all(lookalike.join("steamapps")).unwrap();
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&invalid, &alias).unwrap();
        let mut settings = Settings::default();
        for path in [&invalid, &alias] {
            assert!(add_to_settings(&mut settings, path, SteamLauncher::Flatpak)
                .unwrap_err().contains("non-UTF-8"));
            assert!(settings.steam_libraries.is_empty());
        }
        add_to_settings(&mut settings, &lookalike, SteamLauncher::Native).unwrap();
        assert_eq!(settings.steam_libraries[0].path,
            lookalike.join("steamapps").to_str().unwrap());
        assert!(invalid.join("steamapps").is_dir());
    }

    #[test]
    fn registration_normalizes_updates_and_preserves_other_settings() {
        let root = tempfile::tempdir().unwrap();
        let steamapps = root.path().join("steamapps");
        fs::create_dir(&steamapps).unwrap();
        let mut settings = Settings::default();
        settings.favorites.insert("steam".into(), vec!["42".into()]);
        add_to_settings(&mut settings, root.path(), SteamLauncher::Snap).unwrap();
        add_to_settings(&mut settings, &steamapps, SteamLauncher::Flatpak).unwrap();
        assert_eq!(settings.steam_libraries.len(), 1);
        assert_eq!(
            settings.steam_libraries[0].path,
            steamapps.to_string_lossy()
        );
        assert_eq!(settings.steam_libraries[0].launcher, SteamLauncher::Flatpak);
        assert_eq!(settings.favorites["steam"], ["42"]);
        let roundtrip: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            roundtrip.steam_libraries[0].launcher,
            SteamLauncher::Flatpak
        );
        assert!(serde_json::from_str::<Settings>("{}")
            .unwrap()
            .steam_libraries
            .is_empty());
        assert!(add_to_settings(
            &mut settings,
            &root.path().join("missing"),
            SteamLauncher::Native
        )
        .is_err());
        assert!(serde_json::from_str::<SteamLauncher>("\"unknown\"").is_err());
    }
    fn manifest(root: &Path, id: &str) {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join(format!("appmanifest_{id}.acf")),
            format!("\"appid\" \"{id}\"\n\"name\" \"Game {id}\"\n"),
        )
        .unwrap();
    }
    #[test]
    fn snap_roots_external_libraries_and_legacy_aliases_keep_snap_route() {
        let root = tempfile::tempdir().unwrap();
        let snap = root
            .path()
            .join("snap/steam/common/.local/share/Steam/steamapps");
        let external = root.path().join("External/steamapps");
        manifest(&snap, "1");
        manifest(&external, "2");
        fs::write(
            snap.join("libraryfolders.vdf"),
            format!("\"path\" \"{}\"\n", external.parent().unwrap().display()),
        )
        .unwrap();
        fs::create_dir_all(root.path().join(".steam")).unwrap();
        std::os::unix::fs::symlink(snap.parent().unwrap(), root.path().join(".steam/root"))
            .unwrap();
        let games = scan_steam_in(root.path(), &[]);
        assert_eq!(games.len(), 2);
        assert!(games
            .iter()
            .all(|game| game.launcher == SteamLauncher::Snap));
        let command = steam_command("2", SteamLauncher::Snap);
        assert_eq!(command.get_program(), "snap");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["run", "steam", "-applaunch", "2"]
        );
    }
    #[test]
    fn explicit_custom_route_wins_without_changing_game_identity() {
        let root = tempfile::tempdir().unwrap();
        let library = root.path().join(".local/share/Steam/steamapps");
        manifest(&library, "42");
        let mut settings = Settings::default();
        add_to_settings(&mut settings, &library, SteamLauncher::Flatpak).unwrap();
        let games = scan_steam_in(root.path(), &settings.steam_libraries);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].game.uid, "42");
        assert_eq!(games[0].launcher, SteamLauncher::Flatpak);
        fs::rename(&library, library.with_file_name("offline")).unwrap();
        assert!(!statuses(settings.steam_libraries.clone())[0].available);
        assert!(scan_steam_in(root.path(), &settings.steam_libraries).is_empty());
        assert_eq!(settings.steam_libraries.len(), 1);
    }
    #[test]
    fn custom_roots_have_a_limit_and_do_not_create_directories() {
        let root = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        for i in 0..MAX_CUSTOM_ROOTS {
            let library = root.path().join(i.to_string()).join("steamapps");
            fs::create_dir_all(&library).unwrap();
            add_to_settings(&mut settings, &library, SteamLauncher::Native).unwrap();
        }
        let extra = root.path().join("extra/steamapps");
        fs::create_dir_all(&extra).unwrap();
        assert!(add_to_settings(&mut settings, &extra, SteamLauncher::Native).is_err());
        let missing = root.path().join("missing");
        assert!(add_to_settings(&mut settings, &missing, SteamLauncher::Native).is_err());
        assert!(!missing.exists());
    }
    #[test]
    fn native_route_skips_snap_alias_and_does_not_fall_back() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let aliases = root.path().join("aliases");
        let native = root.path().join("native");
        fs::create_dir_all(&aliases).unwrap();
        fs::create_dir_all(&native).unwrap();
        let snap = root.path().join("snap");
        fs::write(&snap, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&snap, fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink(&snap, aliases.join("steam")).unwrap();
        assert!(native_executable_in(&[aliases.clone()]).is_none());
        let steam = native.join("steam");
        fs::write(&steam, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&steam, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(native_executable_in(&[aliases, native]), Some(steam));
    }
    #[test]
    fn steam_text_reads_are_bounded() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("libraryfolders.vdf");
        fs::write(&path, vec![b'x'; 1_048_577]).unwrap();
        assert!(bounded_text(&path).is_none());
        fs::write(&path, "small").unwrap();
        assert_eq!(bounded_text(&path).as_deref(), Some("small"));
    }
}
