//! Bounded metadata reads must not wait for a FIFO writer or read a device.
use std::{fs, io::Read, os::unix::fs::OpenOptionsExt, path::Path};

pub(crate) fn read_text(path: &Path, limit: usize) -> Option<String> {
    let before = fs::metadata(path).ok()?;
    if !before.is_file() || before.len() > limit as u64 {
        return None;
    }
    // A regular file may be swapped after metadata(). Nonblocking open plus an
    // opened-handle check also rejects that race without hanging on a FIFO.
    // Regular symlink targets remain supported for exported/config files.
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(path)
        .ok()?;
    let opened = file.metadata().ok()?;
    if !opened.is_file() || opened.len() > limit as u64 {
        return None;
    }
    let mut text = String::new();
    file.take(limit.checked_add(1)? as u64)
        .read_to_string(&mut text)
        .ok()?;
    (text.len() <= limit).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regular_metadata_is_bounded_and_symlink_targets_remain_compatible() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("metadata");
        fs::write(&path, b"valid").unwrap();
        assert_eq!(read_text(&path, 5).as_deref(), Some("valid"));
        assert_eq!(read_text(&path, 4), None);
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        assert_eq!(read_text(&alias, 5).as_deref(), Some("valid"));
        fs::write(&path, [0xff]).unwrap();
        assert_eq!(read_text(&path, 5), None);
        assert_eq!(read_text(temp.path(), 5), None);
        assert_eq!(read_text(&temp.path().join("missing"), 5), None);
    }
}
