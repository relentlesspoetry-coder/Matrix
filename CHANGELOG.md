# Changelog

## 0.5.4

- Add guided emulator setup for AppImage, installed Flatpak and native executable profiles. Browse for emulator files, optionally grant only owner execute permission to a validated AppImage, and support extraction mode without FUSE.
- Discover installed Flatpak apps, preserve installation scope, validate selections on save/launch and grant temporary read access to the current console's Games folder. Keep discrete arguments and original child cache environments.
- Make each console's favorites list visible through My favorites, filter counts and empty-list guidance. Persist native favorites in settings, migrate compatible prior UI favorites once, and preserve separation across all 14 consoles and Steam/Windows.
- Cover legacy profile compatibility, AppImage validation/permission consent, Flatpak discovery/launch arguments, favorites console boundaries/migration and persistence across an actual process restart. Real emulator gameplay remains unverified.
- Preserve dated verification notes when updating install instructions for new releases.

## 0.5.3

- Add an Update button: check public GitHub releases, download and verify signed Linux x86-64 packages, then install and restart. DEB installs request system authorization; AppImage installs replace the current file and retain a `.previous` backup. Development builds open the verified DEB in the system installer.
- Automatically build, test, sign and publish a new version pushed to main. Add `scripts/release.py` to bump, commit and push each version.
- Display the supplied artwork for all 14 existing console libraries. Retain four extra supplied images without adding unsupported libraries.
- Update the public GitHub README with the Matrix Play logo, purpose, installation instructions and capability limits.
- Require the running executable to be inside its own AppImage mount before allowing replacement; reject inherited AppImage environments from another launcher.
- Correct a native launch-test timing issue by waiting for the completed cache-environment log.

## 0.5.2 — Matrix Play name and logo

- Apply the supplied Matrix Play logo to the sidebar and its central symbol to compact branding, window and Linux launcher icons. The supplied full PNG is retained unmodified.
- Rename the native window, application menu entry, package metadata, page title and interface copy to Matrix Play.
- Keep existing Matrix libraries and persistent data in place; fresh installs use a Matrix Play game folder. Maintain all 14 separate console libraries and independent Steam/Windows tabs.
- Set the renamed `matrix-play` DEB to replace the former `matrix` application package cleanly. Keep the existing executable and application identifier for compatibility.
- Retain the v0.5.1 runtime-cache startup fix and diagnostics.

## 0.5.1 — Resolve cache-dependent blank startup

- Isolate Matrix's runtime cache before WebKit starts; retain existing data, configuration, artwork and shared caches.
- Restore the original cache environment for every external launch.
- Add bounded startup/load/process diagnostics and a visible interface startup error/retry panel.
- Cover normal painted-window startup outside the project folder, Wayland without forced graphics settings, child-process environments and interface script failure.
- Resolve WebKitWebDriver from PATH before starting the isolated native test, and shut down the complete test process group.
- Fix the failed native test step in GitHub's initial v0.5.0 workflow and add Rust build caching.

The affected user confirmed that the fresh-cache launch works. The rebuilt application also passed the formerly stalled existing-profile startup. The exact offending shared cache file remains unidentified; no shared cache files are deleted.

## 0.5.0 — Automatic PS1/PS2 artwork

Continues the supplied Matrix-v0.4-native-source.zip. Keeps all 14 managed console directories and the independent Steam and Windows sections.

- Adds the `matrix-cover-core` Rust library for serial normalization, bounded SYSTEM.CNF extraction, exact console/serial/style URLs, decoded-image validation, positive/negative disk caching, provenance, cross-process request serialization and provider cooldowns.
- Connects background automatic artwork scans to native startup/Refresh; adds persistent enabled/style controls, result/identity status and explicit serial confirmation in game details.
- Keeps local/manual artwork ahead of downloaded artwork. Restricts generic cover.* discovery to unambiguous console subfolders.
- Adds a manual TheCoverProject browser button; no scraper was enabled because authorization could not be established. COVER-PROVIDERS.md records the evidence and limits.
- Adds deterministic Rust cover-engine tests, a separately invoked four-URL live test, actual headless-backend tests, and expanded browser UI checks. Adds portable Chromium test configuration and updates build instructions.
- Adds a desktop Cargo feature, enabled by default. Tests can compile the backend without Tauri/GTK using `--no-default-features --lib`.

Validation: 12 deterministic cover-engine tests, 5 actual headless-backend tests, one live smoke test covering four downloads, the folder-layout test and the simulated-native Chromium UI suite passed. Native desktop validation is recorded below. Real emulator/Steam/Wine gameplay remains unverified. See README.md.

## Native Linux testing release

- Compiled and checked the complete Tauri desktop application on Zorin 18.1 x86-64; generated DEB and AppImage installers. Both passed the actual native-window test.
- Added pinned npm Tauri tooling and a GitHub Linux build workflow.
- Added an actual native WebDriver test with isolated filesystem fixtures, two optional live artwork requests, manual cover round-trip, settings preservation and safe test launcher dispatch for console/Steam/Windows.
- Added installation/testing instructions and a real native window screenshot. Real gameplay and user-specific emulator/Steam/Wine behavior remain for testing.
