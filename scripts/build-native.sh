#!/usr/bin/env bash
# Build Matrix Play native Linux packages on a developer workstation.
# Does not install system dependencies or modify the game library.
set -euo pipefail
repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
command -v cargo >/dev/null || { echo >&2 'Rust/Cargo not found. Install Rust from https://rustup.rs'; exit 1; }
command -v pkg-config >/dev/null || { echo >&2 'pkg-config is required'; exit 1; }
pkg-config --exists webkit2gtk-4.1 || { echo >&2 'WebKitGTK 4.1 development headers missing: install libwebkit2gtk-4.1-dev'; exit 1; }
if [[ -x "$repo/node_modules/.bin/tauri" ]]; then
  tauri_command=("$repo/node_modules/.bin/tauri")
elif cargo tauri --version >/dev/null 2>&1; then
  tauri_command=(cargo tauri)
else
  echo >&2 'Tauri CLI missing. Run npm ci in the Matrix Play folder.'
  exit 1
fi
cd "$repo"
echo 'Running Rust cover engine and native unit tests...'
cargo test --locked --manifest-path cover-core/Cargo.toml
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features --lib
cargo test --locked --manifest-path src-tauri/Cargo.toml
echo 'Building DEB and AppImage...'
"${tauri_command[@]}" build --ci --bundles deb,appimage -- --locked
printf 'Bundle output: %s\n' "$repo/src-tauri/target/release/bundle/"
