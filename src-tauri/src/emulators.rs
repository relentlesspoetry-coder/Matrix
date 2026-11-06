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
    app_id: String,
    name: String,
    installation: String,
}
fn parse_flatpaks(text: &str) -> Vec<FlatpakApp> {
    let mut apps = BTreeMap::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 {
            continue;
        }
        let (id, name, installation) = (fields[0].trim(), fields[1].trim(), fields[2].trim());
        if id.split('.').count() < 3
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
#[cfg_attr(feature = "desktop", tauri::command)]
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
fn validate_file(path: &str, appimage: bool, allow_execute: bool) -> Result<String, String> {
    let path = Path::new(path)
        .canonicalize()
        .map_err(|_| "Choose an emulator file that exists on this computer.")?;
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
pub(crate) fn command(profile: &EmulatorProfile, id: &str) -> Result<Command, String> {
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
            let folder = games_dir(id)?.canonicalize().map_err(|e| e.to_string())?;
            c.arg(format!("--filesystem={}:ro", folder.display()))
                .arg(&profile.flatpak_id);
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
    fn legacy_profile_stays_executable() {
        let profile:EmulatorProfile=serde_json::from_str(r#"{"emulatorName":"Old emulator","emulatorPath":"/usr/bin/emu","launchArgs":["{game}"]}"#).unwrap();
        assert!(profile.launcher == Launcher::Executable);
    }
}
