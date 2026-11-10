<p align="center"><img src="web/assets/matrix-play-logo.png" alt="Matrix-Play" width="320"></p>

# Matrix-Play

A Linux game-library frontend for your console games, Steam library and Windows game shortcuts.

[Download](https://github.com/relentlesspoetry-coder/Matrix/releases) · [Setup guide](EMULATOR-LAUNCHES.md) · [Report a problem](https://github.com/relentlesspoetry-coder/Matrix/issues)

![Matrix-Play library](screenshots/matrix-linux-home.png)

## Install

Download the **DEB** or **AppImage** from [Releases](https://github.com/relentlesspoetry-coder/Matrix/releases). Linux x86-64; tested on Zorin OS 18.1 and Ubuntu 24.04.

For Zorin or Ubuntu, install the DEB from your download folder:

```bash
sudo apt install ./Matrix-Play_0.5.25_amd64.deb
```

For the portable AppImage:

```bash
chmod +x Matrix-Play_0.5.25_amd64.AppImage
./Matrix-Play_0.5.25_amd64.AppImage
```

Use **Update** inside the app for future releases.

## Features

- 18 console libraries, plus separate Steam and Windows collections.
- Existing game folders and external drives, without moving your files.
- Native, AppImage and Flatpak emulator setup, with automatic detection for supported emulators.
- Cover art, favorites, search, grid/list views and controller navigation.
- Fullscreen game launches for supported emulators.

## Console libraries

| Nintendo | PlayStation | Sega |
| --- | --- | --- |
| Switch, Switch 2, N64, GameCube, Wii, Wii U, Game Boy, GBA, DS, 3DS | PS1, PS2, PS3, PS4, PS5, PSP, PS Vita | Dreamcast |

PS5 and Switch 2 use manually configured emulators. See the [setup guide](EMULATOR-LAUNCHES.md) for formats and launch options.

## Get started

1. Open a console library and choose **Manage folders** to add your games.
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
