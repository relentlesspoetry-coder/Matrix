//! One lock inode for the complete read/modify/replace transaction, across windows
//! and processes. Never lock the settings inode itself: atomic rename replaces it.
use super::Settings;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) fn load(path: &Path) -> Result<Settings, String> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            serde_json::from_str(&contents).map_err(|e| format!("Invalid {}: {e}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(format!("Read {}: {e}", path.display())),
    }
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

pub(super) fn update<T>(
    path: &Path,
    change: impl FnOnce(&mut Settings) -> Result<T, String>,
) -> Result<T, String> {
    let parent = path.parent().ok_or("Invalid configuration directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    // Leave this sidecar in place. Removing it could let another process lock a
    // different inode while an existing transaction is still using the old one.
    let lock = private_options()
        .create(true)
        .open(path.with_extension("json.lock"))
        .map_err(|e| format!("Open settings lock: {e}"))?;
    lock.lock().map_err(|e| format!("Lock settings: {e}"))?;
    let mut settings = load(path)?;
    let before = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
    let result = change(&mut settings)?;
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
    if bytes == before {
        return Ok(result);
    }
    // A unique create_new temporary also prevents crash leftovers and concurrent
    // instances from sharing a staging file. The rename stays on one filesystem.
    let (temporary, mut file) = loop {
        let next = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("json.{}.{next}.tmp", std::process::id()));
        match private_options().create_new(true).open(&temporary) {
            Ok(file) => break (Temporary(temporary), file),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Create settings temporary: {e}")),
        }
    };
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(&temporary.0, path).map_err(|e| e.to_string())?;
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())?;
    // Closing `lock` releases the OS lock even on an earlier error or panic.
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn settings_writer_process() {
        let Some(path) = std::env::var_os("MATRIX_SETTINGS_TEST_PATH") else {
            return;
        };
        let name = std::env::var("MATRIX_SETTINGS_TEST_WRITER").unwrap();
        for n in 0..20 {
            update(Path::new(&path), |settings| {
                settings
                    .favorites
                    .entry("ps2".into())
                    .or_default()
                    .push(format!("{name}-{n}"));
                // Force competing processes to overlap their transactions.
                std::thread::sleep(std::time::Duration::from_millis(2));
                Ok(())
            })
            .unwrap();
        }
    }

    #[test]
    fn concurrent_processes_preserve_every_change() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("frontend.json");
        // Rust's test harness omits the crate name in both binary and lib tests.
        let module = module_path!().split_once("::").unwrap().1;
        let test = format!("{module}::settings_writer_process");
        let mut children = Vec::new();
        for n in 0..4 {
            children.push(
                Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", &test])
                    .env("MATRIX_SETTINGS_TEST_PATH", &path)
                    .env("MATRIX_SETTINGS_TEST_WRITER", n.to_string())
                    .spawn()
                    .unwrap(),
            );
        }
        for mut child in children {
            assert!(child.wait().unwrap().success());
        }
        let saved = load(&path).unwrap();
        assert_eq!(saved.favorites["ps2"].len(), 80);
        let unique: std::collections::HashSet<_> = saved.favorites["ps2"].iter().collect();
        assert_eq!(unique.len(), 80);
        assert!(!fs::read_dir(root.path()).unwrap().any(|p| p
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "tmp")));
    }

    #[test]
    fn failed_transactions_leave_existing_settings_intact() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("frontend.json");
        update(&path, |s| {
            s.artwork.style = "3d".into();
            Ok(())
        })
        .unwrap();
        let before = fs::read(&path).unwrap();
        let result: Result<(), String> = update(&path, |s| {
            s.artwork.style = "default".into();
            Err("Abort this change".into())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        update(&path, |s| {
            s.artwork.enabled = false;
            Ok(())
        })
        .unwrap();
        assert_eq!(load(&path).unwrap().artwork.style, "3d");
    }
}
