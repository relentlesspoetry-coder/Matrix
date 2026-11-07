use super::*;

#[test]
fn installed_metadata_and_new_formats_in_isolated_home() {
    let temp = tempfile::tempdir().unwrap();
    let (_, module) = module_path!()
        .split_once("::")
        .unwrap_or(("", "console_library_tests"));
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("{module}::library_fixture"),
            "--nocapture",
        ])
        .env("MATRIX_LIBRARY_FORMAT_FIXTURE", temp.path())
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("XDG_DATA_HOME", temp.path().join("data"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

#[test]
fn library_fixture() {
    let Some(root) = std::env::var_os("MATRIX_LIBRARY_FORMAT_FIXTURE") else {
        return;
    };
    use std::os::unix::fs::PermissionsExt;
    let root = PathBuf::from(root);
    create_layout().unwrap();
    let bin = root.join("Fixture Emulators");
    fs::create_dir(&bin).unwrap();
    let log = root.join("argv.log");
    std::env::set_var("MATRIX_LIBRARY_FORMAT_LOG", &log);
    for (id, engine, extension) in [
        ("psp", "ppsspp", "CHD"),
        ("gamecube", "dolphin-emu", "WIA"),
        ("gamecube", "dolphin-emu", "ciso"),
        ("wii", "dolphin-emu", "GCZ"),
        ("wii", "dolphin-emu", "ciso"),
    ] {
        let folder = root.join(format!("External {id} {extension}"));
        fs::create_dir(&folder).unwrap();
        let path = folder.join(format!("Game with spaces; literal.{extension}"));
        fs::write(&path, b"synthetic, not a playable disc").unwrap();
        libraries::add_library_folder(id.into(), folder.to_string_lossy().into()).unwrap();
        let game = scan_console_impl(id)
            .unwrap()
            .into_iter()
            .find(|g| Path::new(&g.path) == path)
            .unwrap();
        assert_eq!(validate_game(id, &game.uid).unwrap().title, game.title);
        favorites::set_favorite(id.into(), game.uid.clone(), true).unwrap();
        save_cover(
            id.into(),
            game.uid.clone(),
            include_bytes!("../../tests/fixtures/demo-cover.png").to_vec(),
        )
        .unwrap();
        assert!(get_cover(id.into(), game.uid.clone())
            .unwrap()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let executable = bin.join(engine);
        fs::write(
            &executable,
            b"#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$MATRIX_LIBRARY_FORMAT_LOG\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        update_settings(|settings| {
            settings.profiles.insert(
                id.into(),
                EmulatorProfile {
                    emulator_path: executable.to_string_lossy().into(),
                    ..Default::default()
                },
            );
            Ok(())
        })
        .unwrap();
        if log.exists() {
            fs::remove_file(&log).unwrap();
        }
        launch_console_impl(id.into(), game.uid.clone()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let status = launches::get_launch_status(id.into()).unwrap().unwrap();
            assert_ne!(status.phase, "error", "{}", status.message);
            if status.phase == "exited" {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "fixture launcher timed out: {}",
                status.phase
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let args = fs::read_to_string(&log).unwrap();
        let expected = if id == "psp" {
            vec!["--fullscreen", path.to_str().unwrap()]
        } else {
            vec![
                "-C",
                "Dolphin.Display.Fullscreen=True",
                "-b",
                "-e",
                path.to_str().unwrap(),
            ]
        };
        assert_eq!(args.lines().collect::<Vec<_>>(), expected);
        assert_eq!(fs::read(&path).unwrap(), b"synthetic, not a playable disc");
        libraries::remove_library_folder(id.into(), folder.to_string_lossy().into()).unwrap();
        assert!(validate_game(id, &game.uid).is_err());
        assert!(path.is_file());
        libraries::add_library_folder(id.into(), folder.to_string_lossy().into()).unwrap();
        assert!(load_settings().unwrap().favorites[id].contains(&game.uid));
        assert!(get_cover(id.into(), game.uid).unwrap().is_some());
    }
    assert!(!supported_console_file(
        spec("psp").unwrap(),
        Path::new("title.zso")
    ));
    assert!(!supported_console_file(
        spec("wiiu").unwrap(),
        Path::new("title.gcz")
    ));
    assert!(!supported_console_file(
        spec("gamecube").unwrap(),
        Path::new("title.chd")
    ));
    for (id, relative, metadata) in [
        ("ps3", "TITLE0001/USRDIR/EBOOT.BIN", "TITLE0001/PARAM.SFO"),
        ("ps4", "TITLE0002/eboot.bin", "TITLE0002/sce_sys/param.sfo"),
    ] {
        let folder = root.join(format!("Installed {id}"));
        let path = folder.join(relative);
        let sfo = folder.join(metadata);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::create_dir_all(sfo.parent().unwrap()).unwrap();
        fs::write(&path, b"synthetic eboot").unwrap();
        fs::write(&sfo, installed_titles::fixture("Installed Game [Edition]")).unwrap();
        libraries::add_library_folder(id.into(), folder.to_string_lossy().into()).unwrap();
        let game = scan_console_impl(id).unwrap().remove(0);
        assert_eq!(game.title, "Installed Game [Edition]");
        assert_eq!(validate_game(id, &game.uid).unwrap().title, game.title);
        favorites::set_favorite(id.into(), game.uid.clone(), true).unwrap();
        save_cover(
            id.into(),
            game.uid.clone(),
            include_bytes!("../../tests/fixtures/demo-cover.png").to_vec(),
        )
        .unwrap();
        fs::write(&sfo, installed_titles::fixture("Changed title")).unwrap();
        let changed = scan_console_impl(id).unwrap().remove(0);
        assert_eq!(changed.title, "Changed title");
        assert_eq!(changed.uid, game.uid);
        assert_eq!(changed.path, game.path);
        assert!(load_settings().unwrap().favorites[id].contains(&game.uid));
        assert!(get_cover(id.into(), game.uid.clone()).unwrap().is_some());
        fs::write(&sfo, b"bad optional metadata").unwrap();
        assert_eq!(
            validate_game(id, &game.uid).unwrap().title,
            if id == "ps3" {
                "TITLE0001"
            } else {
                "TITLE0002"
            }
        );
        assert_eq!(scan_console_impl(id).unwrap()[0].uid, game.uid);
        assert_eq!(fs::read(&path).unwrap(), b"synthetic eboot");
    }
    assert!(require_launch_adapter("ps5").is_err());
    assert!(require_launch_adapter("switch2").is_err());
}
