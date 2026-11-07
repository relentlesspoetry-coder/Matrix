//! Standalone unprivileged coordinator; no GTK/WebKit or old AppImage mount.
use super::update_lifecycle as lifecycle;
use matrix_update_core as core;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub signed_manifest: Vec<u8>,
    pub signature: Vec<u8>,
    pub package: PathBuf,
    pub mode: String,
    pub target: PathBuf,
    pub frontend_exe: PathBuf,
    pub games_closed: bool,
}
impl Request {
    pub fn verified(&self) -> Result<core::Downloaded, String> {
        if !self.games_closed {
            return Err("Save and close your games and other work. Wait for installers and updates to finish, then confirm before updating.".into());
        }
        if self.signed_manifest.len() > 65536 || self.signature.len() != 64 {
            return Err("Invalid signed update metadata".into());
        }
        let manifest: core::Manifest =
            serde_json::from_slice(&self.signed_manifest).map_err(|e| e.to_string())?;
        let kind = match self.mode.as_str() { "appimage" => "appimage", "system" => "deb", _ => return Err("Automatic installation is unavailable for this build. Close Matrix Play and open the downloaded installer manually.".into()) };
        if self.mode == "system" && self.target != std::path::Path::new("/usr/bin/matrix-frontend")
        {
            return Err("Invalid system update destination".into());
        }
        let release = core::verify_manifest(
            &self.signed_manifest,
            &self.signature,
            &manifest.tag,
            env!("CARGO_PKG_VERSION"),
            kind,
        )?;
        core::validate_file(&self.package, &release.asset, kind)?;
        Ok(core::Downloaded {
            release,
            path: self.package.clone(),
        })
    }
}
fn install(request: &Request, download: &core::Downloaded) -> Result<(), String> {
    if request.mode == "appimage" {
        core::install_appimage(download, &request.target)?;
        return Ok(());
    }
    let mut child = Command::new("/usr/bin/pkexec")
        .arg("/usr/lib/matrix-play/matrix-install-update")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .current_dir("/")
        .spawn()
        .map_err(|e| format!("Could not open the secure system installer: {e}"))?;
    let sent = core::deb::write_request(
        download,
        &mut child.stdin.take().ok_or("Installer input unavailable")?,
    );
    // Never terminate a privileged installer or release the gate while it runs.
    let status = child.wait().map_err(|e| e.to_string())?;
    sent?;
    if !status.success() {
        return Err("Installation was cancelled or the system installer failed. Reopen Matrix Play and retry; see your system package manager if it reports a partial installation.".into());
    }
    Ok(())
}
pub fn run() -> Result<(), String> {
    // Unlink the private copy once loaded. The process image remains valid,
    // while a frontend exiting early cannot strand a growing pile of helpers.
    if let (Ok(root), Ok(exe)) = (lifecycle::directory(), std::env::current_exe()) {
        if let Some(parent) = exe.parent() {
            if parent.parent() == Some(root.as_path())
                && parent
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("coordinator-"))
            {
                let _ = std::fs::remove_file(&exe);
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(300001)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let prepared = (|| {
        if bytes.len() > 300000 {
            return Err("Update request is too large".into());
        }
        let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let download = request.verified()?;
        let root = lifecycle::directory()?;
        let transaction = lifecycle::Transaction::prepare_with_runtime(
            &root,
            &request.frontend_exe,
            (request.mode == "appimage").then_some(request.target.as_path()),
        )?;
        Ok((request, download, root, transaction))
    })();
    let (request, download, root, transaction) = match prepared {
        Ok(value) => value,
        Err(e) => {
            println!("{}", serde_json::to_string(&Err::<(), _>(&e)).unwrap());
            return Err(e);
        }
    };
    println!("{}", serde_json::to_string(&Ok::<(), String>(())).unwrap());
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let result = transaction
        .shutdown(Duration::from_secs(20))
        .and_then(|_| install(&request, &download));
    let outcome = lifecycle::Outcome {
        success: result.is_ok(),
        message: match &result {
            Ok(()) => format!(
                "Update {} is installed at {}. Matrix Play was closed before installation.",
                download.release.manifest.version,
                request.target.display()
            ),
            Err(e) => e.clone(),
        },
    };
    let saved = lifecycle::save_outcome(&root, &outcome);
    drop(transaction);
    // Reopen once on success or recoverable failure. No game is relaunched.
    let reopened = Command::new(&request.target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir("/")
        .spawn();
    if let Err(error) = reopened {
        return record_reopen_failure(&root, outcome, &error.to_string());
    }
    saved?;
    result
}

fn record_reopen_failure(
    root: &std::path::Path,
    mut outcome: lifecycle::Outcome,
    error: &str,
) -> Result<(), String> {
    outcome.success = false;
    outcome.message = format!("{} Automatic reopening failed: {error}. Open Matrix Play manually. If system installation failed, check your package manager before retrying.", outcome.message);
    lifecycle::save_outcome(root, &outcome)?;
    Err(outcome.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reopen_failure_preserves_install_success_in_persisted_message() {
        let root = tempfile::tempdir().unwrap();
        let outcome = lifecycle::Outcome {
            success: true,
            message: "Update 99.0.0 is installed.".into(),
        };
        assert!(record_reopen_failure(root.path(), outcome, "permission denied").is_err());
        let saved: lifecycle::Outcome =
            serde_json::from_slice(&std::fs::read(root.path().join("last-update.json")).unwrap())
                .unwrap();
        assert!(!saved.success);
        assert!(saved.message.contains("is installed"));
        assert!(saved.message.contains("Open Matrix Play manually"));
        assert!(saved.message.contains("permission denied"));
    }
    #[test]
    fn coordinator_rejects_unconfirmed_unsigned_or_wrong_mode_requests() {
        let mut request = Request {
            signed_manifest: vec![],
            signature: vec![],
            package: PathBuf::from("not-read"),
            mode: "appimage".into(),
            target: PathBuf::from("not-written"),
            frontend_exe: PathBuf::from("not-executed"),
            games_closed: false,
        };
        assert!(request.verified().unwrap_err().contains("Save and close"));
        request.games_closed = true;
        assert!(request.verified().unwrap_err().contains("Invalid signed"));
        request.signature = vec![0; 64];
        request.signed_manifest = serde_json::to_vec(&core::Manifest {
            schema: 1,
            repository: core::REPOSITORY.into(),
            version: "99.0.0".into(),
            tag: "v99.0.0".into(),
            notes: String::new(),
            assets: Default::default(),
        })
        .unwrap();
        assert!(request.verified().is_err());
        request.mode = "download".into();
        assert!(request.verified().unwrap_err().contains("unavailable"));
        request.mode = "system".into();
        assert!(request.verified().unwrap_err().contains("destination"));
    }
}
