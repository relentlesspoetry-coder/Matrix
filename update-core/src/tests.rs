use super::*;
use ed25519_dalek::{Signer, SigningKey};
use std::io::Cursor;

fn fixture(kind: &str, data: &[u8]) -> (Manifest, SigningKey) {
    let extension = if kind == "deb" { "deb" } else { "AppImage" };
    let asset = Asset {
        name: format!("Matrix-Play_0.6.0_amd64.{extension}"),
        size: data.len() as u64,
        sha256: format!("{:x}", Sha256::digest(data)),
    };
    (
        Manifest {
            schema: 1,
            repository: REPOSITORY.into(),
            version: "0.6.0".into(),
            tag: "v0.6.0".into(),
            notes: "Test update".into(),
            assets: BTreeMap::from([(format!("linux-x86_64-{kind}"), asset)]),
        },
        SigningKey::from_bytes(&[7; 32]),
    )
}
fn verified(
    manifest: &Manifest,
    key: &SigningKey,
    current: &str,
    kind: &str,
) -> Result<VerifiedRelease, String> {
    let bytes = serde_json::to_vec(manifest).unwrap();
    verify_with_key(
        &bytes,
        &key.sign(&bytes).to_bytes(),
        "v0.6.0",
        current,
        kind,
        &key.verifying_key().to_bytes(),
    )
}
#[test]
fn semver_selects_newest_published_and_rejects_malformed_tags() {
    let r=br#"[{"tag_name":"v0.9.0","draft":true},{"tag_name":"v0.6.0-beta.1"},{"tag_name":"v0.5.9"},{"tag_name":"vv9.0.0"},{"tag_name":"v0.6.0"}]"#;
    assert_eq!(latest_release(r).unwrap().unwrap().tag_name, "v0.6.0");
    assert!(is_newer("0.6.0", "0.6.0-beta.1").unwrap());
    assert!(!is_newer("0.5.2", "0.6.0").unwrap());
    assert!(asset_url("v0.6.0", "../run.sh").is_err());
}
#[test]
fn signatures_and_manifest_identity_are_required() {
    let (m, k) = fixture("deb", b"!<arch>\nfixture");
    assert!(verified(&m, &k, "0.5.2", "deb").is_ok());
    let mut bytes = serde_json::to_vec(&m).unwrap();
    let sig = k.sign(&bytes).to_bytes();
    bytes[10] ^= 1;
    assert!(verify_with_key(
        &bytes,
        &sig,
        "v0.6.0",
        "0.5.2",
        "deb",
        &k.verifying_key().to_bytes()
    )
    .is_err());
    assert!(verify_manifest(
        &serde_json::to_vec(&m).unwrap(),
        &sig,
        "v0.6.0",
        "0.5.2",
        "deb"
    )
    .is_err());
    assert!(verified(&m, &k, "0.6.0", "deb").is_err());
    assert!(verified(&m, &k, "0.7.0", "deb").is_err());
    assert!(verified(&m, &k, "0.5.2", "appimage").is_err());
    for mutate in [0, 1, 2, 3, 4] {
        let mut bad = m.clone();
        match mutate {
            0 => bad.repository = "other/app".into(),
            1 => bad.tag = "v0.7.0".into(),
            2 => bad.assets.get_mut("linux-x86_64-deb").unwrap().name = "payload.deb".into(),
            3 => bad.assets.get_mut("linux-x86_64-deb").unwrap().size = MAX_PACKAGE + 1,
            _ => bad.schema = 2,
        }
        assert!(verified(&bad, &k, "0.5.2", "deb").is_err());
    }
}
struct Mock {
    data: Vec<u8>,
    requests: usize,
}
impl Transport for Mock {
    fn small(&mut self, _: &str, _: usize) -> Result<Vec<u8>, String> {
        unreachable!()
    }
    fn package(&mut self, _: &str) -> Result<Box<dyn Read>, String> {
        self.requests += 1;
        Ok(Box::new(Cursor::new(self.data.clone())))
    }
}
#[test]
fn download_checks_bytes_reuses_cache_and_removes_partial_files() {
    let data = b"!<arch>\nvalid fixture";
    let (m, k) = fixture("deb", data);
    let release = verified(&m, &k, "0.5.2", "deb").unwrap();
    let root = tempfile::tempdir().unwrap();
    let cancel = AtomicBool::new(false);
    let mut http = Mock {
        data: data.to_vec(),
        requests: 0,
    };
    let result = download(&mut http, &release, root.path(), &cancel, |_, _| {}).unwrap();
    download(&mut http, &release, root.path(), &cancel, |_, _| {}).unwrap();
    assert_eq!(http.requests, 1);
    fs::write(&result.path, b"corrupted").unwrap();
    http.data = b"!<arch>\nwrong fixture".to_vec();
    assert!(download(&mut http, &release, root.path(), &cancel, |_, _| {}).is_err());
    http.data = [data.as_slice(), b"extra"].concat();
    assert!(download(&mut http, &release, root.path(), &cancel, |_, _| {}).is_err());
    fs::remove_file(&result.path).unwrap();
    cancel.store(true, Ordering::Relaxed);
    assert!(download(&mut http, &release, root.path(), &cancel, |_, _| {}).is_err());
    assert_eq!(fs::read_dir(root.path().join("0.6.0")).unwrap().count(), 0);
}
#[test]
fn appimage_replacement_keeps_backup_and_refuses_tampering() {
    let data = b"\x7fELF0000AI\x02new image fixture";
    let old = b"\x7fELF0000AI\x02old image fixture";
    let (m, k) = fixture("appimage", data);
    let release = verified(&m, &k, "0.5.2", "appimage").unwrap();
    let root = tempfile::tempdir().unwrap();
    let package = root.path().join("update");
    let target = root.path().join("Matrix.AppImage");
    fs::write(&package, data).unwrap();
    fs::write(&target, old).unwrap();
    let d = Downloaded {
        release,
        path: package.clone(),
    };
    install_appimage(&d, &target).unwrap();
    assert_eq!(fs::read(&target).unwrap(), data);
    assert_eq!(
        fs::read(root.path().join("Matrix.AppImage.previous")).unwrap(),
        old
    );
    fs::write(&package, b"tampered").unwrap();
    assert!(install_appimage(&d, &target).is_err());
    assert_eq!(fs::read(&target).unwrap(), data);
}
#[test]
fn package_header_and_symlink_checks() {
    let data = b"not a real Linux package";
    let (m, _) = fixture("deb", data);
    let root = tempfile::tempdir().unwrap();
    let p = root.path().join("package");
    fs::write(&p, data).unwrap();
    assert!(validate_file(&p, &m.assets["linux-x86_64-deb"], "deb").is_err());
    #[cfg(unix)]
    {
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&p, &link).unwrap();
        assert!(validate_file(&link, &m.assets["linux-x86_64-deb"], "deb").is_err());
    }
}
