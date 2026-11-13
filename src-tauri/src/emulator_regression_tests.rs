use super::*;
use std::os::unix::fs::PermissionsExt;
fn image(path: &Path) {
    fs::write(path, b"\x7fELF0000AI\x02audit fixture, never executed").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
#[test]
fn shared_emulator_move_retains_both_profiles() {
    if std::env::var_os("MATRIX_EMULATOR_REGRESSION").is_none() {
        return;
    }
    create_layout().unwrap();
    let source = home().unwrap().join("Dolphin.AppImage");
    image(&source);
    let gc = discovery::profile_for_file("gamecube", &source).unwrap();
    let mut wii = discovery::profile_for_file("wii", &source).unwrap();
    wii.launch_args = vec![
        "--user".into(),
        "/keep/Wii settings".into(),
        "-b".into(),
        "-e".into(),
        "{game}".into(),
    ];
    let original_args = wii.launch_args.clone();
    update_settings(|s| {
        s.profiles.insert("gamecube".into(), gc);
        s.profiles.insert("wii".into(), wii);
        Ok(())
    })
    .unwrap();
    let result = emulator_import::import(
        profile_saves::begin().unwrap(),
        "gamecube",
        source.to_str().unwrap(),
    )
    .unwrap();
    let fresh = bootstrap_inventory(true).unwrap();
    println!(
        "RESULT source_exists={} destination_exists={} gamecube_ready={} wii_ready={} wii_path={}",
        source.exists(),
        Path::new(&result.path).exists(),
        fresh.ready_profiles["gamecube"],
        fresh.ready_profiles["wii"],
        fresh.profiles["wii"].emulator_path
    );
    assert!(
        fresh.ready_profiles["wii"],
        "moving a shared emulator broke its other saved console profile"
    );
    assert_eq!(fresh.profiles["wii"].launch_args, original_args);
    assert_eq!(fresh.profiles["wii"].emulator_path, result.path);
    assert!(result.profiles.contains_key("wii"));
}
#[test]
fn explicit_show_all_survives_automatic_discovery() {
    if std::env::var_os("MATRIX_EMULATOR_REGRESSION").is_none() {
        return;
    }
    create_layout().unwrap();
    let path = matrix_root()
        .unwrap()
        .join(spec("n64").unwrap().dir)
        .join("rmg.sh");
    fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let first = bootstrap_inventory(true).unwrap();
    assert!(first.needs_emulator_setup);
    assert!(first.profiles.contains_key("n64"));
    library_features::set_library_preference(
        "hideUnconfigured".into(),
        "".into(),
        serde_json::json!(false),
    )
    .unwrap();
    let second = bootstrap_inventory(true).unwrap();
    println!(
        "RESULT explicit_hide_false_became={:?}",
        second.library_preferences.hide_unconfigured
    );
    assert_eq!(
        second.library_preferences.hide_unconfigured,
        Some(false),
        "automatic discovery overwrote an explicitly saved visibility choice"
    );
}
#[test]
fn standalone_custom_executable_is_discovered() {
    if std::env::var_os("MATRIX_EMULATOR_REGRESSION").is_none() {
        return;
    }
    create_layout().unwrap();
    let path = matrix_root().unwrap().join("PS5/StandaloneCore");
    fs::copy("/bin/true", &path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(emulators::executable_file(&path));
    assert!(discovery::profile_for_file("ps5", &path).is_ok());
    let result = bootstrap_inventory(true).unwrap();
    println!(
        "RESULT executable_present={} ready_profile_present={}",
        path.exists(),
        result.profiles.contains_key("ps5")
    );
    assert!(
        result.profiles.contains_key("ps5"),
        "a valid standalone executable placed in the explicit console folder was silently missed"
    );
}
#[test]
fn committed_import_can_be_followed_by_refresh_failure() {
    if std::env::var_os("MATRIX_EMULATOR_REGRESSION").is_none() {
        return;
    }
    create_layout().unwrap();
    let source = home().unwrap().join("Dolphin.AppImage");
    image(&source);
    let unrelated = matrix_root().unwrap().join("PS2");
    fs::remove_dir(unrelated.join("Covers")).unwrap();
    fs::set_permissions(&unrelated, fs::Permissions::from_mode(0o500)).unwrap();
    let moved = emulator_import::import(
        profile_saves::begin().unwrap(),
        "gamecube",
        source.to_str().unwrap(),
    )
    .unwrap();
    let refresh = bootstrap().err();
    let saved = load_settings().unwrap().profiles["gamecube"]
        .emulator_path
        .clone();
    fs::set_permissions(&unrelated, fs::Permissions::from_mode(0o700)).unwrap();
    println!(
        "RESULT source_exists={} destination_exists={} profile_saved={} refresh_error={:?}",
        source.exists(),
        Path::new(&moved.path).exists(),
        saved == moved.path,
        refresh
    );
    assert!(!source.exists() && Path::new(&moved.path).exists() && refresh.is_some());
}

#[test]
fn isolated_emulator_regressions() {
    let cases = [
        "shared_emulator_move_retains_both_profiles",
        "explicit_show_all_survives_automatic_discovery",
        "standalone_custom_executable_is_discovered",
        "committed_import_can_be_followed_by_refresh_failure",
    ];
    for case in cases {
        let temp = tempfile::tempdir().unwrap();
        let name = format!("{}::{case}", module_path!().split_once("::").unwrap().1);
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env("MATRIX_EMULATOR_REGRESSION", "1")
            .env("HOME", temp.path())
            .env("PATH", temp.path().join("bin"))
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{case}: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
