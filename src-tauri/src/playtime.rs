//! An independent, bounded-checkpoint session clock. The GUI may close without
//! stopping the emulator or its clock. Never infer sessions from process names.
use super::*;
use std::{
    fs::File,
    os::fd::{AsRawFd, FromRawFd},
    os::unix::{fs::OpenOptionsExt, process::CommandExt},
    process::Stdio,
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
struct Session {
    key: String,
    identity: update_lifecycle::Identity,
    lease_fd: i32,
    activity_fd: Option<i32>,
}

pub(crate) fn reserve(id: &str, uid: &str) -> Result<File, String> {
    let dir = config_file()?
        .parent()
        .ok_or("Invalid settings path")?
        .join("playtime-locks");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let lease = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(dir.join(cover_key(id, uid)))
        .map_err(|e| e.to_string())?;
    // A just-exited game's helper may still be flushing its final checkpoint.
    for _ in 0..20 {
        if lease.try_lock().is_ok() {
            return Ok(lease);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("This game already has a tracked emulator session. Close that emulator before launching it again.".into())
}

pub(crate) fn start(
    pid: u32,
    id: &str,
    uid: &str,
    lease: File,
    activity: Option<&File>,
) -> Result<(), String> {
    let identity = match update_lifecycle::Identity::read(pid) {
        Ok(i) => i,
        Err(_) => return Ok(()),
    }; // Already exited.
       // Reserve high descriptors before spawning. Inherit those exact slots so
       // pre_exec never overwrites Rust's own exec-error pipe or another handoff.
    fn duplicate(file: &File) -> Result<File, String> {
        let fd = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
        if fd < 0 {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }
    let lock = duplicate(&lease)?;
    let activity = activity.map(duplicate).transpose()?;
    let lock_fd = lock.as_raw_fd();
    let activity_fd = activity.as_ref().map(AsRawFd::as_raw_fd);
    let session = Session {
        key: format!("{id}|{uid}"),
        identity,
        lease_fd: lock_fd,
        activity_fd,
    };
    #[cfg(not(test))]
    let mut helper = Command::new("/proc/self/exe");
    #[cfg(test)]
    let mut helper = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    #[cfg(not(test))]
    helper.arg("--matrix-playtime");
    #[cfg(test)]
    helper
        .args([
            "--exact",
            &format!(
                "{}::tests::tracker_fixture",
                module_path!().split_once("::").unwrap().1
            ),
            "--nocapture",
        ])
        .env("MATRIX_TEST_PLAYTIME", "1");
    helper
        .env(
            "MATRIX_PLAYTIME_SESSION",
            serde_json::to_string(&session).map_err(|e| e.to_string())?,
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    unsafe {
        helper.pre_exec(move || {
            for fd in [Some(lock_fd), activity_fd].into_iter().flatten() {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = helper.spawn().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn group_alive(identity: &update_lifecycle::Identity) -> bool {
    // A reused leader PID belongs to a new session, never to this game's clock.
    if update_lifecycle::Identity::read(identity.pid).is_ok_and(|i| i != *identity) {
        return false;
    }
    // Normal emulators stay in their original process. Only inspect the wider
    // group when a wrapper exits and leaves children running.
    if identity.alive() {
        return true;
    }
    let Ok(entries) = fs::read_dir("/proc") else {
        return identity.alive();
    };
    entries.flatten().any(|e| {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
            return false;
        };
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        let Some((_, fields)) = stat.rsplit_once(')') else {
            return false;
        };
        let fields: Vec<_> = fields.split_whitespace().collect();
        !matches!(fields.first(), Some(&"Z" | &"X"))
            && fields.get(2).and_then(|v| v.parse::<u32>().ok()) == Some(identity.pid)
    })
}

fn checkpoint(path: &Path, key: &str, millis: u64) -> Result<(), String> {
    if millis == 0 {
        return Ok(());
    }
    settings::update(path, |s| {
        let total = s.playtime.entry(key.into()).or_default();
        *total = total.saturating_add(millis);
        Ok(())
    })
}

pub(crate) fn run() -> Result<(), String> {
    let raw = std::env::var("MATRIX_PLAYTIME_SESSION").map_err(|e| e.to_string())?;
    let session: Session = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if session.key.len() > 8192
        || session
            .key
            .split_once('|')
            .is_none_or(|(id, uid)| spec(id).is_err() || !uid.starts_with('/'))
    {
        return Err("Invalid session key".into());
    }
    fn inherited(fd: i32) -> Result<File, String> {
        if fd < 10 || unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            return Err("Missing tracker descriptor".into());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }
    let _lease = inherited(session.lease_fd)?;
    let _activity = session.activity_fd.map(inherited).transpose()?;
    let path = config_file()?;
    let beginning = Instant::now();
    let mut last = beginning;
    let mut pending = 0u64;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let now = Instant::now();
        let delta = now.duration_since(last);
        last = now;
        // CLOCK_MONOTONIC excludes system suspend on Linux. A stopped tracker
        // also must not manufacture hours after a long scheduling gap.
        if delta <= Duration::from_secs(5) {
            pending = pending.saturating_add(delta.as_millis() as u64);
        }
        let alive = group_alive(&session.identity);
        if !alive && beginning.elapsed() < Duration::from_secs(1) {
            return Ok(());
        }
        if pending >= 5000 || !alive {
            if checkpoint(&path, &session.key, pending).is_ok() {
                pending = 0;
            }
            if !alive {
                return if pending == 0 {
                    Ok(())
                } else {
                    Err("Could not save final playtime checkpoint".into())
                };
            }
        }
    }
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn get_playtime() -> Result<BTreeMap<String, u64>, String> {
    Ok(load_settings()?.playtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracker_fixture() {
        if std::env::var_os("MATRIX_TEST_PLAYTIME").is_some() {
            std::process::exit(if run().is_ok() { 0 } else { 1 });
        }
    }
    #[test]
    fn checkpoints_accumulate_without_overwriting_settings() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        settings::update(&path, |s| {
            s.artwork.style = "3d".into();
            Ok(())
        })
        .unwrap();
        checkpoint(&path, "game-a", 3000).unwrap();
        checkpoint(&path, "game-a", 7000).unwrap();
        checkpoint(&path, "game-b", 42).unwrap();
        let s = settings::load(&path).unwrap();
        assert_eq!(s.playtime["game-a"], 10000);
        assert_eq!(s.playtime["game-b"], 42);
        assert_eq!(s.artwork.style, "3d");
    }
    #[test]
    fn a_background_child_keeps_the_session_alive() {
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "sleep 2 &"]).process_group(0);
        let mut child = c.spawn().unwrap();
        let identity = update_lifecycle::Identity::read(child.id()).unwrap();
        assert!(group_alive(&identity));
        child.wait().unwrap();
        assert!(
            group_alive(&identity),
            "a wrapper's exit must not stop its child clock"
        );
        for _ in 0..60 {
            if !group_alive(&identity) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!group_alive(&identity));
    }
    #[test]
    fn detached_owner_fixture() {
        if std::env::var_os("MATRIX_TEST_CLOCK_OWNER").is_none() {
            return;
        }
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 2; touch \"$1\"; sleep 1", "fixture"])
            .arg(home().unwrap().join("finished-marker"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let child = command.spawn().unwrap();
        start(
            child.id(),
            "gb",
            "/synthetic/Pokemon Red.gb",
            reserve("gb", "/synthetic/Pokemon Red.gb").unwrap(),
            None,
        )
        .unwrap();
        std::process::exit(0);
    }
    #[test]
    fn tracks_after_gui_owner_exits_and_persists_across_restart() {
        let root = tempfile::tempdir().unwrap();
        let fixture = format!(
            "{}::detached_owner_fixture",
            module_path!().split_once("::").unwrap().1
        );
        assert!(Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &fixture])
            .env("MATRIX_TEST_CLOCK_OWNER", "1")
            .env("HOME", root.path())
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .status()
            .unwrap()
            .success());
        let path = root.path().join("config/matrix/frontend.json");
        for _ in 0..100 {
            if path.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let s = settings::load(&path).unwrap();
        let total = s.playtime["gb|/synthetic/Pokemon Red.gb"];
        assert!((2500..5500).contains(&total), "unexpected clock {total}");
        assert!(root.path().join("finished-marker").exists());
        std::thread::sleep(Duration::from_millis(700));
        assert_eq!(
            settings::load(&path).unwrap().playtime["gb|/synthetic/Pokemon Red.gb"],
            total
        );
    }
}
