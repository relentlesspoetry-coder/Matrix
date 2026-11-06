//! Narrow privileged DEB install protocol. All stdin bytes are untrusted.
//! The signed metadata, never a client-supplied digest, authorizes the package.
use crate::{Downloaded, Manifest, VerifiedRelease, MAX_PACKAGE};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

const MAGIC: &[u8; 8] = b"MATRIX01";

/// Send exact signed metadata and a bounded package stream, not a mutable path.
pub fn write_request(download: &Downloaded, out: &mut impl Write) -> Result<(), String> {
    let bytes = &download.release.signed_manifest;
    if bytes.len() > 65536 || download.release.signature.len() != 64 {
        return Err("Invalid installer metadata".into());
    }
    out.write_all(MAGIC).map_err(|e| e.to_string())?;
    out.write_all(&(bytes.len() as u32).to_be_bytes())
        .map_err(|e| e.to_string())?;
    out.write_all(bytes).map_err(|e| e.to_string())?;
    out.write_all(&download.release.signature)
        .map_err(|e| e.to_string())?;
    let source = File::open(&download.path).map_err(|e| e.to_string())?;
    let count = std::io::copy(&mut source.take(MAX_PACKAGE + 1), out).map_err(|e| e.to_string())?;
    if count > MAX_PACKAGE {
        return Err("Installer package is too large".into());
    }
    out.flush().map_err(|e| e.to_string())
}

fn read_metadata(
    input: &mut impl Read,
    current: &str,
    key: &[u8; 32],
) -> Result<VerifiedRelease, String> {
    let mut magic = [0; 8];
    input.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != MAGIC {
        return Err("Invalid installer request".into());
    }
    let mut size = [0; 4];
    input.read_exact(&mut size).map_err(|e| e.to_string())?;
    let size = u32::from_be_bytes(size) as usize;
    if size > 65536 {
        return Err("Update manifest is too large".into());
    }
    let mut bytes = vec![0; size];
    input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    let mut signature = [0; 64];
    input
        .read_exact(&mut signature)
        .map_err(|e| e.to_string())?;
    // The tag is only a consistency hint until verify_with_key authenticates
    // the complete manifest, repository, architecture, version and asset.
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid update manifest")?;
    crate::verify_with_key(&bytes, &signature, &manifest.tag, current, "deb", key)
}

fn stage_package(
    input: &mut impl Read,
    release: &VerifiedRelease,
    directory: &Path,
) -> Result<PathBuf, String> {
    let path = directory.join("update.deb");
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let count = std::io::copy(&mut input.take(release.asset.size + 1), &mut file)
        .map_err(|e| e.to_string())?;
    if count != release.asset.size {
        return Err("Installer package size does not match signed metadata".into());
    }
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    crate::validate_file(&path, &release.asset, "deb")?;
    Ok(path)
}

#[cfg(unix)]
fn require_root_directory(path: &Path, private: bool) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let forbidden = if private { 0o077 } else { 0o022 };
    if !meta.is_dir() || meta.uid() != 0 || meta.permissions().mode() & forbidden != 0 {
        return Err("Installer requires protected root-owned staging".into());
    }
    Ok(())
}

/// Called only by the installed, root-owned helper through pkexec. No paths,
/// commands, digests, keys, versions or APT options are accepted as arguments.
#[cfg(unix)]
pub fn install_from_stdin(input: &mut impl Read, current: &str) -> Result<(), String> {
    let release = read_metadata(input, current, crate::PUBLIC_KEY)?;
    // Fixed root-owned ancestors; never TMPDIR or a caller-selected directory.
    for parent in ["/", "/var", "/var/cache"] {
        require_root_directory(Path::new(parent), false)?;
    }
    let staging = tempfile::Builder::new()
        .prefix("matrix-update-")
        .tempdir_in("/var/cache")
        .map_err(|e| e.to_string())?;
    require_root_directory(staging.path(), true)?;
    let path = stage_package(input, &release, staging.path())?;
    // The validated bytes remain inside a root-owned 0700 directory until APT
    // finishes. Neither replacing the user's download nor mutating stdin can
    // change this file after verification. APT may use its root fallback when
    // its sandbox account cannot traverse this deliberately private directory.
    let status = Command::new("/usr/bin/apt-get")
        .args(["install", "-y", "--"])
        .arg(&path)
        .current_dir("/")
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("HOME", "/root")
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("Cannot run system installer: {e}"))?;
    if !status.success() {
        return Err("The system installer could not complete installation".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use std::{collections::BTreeMap, io::Cursor};

    fn request(data: &[u8]) -> (Vec<u8>, SigningKey) {
        let key = SigningKey::from_bytes(&[7; 32]);
        let manifest = Manifest {
            schema: 1,
            repository: crate::REPOSITORY.into(),
            version: "0.6.0".into(),
            tag: "v0.6.0".into(),
            notes: String::new(),
            assets: BTreeMap::from([(
                "linux-x86_64-deb".into(),
                crate::Asset {
                    name: "Matrix-Play_0.6.0_amd64.deb".into(),
                    size: data.len() as u64,
                    sha256: format!("{:x}", Sha256::digest(data)),
                },
            )]),
        };
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let mut out = MAGIC.to_vec();
        out.extend((bytes.len() as u32).to_be_bytes());
        out.extend(&bytes);
        out.extend(key.sign(&bytes).to_bytes());
        out.extend(data);
        (out, key)
    }

    #[test]
    fn privileged_boundary_rejects_unsigned_metadata_and_downgrades() {
        let (bytes, key) = request(b"!<arch>\nfixture");
        // Fixture keys are never accepted by the production entry point.
        assert!(read_metadata(&mut Cursor::new(&bytes), "0.5.6", crate::PUBLIC_KEY).is_err());
        assert!(read_metadata(
            &mut Cursor::new(&bytes),
            "0.6.0",
            &key.verifying_key().to_bytes()
        )
        .is_err());
        let mut bad = bytes.clone();
        let size = u32::from_be_bytes(bad[8..12].try_into().unwrap()) as usize;
        let mut manifest: Manifest = serde_json::from_slice(&bad[12..12 + size]).unwrap();
        manifest.assets.get_mut("linux-x86_64-deb").unwrap().sha256 =
            format!("{:x}", Sha256::digest(b"!<arch>\nmalware"));
        let forged = serde_json::to_vec(&manifest).unwrap();
        assert_eq!(forged.len(), size);
        bad[12..12 + size].copy_from_slice(&forged);
        // Recomputing a client-side hash cannot authorize replacement bytes.
        assert!(read_metadata(
            &mut Cursor::new(bad),
            "0.5.6",
            &key.verifying_key().to_bytes()
        )
        .is_err());
        let mut oversized = MAGIC.to_vec();
        oversized.extend(65537u32.to_be_bytes());
        assert!(read_metadata(&mut Cursor::new(oversized), "0.5.6", crate::PUBLIC_KEY).is_err());
    }

    #[test]
    fn staged_bytes_are_verified_and_independent_of_mutable_download() {
        let data = b"!<arch>\nfixture";
        let (bytes, key) = request(data);
        let mut input = Cursor::new(bytes);
        let release = read_metadata(&mut input, "0.5.6", &key.verifying_key().to_bytes()).unwrap();
        let source = tempfile::tempdir().unwrap();
        let source_path = source.path().join("download.deb");
        std::fs::write(&source_path, data).unwrap();
        let download = Downloaded {
            release: release.clone(),
            path: source_path.clone(),
        };
        let mut wire = Vec::new();
        write_request(&download, &mut wire).unwrap();
        let mut wire = Cursor::new(wire);
        read_metadata(&mut wire, "0.5.6", &key.verifying_key().to_bytes()).unwrap();
        let protected = tempfile::tempdir().unwrap();
        let staged = stage_package(&mut wire, &release, protected.path()).unwrap();
        std::fs::write(&source_path, b"!<arch>\nmalware").unwrap();
        assert_eq!(std::fs::read(&staged).unwrap(), data);
        crate::validate_file(&staged, &release.asset, "deb").unwrap();
        for modified in [
            b"!<arch>\nmalware".as_slice(),
            b"!<arch>\nfixtur",
            b"!<arch>\nfixtureX",
        ] {
            let protected = tempfile::tempdir().unwrap();
            assert!(stage_package(&mut Cursor::new(modified), &release, protected.path()).is_err());
        }
    }

    #[test]
    #[cfg(unix)]
    fn staging_rejects_symlinks_and_unprotected_directories() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let link = root.path().join("link");
        symlink(root.path(), &link).unwrap();
        assert!(require_root_directory(&link, true).is_err());
        let (bytes, key) = request(b"!<arch>\nfixture");
        let mut input = Cursor::new(bytes);
        let release = read_metadata(&mut input, "0.5.6", &key.verifying_key().to_bytes()).unwrap();
        let target = root.path().join("unrelated");
        std::fs::write(&target, b"keep me").unwrap();
        symlink(&target, root.path().join("update.deb")).unwrap();
        assert!(stage_package(&mut input, &release, root.path()).is_err());
        assert_eq!(std::fs::read(target).unwrap(), b"keep me");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(require_root_directory(root.path(), true).is_err());
        assert!(require_root_directory(root.path(), false).is_err());
    }
}
