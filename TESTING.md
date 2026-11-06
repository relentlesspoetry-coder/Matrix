# Test Matrix Play on Linux

This is the native testing release, version 0.5.5. The build was made on Zorin OS 18.1 (Ubuntu 24.04 base), for **x86-64 / amd64** computers. Other distributions and ARM systems have not been validated.

## Install the DEB — recommended for Zorin 18 / Ubuntu 24.04

1. Open the [v0.5.5 GitHub release](https://github.com/relentlesspoetry-coder/Matrix/releases/tag/v0.5.5) — the repository and installers are public.
2. Download `Matrix-Play_0.5.5_amd64.deb`.
3. Open a terminal in the download folder and run:

   ```bash
   sudo apt install ./Matrix-Play_0.5.5_amd64.deb
   ```

4. Open **Matrix Play** from the application menu. You can also run `matrix-frontend` in a terminal to capture diagnostics.

APT installs the GTK/WebKit runtime dependencies if needed. Rust, Node, the source archive and developer tools are not required to run the installed application.

## Portable AppImage alternative

Download `Matrix-Play_0.5.5_amd64.AppImage` from the same release. In its download folder run:

```bash
chmod +x Matrix-Play_0.5.5_amd64.AppImage
./Matrix-Play_0.5.5_amd64.AppImage
```

If FUSE is unavailable, run it in extraction mode:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Matrix-Play_0.5.5_amd64.AppImage
```

The AppImage's actual native window, IPC, scanning, local covers, settings and launcher dispatch passed the same isolated desktop test in extraction mode. Other Linux distributions and native FUSE mounting have not been validated. The DEB is recommended on Zorin/Ubuntu.

## Upgrade from the blank-window build

Close Matrix Play, then install the v0.5.5 DEB using the same APT command above. The existing game folders, downloaded artwork, configuration and browser data remain in place. Open Matrix Play normally; no special graphics or cache launch command should be needed.

If a new startup problem occurs, attach `~/.local/share/matrix/startup.log` to the issue. This contains bounded startup stages and renderer/load errors, not game data. In this incident, graphics workarounds did not resolve the problem; a fresh cache directory did, which the affected user confirmed. The specific problematic shared cache file was not identified or deleted.

## Rename compatibility

The application menu now shows **Matrix Play** with the supplied logo. APT replaces the old `matrix` application package with `matrix-play`. Existing `~/Games/Matrix/` libraries are reused without moving files. New installations use `~/Games/Matrix Play/`. Settings and cover storage continue using their established `matrix` data directories.

## First-run testing

Matrix Play creates `~/Games/Matrix Play/` and 14 console subfolders. Place your legally obtained game files inside the appropriate nested `* Games` folder, such as `PS2/PS2 Games`. Existing Steam and Windows games stay in their existing locations.

1. Open a console library, choose **Edit emulator**, and verify the executable and argument tokens. Matrix Play can detect one executable/AppImage in a console's parent folder, plus selected system-installed emulators.
2. Select **Refresh** to rescan libraries. PS1/PS2 artwork downloads run in the background. Under **Emulators & folders → Cover artwork**, choose Default or 3D, or disable automatic downloads.
3. A filename with an exact Sony serial or a supported uncompressed SYSTEM.CNF layout can identify artwork. For unknown or conflicting serials, open game details with the information button and confirm the exact serial. Compressed images may need a filename serial or confirmation. Matrix Play does not guess from similar titles.
4. Use **Choose cover** to import a local PNG/JPEG/WebP image. Local covers take priority. TheCoverProject is manual-only.
5. Clicking a game card sends a launch request immediately. Use its small information button to view details without launching.
6. Check **Steam Games** and **Windows Games** independently. Windows registration references an existing `.exe` using system Wine, or a `.desktop` launcher. Custom prefix/Proton setup is not automatically imported.

A game's emulator, firmware and any required keys must already be installed/configured by the user. Matrix Play does not include them. It does not move or rewrite game files.

## What was actually verified

- Optimized native Linux compilation and DEB/AppImage generation succeeded.
- A real Tauri/WebKit desktop window from the DEB payload was exercised through native WebDriver, in a separate X11 virtual display with an isolated temporary home directory.
- That test checked actual IPC, creation of all 14 managed console libraries, PS1/PS2 scans, cross-console path rejection, local cover round-trip, preservation of emulator/Windows settings, Steam and Windows tab separation, and process argument dispatch to harmless test launchers.
- The real desktop automatically downloaded one PS1 and one PS2 default cover using synthetic serial-labelled files. A separate Rust live test verified four sample provider URLs (both default and 3D).
- Twelve Rust cover-engine tests and five headless-backend tests passed. The simulated-native Chromium UI suite and folder-layout tests passed during source development. See README for current build checks.

**Additional v0.5.1 checks:** the rebuilt app reached Interface ready in the same existing-profile Wayland launch that previously stalled. Ordinary non-WebDriver X11 startup was checked by capturing the actual desktop pixels. Native tests cover both an explicitly provided and an unset parent cache directory, and ensure test launchers receive the original environment.

**Not verified:** real emulator gameplay, launching a user's Steam games or real Wine installations, controller support, ARM builds, other Linux releases, every disc format/serial, or universal cover availability. Launch fixtures validate command dispatch, not emulator compatibility. The update button checks and downloads signed GitHub releases. Privileged DEB installation has not been executed by the automated tests; AppImage replacement is tested with synthetic fixtures.

## Version 0.5.3 checks

- Six native/headless backend tests pass, including rejection of inherited/unrelated AppImage locations.
- Five updater-core tests passed: semantic version selection, signatures and repository/package identity, corruption/size limits, cancellation and cached downloads, package headers/symlinks, and staged AppImage replacement with backup using synthetic files.
- The mock-bridge Chromium test exercised check → download → install → restart controls and all 14 decoded console images. Its installer responses are simulated, not system package installation.
- The compiled desktop window passed native IPC/scanning/launch-fixture tests, rendered all 14 console images and exposed the real updater status. Ordinary non-WebDriver startup painted successfully without injected graphics settings.
- Rust cover and backend tests continue to pass. The release pipeline must also pass its Linux build and actual desktop checks before signing and publication.

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
