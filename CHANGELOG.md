# Changelog

## 0.5.11

- Prompt users to save progress, close launched games/apps and wait for installers to finish. Block detected activity, close only cooperating Matrix windows, and verify their exit before installation through a separate coordinator. Never terminate games/installers or signal unrelated processes.
- Gate new Matrix starts during installation, preserve independent privileged-installer protection, and stop on ambiguous legacy/protected process identity. Earlier versions need one manual upgrade after closing Matrix; the new protections cannot apply retroactively.
- Keep private AppImage runtime paths out of external programs. Fix Python-based launcher startup while preserving non-bundled user paths/settings and the internal log sink runtime.
- Preserve the original AppImage rollback copy across repeated or concurrent updates, with cross-process locking, unchanged-target checks and recovery on failed replacement.
- Reject non-regular Steam manifest, desktop-entry and PCSX2 configuration files without blocking scans or launch preflight. Keep bounded regular-file reads and existing regular symlink targets.
- Require an explicit absolute Dolphin user-folder path for ambiguous -u/--user profiles. Reject unsafe launch/save attempts with repair guidance while retaining existing profiles and game/save files unchanged.
- Keep controller focus outlines after Back, and reconcile the actual retained cover after a later cover selection fails.
- Prevent delayed refreshes from restoring older library rows, emulator-picker results/errors from overwriting newer dialogs or drafts, and older cover selections from replacing newer choices. Serialize per-game native cover writes.
- Group PS1 ISO/CUE companions only when the CUE is a scanned regular file. Handle mixed-case extensions, ignore directory/symlink companions and retain favorite or manually illustrated ISO entries. Preserve the 8,000 logical-game limit for large ISO/CUE collections.
- Label release-year sorting accurately and disable it with an explanation when year metadata is unavailable. Update preview test selectors and include controller code in the browser harness.
- Recognize PSP CHD, GameCube WIA/CISO and Wii GCZ/CISO images using the existing PPSSPP and Dolphin launch adapters, with matching preview filters. No conversion or game-file changes are performed.
- Read bounded local PARAM.SFO display titles for registered PS3 installed/disc EBOOT folders and extracted PS4 eboots. Keep canonical game paths, favorites, artwork and launch arguments unchanged; malformed or unavailable metadata falls back to folder names.
- Add malformed-metadata, scope, format, customization-preservation and literal-argv fixtures, plus native IPC/UI coverage. These synthetic checks do not certify real emulator gameplay.

## 0.5.10

- Add external console library folder management with a native folder picker, retained managed folders, offline registrations and safe removal without moving or deleting games, saves, favorites or artwork. Existing RPCS3 installed EBOOT folders can be registered.
- Group valid PS1 .m3u disc playlists for DuckStation while preserving customized disc entries. Reject unsafe, missing, nested and oversized playlists; keep unsupported Dreamcast/.m3u8 discs independent.
- Add standard-mapped gamepad navigation: D-pad/stick movement, selection, Back, sequential controls and select options, with modal boundaries and held/reconnected input safeguards. Text input and system pickers still need keyboard/mouse; real hardware mappings remain unverified.
- Detect Snap Steam and register custom Steam library roots with their explicit native/Flatpak/Snap owner. Preserve disconnected roots and route launches without selecting a different Steam installation.
- Add exact-ID GameCube/Wii Default and 3D artwork from GameTDB, validated disc-header/filename identity and manual confirmation. Keep local covers first and never guess a similar title or another region.
- Expand backend, UI-lifetime and native-window fixture tests. Fix stale folder-list results, draft input races and modal focus. Preserve 18 console libraries and separate Steam/Windows desktop-shortcut tabs. PS4/Vita remain experimental; PS5/Switch 2 remain library-only.

- Fix melonDS Flatpak game launches so the current ROM directory permits sidecar saves, without granting write access to unrelated folders or setup-only launches.
- List all installed native emulator choices and console-folder candidates instead of silently selecting the first match. Preserve saved choices and leave ambiguous discoveries for selection.
- Keep pending emulator saves and Windows shortcut additions/removals from closing or changing a newer dialog. Repeated submissions are blocked.
- Preserve an unsaved artwork serial and keyboard focus when favoriting a game. Add accessible names to collapsed navigation and header controls.
- Add backend, JavaScript and native desktop regression coverage. Real emulator gameplay/save creation and other Linux distributions remain unverified.


## 0.5.9

- Remove direct Windows executable launching and runtime selection. Keep the Windows tab for existing `.desktop` shortcuts and preserve Steam launching through its owning installation. Matrix Play does not provide a Windows compatibility runtime.
- Preserve older direct-executable registrations and artwork, but disable their launch action and explain how to register a working desktop shortcut. Installed game files and external software are unchanged.
- Fix emulator setup keyboard navigation: Tab and Shift+Tab now continue through Advanced launch options and multiline arguments instead of jumping to Close. Skip hidden, disabled and non-tabbable controls when wrapping dialog focus.
- Handle Linux WebKit shifted Tab key events so reverse keyboard navigation cannot escape behind the dialog.
- Add ten production-handler regressions and real WebKit keyboard checks for forward/reverse navigation, collapsed advanced settings, hidden Flatpak/file fields and dialog focus wrapping.
- This release also restricts the Windows library to existing desktop shortcuts; it does not add gamepad support. Real Steam/desktop-shortcut gameplay, emulator save creation and a privileged package upgrade remain separate integration checks.

## 0.5.8

- Grant temporary game-directory write access to recognized SameBoy/mGBA Flatpaks for adjacent save files, while keeping setup access read-only and other libraries separate.
- Route Steam launches through the installation that owns each scanned library, including external Flatpak libraries and legacy aliases. Correct PS3 digital/disc title names.
- Monitor Steam and Windows launcher failures, keep bounded launch details available, and prevent delayed launch responses from dismissing or replacing newer dialogs.
- Serialize cover replacement across processes and preserve existing artwork on failed writes. Revoke discarded preview image URLs and keep async artwork/settings/Flatpak results from disrupting later navigation.
- Keep update controls and keyboard focus stable during progress polling without turning a Cancel press into Install, reject stale updater responses, and allow Check again immediately after a failure or cancellation.
- Require exact, nonempty, unique release-note headings and declare the DEB's pkexec runtime dependency. Run every JavaScript regression suite in CI.
- Verify Linux installer builds, the packaged helper and the real Tauri desktop fixture suite before release. Launcher fixtures do not certify gameplay across emulators or perform a privileged upgrade.

## 0.5.7

- Reject stale artwork results after refresh or cover replacement, and interrupt update downloads during stalled network waits without publishing partial packages.
- Verify signed DEB metadata and package bytes independently in a packaged privileged helper, using protected staging instead of mutable user download paths.
- Serialize settings changes across cooperating processes and merge discovery into the latest saved configuration.
- Preserve emulator configuration arguments in setup mode, validate PS4 eboot favorites/artwork consistently, and retain recent launch diagnostics through GUI closure and log-write failures.
- Correct semantic version ordering and reject Debian-incompatible prerelease suffixes before combined installer builds, release changes or signing.
- Add regression tests and inspect built DEB helper ownership, permissions and architecture. Test fix-branch pushes without running the main-only publication job. Require Rust 1.90 for the locked Tauri dependencies.
- Existing DEB users should install this release manually once to add the secure privileged update helper. Game libraries, artwork and settings are retained.

## 0.5.6

- Start recognized console emulators fullscreen when a game card is clicked, using engine-specific no-GUI/batch/hidden-menu options where supported. Keep Open emulator in normal setup mode. Apply presets to existing recognized profiles without requiring them to be saved again; preserve literal custom-launcher arguments.
- Add the four remaining supplied artwork libraries: PlayStation 4, PlayStation 5, PlayStation Vita and Nintendo Switch 2. Preserve the original 14 console libraries, independent Steam/Windows tabs, existing paths, covers and favorites.
- Add experimental shadPS4 core/current QtLauncher (including its Flatpak) and current Vita3K SDL launch adapters. Scan PS4 extracted eboot/ELF content and Vita VPK/ZIP content. Discover recognizable PS4/Vita AppImages/native executables and the shadPS4 Flatpak.
- Mark PS5 and Switch 2 as library-only collections, supporting folders, local covers and favorites while rejecting launch/setup requests. Never link PS4/Switch emulators to their successor consoles.
- Add native fullscreen argv, collection launch-blocking/favorites and expanded folder/artwork tests; document researched capabilities separately from fixture tests and real gameplay evidence.

## 0.5.5

- Automatically discover and link known installed Flatpaks and recognizable AppImages to their supported console libraries. Scan common app folders and exported desktop paths, deduplicate canonical paths, preserve manual selections and require a choice for ambiguous matches.
- Add Find emulators, Detected emulators, Open emulator and Save and open emulator controls. Preserve all 14 console libraries, favorites, Steam and Windows sections.
- Work around the reproduced PCSX2 Flatpak fontconfig crash using a per-launch runtime-font configuration, enabled by default for PCSX2 and optional for other Flatpaks. Leave shared fonts/caches and persistent Flatpak overrides unchanged.
- Explain unfinished PCSX2 setup/BIOS selection, capture bounded owner-only launch logs, report startup errors and show launch details. Check launcher status without claiming that spawning a process proves gameplay.
- Add discovery, permission/deduplication, argument-identity, setup-preflight, log-bound and real native launch-failure tests. Preserve saving without launching; only Save and open starts the emulator.

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

Validation: 12 deterministic cover-engine tests, 5 actual headless-backend tests, one live smoke test covering four downloads, the folder-layout test and the simulated-native Chromium UI suite passed. Native desktop validation is recorded below. Real emulator/Steam/desktop-shortcut gameplay remains unverified. See README.md.

## Native Linux testing release

- Compiled and checked the complete Tauri desktop application on Zorin 18.1 x86-64; generated DEB and AppImage installers. Both passed the actual native-window test.
- Added pinned npm Tauri tooling and a GitHub Linux build workflow.
- Added an actual native WebDriver test with isolated filesystem fixtures, two optional live artwork requests, manual cover round-trip, settings preservation and safe test launcher dispatch for console/Steam/Windows.
- Added installation/testing instructions and a real native window screenshot. Real gameplay and user-specific emulator/Steam/desktop-shortcut behavior remain for testing.
