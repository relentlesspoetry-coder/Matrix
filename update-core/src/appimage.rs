//! AppImage replacement is serialized across processes, not just UI requests.
//! Stage and sync both images before changing either public path. Keep recovery
//! links until the directory is synced, so a failed commit can undo both names.
use crate::{validate_file, Downloaded};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub fn install_appimage(download: &Downloaded, target: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "linux")]
    return install(download, target, |_| Ok(()));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (download, target);
        Err("AppImage updates are supported on Linux only".into())
    }
}

#[cfg(target_os = "linux")]
struct DirectoryLock<'a>(&'a File);

#[cfg(target_os = "linux")]
impl<'a> DirectoryLock<'a> {
    fn acquire(directory: &'a File) -> Result<Self, String> {
        directory.try_lock().map_err(|e| {
            format!("Cannot lock the AppImage folder; another update may be running: {e}")
        })?;
        Ok(Self(directory))
    }
}

#[cfg(target_os = "linux")]
impl Drop for DirectoryLock<'_> {
    fn drop(&mut self) {
        // flock belongs to an open file description, not a particular fd.
        // Closing our fd alone can leave the lock alive in a child forked by
        // another thread until that child reaches exec (despite CLOEXEC).
        // Explicitly unlock only after this transaction and its cleanup finish.
        let _ = self.0.unlock();
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Staged,
    BackupPublished,
    TargetPublished,
}

#[cfg(target_os = "linux")]
fn snapshot(path: &Path) -> Result<Metadata, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("The AppImage or its backup is no longer a regular file".into());
    }
    Ok(meta)
}

#[cfg(target_os = "linux")]
fn same_inode(a: &Metadata, b: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

#[cfg(target_os = "linux")]
fn same_contents_metadata(a: &Metadata, b: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    same_inode(a, b)
        && a.len() == b.len()
        && a.mode() == b.mode()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
}

#[cfg(target_os = "linux")]
fn unchanged(a: &Metadata, b: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    // Include inode identity and change time: equal size/mtime alone would miss
    // replacement with another file or a writer restoring the modification time.
    same_contents_metadata(a, b) && a.ctime() == b.ctime() && a.ctime_nsec() == b.ctime_nsec()
}

#[cfg(target_os = "linux")]
fn require_unchanged(path: &Path, expected: &Metadata) -> Result<(), String> {
    if unchanged(&snapshot(path)?, expected) {
        Ok(())
    } else {
        Err(
            "The AppImage or its backup changed during installation. Check it before retrying."
                .into(),
        )
    }
}

#[cfg(target_os = "linux")]
fn fingerprint(file: &mut File) -> Result<(String, Vec<u8>), String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut header = Vec::new();
    let mut buf = [0; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        header.extend_from_slice(&buf[..n.min(11 - header.len())]);
        hash.update(&buf[..n]);
    }
    Ok((format!("{:x}", hash.finalize()), header))
}

#[cfg(target_os = "linux")]
fn preserve_file(source: &Path, destination: &Path) -> Result<Metadata, String> {
    preserve_file_with(source, destination, |from, to| fs::hard_link(from, to))
}

#[cfg(target_os = "linux")]
fn preserve_file_with(
    source: &Path,
    destination: &Path,
    link: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<Metadata, String> {
    let before = snapshot(source)?;
    match link(source, destination) {
        Ok(()) => {
            let linked = snapshot(destination)?;
            // Creating a link changes ctime, but no other source attribute.
            if !same_contents_metadata(&before, &linked) {
                return Err("The AppImage backup changed while it was being saved".into());
            }
            require_unchanged(source, &linked)?;
            Ok(linked)
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::Unsupported | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            // Portable AppImages may live on exFAT or another writable volume
            // without hard links. Fall back to a bounded, synced private copy;
            // a failed copy leaves both public files untouched.
            let mut input = File::open(source).map_err(|e| e.to_string())?;
            if !unchanged(&input.metadata().map_err(|e| e.to_string())?, &before) {
                return Err("The AppImage backup changed while it was being opened".into());
            }
            let mut output = File::create_new(destination).map_err(|e| e.to_string())?;
            let count = std::io::copy(
                &mut (&mut input).take(before.len().saturating_add(1)),
                &mut output,
            )
            .map_err(|e| e.to_string())?;
            if count != before.len() {
                return Err("The AppImage backup changed while it was being copied".into());
            }
            output
                .set_permissions(before.permissions())
                .map_err(|e| e.to_string())?;
            output.sync_all().map_err(|e| e.to_string())?;
            require_unchanged(source, &before)?;
            Ok(before)
        }
        Err(error) => Err(format!(
            "Cannot preserve the AppImage rollback file: {error}"
        )),
    }
}

#[cfg(target_os = "linux")]
fn install(
    download: &Downloaded,
    requested_target: &Path,
    checkpoint: impl FnMut(Step) -> Result<(), String>,
) -> Result<PathBuf, String> {
    install_with_preserver(download, requested_target, checkpoint, preserve_file)
}

#[cfg(target_os = "linux")]
fn install_with_preserver(
    download: &Downloaded,
    requested_target: &Path,
    mut checkpoint: impl FnMut(Step) -> Result<(), String>,
    preserve: impl Fn(&Path, &Path) -> Result<Metadata, String>,
) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    if download.release.kind != "appimage" {
        return Err("An AppImage update is required".into());
    }
    validate_file(&download.path, &download.release.asset, "appimage")?;
    let target = requested_target.canonicalize().map_err(|e| e.to_string())?;
    let parent = target.parent().ok_or("AppImage location is invalid")?;
    let directory = File::open(parent).map_err(|e| e.to_string())?;
    // Lock the stable directory inode, never the executable we replace. A lock
    // sidecar that is unlinked after use lets a third process bypass a waiter.
    // The OS releases this advisory lock even when an updater process crashes.
    let _lock = DirectoryLock::acquire(&directory)?;
    let original = snapshot(&target)?;
    let mut old = File::open(&target).map_err(|e| e.to_string())?;
    if !unchanged(&old.metadata().map_err(|e| e.to_string())?, &original) {
        return Err("The AppImage changed while it was being opened".into());
    }
    let (old_hash, header) = fingerprint(&mut old)?;
    let check_target = || -> Result<(), String> {
        if !same_inode(
            &fs::metadata(parent).map_err(|e| e.to_string())?,
            &directory.metadata().map_err(|e| e.to_string())?,
        ) {
            return Err("The AppImage folder changed during installation".into());
        }
        if requested_target.canonicalize().map_err(|e| e.to_string())? != target {
            return Err("The AppImage location changed during installation".into());
        }
        require_unchanged(&target, &original)
    };
    check_target()?;
    if !header.starts_with(b"\x7fELF") || header.get(8..11) != Some(b"AI\x02") {
        return Err("The running AppImage could not be identified".into());
    }
    // A second instance may still hold this download after the first installed
    // it. Returning success must not rotate away the actual rollback image.
    if original.len() == download.release.asset.size && old_hash == download.release.asset.sha256 {
        return Ok(target);
    }
    let mut backup_name = target
        .file_name()
        .ok_or("Invalid AppImage name")?
        .to_os_string();
    backup_name.push(".previous");
    let backup = parent.join(backup_name);
    // The signed bytes and rollback files must remain inaccessible to other
    // users until publication. TempDir's default 0777 masked by umask can leave
    // this directory group-writable, allowing substitution after verification.
    let staging = tempfile::Builder::new()
        .prefix(".matrix-update-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(parent)
        .map_err(|_| "Cannot write the AppImage folder. The update is downloaded; move it to a writable location.")?;
    let next = staging.path().join("next");
    let original_copy = staging.path().join("original");
    let next_backup = staging.path().join("next-backup");
    let previous_backup = staging.path().join("previous-backup");
    let mut staged = File::create_new(&next).map_err(|e| e.to_string())?;
    std::io::copy(
        &mut File::open(&download.path)
            .map_err(|e| e.to_string())?
            .take(download.release.asset.size + 1),
        &mut staged,
    )
    .map_err(|e| e.to_string())?;
    staged.flush().map_err(|e| e.to_string())?;
    validate_file(&next, &download.release.asset, "appimage")?;
    staged
        .set_permissions(fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    staged.sync_all().map_err(|e| e.to_string())?;
    old.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut saved = File::create_new(&original_copy).map_err(|e| e.to_string())?;
    std::io::copy(&mut old.take(original.len() + 1), &mut saved).map_err(|e| e.to_string())?;
    saved.flush().map_err(|e| e.to_string())?;
    saved
        .set_permissions(original.permissions())
        .map_err(|e| e.to_string())?;
    saved.sync_all().map_err(|e| e.to_string())?;
    let (saved_hash, _) = fingerprint(&mut File::open(&original_copy).map_err(|e| e.to_string())?)?;
    if saved_hash != old_hash {
        return Err("The AppImage changed while its rollback copy was being prepared".into());
    }
    preserve(&original_copy, &next_backup)?;
    // Preserve the old rollback file, using a copy if hard links are unavailable.
    // Reject an unexpected directory/symlink before touching either public path.
    let previous = match fs::symlink_metadata(&backup) {
        Ok(meta) => {
            if !meta.is_file() {
                return Err("The AppImage backup is not a regular file".into());
            }
            Some(preserve(&backup, &previous_backup)?)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    File::open(staging.path())
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    directory.sync_all().map_err(|e| e.to_string())?;
    let mut published_backup = None;
    let mut published_target = None;
    let mut backup_committed = false;
    let mut target_committed = false;
    let result = (|| -> Result<(), String> {
        checkpoint(Step::Staged)?;
        check_target()?;
        if let Some(meta) = &previous {
            require_unchanged(&backup, meta)?;
        } else if fs::symlink_metadata(&backup).is_ok() {
            return Err("The AppImage backup appeared during installation".into());
        }
        fs::rename(&next_backup, &backup).map_err(|e| e.to_string())?;
        backup_committed = true;
        published_backup = Some(snapshot(&backup)?);
        checkpoint(Step::BackupPublished)?;
        directory.sync_all().map_err(|e| e.to_string())?;
        check_target()?;
        fs::rename(&next, &target).map_err(|e| e.to_string())?;
        target_committed = true;
        published_target = Some(snapshot(&target)?);
        checkpoint(Step::TargetPublished)?;
        directory.sync_all().map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = result {
        let rollback = (|| -> Result<(), String> {
            // Check both paths before undoing either, and do not overwrite an
            // unrelated replacement made by an uncooperative external writer.
            if target_committed {
                require_unchanged(
                    &target,
                    published_target
                        .as_ref()
                        .ok_or("The installed AppImage could not be identified")?,
                )?;
            }
            if backup_committed {
                require_unchanged(
                    &backup,
                    published_backup
                        .as_ref()
                        .ok_or("The new rollback image could not be identified")?,
                )?;
                if previous.is_some() {
                    fs::rename(&previous_backup, &backup).map_err(|e| e.to_string())?;
                } else {
                    fs::remove_file(&backup).map_err(|e| e.to_string())?;
                }
            }
            if target_committed {
                fs::rename(&original_copy, &target).map_err(|e| e.to_string())?;
            }
            directory.sync_all().map_err(|e| e.to_string())?;
            Ok(())
        })();
        if let Err(rollback_error) = rollback {
            let recovery = staging.keep();
            return Err(format!(
                "{error}. Recovery was interrupted: {rollback_error}. Saved images are in {}",
                recovery.display()
            ));
        }
        return Err(error);
    }
    Ok(target)
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
