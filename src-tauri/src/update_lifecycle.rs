//! Cooperative, per-user lifecycle coordination. Never signal a PID or process group.
//! OS leases, rather than stale PID files, define live Matrix instances/activity.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const BUSY: &str = "Finish and close games, emulators and other launched apps before updating. Let installers and updates finish first. A Matrix launch or log helper is still active. Nothing will be force-stopped.";
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn directory() -> Result<PathBuf, String> {
    // Do not trust TMPDIR/XDG variables to partition the same user's instances.
    let root = PathBuf::from(format!("/tmp/matrix-play-lifecycle-{}", unsafe {
        libc::geteuid()
    }));
    private_directory(&root)?;
    Ok(root)
}
pub fn private_directory(root: &Path) -> Result<(), String> {
    match fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(err(e)),
    }
    let m = fs::symlink_metadata(root).map_err(err)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(
            "Unsafe Matrix lifecycle folder; update/startup stopped without modifying it".into(),
        );
    }
    Ok(())
}
use std::os::unix::fs::DirBuilderExt;
fn file(path: &Path) -> Result<File, String> {
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(err)?;
    let m = f.metadata().map_err(err)?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || m.nlink() != 1
    {
        return Err("Unsafe Matrix lifecycle file".into());
    }
    Ok(f)
}
fn lock(file: &File, exclusive: bool) -> Result<bool, String> {
    let mode = if exclusive {
        libc::LOCK_EX
    } else {
        libc::LOCK_SH
    };
    if unsafe { libc::flock(file.as_raw_fd(), mode | libc::LOCK_NB) } == 0 {
        return Ok(true);
    }
    let e = std::io::Error::last_os_error();
    if e.kind() == std::io::ErrorKind::WouldBlock {
        Ok(false)
    } else {
        Err(err(e))
    }
}
pub fn gate(root: &Path, exclusive: bool) -> Result<File, String> {
    let f = file(&root.join("update.lock"))?;
    if !lock(&f, exclusive)? {
        return Err("A Matrix Play update is in progress. Wait for it to finish before opening Matrix or launching a game.".into());
    }
    Ok(f)
}
pub fn activity(root: &Path, exclusive: bool) -> Result<File, String> {
    let f = file(&root.join("activity.lock"))?;
    if !lock(&f, exclusive)? {
        return Err(BUSY.into());
    }
    Ok(f)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Identity {
    pub pid: u32,
    pub start: u64,
}
impl Identity {
    fn parse(pid: u32, stat: &str) -> Result<(Self, bool), String> {
        // comm can contain spaces and ')' characters.
        let fields: Vec<_> = stat
            .rsplit_once(')')
            .ok_or("Invalid process identity")?
            .1
            .split_whitespace()
            .collect();
        let identity = Self {
            pid,
            start: fields
                .get(19)
                .ok_or("Missing process start time")?
                .parse()
                .map_err(err)?,
        };
        let running = !matches!(fields.first().copied(), Some("Z" | "X"));
        Ok((identity, running))
    }
    pub fn read(pid: u32) -> Result<Self, String> {
        Self::parse(
            pid,
            &fs::read_to_string(format!("/proc/{pid}/stat")).map_err(err)?,
        )
        .map(|(id, _)| id)
    }
    fn alive_from(&self, stat: std::io::Result<String>) -> bool {
        match stat {
            Ok(stat) => Self::parse(self.pid, &stat)
                .map(|(id, running)| id == *self && running)
                .unwrap_or(true),
            Err(e) => e.kind() != std::io::ErrorKind::NotFound,
        }
    }
    pub fn alive(&self) -> bool {
        // Only disappearance, a changed birth time, or a known zombie/dead state
        // proves exit. Permission errors and malformed live records fail closed.
        self.alive_from(fs::read_to_string(format!("/proc/{}/stat", self.pid)))
    }
}
pub struct Instance {
    _lease: File,
    pub path: PathBuf,
}
impl Instance {
    pub fn register(root: &Path) -> Result<Self, String> {
        let _gate = gate(root, false)?;
        let mut lease = tempfile::Builder::new()
            .prefix("instance-")
            .tempfile_in(root)
            .map_err(err)?;
        lock(lease.as_file(), true)?;
        serde_json::to_writer(&mut lease, &Identity::read(std::process::id())?).map_err(err)?;
        lease.flush().map_err(err)?;
        let (lease, path) = lease.keep().map_err(err)?;
        Ok(Self {
            _lease: lease,
            path,
        })
    }
    pub fn token(&self) -> String {
        self.path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }
    pub fn stop_requested(&self) -> bool {
        let Some(root) = self.path.parent() else {
            return false;
        };
        let Ok(mut gate) = file(&root.join("update.lock")) else {
            return false;
        };
        if !matches!(lock(&gate, false), Ok(false)) {
            return false;
        }
        let mut token = String::new();
        if Read::by_ref(&mut gate)
            .take(256)
            .read_to_string(&mut token)
            .is_err()
            || token.is_empty()
        {
            return false;
        }
        fs::read(self.path.with_extension("stop")).is_ok_and(|bytes| bytes == token.as_bytes())
    }
}
pub struct Launch {
    pub activity: File,
    _gate: File,
}
pub fn begin_launch() -> Result<Launch, String> {
    let root = directory()?;
    let gate = gate(&root, false)?;
    Ok(Launch {
        activity: activity(&root, false)?,
        _gate: gate,
    })
}
#[derive(Debug)]
struct Registered {
    path: PathBuf,
    identity: Identity,
}
fn instances(root: &Path) -> Result<Vec<Registered>, String> {
    let mut result = Vec::new();
    for entry in fs::read_dir(root).map_err(err)? {
        let entry = entry.map_err(err)?;
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("instance-") || entry.path().extension().is_some() {
            continue;
        }
        let mut f = file(&entry.path())?;
        if lock(&f, true)? {
            // Stale/crashed instance. Never infer liveness from a stored PID.
            let _ = fs::remove_file(entry.path().with_extension("stop"));
            fs::remove_file(entry.path()).map_err(err)?;
            continue;
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut f)
            .take(1025)
            .read_to_end(&mut bytes)
            .map_err(err)?;
        if bytes.len() > 1024 {
            return Err("Invalid live Matrix instance record".into());
        }
        let identity: Identity = serde_json::from_slice(&bytes).map_err(err)?;
        if !identity.alive() {
            return Err("Matrix process identity changed; close other instances and retry".into());
        }
        let uids = process_uids(identity.pid)?;
        if uids[1] != unsafe { libc::geteuid() } {
            return Err("Matrix instance belongs to another user".into());
        }
        result.push(Registered {
            path: entry.path(),
            identity,
        });
    }
    Ok(result)
}
// Snapshot all children (including WebKit) before cooperative shutdown. We only
// wait for these identities; we never kill them. The worker subtree is excluded.
fn descendants(parents: &[Identity]) -> Result<Vec<Identity>, String> {
    let mut result = parents.to_vec();
    let mut candidates = Vec::new();
    for entry in fs::read_dir("/proc").map_err(err)? {
        let entry = entry.map_err(err)?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == std::process::id() {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((_, rest)) = text.rsplit_once(')') else {
            continue;
        };
        let Some(ppid) = rest
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if let Ok(identity) = Identity::read(pid) {
            candidates.push((identity, ppid));
        }
    }
    loop {
        let before = result.len();
        for (id, ppid) in &candidates {
            if !result.contains(id) && result.iter().any(|p| p.pid == *ppid) {
                result.push(id.clone());
            }
        }
        if result.len() == before {
            return Ok(result);
        }
    }
}
/// Hold both guards until installation (including its privileged child) returns.
pub struct Transaction {
    _gate: File,
    _activity: File,
    instances: Vec<Registered>,
    processes: Vec<Identity>,
    token: String,
    baseline: Vec<Identity>,
}
impl Transaction {
    pub fn prepare(root: &Path, frontend_exe: &Path) -> Result<Self, String> {
        Self::prepare_with_runtime(root, frontend_exe, None)
    }
    pub fn prepare_with_runtime(
        root: &Path,
        frontend_exe: &Path,
        appimage: Option<&Path>,
    ) -> Result<Self, String> {
        // Also respect an installer started after this frontend was opened.
        drop(system_startup_guard(false)?);
        let mut gate = gate(root, true)?;
        let token = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(err)?
                .as_nanos()
        );
        gate.set_len(0).map_err(err)?;
        gate.write_all(token.as_bytes()).map_err(err)?;
        let activity = activity(root, true)?;
        let instances = instances(root)?;
        // An older copy of this installation cannot cooperate. Never terminate
        // an unregistered process by its name or PID. Ask for manual closure.
        let expected = fs::metadata(frontend_exe).map_err(err)?;
        for entry in fs::read_dir("/proc").map_err(err)? {
            let entry = entry.map_err(err)?;
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            if pid == std::process::id() {
                continue;
            }
            let Ok(m) = fs::metadata(entry.path().join("exe")) else {
                continue;
            };
            let same_inode = m.dev() == expected.dev() && m.ino() == expected.ino();
            let same_user_candidate =
                process_uids(pid).is_ok_and(|uids| {
                    uids[0] == unsafe { libc::geteuid() } || uids[1] == unsafe { libc::geteuid() }
                }) && fs::read_link(entry.path().join("exe")).is_ok_and(|p| {
                    p.file_name().is_some_and(|name| {
                        matches!(
                            name.to_str(),
                            Some("matrix-frontend" | "matrix-frontend (deleted)")
                        )
                    })
                });
            if (same_inode || same_user_candidate)
                && !instances.iter().any(|p| p.identity.pid == pid)
            {
                return Err("An older or unregistered Matrix Play process, or an unverified process using its executable name, is still running. Close it manually before updating; no process was terminated.".into());
            }
        }
        let baseline = user_processes()?;
        let mut roots: Vec<_> = instances.iter().map(|p| p.identity.clone()).collect();
        roots.extend(owned_runtimes(&instances, appimage)?);
        let processes = descendants(&roots)?;
        let tokens: Vec<_> = instances
            .iter()
            .filter_map(|p| p.path.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        // Do not request shutdown if hidden orphan/sandbox processes prevent us
        // from establishing ownership. No signal/name fallback is permitted.
        tagged_processes(&tokens, &processes, &baseline)?;
        Ok(Self {
            _gate: gate,
            _activity: activity,
            instances,
            processes,
            token,
            baseline,
        })
    }
    pub fn shutdown(&self, timeout: Duration) -> Result<(), String> {
        for instance in &self.instances {
            let root = instance.path.parent().ok_or("Missing instance folder")?;
            let mut stop = tempfile::NamedTempFile::new_in(root).map_err(err)?;
            stop.write_all(self.token.as_bytes()).map_err(err)?;
            stop.persist(instance.path.with_extension("stop"))
                .map_err(err)?;
        }
        let deadline = Instant::now() + timeout;
        loop {
            let tokens: Vec<_> = self
                .instances
                .iter()
                .filter_map(|p| p.path.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .collect();
            // WebKit descendants inherit a unique instance token. Rescan after
            // parents exit too, covering late children and orphaned web workers.
            let owned = tagged_processes(&tokens, &self.processes, &self.baseline)?;
            if self
                .processes
                .iter()
                .chain(owned.iter())
                .all(|p| !p.alive())
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("Update was not installed because a Matrix process or one of its child processes did not exit. Close it normally, then try again. Nothing was force-stopped.".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
#[derive(Serialize, Deserialize)]
pub struct Outcome {
    pub success: bool,
    pub message: String,
}
pub fn save_outcome(root: &Path, outcome: &Outcome) -> Result<(), String> {
    let mut temp = tempfile::NamedTempFile::new_in(root).map_err(err)?;
    serde_json::to_writer(&mut temp, outcome).map_err(err)?;
    temp.as_file().sync_all().map_err(err)?;
    temp.persist(root.join("last-update.json")).map_err(err)?;
    Ok(())
}
pub fn outcome() -> Option<Outcome> {
    let root = directory().ok()?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(root.join("last-update.json"))
        .ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 16384 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn environment_path(bytes: &[u8], key: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    bytes
        .split(|b| *b == 0)
        .find_map(|entry| entry.strip_prefix(key))
        .map(|value| PathBuf::from(std::ffi::OsStr::from_bytes(value)))
}
/// AppImage/FUSE/extraction runtimes may be ancestors rather than descendants
/// of the frontend. A matching executable inode, never a name, identifies them.
fn owned_runtimes(
    instances: &[Registered],
    target: Option<&Path>,
) -> Result<Vec<Identity>, String> {
    let mut images = Vec::new();
    if let Some(target) = target {
        images.push(fs::metadata(target).map_err(err)?);
    }
    for instance in instances {
        let pid = instance.identity.pid;
        let environment = process_environment(pid).map_err(|_|format!("Cannot verify the runtime for Matrix process {pid}; close it normally before updating."))?;
        if let (Some(image), Some(appdir)) = (
            environment_path(&environment, b"APPIMAGE="),
            environment_path(&environment, b"APPDIR="),
        ) {
            let exe = fs::read_link(format!("/proc/{pid}/exe")).map_err(err)?;
            let appdir = appdir.canonicalize().map_err(err)?;
            // An inherited AppImage environment from a different app does not
            // make that app's runtime part of Matrix's shutdown transaction.
            if exe.starts_with(&appdir)
                && exe
                    .file_name()
                    .is_some_and(|name| name == "matrix-frontend")
            {
                images.push(fs::metadata(image).map_err(err)?);
            }
        }
    }
    let mut runtimes = Vec::new();
    for identity in user_processes()? {
        if identity.pid == std::process::id() {
            continue;
        }
        let Ok(meta) = fs::metadata(format!("/proc/{}/exe", identity.pid)) else {
            continue;
        };
        if images
            .iter()
            .any(|image| image.is_file() && image.dev() == meta.dev() && image.ino() == meta.ino())
        {
            runtimes.push(identity);
        }
    }
    Ok(runtimes)
}
fn process_environment(pid: u32) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(format!("/proc/{pid}/environ"))?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(std::io::Error::other(
            "Process environment is too large to verify",
        ));
    }
    Ok(bytes)
}
fn process_uids(pid: u32) -> Result<Vec<u32>, String> {
    let mut status = String::new();
    File::open(format!("/proc/{pid}/status"))
        .map_err(err)?
        .take(131073)
        .read_to_string(&mut status)
        .map_err(err)?;
    if status.len() > 131072 {
        return Err("Oversize process status".into());
    }
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .ok_or("Missing process credentials")?;
    let uids: Vec<u32> = line
        .split_whitespace()
        .map(|s| s.parse().map_err(err))
        .collect::<Result<_, _>>()?;
    if uids.len() != 4 {
        return Err("Invalid process credentials".into());
    }
    Ok(uids)
}
fn user_processes() -> Result<Vec<Identity>, String> {
    let mut result = Vec::new();
    for entry in fs::read_dir("/proc").map_err(err)? {
        let entry = entry.map_err(err)?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        // Non-dumpable processes can make /proc/PID root-owned even though their
        // actual credentials remain this user's. Kernel Uid fields are decisive.
        let uids = match process_uids(pid) {
            Ok(uids) => uids,
            Err(_) if !entry.path().exists() => continue,
            Err(_) => return Err(format!("Cannot verify credentials for process {pid}; update stopped without terminating it.")),
        };
        let uid = unsafe { libc::geteuid() };
        if uids[0] != uid && uids[1] != uid {
            continue;
        }
        match Identity::read(pid) {
            Ok(identity) => result.push(identity),
            Err(_) if !entry.path().exists() => continue,
            Err(_) => return Err(format!("Cannot verify process {pid}; no update will be installed while its identity is unavailable.")),
        }
    }
    Ok(result)
}
fn tagged_processes(
    tokens: &[String],
    known: &[Identity],
    baseline: &[Identity],
) -> Result<Vec<Identity>, String> {
    let mut found = Vec::new();
    for identity in user_processes()? {
        if !identity.alive() || known.contains(&identity) {
            continue;
        }
        let pid = identity.pid;
        let bytes = match process_environment(pid) {
            Ok(bytes) => bytes,
            Err(_) if !identity.alive() => continue,
            // A sandboxed child can be born after the ancestry snapshot, then
            // reparented when its frontend exits. Missing visibility never proves
            // it belongs to another application, including pre-existing orphans.
            Err(_) => return Err(format!("Cannot verify whether process {pid} belongs to Matrix Play. No update was installed and no process was force-stopped. Leave unrelated apps running; install manually only after confirming Matrix Play and its helpers are closed.")),
        };
        if bytes.split(|b| *b == 0).any(|pair| {
            tokens
                .iter()
                .any(|token| pair == format!("MATRIX_INSTANCE_TOKEN={token}").as_bytes())
        }) {
            found.push(identity);
        } else if !baseline.contains(&identity) && identity.alive() {
            // A late child may also deliberately clear its environment. A new,
            // unclassified process cannot be treated as a completed shutdown.
            return Err(format!("Process {pid} started during shutdown and its ownership could not be verified. No update was installed and no process was force-stopped. Leave unrelated apps running; install manually only after confirming Matrix Play and its helpers are closed."));
        }
    }
    Ok(found)
}
fn optional_startup_guard(
    path: &Path,
    required: bool,
    acquire: impl FnOnce() -> Result<File, String>,
) -> Result<Option<File>, String> {
    match fs::symlink_metadata(path) {
        Err(e) if !required && e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!(
            "Cannot verify Matrix's system installer guard: {e}"
        )),
        _ => acquire().map(Some),
    }
}
/// AppImages/other copies also honor an existing installer lease. The installed
/// system binary requires the lock to exist; a standalone AppImage does not.
pub fn system_startup_guard(required: bool) -> Result<Option<File>, String> {
    optional_startup_guard(
        Path::new("/var/lib/matrix-play/update.lock"),
        required,
        || system_guard(false),
    )
}
/// Stable root-owned system lock, never replaced by the DEB payload. Its inode
/// persists across versions. All system frontends/log helpers hold shared locks;
/// the privileged installer holds an exclusive lock through APT completion.
pub fn system_guard(exclusive: bool) -> Result<File, String> {
    let root = Path::new("/var/lib/matrix-play");
    if exclusive && unsafe { libc::geteuid() } != 0 {
        return Err("System installation requires Linux authorization".into());
    }
    for dir in [
        Path::new("/"),
        Path::new("/var"),
        Path::new("/var/lib"),
        root,
    ] {
        let meta = fs::symlink_metadata(dir).map_err(|e| {
            format!("Cannot verify the system update guard: {e}. Repair/reinstall Matrix Play.")
        })?;
        if !meta.is_dir() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
            return Err("Unsafe system update lock directory".into());
        }
    }
    let path = root.join("update.lock");
    let guard = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .map_err(|e| {
            format!(
                "Cannot open system update guard: {e}. Repair/reinstall the Matrix Play package."
            )
        })?;
    let meta = guard.metadata().map_err(err)?;
    if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o022 != 0 || meta.nlink() != 1 {
        return Err("Unsafe system update guard".into());
    }
    if !lock(&guard, exclusive)? {
        return Err("A Matrix Play system update or another user's Matrix session is active. Close Matrix normally and retry after it finishes.".into());
    }
    if exclusive {
        // Legacy frontends cannot hold this lock. Check the actual installed
        // executable inode across all users, not a process basename.
        let exe = fs::metadata("/usr/bin/matrix-frontend").map_err(err)?;
        for entry in fs::read_dir("/proc").map_err(err)? {
            let entry = entry.map_err(err)?;
            let Ok(meta) = fs::metadata(entry.path().join("exe")) else {
                continue;
            };
            if meta.dev() == exe.dev() && meta.ino() == exe.ino() {
                return Err("An older Matrix Play process is running. Close all Matrix sessions before installing.".into());
            }
        }
    }
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command, Stdio};
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn fixture_name() -> String {
        let (_, module) = module_path!().split_once("::").unwrap();
        format!("{module}::process_fixture")
    }
    /// Invoked by the native CI runner only. The packaged application has no
    /// such entry point, unsigned install mode, or environment-driven bypass.
    #[test]
    fn native_coordinator_fixture() {
        let Some(config) = std::env::var_os("MATRIX_NATIVE_COORDINATOR_CONFIG") else {
            return;
        };
        let config: serde_json::Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
        let root = directory().unwrap();
        let exe = PathBuf::from(config["binary"].as_str().unwrap());
        let output = PathBuf::from(config["output"].as_str().unwrap());
        let mut expected: Vec<u32> = serde_json::from_value(config["pids"].clone()).unwrap();
        expected.sort();
        let mut actual: Vec<_> = instances(&root)
            .unwrap()
            .iter()
            .map(|i| i.identity.pid)
            .collect();
        actual.sort();
        assert_eq!(
            actual, expected,
            "Refuse to affect any instance outside the disposable native test"
        );
        let appimage = config["appimage"].as_str().map(Path::new);
        let prepared = Transaction::prepare_with_runtime(&root, &exe, appimage);
        if config["busy"].as_bool().unwrap_or(false) {
            assert!(prepared.err().unwrap().contains("Finish and close"));
            fs::write(output, b"blocked-without-shutdown").unwrap();
            return;
        }
        let transaction = prepared.unwrap();
        if let Some(runtimes) = config["runtime_pids"].as_array() {
            for pid in runtimes {
                assert!(
                    transaction
                        .processes
                        .iter()
                        .any(|p| p.pid == pid.as_u64().unwrap() as u32),
                    "AppImage wrapper is not part of exit proof"
                );
            }
        }
        assert!(Instance::register(&root).is_err());
        fs::write(output.with_extension("ready"), b"gated").unwrap();
        let proceed = output.with_extension("proceed");
        let deadline = Instant::now() + Duration::from_secs(15);
        while !proceed.exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        transaction.shutdown(Duration::from_secs(20)).unwrap();
        assert!(transaction.processes.iter().all(|p| !p.alive()));
        // Represents the replacement boundary without touching any executable.
        fs::write(output, b"replacement-allowed-after-actual-exit").unwrap();
        drop(transaction);
        assert!(gate(&root, false).is_ok());
    }
    #[test]
    fn process_fixture() {
        let Some(root) = std::env::var_os("MATRIX_LIFECYCLE_FIXTURE") else {
            return;
        };
        let root = PathBuf::from(root);
        let name = std::env::var("MATRIX_FIXTURE_NAME").unwrap();
        let mode = std::env::var("MATRIX_FIXTURE_MODE").unwrap();
        let instance = match Instance::register(&root) {
            Ok(instance) => instance,
            Err(e) => {
                fs::write(root.join(format!("{name}.blocked")), e).unwrap();
                return;
            }
        };
        let _activity = if mode == "busy" {
            Some(activity(&root, false).unwrap())
        } else {
            None
        };
        fs::write(root.join(format!("{name}.ready")), instance.token()).unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            if root.join(format!("{name}.finish")).exists() {
                return;
            }
            if mode != "stubborn" && instance.stop_requested() {
                if mode.starts_with("late-") {
                    let (_, module) = module_path!().split_once("::").unwrap();
                    Command::new(std::env::current_exe().unwrap())
                        .args([
                            "--exact",
                            &format!("{module}::late_child_fixture"),
                            "--nocapture",
                        ])
                        .env("MATRIX_LIFECYCLE_FIXTURE", &root)
                        .env("MATRIX_FIXTURE_MODE", &mode)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .unwrap();
                    wait_file(&root.join("late.ready"));
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("disposable process fixture timed out");
    }
    #[test]
    fn late_child_fixture() {
        let Some(root) = std::env::var_os("MATRIX_LIFECYCLE_FIXTURE") else {
            return;
        };
        let root = PathBuf::from(root);
        if std::env::var("MATRIX_FIXTURE_MODE").unwrap() == "late-hidden" {
            assert_eq!(unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) }, 0);
        }
        fs::write(root.join("late.ready"), std::process::id().to_string()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !root.join("late.finish").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn late_children_with_hidden_or_cleared_environment_never_allow_replacement() {
        let _serial = SERIAL.lock().unwrap();
        for mode in ["late-hidden", "late-cleared"] {
            let root = tempfile::tempdir().unwrap();
            let mut app = child(root.path(), "late", mode);
            wait_file(&root.path().join("late.ready"));
            // The frontend's ready marker must not be confused with its child.
            fs::remove_file(root.path().join("late.ready")).unwrap();
            let txn = Transaction::prepare(root.path(), &fake_exe(root.path())).unwrap();
            let error = txn.shutdown(Duration::from_secs(2)).unwrap_err();
            assert!(
                error.contains("verify") || error.contains("verified"),
                "{error}"
            );
            wait_file(&root.path().join("late.ready"));
            let pid: u32 = fs::read_to_string(root.path().join("late.ready"))
                .unwrap()
                .parse()
                .unwrap();
            let identity = Identity::read(pid).unwrap();
            assert!(identity.alive());
            fs::write(root.path().join("late.finish"), b"").unwrap();
            assert!(app.wait().unwrap().success());
            let deadline = Instant::now() + Duration::from_secs(2);
            while identity.alive() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    #[test]
    fn runtime_wrapper_fixture() {
        let Some(root) = std::env::var_os("MATRIX_RUNTIME_FIXTURE") else {
            return;
        };
        let root = PathBuf::from(root);
        let mut frontend = child(&root, "runtime-app", "normal");
        assert!(frontend.wait().unwrap().success());
        fs::write(root.join("runtime-cleaning"), b"").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !root.join("runtime-release").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn appimage_runtime_ancestor_must_exit_before_replacement() {
        let _serial = SERIAL.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("MatrixFixture.AppImage");
        fs::copy(std::env::current_exe().unwrap(), &image).unwrap();
        let (_, module) = module_path!().split_once("::").unwrap();
        let mut runtime = Command::new(&image)
            .args([
                "--exact",
                &format!("{module}::runtime_wrapper_fixture"),
                "--nocapture",
            ])
            .env("MATRIX_RUNTIME_FIXTURE", root.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        wait_file(&root.path().join("runtime-app.ready"));
        let transaction =
            Transaction::prepare_with_runtime(root.path(), &fake_exe(root.path()), Some(&image))
                .unwrap();
        assert!(transaction.processes.iter().any(|p| p.pid == runtime.id()));
        let release_root = root.path().to_owned();
        let release = std::thread::spawn(move || {
            wait_file(&release_root.join("runtime-cleaning"));
            std::thread::sleep(Duration::from_millis(150));
            fs::write(release_root.join("runtime-release"), b"").unwrap();
        });
        transaction.shutdown(Duration::from_secs(3)).unwrap();
        assert!(root.path().join("runtime-release").exists());
        assert!(runtime.wait().unwrap().success());
        release.join().unwrap();
    }
    fn child(root: &Path, name: &str, mode: &str) -> Child {
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &fixture_name(), "--nocapture"])
            .env("MATRIX_LIFECYCLE_FIXTURE", root)
            .env("MATRIX_FIXTURE_NAME", name)
            .env("MATRIX_FIXTURE_MODE", mode)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }
    fn wait_file(path: &Path) {
        let until = Instant::now() + Duration::from_secs(5);
        while !path.exists() {
            assert!(Instant::now() < until, "missing {}", path.display());
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn fake_exe(root: &Path) -> PathBuf {
        let p = root.join("unrelated-executable-fixture");
        fs::write(&p, b"not run").unwrap();
        p
    }
    #[test]
    fn two_instances_exit_before_replacement_and_gate_blocks_new_startups() {
        let _serial = SERIAL.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut a = child(root.path(), "a", "normal");
        let mut b = child(root.path(), "b", "normal");
        wait_file(&root.path().join("a.ready"));
        wait_file(&root.path().join("b.ready"));
        let txn = Transaction::prepare(root.path(), &fake_exe(root.path())).unwrap();
        assert!(
            gate(root.path(), true).is_err(),
            "second updater must not overlap"
        );
        assert!(
            activity(root.path(), false).is_err(),
            "new launch must not enter"
        );
        let mut newcomer = child(root.path(), "new", "normal");
        assert!(newcomer.wait().unwrap().success());
        assert!(root.path().join("new.blocked").exists());
        txn.shutdown(Duration::from_secs(3)).unwrap();
        assert!(!Identity::read(a.id()).map(|id| id.alive()).unwrap_or(false));
        assert!(!Identity::read(b.id()).map(|id| id.alive()).unwrap_or(false));
        assert!(a.wait().unwrap().success());
        assert!(b.wait().unwrap().success());
        // A simulated failed install releases both gates without touching data.
        drop(txn);
        assert!(gate(root.path(), false).is_ok());
        assert!(activity(root.path(), false).is_ok());
        assert!(instances(root.path()).unwrap().is_empty());
    }
    #[test]
    fn live_game_lease_blocks_without_closing_frontend_or_unrelated_process() {
        let _serial = SERIAL.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut app = child(root.path(), "game", "busy");
        wait_file(&root.path().join("game.ready"));
        assert!(Transaction::prepare(root.path(), &fake_exe(root.path()))
            .err()
            .unwrap()
            .contains("Finish and close"));
        assert!(app.try_wait().unwrap().is_none());
        assert!(gate(root.path(), false).is_ok());
        fs::write(root.path().join("game.finish"), b"").unwrap();
        assert!(app.wait().unwrap().success());
    }
    #[test]
    fn unresponsive_frontend_aborts_without_force_stopping_it() {
        let _serial = SERIAL.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut app = child(root.path(), "stuck", "stubborn");
        wait_file(&root.path().join("stuck.ready"));
        let txn = Transaction::prepare(root.path(), &fake_exe(root.path())).unwrap();
        assert!(txn
            .shutdown(Duration::from_millis(100))
            .unwrap_err()
            .contains("not installed"));
        assert!(app.try_wait().unwrap().is_none());
        drop(txn);
        fs::write(root.path().join("stuck.finish"), b"").unwrap();
        assert!(app.wait().unwrap().success());
    }
    #[test]
    fn legacy_same_inode_and_separate_same_name_candidates_are_never_signalled() {
        let _serial = SERIAL.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        for name in ["legacy", "matrix-frontend"] {
            let exe = root.path().join(name);
            fs::copy("/bin/sleep", &exe).unwrap();
            let mut sleeper = Command::new(&exe).arg("0.4").spawn().unwrap();
            let expected = if name == "legacy" {
                exe
            } else {
                fake_exe(root.path())
            };
            assert!(Transaction::prepare(root.path(), &expected)
                .err()
                .unwrap()
                .contains("unregistered"));
            assert!(sleeper.try_wait().unwrap().is_none());
            assert!(sleeper.wait().unwrap().success());
        }
    }
    #[test]
    fn stale_records_and_reused_pids_do_not_authorize_shutdown() {
        let root = tempfile::tempdir().unwrap();
        let stale = root.path().join("instance-stale");
        fs::write(&stale, b"malformed stale record").unwrap();
        fs::set_permissions(&stale, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
        assert!(instances(root.path()).unwrap().is_empty());
        assert!(!stale.exists());
        let live = Instance::register(root.path()).unwrap();
        let mut wrong = Identity::read(std::process::id()).unwrap();
        wrong.start += 1;
        fs::write(&live.path, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert!(instances(root.path())
            .unwrap_err()
            .contains("identity changed"));
        assert!(!live.stop_requested());
    }
    #[test]
    fn stale_stop_request_requires_current_locked_transaction_token() {
        let root = tempfile::tempdir().unwrap();
        let app = Instance::register(root.path()).unwrap();
        fs::write(app.path.with_extension("stop"), "old").unwrap();
        assert!(!app.stop_requested());
        let mut held = gate(root.path(), true).unwrap();
        held.write_all(b"new").unwrap();
        assert!(!app.stop_requested());
        fs::write(app.path.with_extension("stop"), "new").unwrap();
        assert!(app.stop_requested());
        drop(held);
        assert!(!app.stop_requested());
    }
    #[test]
    fn installed_and_portable_startup_honor_existing_installer_gate() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("installer.lock");
        let installer = file(&path).unwrap();
        assert!(lock(&installer, true).unwrap());
        let acquire = || {
            let f = file(&path)?;
            if !lock(&f, false)? {
                return Err("installer active".into());
            }
            Ok(f)
        };
        for required in [false, true] {
            assert!(optional_startup_guard(&path, required, acquire)
                .unwrap_err()
                .contains("installer active"));
        }
        drop(installer);
        assert!(optional_startup_guard(&path, false, acquire)
            .unwrap()
            .is_some());
        let absent = root.path().join("not-installed");
        assert!(optional_startup_guard(&absent, false, || panic!(
            "portable builds may run without a DEB installation"
        ))
        .unwrap()
        .is_none());
        assert!(optional_startup_guard(&absent, true, || Err(
            "repair missing installed guard".into()
        ))
        .is_err());
    }

    #[test]
    fn process_identity_fails_closed_and_zombies_count_as_exited() {
        let id = Identity::read(std::process::id()).unwrap();
        assert!(id.alive());
        assert!(id.alive_from(Err(std::io::ErrorKind::PermissionDenied.into())));
        assert!(id.alive_from(Ok("truncated".into())));
        assert!(!id.alive_from(Err(std::io::ErrorKind::NotFound.into())));
        let mut reused = id.clone();
        reused.start += 1;
        assert!(!reused.alive());
        let mut child = Command::new("/bin/true").spawn().unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        let identity = Identity::read(child.id()).unwrap();
        while identity.alive() {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(10));
        }
        // The child has not been reaped yet; a zombie is no longer executing.
        assert!(child.wait().unwrap().success());
    }
    #[test]
    fn symlink_and_public_directory_are_rejected_without_touching_target() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("precious");
        fs::write(&target, b"keep").unwrap();
        std::os::unix::fs::symlink(&target, root.path().join("update.lock")).unwrap();
        assert!(gate(root.path(), true).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        let exposed = root.path().join("exposed");
        fs::create_dir(&exposed).unwrap();
        fs::set_permissions(
            &exposed,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
        assert!(private_directory(&exposed).is_err());
    }
}
