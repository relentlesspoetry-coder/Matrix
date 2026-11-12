//! Explicit portable-emulator import. Publish a durable copy and profile before
//! removing the selected source; never overwrite a destination or move system files.
use super::*;
use std::{
    fs::OpenOptions,
    io::{Read, Seek},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Imported {
    pub path: String,
    pub moved: bool,
    pub message: String,
}
fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
}
fn private_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.file_type().is_symlink() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o002 != 0 {
        return Err("Emulator destination must be a real folder owned by your account and not writable by everyone.".into());
    }
    Ok(())
}
pub(crate) fn import(
    request: profile_saves::Request,
    id: &str,
    selected: &str,
) -> Result<Imported, String> {
    let s = spec(id)?;
    let input = Path::new(selected);
    if !input.is_absolute()
        || fs::symlink_metadata(input)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err(
            "Choose the actual portable emulator file, not a shortcut or symbolic link.".into(),
        );
    }
    let source = input.canonicalize().map_err(|e| e.to_string())?;
    actionable_path(&source)?;
    if [
        "/usr", "/bin", "/sbin", "/lib", "/lib64", "/opt", "/app", "/snap", "/var",
    ]
    .iter()
    .any(|p| source.starts_with(p))
    {
        return Err("Installed system emulators must stay in place. Use Set up to link the installed executable or Flatpak instead.".into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&source)
        .map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file()
        || before.uid() != unsafe { libc::geteuid() }
        || before.len() > 4 * 1024 * 1024 * 1024
    {
        return Err("Choose a portable emulator file you own, up to 4 GB.".into());
    }
    let mut magic = [0; 11];
    file.read_exact(&mut magic)
        .map_err(|_| "Choose a Linux emulator executable or AppImage.")?;
    let image = magic.starts_with(b"\x7fELF") && matches!(&magic[8..11], b"AI\x01" | b"AI\x02");
    if !(image || magic.starts_with(b"\x7fELF") || magic.starts_with(b"#!")) {
        return Err(
            "Choose a Linux AppImage or standalone executable, not an archive, installer or game."
                .into(),
        );
    }
    if source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("appimage"))
        && !image
    {
        return Err("This file is not a recognizable AppImage.".into());
    }
    if !image && !emulators::executable_file(&source) {
        return Err("This standalone executable is not runnable. Set its execute permission first, or choose an AppImage.".into());
    }
    // Check identity before making changes (for example, reject PCSX2 in Switch 2).
    discovery::profile_for_file(id, &source)?;
    let root = matrix_root()?;
    private_directory(&root)?;
    let destination_dir = root.join(s.dir);
    private_directory(&destination_dir)?;
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join(".emulator-import.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let destination = destination_dir.join(source.file_name().ok_or("Invalid filename")?);
    let already = destination == source;
    if !already && fs::symlink_metadata(&destination).is_ok() {
        return Err("An emulator with this filename already exists in this console folder. Nothing was replaced; rename the new file or choose the existing one.".into());
    }
    let temporary = if !already {
        let mut temp =
            tempfile::NamedTempFile::new_in(&destination_dir).map_err(|e| e.to_string())?;
        file.rewind().map_err(|e| e.to_string())?;
        let copied = std::io::copy(&mut file, &mut temp).map_err(|e| e.to_string())?;
        if copied != before.len()
            || !same_file(&before, &file.metadata().map_err(|e| e.to_string())?)
        {
            return Err("The selected emulator changed during import. Its original file was kept; try again.".into());
        }
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode((before.mode() & 0o755) | 0o700))
            .map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        // Same-filesystem hard link publishes without replacing any existing file.
        fs::hard_link(temp.path(), &destination)
            .map_err(|e| format!("Cannot publish emulator; the original was kept: {e}"))?;
        Some(temp)
    } else {
        None
    };
    let saved = (|| {
        fs::File::open(&destination_dir)
            .and_then(|d| d.sync_all())
            .map_err(|e| e.to_string())?;
        let mut profile = emulators::prepare(discovery::profile_for_file(id, &destination)?, true)?;
        profile_saves::commit(request, id, || {
            update_settings(|settings| {
                if let Some(old) = settings.profiles.get(id) {
                    let same_engine = discovery::identity(old, id)
                        .is_some_and(|engine| Some(engine) == discovery::identity(&profile, id));
                    if old.emulator_path == profile.emulator_path
                        || (old.launcher != emulators::Launcher::Flatpak && same_engine)
                    {
                        profile.launch_args = old.launch_args.clone();
                        profile.extract_appimage = profile.launcher
                            == emulators::Launcher::Appimage
                            && old.extract_appimage;
                    }
                }
                immersive::validate_profile_paths(&profile, id)?;
                settings.profiles.insert(id.into(), profile);
                // Adding a console makes it visible even if previously hidden manually.
                settings.library_preferences.hidden.retain(|v| v != id);
                Ok(())
            })
        })
    })();
    if let Err(error) = saved {
        if temporary.is_some() {
            let _ = fs::remove_file(&destination);
        }
        return Err(error);
    }
    let moved = already
        || (fs::symlink_metadata(&source).is_ok_and(|now| same_file(&before, &now))
            && fs::remove_file(&source).is_ok());
    // Best effort directory durability after unlink. The destination is already synced.
    if !already {
        if let Some(parent) = source.parent() {
            let _ = fs::File::open(parent).and_then(|d| d.sync_all());
        }
    }
    Ok(Imported{path:actionable_path(&destination)?,moved,message:if moved{"Emulator added and configured."}else{"Emulator added and configured. The original could not be removed and was kept in its original location."}.into()})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image(path: &Path) {
        fs::write(
            path,
            b"\x7fELF0000AI\x02synthetic portable file, never executed",
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn add(id: &str, path: &Path) -> Result<Imported, String> {
        import(profile_saves::begin().unwrap(), id, path.to_str().unwrap())
    }
    #[test]
    fn isolated_imports() {
        let temp = tempfile::tempdir().unwrap();
        let name = format!("{}::fixture", module_path!().split_once("::").unwrap().1);
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env("MATRIX_IMPORT_TEST", "1")
            .env("HOME", temp.path())
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .env("PATH", temp.path().join("bin"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    #[test]
    fn fixture() {
        if std::env::var_os("MATRIX_IMPORT_TEST").is_none() {
            return;
        }
        assert!(bootstrap().unwrap().needs_emulator_setup);
        let source = home().unwrap().join("Lichii-nx2.AppImage");
        image(&source);
        let original = fs::read(&source).unwrap();
        assert!(add("ps2", &source).is_err());
        assert_eq!(fs::read(&source).unwrap(), original);
        let result = add("switch2", &source).unwrap();
        assert!(result.moved);
        assert!(!source.exists());
        let target = PathBuf::from(&result.path);
        assert_eq!(
            target,
            matrix_root().unwrap().join("Switch 2/Lichii-nx2.AppImage")
        );
        assert_eq!(fs::read(&target).unwrap(), original);
        assert!(emulators::executable_file(&target));
        let profile = load_settings().unwrap().profiles["switch2"].clone();
        assert_eq!(discovery::identity(&profile, "switch2"), Some("Lichii"));
        assert_eq!(
            immersive::game_args(&profile, "switch2"),
            ["-f", "-g", "{game}"]
        );
        assert!(immersive::setup_args(&profile, "switch2").is_empty());
        assert_eq!(discovery::identity(&profile, "switch"), None);
        // Existing destination, symlink, invalid header and failed settings writes
        // must all retain the exact source and destination bytes.
        image(&source);
        assert!(add("switch2", &source).is_err());
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(fs::read(&target).unwrap(), original);
        let alias = home().unwrap().join("alias.AppImage");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        assert!(add("switch2", &alias).is_err());
        assert!(source.exists());
        let bad = home().unwrap().join("fake.AppImage");
        fs::write(&bad, b"not an executable").unwrap();
        assert!(add("switch2", &bad).is_err());
        assert!(bad.exists());
        let broken = home().unwrap().join("pcsx2-next.AppImage");
        image(&broken);
        let config = config_file().unwrap();
        let valid = fs::read(&config).unwrap();
        fs::write(&config, b"invalid JSON").unwrap();
        assert!(add("ps2", &broken).is_err());
        assert!(broken.exists());
        assert!(!matrix_root()
            .unwrap()
            .join("PS2/pcsx2-next.AppImage")
            .exists());
        fs::write(&config, valid).unwrap();
        update_settings(|s| {
            s.profiles.get_mut("switch2").unwrap().launch_args = vec![
                "--config".into(),
                "/keep/my settings".into(),
                "{game}".into(),
            ];
            Ok(())
        })
        .unwrap();
        assert!(add("switch2", &target).unwrap().moved);
        assert_eq!(fs::read(&target).unwrap(), original);
        assert_eq!(
            load_settings().unwrap().profiles["switch2"].launch_args,
            ["--config", "/keep/my settings", "{game}"]
        );
        // A newly placed, executable file is linked on the bounded managed scan.
        let dropped = matrix_root().unwrap().join("PS2/pcsx2-auto.AppImage");
        image(&dropped);
        fs::set_permissions(&dropped, fs::Permissions::from_mode(0o700)).unwrap();
        let poll = bootstrap_inventory(true).unwrap();
        assert!(poll.ready_profiles["ps2"]);
        assert_eq!(
            poll.profiles["ps2"].emulator_path,
            dropped.to_str().unwrap()
        );
        let game = games_dir("ps2").unwrap().join("New Game.iso");
        fs::write(&game, b"synthetic game, never executed").unwrap();
        assert_eq!(scan_console("ps2".into()).unwrap().len(), 1);
        library_features::set_library_preference(
            "setupCompleted".into(),
            "".into(),
            serde_json::json!(true),
        )
        .unwrap();
        library_features::set_library_preference(
            "hideUnconfigured".into(),
            "".into(),
            serde_json::json!(true),
        )
        .unwrap();
        assert!(!bootstrap().unwrap().needs_emulator_setup);
        assert!(load_settings()
            .unwrap()
            .library_preferences
            .hide_unconfigured
            .unwrap());
    }
}
