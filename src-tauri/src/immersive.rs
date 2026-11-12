//! Launch-time options only: never rewrite the emulator's persistent settings.
use super::*;

/// Never guess which existing Dolphin save/config directory a relative profile
/// meant: setup and game launch have different working directories.
pub(crate) fn validate_profile_paths(
    profile: &EmulatorProfile,
    console: &str,
) -> Result<(), String> {
    if discovery::identity(profile, console) != Some("Dolphin") {
        return Ok(());
    }
    let mut args = profile.launch_args.iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        if matches!(
            arg.as_str(),
            "-C" | "--config"
                | "-e"
                | "--exec"
                | "-n"
                | "--nand_title"
                | "-m"
                | "--movie"
                | "-s"
                | "--save_state"
                | "-v"
                | "--video_backend"
                | "-a"
                | "--audio_emulation"
        ) {
            args.next();
            continue;
        }
        let value = if arg == "-u" || arg == "--user" {
            Some(args.next().map(String::as_str).unwrap_or(""))
        } else if let Some(value) = arg.strip_prefix("--user=") {
            Some(value)
        } else {
            arg.strip_prefix("-u")
                .map(|value| value.strip_prefix('=').unwrap_or(value))
        };
        if value.is_some_and(|value| !Path::new(value).is_absolute()) {
            return Err("Dolphin's user-folder option (-u/--user) needs an absolute path. Choose Edit emulator and enter the full path of your existing Dolphin user folder. Relative paths can select different settings/saves in setup and game launch; Matrix has not moved files or changed the saved profile.".into());
        }
    }
    Ok(())
}

/// Open the same emulator configuration as game launches, without booting a game
/// or applying gameplay-only fullscreen/headless options. Do not call game_args:
/// its injected options deliberately hide the configuration interface.
pub(crate) fn setup_args(profile: &EmulatorProfile, console: &str) -> Vec<String> {
    let engine = discovery::identity(profile, console).unwrap_or("");
    // A custom PS5/Switch 2 command has no verified setup CLI. Game arguments
    // may include arbitrary option/value pairs, so never guess which to retain.
    // Open the chosen program with no arguments; explain this in setup.
    if custom_emulator_only(console) && engine != "Lichii" {
        return Vec::new();
    }
    // Mednafen is configured through its own files/hotkeys. Do not pass a game
    // or module on an "Open emulator" request; xemu keeps its normal setup UI.

    let (flags, pairs): (&[&str], &[&str]) = match engine {
        "Mednafen" => (&[], &["-force_module", "-video.fs"]),
        "xemu" => (&["-full-screen"], &["-dvd_path"]),
        "PCSX2" => (
            &[
                "-batch",
                "-nogui",
                "-fullscreen",
                "-nofullscreen",
                "-bigpicture",
                "-fastboot",
                "-slowboot",
                "-resume",
            ],
            &["-state"],
        ),
        "DuckStation" => (
            &[
                "-batch",
                "-nogui",
                "-fullscreen",
                "-nofullscreen",
                "-bigpicture",
                "-fastboot",
                "-slowboot",
                "-resume",
            ],
            &["-state"],
        ),
        "RPCS3" => (&["--no-gui", "--fullscreen"], &[]),
        "RMG" => (
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
        "Dolphin" => (&["-b", "--batch"], &["-e", "--exec", "-n", "--nand_title"]),
        "Cemu" => (&["-f", "--fullscreen"], &["-g"]),
        "Eden" | "Lichii" => (&["-f", "--fullscreen"], &["-g", "--game"]),
        "SameBoy" | "mGBA" | "melonDS" | "Ryujinx" | "Azahar" => {
            (&["-f", "--fullscreen", "--windowed"], &[])
        }
        "PPSSPP" => (&["--fullscreen", "--windowed"], &[]),
        "Vita3K" => (
            &["--fullscreen", "-F", "--keep-config", "-w"],
            &["--run", "-r", "--frontend"],
        ),
        "shadPS4" => (&[], &["--fullscreen", "-g", "--game"]),
        "shadPS4 QtLauncher" => (&["-d"], &["-g", "--game"]),
        _ => (&[], &[]),
    };
    let mut result = Vec::new();
    let mut args = profile.launch_args.iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            // The suffix is the game operand (or QtLauncher's core-only argv),
            // never settings for the setup interface.
            break;
        }
        if pairs.contains(&arg.as_str()) {
            args.next();
            continue;
        }
        if arg == "{game}" || flags.contains(&arg.as_str()) {
            continue;
        }
        if (engine == "Dolphin" && (arg == "-C" || arg == "--config"))
            || (engine == "Flycast" && arg == "-config")
        {
            if let Some(value) = args.next() {
                let fullscreen = if engine == "Dolphin" {
                    "Dolphin.Display.Fullscreen="
                } else {
                    "window:fullscreen="
                };
                if !value.starts_with(fullscreen) {
                    result.extend([arg.clone(), value.clone()]);
                }
            }
            continue;
        }
        // Preserve configuration paths as literal tokens, even if a pathname
        // happens to look like an option. These select the settings being edited.
        if matches!(
            arg.as_str(),
            "-datapath" | "--config" | "-c" | "--configdir" | "--config-dir" | "--user" | "-u"
        ) {
            result.push(arg.clone());
            if let Some(value) = args.next() {
                result.push(value.clone());
            }
            continue;
        }
        result.push(arg.clone());
    }
    result
}

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
    if engine == "Mednafen" || engine == "xemu" {
        let mut kept = Vec::new();
        let mut values = args.into_iter();
        while let Some(value) = values.next() {
            if (engine == "Mednafen" && matches!(value.as_str(), "-force_module" | "-video.fs"))
                || (engine == "xemu" && value == "-dvd_path") {
                values.next();
            } else if !(engine == "xemu" && value == "-full-screen") {
                kept.push(value);
            }
        }
        if engine == "Mednafen" {
            let module = match console { "nes" => "nes", "snes" => "snes_faust", "genesis" => "md", "saturn" => "ss", _ => unreachable!() };
            let mut result = vec!["-video.fs".into(), "1".into(), "-force_module".into(), module.into()];
            result.extend(kept);
            return result;
        }
        let index = kept.iter().position(|v| v == "{game}").unwrap_or(kept.len());
        kept.insert(index, "-dvd_path".into());
        kept.insert(0, "-full-screen".into());
        return kept;
    }
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
        "Eden" | "Lichii" => (&["-f"], &["-f", "--fullscreen"], &[]),
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
    if matches!(engine, "Eden" | "Lichii") && !kept.iter().any(|s| s == "-g" || s == "--game") {
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
    // A trailing game option relies on Matrix's automatic operand append.
    // Resolve it here, before the launcher/core delimiter is introduced.
    if args.last().is_some_and(|arg| arg == "-g" || arg == "--game") {
        args.push("{game}".into());
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
    #[test]
    fn lichii_switch2_gets_fullscreen_without_touching_custom_or_original_switch() {
        for file in ["/tmp/Lichii-nx2.AppImage", "/tmp/Fritchii-130.AppImage"] {
            let p = profile(file, &["--config", "/tmp/user config", "{game}"]);
            assert_eq!(game_args(&p,"switch2"), ["-f","--config","/tmp/user config","-g","{game}"]);
            assert_eq!(setup_args(&p,"switch2"),["--config","/tmp/user config"]);
            assert_eq!(game_args(&p,"switch"),p.launch_args);
        }
        let custom=profile("/tmp/CustomSwitch2.AppImage",&["--own","{game}"]);
        assert_eq!(game_args(&custom,"switch2"),custom.launch_args);
    }
    fn profile(path: &str, args: &[&str]) -> EmulatorProfile {
        EmulatorProfile {
            emulator_path: path.into(),
            launch_args: args.iter().map(|s| (*s).into()).collect(),
            ..Default::default()
        }
    }
    #[test]
    fn setup_keeps_the_saved_configuration_and_removes_game_only_arguments() {
        let p = profile(
            "/usr/bin/pcsx2-qt",
            &[
                "-portable",
                "-datapath",
                "/tmp/My settings",
                "-nogui",
                "-fullscreen",
                "--",
                "{game}",
            ],
        );
        assert_eq!(
            setup_args(&p, "ps2"),
            ["-portable", "-datapath", "/tmp/My settings"]
        );
        assert_eq!(p.launch_args.last().unwrap(), "{game}");
        assert_eq!(
            setup_args(
                &profile(
                    "/usr/bin/dolphin-emu",
                    &[
                        "-u",
                        "/tmp/Dolphin user",
                        "-C",
                        "Dolphin.Display.Fullscreen=True",
                        "-b",
                        "-e",
                        "{game}"
                    ]
                ),
                "wii"
            ),
            ["-u", "/tmp/Dolphin user"]
        );
        assert_eq!(
            setup_args(
                &profile(
                    "/usr/bin/cemu",
                    &["--config", "/tmp/config.xml", "-g", "{game}"]
                ),
                "wiiu"
            ),
            ["--config", "/tmp/config.xml"]
        );
        assert_eq!(
            setup_args(
                &profile(
                    "/apps/shadPS4QtLauncher",
                    &["-d", "-g", "{game}", "--", "--fullscreen", "true"]
                ),
                "ps4"
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            setup_args(
                &profile(
                    "/apps/Vita3K.AppImage",
                    &["--frontend", "sdl", "--fullscreen", "{game}"]
                ),
                "vita"
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn setup_supports_flatpak_and_preserves_custom_configuration_tokens() {
        let mut p = profile(
            "",
            &["-datapath", "/tmp/Flatpak settings", "-portable", "{game}"],
        );
        p.launcher = emulators::Launcher::Flatpak;
        p.flatpak_id = "net.pcsx2.PCSX2".into();
        assert_eq!(
            setup_args(&p, "ps2"),
            ["-datapath", "/tmp/Flatpak settings", "-portable"]
        );
        assert_eq!(
            setup_args(
                &profile(
                    "/tmp/custom.sh",
                    &["--custom-config", "/tmp/settings", "{game}"]
                ),
                "ps2"
            ),
            ["--custom-config", "/tmp/settings"]
        );
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
    #[test]
    fn qtlauncher_automatic_game_operand_precedes_core_arguments() {
        let game = Path::new("/library/Title with spaces/eboot.bin");
        for option in ["-g", "--game"] {
            for with_core in [false, true] {
                let mut args = vec!["-d", option];
                if with_core {
                    args.extend(["--", "--fullscreen", "false", "--log"]);
                }
                let p = profile("/apps/shadPS4-qtlauncher.AppImage", &args);
                let actual = console_launch_args(&p, "ps4", game).unwrap();
                let mut expected = vec!["-d", option, game.to_str().unwrap(), "--"];
                if with_core {
                    expected.push("--log");
                }
                expected.extend(["--fullscreen", "true"]);
                assert_eq!(actual, expected);
                assert_eq!(
                    actual.iter().filter(|arg| arg.as_str() == game.to_str().unwrap()).count(),
                    1
                );
            }
        }
        // A supplied placeholder remains a single game operand.
        let p = profile("/apps/shadPS4-qtlauncher.AppImage", &["-g", "{game}"]);
        assert_eq!(console_launch_args(&p, "ps4", game).unwrap(),
            ["-d", "-g", game.to_str().unwrap(), "--", "--fullscreen", "true"]);
    }

    #[test]
    fn dolphin_user_folder_validation_does_not_guess_existing_save_paths() {
        for args in [
            vec!["-u", "relative"],
            vec!["--user", "relative"],
            vec!["--user=relative"],
            vec!["-urelative"],
            vec!["-u=relative"],
            vec!["-u"],
            vec!["--user="],
        ] {
            let p = profile("/apps/dolphin-emu", &args);
            assert!(
                validate_profile_paths(&p, "gamecube")
                    .unwrap_err()
                    .contains("absolute path"),
                "{args:?}"
            );
        }
        for args in [
            vec!["-u", "/existing/user"],
            vec!["--user=/existing/user"],
            vec!["-u/existing/user"],
            vec!["-u=/existing/user"],
            vec!["{game}"],
            vec!["--", "-urelative"],
            vec!["-C", "--user=literal-value"],
            vec!["-m", "-user-recording.dtm"],
            vec!["--save_state", "-user-state.sav"],
            vec!["-v", "-unknown-backend"],
            vec!["--audio_emulation", "-user-value"],
        ] {
            assert!(
                validate_profile_paths(&profile("/apps/dolphin-emu", &args), "gamecube").is_ok(),
                "{args:?}"
            );
        }
        let mut flatpak = profile("", &["--user=relative"]);
        flatpak.launcher = emulators::Launcher::Flatpak;
        flatpak.flatpak_id = "org.DolphinEmu.dolphin-emu".into();
        assert!(validate_profile_paths(&flatpak, "wii").is_err());
        assert!(validate_profile_paths(
            &profile("/apps/custom-script", &["--user", "account-id"]),
            "gamecube"
        )
        .is_ok());
        assert!(
            validate_profile_paths(&profile("/apps/pcsx2", &["--user", "account-id"]), "ps2")
                .is_ok()
        );
    }
}
