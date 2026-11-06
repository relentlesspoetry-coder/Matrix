use super::*;
use std::{
    io::{Read, Write},
    process::Stdio,
    sync::{Arc, Mutex, OnceLock},
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
fn drain(
    mut pipe: impl Read + Send + 'static,
    record: Arc<Mutex<Record>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = [0; 2048];
        while let Ok(n) = pipe.read(&mut buf) {
            if n == 0 {
                break;
            }
            append(&record, &String::from_utf8_lossy(&buf[..n]));
        }
    })
}
pub(crate) fn start(mut command: Command, id: &str) -> Result<Info, String> {
    spec(id)?;
    let root = dirs::data_local_dir()
        .ok_or("Cannot locate Matrix Play launch logs")?
        .join("matrix/launches");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = root.join(format!("{id}-{stamp}-{}.log", std::process::id()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
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
            message: "Starting emulator…".into(),
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
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            append(&record, &format!("Could not start emulator: {e}"));
            let mut r = record.lock().map_err(|_| "Launch status unavailable")?;
            r.info.phase = "error".into();
            r.info.message = format!("Could not start emulator: {e}");
            return Ok(r.info.clone());
        }
    };
    let out = drain(
        child
            .stdout
            .take()
            .ok_or("Cannot capture emulator output")?,
        record.clone(),
    );
    let err = drain(
        child
            .stderr
            .take()
            .ok_or("Cannot capture emulator errors")?,
        record.clone(),
    );
    let watcher = record.clone();
    std::thread::spawn(move || {
        let status = child.wait();
        let _ = out.join();
        let _ = err.join();
        if let Ok(mut r) = watcher.lock() {
            match status {
                Ok(s) if s.success() => {
                    r.info.phase = "exited".into();
                    r.info.message="Emulator launcher completed. An existing emulator window may have received the game.".into();
                }
                Ok(s) => {
                    r.info.phase = "error".into();
                    r.info.message = format!(
                        "Emulator exited with {s}. Open launch details for its error output."
                    );
                }
                Err(e) => {
                    r.info.phase = "error".into();
                    r.info.message = format!("Could not monitor emulator: {e}");
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
        r.info.message = "Emulator process started.".into();
    }
    Ok(r.info.clone())
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn get_launch_status(id: String) -> Result<Option<Info>, String> {
    spec(&id)?;
    let list = records().lock().map_err(|_| "Launch status unavailable")?;
    list.get(&id)
        .map(|r| {
            r.lock()
                .map(|r| r.info.clone())
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
    if fs::metadata(&config).is_ok_and(|m| m.len() <= 1024 * 1024) {
        if let Ok(text) = fs::read_to_string(config) {
            if let Some(message) = setup_issue(&text) {
                return Err(message);
            }
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
    fn captured_output_is_drained_and_bounded() {
        // Test subprocess capture directly, without touching another test's HOME.
        let root = tempfile::tempdir().unwrap();
        let file = fs::File::create(root.path().join("output.log")).unwrap();
        let record = Arc::new(Mutex::new(Record {
            info: Info {
                console_id: "ps2".into(),
                phase: "starting".into(),
                message: String::new(),
                log: String::new(),
                log_path: String::new(),
            },
            file,
            written: 0,
        }));
        let data = vec![b'x'; 200000];
        let thread = drain(std::io::Cursor::new(data), record.clone());
        thread.join().unwrap();
        let r = record.lock().unwrap();
        assert_eq!(r.written, 65536);
        assert_eq!(r.info.log.len(), 8192);
        assert_eq!(
            fs::metadata(root.path().join("output.log")).unwrap().len(),
            65536
        );
    }
}
