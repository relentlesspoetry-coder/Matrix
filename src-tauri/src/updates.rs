use matrix_update_core as core;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Info {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub phase: String,
    pub message: String,
    pub notes: String,
    pub received: u64,
    pub total: u64,
    pub install_mode: String,
}
struct Runtime {
    info: Info,
    release: Option<core::VerifiedRelease>,
    download: Option<core::Downloaded>,
    installed: Option<PathBuf>,
    last_check: Option<Instant>,
}
#[derive(Clone)]
pub(crate) struct Service {
    inner: Arc<Mutex<Runtime>>,
    cancel: Arc<AtomicBool>,
}
fn identify_appimage(
    image: &std::path::Path,
    appdir: &std::path::Path,
    exe: &std::path::Path,
) -> Option<PathBuf> {
    super::appimage_environment::Context::identify(image, appdir, exe).map(|context| context.image)
}
fn running_appimage() -> Option<PathBuf> {
    identify_appimage(
        &PathBuf::from(std::env::var_os("APPIMAGE")?),
        &PathBuf::from(std::env::var_os("APPDIR")?),
        &std::env::current_exe().ok()?,
    )
}
fn mode() -> String {
    if running_appimage().is_some() {
        "appimage"
    } else if std::env::current_exe()
        .ok()
        .is_some_and(|p| p == std::path::Path::new("/usr/bin/matrix-frontend"))
    {
        "system"
    } else {
        "download"
    }
    .into()
}
impl Default for Service {
    fn default() -> Self {
        Self::with_outcome(super::update_lifecycle::outcome())
    }
}
impl Service {
    fn with_outcome(outcome: Option<super::update_lifecycle::Outcome>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Runtime {
                info: Info {
                    current_version: env!("CARGO_PKG_VERSION").into(),
                    latest_version: None,
                    phase: if outcome.is_some() {
                        "recovery"
                    } else {
                        "idle"
                    }
                    .into(),
                    message: outcome.map(|o| o.message).unwrap_or_else(|| {
                        "Check GitHub for the newest Matrix Play release.".into()
                    }),
                    notes: String::new(),
                    received: 0,
                    total: 0,
                    install_mode: mode(),
                },
                release: None,
                download: None,
                installed: None,
                last_check: None,
            })),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}
impl Service {
    fn status(&self) -> Result<Info, String> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| "Update state unavailable")?
            .info
            .clone())
    }
    fn fail(&self, error: String) -> Result<Info, String> {
        let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
        // Only successful checks may be reused. Check/download/install errors
        // (including cancellation) must not disable the visible Check again.
        s.last_check = None;
        s.info.phase = "error".into();
        s.info.message = error;
        Ok(s.info.clone())
    }
    fn check(&self) -> Result<Info, String> {
        self.check_with(|kind| {
            core::check(&mut core::Http::new()?, env!("CARGO_PKG_VERSION"), kind)
        })
    }
    fn check_with(
        &self,
        fetch: impl FnOnce(&str) -> Result<(Option<core::VerifiedRelease>, String), String>,
    ) -> Result<Info, String> {
        let kind = {
            let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
            if ["checking", "downloading", "installing"].contains(&s.info.phase.as_str()) {
                return Ok(s.info.clone());
            }
            if s.last_check
                .is_some_and(|t| t.elapsed() < Duration::from_secs(60))
            {
                return Ok(s.info.clone());
            }
            s.last_check = Some(Instant::now());
            s.info.phase = "checking".into();
            s.info.message = "Checking GitHub…".into();
            s.release = None;
            s.download = None;
            if s.info.install_mode == "appimage" {
                "appimage"
            } else {
                "deb"
            }
        };
        let result = fetch(kind);
        match result {
            Ok((release, latest)) => {
                let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
                s.info.latest_version = Some(latest);
                s.info.phase = if release.is_some() {
                    "available"
                } else {
                    "current"
                }
                .into();
                s.info.message = if release.is_some() {
                    "A newer Matrix Play release is available."
                } else {
                    "You have the latest available version."
                }
                .into();
                s.info.notes = release
                    .as_ref()
                    .map(|r| r.manifest.notes.chars().take(8000).collect())
                    .unwrap_or_default();
                s.release = release;
                Ok(s.info.clone())
            }
            Err(e) => self.fail(e),
        }
    }
    fn begin_download(&self) -> Result<core::VerifiedRelease, String> {
        let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
        if s.info.phase != "available" {
            return Err("Check for an available update first".into());
        }
        let release = s.release.clone().ok_or("No verified release selected")?;
        // Reset the previous attempt before publishing the cancellable phase.
        // A caller observing "downloading" through status() acquires this same
        // mutex, so its cancellation can never be overwritten by this reset.
        self.cancel.store(false, Ordering::Relaxed);
        s.info.phase = "downloading".into();
        s.info.message = "Downloading and verifying the update…".into();
        s.info.received = 0;
        s.info.total = release.asset.size;
        Ok(release)
    }
    fn download(&self) -> Result<Info, String> {
        let release = self.begin_download()?;
        let result = (|| {
            let root = dirs::data_local_dir()
                .ok_or("Cannot locate the update download folder")?
                .join("matrix/updates");
            core::download(
                &mut core::Http::new()?,
                &release,
                &root,
                &self.cancel,
                |received, total| {
                    if let Ok(mut s) = self.inner.lock() {
                        s.info.received = received;
                        s.info.total = total;
                    }
                },
            )
        })();
        match result {
            Ok(download) => {
                let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
                s.download = Some(download);
                s.info.phase = "downloaded".into();
                s.info.message = if s.info.install_mode == "download" {
                    "Verified installer downloaded. Open it to install Matrix Play."
                } else {
                    "Verified update downloaded. Ready to install."
                }
                .into();
                Ok(s.info.clone())
            }
            Err(e) => self.fail(e),
        }
    }
    fn install(&self, helper: &std::path::Path, games_closed: bool) -> Result<Info, String> {
        if !games_closed {
            return Err("Save and close your games and other work. Wait for installers and updates to finish, then confirm before updating.".into());
        }
        let (download, install_mode) = {
            let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
            if s.info.phase != "downloaded" {
                return Err("Download and verify an update first".into());
            }
            let d = s.download.clone().ok_or("No verified installer is ready")?;
            s.info.phase = "installing".into();
            s.info.message =
                "Preparing to close all Matrix Play windows before installation…".into();
            (d, s.info.install_mode.clone())
        };
        let result = (|| {
            let target = match install_mode.as_str() {
                "appimage" => running_appimage().ok_or("Cannot identify this Matrix Play AppImage")?,
                "system" => PathBuf::from("/usr/bin/matrix-frontend"),
                _ => return Err(format!("This development build cannot install automatically. Close every Matrix Play window, then open the verified installer manually: {}", download.path.display())),
            };
            let request = super::update_worker::Request {
                signed_manifest: download.release.signed_manifest,
                signature: download.release.signature,
                package: download.path,
                mode: install_mode,
                target,
                frontend_exe: std::env::current_exe().map_err(|e| e.to_string())?,
                games_closed,
            };
            // Verify before any shutdown, and again in the independent helper.
            request.verified()?;
            let root = super::update_lifecycle::directory()?;
            let staging = tempfile::Builder::new()
                .prefix("coordinator-")
                .tempdir_in(&root)
                .map_err(|e| e.to_string())?;
            let executable = staging.path().join("matrix-update-coordinator");
            std::fs::copy(helper, &executable).map_err(|e|format!("Cannot prepare the update coordinator: {e}. Install this version manually after closing Matrix Play."))?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            let mut command = super::launch_command(&executable);
            let mut child = command
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .current_dir("/")
                .spawn()
                .map_err(|e| format!("Cannot start the update coordinator: {e}"))?;
            let mut input = child.stdin.take().ok_or("Coordinator input unavailable")?;
            let sent = serde_json::to_writer(&mut input, &request).map_err(|e| e.to_string());
            drop(input);
            use std::io::{BufRead, Read};
            let mut reply = String::new();
            let read = std::io::BufReader::new(
                child
                    .stdout
                    .take()
                    .ok_or("Coordinator output unavailable")?,
            )
            .take(16385)
            .read_line(&mut reply)
            .map_err(|e| e.to_string());
            // The temporary executable is independent of the old AppImage mount.
            // Keep it until the coordinator exits; frontend shutdown may orphan it,
            // but the helper has no need to execute it again.
            std::thread::spawn(move || {
                let _ = child.wait();
                drop(staging);
            });
            sent?;
            read?;
            let ready: Result<(), String> = serde_json::from_str(&reply)
                .map_err(|_| "Update coordinator stopped before confirming safe shutdown")?;
            ready?;
            self.status()
        })();
        match result {
            Ok(info) => Ok(info),
            Err(e) => self.fail(e),
        }
    }
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) fn update_status(service: tauri::State<'_, Service>) -> Result<Info, String> {
    service.status()
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) async fn check_updates(service: tauri::State<'_, Service>) -> Result<Info, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.check())
        .await
        .map_err(|e| e.to_string())?
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) async fn download_update(service: tauri::State<'_, Service>) -> Result<Info, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.download())
        .await
        .map_err(|e| e.to_string())?
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) async fn install_update(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    games_closed: bool,
) -> Result<Info, String> {
    use tauri::Manager;
    let helper = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("matrix-update-coordinator");
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.install(&helper, games_closed))
        .await
        .map_err(|e| e.to_string())?
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) fn cancel_update(service: tauri::State<'_, Service>) {
    service.cancel.store(true, Ordering::Relaxed);
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) fn restart_updated(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
) -> Result<(), String> {
    let target = service
        .inner
        .lock()
        .map_err(|_| "Update state unavailable")?
        .installed
        .clone()
        .ok_or("Install an update before restarting")?;
    let mut command = super::launch_command(target);
    command.spawn().map_err(|e|format!("Update is installed, but automatic restart failed: {e}. Open Matrix Play again from your application menu."))?;
    app.exit(0);
    Ok(())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) fn open_update_releases() -> Result<(), String> {
    super::launch_command("xdg-open")
        .arg("https://github.com/relentlesspoetry-coder/Matrix/releases")
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_install_outcome_is_retained_until_explicit_retry() {
        for success in [false, true] {
            let message = "Installation result; review package-manager guidance before retrying.";
            let service = Service::with_outcome(Some(super::super::update_lifecycle::Outcome {
                success,
                message: message.into(),
            }));
            assert_eq!(service.status().unwrap().phase, "recovery");
            assert_eq!(service.status().unwrap().message, message);
            let retried = service
                .check_with(|_| Ok((None, env!("CARGO_PKG_VERSION").into())))
                .unwrap();
            assert_eq!(retried.phase, "current");
            assert_ne!(retried.message, message);
        }
    }

    #[test]
    fn check_again_retries_failed_checks_immediately() {
        let service = Service::default();
        let failed = service
            .check_with(|_| Err("Temporary network failure".into()))
            .unwrap();
        assert_eq!(failed.phase, "error");
        assert!(service.inner.lock().unwrap().last_check.is_none());
        let retried = service
            .check_with(|_| {
                assert_eq!(service.status().unwrap().phase, "checking");
                Ok((None, env!("CARGO_PKG_VERSION").into()))
            })
            .unwrap();
        assert_eq!(retried.phase, "current");
        // Retain throttling after successful requests.
        assert_eq!(
            service
                .check_with(|_| panic!("A successful check should still be cached"))
                .unwrap()
                .phase,
            "current"
        );
    }

    #[test]
    fn check_again_retries_download_cancellation_and_install_errors() {
        for (phase, error) in [
            ("downloading", "Download cancelled"),
            ("downloading", "Download failed"),
            ("installing", "Installation cancelled or failed"),
        ] {
            let service = Service::default();
            {
                let mut runtime = service.inner.lock().unwrap();
                runtime.last_check = Some(Instant::now());
                runtime.info.phase = phase.into();
            }
            // Busy operations must not be replaced by a concurrent check.
            assert_eq!(
                service
                    .check_with(|_| panic!("Do not interrupt an active operation"))
                    .unwrap()
                    .phase,
                phase
            );
            service.fail(error.into()).unwrap();
            let retried = service
                .check_with(|_| Ok((None, env!("CARGO_PKG_VERSION").into())))
                .unwrap();
            assert_eq!(retried.phase, "current", "{error}");
        }
    }

    #[test]
    fn concurrent_checks_only_fetch_once_and_expired_checks_fetch_again() {
        let service = Service::default();
        service
            .check_with(|_| {
                assert_eq!(
                    service
                        .check_with(|_| panic!("A check is already in progress"))
                        .unwrap()
                        .phase,
                    "checking"
                );
                Ok((None, env!("CARGO_PKG_VERSION").into()))
            })
            .unwrap();
        service.inner.lock().unwrap().last_check = Some(Instant::now() - Duration::from_secs(61));
        let mut fetched = false;
        service
            .check_with(|_| {
                fetched = true;
                Ok((None, env!("CARGO_PKG_VERSION").into()))
            })
            .unwrap();
        assert!(fetched);
    }

    #[test]
    fn cancellation_after_downloading_is_published_is_not_lost() {
        let service = Service::default();
        let asset = core::Asset {
            name: "fixture.deb".into(),
            size: 14,
            sha256: "0".repeat(64),
        };
        // This fixture exercises the state transition only, never verification,
        // network access or installation.
        {
            let mut runtime = service.inner.lock().unwrap();
            runtime.info.phase = "available".into();
            runtime.release = Some(core::VerifiedRelease {
                manifest: core::Manifest {
                    schema: 1,
                    repository: core::REPOSITORY.into(),
                    version: "99.0.0".into(),
                    tag: "v99.0.0".into(),
                    notes: String::new(),
                    assets: std::collections::BTreeMap::new(),
                },
                asset,
                kind: "deb".into(),
                signed_manifest: Vec::new(),
                signature: Vec::new(),
            });
        }
        service.cancel.store(true, Ordering::Relaxed);
        let worker_service = service.clone();
        let (published, observed) = std::sync::mpsc::channel();
        let (resume, cancelled) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            worker_service.begin_download().unwrap();
            assert!(!worker_service.cancel.load(Ordering::Relaxed));
            published.send(()).unwrap();
            cancelled.recv().unwrap();
            assert!(worker_service.cancel.load(Ordering::Relaxed));
        });
        observed.recv().unwrap();
        assert_eq!(service.status().unwrap().phase, "downloading");
        service.cancel.store(true, Ordering::Relaxed);
        // A concurrent duplicate Download must not reset a pending cancellation.
        assert!(service.begin_download().is_err());
        assert!(service.cancel.load(Ordering::Relaxed));
        resume.send(()).unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn install_without_closed_games_confirmation_never_starts_helper() {
        let service = Service::default();
        service.inner.lock().unwrap().info.phase = "downloaded".into();
        assert!(service
            .install(std::path::Path::new("not-executed"), false)
            .unwrap_err()
            .contains("Save and close"));
        assert_eq!(service.status().unwrap().phase, "downloaded");
    }

    #[test]
    fn inherited_appimage_cannot_select_another_app_for_replacement() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("Matrix.AppImage");
        std::fs::write(&image, b"fixture").unwrap();
        let mount = root.path().join("mount");
        std::fs::create_dir_all(mount.join("usr/bin")).unwrap();
        let exe = mount.join("usr/bin/matrix-frontend");
        std::fs::write(&exe, b"fixture").unwrap();
        assert_eq!(identify_appimage(&image, &mount, &exe), Some(image.clone()));
        let outside = root.path().join("matrix-frontend");
        std::fs::write(&outside, b"fixture").unwrap();
        assert!(identify_appimage(&image, &mount, &outside).is_none());
        let wrong = mount.join("usr/bin/other-app");
        std::fs::write(&wrong, b"fixture").unwrap();
        assert!(identify_appimage(&image, &mount, &wrong).is_none());
        #[cfg(unix)]
        {
            let link = mount.join("matrix-frontend");
            std::os::unix::fs::symlink(&outside, &link).unwrap();
            assert!(identify_appimage(&image, &mount, &link).is_none());
        }
    }
}
