//! Read-only emulator activity detection. No process is signalled or modified.
use super::*;
use std::{
    io::Read,
    os::unix::fs::MetadataExt,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Info {
    pub active: bool,
    pub emulators: Vec<String>,
}
fn bytes(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let mut b = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(limit as u64 + 1)
        .read_to_end(&mut b)
        .ok()?;
    if b.len() > limit {
        None
    } else {
        Some(b)
    }
}
fn recognized(path: &Path) -> Option<String> {
    let text = path.to_string_lossy();
    let name = Path::new(text.trim_end_matches(" (deleted)"))
        .file_name()?
        .to_str()?;
    if name.eq_ignore_ascii_case("retroarch") {
        return Some("RetroArch".into());
    }
    discovery::process_engine(name).map(str::to_owned)
}
fn flatpak_name(id: &str) -> Option<String> {
    if id == "org.libretro.RetroArch" {
        return Some("RetroArch".into());
    }
    discovery::flatpak_engine(id).map(str::to_owned)
}
fn image_match(
    exe: &Path,
    image: &str,
    appdir: &str,
    profiles: &BTreeMap<String, EmulatorProfile>,
) -> Option<String> {
    if image.is_empty() || appdir.is_empty() {
        return None;
    }
    let image = Path::new(image).canonicalize().ok()?;
    let appdir = Path::new(appdir).canonicalize().ok()?;
    if !image.is_file() || !appdir.is_dir() || !exe.starts_with(&appdir) {
        return None;
    }
    profiles
        .iter()
        .find(|(_, p)| {
            p.launcher != emulators::Launcher::Flatpak
                && Path::new(&p.emulator_path).canonicalize().ok().as_ref() == Some(&image)
        })
        .and_then(|(id, _)| spec(id).ok().map(|s| s.dir.into()))
}
fn process_match(
    exe: &Path,
    exe_meta: Option<&fs::Metadata>,
    args: &[String],
    flatpak: &str,
    profiles: &BTreeMap<String, EmulatorProfile>,
    native_files: &BTreeMap<String, (u64, u64)>,
) -> Option<String> {
    if let Some(name) = recognized(exe) {
        return Some(name);
    }
    if let Some(name) = flatpak_name(flatpak) {
        return Some(name);
    }
    for (id, p) in profiles {
        if p.launcher == emulators::Launcher::Flatpak {
            if !flatpak.is_empty() && p.flatpak_id == flatpak {
                return Some(spec(id).ok()?.dir.into());
            }
            // Inventory commands and editors referencing an app ID are not emulators.
            if exe.file_name().is_some_and(|n| n == "flatpak")
                && args.get(1).is_some_and(|a| a == "run")
                && args.iter().skip(2).any(|a| a == &p.flatpak_id)
            {
                return Some(spec(id).ok()?.dir.into());
            }
            continue;
        }
        if p.emulator_path.is_empty() {
            continue;
        }
        let same = exe_meta
            .zip(native_files.get(id))
            .is_some_and(|(a, (dev, ino))| a.dev() == *dev && a.ino() == *ino);
        let interpreter = exe.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            matches!(
                n,
                "sh" | "bash" | "dash" | "zsh" | "python" | "python3" | "perl" | "ruby"
            )
        });
        let argument = args.first().is_some_and(|v| v == &p.emulator_path)
            || (interpreter && args.get(1).is_some_and(|v| v == &p.emulator_path));
        if same || argument {
            return Some(spec(id).ok()?.dir.into());
        }
    }
    None
}
pub(crate) fn scan(profiles: &BTreeMap<String, EmulatorProfile>) -> Result<Info, String> {
    let native_files: BTreeMap<_, _> = profiles
        .iter()
        .filter(|(_, p)| p.launcher != emulators::Launcher::Flatpak)
        .filter_map(|(id, p)| {
            fs::metadata(&p.emulator_path)
                .ok()
                .map(|m| (id.clone(), (m.dev(), m.ino())))
        })
        .collect();
    let mut names = HashSet::new();
    let current = std::env::current_exe().ok();
    for entry in fs::read_dir("/proc")
        .map_err(|_| "Cannot inspect emulator activity")?
        .flatten()
    {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if pid == std::process::id()
            || !entry
                .metadata()
                .is_ok_and(|m| m.uid() == unsafe { libc::geteuid() })
        {
            continue;
        }
        let root = entry.path();
        let Some(stat) = bounded_io::read_text(&root.join("stat"), 16384) else {
            continue;
        };
        if stat
            .rsplit_once(')')
            .is_some_and(|(_, s)| matches!(s.split_whitespace().next(), Some("Z" | "X")))
        {
            continue;
        }
        let Ok(exe) = fs::read_link(root.join("exe")) else {
            continue;
        };
        if current.as_ref() == Some(&exe) {
            continue;
        }
        let args: Vec<String> = bytes(&root.join("cmdline"), 16384)
            .unwrap_or_default()
            .split(|b| *b == 0)
            .filter(|a| !a.is_empty())
            .map(|a| String::from_utf8_lossy(a).into())
            .collect();
        let env = bytes(&root.join("environ"), 262144).unwrap_or_default();
        let flatpak = env
            .split(|b| *b == 0)
            .find_map(|v| v.strip_prefix(b"FLATPAK_ID="))
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("");
        let variable = |key: &[u8]| {
            env.split(|b| *b == 0)
                .find_map(|v| v.strip_prefix(key))
                .and_then(|v| std::str::from_utf8(v).ok())
                .unwrap_or("")
        };
        if let Some(name) =
            image_match(&exe, variable(b"APPIMAGE="), variable(b"APPDIR="), profiles)
        {
            names.insert(name);
        }
        if let Some(name) = process_match(
            &exe,
            fs::metadata(root.join("exe")).ok().as_ref(),
            &args,
            flatpak,
            profiles,
            &native_files,
        ) {
            names.insert(name);
        }
    }
    // Owned session clocks cover wrappers that have already handed off a game.
    let lock_dir = config_file()?
        .parent()
        .ok_or("Invalid configuration path")?
        .join("playtime-locks");
    if let Ok(entries) = fs::read_dir(lock_dir) {
        for e in entries.flatten() {
            if !e.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            if let Ok(f) = fs::File::open(e.path()) {
                if f.try_lock().is_err() {
                    names.insert("Tracked emulator session".into());
                    break;
                }
            }
        }
    }
    let mut emulators: Vec<_> = names.into_iter().collect();
    emulators.sort();
    Ok(Info {
        active: !emulators.is_empty(),
        emulators,
    })
}
static CACHE: OnceLock<Mutex<Option<(Instant, Info)>>> = OnceLock::new();
pub(crate) fn status() -> Result<Info, String> {
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "Emulator monitor unavailable")?;
    if let Some((time, info)) = &*cache {
        if time.elapsed() < Duration::from_secs(2) {
            return Ok(info.clone());
        }
    }
    let settings = load_settings()?;
    let info = scan(&settings.profiles)?;
    *cache = Some((Instant::now(), info.clone()));
    Ok(info)
}
pub(crate) fn active() -> bool {
    status().is_ok_and(|i| i.active)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_paths_and_real_emulator_names_do_not_match_editors_or_inventory() {
        let profiles = BTreeMap::from([
            (
                "ps2".into(),
                EmulatorProfile {
                    emulator_path: "/emulators/Custom.sh".into(),
                    ..Default::default()
                },
            ),
            (
                "ps1".into(),
                EmulatorProfile {
                    launcher: emulators::Launcher::Flatpak,
                    flatpak_id: "org.example.Emulator".into(),
                    ..Default::default()
                },
            ),
        ]);
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(process_match(
            Path::new("/usr/bin/dash"),
            None,
            &args(&["sh", "/emulators/Custom.sh"]),
            "",
            &profiles,
            &BTreeMap::new()
        )
        .is_some());
        assert!(process_match(
            Path::new("/usr/bin/editor"),
            None,
            &args(&["editor", "/emulators/Custom.sh"]),
            "",
            &profiles,
            &BTreeMap::new()
        )
        .is_none());
        assert!(process_match(
            Path::new("/usr/bin/flatpak"),
            None,
            &args(&["flatpak", "list", "org.example.Emulator"]),
            "",
            &profiles,
            &BTreeMap::new()
        )
        .is_none());
        assert!(process_match(
            Path::new("/usr/bin/flatpak"),
            None,
            &args(&["flatpak", "run", "org.example.Emulator"]),
            "",
            &profiles,
            &BTreeMap::new()
        )
        .is_some());
        assert!(process_match(
            Path::new("/tmp/.mount/AppDir/usr/bin/pcsx2-qt"),
            None,
            &[],
            "",
            &profiles,
            &BTreeMap::new()
        )
        .is_some());
        assert!(process_match(
            Path::new("/usr/bin/unknown"),
            None,
            &[],
            "org.ppsspp.PPSSPP",
            &profiles,
            &BTreeMap::new()
        )
        .is_some());
    }
    #[test]
    fn extracted_images_require_the_running_executable_to_belong_to_that_image() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("Custom.AppImage");
        fs::write(&image, b"fixture").unwrap();
        let appdir = root.path().join("AppDir");
        fs::create_dir(&appdir).unwrap();
        let profiles = BTreeMap::from([(
            "ps5".into(),
            EmulatorProfile {
                emulator_path: image.to_str().unwrap().into(),
                ..Default::default()
            },
        )]);
        assert!(image_match(
            &appdir.join("AppRun"),
            image.to_str().unwrap(),
            appdir.to_str().unwrap(),
            &profiles
        )
        .is_some());
        assert!(image_match(
            Path::new("/usr/bin/browser"),
            image.to_str().unwrap(),
            appdir.to_str().unwrap(),
            &profiles
        )
        .is_none());
    }
    #[test]
    fn detects_an_external_configured_emulator_and_releases_without_signalling() {
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("external.sh");
        fs::write(&script, "#!/bin/sh\nsleep 10\n").unwrap();
        let mut child = Command::new("/bin/sh").arg(&script).spawn().unwrap();
        let profiles = BTreeMap::from([(
            "ps2".into(),
            EmulatorProfile {
                emulator_path: script.to_str().unwrap().into(),
                ..Default::default()
            },
        )]);
        for _ in 0..20 {
            if scan(&profiles)
                .unwrap()
                .emulators
                .contains(&"PS2".to_string())
            {
                child.kill().unwrap();
                child.wait().unwrap();
                assert!(!scan(&profiles)
                    .unwrap()
                    .emulators
                    .contains(&"PS2".to_string()));
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = child.kill();
        let _ = child.wait();
        panic!("External emulator was not detected");
    }
}
