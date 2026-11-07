use super::*;
use crate::{Asset, Manifest, VerifiedRelease, REPOSITORY};
use ed25519_dalek::{Signer, SigningKey};
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt};

const NEW: &[u8] = b"\x7fELF0000AI\x02new image fixture";
const OLD: &[u8] = b"\x7fELF0000AI\x02old image fixture";
const PRIOR: &[u8] = b"\x7fELF0000AI\x02prior rollback fixture";

fn release() -> VerifiedRelease {
    let key = SigningKey::from_bytes(&[7; 32]);
    let manifest = Manifest {
        schema: 1,
        repository: REPOSITORY.into(),
        version: "0.6.0".into(),
        tag: "v0.6.0".into(),
        notes: String::new(),
        assets: BTreeMap::from([(
            "linux-x86_64-appimage".into(),
            Asset {
                name: "Matrix-Play_0.6.0_amd64.AppImage".into(),
                size: NEW.len() as u64,
                sha256: format!("{:x}", Sha256::digest(NEW)),
            },
        )]),
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    crate::verify_with_key(
        &bytes,
        &key.sign(&bytes).to_bytes(),
        "v0.6.0",
        "0.5.2",
        "appimage",
        &key.verifying_key().to_bytes(),
    )
    .unwrap()
}

struct Fixture {
    root: tempfile::TempDir,
    download: Downloaded,
    target: PathBuf,
    backup: PathBuf,
}
impl Fixture {
    fn new(prior_backup: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("Matrix.AppImage");
        let backup = root.path().join("Matrix.AppImage.previous");
        let package = root.path().join("download");
        fs::write(&target, OLD).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o751)).unwrap();
        fs::write(&package, NEW).unwrap();
        if prior_backup {
            fs::write(&backup, PRIOR).unwrap();
        }
        Self {
            root,
            target,
            backup,
            download: Downloaded {
                release: release(),
                path: package,
            },
        }
    }
    fn assert_original(&self, prior_backup: bool) {
        assert_eq!(fs::read(&self.target).unwrap(), OLD);
        assert_eq!(
            fs::metadata(&self.target).unwrap().permissions().mode() & 0o777,
            0o751
        );
        if prior_backup {
            assert_eq!(fs::read(&self.backup).unwrap(), PRIOR);
        } else {
            assert!(!self.backup.exists());
        }
    }
    fn assert_no_staging(&self) {
        assert!(fs::read_dir(self.root.path()).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .as_encoded_bytes()
            .starts_with(b".matrix-update-")));
    }
}

#[test]
fn repeated_same_signed_image_preserves_original_backup_and_inode() {
    let f = Fixture::new(true);
    install_appimage(&f.download, &f.target).unwrap();
    let installed = snapshot(&f.target).unwrap();
    let rollback = snapshot(&f.backup).unwrap();
    for _ in 0..3 {
        install_appimage(&f.download, &f.target).unwrap();
        assert!(unchanged(&snapshot(&f.target).unwrap(), &installed));
        assert!(unchanged(&snapshot(&f.backup).unwrap(), &rollback));
        assert_eq!(fs::read(&f.target).unwrap(), NEW);
        assert_eq!(fs::read(&f.backup).unwrap(), OLD);
        assert_eq!(
            fs::metadata(&f.backup).unwrap().permissions().mode() & 0o777,
            0o751
        );
    }
    f.assert_no_staging();
}

#[test]
fn already_installed_without_backup_is_noop() {
    let f = Fixture::new(false);
    fs::write(&f.target, NEW).unwrap();
    let before = snapshot(&f.target).unwrap();
    install_appimage(&f.download, &f.target).unwrap();
    assert!(unchanged(&snapshot(&f.target).unwrap(), &before));
    assert!(!f.backup.exists());
    f.assert_no_staging();
}

#[test]
fn failures_at_each_commit_boundary_restore_both_files() {
    for prior_backup in [false, true] {
        for step in [Step::Staged, Step::BackupPublished, Step::TargetPublished] {
            let f = Fixture::new(prior_backup);
            let error = install(&f.download, &f.target, |at| {
                if at == step {
                    Err("simulated filesystem failure".into())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error, "simulated filesystem failure", "{step:?}");
            f.assert_original(prior_backup);
            f.assert_no_staging();
            // Failed transactions release their lock and remain retryable.
            install_appimage(&f.download, &f.target).unwrap();
            assert_eq!(fs::read(&f.backup).unwrap(), OLD);
        }
    }
}

#[test]
fn actual_target_rename_failure_restores_existing_backup() {
    let f = Fixture::new(true);
    let error = install(&f.download, &f.target, |at| {
        if at == Step::BackupPublished {
            let dir = fs::read_dir(f.root.path())
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| p.is_dir())
                .unwrap();
            fs::remove_file(dir.join("next")).unwrap();
        }
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("No such file"), "{error}");
    f.assert_original(true);
    f.assert_no_staging();
}

#[test]
fn changed_target_is_not_overwritten_and_prior_backup_is_retained() {
    for at_step in [Step::Staged, Step::BackupPublished] {
        for replace_inode in [false, true] {
            let f = Fixture::new(true);
            let error = install(&f.download, &f.target, |at| {
                if at == at_step {
                    if replace_inode {
                        let replacement = f.root.path().join("replacement");
                        fs::write(&replacement, PRIOR).unwrap();
                        fs::rename(replacement, &f.target).unwrap();
                    } else {
                        fs::write(&f.target, PRIOR).unwrap();
                    }
                }
                Ok(())
            })
            .unwrap_err();
            assert!(error.contains("changed during installation"), "{error}");
            assert_eq!(fs::read(&f.target).unwrap(), PRIOR);
            assert_eq!(fs::read(&f.backup).unwrap(), PRIOR);
            f.assert_no_staging();
        }
    }
}

#[test]
fn changed_or_symlinked_backup_is_not_overwritten() {
    let f = Fixture::new(true);
    let unrelated = f.root.path().join("unrelated");
    fs::write(&unrelated, b"unrelated").unwrap();
    fs::remove_file(&f.backup).unwrap();
    std::os::unix::fs::symlink(&unrelated, &f.backup).unwrap();
    assert!(install_appimage(&f.download, &f.target)
        .unwrap_err()
        .contains("not a regular file"));
    assert_eq!(fs::read(&f.target).unwrap(), OLD);
    assert_eq!(fs::read(&unrelated).unwrap(), b"unrelated");
    f.assert_no_staging();
    fs::remove_file(&f.backup).unwrap();
    fs::write(&f.backup, PRIOR).unwrap();
    let error = install(&f.download, &f.target, |at| {
        if at == Step::Staged {
            fs::remove_file(&f.backup).unwrap();
            fs::write(&f.backup, b"external backup").unwrap();
        }
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("changed during installation"), "{error}");
    assert_eq!(fs::read(&f.target).unwrap(), OLD);
    assert_eq!(fs::read(&f.backup).unwrap(), b"external backup");
}

#[test]
fn symlink_retargeting_during_install_is_rejected() {
    let f = Fixture::new(true);
    let alias = f.root.path().join("alias.AppImage");
    let unrelated = f.root.path().join("unrelated");
    fs::write(&unrelated, PRIOR).unwrap();
    std::os::unix::fs::symlink(&f.target, &alias).unwrap();
    let error = install(&f.download, &alias, |at| {
        if at == Step::Staged {
            fs::remove_file(&alias).unwrap();
            std::os::unix::fs::symlink(&unrelated, &alias).unwrap();
        }
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("location changed"), "{error}");
    f.assert_original(true);
    assert_eq!(fs::read(unrelated).unwrap(), PRIOR);
}

#[test]
fn interrupted_recovery_keeps_original_images_without_clobbering_external_change() {
    let f = Fixture::new(true);
    let error = install(&f.download, &f.target, |at| {
        if at == Step::TargetPublished {
            fs::write(&f.target, b"external change").unwrap();
            return Err("simulated post-install failure".into());
        }
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("Saved images are in"), "{error}");
    assert_eq!(fs::read(&f.target).unwrap(), b"external change");
    let dir = fs::read_dir(f.root.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.is_dir())
        .unwrap();
    assert_eq!(fs::read(dir.join("original")).unwrap(), OLD);
    assert_eq!(fs::read(dir.join("previous-backup")).unwrap(), PRIOR);
}

// Run this same test binary as a separate OS process; no process-global mutex
// could satisfy the lock contention and crash-release tests below.
#[test]
fn appimage_child_install() {
    let Ok(directory) = std::env::var("MATRIX_UPDATE_TEST_DIRECTORY") else {
        return;
    };
    let root = PathBuf::from(directory);
    let download = Downloaded {
        release: release(),
        path: root.join("download"),
    };
    install(&download, &root.join("Matrix.AppImage"), |at| {
        if format!("{at:?}") == std::env::var("MATRIX_UPDATE_PAUSE_AT").unwrap() {
            fs::write(root.join("ready"), b"ready").unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            while !root.join("continue").exists() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "parent did not release child"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        Ok(())
    })
    .unwrap();
}

fn child_at(f: &Fixture, step: Step) -> std::process::Child {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "appimage::tests::appimage_child_install",
            "--nocapture",
        ])
        .env("MATRIX_UPDATE_TEST_DIRECTORY", f.root.path())
        .env("MATRIX_UPDATE_PAUSE_AT", format!("{step:?}"))
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !f.root.path().join("ready").exists() {
        if let Some(exit) = child.try_wait().unwrap() {
            panic!("child exited early: {exit}");
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("child did not lock folder");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    child
}

#[test]
fn independent_processes_cannot_rotate_backup_concurrently() {
    let f = Fixture::new(true);
    let mut child = child_at(&f, Step::Staged);
    let result = install_appimage(&f.download, &f.target);
    // Always release and reap the child, including when a regression is present.
    fs::write(f.root.path().join("continue"), b"continue").unwrap();
    assert!(child.wait().unwrap().success());
    assert!(result
        .unwrap_err()
        .contains("another update may be running"));
    install_appimage(&f.download, &f.target).unwrap();
    assert_eq!(fs::read(&f.target).unwrap(), NEW);
    assert_eq!(fs::read(&f.backup).unwrap(), OLD);
    f.assert_no_staging();
}

#[test]
fn process_exit_releases_install_lock_without_stale_lock_file() {
    for step in [Step::Staged, Step::BackupPublished, Step::TargetPublished] {
        let f = Fixture::new(true);
        let mut child = child_at(&f, step);
        child.kill().unwrap();
        child.wait().unwrap();
        let recovery = fs::read_dir(f.root.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.is_dir())
            .unwrap();
        assert_eq!(fs::read(recovery.join("original")).unwrap(), OLD);
        assert_eq!(fs::read(recovery.join("previous-backup")).unwrap(), PRIOR);
        if step == Step::Staged {
            f.assert_original(true);
        } else {
            assert_eq!(fs::read(&f.backup).unwrap(), OLD);
        }
        install_appimage(&f.download, &f.target).unwrap();
        assert_eq!(fs::read(&f.target).unwrap(), NEW);
        assert_eq!(fs::read(&f.backup).unwrap(), OLD);
    }
}

#[test]
fn newly_created_backup_is_not_overwritten() {
    let f = Fixture::new(false);
    let error = install(&f.download, &f.target, |at| {
        if at == Step::Staged {
            fs::write(&f.backup, PRIOR).unwrap();
        }
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("backup appeared"), "{error}");
    assert_eq!(fs::read(&f.target).unwrap(), OLD);
    assert_eq!(fs::read(&f.backup).unwrap(), PRIOR);
    f.assert_no_staging();
}

#[test]
fn non_utf8_appimage_names_have_distinct_lossless_backup_names() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let f = Fixture::new(false);
    let target = f
        .root
        .path()
        .join(OsString::from_vec(b"Matrix-\xff.AppImage".to_vec()));
    fs::rename(&f.target, &target).unwrap();
    install_appimage(&f.download, &target).unwrap();
    let backup = f.root.path().join(OsString::from_vec(
        b"Matrix-\xff.AppImage.previous".to_vec(),
    ));
    assert_eq!(fs::read(backup).unwrap(), OLD);
    assert_eq!(fs::read(target).unwrap(), NEW);
}

#[test]
fn rollback_preservation_copies_on_filesystems_without_hard_links() {
    for kind in [
        std::io::ErrorKind::Unsupported,
        std::io::ErrorKind::PermissionDenied,
    ] {
        let f = Fixture::new(true);
        let saved = f.root.path().join("saved");
        let source_before = snapshot(&f.target).unwrap();
        let expected =
            preserve_file_with(&f.target, &saved, |_, _| Err(std::io::Error::from(kind))).unwrap();
        assert!(unchanged(&snapshot(&f.target).unwrap(), &source_before));
        assert!(unchanged(&snapshot(&f.target).unwrap(), &expected));
        assert!(!same_inode(&snapshot(&saved).unwrap(), &expected));
        assert_eq!(fs::read(&saved).unwrap(), OLD);
        assert_eq!(
            fs::metadata(&saved).unwrap().permissions().mode() & 0o777,
            0o751
        );
        // Copies remain independent if a later writer changes the source.
        fs::write(&f.target, PRIOR).unwrap();
        assert_eq!(fs::read(saved).unwrap(), OLD);
    }
}

#[test]
fn copy_fallback_does_not_accept_changed_source_or_overwrite_destination() {
    let f = Fixture::new(true);
    let saved = f.root.path().join("saved");
    let error = preserve_file_with(&f.target, &saved, |_, _| {
        fs::write(&f.target, NEW).unwrap();
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    })
    .unwrap_err();
    assert!(error.contains("changed"), "{error}");
    assert!(!saved.exists());
    fs::write(&saved, PRIOR).unwrap();
    assert!(
        preserve_file_with(&f.target, &saved, |_, _| Err(std::io::Error::from(
            std::io::ErrorKind::Unsupported
        )))
        .is_err()
    );
    assert_eq!(fs::read(saved).unwrap(), PRIOR);
}

#[test]
fn full_transaction_and_rollback_work_with_copy_fallback() {
    for prior_backup in [false, true] {
        for failure in [
            None,
            Some(Step::BackupPublished),
            Some(Step::TargetPublished),
        ] {
            let f = Fixture::new(prior_backup);
            let result = install_with_preserver(
                &f.download,
                &f.target,
                |at| {
                    if Some(at) == failure {
                        Err("injected failure".into())
                    } else {
                        Ok(())
                    }
                },
                |source, destination| {
                    preserve_file_with(source, destination, |_, _| {
                        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
                    })
                },
            );
            if failure.is_some() {
                assert_eq!(result.unwrap_err(), "injected failure");
                f.assert_original(prior_backup);
            } else {
                result.unwrap();
                assert_eq!(fs::read(&f.target).unwrap(), NEW);
                assert_eq!(fs::read(&f.backup).unwrap(), OLD);
            }
            f.assert_no_staging();
        }
    }
}

#[test]
fn completed_transaction_unlocks_even_when_a_duplicate_descriptor_survives() {
    let root = tempfile::tempdir().unwrap();
    let directory = File::open(root.path()).unwrap();
    let lock = DirectoryLock::acquire(&directory).unwrap();
    // dup and fork both retain this same open file description. CLOEXEC alone
    // cannot release it while a concurrent child has not reached exec yet.
    let inherited = directory.try_clone().unwrap();
    let contender = File::open(root.path()).unwrap();
    assert!(DirectoryLock::acquire(&contender).is_err());
    drop(lock);
    drop(directory);
    let next = DirectoryLock::acquire(&contender).unwrap();
    let third = File::open(root.path()).unwrap();
    assert!(DirectoryLock::acquire(&third).is_err());
    // Closing the old inherited descriptor must not release the new owner's
    // independent open-file-description lock.
    drop(inherited);
    assert!(DirectoryLock::acquire(&third).is_err());
    drop(next);
    DirectoryLock::acquire(&third).unwrap();
}

#[test]
fn completed_transaction_unlocks_while_an_unrelated_child_is_still_before_exec() {
    use std::os::unix::process::CommandExt;
    let root = tempfile::tempdir().unwrap();
    let directory = File::open(root.path()).unwrap();
    let lock = DirectoryLock::acquire(&directory).unwrap();
    let (mut ready_reader, mut ready_writer) = std::io::pipe().unwrap();
    let (mut resume_reader, mut resume_writer) = std::io::pipe().unwrap();
    let child = std::thread::spawn(move || {
        let mut command = std::process::Command::new("/bin/true");
        // SAFETY: The post-fork closure only calls read/write on prepared pipe
        // handles. It does not allocate, lock, panic, or invoke the Rust runtime.
        unsafe {
            command.pre_exec(move || {
                ready_writer.write_all(&[1])?;
                resume_reader.read_exact(&mut [0])
            });
        }
        command.status()
    });
    ready_reader.read_exact(&mut [0]).unwrap();
    let contender = File::open(root.path()).unwrap();
    let excluded_before_exit = DirectoryLock::acquire(&contender).is_err();
    drop(lock);
    drop(directory);
    let acquired_after_exit = DirectoryLock::acquire(&contender);
    // Unblock and reap before asserting so a regression never leaves a stalled
    // child holding inherited descriptors from unrelated parallel tests.
    resume_writer.write_all(&[1]).unwrap();
    assert!(child.join().unwrap().unwrap().success());
    assert!(excluded_before_exit);
    assert!(acquired_after_exit.is_ok());
}
