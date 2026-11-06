# v0.5.1 — Resolve cache-dependent blank startup

- Isolate Matrix's runtime cache before WebKit starts; retain existing data, configuration, artwork and shared caches.
- Restore the original cache environment for every external launch.
- Add bounded startup/load/process diagnostics and a visible interface startup error/retry panel.
- Cover normal painted-window startup outside the project folder, Wayland without forced graphics settings, child-process environments and interface script failure.
- Resolve WebKitWebDriver from PATH before starting the isolated native test, and shut down the complete test process group.
- Fix the failed native test step in GitHub's initial v0.5.0 workflow and add Rust build caching.

The affected user confirmed that the fresh-cache launch works. The rebuilt application also passed the formerly stalled existing-profile startup. The exact offending shared cache file remains unidentified; no shared cache files are deleted.

# v0.5.0 — Automatic PS1/PS2 artwork

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
