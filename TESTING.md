# Test Matrix Play on Linux

## Version 0.5.10 reviewed bug repairs

The repair covers scoped melonDS Flatpak sidecar-save permissions, complete native
emulator discovery, async dialog ownership, draft artwork identity preservation
and accessible names on narrow navigation. New tests exercise multiple installed
choices, aliased executables, ambiguous selections, nested/direct ROM parents,
outside-root and symlink rejection, repeated form submissions and newer dialogs.
The desktop suite also checks melonDS permission arguments through actual Tauri IPC
with a harmless launcher. These tests do not prove real emulator gameplay or save
creation, and they do not certify additional distributions or hardware.

Local checks use Rust 1.99.1. GTK/WebKit development packages are absent in this
executor; the exact release commit must pass the Linux CI native-window and
DEB/AppImage packaging gates before publication.

## Version 0.5.9 keyboard and desktop-shortcut checks

The Windows library accepts only existing `.desktop` shortcuts and dispatches
through `gio launch`. There is no direct Windows executable/runtime launch path.
New backend/native regression cases reject direct `.exe` registration and saved
legacy launches, preserve legacy entries and installed files, and verify that the
Windows UI explains the limitation while Steam routing remains unchanged.
The native tests use harmless `gio` and Steam fixtures, not real gameplay.

Final removal commit `0f87126ddc91447e975d2c6f8add3f23a9c97399` passed
[GitHub Actions run 42277673138](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42277673138):
13 updater-core tests, 12 cover-core tests (the opt-in provider test was skipped),
47 backend cases in both headless and desktop builds, all 52 JavaScript fixtures,
folder/release/package/log-lifetime checks, DEB/AppImage builds, packaged-helper
validation, ordinary painted startup, and the actual Tauri/WebKit suite. The
native suite passed the keyboard boundaries and desktop-only/legacy-preservation
checks together. Versioned main runs these gates again before publication.

The emulator setup dialog's focus trap omitted both the Advanced launch options
summary and its multiline arguments field. Five new fixture cases failed before
the repair. All ten keyboard cases and all 52 production-JavaScript fixture tests
pass after it, along with the folder, release/signing and DEB packaging fixtures.
Native testing also reproduced GTK/WebKit reporting Shift+Tab as an unidentified
key, allowing focus to escape behind the dialog; the trap now recognizes Tab
through its physical key code or legacy key code too. The native desktop suite uses WebDriver key actions to verify Tab/Shift+Tab
through expanded and collapsed advanced settings, hidden Flatpak/file controls,
and the dialog boundaries. The release pipeline must pass these native checks
on its exact commit before publication.

Current local Rust and native-browser checks were unavailable: no Rust toolchain
is installed and Chromium cannot open its IPC socket in this executor. The
GitHub Linux workflow runs the full Rust/build/native gates. No new definite
save-permission, Steam/desktop-shortcut or updater bug was found in the targeted source
review. Synthetic launchers are not real Steam/desktop-shortcut gameplay or emulator save
creation, and these checks do not perform privileged installation. This repair
improves keyboard access; it does not add or certify physical gamepad support.

## Version 0.5.8 repair verification — 2026-11-06

The combined repairs were integrated against main
`c13edefbe81e73a6f3200d2a1ef57fe28baf83de`. The exact repair commit
`b981213b1e2126244331a4d14dcf2ead6f0ca083` passed
[GitHub Actions run 42248122522](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42248122522):
all Rust/JavaScript/Python regressions, DEB and AppImage builds, packaged helper
validation, ordinary painted startup, and the real Tauri/WebKit desktop fixture
suite. The release pipeline runs these gates again on versioned main before
signing and publishing 0.5.8.
Local Rust 1.99.1 checks passed: 13 updater-core tests, 12 cover-core tests
(the opt-in live-provider test was skipped), and 45 headless native-backend
tests. The release-mode `matrix-install-update` helper also compiled.

`node --test tests/test_*.cjs` passed 37 production-JavaScript fixture tests,
including delayed launch failures, dialog ownership, artwork transactions,
stale requests, updater focus, safe Cancel/Install transitions and repeated or
interrupted operations. CI runs this
complete glob rather than only the artwork suite. Folder structure, all three
launch-log lifetime tests, all three DEB packaging fixtures, release/signing
fixtures, Rust formatting, JavaScript/Python syntax and patch whitespace checks
also passed. The release fixtures use disposable local remotes and keys, not
the project's GitHub repository or signing credentials.

The native suite verified scoped sidecar-save Flatpak arguments, native/Flatpak
Steam routing, delayed Windows runner diagnostics, artwork round-trips and
settings/favorites across restart using harmless launchers. An initial CI run
stopped because the added test expected "exit status 17" instead of Rust's
"exit status: 17". The corrected assertion compares the actual backend message
with the visible error notice and still requires the exit code and captured
diagnostic; production launch code did not change for that correction.

These are synthetic launch tests, not real emulator/Steam/desktop-shortcut gameplay, real
save-file creation or a privileged package upgrade. Local desktop testing was
blocked by missing GTK/WebKit development packages and restricted browser IPC;
the full desktop and built-package checks above ran in Linux CI instead.

## Reviewed-fix verification

The integrated fixes were checked locally with Rust 1.99.1: updater-core passed
13 tests, cover-core passed 12 tests (its opt-in live-provider test was not run),
and the headless native backend passed 32 tests. The production launch-log
harness passed all three tests, the release-mode privileged helper compiled,
and the artwork suite passed seven tests. Rust formatting, folder layout,
release/signing fixtures and syntax checks also passed. These tests use synthetic
data and do not perform a privileged package upgrade or real emulator gameplay.

The local desktop build stopped at missing GLib/GTK/WebKit development packages.
The browser test could not start because the environment restricts local IPC
sockets. Native desktop and built-installer verification therefore run in the
Linux CI job on the fix branch; check that exact commit's job before treating
those checks as passed. CI inspects the built DEB helper's path, root ownership,
executable permissions and ELF architecture without installing the package.
The publish job is restricted to main and does not run for fix-branch pushes.

The exact reviewed-fix commit `2909e04b725cff3fc03ec296e098adc0b0dacb54`
passed [GitHub Actions run 42213232803](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42213232803):
Rust/regression checks, Linux DEB and AppImage builds, packaged helper validation,
ordinary painted startup and the actual native desktop fixture suite. Publishing
was skipped for that fix-branch run. Release 0.5.7 runs the same gates again on
its versioned main commit before signing and publication. These checks do not
perform a privileged installation or real emulator gameplay.

## Backend regression checks

The headless backend suite includes separate-process concurrent settings writers,
discovery merging after another window saves, failed-transaction preservation,
PS4 eboot favorites/artwork, and setup argument preservation. Every settings
writer uses the same persistent lock sidecar and unique atomic-replace temporary;
discovery scans outside the lock and applies candidates to the latest settings.
This protects cooperating current-version processes, not older versions that do
not use the lock.

The update-core suite also checks the signed privileged-install protocol, changed
package bytes and protected staging. `python3 tests/test_launch_log_lifetime.py`
checks output after the launching process exits and log-write failure using
synthetic subprocesses. Run the normal Rust suites and desktop tests as well;
these checks do not replace a real emulator test or privileged DEB upgrade test.

This is the native testing release, version 0.5.9. GitHub Actions builds the installers on Ubuntu 24.04 for **x86-64 / amd64** computers. Earlier installed-app testing used Zorin OS 18.1 (Ubuntu 24.04 base). Other distributions and ARM systems have not been validated.

## Install the DEB — recommended for Zorin 18 / Ubuntu 24.04

1. Open the [v0.5.9 GitHub release](https://github.com/relentlesspoetry-coder/Matrix/releases/tag/v0.5.9) — the repository and installers are public.
2. Download `Matrix-Play_0.5.9_amd64.deb`.
3. Open a terminal in the download folder and run:

   ```bash
   sudo apt install ./Matrix-Play_0.5.9_amd64.deb
   ```

4. Open **Matrix Play** from the application menu. You can also run `matrix-frontend` in a terminal to capture diagnostics.

APT installs the GTK/WebKit runtime dependencies if needed. Rust, Node, the source archive and developer tools are not required to run the installed application.

## Portable AppImage alternative

Download `Matrix-Play_0.5.9_amd64.AppImage` from the same release. In its download folder run:

```bash
chmod +x Matrix-Play_0.5.9_amd64.AppImage
./Matrix-Play_0.5.9_amd64.AppImage
```

If FUSE is unavailable, run it in extraction mode:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Matrix-Play_0.5.9_amd64.AppImage
```

The AppImage's actual native window, IPC, scanning, local covers, settings and launcher dispatch passed the same isolated desktop test in extraction mode. Other Linux distributions and native FUSE mounting have not been validated. The DEB is recommended on Zorin/Ubuntu.

## Upgrade from the blank-window build

Close Matrix Play, then install the v0.5.9 DEB using the same APT command above. The existing game folders, downloaded artwork, configuration and browser data remain in place. Open Matrix Play normally; no special graphics or cache launch command should be needed.

If a new startup problem occurs, attach `~/.local/share/matrix/startup.log` to the issue. This contains bounded startup stages and renderer/load errors, not game data. In this incident, graphics workarounds did not resolve the problem; a fresh cache directory did, which the affected user confirmed. The specific problematic shared cache file was not identified or deleted.

## Rename compatibility

The application menu now shows **Matrix Play** with the supplied logo. APT replaces the old `matrix` application package with `matrix-play`. Existing `~/Games/Matrix/` libraries are reused without moving files. New installations use `~/Games/Matrix Play/`. Settings and cover storage continue using their established `matrix` data directories.

## First-run testing

Matrix Play creates `~/Games/Matrix Play/` and 18 console subfolders. Place your legally obtained game files inside the appropriate nested `* Games` folder, such as `PS2/PS2 Games`. Existing Steam and Windows games stay in their existing locations.

1. Open a console library, choose **Edit emulator**, and verify the executable and argument tokens. Matrix Play can detect one executable/AppImage in a console's parent folder, plus selected system-installed emulators.
2. Select **Refresh** to rescan libraries. PS1/PS2 artwork downloads run in the background. Under **Emulators & folders → Cover artwork**, choose Default or 3D, or disable automatic downloads.
3. A filename with an exact Sony serial or a supported uncompressed SYSTEM.CNF layout can identify artwork. For unknown or conflicting serials, open game details with the information button and confirm the exact serial. Compressed images may need a filename serial or confirmation. Matrix Play does not guess from similar titles.
4. Use **Choose cover** to import a local PNG/JPEG/WebP image. Local covers take priority. TheCoverProject is manual-only.
5. Clicking a game card sends a launch request immediately. Use its small information button to view details without launching.
6. Check **Steam Games** and **Windows Games** independently. Windows registration accepts only an existing `.desktop` shortcut that works outside Matrix Play. Direct `.exe` registration and launch are rejected, including saved legacy entries. Legacy entries and their artwork are preserved; register a working shortcut to replace them. External compatibility software is not supplied or configured by Matrix Play.

A game's emulator, firmware and any required keys must already be installed/configured by the user. Matrix Play does not include them. It does not move or rewrite game files.

## What was actually verified

- Optimized native Linux compilation and DEB/AppImage generation succeeded.
- A real Tauri/WebKit desktop window from the DEB payload was exercised through native WebDriver, in a separate X11 virtual display with an isolated temporary home directory.
- That test checked actual IPC, creation of all 14 managed console libraries, PS1/PS2 scans, cross-console path rejection, local cover round-trip, preservation of emulator/Windows settings, Steam and Windows tab separation, and process argument dispatch to harmless test launchers.
- The real desktop automatically downloaded one PS1 and one PS2 default cover using synthetic serial-labelled files. A separate Rust live test verified four sample provider URLs (both default and 3D).
- Twelve Rust cover-engine tests and five headless-backend tests passed. The simulated-native Chromium UI suite and folder-layout tests passed during source development. See README for current build checks.

**Additional v0.5.1 checks:** the rebuilt app reached Interface ready in the same existing-profile Wayland launch that previously stalled. Ordinary non-WebDriver X11 startup was checked by capturing the actual desktop pixels. Native tests cover both an explicitly provided and an unset parent cache directory, and ensure test launchers receive the original environment.

**Not verified across the emulator lineup:** fullscreen gameplay on every engine (one real PCSX2 launch was confirmed in v0.5.5), launching a user's Steam games or existing desktop shortcuts, controller support, ARM builds, other Linux releases, every disc format/serial, or universal cover availability. Launch fixtures validate command dispatch, not emulator compatibility. The update button checks and downloads signed GitHub releases. Privileged DEB installation has not been executed by the automated tests; AppImage replacement is tested with synthetic fixtures.

## Version 0.5.3 checks

- Six native/headless backend tests pass, including rejection of inherited/unrelated AppImage locations.
- Five updater-core tests passed: semantic version selection, signatures and repository/package identity, corruption/size limits, cancellation and cached downloads, package headers/symlinks, and staged AppImage replacement with backup using synthetic files.
- The mock-bridge Chromium test exercised check → download → install → restart controls and all 14 decoded console images. Its installer responses are simulated, not system package installation.
- The compiled desktop window passed native IPC/scanning/launch-fixture tests, rendered all 14 console images and exposed the real updater status. Ordinary non-WebDriver startup painted successfully without injected graphics settings.
- Rust cover and backend tests continue to pass. The release pipeline must also pass its Linux build and actual desktop checks before signing and publication.

## Version 0.5.6 checks

Nineteen backend tests pass, including engine identity/capability boundaries, fullscreen and no-GUI argv, current shadPS4 core versus QtLauncher syntax, per-launch configuration options, and rejection of launches from the PS5/Switch 2 library-only collections. The isolated-home test scans PS4 extracted eboot content, excludes PS4 PKG installation packages, scans Vita content archives and verifies favorites on the new collection libraries.

The native Tauri/WebKit fixture suite passed with 18 folders and decoded console images. It checked actual PCSX2 Flatpak dispatch of `-nogui -fullscreen -- <game path>` with the runtime-font workaround intact, RMG fullscreen/hidden-menu options, normal Open emulator arguments without game-only options, and library-only launch rejection. Launch paths containing spaces remain one argv token. These launchers are harmless scripts, not real emulators; this validates Matrix's dispatch, not fullscreen behavior on every emulator version.

Chromium UI checks passed using a mock native bridge, including the PS5 library-only notice, no Play/Setup action, collection favorites, Vita setup guidance and all 18 supplied images. Ordinary painted native startup passed in an isolated X11 display without graphics workaround variables. Cover-core (12) and updater-core (5) tests and release packaging tests passed. Live cover-provider checks from earlier versions were not repeated for this launch/library change.

The first cloud run (42155969970) stopped publication because the native test clicked Open emulator before the preceding asynchronous game request settled. The test now waits for that request to finish; it passed locally with the cloud software-rendering environment. Application launch code did not change for this correction.

After installing Matrix Play 0.5.6 and closing/reopening it, the affected user explicitly confirmed: **“Yes, fullscreen with no menus”** when clicking their actual PS2 game. This verifies the PCSX2 Flatpak fullscreen/no-GUI path on that machine, in addition to the fixture dispatch tests.

PS4/Vita adapters are researched against current upstream source but have not been tested with real PS4/Vita games. PS5 and Switch 2 have no Matrix launch adapter. Emulator errors, first-run requirements and overlays can still appear; unknown custom launchers need their own fullscreen options.

## Published v0.5.5 verification — 2026-11-05

The automatic [Linux build and signed release](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42152895832) passed. Downloaded DEB and AppImage hashes matched the manifest verified with the pinned signing key. Installed Matrix Play 0.5.5 was verified, and the affected user explicitly confirmed that clicking their PS2 game now launches it. Other emulator/game combinations remain unverified.

## Version 0.5.5 development checks

Fifteen headless/native backend tests cover exact-ID matching, shared console capabilities, ambiguous installations, valid/executable AppImage matching, canonical deduplication, preservation of saved choices, and avoiding engine-specific arguments based only on a display label. They also cover PCSX2 setup messages and bounded output capture.

The updated Chromium suite uses a mock bridge to verify detected choices, runtime-font selection, Open emulator, Save and open, escaped launch output and visible failure notices. Native fixtures exercise real discovery/IPC/launch dispatch, a failing Flatpak launcher with captured stderr, scoped runtime-font arguments, setup preflight and persistence. The fixture launchers are harmless scripts; they do not certify all real emulators.

Locally, the actual installed PCSX2 Flatpak passed its configuration check but crashed in fontconfig while opening the first-run wizard. Display/style/cache trials alone did not resolve it. A temporary abort diagnostic identified the crashing library; that diagnostic is outside the product/source repository and is not shipped. A minimal runtime-font configuration allowed the real setup window to remain open, and subsequent PCSX2 logs showed completed BIOS setup and a real game boot/run. This verifies the targeted workaround on the affected machine; it is not universal emulator/game compatibility testing.

## Version 0.5.4 development checks

Nine native/headless backend tests pass, including backward-compatible executable profiles, AppImage header validation and explicit execute-permission consent, installed-Flatpak parsing/scope, and favorites migration/deduplication/console boundaries. Mock-bridge Chromium tests exercise AppImage Browse dispatch, Flatpak selection/save, favorite add/remove, counts and empty states.

The compiled desktop test passed favorites persistence across a webview reload and a complete frontend process restart. It selected a Flatpak through the real setup form, persisted its installation scope and launched it with expected discrete arguments and console-specific read access. That launch uses a harmless fixture `flatpak` executable; it does not test real emulator gameplay. Ordinary non-WebDriver painted-window startup also passed without forced graphics settings. The native picker uses Tauri's dialog plugin; the browser test simulates its selected-path response.

## Published v0.5.4 verification — 2026-11-05

The automatic [Linux build and signed release](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42147273955) passed. The published DEB and AppImage matched the pinned-key signed manifest and all downloaded release checksums passed.

A real GTK emulator picker was opened in an isolated X11 display and cancelled; it returned no selected path. The published AppImage passed the native favorites/Flatpak fixture suite, including favorites persistence across a full process restart and correct AppImage self-update identity. Selecting a file in the UI test uses a simulated returned path; AppImage permission/header handling is tested against synthetic files. Actual emulator gameplay remains unverified.

The installed 0.5.3 application's real Update controls downloaded, verified and installed signed 0.5.4 through the system package installer. Its Restart control launched installed 0.5.4 and reached Interface ready. This was an intentional local upgrade with an isolated test home/display, not a privileged step in the ordinary test harness or GitHub workflow. A complete AppImage replacement/restart cycle remains unverified; its replacement/backup logic uses synthetic fixtures.

## Published release and installed-app verification — 2026-11-05

The public GitHub Actions run [42143081462](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42143081462) passed both build/test and signing/publication jobs for v0.5.3. Its six published assets include DEB, AppImage, source ZIP, checksums and the signed update manifest. Downloaded asset hashes were checked against SHA256SUMS.

The real updater engine verified the public signed manifest and complete AppImage download. An updater-capable temporary 0.5.2 test build used its actual desktop Update controls to download and verify the newer public 0.5.3 DEB, without running installation from the test harness. The published AppImage passed the native window/IPC/launch-fixture test in extraction mode and identified its own mounted/extracted location for self-update. A complete in-app AppImage install/restart cycle has not been exercised.

The affected user's normal launcher was still using installed `matrix` 0.5.0; the previously tested cache fix had not replaced it. The signed 0.5.3 DEB was verified and installed through the system package installer, replacing only `matrix` with `matrix-play`. Normal startup in the existing user's Wayland session reached Interface ready with no XDG_CACHE_HOME or graphics workaround supplied. The user then closed/reopened **Matrix Play from the application menu** and confirmed: **“Yes, it opens normally.”** The normal-launch problem is resolved on that tested installation. Other machines and real emulator gameplay remain separate testing work.

## Report a problem

Create a GitHub issue with your distribution/version, package version, console, file format, what you clicked, the result and relevant terminal output. Include the serial and artwork style for cover problems. You do not need to upload a game or firmware file.

Configuration: `$XDG_CONFIG_HOME/matrix/frontend.json` (normally `~/.config/matrix/frontend.json`).
Artwork: `$XDG_DATA_HOME/matrix/covers/` (normally `~/.local/share/matrix/covers/`).

To uninstall the application: `sudo apt remove matrix-play`. Your game library, artwork and user configuration are retained.

## Launch regression coverage

`node --test tests/test_launches.cjs` runs six deterministic production-UI checks
for delayed external errors, details escaping, repeat-click suppression, stale
poll cancellation, monitoring beyond startup and spawn rejection. These are
mock IPC tests, not native desktop or game execution.

The headless backend suite now has 37 tests, including native/Flatpak Steam
provenance for external libraries and aliases, both PS3 title layouts, canonical
Flatpak save-write boundaries, and delayed failures from synthetic gio
executables. `tests/test_native.py` also covers these dispatch/UI paths with
harmless launch fixtures when run in an environment with GTK/WebKit and an X11
display. Real emulator save creation and real Steam/desktop-shortcut games are separate manual
checks; passing fixture tests does not certify those integrations.
