use super::*;
use std::{
    io::Write,
    os::unix::{fs::FileExt, io::OwnedFd, process::CommandExt},
    process::Stdio,
    sync::{mpsc, Arc, Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Info {
    pub console_id: String,
    pub phase: String,
    pub message: String,
    pub log: String,
    pub log_path: String,
}
struct Record {
    info: Info,
    file: fs::File,
    written: usize,
}
static CURRENT: OnceLock<Mutex<BTreeMap<String, Arc<Mutex<Record>>>>> = OnceLock::new();
fn records() -> &'static Mutex<BTreeMap<String, Arc<Mutex<Record>>>> {
    CURRENT.get_or_init(|| Mutex::new(BTreeMap::new()))
}
fn append(record: &Arc<Mutex<Record>>, text: &str) {
    if let Ok(mut r) = record.lock() {
        let n = text.len().min(65536usize.saturating_sub(r.written));
        if n > 0 {
            let _ = r.file.write_all(&text.as_bytes()[..n]);
            r.written += n;
        }
        r.info.log.push_str(text);
        if r.info.log.len() > 8192 {
            let mut n = r.info.log.len() - 8192;
            while !r.info.log.is_char_boundary(n) {
                n += 1;
            }
            r.info.log.drain(..n);
        }
    }
}
// A separate copy of this executable drains output even after the GUI exits.
fn capture_output(
    command: &mut Command,
    file: &fs::File,
    activity: Option<&fs::File>,
) -> std::io::Result<mpsc::Receiver<()>> {
    // Re-exec the running inode even if a package update replaced/unlinked its
    // original path. This also prevents accidentally executing a newer binary.
    #[cfg(not(test))]
    let mut helper = Command::new("/proc/self/exe");
    #[cfg(test)]
    let mut helper = Command::new(std::env::current_exe()?);
    #[cfg(not(test))]
    helper.arg("--matrix-launch-log-sink");
    #[cfg(test)]
    helper
        .args(["--exact", &launch_log::fixture_name(), "--nocapture"])
        .env("MATRIX_TEST_LOG_SINK", "1");
    if let Some(activity) = activity {
        use std::os::fd::AsRawFd;
        let fd = activity.as_raw_fd();
        // Only the owned log sink inherits the activity lease. Never give the
        // game a coordinator descriptor or depend on the launcher staying alive.
        unsafe {
            helper.pre_exec(move || {
                if libc::dup2(fd, 3) < 0 || libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        helper.env("MATRIX_LOG_ACTIVITY_FD", "3");
    }
    let mut sink = helper
        .stdin(Stdio::piped())
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    let output: OwnedFd = sink.stdin.take().expect("piped sink stdin").into();
    let error_output = match output.try_clone() {
        Ok(fd) => fd,
        Err(e) => {
            drop(output);
            let _ = sink.wait();
            return Err(e);
        }
    };
    command
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(error_output));
    // Reap while Matrix is alive, without making emulator status wait for any
    // grandchildren that inherited its output. Dropping Child does not kill it.
    let (finished, completion) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sink.wait();
        let _ = finished.send(());
    });
    Ok(completion)
}
fn refresh_log(record: &mut Record) {
    // Positional reads do not move the shared write offset of the sink's file.
    let mut bytes = vec![0; 65536];
    let mut n = 0;
    while n < bytes.len() {
        match record.file.read_at(&mut bytes[n..], n as u64) {
            Ok(0) => break,
            Ok(read) => n += read,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&bytes[..n]);
    let mut start = text.len().saturating_sub(8192);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    record.info.log = text[start..].to_owned();
}
fn validate_launch_id(id: &str) -> Result<(), String> {
    if matches!(id, "steam" | "windows") {
        Ok(())
    } else {
        spec(id).map(|_| ())
    }
}
fn launcher_name(id: &str) -> &'static str {
    match id {
        "steam" => "Steam launcher",
        "windows" => "Windows launcher",
        _ => "Emulator",
    }
}
pub(crate) fn start(command: Command, id: &str) -> Result<Info, String> {
    validate_launch_id(id)?;
    let root = dirs::data_local_dir()
        .ok_or("Cannot locate Matrix Play launch logs")?
        .join("matrix/launches");
    let launch = update_lifecycle::begin_launch()?;
    start_in_root_guarded(command, id, &root, Some(launch))
}
fn start_in_root(command: Command, id: &str, root: &Path) -> Result<Info, String> {
    start_in_root_guarded(command, id, root, None)
}
fn start_in_root_guarded(
    mut command: Command,
    id: &str,
    root: &Path,
    launch: Option<update_lifecycle::Launch>,
) -> Result<Info, String> {
    let label = launcher_name(id);
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = root.join(format!("{id}-{stamp}-{}.log", std::process::id()));
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&path).map_err(|e| e.to_string())?;
    let record = Arc::new(Mutex::new(Record {
        info: Info {
            console_id: id.into(),
            phase: "starting".into(),
            message: format!("Starting {}…", label.to_lowercase()),
            log: String::new(),
            log_path: path.to_string_lossy().into_owned(),
        },
        file,
        written: 0,
    }));
    records()
        .lock()
        .map_err(|_| "Launch status unavailable")?
        .insert(id.into(), record.clone());
    // Keep only the most recent five bounded logs per console.
    let mut old: Vec<_> = fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|f| {
            f.file_type().is_ok_and(|t| t.is_file())
                && f.file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{id}-"))
                && f.path().extension().is_some_and(|e| e == "log")
        })
        .collect();
    old.sort_by_key(|e| e.file_name());
    let count = old.len().saturating_sub(5);
    for entry in old.into_iter().take(count) {
        let _ = fs::remove_file(entry.path());
    }
    let capture = {
        let r = record.lock().map_err(|_| "Launch status unavailable")?;
        capture_output(&mut command, &r.file, launch.as_ref().map(|l| &l.activity))
    };
    let capture_done = match capture {
        Ok(done) => done,
        Err(e) => {
            let message = format!("Cannot start independent launch log capture: {e}");
            append(&record, &message);
            let mut r = record.lock().map_err(|_| "Launch status unavailable")?;
            r.info.phase = "error".into();
            r.info.message = message;
            return Ok(r.info.clone());
        }
    };
    let activity = launch
        .as_ref()
        .map(|l| l.activity.try_clone())
        .transpose()
        .map_err(|e| e.to_string())?;
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            append(
                &record,
                &format!("Could not start {}: {e}", label.to_lowercase()),
            );
            let mut r = record.lock().map_err(|_| "Launch status unavailable")?;
            r.info.phase = "error".into();
            r.info.message = format!("Could not start {}: {e}", label.to_lowercase());
            return Ok(r.info.clone());
        }
    };
    // Command retains its configured Stdio handles after spawn. Close our copies
    // now, so short-lived launchers can deliver EOF and flush their diagnostics.
    drop(command);
    let watcher = record.clone();
    std::thread::spawn(move || {
        let _activity = activity;
        let status = child.wait();
        // A descendant may keep stdout open indefinitely. Allow a bounded flush
        // window, then publish status even if that descendant is still running.
        let _ = capture_done.recv_timeout(Duration::from_millis(200));
        if let Ok(mut r) = watcher.lock() {
            refresh_log(&mut r);
            match status {
                Ok(s) if s.success() => {
                    r.info.phase = "exited".into();
                    r.info.message = format!(
                        "{label} completed. An existing window may have received the game."
                    );
                }
                Ok(s) => {
                    r.info.phase = "error".into();
                    r.info.message = format!(
                        "{label} exited with {s}. Open launch details for its error output."
                    );
                }
                Err(e) => {
                    r.info.phase = "error".into();
                    r.info.message = format!("Could not monitor {}: {e}", label.to_lowercase());
                }
            }
        }
    });
    for _ in 0..8 {
        std::thread::sleep(Duration::from_millis(50));
        if record
            .lock()
            .map_err(|_| "Launch status unavailable")?
            .info
            .phase
            != "starting"
        {
            break;
        }
    }
    let mut r = record.lock().map_err(|_| "Launch status unavailable")?;
    if r.info.phase == "starting" {
        r.info.phase = "started".into();
        r.info.message = format!("{label} process started.");
    }
    refresh_log(&mut r);
    Ok(r.info.clone())
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn get_launch_status(id: String) -> Result<Option<Info>, String> {
    validate_launch_id(&id)?;
    let list = records().lock().map_err(|_| "Launch status unavailable")?;
    list.get(&id)
        .map(|r| {
            r.lock()
                .map(|mut r| {
                    refresh_log(&mut r);
                    r.info.clone()
                })
                .map_err(|_| "Launch status unavailable".into())
        })
        .transpose()
}
fn ini_value<'a>(text: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut current = "";
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            current = &line[1..line.len() - 1];
            continue;
        }
        if current.eq_ignore_ascii_case(section) {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim());
                }
            }
        }
    }
    None
}
fn setup_issue(text: &str) -> Option<String> {
    if ini_value(text, "UI", "SetupWizardIncomplete")
        .is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1")
    {
        return Some("PCSX2 setup is unfinished. Choose Open emulator to complete its wizard and select your PS2 BIOS.".into());
    }
    if ini_value(text, "Filenames", "BIOS").is_some_and(|v| v.is_empty()) {
        return Some("PCSX2 has no BIOS selected. Choose Open emulator and select your PS2 BIOS in its settings.".into());
    }
    None
}
pub(crate) fn preflight(profile: &EmulatorProfile) -> Result<(), String> {
    if profile.launcher != emulators::Launcher::Flatpak
        || profile.flatpak_id != "net.pcsx2.PCSX2"
        || profile
            .launch_args
            .iter()
            .any(|a| a == "-datapath" || a == "-portable")
    {
        return Ok(());
    }
    let config = home()?.join(".var/app/net.pcsx2.PCSX2/config/PCSX2/inis/PCSX2.ini");
    if !config.exists() {
        return Err("PCSX2 needs its first-run setup. Choose Open emulator to complete the wizard and select your PS2 BIOS.".into());
    }
    if let Some(text) = bounded_io::read_text(&config, 1024 * 1024) {
        if let Some(message) = setup_issue(&text) {
            return Err(message);
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unfinished_pcsx2_setup_is_explained() {
        assert!(setup_issue("[UI]\nSetupWizardIncomplete=true\n")
            .unwrap()
            .contains("Open emulator"));
        assert!(
            setup_issue("[UI]\nSetupWizardIncomplete=false\n[Filenames]\nBIOS=\n")
                .unwrap()
                .contains("BIOS")
        );
        assert!(
            setup_issue("[UI]\nSetupWizardIncomplete=false\n[Filenames]\nBIOS=my-bios.bin\n")
                .is_none()
        );
    }
    #[test]
    fn launcher_exit_fixture() {
        let Some(root) = std::env::var_os("MATRIX_TEST_EXIT_LAUNCHER") else {
            return;
        };
        let root = PathBuf::from(root);
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(root.join("output.log"))
            .unwrap();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "for i in $(seq 1 500); do [ -e \"$1\" ] && break; sleep 0.01; done; [ -e \"$1\" ] || exit 1; /usr/bin/head -c 200000 /dev/zero; /usr/bin/head -c 200000 /dev/zero >&2; printf 'late diagnostic' >&2; touch \"$2\"", "fixture"])
            .arg(root.join("ready")).arg(root.join("done"));
        let activity = update_lifecycle::activity(&root, false).unwrap();
        let _completion = capture_output(&mut command, &file, Some(&activity)).unwrap();
        let _child = command.spawn().unwrap();
        drop(command);
        std::process::exit(0);
    }

    #[test]
    fn output_survives_launcher_exit() {
        let root = tempfile::tempdir().unwrap();
        let (_, module) = module_path!().split_once("::").unwrap();
        let fixture = format!("{module}::launcher_exit_fixture");
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &fixture, "--nocapture"])
            .env("MATRIX_TEST_EXIT_LAUNCHER", root.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        assert!(
            root.path().join("output.log").is_file(),
            "launcher fixture did not run"
        );
        assert!(
            update_lifecycle::activity(root.path(), true).is_err(),
            "log sink must retain game activity after launcher exit"
        );
        // Only release the writer after its launcher process has exited.
        fs::write(root.path().join("ready"), b"").unwrap();
        for _ in 0..500 {
            let data = fs::read(root.path().join("output.log")).unwrap();
            if root.path().join("done").exists() && data.ends_with(b"late diagnostic") {
                assert_eq!(data.len(), 65536);
                // The sink releases its lease only when the writer closes EOF.
                for _ in 0..100 {
                    if update_lifecycle::activity(root.path(), true).is_ok() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                panic!("completed log sink retained its activity lease");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("writer failed or blocked after launcher exited");
    }

    #[test]
    fn short_failure_returns_its_diagnostics() {
        let root = tempfile::tempdir().unwrap();
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("printf 'fixture failure\\n' >&2; exit 7");
        let info = start_in_root(command, "short-failure-fixture", root.path()).unwrap();
        assert_eq!(info.phase, "error");
        assert!(
            info.log.contains("fixture failure"),
            "missing diagnostics: {}",
            info.log
        );
    }
    #[test]
    fn captured_output_is_drained_and_bounded() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("output.log");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("/usr/bin/head -c 200000 /dev/zero; printf stderr >&2");
        let done = capture_output(&mut command, &file, None).unwrap();
        let mut child = command.spawn().unwrap();
        drop(command);
        assert!(child.wait().unwrap().success());
        done.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut record = Record {
            info: Info {
                console_id: "ps2".into(),
                phase: "starting".into(),
                message: String::new(),
                log: String::new(),
                log_path: String::new(),
            },
            file,
            written: 0,
        };
        refresh_log(&mut record);
        assert_eq!(fs::metadata(&path).unwrap().len(), 65536);
        assert_eq!(record.info.log.len(), 8192);
        assert!(record.info.log.ends_with("stderr"));
    }
}
