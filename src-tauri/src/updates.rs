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
fn mode() -> String {
    if std::env::var_os("APPIMAGE").is_some_and(|p| PathBuf::from(p).is_file()) {
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
        s.info.phase = "error".into();
        s.info.message = error;
        Ok(s.info.clone())
    }
    fn check(&self) -> Result<Info, String> {
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
        let result = (|| core::check(&mut core::Http::new()?, env!("CARGO_PKG_VERSION"), kind))();
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
    fn download(&self) -> Result<Info, String> {
        let release = {
            let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
            if s.info.phase != "available" {
                return Err("Check for an available update first".into());
            }
            let release = s.release.clone().ok_or("No verified release selected")?;
            s.info.phase = "downloading".into();
            s.info.message = "Downloading and verifying the update…".into();
            s.info.received = 0;
            s.info.total = release.asset.size;
            release
        };
        self.cancel.store(false, Ordering::Relaxed);
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
                let target =
                    std::env::var_os("APPIMAGE").ok_or("Cannot locate the running AppImage")?;
                return core::install_appimage(&download, &PathBuf::from(target)).map(Some);
            }
            if install_mode == "system" {
                let status=super::launch_command("pkexec").arg("/usr/bin/apt-get").arg("install").arg("-y").arg(&download.path).current_dir("/").status().map_err(|_|"Could not open the system installer. The verified installer is saved in your Matrix Play update folder.")?;
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
