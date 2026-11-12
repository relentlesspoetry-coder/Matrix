<p align="center"><img src="web/assets/matrix-play-logo.png" alt="Matrix-Play" width="320"></p>

# Matrix-Play

A Linux game-library frontend for your console games, Steam library and Windows Games.

[Download](https://github.com/relentlesspoetry-coder/Matrix-Play/releases) · [Setup guide](EMULATOR-LAUNCHES.md) · [Report a problem](https://github.com/relentlesspoetry-coder/Matrix-Play/issues)

## Install

Download the **DEB** or **AppImage** from [Releases](https://github.com/relentlesspoetry-coder/Matrix-Play/releases). Linux x86-64; tested on Zorin OS 18.1 and Ubuntu 24.04.

For Zorin or Ubuntu, install the DEB from your download folder:

```bash
sudo apt install ./Matrix-Play_0.6.5_amd64.deb
```

For the portable AppImage:

```bash
chmod +x Matrix-Play_0.6.5_amd64.AppImage
./Matrix-Play_0.6.5_amd64.AppImage
```

For future releases, open **Settings → Check for updates**.

## Console covers

Every console folder contains its game folder and a `Covers` folder. Existing installations receive missing folders automatically at startup. For example:

- `~/Games/Matrix/PS2/PS2 Games/`
- `~/Games/Matrix/PS2/Covers/`

New installations use `~/Games/Matrix Play/`; existing `~/Games/Matrix/` libraries stay in place.

Place manual covers in the console's `Covers` folder with the same filename stem as the game: `Example Game.iso` uses `Example Game.png`, `.jpg`, `.jpeg` or `.webp`. Matching is case sensitive for the game name. This also works for games in registered external folders. Games sharing the same filename stem in one console share that manually named cover; use the in-app cover importer to assign separate covers.

In-app imports and automatic downloads save their images directly in that console's `Covers` folder using managed filenames to preserve game identity and cover styles. Existing imported covers, sidecar images and downloaded caches remain readable; reusing an old download copies it to the new folder without deleting the original. Manual artwork takes priority over downloads.

## Features

- 24 console libraries, plus separate Steam and Windows collections.
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

Steam detects existing installations. The Windows collection uses working `.desktop` shortcuts.

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
