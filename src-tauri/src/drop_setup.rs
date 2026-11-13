//! Native drops carry paths, not browser File objects. Inspection never executes,
//! moves or registers anything; explicit confirmation uses the normal validators.
use super::*;
use std::io::Read;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DropItem {
    path: String,
    kind: String,
    console_ids: Vec<String>,
}
pub(crate) fn inspect(paths: Vec<String>) -> Result<Vec<DropItem>, String> {
    if paths.is_empty() || paths.len() > 16 {
        return Err("Drop between 1 and 16 files or folders".into());
    }
    paths
        .into_iter()
        .map(|p| {
            let path = Path::new(&p);
            if !path.is_absolute() {
                return Err("Drop an absolute local path".into());
            }
            let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() {
                return Err("Drop the actual file or folder, not a symbolic link".into());
            }
            let path = path.canonicalize().map_err(|e| e.to_string())?;
            let mut kind = "unsupported";
            let mut console_ids = Vec::new();
            if meta.is_dir() {
                kind = "folder";
            } else if meta.is_file() {
                let mut magic = Vec::new();
                fs::File::open(&path)
                    .map_err(|e| e.to_string())?
                    .take(16)
                    .read_to_end(&mut magic)
                    .map_err(|e| e.to_string())?;
                if cover_format(&magic).is_some()
                    || path.extension().and_then(|v| v.to_str()).is_some_and(|e| {
                        ["png", "jpg", "jpeg", "webp"].contains(&e.to_ascii_lowercase().as_str())
                    })
                {
                    kind = "cover";
                } else if magic.starts_with(b"\x7fELF") || magic.starts_with(b"#!") {
                    kind = "emulator";
                    for s in SPECS {
                        if discovery::profile_for_file(s.id, &path).is_ok() {
                            console_ids.push(s.id.into());
                        }
                    }
                }
            }
            Ok(DropItem {
                path: actionable_path(&path)?,
                kind: kind.into(),
                console_ids,
            })
        })
        .collect()
}
pub(crate) fn cover(id: &str, uid: &str, path: &str) -> Result<(), String> {
    let mut bytes = Vec::new();
    let path = Path::new(path);
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_COVER_BYTES as u64 {
        return Err("Choose a regular cover image up to 4 MB".into());
    }
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_COVER_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    artwork::run_cover_operation(|| save_cover(id.into(), uid.into(), bytes))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspect_does_not_execute_or_move() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("portable");
        fs::write(&file, b"#! /bin/sh\ntouch never\n").unwrap();
        let items = inspect(vec![
            file.to_str().unwrap().into(),
            root.path().to_str().unwrap().into(),
        ])
        .unwrap();
        assert_eq!(items[0].kind, "emulator");
        assert_eq!(items[1].kind, "folder");
        assert!(file.exists());
        assert!(!root.path().join("never").exists());
        assert!(inspect(vec!["relative".into()]).is_err());
        assert!(inspect(Vec::new()).is_err());
    }
}
