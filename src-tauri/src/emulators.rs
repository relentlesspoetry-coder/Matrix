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
    let mut command = launch_command("flatpak");
    command.args(["list","--app","--columns=application,name,installation"]).env("LC_ALL","C");
    Ok(parse_flatpaks(&bounded_inventory(command, std::time::Duration::from_secs(10))?))
}

/// File-backed stdout cannot deadlock on an inherited pipe. Bound both the
/// command's lifetime and output; never terminate an emulator or unrelated app.
fn bounded_inventory(mut command: Command, timeout: std::time::Duration) -> Result<String, String> {
    use std::{io::Seek, os::unix::process::CommandExt, process::Stdio, time::Instant};
    const LIMIT: u64 = 1024 * 1024;
    let mut capture = tempfile::tempfile().map_err(|e|e.to_string())?;
    command.stdin(Stdio::null()).stderr(Stdio::null()).stdout(capture.try_clone().map_err(|e|e.to_string())?).process_group(0);
    let mut child = command.spawn().map_err(|_|"Flatpak was not found. Install your emulator through Software first, or choose an AppImage / executable.")?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e|e.to_string())? {
            if !status.success() { return Err("Could not read installed Flatpak apps. Check Flatpak, then try Reload apps.".into()); }
            break;
        }
        if started.elapsed() >= timeout || capture.metadata().map_err(|e|e.to_string())?.len() > LIMIT {
            // This unreaped child's private process group belongs to this one
            // inventory command; no process-name matching or game termination.
            unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL); }
            let _ = child.wait();
            return Err("Flatpak inventory timed out or exceeded its output limit. File-based emulators remain available; try Reload apps later.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    capture.rewind().map_err(|e|e.to_string())?;
    let mut bytes = Vec::new();
    capture.take(LIMIT + 1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.len() as u64 > LIMIT {return Err("Flatpak inventory exceeded its output limit.".into());}
    String::from_utf8(bytes).map_err(|_|"Flatpak returned invalid text.".into())
}

/// Recognize same-architecture standalone Linux executables without requiring a
/// brand name or suffix. Libraries and foreign-console ELF files are excluded.
pub(crate) fn native_binary(path: &Path) -> bool {
    use std::{io::{Seek, SeekFrom}, os::unix::fs::OpenOptionsExt};
    if !executable_file(path) { return false; }
    let Ok(mut file) = fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK|libc::O_NOCTTY).open(path) else {return false};
    if !file.metadata().is_ok_and(|m|m.is_file()) {return false;}
    let mut header=[0;64]; if file.read_exact(&mut header).is_err() {return false;}
    let machine = match std::env::consts::ARCH {"x86_64"=>62,"x86"=>3,"aarch64"=>183,"arm"=>40,"riscv64"=>243,_=>return false};
    let class = if cfg!(target_pointer_width="64") {2} else {1};
    if &header[..4]!=b"\x7fELF" || header[4]!=class || header[5]!=1 || header[6]!=1 || !matches!(header[7],0|3)
        || u16::from_le_bytes([header[18],header[19]])!=machine {return false;}
    match u16::from_le_bytes([header[16],header[17]]) {
        2=>true, // static/native ET_EXEC
        3=>{ // PIE executables need PT_INTERP; an arbitrary .so is not an emulator.
            let (offset,size,count)=if class==2 {(u64::from_le_bytes(header[32..40].try_into().unwrap()),u16::from_le_bytes([header[54],header[55]]),u16::from_le_bytes([header[56],header[57]]))}
                else {(u32::from_le_bytes(header[28..32].try_into().unwrap()) as u64,u16::from_le_bytes([header[42],header[43]]),u16::from_le_bytes([header[44],header[45]]))};
            if offset>4*1024*1024 || !(4..=256).contains(&size) || count>128 {return false;}
            for index in 0..count {let mut kind=[0;4];if file.seek(SeekFrom::Start(offset+index as u64*size as u64)).is_err() || file.read_exact(&mut kind).is_err(){return false;}if u32::from_le_bytes(kind)==3{return true;}}
            false
        },
        _=>false,
    }
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
/// Check the effective IDs that will execute this regular file. Any execute
/// bit is insufficient: permission-class selection, ACLs and mount options
/// can still deny access. Keep executable symlinks supported.
pub(crate) fn executable_file(path: &Path) -> bool {
    if !fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        return false;
    }
    #[cfg(unix)]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        CString::new(path.as_os_str().as_bytes()).is_ok_and(|path| unsafe {
            libc::faccessat(
                libc::AT_FDCWD,
                path.as_ptr(),
                libc::X_OK,
                libc::AT_EACCESS,
            ) == 0
        })
    }
    #[cfg(not(unix))]
    {
        true
    }
}
fn validate_file(path: &str, appimage: bool, allow_execute: bool) -> Result<String, String> {
    let path = Path::new(path)
        .canonicalize()
        .map_err(|error| file_path_error(path, error))?;
    let path_string = actionable_path(&path)?;
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
        if !executable_file(&path) {
            if appimage && allow_execute {
                let mut permissions = fs::metadata(&path)
                    .map_err(|e| e.to_string())?
                    .permissions();
                // Consent requests only the owner's execute bit, no extra
                // group/other permissions or mount-policy changes.
                if permissions.mode() & 0o100 == 0 {
                    permissions.set_mode(permissions.mode() | 0o100);
                    fs::set_permissions(&path,permissions).map_err(|_|"Cannot allow this AppImage to run. Choose a file you own, or change its permissions in Files.")?;
                }
            }
            // Other access restrictions can still prevent execution after the
            // mode adjustment. Reject a file still denied execute access.
            if !executable_file(&path) {
                let consent_hint = if appimage && !allow_execute {
                    " For an AppImage you own, enable ‘Allow this AppImage to run’."
                } else {
                    ""
                };
                return Err(format!("Matrix Play cannot execute the selected emulator file. Check its permissions, access to containing folders, and whether its drive permits execution.{consent_hint} Selected path: {}", path.display()));
            }
        }
    }
    Ok(path_string)
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
                    .map_err(|e| e.to_string())
                    .and_then(|p| actionable_path(&p))
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
    fn executable_validation_checks_effective_access_without_changing_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("emulator");
        let bytes = b"synthetic fixture, never executed";
        fs::write(&file, bytes).unwrap();
        for mode in [0o600, 0o601] {
            if mode == 0o601 && unsafe { libc::geteuid() } == 0 {
                continue;
            }
            fs::set_permissions(&file, fs::Permissions::from_mode(mode)).unwrap();
            assert!(!executable_file(&file));
            // Permission consent applies only to AppImages, not executables.
            for allow in [false, true] {
                let error = validate_file(file.to_str().unwrap(), false, allow).unwrap_err();
                assert!(error.contains("cannot execute"));
                assert!(error.contains(file.to_str().unwrap()));
                assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, mode);
            }
        }
        fs::set_permissions(&file, fs::Permissions::from_mode(0o700)).unwrap();
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(&file, &alias).unwrap();
        assert!(executable_file(&alias));
        assert_eq!(
            validate_file(alias.to_str().unwrap(), false, false).unwrap(),
            file.to_str().unwrap()
        );
        assert!(!executable_file(root.path()));
        assert_eq!(fs::read(&file).unwrap(), bytes);
        fs::remove_file(&file).unwrap();
        assert!(!executable_file(&alias));
    }

    #[test]
    fn appimage_consent_repairs_only_owner_execute_and_preserves_bytes() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("pcsx2.AppImage");
        let alias = root.path().join("pcsx2-alias.AppImage");
        let bytes = b"\x7fELF0000AI\x02synthetic image, never executed";
        fs::write(&image, bytes).unwrap();
        std::os::unix::fs::symlink(&image, &alias).unwrap();
        for mode in [0o600, 0o601] {
            if mode == 0o601 && unsafe { libc::geteuid() } == 0 {
                continue;
            }
            fs::set_permissions(&image, fs::Permissions::from_mode(mode)).unwrap();
            assert!(validate_file(alias.to_str().unwrap(), true, false).is_err());
            assert_eq!(fs::metadata(&image).unwrap().permissions().mode() & 0o777, mode);
            assert_eq!(
                validate_file(alias.to_str().unwrap(), true, true).unwrap(),
                image.to_str().unwrap()
            );
            assert_eq!(
                fs::metadata(&image).unwrap().permissions().mode() & 0o777,
                mode | 0o100
            );
            assert!(executable_file(&image));
            assert_eq!(fs::read(&image).unwrap(), bytes);
        }
        fs::write(&image, b"invalid AppImage header").unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(validate_file(alias.to_str().unwrap(), true, true).is_err());
        assert_eq!(fs::metadata(&image).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn selected_emulator_never_aliases_non_utf8_canonical_target() {
        use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join(std::ffi::OsString::from_vec(b"pcsx2-\xff.AppImage".to_vec()));
        let bytes = b"\x7fELF0000AI\x02synthetic image";
        fs::write(&target, bytes).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        let alias = temp.path().join("pcsx2-alias.AppImage");
        std::os::unix::fs::symlink(&target, &alias).unwrap();
        let lookalike = temp.path().join("pcsx2-\u{fffd}.AppImage");
        fs::write(&lookalike, bytes).unwrap();
        fs::set_permissions(&lookalike, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(validate_file(alias.to_str().unwrap(), true, true)
            .unwrap_err().contains("non-UTF-8"));
        assert_eq!(fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read(&target).unwrap(), bytes);
        assert_eq!(validate_file(lookalike.to_str().unwrap(), true, false).unwrap(),
            lookalike.to_str().unwrap());
    }

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

#[cfg(test)]
mod inventory_regressions {
    use super::*;
    #[test]
    fn flatpak_inventory_timeout_and_output_are_bounded() {
        let mut sleeping=Command::new("/bin/sh");sleeping.args(["-c","exec /bin/sleep 30"]);
        let start=std::time::Instant::now();
        assert!(bounded_inventory(sleeping,std::time::Duration::from_millis(100)).unwrap_err().contains("timed out"));
        assert!(start.elapsed()<std::time::Duration::from_secs(3));
        let mut excessive=Command::new("/usr/bin/head");excessive.args(["-c","1048577","/dev/zero"]);
        assert!(bounded_inventory(excessive,std::time::Duration::from_secs(3)).unwrap_err().contains("output limit"));
        let mut valid=Command::new("/bin/sh");valid.args(["-c","printf 'org.test.Emulator\\tTest emulator\\tuser\\n'"]);
        let text=bounded_inventory(valid,std::time::Duration::from_secs(3)).unwrap();assert_eq!(parse_flatpaks(&text).len(),1);
    }
    #[test]
    fn standalone_detection_rejects_data_libraries_and_foreign_binaries() {
        use std::os::unix::fs::PermissionsExt;
        let temp=tempfile::tempdir().unwrap();let path=temp.path().join("CustomCore");fs::copy("/bin/true",&path).unwrap();
        fs::set_permissions(&path,fs::Permissions::from_mode(0o700)).unwrap();assert!(native_binary(&path));
        let bytes=fs::read(&path).unwrap();let mut foreign=bytes.clone();foreign[18..20].copy_from_slice(&0xffffu16.to_le_bytes());fs::write(&path,foreign).unwrap();assert!(!native_binary(&path));
        fs::write(&path,b"not a Linux executable").unwrap();assert!(!native_binary(&path));
        fs::write(&path,bytes).unwrap();fs::set_permissions(&path,fs::Permissions::from_mode(0o600)).unwrap();assert!(!native_binary(&path));
    }
}
