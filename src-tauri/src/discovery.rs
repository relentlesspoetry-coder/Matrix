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
        name: "Ryujinx",
        flatpak: "org.ryujinx.Ryujinx",
        prefixes: &["ryujinx"],
        consoles: &["switch"],
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
fn args(engine: &Engine, id: &str) -> Vec<String> {
    if engine.flatpak == "net.pcsx2.PCSX2" {
        vec!["--".into(), "{game}".into()]
    } else {
        default_args(id)
    }
}
fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
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
    let profile = EmulatorProfile {
        emulator_name: engine.name.into(),
        emulator_path: path.to_string_lossy().into_owned(),
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
            ready: executable(&path),
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
fn auto_link(settings: &mut Settings, report: &mut Report) {
    for spec in SPECS {
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
            {
                continue;
            }
        }
        let choices: Vec<_> = report
            .candidates
            .iter()
            .filter(|c| c.ready && c.console_ids.iter().any(|id| id == spec.id))
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
pub(crate) fn discover(settings: &mut Settings) -> Result<Report, String> {
    let mut report = Report::default();
    let mut map = BTreeMap::new();
    match emulators::list_flatpaks() {
        Ok(apps) => add_flatpaks(&mut map, &apps),
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
        if path.extension().is_some_and(|e| e == "desktop")
            && fs::metadata(&path).is_ok_and(|m| m.len() < 65536)
        {
            if let Ok(text) = fs::read_to_string(path) {
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
        for path in [detect_emulator(spec), detect_installed_emulator(spec.id)]
            .into_iter()
            .flatten()
        {
            let known = path
                .file_name()
                .and_then(|s| s.to_str())
                .and_then(engine_for_name);
            if known.is_some_and(|e| !e.consoles.contains(&spec.id)) {
                continue;
            }
            let image_ext = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("appimage"));
            if image_ext && known.is_some() && !appimage(&path) {
                continue;
            }
            let path = path.canonicalize().unwrap_or(path);
            insert(
                &mut map,
                Candidate {
                    id: format!("file|{}", path.display()),
                    console_ids: vec![spec.id.into()],
                    profile: EmulatorProfile {
                        emulator_name: known
                            .map(|e| e.name)
                            .unwrap_or(guessed_emulator(spec.id))
                            .into(),
                        emulator_path: path.to_string_lossy().into_owned(),
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
                    ready: executable(&path),
                },
            );
        }
    }
    report.candidates = map.into_values().collect();
    auto_link(settings, &mut report);
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
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
