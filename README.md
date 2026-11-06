<p align="center"><img src="web/assets/matrix-play-logo.png" alt="Matrix Play" width="420"></p>

# Matrix Play

A native Linux game-library and emulator frontend. Browse games by console, connect each library to your installed emulator, and launch games from one application. Steam games and existing Windows/Wine launchers have their own tabs.

Matrix Play organizes and launches your existing setup. Emulators, game files, firmware, keys and Wine are supplied and configured separately.

Built with Rust and Tauri 2, Matrix Play keeps **14 console libraries**, **Steam Games**, and **Windows Games** separate. It is a library frontend, not an emulator or game store. Actual game compatibility depends on your installed emulator or Windows runner.

## Matrix Play branding

Matrix Play adopts the supplied **Matrix Play** name and artwork. The full original PNG is used in the sidebar; a crop of its central symbol is used for the compact sidebar, window and application launcher icons.

Fresh installs create `~/Games/Matrix Play/`. If `~/Games/Matrix/` already exists, it is reused in place so game paths, cover identities and emulator profiles keep working. Configuration, artwork, the startup-cache fix, application identifier and command-line executable `matrix-frontend` remain compatible.

The Linux package is now `matrix-play`. Its package metadata replaces/conflicts with the old `matrix` package so APT can upgrade the application without colliding over its executable or launcher files. User configuration and games are retained. The GitHub repository URL remains unchanged.

## Updates and automatic releases

Use **Update** in the top bar to check public GitHub releases, read the release notes and download the newest Linux x86-64 package. Downloads persist under `$XDG_DATA_HOME/matrix/updates/`; the app checks an Ed25519 signature pinned in the application, package size and SHA-256 before installation. GitHub sign-in is not required.

For a DEB installation, **Install update** opens Linux's system authorization dialog and runs the system package installer. For an AppImage, it replaces the running AppImage in its writable folder and keeps a `.previous` backup. **Restart Matrix Play** finishes either update. A development build opens the verified DEB with your system installer. If an update cannot complete, use **GitHub releases** in the dialog to download an installer manually. Earlier versions need one manual upgrade to obtain this button.

Maintainers: commit the changes and the next version's CHANGELOG notes, then run `python3 scripts/release.py X.Y.Z`. It updates the version, commits and pushes to main. GitHub Actions builds both packages and runs the Rust and real desktop tests. Only a successful build publishes a signed testing release; an existing published version is never overwritten. The encrypted `MATRIX_UPDATE_SIGNING_KEY` repository secret is required. See [UPDATES.md](UPDATES.md).

## v0.5.1 startup fix

A cache-dependent stall could leave v0.5.0's window blank before the first page loaded. Giving only the process a fresh cache directory reproduced a successful startup with the original data/configuration, and the affected user confirmed the workaround. The exact offending shared cache file was not identified.

Matrix Play now isolates its runtime cache under `$XDG_CACHE_HOME/matrix/runtime-v1/` (normally `~/.cache/matrix/runtime-v1/`) before creating WebKit. It does not delete shared caches, migrate game files, or replace the browser data/profile. Launched emulators, Steam, Flatpak, Wine, desktop launchers and browser/folder openers receive the original cache environment.

Startup stages and WebKit failures are logged to `$XDG_DATA_HOME/matrix/startup.log` (normally `~/.local/share/matrix/startup.log`); each launch resets that bounded diagnostic file. A visible error/retry panel handles interface script failures. This cannot replace native diagnostics when the entire web process cannot load a page.

Regression checks now include an actual painted, non-WebDriver window, normal launch from outside the project directory, Wayland without injected graphics workarounds, and restoration of launched programs' cache environments. The native driver lookup failure in the initial GitHub workflow is also corrected.

## Install the Linux test release

Download **Matrix-Play_0.5.3_amd64.deb** from the [v0.5.3 release](https://github.com/relentlesspoetry-coder/Matrix/releases/tag/v0.5.3), then run `sudo apt install ./Matrix-Play_0.5.3_amd64.deb` in its download folder. Open **Matrix Play** from the application menu. Tested on Zorin OS 18.1 / Ubuntu 24.04 base, x86-64. See [TESTING.md](TESTING.md) for setup, verified behavior and remaining limits.

![Real compiled Matrix Play window in an isolated test display](screenshots/matrix-linux-home.png)

## What works in the current source

- First-run directory creation for **14 systems**, under `$HOME/Games/Matrix Play/`, plus per-console folders named `* Games`.
- Independent game scanning for each console using recognized disc/ROM extensions and selected extracted layouts.
- Select an emulator executable/AppImage stored in its console directory; when unambiguous Matrix Play detects it. It can also detect several conventional emulator binaries already installed on `PATH`. Flatpak emulator discovery is not implemented yet.
- **Single-click launching:** in a native Matrix Play build, clicking a game card immediately invokes its assigned emulator, `steam -applaunch` or a registered Wine/desktop launcher. The small information button opens details without launching.
- Steam game discovery using `appmanifest_*.acf` from standard Steam and Flatpak locations, plus Steam library folders declared in `libraryfolders.vdf`.
- Windows game registration by referencing existing `.exe` (system Wine) and `.desktop` files; no game relocation.
- **Local cover art:** searches for a game-name-matched `.png`, `.jpg` or `.webp` beside the ROM (or `cover.*` in a game folder), plus Steam's local library art cache. Supports manually chosen covers saved under `$XDG_DATA_HOME/matrix/covers/` (normally `~/.local/share/matrix/covers/`), with safe per-game hash-based filenames. Artwork is loaded only for on-screen cards.
- Offline favorites and console-specific search, sorting and grid/list views.
- Safe process creation using discrete arguments, not shell-interpolated commands. Emulator configuration never automatically rewrites game files.

## Automatic PS1 / PS2 cover art

Automatic missing-art scans now run after native library scans and Refresh. **Emulators & folders → Cover artwork** controls automatic downloads and default/3D style. The requested xlenore templates are used directly, with serial-based identity; titles are never fuzzily matched. Details show the result and identity source, and allow an explicit serial confirmation. Choose cover imports an image for any library.

Serial detection supports filenames and bounded ISO9660 SYSTEM.CNF parsing, including simple one-file, zero-origin raw BIN/CUE data tracks. Compressed images need a filename serial or user confirmation. Unsupported/ambiguous layouts stay unresolved. Local/manual covers take priority. Downloaded art, provenance, positive and negative caches persist in the XDG data directory. Provider requests are serialized, spaced, bounded, and back off on errors. The browser preview never downloads covers.

**TheCoverProject is manual only:** an explicit button opens its site, and Choose cover imports your downloaded image. Current automation authorization, API rules and copyright licensing could not be established. See [COVER-PROVIDERS.md](COVER-PROVIDERS.md) for dated evidence, exact templates, limits and policy decisions.

### Actual validation performed

Verified in source development (CI results are visible in the Actions tab):

- `cargo test --locked --manifest-path update-core/Cargo.toml`: **5 passed**, covering signed manifest identity, downgrade rejection, damaged/oversized downloads, cancellation/cache, and synthetic AppImage replacement with backup. These tests do not execute a privileged DEB upgrade.

- `cargo test --manifest-path cover-core/Cargo.toml`: **12 passed**, one live test skipped by default. These execute the actual Rust cover engine with synthetic, non-proprietary disc fixtures and an injected transport for deterministic HTTP outcomes.
- `cargo test --manifest-path cover-core/Cargo.toml live_provider_templates -- --ignored --nocapture`: **1 passed**. The real HTTPS transport retrieved and decoded default/3D covers for one PS1 and one PS2 serial; all four sample URLs worked. Temporary images were removed.
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --lib`: **6 passed**. This compiles and executes the same native backend functions with desktop-specific Tauri wiring excluded. The isolated-home lifecycle test verifies library separation, path validation, persisted cover settings without profile loss, loading the real online cache, local cover priority and invalid-image rejection.
- `python3 tests/test_structure.py`: all 14 console folder pairs and repeated creation preserve existing files.
- `python3 tests/test_ui.py`: Chromium integration test passed with a **mock Tauri bridge**. It covers the 14 consoles, independent Steam/Windows tabs, launch dispatch, cover controls/status, explicit serial submission, manual save, TheCoverProject button, style/disable persistence requests, responsive layout and no page script errors.
- `node --check web/app.js` and Rust formatting/parser checks passed.

**Native testing release:** full desktop `cargo check` and an optimized native build succeeded. DEB and AppImage packages were generated. The DEB was extracted and tested through its actual Tauri/WebKit window in an isolated temporary home/X11 display. `tests/test_native.py --online` passed, covering actual IPC, folders/scanning, persistent settings, local/downloaded artwork, console boundaries and launch argument dispatch to harmless test executables. Actual automatic PS1 and PS2 downloads worked in that window. The AppImage also passed the real desktop test in extraction mode, and native binary unit tests passed. Real emulator, Steam and Wine execution with user games and other Linux platforms remain unverified; see [TESTING.md](TESTING.md).

To rerun UI tests, install Python Playwright and Chromium (`python3 -m pip install playwright`, `python3 -m playwright install chromium`). Set `MATRIX_CHROMIUM=/path/to/chrome` if using an existing Chromium binary. Run tests from the extracted folder; no proprietary ROMs are required. Standard Rust tests are offline after dependencies have been fetched; the explicit ignored smoke test requires internet access.

## Created folder structure

```text
~/Games/Matrix Play/
├── Switch/
│   └── Switch Games/
├── PS1/
│   └── PS1 Games/
├── PS2/
│   └── PS2 Games/
├── PS3/
│   └── PS3 Games/
├── PSP/
│   └── PSP Games/
├── N64/
│   └── N64 Games/
├── GameCube/
│   └── GameCube Games/
├── Dreamcast/
│   └── Dreamcast Games/
├── Wii/
│   └── Wii Games/
├── Wii U/
│   └── Wii U Games/
├── Game Boy/
│   └── Game Boy Games/
├── GBA/
│   └── GBA Games/
├── Nintendo DS/
│   └── Nintendo DS Games/
└── Nintendo 3DS/
    └── Nintendo 3DS Games/
```

Each nested `* Games` folder belongs **inside** its console's parent folder. For example:

```text
~/Games/Matrix Play/PS1/
├── DuckStation.AppImage        # emulator, if using a local executable
└── PS1 Games/
    ├── My Game.cue             # disc descriptor
    ├── My Game.bin             # track data
    └── My Game.jpg             # optional locally supplied cover art
```

The native app creates these folders on first run using `bootstrap()`. Users can create them early with `./scripts/setup-folders.sh`, which only creates missing directories. It never downloads emulators, games, firmware or proprietary keys.

Steam and Windows games are **not** copied into `~/Games/Matrix Play`. They stay at their current install locations.

## Build Matrix Play on Zorin OS 18 or Ubuntu 24.04

Install Tauri's documented Linux dependencies:

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libdbus-1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev pkg-config
```

Install Rust from <https://rustup.rs> and Node.js 20 or later. Then install the pinned Tauri CLI from this repository:

```bash
npm ci
```

Extract this archive. From the extracted source folder:

```bash
npm run tauri -- dev             # native development window
./scripts/build-native.sh        # unit tests followed by .deb and AppImage packaging
```

The build script checks Cargo, WebKitGTK 4.1 and the local npm Tauri CLI (or a Cargo-installed Tauri CLI) before building. When successful, outputs are in `src-tauri/target/release/bundle/`.

UI-only preview: open `web/index.html` in Chromium, Firefox or another browser. It uses sample games and **cannot** launch emulators or access installed game libraries. No npm install is needed.

## Limitations / next implementation work

1. Real emulator/game testing on Zorin, installed-package use on user machines, and Wayland/native-display behavior. Compilation, DEB generation and the actual Tauri window have been tested in a virtual X11 display.
2. Real emulator-specific adapters for unusual command-line syntax, PS3 installed games, Switch installed content, Wii U extracted directories, multidisc grouping and Flatpak emulators. Emulator support differs between versions; the generic launch template is not guaranteed to work for every executable.
3. Additional authorized artwork/metadata providers, nonstandard Sony serial formats and compressed-disc serial extraction. These are not implemented. PS1/PS2 serial-based xlenore downloads and manual cover imports are implemented; TheCoverProject automation is not.
4. Windows runner/prefix profiles for existing non-Steam Proton/Wine games. `.exe` uses system Wine and a working directory, but custom Wine prefixes / launchers must currently be configured through a `.desktop` launcher; registry import from other tools is pending.
5. Full gamepad navigation, controller setup, fullscreen experience, launch/exit monitoring and UI refinement.
6. Broader emulator-specific and distribution testing. The existing native harness uses non-proprietary fixtures and harmless test launchers; it does not validate actual gameplay.

Matrix Play is not an emulator. Do not distribute copyrighted ROMs, BIOS, encryption keys or proprietary system firmware with it.

## Native desktop test and GitHub builds

The [Linux workflow](.github/workflows/linux.yml) builds DEB/AppImage artifacts on Ubuntu 24.04 for pushes to main, pull requests and manual runs. It runs Rust, folder and native-window checks. The first published installers were also built locally on Zorin 18.1. GitHub workflow status is independent of the local checks.

To run the real desktop test after building, install `webkit2gtk-driver xvfb xauth`, install `tauri-driver` with `cargo install tauri-driver --version 2.1.0 --locked`, then run:

```bash
xvfb-run -a python3 tests/test_native.py src-tauri/target/release/matrix-frontend
# Add --online to exercise real PS1/PS2 cover downloads.
```

This test creates an isolated temporary library and harmless launcher fixtures; it does not test actual gameplay or use your installed libraries. The browser test's `matrix-native-*` screenshots show a simulated native bridge; `matrix-linux-home.png` shows the compiled desktop window.

The cloud runner has no physical GPU: its Xvfb checks explicitly use software graphics and allow up to 15 seconds for the window to paint. Those CI settings are confined to the test steps. The application does not force those graphics flags on user machines. Ordinary local X11/Wayland startup checks are reported separately from cloud software-rendering checks. Failed paint checks retain a screenshot artifact for diagnosis.

The native window/asset-loading check passed with an existing legacy library, the ordinary painted-window check passed with the new default library root, and the v0.5.2 branding migration's APT simulation selected only replacement of `matrix` by `matrix-play`. These are migration/build checks; actual installation still requires the user's APT command. The supplied full logo's bytes were verified unchanged.
