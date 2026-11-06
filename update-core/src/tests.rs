use super::*;
use ed25519_dalek::{Signer, SigningKey};

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
    fn package(
        &mut self,
        _: &str,
        cancel: &AtomicBool,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        self.requests += 1;
        check_cancelled(cancel)?;
        consume(&self.data)
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
    let requests_before_cancel = http.requests;
    cancel.store(true, Ordering::Relaxed);
    assert!(download(&mut http, &release, root.path(), &cancel, |_, _| {}).is_err());
    assert_eq!(http.requests, requests_before_cancel);
    assert_eq!(fs::read_dir(root.path().join("0.6.0")).unwrap().count(), 0);
}

// Exercise the production async HTTP implementation against a deliberately
// stalled local server. Only this test transport permits HTTP or rewrites URLs.
struct LocalHttp {
    http: Http,
    url: String,
}
impl Transport for LocalHttp {
    fn small(&mut self, _: &str, _: usize) -> Result<Vec<u8>, String> {
        unreachable!()
    }
    fn package(
        &mut self,
        _: &str,
        cancel: &AtomicBool,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        self.http.package(&self.url, cancel, consume)
    }
}

fn stalled_download_is_cancelled(send_headers: bool) {
    use std::{
        net::TcpListener,
        sync::{mpsc, Arc},
        thread,
        time::Instant,
    };
    let data = b"!<arch>\nvalid fixture";
    let (m, k) = fixture("deb", data);
    let release = verified(&m, &k, "0.5.2", "deb").unwrap();
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().to_owned();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/package", listener.local_addr().unwrap());
    let (ready_tx, ready_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            assert_eq!(socket.read(&mut byte).unwrap(), 1);
            request.push(byte[0]);
        }
        if send_headers {
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n!<arch>\n",
                data.len()
            )
            .unwrap();
            socket.flush().unwrap();
        }
        ready_tx.send(()).unwrap();
        // No more bytes arrive. Cancellation must drop the request, not leave
        // a detached blocking download consuming a connection for 15 minutes.
        matches!(socket.read(&mut byte), Ok(0))
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let (progress_tx, progress_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut transport = LocalHttp {
            http: Http {
                client: reqwest::Client::builder().no_proxy().build().unwrap(),
                runtime: Some(
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap(),
                ),
            },
            url,
        };
        let result = download(
            &mut transport,
            &release,
            &directory,
            &worker_cancel,
            |received, _| {
                progress_tx.send(received).unwrap();
            },
        );
        drop(transport);
        done_tx.send(result).unwrap();
    });
    ready_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    if send_headers {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let received = progress_rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if received == 8 {
                break;
            }
            assert!(received < 8);
        }
    }
    let started = Instant::now();
    cancel.store(true, Ordering::Relaxed);
    let result = done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("Cancel must interrupt a stalled HTTP operation");
    assert_eq!(result.unwrap_err(), "Update download cancelled");
    assert!(started.elapsed() < Duration::from_secs(2));
    worker.join().unwrap();
    assert!(server.join().unwrap());
    assert_eq!(fs::read_dir(root.path().join("0.6.0")).unwrap().count(), 0);
}

#[test]
fn cancellation_interrupts_waiting_for_response_headers() {
    stalled_download_is_cancelled(false);
}

#[test]
fn cancellation_interrupts_stalled_body_and_removes_partial_package() {
    stalled_download_is_cancelled(true);
}

#[test]
fn cancellation_after_the_last_chunk_does_not_publish_a_package() {
    let data = b"!<arch>\nvalid fixture";
    let (m, k) = fixture("deb", data);
    let release = verified(&m, &k, "0.5.2", "deb").unwrap();
    let root = tempfile::tempdir().unwrap();
    let cancel = AtomicBool::new(false);
    let mut http = Mock {
        data: data.to_vec(),
        requests: 0,
    };
    let result = download(&mut http, &release, root.path(), &cancel, |_, _| {
        cancel.store(true, Ordering::Relaxed);
    });
    assert_eq!(result.unwrap_err(), "Update download cancelled");
    assert_eq!(fs::read_dir(root.path().join("0.6.0")).unwrap().count(), 0);
}

#[test]
fn cancellation_drops_one_pending_operation_without_restarting_it() {
    use std::sync::{atomic::AtomicUsize, Arc};
    struct DropFlag(Arc<AtomicBool>);
    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let task_cancel = cancel.clone();
    runtime.spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        task_cancel.store(true, Ordering::Relaxed);
    });
    let starts = AtomicUsize::new(0);
    let dropped = Arc::new(AtomicBool::new(false));
    let result = runtime.block_on(until_cancelled(&cancel, async {
        starts.fetch_add(1, Ordering::Relaxed);
        let _drop_flag = DropFlag(dropped.clone());
        std::future::pending::<Result<(), String>>().await
    }));
    assert_eq!(result.unwrap_err(), "Update download cancelled");
    assert_eq!(starts.load(Ordering::Relaxed), 1);
    assert!(dropped.load(Ordering::Relaxed));
}

#[test]
fn http_shutdown_does_not_wait_for_a_blocking_resolver_task() {
    use std::{sync::mpsc, thread};
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let http = Http::new().unwrap();
        http.runtime.as_ref().unwrap().spawn_blocking(move || {
            entered_tx.send(()).unwrap();
            let _ = release_rx.recv();
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(http);
        done_tx.send(()).unwrap();
    });
    let finished = done_rx.recv_timeout(Duration::from_secs(2));
    // Release the simulated system resolver even if the regression is present.
    let _ = release_tx.send(());
    worker.join().unwrap();
    assert!(
        finished.is_ok(),
        "Dropping the HTTP client must not delay Cancel"
    );
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
