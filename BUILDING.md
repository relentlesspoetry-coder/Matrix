# Build Matrix-Play

Requires Linux, Rust 1.90+, Node.js 20+ and the Tauri 2 Linux dependencies.

## Ubuntu 24.04 / Zorin OS 18

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libdbus-1-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev pkg-config
npm ci
```

Start the desktop development app:

```bash
npm run tauri -- dev
```

Run the build script to test and package DEB and AppImage installers:

```bash
./scripts/build-native.sh
```

Packages are written to `src-tauri/target/release/bundle/`.

## Tests

```bash
node --test tests/test_*.cjs
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features --lib -- --test-threads=1
```

Direct desktop Cargo builds need the update helpers first:

```bash
cargo build --locked --release --manifest-path src-tauri/Cargo.toml --no-default-features --bin matrix-install-update --bin matrix-update-coordinator
```

See [TESTING.md](TESTING.md) for native-window tests and [UPDATES.md](UPDATES.md) for the release workflow.

## Browser preview

Open `web/index.html` to preview the interface with sample games. Native library scanning and launching require the desktop app.
