# Console game launch presets — checked 2026-11-05

Matrix Play v0.5.6 identifies a known engine by its exact Flatpak ID or an anchored executable/AppImage basename, restricted to that engine's console capabilities. A user-editable emulator display name is never enough. Game launches add the options below; Open emulator sends none of these game-only arguments. Emulator versions, setup/firmware requirements, errors and in-game overlays can still affect what is displayed. Matrix Play does not edit emulator configuration files to force fullscreen.

Custom/unknown launchers keep their advanced literal argv. They need their own documented fullscreen/menu-hiding options. Steam and Windows retain their existing launch behavior; those games/runners control fullscreen themselves.

| Engine | Game launch options added | Upstream evidence |
| --- | --- | --- |
| PCSX2 | `-nogui -fullscreen`, filename after `--` | [QtHost command line](https://github.com/PCSX2/pcsx2/blob/master/pcsx2-qt/QtHost.cpp); also checked with the installed Flatpak's `-help` |
| DuckStation | `-nogui -fullscreen`, filename after `--` | [QtHost command line](https://github.com/stenzek/duckstation/blob/master/src/duckstation-qt/qthost.cpp) |
| RPCS3 | `--no-gui --fullscreen` | [RPCS3 command line](https://github.com/RPCS3/rpcs3/blob/master/rpcs3/rpcs3.cpp) |
| RMG | `--fullscreen --nogui --quit-after-emulation` | [RMG options](https://github.com/Rosalie241/RMG/blob/master/Source/RMG/main.cpp), explicitly documents hidden menu/toolbar/statusbar |
| Dolphin | `-b -e {game}`, `-C Dolphin.Display.Fullscreen=True` | [Batch/config parser](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/UICommon/CommandLineParse.cpp), [fullscreen setting](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/Config/MainSettings.cpp); command-line config layer is not saved |
| PPSSPP | `--fullscreen` | [CLI options](https://github.com/hrydgard/ppsspp/blob/master/Core/CmdLine.cpp) |
| Flycast | `-config window:fullscreen=yes` | [Upstream manual](https://github.com/flyinghead/flycast/blob/master/shell/linux/man/flycast.1) |
| Cemu | `--fullscreen`, existing `-g {game}` | [Launch settings](https://github.com/cemu-project/Cemu/blob/main/src/config/LaunchSettings.cpp); fullscreen is requested, but emulator menu/overlay preferences still apply |
| SameBoy SDL | `--fullscreen` | [SDL entry point](https://github.com/LIJI32/SameBoy/blob/master/SDL/main.c) |
| mGBA | `--fullscreen` | [Command-line parser](https://github.com/mgba-emu/mgba/blob/master/src/feature/commandline.c) |
| melonDS | `--fullscreen` | [Qt/SDL options](https://github.com/melonDS-emu/melonDS/blob/master/src/frontend/qt_sdl/CLI.cpp) |
| Azahar | `--fullscreen` | [Qt entry point](https://github.com/azahar-emu/azahar/blob/master/src/citra_qt/citra_qt.cpp) |
| Ryujinx | `--fullscreen` | [Retained source in a maintained fork](https://github.com/NextendoNetwork/Ryujinx-Nextendo/blob/81dfe2f7cede0f96c49ffdb0ad7b05b3f551fc9e/src/Ryujinx/Utilities/CommandLineState.cs); no game/version compatibility certification |
| Eden | `-f -g {game}` | [Project command-line documentation](https://git.eden-emu.dev/eden-emu/eden/src/branch/master/docs/user/CommandLine.md); original Switch only |
| shadPS4 core | `--fullscreen true {game}` | [Project usage](https://github.com/shadps4-emu/shadPS4#usage); experimental PS4 support |
| shadPS4 QtLauncher | `-d -g {game} -- --fullscreen true` | [Launcher parser](https://github.com/shadps4-emu/shadps4-qtlauncher/blob/main/src/main.cpp); `-d` selects the configured core, launcher GUI stays hidden; [Flathub manifest](https://github.com/flathub/net.shadps4.shadPS4/blob/master/net.shadps4.shadPS4.yaml) starts this launcher, not the core |
| Current Vita3K | `--frontend sdl --fullscreen --keep-config {game}` | [Vita3K parser](https://github.com/Vita3K/Vita3K/blob/master/vita3k/config/src/config.cpp), [entry point](https://github.com/Vita3K/Vita3K/blob/master/vita3k/main.cpp); current SDL frontend is the plain game window. VPK/ZIP content is installed and run by Vita3K; older versions may lack these options |

Current Vita3K is detected as an AppImage/native executable. Matrix has no verified Vita Flatpak ID in its automatic discovery catalog. The shadPS4 Flatpak is `net.shadps4.shadPS4`; setup must first choose a compatible downloaded core. PCSX2 keeps its per-launch runtime-font workaround and unfinished-setup check.

PS4 scanning accepts extracted `eboot.bin` only beside `sce_sys/param.sfo`, or explicit ELF files. PS4 PKG installation packages are excluded from game launches. Vita scanning accepts VPK/ZIP content archives, not arbitrary eboot files or a guessed installed title ID. Importing installed Vita3K title libraries from external configuration is not implemented.

PS5 and Switch 2 are **library-only collections**. Matrix has no verified Linux launch adapter for them. The backend blocks game launch, emulator open and emulator setup, while permitting local covers and console-scoped favorites. Stored PKG/ISO (PS5) and NSP/XCI (Switch 2) extensions are collection filters, not evidence of playable format support. PS4/original-Switch engine capabilities never extend to these successor consoles.

## Validation boundary

Rust tests check real option selection, conflicting saved options, exact engine/console identity and distinct shadPS4 launcher/core syntax. Native desktop tests run the real Matrix IPC and execute harmless fixture launchers to check PCSX2/RMG argv, paths with spaces, normal Open emulator behavior, log handling and blocked collection launches. They do not emulate games or prove every engine's fullscreen rendering. Chromium tests use a mock IPC bridge. The user confirmed real PCSX2 Flatpak game launching in v0.5.5 and, after installing/reopening v0.5.6, explicitly confirmed fullscreen gameplay with no PCSX2 menus. Other engines and PS4/Vita gameplay still need real hardware/game testing. No emulators, games, firmware, licenses or keys are distributed.


## Launch log lifetime

Open emulator preserves saved configuration selectors such as PCSX2 `-datapath`
and `-portable`, as literal argv tokens. It removes the game operand and known
game-only/headless/fullscreen flags instead of discarding all saved arguments.
PS4 extracted eboots use the same format check for scanning, favorites and local
artwork, including the required adjacent `sce_sys/param.sfo` file.

Console emulator stdout and stderr share an independent copy of the Matrix
executable running only its internal log-sink mode, before GUI initialization.
The sink has its own process group, survives Matrix closing, and keeps draining
until all emulator writers close. It retains the most recent 64 KiB in a private,
already-open log file and continues draining if writing the file fails. No shell
is involved. Launch details display the last 8 KiB of the rolling log, including
errors emitted after the first 64 KiB. Five-logs-per-console retention is unchanged.

After an emulator exits, Matrix allows up to 200 ms for the sink to finish before
publishing exit status and its diagnostic snapshot. It does not wait indefinitely
for descendants holding stdout open. Closing Matrix's own copies of the output
handles immediately after spawn lets short failures flush before status is shown.
This isolates logging from GUI lifetime; it does not guarantee survival of logout,
system shutdown, external kills, or an emulator's own failures.

Rust fixtures exercise bounded recent output, continued draining on disk failure,
and short failure diagnostics in returned launch status. These synthetic processes
do not emulate games or prove real-emulator behavior.

`python3 tests/test_launch_log_lifetime.py` compiles the production sink module
with rustc and checks process lifetime, late diagnostics, short EOF flushing, and
disk-write failure. It explicitly skips if rustc is unavailable. The GUI and IPC
remain outside this standalone test's coverage.

## Save files, Steam provenance and external diagnostics

Flatpak launches remain read-only for the managed console library by default.
For the recognized SameBoy Game Boy and mGBA Game Boy/GBA profiles, a game launch
also grants write access to that game's canonical containing directory so the
emulator can create battery saves, save states and their temporary replacements.
Nested titles do not grant writes to sibling game directories. A ROM directly in
the console's games directory needs that directory writable. Open emulator does
not add this permission. The ROM must resolve to a file within that console's
library; escaped symlinks and unrelated paths are rejected. Matrix does not move,
rewrite or delete any ROMs or existing saves. Filesystem mount flags are covered
by synthetic tests; actual SameBoy/mGBA saving still needs an installed-emulator
check.

Steam discovery retains the native or Flatpak origin of primary libraries and
libraries referenced by `libraryfolders.vdf`, including native aliases pointing
into Flatpak data. Launch uses that origin instead of whichever `steam` binary is
on PATH. A missing client is reported, never silently replaced with the other
installation. Duplicate app IDs remain one library entry and existing IDs,
favorites and artwork keys are unchanged.

Steam, Wine and `gio launch` now use the same bounded diagnostics and exit-status
monitoring as console emulators. Their library pages provide Launch details and
show launcher errors, including errors after the initial spawn. Polling slows
after startup and continues while Matrix is open. A successful launcher exit is
not proof of game startup: Steam or a desktop launcher can forward a request or
start another process. Matrix reports that distinction and retains captured
output, but cannot infer an unreported descendant process's exit status.

PS3 EBOOT titles use the directory above `USRDIR` for digital layouts, or above
`PS3_GAME/USRDIR` for disc layouts. Scan, favorite validation and artwork lookup
use the same title function without changing any paths.
