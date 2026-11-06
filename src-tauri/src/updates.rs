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

#[derive(Clone, Serialize)]
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
    let image = image.canonicalize().ok()?;
    let appdir = appdir.canonicalize().ok()?;
    let exe = exe.canonicalize().ok()?;
    // A launcher can inherit APPIMAGE from another application. Only our own
    // mounted/extracted AppImage may be replaced by this process.
    if image.is_file() && exe.starts_with(&appdir) && exe.file_name()? == "matrix-frontend" {
        Some(image)
    } else {
        None
    }
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
        Self {
            inner: Arc::new(Mutex::new(Runtime {
                info: Info {
                    current_version: env!("CARGO_PKG_VERSION").into(),
                    latest_version: None,
                    phase: "idle".into(),
                    message: "Check GitHub for the newest Matrix Play release.".into(),
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
    fn install(&self) -> Result<Info, String> {
        let (download, install_mode) = {
            let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
            if s.info.phase != "downloaded" {
                return Err("Download and verify an update first".into());
            }
            let d = s.download.clone().ok_or("No verified installer is ready")?;
            s.info.phase = "installing".into();
            s.info.message = "Installing update…".into();
            (d, s.info.install_mode.clone())
        };
        let result = (|| {
            core::validate_file(
                &download.path,
                &download.release.asset,
                &download.release.kind,
            )?;
            if !core::is_newer(
                &download.release.manifest.version,
                env!("CARGO_PKG_VERSION"),
            )? {
                return Err("This update is no longer newer".into());
            }
            if install_mode == "appimage" {
                let target = running_appimage().ok_or("Cannot identify the running Matrix Play AppImage. Use the downloaded installer manually.")?;
                return core::install_appimage(&download, &target).map(Some);
            }
            if install_mode == "system" {
                let mut child = super::launch_command("/usr/bin/pkexec")
                    .arg("/usr/lib/matrix-play/matrix-install-update")
                    .stdin(std::process::Stdio::piped())
                    .current_dir("/")
                    .spawn()
                    .map_err(|_| "Could not open the secure system installer. Install the update manually if this installation predates the secure helper.")?;
                let sent = core::deb::write_request(
                    &download,
                    &mut child.stdin.take().ok_or("Installer input unavailable")?,
                );
                // Always reap the helper, including a rejected/truncated request.
                let status = child.wait().map_err(|e| e.to_string())?;
                sent?;
                if !status.success() {
                    return Err("Installation was cancelled or the system installer could not complete it. Your current Matrix Play remains available.".into());
                }
                return Ok(Some(PathBuf::from("/usr/bin/matrix-frontend")));
            }
            super::launch_command("xdg-open")
                .arg(&download.path)
                .spawn()
                .map_err(|e| format!("Cannot open the downloaded installer: {e}"))?;
            Ok(None)
        })();
        match result {
            Ok(installed) => {
                let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
                s.installed = installed;
                s.info.phase = if s.installed.is_some() {
                    "installed"
                } else {
                    "downloaded"
                }
                .into();
                s.info.message = if s.installed.is_some() {
                    "Update installed. Restart Matrix Play to finish."
                } else {
                    "The system installer has been opened. Complete installation there."
                }
                .into();
                Ok(s.info.clone())
            }
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
pub(crate) async fn install_update(service: tauri::State<'_, Service>) -> Result<Info, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.install())
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
    if std::env::var_os("APPIMAGE").is_some() {
        command
            .env_remove("APPDIR")
            .env_remove("APPIMAGE")
            .env_remove("ARGV0");
        if let Some(original) = std::env::var_os("LD_LIBRARY_PATH_ORIG") {
            command.env("LD_LIBRARY_PATH", original);
        } else {
            command.env_remove("LD_LIBRARY_PATH");
        }
    }
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
