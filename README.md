<p align="center"><img src="web/assets/matrix-play-logo.png" alt="Matrix-Play" width="320"></p>

# Matrix-Play

A Linux console-game library and emulator frontend.

[Download](https://github.com/relentlesspoetry-coder/Matrix-Play/releases) · [Setup guide](EMULATOR-LAUNCHES.md) · [Report a problem](https://github.com/relentlesspoetry-coder/Matrix-Play/issues)

## Install

Download the **DEB** or **AppImage** from [Releases](https://github.com/relentlesspoetry-coder/Matrix-Play/releases). Linux x86-64; tested on Zorin OS 18.1 and Ubuntu 24.04.

For Zorin or Ubuntu, install the DEB from your download folder:

```bash
sudo apt install ./Matrix-Play_0.6.14_amd64.deb
```

For the portable AppImage:

```bash
chmod +x Matrix-Play_0.6.14_amd64.AppImage
./Matrix-Play_0.6.14_amd64.AppImage
```

For future releases, open **Settings → Check for updates**.

## Add emulators

First launch opens emulator setup. Choose the console and a downloaded Linux AppImage or standalone executable, then select **Move file and set up**. Matrix Play moves it into that console's managed folder and configures its launcher. **Add emulator** remains available on the console menu and in Settings.

Use **Link installed emulator** for Flatpaks, system-installed programs or emulator distributions that need adjacent resources; those remain in place. Firmware, BIOS, keys and game compatibility are configured in the emulator itself. Existing files are never overwritten by import.

After setup, consoles without a working emulator are hidden. Configured consoles remain visible without games, and their dot turns green once games are present. To show all libraries, disable **Settings → Console visibility & order → Hide consoles without a working emulator**. Newly added emulators and games in managed folders are detected every 30 seconds while the interface is idle; Settings provides an opt-out.

## Console covers

Every console folder contains its game folder and a `Covers` folder. Existing installations receive missing folders automatically at startup. For example:

- `~/Games/Matrix/PS2/PS2 Games/`
- `~/Games/Matrix/PS2/Covers/`

New installations use `~/Games/Matrix Play/`; existing `~/Games/Matrix/` libraries stay in place.

Place manual covers in the console's `Covers` folder with the same filename stem as the game: `Example Game.iso` uses `Example Game.png`, `.jpg`, `.jpeg` or `.webp`. Matching is case sensitive for the game name. This also works for games in registered external folders. Games sharing the same filename stem in one console share that manually named cover; use the in-app cover importer to assign separate covers.

In-app imports and automatic downloads save their images directly in that console's `Covers` folder using managed filenames to preserve game identity and cover styles. Existing imported covers, sidecar images and downloaded caches remain readable; reusing an old download copies it to the new folder without deleting the original. Manual artwork takes priority over downloads.

## Features

- 24 separate console libraries.
- Existing game folders and external drives, without moving your files.
- Native, AppImage and Flatpak emulator setup, with automatic detection for supported emulators.
- Cover art, favorites, search, grid/list views and controller navigation.
- Fullscreen game launches for supported emulators.

## Console libraries

| Nintendo | PlayStation | Sega | Xbox |
| --- | --- | --- | --- |
| NES, Super Nintendo, Switch, Switch 2, N64, GameCube, Wii, Wii U, Game Boy, GBA, DS, 3DS | PS1, PS2, PS3, PS4, PS5, PSP, PS Vita | Genesis / Mega Drive, Saturn, Dreamcast | Xbox, Xbox 360 |

PS5, Switch 2 and Xbox 360 use manually configured emulators. See the [setup guide](EMULATOR-LAUNCHES.md) for formats and launch options.

## Get started

1. Open a console library, then **··· → Manage folders** to add your games.
2. Choose **Set up emulator**, select your installed emulator and save.
3. Click **Refresh**, then select a game to launch it.

Emulators, games, firmware and keys are supplied separately.

## Documentation

- [Emulators and game formats](EMULATOR-LAUNCHES.md)
- [Cover artwork](COVER-PROVIDERS.md)
- [Building from source](BUILDING.md)
- [Updates](UPDATES.md) · [Testing](TESTING.md) · [Changelog](CHANGELOG.md)

## License

[MIT](LICENSE). Third-party artwork and trademarks belong to their respective owners.

## SteamGridDB covers

In Settings → Artwork → SteamGridDB, use **Get API key** to open your SteamGridDB preferences, then paste your key and choose **Save and enable**. This enables missing-cover lookups for every console using unique exact game-title matches. Title lookups send titles to SteamGridDB; full paths and game files stay local. Your key is saved in the private local settings file and is not returned to the web UI.

For ambiguous titles or a different cover, open game details → **Find on SteamGridDB**, select the correct game, and choose a cover. Selection explicitly replaces the current cover. Images are validated and saved in the console’s `Covers` folder. Existing local covers take priority over automatic downloads. Disable SteamGridDB to use the existing serial/Libretro providers again. SteamGridDB uses static portrait covers and does not supply the existing 3D-cover style.

## Library tools

- **Recently played** on the console menu shows the last eight successful launch requests across console libraries; the latest 20 are saved. This is launch history, not measured playtime.
- Open a console game's details → **Launch settings** to override its emulator arguments. Enter one literal argument per line, with `{game}` for the game path. Leave blank to restore console defaults.
- Settings → **Backup and restore** exports settings, favorites, recent history, launch overrides and managed console cover files. API keys, game files, external sidecar art and old online caches are excluded. Paths remain unchanged; backups are intended for the same library layout. Restore keeps existing covers, fills in missing files and saves `before-restore.json` beside the settings first. Only restore trusted backups: emulator profiles contain executable paths and arguments. A backup is limited to 64 MB, with at most 32 MB of source covers (4 MB per image).
- Settings → Artwork → **Cover download queue** shows results and errors. Pause stops scheduling further jobs; an in-flight request may finish. Resume retries pending/interrupted/failed jobs and skips completed jobs. Provider rate limits still apply.

## Library organization and save backups

Settings → **Console visibility & order** controls hidden consoles, pins, order and the ready-only filter. Hidden consoles remain registered and can be shown again there. **Library behavior** enables optional version grouping and automatic refresh (every 30 seconds while visible, outside dialogs). Registered console drives that disconnect retain indexed entries marked Offline; reconnecting reconciles the index.

Game details → **Title & versions** edits the display name, separate SteamGridDB/Libretro cover-search title, and version group. Empty titles use the original filename-derived identity. Automatic grouping recognizes base/update and common region/revision suffixes; other editions can share an explicit group name. Use `-` to keep a title separate. Enable grouping in Settings, then select a version in game details; files, per-version favorites and artwork stay separate. Each console remembers its own view, sorting and last selection.

Console menu **··· → Back up saves / Restore saves** supports configured PCSX2, PPSSPP and Dolphin profiles. Close the emulator first, including instances started outside Matrix Play. Choose its actual `memcards`, `sstates`, `GC`, `StateSaves`, `SAVEDATA` or `PPSSPP_STATE` folder. This backs up the selected folder’s regular files, not game ROMs. Full Wii NAND backups, emulator cloud saves and other emulator profiles are not supported yet.

Save backups are separate JSON files, limited to 128 MB of source files, 32 MB per file and 10,000 files. Symlinks and special files are rejected. Restore accepts a backup for the same console, lists every file and whether it replaces an existing file, then requires **Restore these files**. Files outside the list are kept. A private, restorable `before-restore-*.json` backup is saved under the Matrix configuration folder’s `save-backups` directory before writing; failed writes trigger rollback. Restore only trusted backups and choose the intended save folder.

## Themes

Open **Settings → Themes** to choose **Arctic Current** (the original layout) or **White Frost Current** (the approved open gallery with a compact header, six-column desktop grid and bottom selection bar). Selection is saved in native settings, included in settings backups, and applied across libraries and dialogs. Hover or focus a console to update the bottom strip. Smaller windows use fewer columns. System reduced-motion preferences stop the animated currents.

Steam and Windows game integrations are removed. Existing installations, shortcuts and saved legacy registration data are left untouched. SteamGridDB remains available for console artwork.
