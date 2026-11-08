# Matrix Play updates

The app checks published releases in the public `relentlesspoetry-coder/Matrix` GitHub repository, including testing/prerelease versions. It selects the highest valid semantic version among the 100 most recent releases, compares it with its compiled version, and rejects downgrades. Successful checks are cached for 60 seconds. Failed checks, downloads and installations, including cancellation, clear that cache so Check again can retry immediately. It does not run a background updater or install without a click.

A release contains `update-manifest.json`, a raw detached 64-byte Ed25519 `update-manifest.sig`, DEB and AppImage packages, source ZIP and `SHA256SUMS`. The pinned public key is `update-core/update-public-key.bin`; its SHA-256 fingerprint is `b657a26878bcfa579481379f1cc650d9322b27a0d99bf65f3a30e07de4c3aaff`. Key rotation requires distributing an app with the new public key. Back up the private key securely; never add it to the source repository.

The signed manifest binds schema, repository, version, tag, Linux architecture/package, exact file name, size and SHA-256. URL construction is fixed to this repository, HTTPS redirects are limited to GitHub's release hosts, and metadata/package reads are bounded. Unknown manifest fields, invalid signatures, unexpected packages and size/hash mismatches fail closed. Downloading uses a temporary file and promotes only a verified complete package; cancellation removes the partial file. Valid cached downloads can be reused. Files are checked again immediately before installation.

DEB installations at `/usr/bin/matrix-frontend` use `pkexec /usr/lib/matrix-play/matrix-install-update`; Linux handles authorization, and Matrix Play never reads a password. The DEB packages this compiled helper as a root-owned executable and declares `pkexec` as a required dependency. The app sends the original signed manifest, signature and bounded package bytes through stdin. The helper accepts no command-line arguments, hashes or caller-selected file paths. It independently verifies the pinned signing key, repository, version, architecture and DEB identity, then copies the package into a new root-owned 0700 directory under `/var/cache` (never a caller-supplied TMPDIR). It checks the exact staged file against the authenticated size, hash and package header before invoking absolute `/usr/bin/apt-get` with a cleared environment. That private staging directory remains alive through APT completion and is then removed. Replacing or modifying the user-writable download after verification cannot alter the installed bytes. Existing installations without the helper must install this security update manually; there is no fallback to privileged installation of a mutable user path. AppImage mode requires the running Matrix Play executable to be inside its declared mounted/extracted APPDIR, preventing an inherited APPIMAGE from targeting another program. AppImage installation stages and verifies a new image in the same directory, preserves the existing file as `.previous`, then atomically replaces it. The folder must be writable. A development binary gives manual-installation guidance instead of starting an uncoordinated installer. Game folders, settings and covers are not updated by this process.

## DEB updater staging repair in v0.5.15

Affected DEB helpers created their temporary directory with the filesystem's
ordinary default permissions (usually 0755), then correctly rejected it because
installer staging must be private (0700). The error was “Installer requires
protected root-owned staging”, sometimes accompanied by Broken pipe because the
helper rejected the request before reading package bytes. No installation was
started, and the coordinator reopened Matrix Play to show the failure.

Version 0.5.15 requests 0700 atomically when creating staging, without weakening
root ownership, protected-parent, signed-manifest, package hash or downgrade
checks and without changing the caller's umask. Existing affected DEB installations
need one manual installation of the official 0.5.15 DEB after saving work and
closing Matrix Play. Downloading a new package through the broken helper cannot
replace that helper before it runs. Games, saves, settings and artwork are retained.
The AppImage installer does not use this DEB staging path.

For a reproducible real package transaction, use `tests/signed_deb_integration.py`
with the official v0.5.13 signed manifest, signature and DEB. It runs the current
production-key verification and APT/dpkg in a disposable user/mount/network-isolated
chroot and checks installed files, rejection and cleanup. Its dependency records
are synthetic so it does not certify GUI startup, desktop libraries or the pkexec
authorization prompt. The host package database is not exposed to the chroot.

## Save, close and coordinate before installation

Starting with v0.5.11, the built-in updater requires an explicit prompt confirmation:
save progress, close games/emulators and other apps opened through Matrix, and wait
for installers/updates to finish. Known active launchers and log sinks hold leases
that block installation regardless of the checkbox. Detached launchers and apps
started independently cannot all be observed; the user must confirm they are closed.
No game, installer, process group or arbitrary matching process name is terminated.

A packaged unprivileged `matrix-update-coordinator` verifies the signed request and
package again, takes a per-user update gate, and requests cooperative shutdown of
registered frontend instances. PID plus kernel start time, live leases, process
ancestry and inherited instance markers establish ownership; stale PID records do
not authorize shutdown. The coordinator waits for actual exit, including verified
AppImage runtime ancestors and owned WebKit helpers, before invoking either existing
installer. It is copied outside the old AppImage mount before any frontend exits.

The shutdown scope is Matrix's registered windows, verified descendants/runtime
processes and positively identified instance-token helpers. A known Matrix identity
whose exit cannot be verified still blocks installation. Unrelated protected apps,
unreadable environments and ordinary background-process churn do not block updates.
An arbitrary hidden process that has already detached and cleared its Matrix token
cannot be identified; this is why the save/close confirmation remains required.
Legacy/unregistered Matrix copies still need manual closure. Affected v0.5.11 and
v0.5.12 clients may require this fix to be installed manually once, since their old
coordinator refuses updates before new code can be installed. Versions through
v0.5.10 also require manual upgrade because they lack the lifecycle guards.

The DEB post-install script provisions a persistent root-owned
`/var/lib/matrix-play/update.lock`, outside the replaceable package payload. System
frontends and log helpers require its shared lease; portable copies honor it when
present. The privileged helper rejects non-root calls before taking the exclusive
lease and never provisions the gate itself. It holds the gate independently through
APT completion, so a coordinator failure cannot reopen Matrix during installation.
A missing/unsafe gate requires package repair or reinstallation, not weakened checks.

AppImage replacement also has a cross-process directory lock, target-change checks,
a same-content no-op and transactional rollback. Explicit unlock releases a completed
transaction even when an unrelated fork temporarily inherited a descriptor. Hard-link
preservation has a bounded synced-copy fallback for filesystems without hard links.
Failed recovery retains identifiable recovery images rather than overwriting unrelated
changes. Repeated installation keeps the original `.previous` rollback bytes.

After the user selects Agree and update, the coordinator closes Matrix, installs
the verified update, and leaves Matrix closed on success. The user can reopen it
when ready. On a recoverable failure it attempts to reopen Matrix once to show the
error. The outcome, including recovery-reopen failures, is preserved for next startup.
An installer is never killed on timeout or cancellation; package-manager failures
may require normal system-package repair. The native regression test uses a test-only
replacement marker and harmless processes, not an actual privileged installation.

`python3 scripts/release.py X.Y.Z` requires a clean main checkout and committed, nonempty release notes under exactly one complete `## X.Y.Z` heading. Both the version helper and signer reject prefix matches such as `0.6.10` or `0.6.1-rc.1` when releasing `0.6.1`. It bumps all application versions, commits and pushes. The Actions build runs tests before the publish job receives the signing secret. Publishing uses only the tested main commit, creates a draft with all assets, then publishes it. Already-published versions are skipped. A failed build or missing signing secret leaves the new version unpublished; fix the issue and rerun or push the correction. Never manually replace signed release assets under an existing version.

The combined DEB/AppImage release and build commands require a numeric `X.Y.Z` version. Tauri CLI 2.12.4 writes the application version directly into the Debian control file and has no DEB-specific version override. Debian sorts `0.6.1-rc.10` above `0.6.1`, so accepting that suffix would break the upgrade to stable. Use GitHub's testing/prerelease status with a numeric version instead. The signer also rejects version suffixes, and the release helper checks both SemVer and actual `dpkg` upgrade ordering before modifying files. Full SemVer comparison remains available for non-DEB consumers such as AppImage update selection; this restriction belongs to the combined DEB release workflow.

Validation must be reported precisely: updater core tests exercise signature/identity checks, corrupt and oversized downloads, cancellation, caching and staged AppImage replacement using synthetic fixtures. UI tests simulate update responses. Real desktop tests check the registered IPC and rendered assets. A separate public-release smoke check can exercise the real HTTPS signature/download path. Those tests do not execute a privileged system package upgrade or certify emulator/game compatibility.


## Secure DEB helper regression checks

Run `cargo test --locked --manifest-path update-core/Cargo.toml` for the privileged protocol tests: forged/oversized metadata, signed downgrade rejection, modified/truncated/oversized package streams, source mutation after staging, and unsafe staging directory rejection. These tests use synthetic packages and a test-only signing key and never invoke APT or pkexec. `cargo build --locked --release --manifest-path src-tauri/Cargo.toml --no-default-features --bin matrix-install-update --bin matrix-update-coordinator` builds the helpers; Tauri development/build commands run this automatically, and the build script/CI prepare it before desktop tests. Direct desktop Cargo commands also need this preparation because Tauri validates the coordinator resource during compilation. Run `python3 tests/test_deb_package.py path/to/package.deb` to confirm the built DEB requires `pkexec` and contains a root-owned, non-writable-by-ordinary-users `/usr/lib/matrix-play/matrix-install-update`. A real authorization/package installation remains a separate, explicitly authorized integration test.
