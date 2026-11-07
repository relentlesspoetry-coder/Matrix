# Test Matrix Play on Linux

## Version 0.5.12 PS5 / Switch 2 custom-emulator support

Local Rust 1.99.1 passes all 133 headless backend tests using the required
`--test-threads=1` setting, plus 16 cover-core and 31 updater-core tests (two
opt-in live-cover checks are not run). All 117 JavaScript fixtures pass. Folder,
CI-account, native-lifecycle-fixture, release/signing, DEB-helper and standalone
launch-log-lifetime checks pass. Release tests use disposable local repositories
and signing keys; they do not publish the project. Formatting, JavaScript/Python
syntax and patch-whitespace checks pass.

New fixtures cover manually saved profiles, retained favorites/covers/files,
exact PS5 eboot/ELF/ZAR recognition, legacy collection formats, no automatic
linking, literal file/folder argv with spaces and shell-looking text, setup with
no arguments, and PS5/Switch 2 setup/save/open/game UI dispatch. Native fixtures
were extended for executable and selected Flatpak launches through real IPC,
including read-only mounts and the distinct file/folder argument contracts.

The native fixtures are written but not run locally: GTK/WebKit development
packages and desktop drivers are absent here. Chromium cannot create its IPC
socket in this executor, including with the supported escalation, so the updated
Chromium UI suite and screenshots are also unverified. CI/native-window checks
must pass on the exact candidate before release. An initial parallel backend run
passed 131/132 tests but raced the pre-existing installer-gate fixture; the
required serial run passed, and all 133 tests passed after final test additions.
No real PS5/Switch 2 emulator binary or game was executed. These checks do not
certify gameplay, fullscreen rendering or emulator version compatibility.

## Version 0.5.11 async ownership and console-library checks

Local Rust 1.99.1 passes 130 headless backend tests, 16 cover-core tests (two
opt-in live-provider checks omitted), 31 updater-core tests and 114 JavaScript
fixtures. Folder, release/signing, DEB-helper and log-lifetime checks also pass,
along with formatting, syntax and patch-whitespace checks. Release fixtures use
disposable local repositories and keys, not a real project publication.
New isolated-home tests register external PSP CHD, GameCube WIA/CISO and Wii
GCZ/CISO images, validate and favorite them, round-trip imported artwork, launch
harmless named PPSSPP/Dolphin shims, and verify exact literal arguments for paths
containing spaces and shell punctuation. Unregistration preserves files and
customizations; re-registration restores access. These are synthetic fixtures,
not real image decoding or gameplay.

SFO tests cover PS3 installed/disc and PS4 extracted layouts, UTF-8 titles and
punctuation, truncated files, invalid offsets/lengths/types, size limits, invalid
UTF-8, control characters, missing terminators, symlink exclusion, registration
boundaries and malformed-metadata fallback. Title changes preserve canonical
UIDs, favorites, artwork and original game bytes. Native IPC/UI cases also cover
new-format launch arguments and escaping of metadata-derived titles.

The installed-title/format slice at `5be72628dcf7b531372b605812eebd3b15cc07e6` passed
[Linux CI run 42401537952](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42401537952),
including the actual native window, both installers and live cover-provider tests.
Later async and CUE repairs repeat all native/installer gates on the exact versioned
release before publication. GTK/WebKit development libraries and desktop drivers
are absent in this local executor. No gameplay compatibility claim is made.

Additional regressions cover out-of-order library refreshes, emulator browsing
after dialog/draft changes, consecutive cover reads/writes, preview IndexedDB
transaction order and unavailable release-year sorting. CUE grouping tests cover
uppercase extensions, directories, symlinks, distinct Linux filename stems,
console separation and favorite/manual-cover preservation. Native fixtures also
exercise CUE scanning and release-year availability through the running app.
Independent backend checks passed all three reproduced CUE cases and synthetic
launch/data-preservation cases across 16 adapters, including external-drive
disconnection/reconnection. These remain harmless fixture launches, not gameplay.

Deeper independent checks reproduce and cover FIFO metadata hangs, 4,100-game
ISO/CUE capacity, and relative Dolphin user-folder ambiguity. Profile validation
requires the user to choose an absolute existing save/config location; it does not
silently rebase relative paths. The local reader checks regular files before and
after a nonblocking open, preserving ordinary symlinked configuration files.
Controller Back and failed-latest-cover recovery regressions also pass.

The earlier repair batch at `25d3d473589f18caa8f58678ac657bb2daedd63e` passed
[Linux CI run 42403804510](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42403804510),
including native and installer checks. This does not certify the later coordinated
shutdown changes; their exact versioned commit must pass the expanded pipeline.

The shutdown fixtures require an unchecked save-and-close confirmation, reject
known active launches even after confirmation, and preserve games, log sinks,
installers and unrelated processes. Independent disposable-process checks cover
late hidden children, retained helpers from crashed prior instances, PID identity,
duplicate updaters, startup gates, partial
failure and reopening. Recovery and cancellation messages remain visible until an
explicit retry rather than being erased by an automatic update check. A real signed AppImage was staged only onto disposable
fixture targets to verify duplicate-install rollback preservation. Repeated core
suites exercise inherited lock descriptors and concurrent transactions.

Process-ownership fixtures run under a separate unprivileged CI account with a
clean environment and copied test binaries. The runner's protected background
service deliberately fails the production ownership check when it shares a UID;
isolation avoids that unrelated test-environment blocker without relaxing the
updater's fail-closed checks or sharing runner credentials. Built-DEB checks also
inspect the executable, root-owned post-install hook without running it, and
reject a replaceable update-lock file in the package payload.

The expanded native pipeline opens two actual Matrix windows, refuses a third
while the update gate is held, verifies both actual exits before a harmless
replacement marker, and checks that a launched writer survives, finishes, and
keeps its final log output. It repeats shutdown through the built AppImage in
extraction mode. The marker is available only in test code; production update
coordinators require signed metadata and installer bytes. These tests do not
perform a privileged APT installation. The root helper's system guard is checked
by source and package inspection, not by changing the executor's system files.
Unverifiable processes intentionally block updates rather than being killed.
Shutdown retains each observed PID and birth identity while waiting within its
existing timeout. Brief processes may exit normally; a still-running process
cannot become safe merely by clearing its token or restoring environment access.
Replacement also requires a clean scan after all tracked processes have exited.
Independent synchronized fixtures cover a child handing off to a grandchild
after enumeration, disappearance between UID and identity reads, and a baseline
process becoming a zombie before identity collection. Each keeps replacement
blocked while the descendant lives. Newly observed dead identities invalidate a
scan once and are retained, avoiding endless retries for stable zombies.
The raw native and extracted-AppImage checks each run twice, with both passes
required and bounded test-only process diagnostics retained for each run.

Installer transport fixtures reproduce the old raw broken-pipe error using a
harmless closed reader, then check retained cancellation/rejection details,
incomplete transfers, successful full transfers, verbose concurrent output and
a descendant holding stderr open. Raw and normalized diagnostic tails are capped
at 4 KiB, including invalid UTF-8 and JSON-escaping cases that must survive the
16 KiB persisted-outcome limit. No real pkexec or package installer is executed.
These checks improve failure diagnosis; they do not establish the underlying
cause of a particular user's authorization or package-manager failure.

Versions through 0.5.10 do not implement these lifecycle gates. Save and close
running work and Matrix Play, then install the first upgraded version manually.

The expanded AppImage test exposed a real external-launch defect: AppRun's
bundle-specific PYTHONHOME/PYTHONPATH prevented the host Python writer from loading
its standard library. A subprocess regression reproduces the failure, then verifies
shared launch cleanup while retaining a user-provided module path. Cleanup applies
only to a verified Matrix AppImage and removes bundle-owned paths, not unrelated
user environment values. An original PYTHONHOME already overwritten by AppRun
without a saved copy cannot be reconstructed. The internal log sink retains its
bundled runtime.
Actual extracted-AppImage verification must pass on the final exact commit.

PS3/Wii U automatic artwork is not included: a separately staged provider change
needs correction and production-transport verification.

## Version 0.5.10 library, navigation and repair validation

Local Rust 1.99.1 checks pass: 67 headless backend tests, 16 cover-core tests
(two explicit live-provider tests omitted) and 13 updater-core tests. JavaScript
regressions cover gamepad commands, dialog boundaries, external-folder ownership,
draft preservation, exact cover queues and Steam registration parameters. Folder,
release/signing, DEB-helper fixtures and launch-log lifetime checks pass locally.

Feature commit `0eacb38db31a022e3addd657808d3a6e83948380` passed
[Linux CI run 42396471855](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42396471855):
67 backend tests in both headless and desktop builds, 16 cover-core tests,
13 updater-core tests, all 92 JavaScript regressions, Python gates, DEB/AppImage
builds, packaged-helper verification, ordinary painted startup and the actual
Tauri/WebKit fixture suite. Both live-provider smoke tests passed in CI.
The exact versioned commit repeats these gates before signed publication.

The native fixtures cover external-folder dialog/IPC, retained favorites/imported
artwork, PS1 playlist arguments, disconnected roots, Snap/custom Steam routing,
and simulated WebKit gamepad input. They use synthetic game files and harmless
launchers. The folder dialog and startup screenshots were visually inspected.
Do not treat synthetic gamepad input as physical-controller certification.

The current executor rejects both old xlenore and new GameTDB production HTTP
requests with an unknown-issuer TLS error. TLS checks were not bypassed. Two
public GameTDB samples downloaded and decoded through the system HTTP client;
the release workflow separately records the production live-provider smoke test
as informational because network/provider outages should not break offline use.

Playlists are PS1 .m3u only. DuckStation source supports that format; standalone
Flycast does not provide the same playlist adapter, so Dreamcast grouping and
.m3u8 are excluded. Customized disc rows remain visible with the playlist.
PS3 installed EBOOT directories work through explicit folder registration, not
a universal installed-title importer. Other distributions, physical controllers,
real emulator save creation, encrypted content and a privileged package upgrade
still need their own integration checks.

### Reviewed bug repairs

Repair commit `e417aa97279514984c86da0b9d8c4839ebe6fe5c` passed
[Linux CI run 42395357141](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42395357141):
all Rust/JavaScript/Python gates, DEB/AppImage builds, packaged-helper checks,
ordinary painted startup and the actual Tauri/WebKit fixture suite.
Versioned main repeats these gates before signed publication.

The repair covers scoped melonDS Flatpak sidecar-save permissions, complete native
emulator discovery, async dialog ownership, draft artwork identity preservation
and accessible names on narrow navigation. New tests exercise multiple installed
choices, aliased executables, ambiguous selections, nested/direct ROM parents,
outside-root and symlink rejection, repeated form submissions and newer dialogs.
The desktop suite also checks melonDS permission arguments through actual Tauri IPC
with a harmless launcher. These tests do not prove real emulator gameplay or save
creation, and they do not certify additional distributions or hardware.

Local checks use Rust 1.99.1. GTK/WebKit development packages are absent in this
executor; the exact release commit must pass the Linux CI native-window and
DEB/AppImage packaging gates before publication.


## Version 0.5.9 keyboard and desktop-shortcut checks

The Windows library accepts only existing `.desktop` shortcuts and dispatches
through `gio launch`. There is no direct Windows executable/runtime launch path.
New backend/native regression cases reject direct `.exe` registration and saved
legacy launches, preserve legacy entries and installed files, and verify that the
Windows UI explains the limitation while Steam routing remains unchanged.
The native tests use harmless `gio` and Steam fixtures, not real gameplay.

Final removal commit `0f87126ddc91447e975d2c6f8add3f23a9c97399` passed
[GitHub Actions run 42277673138](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42277673138):
13 updater-core tests, 12 cover-core tests (the opt-in provider test was skipped),
47 backend cases in both headless and desktop builds, all 52 JavaScript fixtures,
folder/release/package/log-lifetime checks, DEB/AppImage builds, packaged-helper
validation, ordinary painted startup, and the actual Tauri/WebKit suite. The
native suite passed the keyboard boundaries and desktop-only/legacy-preservation
checks together. Versioned main runs these gates again before publication.

The emulator setup dialog's focus trap omitted both the Advanced launch options
summary and its multiline arguments field. Five new fixture cases failed before
the repair. All ten keyboard cases and all 52 production-JavaScript fixture tests
pass after it, along with the folder, release/signing and DEB packaging fixtures.
Native testing also reproduced GTK/WebKit reporting Shift+Tab as an unidentified
key, allowing focus to escape behind the dialog; the trap now recognizes Tab
through its physical key code or legacy key code too. The native desktop suite uses WebDriver key actions to verify Tab/Shift+Tab
through expanded and collapsed advanced settings, hidden Flatpak/file controls,
and the dialog boundaries. The release pipeline must pass these native checks
on its exact commit before publication.

Current local Rust and native-browser checks were unavailable: no Rust toolchain
is installed and Chromium cannot open its IPC socket in this executor. The
GitHub Linux workflow runs the full Rust/build/native gates. No new definite
save-permission, Steam/desktop-shortcut or updater bug was found in the targeted source
review. Synthetic launchers are not real Steam/desktop-shortcut gameplay or emulator save
creation, and these checks do not perform privileged installation. This repair
improves keyboard access; it does not add or certify physical gamepad support.

## Version 0.5.8 repair verification — 2026-11-06

The combined repairs were integrated against main
`c13edefbe81e73a6f3200d2a1ef57fe28baf83de`. The exact repair commit
`b981213b1e2126244331a4d14dcf2ead6f0ca083` passed
[GitHub Actions run 42248122522](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42248122522):
all Rust/JavaScript/Python regressions, DEB and AppImage builds, packaged helper
validation, ordinary painted startup, and the real Tauri/WebKit desktop fixture
suite. The release pipeline runs these gates again on versioned main before
signing and publishing 0.5.8.
Local Rust 1.99.1 checks passed: 13 updater-core tests, 12 cover-core tests
(the opt-in live-provider test was skipped), and 45 headless native-backend
tests. The release-mode `matrix-install-update` helper also compiled.

`node --test tests/test_*.cjs` passed 37 production-JavaScript fixture tests,
including delayed launch failures, dialog ownership, artwork transactions,
stale requests, updater focus, safe Cancel/Install transitions and repeated or
interrupted operations. CI runs this
complete glob rather than only the artwork suite. Folder structure, all three
launch-log lifetime tests, all three DEB packaging fixtures, release/signing
fixtures, Rust formatting, JavaScript/Python syntax and patch whitespace checks
also passed. The release fixtures use disposable local remotes and keys, not
the project's GitHub repository or signing credentials.

The native suite verified scoped sidecar-save Flatpak arguments, native/Flatpak
Steam routing, delayed Windows runner diagnostics, artwork round-trips and
settings/favorites across restart using harmless launchers. An initial CI run
stopped because the added test expected "exit status 17" instead of Rust's
"exit status: 17". The corrected assertion compares the actual backend message
with the visible error notice and still requires the exit code and captured
diagnostic; production launch code did not change for that correction.

These are synthetic launch tests, not real emulator/Steam/desktop-shortcut gameplay, real
save-file creation or a privileged package upgrade. Local desktop testing was
blocked by missing GTK/WebKit development packages and restricted browser IPC;
the full desktop and built-package checks above ran in Linux CI instead.

## Reviewed-fix verification

The integrated fixes were checked locally with Rust 1.99.1: updater-core passed
13 tests, cover-core passed 12 tests (its opt-in live-provider test was not run),
and the headless native backend passed 32 tests. The production launch-log
harness passed all three tests, the release-mode privileged helper compiled,
and the artwork suite passed seven tests. Rust formatting, folder layout,
release/signing fixtures and syntax checks also passed. These tests use synthetic
data and do not perform a privileged package upgrade or real emulator gameplay.

The local desktop build stopped at missing GLib/GTK/WebKit development packages.
The browser test could not start because the environment restricts local IPC
sockets. Native desktop and built-installer verification therefore run in the
Linux CI job on the fix branch; check that exact commit's job before treating
those checks as passed. CI inspects the built DEB helper's path, root ownership,
executable permissions and ELF architecture without installing the package.
The publish job is restricted to main and does not run for fix-branch pushes.

The exact reviewed-fix commit `2909e04b725cff3fc03ec296e098adc0b0dacb54`
passed [GitHub Actions run 42213232803](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42213232803):
Rust/regression checks, Linux DEB and AppImage builds, packaged helper validation,
ordinary painted startup and the actual native desktop fixture suite. Publishing
was skipped for that fix-branch run. Release 0.5.7 runs the same gates again on
its versioned main commit before signing and publication. These checks do not
perform a privileged installation or real emulator gameplay.

## Backend regression checks

The headless backend suite includes separate-process concurrent settings writers,
discovery merging after another window saves, failed-transaction preservation,
PS4 eboot favorites/artwork, and setup argument preservation. Every settings
writer uses the same persistent lock sidecar and unique atomic-replace temporary;
discovery scans outside the lock and applies candidates to the latest settings.
This protects cooperating current-version processes, not older versions that do
not use the lock.

The update-core suite also checks the signed privileged-install protocol, changed
package bytes and protected staging. `python3 tests/test_launch_log_lifetime.py`
checks output after the launching process exits and log-write failure using
synthetic subprocesses. Run the normal Rust suites and desktop tests as well;
these checks do not replace a real emulator test or privileged DEB upgrade test.

This is the native testing release, version 0.5.12. GitHub Actions builds the installers on Ubuntu 24.04 for **x86-64 / amd64** computers. Earlier installed-app testing used Zorin OS 18.1 (Ubuntu 24.04 base). Other distributions and ARM systems have not been validated.

## Install the DEB — recommended for Zorin 18 / Ubuntu 24.04

1. Open the [v0.5.12 GitHub release](https://github.com/relentlesspoetry-coder/Matrix/releases/tag/v0.5.12) — the repository and installers are public.
2. Download `Matrix-Play_0.5.12_amd64.deb`.
3. Open a terminal in the download folder and run:

   ```bash
   sudo apt install ./Matrix-Play_0.5.12_amd64.deb
   ```

4. Open **Matrix Play** from the application menu. You can also run `matrix-frontend` in a terminal to capture diagnostics.

APT installs the GTK/WebKit runtime dependencies if needed. Rust, Node, the source archive and developer tools are not required to run the installed application.

## Portable AppImage alternative

Download `Matrix-Play_0.5.12_amd64.AppImage` from the same release. In its download folder run:

```bash
chmod +x Matrix-Play_0.5.12_amd64.AppImage
./Matrix-Play_0.5.12_amd64.AppImage
```

If FUSE is unavailable, run it in extraction mode:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Matrix-Play_0.5.12_amd64.AppImage
```

The AppImage's actual native window, IPC, scanning, local covers, settings and launcher dispatch passed the same isolated desktop test in extraction mode. Other Linux distributions and native FUSE mounting have not been validated. The DEB is recommended on Zorin/Ubuntu.

## Upgrade from the blank-window build

Close Matrix Play, then install the v0.5.12 DEB using the same APT command above. The existing game folders, downloaded artwork, configuration and browser data remain in place. Open Matrix Play normally; no special graphics or cache launch command should be needed.

If a new startup problem occurs, attach `~/.local/share/matrix/startup.log` to the issue. This contains bounded startup stages and renderer/load errors, not game data. In this incident, graphics workarounds did not resolve the problem; a fresh cache directory did, which the affected user confirmed. The specific problematic shared cache file was not identified or deleted.

## Rename compatibility

The application menu now shows **Matrix Play** with the supplied logo. APT replaces the old `matrix` application package with `matrix-play`. Existing `~/Games/Matrix/` libraries are reused without moving files. New installations use `~/Games/Matrix Play/`. Settings and cover storage continue using their established `matrix` data directories.

## First-run testing

Matrix Play creates `~/Games/Matrix Play/` and 18 console subfolders. Place your legally obtained game files inside the appropriate nested `* Games` folder, such as `PS2/PS2 Games`. Existing Steam and Windows games stay in their existing locations.

1. Open a console library, choose **Edit emulator**, and verify the executable and argument tokens. Matrix Play lists recognized executable/AppImage choices in a console's parent folder and supported system installations. Ambiguous matches require a choice.
2. Select **Refresh** to rescan libraries. PS1/PS2 and exact-ID GameCube/Wii artwork downloads run in the background. Under **Emulators & folders → Cover artwork**, choose Default or 3D, or disable automatic downloads.
3. A filename with an exact Sony serial or a supported uncompressed SYSTEM.CNF layout can identify artwork. GameCube/Wii artwork uses a validated uncompressed disc header or explicit filename ID. For unknown or conflicting identities, open game details with the information button and confirm the exact serial or game ID. Compressed images may need a filename serial or confirmation. Matrix Play does not guess from similar titles.
4. Use **Choose cover** to import a local PNG/JPEG/WebP image. Local covers take priority. TheCoverProject is manual-only.
5. Clicking a game card sends a launch request immediately. Use its small information button to view details without launching.
6. Check **Steam Games** and **Windows Games** independently. Windows registration accepts only an existing `.desktop` shortcut that works outside Matrix Play. Direct `.exe` registration and launch are rejected, including saved legacy entries. Legacy entries and their artwork are preserved; register a working shortcut to replace them. External compatibility software is not supplied or configured by Matrix Play.

A game's emulator, firmware and any required keys must already be installed/configured by the user. Matrix Play does not include them. It does not move or rewrite game files.

## What was actually verified

- Optimized native Linux compilation and DEB/AppImage generation succeeded.
- A real Tauri/WebKit desktop window from the DEB payload was exercised through native WebDriver, in a separate X11 virtual display with an isolated temporary home directory.
- That test checked actual IPC, creation of all 14 managed console libraries, PS1/PS2 scans, cross-console path rejection, local cover round-trip, preservation of emulator/Windows settings, Steam and Windows tab separation, and process argument dispatch to harmless test launchers.
- The real desktop automatically downloaded one PS1 and one PS2 default cover using synthetic serial-labelled files. A separate Rust live test verified four sample provider URLs (both default and 3D).
- Twelve Rust cover-engine tests and five headless-backend tests passed. The simulated-native Chromium UI suite and folder-layout tests passed during source development. See README for current build checks.

**Additional v0.5.1 checks:** the rebuilt app reached Interface ready in the same existing-profile Wayland launch that previously stalled. Ordinary non-WebDriver X11 startup was checked by capturing the actual desktop pixels. Native tests cover both an explicitly provided and an unset parent cache directory, and ensure test launchers receive the original environment.

**Not verified across the emulator lineup:** fullscreen gameplay on every engine (one real PCSX2 launch was confirmed in v0.5.5), launching a user's Steam games or existing desktop shortcuts, controller support, ARM builds, other Linux releases, every disc format/serial, or universal cover availability. Launch fixtures validate command dispatch, not emulator compatibility. The update button checks and downloads signed GitHub releases. Privileged DEB installation has not been executed by the automated tests; AppImage replacement is tested with synthetic fixtures.

## Version 0.5.3 checks

- Six native/headless backend tests pass, including rejection of inherited/unrelated AppImage locations.
- Five updater-core tests passed: semantic version selection, signatures and repository/package identity, corruption/size limits, cancellation and cached downloads, package headers/symlinks, and staged AppImage replacement with backup using synthetic files.
- The mock-bridge Chromium test exercised check → download → install → restart controls and all 14 decoded console images. Its installer responses are simulated, not system package installation.
- The compiled desktop window passed native IPC/scanning/launch-fixture tests, rendered all 14 console images and exposed the real updater status. Ordinary non-WebDriver startup painted successfully without injected graphics settings.
- Rust cover and backend tests continue to pass. The release pipeline must also pass its Linux build and actual desktop checks before signing and publication.

## Version 0.5.6 checks

Nineteen backend tests pass, including engine identity/capability boundaries, fullscreen and no-GUI argv, current shadPS4 core versus QtLauncher syntax, per-launch configuration options, and rejection of launches from the PS5/Switch 2 library-only collections. The isolated-home test scans PS4 extracted eboot content, excludes PS4 PKG installation packages, scans Vita content archives and verifies favorites on the new collection libraries.

The native Tauri/WebKit fixture suite passed with 18 folders and decoded console images. It checked actual PCSX2 Flatpak dispatch of `-nogui -fullscreen -- <game path>` with the runtime-font workaround intact, RMG fullscreen/hidden-menu options, normal Open emulator arguments without game-only options, and library-only launch rejection. Launch paths containing spaces remain one argv token. These launchers are harmless scripts, not real emulators; this validates Matrix's dispatch, not fullscreen behavior on every emulator version.

Chromium UI checks passed using a mock native bridge, including the PS5 library-only notice, no Play/Setup action, collection favorites, Vita setup guidance and all 18 supplied images. Ordinary painted native startup passed in an isolated X11 display without graphics workaround variables. Cover-core (12) and updater-core (5) tests and release packaging tests passed. Live cover-provider checks from earlier versions were not repeated for this launch/library change.

The first cloud run (42155969970) stopped publication because the native test clicked Open emulator before the preceding asynchronous game request settled. The test now waits for that request to finish; it passed locally with the cloud software-rendering environment. Application launch code did not change for this correction.

After installing Matrix Play 0.5.6 and closing/reopening it, the affected user explicitly confirmed: **“Yes, fullscreen with no menus”** when clicking their actual PS2 game. This verifies the PCSX2 Flatpak fullscreen/no-GUI path on that machine, in addition to the fixture dispatch tests.

PS4/Vita adapters are researched against current upstream source but have not been tested with real PS4/Vita games. PS5 and Switch 2 have no Matrix launch adapter. Emulator errors, first-run requirements and overlays can still appear; unknown custom launchers need their own fullscreen options.

## Published v0.5.5 verification — 2026-11-05

The automatic [Linux build and signed release](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42152895832) passed. Downloaded DEB and AppImage hashes matched the manifest verified with the pinned signing key. Installed Matrix Play 0.5.5 was verified, and the affected user explicitly confirmed that clicking their PS2 game now launches it. Other emulator/game combinations remain unverified.

## Version 0.5.5 development checks

Fifteen headless/native backend tests cover exact-ID matching, shared console capabilities, ambiguous installations, valid/executable AppImage matching, canonical deduplication, preservation of saved choices, and avoiding engine-specific arguments based only on a display label. They also cover PCSX2 setup messages and bounded output capture.

The updated Chromium suite uses a mock bridge to verify detected choices, runtime-font selection, Open emulator, Save and open, escaped launch output and visible failure notices. Native fixtures exercise real discovery/IPC/launch dispatch, a failing Flatpak launcher with captured stderr, scoped runtime-font arguments, setup preflight and persistence. The fixture launchers are harmless scripts; they do not certify all real emulators.

Locally, the actual installed PCSX2 Flatpak passed its configuration check but crashed in fontconfig while opening the first-run wizard. Display/style/cache trials alone did not resolve it. A temporary abort diagnostic identified the crashing library; that diagnostic is outside the product/source repository and is not shipped. A minimal runtime-font configuration allowed the real setup window to remain open, and subsequent PCSX2 logs showed completed BIOS setup and a real game boot/run. This verifies the targeted workaround on the affected machine; it is not universal emulator/game compatibility testing.

## Version 0.5.4 development checks

Nine native/headless backend tests pass, including backward-compatible executable profiles, AppImage header validation and explicit execute-permission consent, installed-Flatpak parsing/scope, and favorites migration/deduplication/console boundaries. Mock-bridge Chromium tests exercise AppImage Browse dispatch, Flatpak selection/save, favorite add/remove, counts and empty states.

The compiled desktop test passed favorites persistence across a webview reload and a complete frontend process restart. It selected a Flatpak through the real setup form, persisted its installation scope and launched it with expected discrete arguments and console-specific read access. That launch uses a harmless fixture `flatpak` executable; it does not test real emulator gameplay. Ordinary non-WebDriver painted-window startup also passed without forced graphics settings. The native picker uses Tauri's dialog plugin; the browser test simulates its selected-path response.

## Published v0.5.4 verification — 2026-11-05

The automatic [Linux build and signed release](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42147273955) passed. The published DEB and AppImage matched the pinned-key signed manifest and all downloaded release checksums passed.

A real GTK emulator picker was opened in an isolated X11 display and cancelled; it returned no selected path. The published AppImage passed the native favorites/Flatpak fixture suite, including favorites persistence across a full process restart and correct AppImage self-update identity. Selecting a file in the UI test uses a simulated returned path; AppImage permission/header handling is tested against synthetic files. Actual emulator gameplay remains unverified.

The installed 0.5.3 application's real Update controls downloaded, verified and installed signed 0.5.4 through the system package installer. Its Restart control launched installed 0.5.4 and reached Interface ready. This was an intentional local upgrade with an isolated test home/display, not a privileged step in the ordinary test harness or GitHub workflow. A complete AppImage replacement/restart cycle remains unverified; its replacement/backup logic uses synthetic fixtures.

## Published release and installed-app verification — 2026-11-05

The public GitHub Actions run [42143081462](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42143081462) passed both build/test and signing/publication jobs for v0.5.3. Its six published assets include DEB, AppImage, source ZIP, checksums and the signed update manifest. Downloaded asset hashes were checked against SHA256SUMS.

The real updater engine verified the public signed manifest and complete AppImage download. An updater-capable temporary 0.5.2 test build used its actual desktop Update controls to download and verify the newer public 0.5.3 DEB, without running installation from the test harness. The published AppImage passed the native window/IPC/launch-fixture test in extraction mode and identified its own mounted/extracted location for self-update. A complete in-app AppImage install/restart cycle has not been exercised.

The affected user's normal launcher was still using installed `matrix` 0.5.0; the previously tested cache fix had not replaced it. The signed 0.5.3 DEB was verified and installed through the system package installer, replacing only `matrix` with `matrix-play`. Normal startup in the existing user's Wayland session reached Interface ready with no XDG_CACHE_HOME or graphics workaround supplied. The user then closed/reopened **Matrix Play from the application menu** and confirmed: **“Yes, it opens normally.”** The normal-launch problem is resolved on that tested installation. Other machines and real emulator gameplay remain separate testing work.

## Report a problem

Create a GitHub issue with your distribution/version, package version, console, file format, what you clicked, the result and relevant terminal output. Include the serial and artwork style for cover problems. You do not need to upload a game or firmware file.

Configuration: `$XDG_CONFIG_HOME/matrix/frontend.json` (normally `~/.config/matrix/frontend.json`).
Artwork: `$XDG_DATA_HOME/matrix/covers/` (normally `~/.local/share/matrix/covers/`).

To uninstall the application: `sudo apt remove matrix-play`. Your game library, artwork and user configuration are retained.

## Launch regression coverage

`node --test tests/test_launches.cjs` runs six deterministic production-UI checks
for delayed external errors, details escaping, repeat-click suppression, stale
poll cancellation, monitoring beyond startup and spawn rejection. These are
mock IPC tests, not native desktop or game execution.

The headless backend suite now has 37 tests, including native/Flatpak Steam
provenance for external libraries and aliases, both PS3 title layouts, canonical
Flatpak save-write boundaries, and delayed failures from synthetic gio
executables. `tests/test_native.py` also covers these dispatch/UI paths with
harmless launch fixtures when run in an environment with GTK/WebKit and an X11
display. Real emulator save creation and real Steam/desktop-shortcut games are separate manual
checks; passing fixture tests does not certify those integrations.
