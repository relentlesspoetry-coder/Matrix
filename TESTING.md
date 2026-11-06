# Test Matrix Play on Linux

This is the native testing release, version 0.5.2. The build was made on Zorin OS 18.1 (Ubuntu 24.04 base), for **x86-64 / amd64** computers. Other distributions and ARM systems have not been validated.

## Install the DEB — recommended for Zorin 18 / Ubuntu 24.04

1. Open the [v0.5.2 GitHub release](https://github.com/relentlesspoetry-coder/Matrix/releases/tag/v0.5.2) while signed in to the account that owns this private repository.
2. Download `Matrix-Play_0.5.2_amd64.deb`.
3. Open a terminal in the download folder and run:

   ```bash
   sudo apt install ./Matrix-Play_0.5.2_amd64.deb
   ```

4. Open **Matrix Play** from the application menu. You can also run `matrix-frontend` in a terminal to capture diagnostics.

APT installs the GTK/WebKit runtime dependencies if needed. Rust, Node, the source archive and developer tools are not required to run the installed application.

## Portable AppImage alternative

Download `Matrix-Play_0.5.2_amd64.AppImage` from the same release. In its download folder run:

```bash
chmod +x Matrix-Play_0.5.2_amd64.AppImage
./Matrix-Play_0.5.2_amd64.AppImage
```

If FUSE is unavailable, run it in extraction mode:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Matrix-Play_0.5.2_amd64.AppImage
```

The AppImage's actual native window, IPC, scanning, local covers, settings and launcher dispatch passed the same isolated desktop test in extraction mode. Other Linux distributions and native FUSE mounting have not been validated. The DEB is recommended on Zorin/Ubuntu.

## Upgrade from the blank-window build

Close Matrix Play, then install the v0.5.2 DEB using the same APT command above. The existing game folders, downloaded artwork, configuration and browser data remain in place. Open Matrix Play normally; no special graphics or cache launch command should be needed.

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

**Not verified:** real emulator gameplay, launching a user's Steam games or real Wine installations, controller support, ARM builds, other Linux releases, every disc format/serial, or universal cover availability. Launch fixtures validate command dispatch, not emulator compatibility. There is no automatic application updater in this release.

## Report a problem

Create a GitHub issue with your distribution/version, package version, console, file format, what you clicked, the result and relevant terminal output. Include the serial and artwork style for cover problems. You do not need to upload a game or firmware file.

Configuration: `$XDG_CONFIG_HOME/matrix/frontend.json` (normally `~/.config/matrix/frontend.json`).
Artwork: `$XDG_DATA_HOME/matrix/covers/` (normally `~/.local/share/matrix/covers/`).

To uninstall the application: `sudo apt remove matrix-play`. Your game library, artwork and user configuration are retained.
