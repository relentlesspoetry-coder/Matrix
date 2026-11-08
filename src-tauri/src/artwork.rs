//! Serialize each game's cover replacement across threads and app processes.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

const FORMATS: [&str; 3] = ["png", "jpg", "webp"];
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
static COVER_OPERATION: Mutex<()> = Mutex::new(());

// Cover reads can fully decode a 4096-square image. Moving IPC off the UI thread
// must not multiply that memory cost by every visible card. Call this only from
// a blocking worker, never while holding up GTK or the async executor.
pub(super) fn run_cover_operation<T>(
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _guard = COVER_OPERATION
        .lock()
        .map_err(|_| "Cover worker is unavailable; restart Matrix Play".to_string())?;
    operation()
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

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

pub(super) fn save(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let extension = target.extension().and_then(|e| e.to_str());
    if !extension.is_some_and(|e| FORMATS.contains(&e)) {
        return Err("Unsupported artwork extension".into());
    }
    let parent = target.parent().ok_or("Invalid artwork directory")?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create artwork directory: {e}"))?;
    // All formats share one permanent lock inode. Never delete this sidecar:
    // waiting processes must not end up locking a different inode.
    let lock = private_options()
        .create(true)
        .open(target.with_extension("lock"))
        .map_err(|e| format!("Could not open artwork lock: {e}"))?;
    lock.lock()
        .map_err(|e| format!("Could not lock artwork: {e}"))?;
    let (temporary, mut file) = loop {
        let next = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = target.with_extension(format!("{}.{next}.tmp", std::process::id()));
        match private_options().create_new(true).open(&path) {
            Ok(file) => break (Temporary(path), file),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Could not create artwork temporary: {e}")),
        }
    };
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("Could not save artwork: {e}"))?;
    // Close before renaming so replacement also works on Windows.
    drop(file);
    fs::rename(&temporary.0, target).map_err(|e| format!("Could not finalize artwork: {e}"))?;
    // Only remove old formats after successful publication, while still locked.
    // A cleanup error must not remove the newly saved cover or claim a clean save.
    let mut errors = Vec::new();
    for format in FORMATS {
        if Some(format) != extension {
            if let Err(e) = fs::remove_file(target.with_extension(format)) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    errors.push(format!("{format}: {e}"));
                }
            }
        }
    }
    if !errors.is_empty() {
        return Err(format!(
            "Artwork saved, but could not remove older formats: {}",
            errors.join("; ")
        ));
    }
    // Closing lock releases it on success and on every error path.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command};
    use std::time::{Duration, Instant};

    #[test]
    fn concurrent_cover_operations_have_one_shared_memory_limit() {
        use std::sync::{atomic::AtomicUsize, Barrier};
        let start = Barrier::new(8);
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    start.wait();
                    run_cover_operation(|| {
                        let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                        maximum.fetch_max(count, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(2));
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    })
                    .unwrap();
                });
            }
        });
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        let failed: Result<(), String> = run_cover_operation(|| Err("fixture failure".into()));
        assert!(failed.is_err());
        assert_eq!(run_cover_operation(|| Ok(7)).unwrap(), 7);
    }

    fn writer(root: &Path, format: &str) -> Child {
        let module = module_path!().split_once("::").unwrap().1;
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("{module}::artwork_writer_process")])
            .env("MATRIX_ARTWORK_TEST_ROOT", root)
            .env("MATRIX_ARTWORK_TEST_FORMAT", format)
            .spawn()
            .unwrap()
    }

    #[test]
    fn artwork_writer_process() {
        let Some(root) = std::env::var_os("MATRIX_ARTWORK_TEST_ROOT") else {
            return;
        };
        let root = Path::new(&root);
        let format = std::env::var("MATRIX_ARTWORK_TEST_FORMAT").unwrap();
        fs::write(root.join(format!("{format}.ready")), b"ready").unwrap();
        // Simulate a crashed process's staging file (including PID reuse).
        let stale = root.join(format!("game.{}.0.tmp", std::process::id()));
        fs::write(&stale, b"stale temporary").unwrap();
        for _ in 0..40 {
            save(&root.join(format!("game.{format}")), format.as_bytes()).unwrap();
        }
        assert_eq!(fs::read(&stale).unwrap(), b"stale temporary");
        fs::remove_file(stale).unwrap();
    }

    fn assert_no_temporaries(root: &Path) {
        assert!(!fs::read_dir(root).unwrap().any(|entry| entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "tmp")));
    }

    #[test]
    fn independent_processes_wait_for_shared_lock_and_leave_one_complete_cover() {
        let root = tempfile::tempdir().unwrap();
        let lock_path = root.path().join("game.lock");
        let lock = private_options().create(true).open(&lock_path).unwrap();
        lock.lock().unwrap();
        let mut children: Vec<_> = FORMATS.iter().map(|f| writer(root.path(), f)).collect();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !FORMATS
            .iter()
            .all(|f| root.path().join(format!("{f}.ready")).exists())
        {
            assert!(Instant::now() < deadline, "Child processes did not start");
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(100));
        for child in &mut children {
            assert!(
                child.try_wait().unwrap().is_none(),
                "Writer ignored shared lock"
            );
        }
        for format in FORMATS {
            assert!(!root.path().join(format!("game.{format}")).exists());
        }
        drop(lock);
        for mut child in children {
            assert!(child.wait().unwrap().success());
        }
        let mut remaining = 0;
        for format in FORMATS {
            let path = root.path().join(format!("game.{format}"));
            if path.exists() {
                remaining += 1;
                assert_eq!(fs::read(path).unwrap(), format.as_bytes());
            }
        }
        assert_eq!(remaining, 1);
        assert!(lock_path.is_file());
        assert_no_temporaries(root.path());
    }

    #[test]
    fn failed_replacement_preserves_old_cover_and_cleans_temporary() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("game.jpg");
        fs::write(&old, b"existing cover").unwrap();
        let target = root.path().join("game.png");
        fs::create_dir(&target).unwrap(); // Deterministic rename failure, even as root.
        assert!(save(&target, b"new cover")
            .unwrap_err()
            .contains("finalize"));
        assert_eq!(fs::read(old).unwrap(), b"existing cover");
        assert_no_temporaries(root.path());
    }

    #[test]
    fn cleanup_failure_reports_committed_save_and_preserves_new_cover() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("game.webp");
        fs::create_dir(root.path().join("game.png")).unwrap();
        let error = save(&target, b"new cover").unwrap_err();
        assert!(error.contains("Artwork saved, but"));
        assert_eq!(fs::read(&target).unwrap(), b"new cover");
        assert_no_temporaries(root.path());
        fs::remove_dir(root.path().join("game.png")).unwrap();
        save(&target, b"retry").unwrap(); // Lock was released on the error path.
        assert_eq!(fs::read(target).unwrap(), b"retry");
    }

    #[test]
    fn replacement_keeps_other_games_and_removes_old_formats() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("game.png");
        save(&target, b"first").unwrap();
        save(&root.path().join("other.png"), b"other").unwrap();
        save(&target, b"replacement").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"replacement");
        save(&target.with_extension("jpg"), b"jpeg").unwrap();
        assert!(!target.exists());
        assert_eq!(fs::read(root.path().join("other.png")).unwrap(), b"other");
        assert_no_temporaries(root.path());
    }
}
