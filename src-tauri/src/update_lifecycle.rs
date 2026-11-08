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
    // Fixed account locations keep all instances together regardless of HOME,
    // TMPDIR or XDG overrides. Never reserve a public /tmp name: another user
    // can squat on it and prevent the victim from starting the application.
    let uid = unsafe { libc::geteuid() };
    lifecycle_directory(&account_home(uid)?, uid)
}

fn account_home(uid: u32) -> Result<PathBuf, String> {
    use std::{ffi::CStr, os::unix::ffi::OsStrExt};
    // getpwuid_r avoids both environment-dependent homes and libc's shared
    // passwd buffer. Retry ERANGE, but never allocate without a bound.
    let mut size = 4096;
    loop {
        let mut buffer = vec![0u8; size];
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE && size < 1024 * 1024 {
            size *= 2;
            continue;
        }
        if status != 0 || result.is_null() {
            return Err("Cannot locate the account home for Matrix lifecycle coordination".into());
        }
        let entry = unsafe { entry.assume_init() };
        if entry.pw_dir.is_null() {
            return Err("The account has no home directory".into());
        }
        let bytes = unsafe { CStr::from_ptr(entry.pw_dir) }.to_bytes();
        let home = PathBuf::from(std::ffi::OsStr::from_bytes(bytes));
        if !home.is_absolute() {
            return Err("The account home must be an absolute directory".into());
        }
        return Ok(home);
    }
}

fn protected_parent(path: &Path, uid: u32) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Matrix lifecycle parent must be an absolute directory".into());
    }
    for (index, ancestor) in path.ancestors().enumerate() {
        let meta = fs::symlink_metadata(ancestor).map_err(err)?;
        // Reject symlinks as well as writable ancestors. A root-owned sticky
        // directory is safe above an already validated owned child (e.g. a
        // temporary home used by tests); it is never an acceptable parent itself.
        let protected = meta.mode() & 0o022 == 0
            || (index > 0 && meta.uid() == 0 && meta.mode() & 0o1000 != 0);
        if !meta.is_dir()
            || (meta.uid() != 0 && meta.uid() != uid)
            || !protected
            || (index == 0 && meta.uid() != uid)
        {
            return Err(format!(
                "Unsafe Matrix lifecycle parent {} (owner {}, mode {:04o}, expected user {}); startup stopped without modifying it",
                ancestor.display(),
                meta.uid(),
                meta.mode() & 0o7777,
                uid,
            ));
        }
    }
    Ok(())
}

fn lifecycle_directory(home: &Path, uid: u32) -> Result<PathBuf, String> {
    // Do not switch to /run/user when a graphical login appears: a process
    // started before that login must share the same gate with later instances.
    if !home.is_absolute() {
        return Err("The account home must be an absolute directory".into());
    }
    // NSS is the trusted source of the home, and systems such as Silverblue
    // intentionally symlink /home to /var/home. Validate its resolved location
    // and every ancestor; never resolve the private lifecycle entry itself.
    let home = home.canonicalize().map_err(err)?;
    protected_parent(&home, uid)?;
    let root = home.join(".matrix-play-lifecycle");
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
        Self::observe(pid).map(|(id, _)| id)
    }
    fn observe(pid: u32) -> Result<(Self, bool), String> {
        Self::parse(
            pid,
            &fs::read_to_string(format!("/proc/{pid}/stat")).map_err(err)?,
        )
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
    // A reused PID must not turn an unrelated process into a Matrix ancestor.
    let mut result: Vec<_> = parents
        .iter()
        .filter(|parent| {
            Identity::observe(parent.pid)
                .is_ok_and(|(current, running)| current == **parent && running)
        })
        .cloned()
        .collect();
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
        if let Ok((identity, true)) = Identity::parse(pid, &text) {
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
        let mut roots: Vec<_> = instances.iter().map(|p| p.identity.clone()).collect();
        roots.extend(owned_runtimes(&instances, appimage)?);
        let mut processes = roots.clone();
        for identity in descendants(&roots)? {
            if !processes.contains(&identity) {
                processes.push(identity);
            }
        }
        let tokens: Vec<_> = instances
            .iter()
            .filter_map(|p| p.path.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        // Only positive Matrix ownership is relevant. An unrelated sandbox or
        // protected user service is not required to expose its environment.
        let scanned = tagged_processes(&tokens, &processes)?;
        if let Some(unverified) = scanned.unverified.first() {
            return Err(unverified.message.clone());
        }
        scanned.remember(&mut processes, &mut Vec::new());
        Ok(Self {
            _gate: gate,
            _activity: activity,
            instances,
            processes,
            token,
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
        let tokens: Vec<_> = self
            .instances
            .iter()
            .filter_map(|p| p.path.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let mut tracked = self.processes.clone();
        let mut unverified = Vec::new();
        loop {
            // WebKit descendants inherit a unique instance token. Rescan after
            // parents exit too, covering late children and orphaned web workers.
            let scanned = tagged_processes(&tokens, &tracked)?;
            scanned.remember(&mut tracked, &mut unverified);
            if tracked.iter().all(|p| !p.alive()) {
                // Check readable Matrix tokens once more after known roots exit.
                // Do not claim global quiescence for arbitrary hidden processes.
                let final_scan = tagged_processes(&tokens, &tracked)?;
                let clean = final_scan.owned.is_empty() && final_scan.unverified.is_empty();
                final_scan.remember(&mut tracked, &mut unverified);
                if clean && tracked.iter().all(|p| !p.alive()) {
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                if let Some(process) = unverified.iter().find(|process| process.identity.alive()) {
                    return Err(process.message.clone());
                }
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
/// Discovery is best effort for unrelated processes. Known Matrix identities
/// are checked separately and still fail closed if their exit cannot be read.
fn user_processes() -> Result<Vec<Identity>, String> {
    let mut result = Vec::new();
    for entry in fs::read_dir("/proc").map_err(err)? {
        let Ok(entry) = entry else { continue };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(uids) = process_uids(pid) else {
            continue;
        };
        let uid = unsafe { libc::geteuid() };
        if uids[0] != uid && uids[1] != uid {
            continue;
        }
        if let Ok((identity, true)) = Identity::observe(pid) {
            result.push(identity);
        }
    }
    Ok(result)
}
struct UnverifiedProcess {
    identity: Identity,
    message: String,
}
impl UnverifiedProcess {
    fn new(identity: Identity, message: String) -> Self {
        #[cfg(test)]
        if std::env::var_os("MATRIX_NATIVE_COORDINATOR_CONFIG").is_some() {
            eprintln!(
                "Matrix lifecycle unverified process: {}",
                tests::process_diagnostic(&identity)
            );
        }
        Self { identity, message }
    }
}
#[derive(Default)]
struct ProcessScan {
    owned: Vec<Identity>,
    unverified: Vec<UnverifiedProcess>,
}
impl ProcessScan {
    fn remember(self, tracked: &mut Vec<Identity>, unverified: &mut Vec<UnverifiedProcess>) {
        for identity in self
            .owned
            .iter()
            .chain(self.unverified.iter().map(|process| &process.identity))
        {
            if !tracked.contains(identity) {
                tracked.push(identity.clone());
            }
        }
        // Once positively identified, a Matrix helper remains tracked even if
        // its environment later becomes private or loses the instance token.
        unverified.extend(self.unverified);
    }
}
fn tagged_processes(tokens: &[String], known: &[Identity]) -> Result<ProcessScan, String> {
    let mut found = ProcessScan::default();
    // Rescan ancestry while a verified parent remains alive. No names, unknown
    // processes or global "new since baseline" guesses establish ownership.
    found.owned = descendants(known)?
        .into_iter()
        .filter(|id| !known.contains(id))
        .collect();
    for identity in user_processes()? {
        if identity.pid == std::process::id()
            || known.contains(&identity)
            || found.owned.contains(&identity)
        {
            continue;
        }
        let Ok(bytes) = process_environment(identity.pid) else {
            continue;
        };
        let inherited_tokens: Vec<_> = bytes
            .split(|b| *b == 0)
            .filter_map(|pair| pair.strip_prefix(b"MATRIX_INSTANCE_TOKEN="))
            .filter(|token| token.starts_with(b"instance-") && token.len() > 9)
            .collect();
        if inherited_tokens.is_empty() || !identity.alive() {
            continue;
        }
        if inherited_tokens
            .iter()
            .any(|value| !tokens.iter().any(|token| token.as_bytes() == *value))
        {
            found.unverified.push(UnverifiedProcess::new(identity.clone(), format!("A Matrix Play helper from an exited or unregistered instance is still running (process {}). Close that Matrix helper normally and retry. No update was installed and nothing was force-stopped.", identity.pid)));
        } else {
            found.owned.push(identity);
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
    /// Native-test-only evidence, captured while an unverified PID still exists.
    /// Do not record argv or environment values (which may contain secrets).
    pub(super) fn process_diagnostic(expected: &Identity) -> String {
        let mut chain = Vec::new();
        let mut pid = expected.pid;
        for _ in 0..4 {
            let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            let parent = stat
                .rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().nth(1))
                .and_then(|value| value.parse::<u32>().ok());
            let observed = Identity::parse(pid, &stat).ok();
            let exe = fs::read_link(format!("/proc/{pid}/exe")).ok().map(|path| {
                path.to_string_lossy()
                    .chars()
                    .take(1024)
                    .collect::<String>()
            });
            let comm = fs::read_to_string(format!("/proc/{pid}/comm"))
                .ok()
                .map(|text| text.trim().chars().take(128).collect::<String>());
            let environment = process_environment(pid).ok();
            let marker_names: Vec<_> = [
                "MATRIX_INSTANCE_TOKEN",
                "MATRIX_NATIVE_COORDINATOR_CONFIG",
                "MATRIX_LIFECYCLE_FIXTURE",
                "MATRIX_NATIVE_TEST_LAUNCH_LOG",
                "APPIMAGE",
                "APPDIR",
            ]
            .into_iter()
            .filter(|name| {
                environment.as_ref().is_some_and(|bytes| {
                    bytes
                        .split(|byte| *byte == 0)
                        .any(|entry| entry.starts_with(format!("{name}=").as_bytes()))
                })
            })
            .collect();
            chain.push(serde_json::json!({"pid":pid,"parent":parent,"observed":observed,
                "exe":exe,"comm":comm,"environment_readable":environment.is_some(),"marker_names":marker_names}));
            let Some(parent) = parent.filter(|parent| *parent != 0 && *parent != pid) else {
                break;
            };
            pid = parent;
        }
        serde_json::json!({"expected":expected,"process_and_ancestors":chain}).to_string()
    }
    /// Only wraps explicitly spawned disposable test children. On a test panic,
    /// release their fixture barriers and reap them instead of contaminating the
    /// following fail-closed process scan. No production process is signalled.
    struct FixtureChild {
        child: Child,
        root: PathBuf,
    }
    impl FixtureChild {
        fn new(child: Child, root: &Path) -> Self {
            Self {
                child,
                root: root.to_owned(),
            }
        }
    }
    impl std::ops::Deref for FixtureChild {
        type Target = Child;
        fn deref(&self) -> &Child {
            &self.child
        }
    }
    impl std::ops::DerefMut for FixtureChild {
        fn deref_mut(&mut self) -> &mut Child {
            &mut self.child
        }
    }
    impl Drop for FixtureChild {
        fn drop(&mut self) {
            // Each fixture directory is private to one test. The marker names
            // are handled exclusively by the harmless child entry points below.
            if let Ok(entries) = fs::read_dir(&self.root) {
                for entry in entries.flatten() {
                    if entry.path().extension().is_some_and(|ext| ext == "ready") {
                        let _ = fs::write(entry.path().with_extension("finish"), b"");
                    }
                }
            }
            for name in ["late.finish", "runtime-app.finish", "runtime-release"] {
                let _ = fs::write(self.root.join(name), b"");
            }
            let until = Instant::now() + Duration::from_secs(2);
            loop {
                match self.child.try_wait() {
                    Ok(Some(_)) | Err(_) => break,
                    Ok(None) if Instant::now() >= until => {
                        // Exact unreaped Child handle from this fixture only.
                        let _ = self.child.kill();
                        let _ = self.child.wait();
                        break;
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
            // A deliberately orphaned late-child fixture must finish before its
            // temporary directory is removed. Never signal a PID from a file.
            for name in ["late.ready"] {
                if let Ok(pid) = fs::read_to_string(self.root.join(name))
                    .unwrap_or_default()
                    .parse::<u32>()
                {
                    if let Ok(identity) = Identity::read(pid) {
                        let until = Instant::now() + Duration::from_secs(2);
                        while identity.alive() && Instant::now() < until {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
            }
        }
    }
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
        if mode == "orphan-tagged" {
            let (_, module) = module_path!().split_once("::").unwrap();
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &format!("{module}::late_child_fixture"),
                    "--nocapture",
                ])
                .env("MATRIX_LIFECYCLE_FIXTURE", &root)
                .env("MATRIX_FIXTURE_MODE", &mode)
                .env("MATRIX_INSTANCE_TOKEN", instance.token())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            wait_file(&root.join("late.ready"));
            fs::write(root.join(format!("{name}.ready")), instance.token()).unwrap();
            // Leave an unlocked registry record and a live tagged child, just
            // as an exited/crashed frontend does. No signal is needed.
            return;
        }
        if mode == "owned-hidden" {
            let (_, module) = module_path!().split_once("::").unwrap();
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &format!("{module}::late_child_fixture"),
                    "--nocapture",
                ])
                .env("MATRIX_LIFECYCLE_FIXTURE", &root)
                .env("MATRIX_FIXTURE_MODE", "late-hidden")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            wait_file(&root.join("late.ready"));
        }
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
                    let mut command = Command::new(std::env::current_exe().unwrap());
                    if mode == "late-tagged" {
                        command.env("MATRIX_INSTANCE_TOKEN", instance.token());
                    }
                    command
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
        let mode = std::env::var("MATRIX_FIXTURE_MODE").unwrap();
        if mode == "late-hidden" {
            assert_eq!(unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) }, 0);
        }
        fs::write(root.join("late.ready"), std::process::id().to_string()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !root.join("late.finish").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn unregistered_fixture(root: &Path, mode: &str) -> FixtureChild {
        let (_, module) = module_path!().split_once("::").unwrap();
        FixtureChild::new(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &format!("{module}::late_child_fixture"),
                    "--nocapture",
                ])
                .env("MATRIX_LIFECYCLE_FIXTURE", root)
                .env("MATRIX_FIXTURE_MODE", mode)
                .env_remove("MATRIX_INSTANCE_TOKEN")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
            root,
        )
    }
    #[test]
    fn unrelated_protected_processes_before_and_during_shutdown_do_not_block() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        for before in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let unrelated_root = tempfile::tempdir().unwrap();
            let mut app = child(root.path(), "app", "normal");
            wait_file(&root.path().join("app.ready"));
            let mut unrelated =
                before.then(|| unregistered_fixture(unrelated_root.path(), "late-hidden"));
            if before {
                wait_file(&unrelated_root.path().join("late.ready"));
            }
            let transaction = Transaction::prepare(root.path(), &fake_exe(root.path())).unwrap();
            if !before {
                unrelated = Some(unregistered_fixture(unrelated_root.path(), "late-hidden"));
            }
            wait_file(&unrelated_root.path().join("late.ready"));
            let mut unrelated = unrelated.unwrap();
            assert!(
                process_environment(unrelated.id()).is_err(),
                "fixture must reproduce protected /proc environment"
            );
            transaction.shutdown(Duration::from_secs(2)).unwrap();
            assert!(app.wait().unwrap().success());
            assert!(
                unrelated.try_wait().unwrap().is_none(),
                "unrelated process was closed"
            );
            fs::write(unrelated_root.path().join("late.finish"), b"").unwrap();
            assert!(unrelated.wait().unwrap().success());
        }
    }
    #[test]
    fn unrelated_new_process_and_churn_do_not_block_shutdown() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let mut app = child(root.path(), "app", "normal");
        wait_file(&root.path().join("app.ready"));
        let transaction = Transaction::prepare(root.path(), &fake_exe(root.path())).unwrap();
        let mut unrelated = unregistered_fixture(other.path(), "late-cleared");
        wait_file(&other.path().join("late.ready"));
        let churn = std::thread::spawn(|| {
            for _ in 0..100 {
                assert!(Command::new("/bin/true").status().unwrap().success());
            }
        });
        transaction.shutdown(Duration::from_secs(2)).unwrap();
        assert!(app.wait().unwrap().success());
        assert!(unrelated.try_wait().unwrap().is_none());
        fs::write(other.path().join("late.finish"), b"").unwrap();
        assert!(unrelated.wait().unwrap().success());
        churn.join().unwrap();
    }
    #[test]
    fn known_protected_child_is_waited_for_without_force_stopping() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let root = tempfile::tempdir().unwrap();
        let mut app = child(root.path(), "app", "owned-hidden");
        wait_file(&root.path().join("app.ready"));
        wait_file(&root.path().join("late.ready"));
        let pid: u32 = fs::read_to_string(root.path().join("late.ready"))
            .unwrap()
            .parse()
            .unwrap();
        let identity = Identity::read(pid).unwrap();
        assert!(process_environment(pid).is_err());
        let transaction = Transaction::prepare(root.path(), &fake_exe(root.path())).unwrap();
        assert!(transaction.processes.contains(&identity));
        assert!(transaction
            .shutdown(Duration::from_millis(150))
            .unwrap_err()
            .contains("did not exit"));
        assert!(identity.alive());
        fs::write(root.path().join("late.finish"), b"").unwrap();
        transaction.shutdown(Duration::from_secs(2)).unwrap();
        assert!(app.wait().unwrap().success());
    }
    #[test]
    fn reused_pid_is_not_a_matrix_ancestor() {
        let mut wrong = Identity::read(std::process::id()).unwrap();
        wrong.start += 1;
        assert!(descendants(&[wrong]).unwrap().is_empty());
    }
    #[test]
    fn newly_observed_persistent_zombie_does_not_prevent_eventual_quiescence() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = tempfile::tempdir().unwrap();
        let transaction = Transaction::prepare(root.path(), &fake_exe(root.path())).unwrap();
        let mut child = Command::new("/bin/true").spawn().unwrap();
        let identity = Identity::read(child.id()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while identity.alive() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(Path::new(&format!("/proc/{}", identity.pid)).exists());
        let result = transaction.shutdown(Duration::from_secs(1));
        assert!(child.wait().unwrap().success());
        assert!(
            result.is_ok(),
            "stable zombie kept invalidating the scan: {result:?}"
        );
    }
    #[test]
    fn test_process_diagnostics_are_bounded_and_do_not_include_environment_values() {
        let root = tempfile::tempdir().unwrap();
        let ready = root.path().join("ready");
        let mut process = Command::new("/bin/sh")
            .args([
                "-c",
                "printf ready > \"$1\"; read line",
                "matrix-diagnostic-fixture",
            ])
            .arg(&ready)
            .env(
                "MATRIX_LIFECYCLE_FIXTURE",
                "synthetic-secret-must-not-be-logged",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        wait_file(&ready);
        let identity = Identity::read(process.id()).unwrap();
        let executable = fs::read_link(format!("/proc/{}/exe", process.id())).unwrap();
        let diagnostic = process_diagnostic(&identity);
        process
            .stdin
            .take()
            .unwrap()
            .write_all(b"finish\n")
            .unwrap();
        assert!(process.wait().unwrap().success());
        assert!(diagnostic.len() < 16384);
        assert!(!diagnostic.contains("synthetic-secret-must-not-be-logged"));
        let diagnostic: serde_json::Value = serde_json::from_str(&diagnostic).unwrap();
        assert_eq!(diagnostic["expected"]["pid"], identity.pid);
        assert_eq!(
            diagnostic["process_and_ancestors"][0]["exe"]
                .as_str()
                .unwrap(),
            executable.to_string_lossy()
        );
        assert!(diagnostic["process_and_ancestors"][0]["marker_names"]
            .as_array()
            .unwrap()
            .iter()
            .any(|name| name == "MATRIX_LIFECYCLE_FIXTURE"));
    }
    #[test]
    fn prior_instance_tagged_orphan_never_allows_replacement() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = tempfile::tempdir().unwrap();
        let mut prior = child(root.path(), "prior", "orphan-tagged");
        wait_file(&root.path().join("prior.ready"));
        assert!(prior.wait().unwrap().success());
        let token = fs::read_to_string(root.path().join("prior.ready")).unwrap();
        assert!(root.path().join(&token).exists());
        let pid: u32 = fs::read_to_string(root.path().join("late.ready"))
            .unwrap()
            .parse()
            .unwrap();
        let orphan = Identity::read(pid).unwrap();
        assert!(process_environment(pid)
            .unwrap()
            .split(|b| *b == 0)
            .any(|pair| pair == format!("MATRIX_INSTANCE_TOKEN={token}").as_bytes()));
        let mut current = child(root.path(), "current", "normal");
        wait_file(&root.path().join("current.ready"));
        let result = Transaction::prepare(root.path(), &fake_exe(root.path()))
            .and_then(|transaction| transaction.shutdown(Duration::from_secs(2)));
        assert!(orphan.alive(), "the old child must not be force-stopped");
        assert!(
            result.is_err(),
            "replacement was allowed while a readable prior-instance child remained alive"
        );
        assert!(result.unwrap_err().contains("unregistered instance"));
        assert!(!root.path().join(&token).exists());
        assert!(Transaction::prepare(root.path(), &fake_exe(root.path()))
            .err()
            .unwrap()
            .contains("unregistered instance"));
        assert!(current.try_wait().unwrap().is_none());
        assert!(!root.path().join("current.blocked").exists());
        assert!(gate(root.path(), false).is_ok());
        fs::write(root.path().join("current.finish"), b"").unwrap();
        assert!(current.wait().unwrap().success());
        fs::write(root.path().join("late.finish"), b"").unwrap();
    }
    #[test]
    fn current_instance_tagged_orphan_is_waited_for_without_force_stopping() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = tempfile::tempdir().unwrap();
        let mut app = child(root.path(), "current", "late-tagged");
        wait_file(&root.path().join("current.ready"));
        let transaction = Transaction::prepare(root.path(), &fake_exe(root.path())).unwrap();
        let error = transaction
            .shutdown(Duration::from_millis(150))
            .unwrap_err();
        assert!(error.contains("did not exit"), "{error}");
        assert!(app.wait().unwrap().success());
        let pid: u32 = fs::read_to_string(root.path().join("late.ready"))
            .unwrap()
            .parse()
            .unwrap();
        let orphan = Identity::read(pid).unwrap();
        assert!(orphan.alive());
        // The parent has definitely gone; recognition of its token, rather than
        // the ancestry snapshot, must keep the transaction waiting for this child.
        assert!(transaction
            .shutdown(Duration::from_millis(100))
            .unwrap_err()
            .contains("did not exit"));
        fs::write(root.path().join("late.finish"), b"").unwrap();
        transaction.shutdown(Duration::from_secs(2)).unwrap();
        assert!(!orphan.alive());
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
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("MatrixFixture.AppImage");
        fs::copy(std::env::current_exe().unwrap(), &image).unwrap();
        let (_, module) = module_path!().split_once("::").unwrap();
        let runtime = Command::new(&image)
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
        let mut runtime = FixtureChild::new(runtime, root.path());
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
    fn child(root: &Path, name: &str, mode: &str) -> FixtureChild {
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &fixture_name(), "--nocapture"])
            .env("MATRIX_LIFECYCLE_FIXTURE", root)
            .env("MATRIX_FIXTURE_NAME", name)
            .env("MATRIX_FIXTURE_MODE", mode)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        FixtureChild::new(child, root)
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
    fn fixture_panic_releases_owned_children_before_the_next_test() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = tempfile::tempdir().unwrap();
        let captured = std::sync::Mutex::new(None);
        let result = std::panic::catch_unwind(|| {
            let app = child(root.path(), "panic-cleanup", "stubborn");
            wait_file(&root.path().join("panic-cleanup.ready"));
            *captured.lock().unwrap() = Some(Identity::read(app.id()).unwrap());
            panic!("intentional fixture cleanup regression");
        });
        assert!(result.is_err());
        assert!(!captured.into_inner().unwrap().unwrap().alive());
        assert!(instances(root.path()).unwrap().is_empty());
    }

    #[test]
    fn two_instances_exit_before_replacement_and_gate_blocks_new_startups() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = tempfile::tempdir().unwrap();
        for name in ["legacy", "matrix-frontend"] {
            let exe = root.path().join(name);
            fs::copy("/bin/sleep", &exe).unwrap();
            let mut sleeper =
                FixtureChild::new(Command::new(&exe).arg("0.4").spawn().unwrap(), root.path());
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
    fn lifecycle_fixture() -> tempfile::TempDir {
        use std::os::unix::fs::PermissionsExt;
        // TempDir defaults to 0777 masked by umask. CI can inherit umask 002,
        // making an ordinary tempfile ancestor group-writable. Model a protected
        // account home explicitly instead of weakening the production guard.
        let fixture = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        protected_parent(fixture.path(), unsafe { libc::geteuid() }).unwrap();
        fixture
    }

    fn protected_fixture_directory(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn account_home_does_not_follow_environment_overrides() {
        const EXPECTED: &str = "MATRIX_ACCOUNT_HOME_FIXTURE";
        let uid = unsafe { libc::geteuid() };
        if let Some(expected) = std::env::var_os(EXPECTED) {
            assert_eq!(account_home(uid).unwrap(), PathBuf::from(expected));
            return;
        }
        // Change process-global environment only in a disposable test process.
        let home = account_home(uid).unwrap();
        let (_, module) = module_path!().split_once("::").unwrap();
        let name = format!("{module}::account_home_does_not_follow_environment_overrides");
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name])
            .env(EXPECTED, &home)
            .env("HOME", "/not-the-account-home")
            .env("TMPDIR", "/not-the-account-home")
            .env("XDG_RUNTIME_DIR", "/not-the-account-home")
            .env("XDG_DATA_HOME", "/not-the-account-home")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed;"),
            "Expected exactly one passing child test, got: {stdout}\n{stderr}"
        );
    }

    #[test]
    fn lifecycle_location_is_shared_and_does_not_use_public_tmp_names() {
        let fixture = lifecycle_fixture();
        let home = fixture.path().join("account-home");
        protected_fixture_directory(&home);
        let uid = unsafe { libc::geteuid() };
        let first = lifecycle_directory(&home, uid).unwrap();
        let second = lifecycle_directory(&home, uid).unwrap();
        assert_eq!(first, home.join(".matrix-play-lifecycle"));
        assert_eq!(first, second);
        let held = gate(&first, true).unwrap();
        assert!(gate(&second, false).is_err());
        drop(held);
        assert!(gate(&second, false).is_ok());
    }

    #[test]
    fn lifecycle_requires_a_protected_account_home() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = lifecycle_fixture();
        let home = fixture.path().join("account-home");
        protected_fixture_directory(&home);
        let uid = unsafe { libc::geteuid() };
        // A normal readable home is fine, but other users must not write it.
        fs::set_permissions(&home, fs::Permissions::from_mode(0o755)).unwrap();
        lifecycle_directory(&home, uid).unwrap();
        fs::set_permissions(&home, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(lifecycle_directory(&home, uid).is_err());
        assert_eq!(fs::metadata(&home).unwrap().mode() & 0o777, 0o777);
        assert!(lifecycle_directory(Path::new("relative"), uid).is_err());
        assert!(lifecycle_directory(&fixture.path().join("absent"), uid).is_err());
        fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(protected_parent(&home, uid.wrapping_add(1)).is_err());
    }

    #[test]
    fn lifecycle_resolves_account_home_links_but_rejects_writable_ancestors() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let fixture = lifecycle_fixture();
        let parent = fixture.path().join("parent");
        protected_fixture_directory(&parent);
        let home = parent.join("home");
        protected_fixture_directory(&home);
        let link = fixture.path().join("linked-parent");
        symlink(&parent, &link).unwrap();
        let uid = unsafe { libc::geteuid() };
        let expected = home.join(".matrix-play-lifecycle");
        assert_eq!(
            lifecycle_directory(&link.join("home"), uid).unwrap(),
            expected
        );
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        let direct_link = fixture.path().join("home-link");
        symlink(&home, &direct_link).unwrap();
        assert_eq!(lifecycle_directory(&direct_link, uid).unwrap(), expected);
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(lifecycle_directory(&home, uid).is_err());
        assert!(lifecycle_directory(&direct_link, uid).is_err());
        assert_eq!(fs::metadata(&parent).unwrap().mode() & 0o777, 0o777);
    }

    #[test]
    fn lifecycle_rejects_existing_unsafe_entries_without_mutation() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let fixture = lifecycle_fixture();
        let home = fixture.path().join("home");
        protected_fixture_directory(&home);
        let uid = unsafe { libc::geteuid() };
        protected_parent(&home, uid).unwrap();
        let root = home.join(".matrix-play-lifecycle");
        let target = fixture.path().join("precious");
        fs::write(&target, b"keep").unwrap();
        symlink(&target, &root).unwrap();
        assert!(lifecycle_directory(&home, uid).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        assert!(fs::symlink_metadata(&root).unwrap().file_type().is_symlink());
        fs::remove_file(&root).unwrap();
        protected_fixture_directory(&root);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(lifecycle_directory(&home, uid).is_err());
        assert_eq!(fs::metadata(&root).unwrap().mode() & 0o777, 0o755);
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
