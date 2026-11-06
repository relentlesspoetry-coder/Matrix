//! Launch-time options only: never rewrite the emulator's persistent settings.
use super::*;

/// Identify the actual launcher, not its user-editable display label or the console alone.
/// Unknown/custom launchers keep their literal argv; users can supply their own fullscreen options.
pub(crate) fn game_args(profile: &EmulatorProfile, console: &str) -> Vec<String> {
    let mut args = if profile.launch_args.is_empty() {
        default_args(console)
    } else {
        profile.launch_args.clone()
    };
    let Some(engine) = discovery::identity(profile, console) else {
        return args;
    };
    let (flags, remove, pairs): (&[&str], &[&str], &[&str]) = match engine {
        "PCSX2" | "DuckStation" => (
            &["-nogui", "-fullscreen"],
            &["-nogui", "-fullscreen", "-nofullscreen", "-bigpicture"],
            &[],
        ),
        "RPCS3" => (
            &["--no-gui", "--fullscreen"],
            &["--no-gui", "--fullscreen"],
            &[],
        ),
        "RMG" => (
            &["--fullscreen", "--nogui", "--quit-after-emulation"],
            &[
                "-f",
                "--fullscreen",
                "-n",
                "--nogui",
                "-q",
                "--quit-after-emulation",
            ],
            &[],
        ),
        "Dolphin" => (&["-C", "Dolphin.Display.Fullscreen=True"], &[], &[]),
        "Flycast" => (&["-config", "window:fullscreen=yes"], &[], &[]),
        "Cemu" => (&["--fullscreen"], &["-f", "--fullscreen"], &[]),
        "SameBoy" | "mGBA" | "melonDS" | "Ryujinx" => {
            (&["--fullscreen"], &["-f", "--fullscreen"], &[])
        }
        "PPSSPP" => (&["--fullscreen"], &["--fullscreen", "--windowed"], &[]),
        "Azahar" => (
            &["--fullscreen"],
            &["-f", "--fullscreen", "--windowed"],
            &[],
        ),
        "Eden" => (&["-f"], &["-f", "--fullscreen"], &[]),
        "Vita3K" => (
            &["--frontend", "sdl", "--fullscreen", "--keep-config"],
            &["--fullscreen", "-F", "--keep-config", "-w"],
            &["--frontend"],
        ),
        "shadPS4" => (&["--fullscreen", "true"], &[], &["--fullscreen"]),
        "shadPS4 QtLauncher" => return shad_launcher(args),
        _ => return args,
    };
    // Remove only options before the filename delimiter. Tokens after it are data.
    let barrier = args.iter().position(|s| s == "--").unwrap_or(args.len());
    let mut tail = args.split_off(barrier);
    let mut kept = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if pairs.contains(&arg.as_str()) {
            iter.next();
            continue;
        }
        if remove.contains(&arg.as_str()) {
            continue;
        }
        // Per-launch fullscreen overrides must win over a saved windowed preference.
        if engine == "Dolphin" && (arg == "-C" || arg == "--config") {
            if let Some(value) = iter.next() {
                if !value.starts_with("Dolphin.Display.Fullscreen=") {
                    kept.extend([arg, value]);
                }
            }
            continue;
        }
        if engine == "Flycast" && arg == "-config" {
            if let Some(value) = iter.next() {
                if !value.starts_with("window:fullscreen=") {
                    kept.extend([arg, value]);
                }
            }
            continue;
        }
        kept.push(arg);
    }
    if engine == "Dolphin" {
        if !kept.iter().any(|s| s == "-b" || s == "--batch") {
            kept.insert(0, "-b".into());
        }
        if !kept
            .iter()
            .any(|s| s == "-e" || s == "--exec" || s == "--nand_title" || s == "-n")
        {
            let index = kept
                .iter()
                .position(|s| s == "{game}")
                .unwrap_or(kept.len());
            kept.insert(index, "-e".into());
        }
    }
    if engine == "Eden" && !kept.iter().any(|s| s == "-g" || s == "--game") {
        let index = kept
            .iter()
            .position(|s| s == "{game}")
            .unwrap_or(kept.len());
        kept.insert(index, "-g".into());
    }
    let mut result: Vec<String> = flags.iter().map(|s| (*s).into()).collect();
    result.extend(kept);
    result.append(&mut tail);
    // Protect Sony filenames with an explicit end-of-options marker.
    if matches!(engine, "PCSX2" | "DuckStation") && !result.iter().any(|s| s == "--") {
        let index = result
            .iter()
            .position(|s| s == "{game}")
            .unwrap_or(result.len());
        result.insert(index, "--".into());
    }
    result
}

fn shad_launcher(mut args: Vec<String>) -> Vec<String> {
    // Current QtLauncher has its own CLI. Core options go AFTER its -- barrier.
    let barrier = args.iter().position(|s| s == "--").unwrap_or(args.len());
    let core = args.split_off(barrier);
    args.retain(|s| !matches!(s.as_str(), "-s" | "--show-gui"));
    if !args
        .iter()
        .any(|s| matches!(s.as_str(), "-e" | "--emulator" | "-d"))
    {
        args.insert(0, "-d".into());
    }
    if !args.iter().any(|s| s == "-g" || s == "--game") {
        let index = args
            .iter()
            .position(|s| s == "{game}")
            .unwrap_or(args.len());
        args.insert(index, "-g".into());
        if !args.iter().any(|s| s == "{game}") {
            args.push("{game}".into());
        }
    }
    args.push("--".into());
    let mut core = core.into_iter().skip(1);
    while let Some(arg) = core.next() {
        if arg == "--fullscreen" {
            core.next();
        } else {
            args.push(arg);
        }
    }
    args.extend(["--fullscreen".into(), "true".into()]);
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile(path: &str, args: &[&str]) -> EmulatorProfile {
        EmulatorProfile {
            emulator_path: path.into(),
            launch_args: args.iter().map(|s| (*s).into()).collect(),
            ..Default::default()
        }
    }
    #[test]
    fn fullscreen_uses_engine_identity_and_preserves_custom_launchers() {
        let custom = EmulatorProfile {
            emulator_name: "PCSX2".into(),
            ..profile("/tmp/Custom.sh", &["--custom", "{game}"])
        };
        assert_eq!(game_args(&custom, "ps2"), ["--custom", "{game}"]);
        assert_eq!(
            game_args(
                &profile("/tmp/pcsx2-qt.AppImage", &["-nofullscreen", "--", "{game}"]),
                "ps2"
            ),
            ["-nogui", "-fullscreen", "--", "{game}"]
        );
        assert_eq!(
            game_args(&profile("/usr/bin/rpcs3", &["{game}"]), "ps3"),
            ["--no-gui", "--fullscreen", "{game}"]
        );
        assert_eq!(
            game_args(&profile("/usr/bin/RMG", &["{game}"]), "n64"),
            [
                "--fullscreen",
                "--nogui",
                "--quit-after-emulation",
                "{game}"
            ]
        );
        // No PS4/PS5 or original Switch/Switch 2 capability crossover.
        assert_eq!(
            game_args(&profile("/apps/shadPS4", &["{game}"]), "ps5"),
            ["{game}"]
        );
        assert_eq!(
            game_args(&profile("/apps/ryujinx", &["{game}"]), "switch2"),
            ["{game}"]
        );
    }
    #[test]
    fn config_overrides_are_per_launch_and_game_tokens_stay_literal() {
        assert_eq!(
            game_args(
                &profile(
                    "/usr/bin/dolphin-emu",
                    &["-C", "Dolphin.Display.Fullscreen=False", "-e", "{game}"]
                ),
                "wii"
            ),
            [
                "-C",
                "Dolphin.Display.Fullscreen=True",
                "-b",
                "-e",
                "{game}"
            ]
        );
        assert_eq!(
            game_args(
                &profile(
                    "/usr/bin/flycast",
                    &["-config", "window:fullscreen=no", "{game}"]
                ),
                "dreamcast"
            ),
            ["-config", "window:fullscreen=yes", "{game}"]
        );
        assert_eq!(
            game_args(
                &profile("/apps/Vita3K.AppImage", &["--frontend", "qt", "{game}"]),
                "vita"
            ),
            [
                "--frontend",
                "sdl",
                "--fullscreen",
                "--keep-config",
                "{game}"
            ]
        );
    }
    #[test]
    fn recognized_flatpaks_request_fullscreen_without_cross_console_flags() {
        for (id, console, flag) in [
            ("org.ppsspp.PPSSPP", "psp", "--fullscreen"),
            ("info.cemu.Cemu", "wiiu", "--fullscreen"),
            ("io.mgba.mGBA", "gba", "--fullscreen"),
            ("io.github.sameboy.SameBoy", "gb", "--fullscreen"),
            ("net.kuribo64.melonDS", "nds", "--fullscreen"),
            ("org.azahar_emu.Azahar", "3ds", "--fullscreen"),
            ("org.ryujinx.Ryujinx", "switch", "--fullscreen"),
            ("dev.eden_emu.eden", "switch", "-f"),
        ] {
            let p = EmulatorProfile {
                launcher: emulators::Launcher::Flatpak,
                flatpak_id: id.into(),
                ..profile("", &["{game}"])
            };
            let args = game_args(&p, console);
            assert!(args.iter().any(|s| s == flag), "{id}");
            assert_eq!(args.iter().filter(|s| s.as_str() == "{game}").count(), 1);
            assert_eq!(game_args(&p, "ps5"), ["{game}"]);
        }
    }
    #[test]
    fn qtlauncher_is_distinct_from_core_and_suppresses_launcher_gui() {
        let mut p = profile("", &["{game}"]);
        p.launcher = emulators::Launcher::Flatpak;
        p.flatpak_id = "net.shadps4.shadPS4".into();
        assert_eq!(
            game_args(&p, "ps4"),
            ["-d", "-g", "{game}", "--", "--fullscreen", "true"]
        );
        assert_eq!(
            game_args(
                &profile(
                    "/apps/shadPS4-qtlauncher.AppImage",
                    &[
                        "-d",
                        "--show-gui",
                        "-g",
                        "{game}",
                        "--",
                        "--fullscreen",
                        "false"
                    ]
                ),
                "ps4"
            ),
            ["-d", "-g", "{game}", "--", "--fullscreen", "true"]
        );
        assert_eq!(
            game_args(&profile("/apps/shadPS4", &["{game}"]), "ps4"),
            ["--fullscreen", "true", "{game}"]
        );
    }
}
