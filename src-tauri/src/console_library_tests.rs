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
        ("switch2", "custom-switch2-fixture", "NUP"),
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
        if id == "switch2" {
            assert!(validate_game("switch", &game.uid).is_err());
        }
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
        let expected = if id == "switch2" {
            vec![path.to_str().unwrap()]
        } else if id == "psp" {
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
    assert!(custom_emulator_only("ps5"));
    assert!(custom_emulator_only("switch2"));
    assert!(!custom_emulator_only("ps4"));
    assert!(!custom_emulator_only("switch"));
}

#[test]
fn cue_grouping_uses_scanned_regular_files_and_case_insensitive_extensions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let iso = root.join("Working.iso");
    let cue = root.join("Working.cue");
    fs::write(&iso, b"unchanged ISO").unwrap();
    fs::create_dir(&cue).unwrap();
    assert_eq!(
        scan_console_in("ps1", &[root.clone()]).unwrap()[0].path,
        iso.to_string_lossy()
    );
    fs::remove_dir(&cue).unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), &cue).unwrap();
    assert_eq!(
        scan_console_in("ps1", &[root.clone()]).unwrap()[0].path,
        iso.to_string_lossy()
    );
    fs::remove_file(&cue).unwrap();
    // A same-directory alias is not scanned either and must not hide the ISO.
    std::os::unix::fs::symlink(&iso, &cue).unwrap();
    assert_eq!(scan_console_in("ps1", &[root.clone()]).unwrap().len(), 1);
    fs::remove_file(&cue).unwrap();
    let upper = root.join("Working.CUE");
    fs::write(&upper, b"FILE \"Working.iso\" BINARY\n").unwrap();
    let games = scan_console_in("ps1", &[root.clone()]).unwrap();
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].path, upper.to_string_lossy());
    fs::rename(&iso, root.join("Working.ISO")).unwrap();
    assert_eq!(scan_console_in("ps1", &[root.clone()]).unwrap().len(), 1);
    // Distinct Linux stems and other consoles retain independent entries.
    fs::write(root.join("working.iso"), b"different case game").unwrap();
    assert_eq!(scan_console_in("ps1", &[root.clone()]).unwrap().len(), 2);
    assert_eq!(scan_console_in("ps2", &[root.clone()]).unwrap().len(), 2);
    assert_eq!(
        fs::read(root.join("Working.ISO")).unwrap(),
        b"unchanged ISO"
    );
}

#[test]
fn cue_grouping_preserves_customized_iso_identities() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("games");
    let covers = temp.path().join("covers");
    fs::create_dir(&root).unwrap();
    fs::create_dir_all(covers.join("ps1")).unwrap();
    let mut uids = vec![];
    for stem in ["Favorite", "Artwork", "Plain"] {
        let iso = root.join(format!("{stem}.ISO"));
        fs::write(&iso, b"original ISO").unwrap();
        fs::write(
            root.join(format!("{stem}.CuE")),
            format!("FILE \"{stem}.ISO\" BINARY\n"),
        )
        .unwrap();
        uids.push(iso.to_string_lossy().into_owned());
    }
    let favorites = HashSet::from([uids[0].clone()]);
    let cover = covers
        .join("ps1")
        .join(format!("{}.png", cover_key("ps1", &uids[1])));
    fs::write(&cover, b"original artwork").unwrap();
    let games =
        scan_console_with_customizations("ps1", &[root], &favorites, Some(&covers)).unwrap();
    assert_eq!(games.len(), 5);
    assert!(games.iter().any(|g| g.uid == uids[0]));
    assert!(games.iter().any(|g| g.uid == uids[1]));
    assert!(!games.iter().any(|g| g.uid == uids[2]));
    assert_eq!(fs::read(&cover).unwrap(), b"original artwork");
    assert_eq!(favorites, HashSet::from([uids[0].clone()]));
}

#[test]
fn large_iso_cue_collection_is_not_truncated_below_8000_games() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    for n in 0..4100 {
        let folder = root.join(format!("Game-{n:04}"));
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("disc.iso"), b"disc").unwrap();
        fs::write(
            folder.join("disc.cue"),
            "FILE \"disc.iso\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
        )
        .unwrap();
    }
    let rows = scan_console_in("ps1", &[root]).unwrap();
    assert_eq!(rows.len(),4100,"4100 logical games are below the 8000-game cap; companion tracks must not consume two game slots");
}

fn fifo_case_completes(mode: &str) {
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let fifo = match mode {
        "steam" => root.join(".local/share/Steam/steamapps/appmanifest_42.acf"),
        "discovery" => root.join(".local/share/applications/fixture.desktop"),
        _ => root.join(".var/app/net.pcsx2.PCSX2/config/PCSX2/inis/PCSX2.ini"),
    };
    fs::create_dir_all(fifo.parent().unwrap()).unwrap();
    assert!(Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let module = module_path!().split_once("::").unwrap().1;
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("{module}::fifo_reader_fixture"),
            "--nocapture",
        ])
        .env("MATRIX_FIFO_REPRO", mode)
        .env("MATRIX_FIFO_ROOT", root)
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !root.join("ready").exists() && Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "fixture startup failed");
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "fixture assertion failed");
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    panic!(
        "{mode} blocks while opening a FIFO that should be rejected as non-regular: {}",
        fifo.display()
    );
}
#[test]
fn steam_manifest_fifo_does_not_hang_scan() {
    fifo_case_completes("steam");
}
#[test]
fn desktop_metadata_fifo_does_not_hang_discovery() {
    fifo_case_completes("discovery");
}
#[test]
fn pcsx2_config_fifo_does_not_hang_launch_preflight() {
    fifo_case_completes("pcsx2");
}

#[test]
fn fifo_reader_fixture() {
    let Ok(mode) = std::env::var("MATRIX_FIFO_REPRO") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("MATRIX_FIFO_ROOT").unwrap());
    fs::write(root.join("ready"), b"ready").unwrap();
    match mode.as_str() {
        "steam" => {
            assert!(scan_steam_in(&root, &[]).is_empty());
        }
        "discovery" => {
            let _ = discovery::discover();
        }
        _ => {
            let p = EmulatorProfile {
                launcher: emulators::Launcher::Flatpak,
                flatpak_id: "net.pcsx2.PCSX2".into(),
                ..Default::default()
            };
            let _ = launches::preflight(&p);
        }
    }
}

#[test]
fn relative_dolphin_config_requires_explicit_absolute_repair() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let module = module_path!().split_once("::").unwrap().1;
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("{module}::relative_config_fixture"),
            "--nocapture",
        ])
        .env("MATRIX_RELATIVE_CONFIG_ROOT", root)
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn relative_config_fixture() {
    let Some(root) = std::env::var_os("MATRIX_RELATIVE_CONFIG_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    create_layout().unwrap();
    use std::os::unix::fs::PermissionsExt;
    let log = root.join("resolved-config.txt");
    std::env::set_var("MATRIX_RELATIVE_CONFIG_LOG", &log);
    let emulator = root.join("dolphin-emu");
    fs::write(&emulator,b"#!/bin/sh\nwhile [ \"$#\" -gt 0 ]; do\n if [ \"$1\" = \"-u\" ]; then shift; printf '%s/%s' \"$PWD\" \"$1\" > \"$MATRIX_RELATIVE_CONFIG_LOG\"; exit 0; fi\n shift\ndone\nexit 2\n").unwrap();
    fs::set_permissions(&emulator, fs::Permissions::from_mode(0o700)).unwrap();
    let external = root.join("External/GameCube");
    fs::create_dir_all(&external).unwrap();
    let game = external.join("Fixture.iso");
    fs::write(&game, b"synthetic disc").unwrap();
    libraries::add_library_folder("gamecube".into(), external.to_string_lossy().into()).unwrap();
    update_settings(|s| {
        s.profiles.insert(
            "gamecube".into(),
            EmulatorProfile {
                emulator_path: emulator.to_string_lossy().into(),
                launch_args: vec!["-u".into(), "MyProfile".into(), "{game}".into()],
                ..Default::default()
            },
        );
        Ok(())
    })
    .unwrap();
    let before = serde_json::to_value(load_settings().unwrap()).unwrap();
    let setup_error = open_emulator_impl("gamecube".into())
        .err()
        .expect("relative setup must be rejected");
    let game_error = launch_console_impl("gamecube".into(), game.to_string_lossy().into())
        .err()
        .expect("relative launch must be rejected");
    assert!(setup_error.contains("absolute path") && game_error.contains("absolute path"));
    assert!(!log.exists(), "No ambiguous profile may launch");
    let save = |args: Vec<String>| {
        save_emulator(
            "gamecube".into(),
            "Dolphin".into(),
            emulator.to_string_lossy().into(),
            args,
            Some("executable".into()),
            None,
            None,
            None,
            None,
            None,
        )
    };
    assert!(save(vec!["--user=MyProfile".into(), "{game}".into()])
        .unwrap_err()
        .contains("absolute path"));
    assert_eq!(
        serde_json::to_value(load_settings().unwrap()).unwrap(),
        before
    );
    assert_eq!(fs::read(&game).unwrap(), b"synthetic disc");
    // A user-selected absolute location is accepted unchanged for both modes.
    let chosen = root.join("Existing Dolphin User");
    fs::create_dir(&chosen).unwrap();
    fs::write(chosen.join("save.sav"), b"existing save").unwrap();
    save(vec![
        "-u".into(),
        chosen.to_string_lossy().into(),
        "{game}".into(),
    ])
    .unwrap();
    fs::write(&emulator, b"#!/bin/sh\nwhile [ \"$#\" -gt 0 ]; do\n if [ \"$1\" = \"-u\" ]; then shift; printf '%s' \"$1\" > \"$MATRIX_RELATIVE_CONFIG_LOG\"; exit 0; fi\n shift\ndone\nexit 2\n").unwrap();
    let wait = || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let status = launches::get_launch_status("gamecube".into())
                .unwrap()
                .unwrap();
            if status.phase == "exited" {
                break;
            }
            assert_ne!(status.phase, "error");
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    open_emulator_impl("gamecube".into()).unwrap();
    wait();
    assert_eq!(fs::read_to_string(&log).unwrap(), chosen.to_string_lossy());
    launch_console_impl("gamecube".into(), game.to_string_lossy().into()).unwrap();
    wait();
    assert_eq!(fs::read_to_string(&log).unwrap(), chosen.to_string_lossy());
    assert_eq!(fs::read(chosen.join("save.sav")).unwrap(), b"existing save");
}
