//! Local discovery only. Never executes an AppImage or changes file permissions.
use super::*;
use emulators::{FlatpakApp, Launcher};

struct Engine {
    name: &'static str,
    flatpak: &'static str,
    prefixes: &'static [&'static str],
    consoles: &'static [&'static str],
}
const ENGINES: &[Engine] = &[
    Engine { name: "Lichii", flatpak: "", prefixes: &["lichii", "fritchii"], consoles: &["switch2"] },
    Engine { name: "Mednafen", flatpak: "", prefixes: &["mednafen"], consoles: &["nes", "snes", "genesis", "saturn"] },
    Engine { name: "xemu", flatpak: "app.xemu.xemu", prefixes: &["xemu"], consoles: &["xbox"] },

    Engine {
        name: "PCSX2",
        flatpak: "net.pcsx2.PCSX2",
        prefixes: &["pcsx2"],
        consoles: &["ps2"],
    },
    Engine {
        name: "DuckStation",
        flatpak: "org.duckstation.DuckStation",
        prefixes: &["duckstation"],
        consoles: &["ps1"],
    },
    Engine {
        name: "RPCS3",
        flatpak: "net.rpcs3.RPCS3",
        prefixes: &["rpcs3"],
        consoles: &["ps3"],
    },
    Engine {
        name: "PPSSPP",
        flatpak: "org.ppsspp.PPSSPP",
        prefixes: &["ppsspp", "ppssppsdl"],
        consoles: &["psp"],
    },
    Engine {
        name: "RMG",
        flatpak: "com.github.Rosalie241.RMG",
        prefixes: &["rmg"],
        consoles: &["n64"],
    },
    Engine {
        name: "Dolphin",
        flatpak: "org.DolphinEmu.dolphin-emu",
        prefixes: &["dolphin-emu", "dolphin-emulator"],
        consoles: &["gamecube", "wii"],
    },
    Engine {
        name: "Flycast",
        flatpak: "org.flycast.Flycast",
        prefixes: &["flycast"],
        consoles: &["dreamcast"],
    },
    Engine {
        name: "Cemu",
        flatpak: "info.cemu.Cemu",
        prefixes: &["cemu"],
        consoles: &["wiiu"],
    },
    Engine {
        name: "SameBoy",
        flatpak: "io.github.sameboy.SameBoy",
        prefixes: &["sameboy"],
        consoles: &["gb"],
    },
    Engine {
        name: "mGBA",
        flatpak: "io.mgba.mGBA",
        prefixes: &["mgba"],
        consoles: &["gb", "gba"],
    },
    Engine {
        name: "melonDS",
        flatpak: "net.kuribo64.melonDS",
        prefixes: &["melonds"],
        consoles: &["nds"],
    },
    Engine {
        name: "Azahar",
        flatpak: "org.azahar_emu.Azahar",
        prefixes: &["azahar"],
        consoles: &["3ds"],
    },
    Engine {
        name: "Eden",
        flatpak: "dev.eden_emu.eden",
        prefixes: &["eden"],
        consoles: &["switch"],
    },
    Engine {
        name: "Ryujinx",
        flatpak: "org.ryujinx.Ryujinx",
        prefixes: &["ryujinx"],
        consoles: &["switch"],
    },
    Engine {
        name: "shadPS4 QtLauncher",
        flatpak: "net.shadps4.shadPS4",
        prefixes: &["shadps4qtlauncher", "shadps4-qtlauncher"],
        consoles: &["ps4"],
    },
    Engine {
        name: "shadPS4",
        flatpak: "",
        prefixes: &["shadps4"],
        consoles: &["ps4"],
    },
    Engine {
        name: "Vita3K",
        flatpak: "",
        prefixes: &["vita3k"],
        consoles: &["vita"],
    },
];
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Candidate {
    pub id: String,
    pub console_ids: Vec<String>,
    pub profile: EmulatorProfile,
    pub source: String,
    pub ready: bool,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
    pub candidates: Vec<Candidate>,
    pub linked: Vec<String>,
    pub ambiguous: Vec<String>,
    pub notices: Vec<String>,
    // None means listing failed or no complete inventory was supplied. Absence
    // from the bounded candidate scan must never imply an uninstall.
    #[serde(skip)]
    pub(crate) flatpak_inventory: Option<Vec<FlatpakApp>>,
}
fn engine_for_name(name: &str) -> Option<&'static Engine> {
    let lower = name.to_ascii_lowercase();
    ENGINES.iter().find(|engine| {
        engine.prefixes.iter().any(|prefix| {
            lower
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['-', '_', '.', ' ']))
        })
    })
}
pub(crate) fn supports_filename(name: &str, id: &str) -> bool {
    engine_for_name(name).is_some_and(|e|e.consoles.contains(&id))
}
pub(crate) fn identity(profile: &EmulatorProfile, console: &str) -> Option<&'static str> {
    let engine = if profile.launcher == Launcher::Flatpak {
        ENGINES
            .iter()
            .find(|e| !e.flatpak.is_empty() && e.flatpak == profile.flatpak_id)
    } else {
        Path::new(&profile.emulator_path)
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(engine_for_name)
    }?;
    engine.consoles.contains(&console).then_some(engine.name)
}
fn args(engine: &Engine, id: &str) -> Vec<String> {
    if engine.flatpak == "net.pcsx2.PCSX2" {
        vec!["--".into(), "{game}".into()]
    } else {
        default_args(id)
    }
}
pub(crate) fn profile_for_file(id: &str, path: &Path) -> Result<EmulatorProfile, String> {
    spec(id)?;
    let name = path.file_name().and_then(|n| n.to_str()).ok_or("Invalid emulator filename")?;
    let engine = engine_for_name(name);
    if engine.is_some_and(|e| !e.consoles.contains(&id)) {
        return Err("This emulator belongs to a different console. Choose its supported console.".into());
    }
    Ok(EmulatorProfile {
        emulator_name: engine.map(|e| e.name).unwrap_or(name).into(),
        emulator_path: actionable_path(path)?,
        launcher: if appimage(path) { Launcher::Appimage } else { Launcher::Executable },
        launch_args: engine.map(|e| args(e, id)).unwrap_or_else(|| default_args(id)),
        ..Default::default()
    })
}

/// Frequent polling inspects only the managed console folders, not Downloads.
pub(crate) fn discover_managed() -> Result<Report, String> {
    let mut report = Report::default();
    report.flatpak_inventory = emulators::list_flatpaks().ok();
    let mut map = BTreeMap::new();
    for s in SPECS {
        // Read-only visibility for downloaded AppImages without execute permission.
        for entry in fs::read_dir(matrix_root()?.join(s.dir)).into_iter().flatten().flatten().take(100) {
            if entry.path().is_file() { add_image(&mut map, &entry.path()); }
        }
        for path in detect_emulators(s) {
            add_image(&mut map, &path);
            add_native(&mut map, s, path);
        }
    }
    report.candidates = map.into_values().collect();
    Ok(report)
}
fn appimage(path: &Path) -> bool {
    use std::io::Read;
    let mut bytes = [0; 11];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_ok()
        && bytes.starts_with(b"\x7fELF")
        && (&bytes[8..11] == b"AI\x01" || &bytes[8..11] == b"AI\x02")
}
fn insert(map: &mut BTreeMap<String, Candidate>, candidate: Candidate) {
    if let Some(existing) = map.get_mut(&candidate.id) {
        for id in candidate.console_ids {
            if !existing.console_ids.contains(&id) {
                existing.console_ids.push(id);
            }
        }
    } else {
        map.insert(candidate.id.clone(), candidate);
    }
}
fn add_image(map: &mut BTreeMap<String, Candidate>, path: &Path) {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("appimage"))
    {
        return;
    }
    let Some(engine) = path
        .file_stem()
        .and_then(|n| n.to_str())
        .and_then(engine_for_name)
    else {
        return;
    };
    if !appimage(path) {
        return;
    }
    let Ok(path) = path.canonicalize() else {
        return;
    };
    let Ok(path_string) = actionable_path(&path) else {
        return;
    };
    let profile = EmulatorProfile {
        emulator_name: engine.name.into(),
        emulator_path: path_string,
        launcher: Launcher::Appimage,
        launch_args: args(engine, engine.consoles[0]),
        auto_detected: true,
        ..Default::default()
    };
    insert(
        map,
        Candidate {
            id: format!("file|{}", path.display()),
            console_ids: engine.consoles.iter().map(|s| s.to_string()).collect(),
            profile,
            source: "AppImage name and header".into(),
            ready: emulators::executable_file(&path),
        },
    );
}
fn add_flatpaks(map: &mut BTreeMap<String, Candidate>, apps: &[FlatpakApp]) {
    for app in apps {
        let Some(engine) = ENGINES.iter().find(|e| e.flatpak == app.app_id) else {
            continue;
        };
        let profile = EmulatorProfile {
            emulator_name: engine.name.into(),
            launcher: Launcher::Flatpak,
            flatpak_id: app.app_id.clone(),
            flatpak_installation: app.installation.clone(),
            launch_args: args(engine, engine.consoles[0]),
            auto_detected: true,
            ..Default::default()
        };
        insert(
            map,
            Candidate {
                id: format!("flatpak|{}|{}", app.app_id, app.installation),
                console_ids: engine.consoles.iter().map(|s| s.to_string()).collect(),
                profile,
                source: format!("Flatpak ({})", app.installation),
                ready: true,
            },
        );
    }
}
/// Parse desktop Exec tokens as data; never expand variables, substitute or run them.
fn desktop_paths(text: &str) -> Vec<PathBuf> {
    let mut inside = false;
    let mut result = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[Desktop Entry]";
            continue;
        }
        if !inside {
            continue;
        }
        let Some(exec) = line.strip_prefix("Exec=") else {
            continue;
        };
        let mut token = String::new();
        let mut quoted = false;
        let mut escape = false;
        for c in exec.chars().chain(std::iter::once(' ')) {
            if escape {
                token.push(c);
                escape = false;
                continue;
            }
            if c == '\\' {
                escape = true;
                continue;
            }
            if c == '"' {
                quoted = !quoted;
                continue;
            }
            if c.is_whitespace() && !quoted {
                let path = PathBuf::from(&token);
                if path.is_absolute()
                    && path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("appimage"))
                {
                    result.push(path);
                }
                token.clear();
            } else {
                token.push(c);
            }
        }
    }
    result
}
fn confirmed_unavailable(profile: &EmulatorProfile, report: &Report) -> bool {
    if profile.launcher == Launcher::Flatpak {
        return report.flatpak_inventory.as_ref().is_some_and(|apps| {
            !apps.iter().any(|app| {
                app.app_id == profile.flatpak_id
                    && app.installation == profile.flatpak_installation
            })
        });
    }
    // A scan limit, inaccessible directory, or transient I/O error is not
    // evidence that the selected installation has been removed.
    // Unlike candidate readiness, an effective-access failure alone is not
    // proof of removal: preserve the saved choice on uncertain access errors.
    match fs::metadata(&profile.emulator_path) {
        Ok(metadata) => {
            use std::os::unix::fs::PermissionsExt;
            !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0
        }
        Err(error) => matches!(error.kind(), std::io::ErrorKind::NotFound),
    }
}

pub(crate) fn auto_link(settings: &mut Settings, report: &mut Report) {
    for spec in SPECS {
        // Successor consoles only receive candidates whose engine explicitly
        // supports them, or files deliberately placed in that console folder.
        // Manual choices win. Replace a missing auto-link only when a single
        // ready replacement exists; never change a working selected install.
        if let Some(profile) = settings.profiles.get(spec.id) {
            let identity = if profile.launcher == Launcher::Flatpak {
                format!(
                    "flatpak|{}|{}",
                    profile.flatpak_id, profile.flatpak_installation
                )
            } else {
                format!("file|{}", profile.emulator_path)
            };
            if !profile.auto_detected
                || report
                    .candidates
                    .iter()
                    .any(|c| c.ready && c.id == identity)
                || !confirmed_unavailable(profile, report)
            {
                continue;
            }
        }
        let choices: Vec<_> = report
            .candidates
            .iter()
            .filter(|c| c.ready && c.console_ids.iter().any(|id| id == spec.id)
                && (!custom_emulator_only(spec.id)
                    || identity(&c.profile, spec.id) == Some("Lichii")
                    || matrix_root().is_ok_and(|root| Path::new(&c.profile.emulator_path).starts_with(root.join(spec.dir)))))
            .collect();
        if choices.len() == 1 {
            let mut profile = choices[0].profile.clone();
            let engine = if profile.launcher == Launcher::Flatpak {
                ENGINES.iter().find(|e| e.flatpak == profile.flatpak_id)
            } else {
                Path::new(&profile.emulator_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(engine_for_name)
            };
            if let Some(engine) = engine {
                profile.launch_args = args(engine, spec.id);
            }
            settings.profiles.insert(spec.id.into(), profile);
            report.linked.push(spec.id.into());
        } else if choices.len() > 1 {
            report.ambiguous.push(spec.id.into());
        }
    }
}
fn add_native(map: &mut BTreeMap<String, Candidate>, spec: &ConsoleSpec, path: PathBuf) {
    let known = path
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(engine_for_name);
    if known.is_some_and(|e| !e.consoles.contains(&spec.id)) {
        return;
    }
    let image_ext = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("appimage"));
    if image_ext && !appimage(&path) {
        return;
    }
    let path = path.canonicalize().unwrap_or(path);
    let Ok(path_string) = actionable_path(&path) else {
        return;
    };
    insert(
        map,
        Candidate {
            id: format!("file|{}", path.display()),
            console_ids: vec![spec.id.into()],
            profile: EmulatorProfile {
                emulator_name: known
                    .map(|e| e.name)
                    .unwrap_or(guessed_emulator(spec.id))
                    .into(),
                emulator_path: path_string,
                launcher: if appimage(&path) {
                    Launcher::Appimage
                } else {
                    Launcher::Executable
                },
                launch_args: known
                    .map(|e| args(e, spec.id))
                    .unwrap_or_else(|| default_args(spec.id)),
                auto_detected: true,
                ..Default::default()
            },
            source: "Installed executable or explicit console folder".into(),
            ready: emulators::executable_file(&path),
        },
    );
}
pub(crate) fn discover() -> Result<Report, String> {
    let mut report = Report::default();
    let mut map = BTreeMap::new();
    match emulators::list_flatpaks() {
        Ok(apps) => {
            add_flatpaks(&mut map, &apps);
            report.flatpak_inventory = Some(apps);
        }
        Err(_) => report.notices.push(
            "Flatpak discovery unavailable; AppImage and executable detection still work.".into(),
        ),
    }
    let home = home()?;
    let roots = [
        home.join("Applications"),
        home.join("AppImages"),
        home.join("Downloads"),
        home.join(".local/bin"),
        home.join(".local/share/appimages"),
    ];
    let mut count = 0;
    for root in roots {
        for entry in WalkDir::new(root)
            .follow_links(false)
            .max_depth(3)
            .into_iter()
            .filter_map(Result::ok)
            .take(2000)
        {
            count += 1;
            if count > 6000 {
                break;
            }
            if entry.path().is_file() {
                add_image(&mut map, entry.path());
            }
        }
        if count > 6000 {
            report
                .notices
                .push("AppImage scan limit reached. Browse manually for other locations.".into());
            break;
        }
    }
    let desktop = home.join(".local/share/applications");
    for entry in fs::read_dir(desktop)
        .into_iter()
        .flatten()
        .flatten()
        .take(500)
    {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "desktop") {
            if let Some(text) = bounded_io::read_text(&path, 65535) {
                for path in desktop_paths(&text) {
                    add_image(&mut map, &path);
                }
            }
        }
    }
    for spec in SPECS {
        let dir = matrix_root()?.join(spec.dir);
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten().take(100) {
            if entry.path().is_file() {
                add_image(&mut map, &entry.path());
            }
        }
        for path in detect_emulators(spec)
            .into_iter()
            .chain(detect_installed_emulators(spec.id))
        {
            add_native(&mut map, spec, path);
        }
    }
    report.candidates = map.into_values().collect();
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn executable_fixture(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[test]
    fn inaccessible_execute_class_never_auto_links_or_creates_false_ambiguity() {
        use std::os::unix::fs::PermissionsExt;
        // Root can execute a file with any execute bit. Nonroot CI exercises
        // permission-class selection without ever running these fixture files.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let restricted = root.path().join("pcsx2-restricted.AppImage");
        let ready = root.path().join("pcsx2-ready.AppImage");
        let bytes = b"\x7fELF0000AI\x02synthetic image, never executed";
        fs::write(&restricted, bytes).unwrap();
        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o601)).unwrap();
        let mut map = BTreeMap::new();
        add_image(&mut map, &restricted);
        assert_eq!(map.len(), 1);
        let selected = map.values().next().unwrap().profile.clone();
        assert!(!map.values().next().unwrap().ready);
        let mut report = Report {
            candidates: map.values().cloned().collect(),
            ..Default::default()
        };
        let mut settings = Settings::default();
        auto_link(&mut settings, &mut report);
        assert!(settings.profiles.is_empty());
        assert!(report.linked.is_empty());
        assert!(report.ambiguous.is_empty());

        fs::write(&ready, bytes).unwrap();
        fs::set_permissions(&ready, fs::Permissions::from_mode(0o700)).unwrap();
        add_image(&mut map, &ready);
        report.candidates = map.into_values().collect();
        auto_link(&mut settings, &mut report);
        assert_eq!(settings.profiles["ps2"].emulator_path, ready.to_str().unwrap());
        assert_eq!(report.linked, ["ps2"]);
        assert!(report.ambiguous.is_empty());

        // Failed access is not evidence that a previously selected install
        // disappeared, so preserve it rather than switching silently.
        settings.profiles.insert("ps2".into(), selected);
        report.linked.clear();
        auto_link(&mut settings, &mut report);
        assert_eq!(
            settings.profiles["ps2"].emulator_path,
            restricted.to_str().unwrap()
        );
        assert!(report.linked.is_empty());
        assert_eq!(
            fs::metadata(&restricted).unwrap().permissions().mode() & 0o777,
            0o601
        );
        assert_eq!(fs::read(&restricted).unwrap(), bytes);
    }

    #[test]
    fn native_and_console_folder_discovery_skip_inaccessible_executables() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let restricted = root.path().join("restricted");
        let ready = root.path().join("ready");
        fs::create_dir(&restricted).unwrap();
        fs::create_dir(&ready).unwrap();
        let bad = restricted.join("pcsx2");
        let good = ready.join("pcsx2");
        executable_fixture(&bad);
        executable_fixture(&good);
        for mode in [0o600, 0o601] {
            if mode == 0o601 && unsafe { libc::geteuid() } == 0 {
                continue;
            }
            fs::set_permissions(&bad, fs::Permissions::from_mode(mode)).unwrap();
            assert!(detect_emulators_in(spec("ps2").unwrap(), &restricted).is_empty());
            assert_eq!(
                detect_installed_emulators_in("ps2", &[restricted.clone(), ready.clone()]),
                [good.clone()]
            );
            let mut map = BTreeMap::new();
            add_native(&mut map, spec("ps2").unwrap(), bad.clone());
            assert!(!map.values().next().unwrap().ready);
        }
    }

    #[test]
    fn discovery_skips_non_utf8_targets_but_keeps_literal_replacement_names() {
        use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let mut map = BTreeMap::new();
        for byte in [0xff, 0xfe] {
            let parent = temp.path().join(std::ffi::OsString::from_vec(vec![byte]));
            fs::create_dir(&parent).unwrap();
            let target = parent.join("pcsx2.AppImage");
            fs::write(&target, b"\x7fELF0000AI\x02synthetic image").unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
            let alias = temp.path().join(format!("pcsx2-{byte}.AppImage"));
            std::os::unix::fs::symlink(&target, &alias).unwrap();
            add_image(&mut map, &alias);
            add_native(&mut map, spec("ps2").unwrap(), alias);
            assert!(map.is_empty());
        }
        let valid = temp.path().join("pcsx2-\u{fffd}.AppImage");
        fs::write(&valid, b"\x7fELF0000AI\x02synthetic image").unwrap();
        fs::set_permissions(&valid, fs::Permissions::from_mode(0o700)).unwrap();
        add_image(&mut map, &valid);
        add_native(&mut map, spec("ps2").unwrap(), valid.clone());
        assert_eq!(map.len(), 1);
        assert_eq!(map.values().next().unwrap().profile.emulator_path, valid.to_str().unwrap());
    }

    #[test]
    fn incomplete_scan_preserves_existing_executable_and_uncertain_paths() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("pcsx2-selected");
        let replacement = root.path().join("pcsx2-replacement");
        executable_fixture(&selected);
        executable_fixture(&replacement);
        let mut candidates = BTreeMap::new();
        add_native(&mut candidates, spec("ps2").unwrap(), replacement.clone());
        let mut report = Report {
            candidates: candidates.into_values().collect(),
            ..Default::default()
        };
        let mut settings = Settings::default();
        settings.profiles.insert("ps2".into(), EmulatorProfile {
            emulator_path: selected.to_string_lossy().into_owned(),
            auto_detected: true,
            ..Default::default()
        });
        auto_link(&mut settings, &mut report);
        assert_eq!(settings.profiles["ps2"].emulator_path, selected.to_string_lossy());
        assert!(report.linked.is_empty());

        // An I/O error (here, a symlink loop) is uncertainty, not removal.
        fs::remove_file(&selected).unwrap();
        std::os::unix::fs::symlink(&selected, &selected).unwrap();
        auto_link(&mut settings, &mut report);
        assert_eq!(settings.profiles["ps2"].emulator_path, selected.to_string_lossy());
        assert!(report.linked.is_empty());

        // A confirmed missing target can still recover automatically.
        fs::remove_file(&selected).unwrap();
        auto_link(&mut settings, &mut report);
        assert_eq!(settings.profiles["ps2"].emulator_path, replacement.to_string_lossy());
        assert_eq!(report.linked, ["ps2"]);
    }

    #[test]
    fn flatpak_replacement_requires_successful_inventory_confirming_removal() {
        let selected = FlatpakApp {
            app_id: "net.pcsx2.PCSX2".into(),
            name: "PCSX2".into(),
            installation: "user".into(),
        };
        let replacement = FlatpakApp {
            installation: "system".into(),
            ..selected.clone()
        };
        let mut map = BTreeMap::new();
        add_flatpaks(&mut map, &[replacement.clone()]);
        let mut report = Report {
            candidates: map.into_values().collect(),
            ..Default::default()
        };
        let mut settings = Settings::default();
        settings.profiles.insert("ps2".into(), EmulatorProfile {
            launcher: Launcher::Flatpak,
            flatpak_id: selected.app_id.clone(),
            flatpak_installation: selected.installation.clone(),
            auto_detected: true,
            ..Default::default()
        });
        // Failed listing, then a successful inventory that includes the selection.
        for inventory in [None, Some(vec![selected, replacement.clone()])] {
            report.flatpak_inventory = inventory;
            auto_link(&mut settings, &mut report);
            assert_eq!(settings.profiles["ps2"].flatpak_installation, "user");
            assert!(report.linked.is_empty());
        }
        report.flatpak_inventory = Some(vec![replacement]);
        auto_link(&mut settings, &mut report);
        assert_eq!(settings.profiles["ps2"].flatpak_installation, "system");
        assert_eq!(report.linked, ["ps2"]);
        assert!(serde_json::to_value(&report).unwrap().get("flatpakInventory").is_none());
    }

    #[test]
    fn experimental_custom_profiles_are_never_automatically_linked_or_replaced() {
        let mut report = Report::default();
        for id in ["ps5", "switch2"] {
            report.candidates.push(Candidate {
                id: format!("fixture|{id}"),
                console_ids: vec![id.into()],
                profile: EmulatorProfile {
                    emulator_name: "Fixture candidate".into(),
                    auto_detected: true,
                    ..Default::default()
                },
                source: "Synthetic candidate".into(),
                ready: true,
            });
        }
        let mut settings = Settings::default();
        auto_link(&mut settings, &mut report);
        assert!(settings.profiles.is_empty());
        settings.profiles.insert(
            "ps5".into(),
            EmulatorProfile {
                emulator_name: "My selected emulator".into(),
                emulator_path: "/apps/custom".into(),
                ..Default::default()
            },
        );
        auto_link(&mut settings, &mut report);
        assert_eq!(settings.profiles["ps5"].emulator_path, "/apps/custom");
        assert!(report.linked.is_empty());
    }

    #[test]
    fn moved_shadps4_appimage_is_rediscovered_without_changing_its_bytes_or_profile() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let downloads = root
            .path()
            .join("Downloads/shadPS4QtLauncher-linux-qt-build");
        let installed = root.path().join("Games/Matrix/PS4");
        fs::create_dir_all(&downloads).unwrap();
        fs::create_dir_all(&installed).unwrap();
        let old = downloads.join("shadPS4QtLauncher-qt.AppImage");
        let new = installed.join("shadPS4QtLauncher-qt.AppImage");
        let bytes = b"\x7fELF0000AI\x02synthetic, never executed";
        fs::write(&old, bytes).unwrap();
        fs::set_permissions(&old, fs::Permissions::from_mode(0o744)).unwrap();
        let mut before = BTreeMap::new();
        add_image(&mut before, &old);
        let profile = before.values().next().unwrap().profile.clone();
        assert_eq!(identity(&profile, "ps4"), Some("shadPS4 QtLauncher"));
        fs::rename(&old, &new).unwrap();
        let error = match emulators::prepare(profile.clone(), false) {
            Err(error) => error,
            Ok(_) => panic!("A moved emulator must not save its stale path"),
        };
        assert!(error.contains("no longer available"));
        assert!(error.contains(old.to_str().unwrap()));
        assert!(error.contains("Refresh detected"));
        let mut after = BTreeMap::new();
        add_image(&mut after, &old);
        add_image(&mut after, &new);
        assert_eq!(after.len(), 1);
        let selected = after.values().next().unwrap();
        assert!(selected.ready);
        let prepared = emulators::prepare(selected.profile.clone(), false).unwrap();
        assert_eq!(prepared.emulator_path, new.to_str().unwrap());
        assert_eq!(profile.emulator_path, old.to_str().unwrap());
        assert_eq!(fs::read(&new).unwrap(), bytes);
        assert_eq!(
            fs::metadata(&new).unwrap().permissions().mode() & 0o777,
            0o744
        );
    }

    #[test]
    fn native_discovery_lists_all_choices_and_does_not_auto_choose() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        executable_fixture(&first.join("eden"));
        executable_fixture(&second.join("ryujinx"));
        fs::write(first.join("ryujinx"), b"not executable").unwrap();
        let paths = detect_installed_emulators_in("switch", &[first.clone(), second, first]);
        assert_eq!(paths.len(), 2);
        let mut map = BTreeMap::new();
        for path in paths {
            add_native(&mut map, spec("switch").unwrap(), path);
        }
        let mut report = Report {
            candidates: map.into_values().collect(),
            ..Default::default()
        };
        let mut settings = Settings::default();
        auto_link(&mut settings, &mut report);
        assert_eq!(report.candidates.len(), 2);
        assert!(report.ambiguous.iter().any(|id| id == "switch"));
        assert!(!settings.profiles.contains_key("switch"));
    }
    #[test]
    fn native_discovery_deduplicates_symlinks_and_includes_mgba_for_gb() {
        let root = tempfile::tempdir().unwrap();
        executable_fixture(&root.path().join("mgba-qt"));
        std::os::unix::fs::symlink(root.path().join("mgba-qt"), root.path().join("mgba")).unwrap();
        let paths = detect_installed_emulators_in("gb", &[root.path().to_path_buf()]);
        assert_eq!(paths, [root.path().join("mgba-qt")]);
        let mut map = BTreeMap::new();
        add_native(&mut map, spec("gb").unwrap(), paths[0].clone());
        assert_eq!(map.len(), 1);
        assert_eq!(map.values().next().unwrap().profile.emulator_name, "mGBA");
    }
    #[test]
    fn console_directory_lists_multiple_executables_instead_of_hiding_them() {
        let root = tempfile::tempdir().unwrap();
        executable_fixture(&root.path().join("first.sh"));
        executable_fixture(&root.path().join("second.sh"));
        fs::write(root.path().join("third.sh"), b"not executable").unwrap();
        let paths = detect_emulators_in(spec("ps2").unwrap(), root.path());
        assert_eq!(
            paths,
            [root.path().join("first.sh"), root.path().join("second.sh")]
        );
        let mut map = BTreeMap::new();
        for path in paths {
            add_native(&mut map, spec("ps2").unwrap(), path);
        }
        assert_eq!(map.len(), 2);
    }
    #[test]
    fn discovery_merge_preserves_saves_made_during_the_scan() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("frontend.json");
        let mut map = BTreeMap::new();
        add_flatpaks(
            &mut map,
            &[FlatpakApp {
                app_id: "net.pcsx2.PCSX2".into(),
                name: "PCSX2".into(),
                installation: "system".into(),
            }],
        );
        let mut report = Report {
            candidates: map.into_values().collect(),
            ..Default::default()
        };
        // A different window saves while the candidate scan is in flight.
        settings::update(&path, |current| {
            current.profiles.insert(
                "ps2".into(),
                EmulatorProfile {
                    emulator_name: "Manual selection".into(),
                    auto_detected: false,
                    ..Default::default()
                },
            );
            current
                .favorites
                .insert("ps2".into(), vec!["/games/keep.iso".into()]);
            current.artwork.enabled = false;
            current.legacy_fields.insert("windows".into(),serde_json::json!([{"uid":"keep","title":"Keep this game","target":"/games/keep.desktop","runner":"desktop"}]));
            Ok(())
        })
        .unwrap();
        settings::update(&path, |current| {
            auto_link(current, &mut report);
            Ok(())
        })
        .unwrap();
        let saved = settings::load(&path).unwrap();
        assert_eq!(saved.profiles["ps2"].emulator_name, "Manual selection");
        assert_eq!(saved.favorites["ps2"], ["/games/keep.iso"]);
        assert!(!saved.artwork.enabled);
        assert_eq!(saved.legacy_fields["windows"][0]["uid"], "keep");
        assert!(report.linked.is_empty());
    }

    #[test]
    fn exact_ids_and_shared_console_capabilities() {
        let apps = vec![
            FlatpakApp {
                app_id: "net.pcsx2.PCSX2".into(),
                name: "Renamed app".into(),
                installation: "system".into(),
            },
            FlatpakApp {
                app_id: "org.DolphinEmu.dolphin-emu".into(),
                name: "Dolphin".into(),
                installation: "user".into(),
            },
            FlatpakApp {
                app_id: "org.example.Unknown".into(),
                name: "PCSX2".into(),
                installation: "user".into(),
            },
        ];
        let mut map = BTreeMap::new();
        add_flatpaks(&mut map, &apps);
        assert_eq!(map.len(), 2);
        let mut report = Report {
            candidates: map.into_values().collect(),
            ..Default::default()
        };
        let mut settings = Settings::default();
        auto_link(&mut settings, &mut report);
        assert!(settings.profiles.contains_key("ps2"));
        assert!(settings.profiles.contains_key("gamecube"));
        assert!(settings.profiles.contains_key("wii"));
        assert!(!settings.profiles.contains_key("ps1"));
        settings.profiles.get_mut("ps2").unwrap().emulator_name = "Keep manual choice".into();
        auto_link(&mut settings, &mut report);
        assert_eq!(settings.profiles["ps2"].emulator_name, "Keep manual choice");
    }
    #[test]
    fn appimage_matching_is_bounded_validated_and_deduplicated() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("pcsx2-v2.8.2-linux.AppImage");
        fs::write(&image, b"\x7fELF0000AI\x02fixture").unwrap();
        let mut map = BTreeMap::new();
        add_image(&mut map, &image);
        assert_eq!(map.len(), 1);
        assert!(!map.values().next().unwrap().ready);
        fs::set_permissions(&image, fs::Permissions::from_mode(0o700)).unwrap();
        map.clear();
        add_image(&mut map, &image);
        add_image(&mut map, &image);
        assert_eq!(map.len(), 1);
        assert!(map.values().next().unwrap().ready);
        assert!(engine_for_name("notpcsx2.AppImage").is_none());
        assert!(engine_for_name("dolphin").is_none());
        assert!(engine_for_name("pcsx2unrelated").is_none());
        fs::write(&image, b"not an AppImage").unwrap();
        map.clear();
        add_image(&mut map, &image);
        assert!(map.is_empty());
        assert_eq!(desktop_paths("[Desktop Entry]\nExec=\"/custom path/PCSX2.AppImage\" %f\n[Other]\nExec=/bad.AppImage"),vec![PathBuf::from("/custom path/PCSX2.AppImage")]);
    }
    #[test]
    fn folder_label_does_not_imply_engine_specific_arguments() {
        let candidate = Candidate {
            id: "file|/tmp/TestEmulator.AppImage".into(),
            console_ids: vec!["ps2".into()],
            profile: EmulatorProfile {
                emulator_name: "PCSX2".into(),
                emulator_path: "/tmp/TestEmulator.AppImage".into(),
                launch_args: vec!["{game}".into()],
                auto_detected: true,
                ..Default::default()
            },
            source: "Explicit folder".into(),
            ready: true,
        };
        let mut report = Report {
            candidates: vec![candidate],
            ..Default::default()
        };
        let mut settings = Settings::default();
        auto_link(&mut settings, &mut report);
        assert_eq!(settings.profiles["ps2"].launch_args, vec!["{game}"]);
    }
    #[test]
    fn multiple_installations_need_a_choice() {
        let apps = ["user", "system"].map(|s| FlatpakApp {
            app_id: "net.pcsx2.PCSX2".into(),
            name: "PCSX2".into(),
            installation: s.into(),
        });
        let mut map = BTreeMap::new();
        add_flatpaks(&mut map, &apps);
        let mut report = Report {
            candidates: map.into_values().collect(),
            flatpak_inventory: Some(apps.to_vec()),
            ..Default::default()
        };
        let mut settings = Settings::default();
        auto_link(&mut settings, &mut report);
        assert!(settings.profiles.is_empty());
        assert_eq!(report.ambiguous, vec!["ps2"]);
        let mut missing = report.candidates[0].profile.clone();
        missing.flatpak_installation = "removed".into();
        missing.auto_detected = true;
        settings.profiles.insert("ps2".into(), missing);
        report
            .candidates
            .retain(|c| c.profile.flatpak_installation == "user");
        report.ambiguous.clear();
        auto_link(&mut settings, &mut report);
        assert_eq!(settings.profiles["ps2"].flatpak_installation, "user");
        let manual = settings.profiles.get_mut("ps2").unwrap();
        manual.auto_detected = false;
        manual.flatpak_installation = "keep-custom-choice".into();
        auto_link(&mut settings, &mut report);
        assert_eq!(
            settings.profiles["ps2"].flatpak_installation,
            "keep-custom-choice"
        );
    }
}
