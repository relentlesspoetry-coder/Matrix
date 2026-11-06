use ed25519_dalek::{Signature, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    future::Future,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub mod deb;

pub const REPOSITORY: &str = "relentlesspoetry-coder/Matrix";
pub const API_URL: &str =
    "https://api.github.com/repos/relentlesspoetry-coder/Matrix/releases?per_page=100";
pub const MAX_PACKAGE: u64 = 512 * 1024 * 1024;
pub const PUBLIC_KEY: &[u8; 32] = include_bytes!("../update-public-key.bin");

#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub body: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub repository: String,
    pub version: String,
    pub tag: String,
    pub notes: String,
    pub assets: BTreeMap<String, Asset>,
}
#[derive(Clone, Debug)]
pub struct VerifiedRelease {
    pub manifest: Manifest,
    pub asset: Asset,
    pub kind: String,
    // Preserve the exact signed bytes for independent privileged verification.
    pub signed_manifest: Vec<u8>,
    pub signature: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct Downloaded {
    pub release: VerifiedRelease,
    pub path: PathBuf,
}

pub fn latest_release(bytes: &[u8]) -> Result<Option<Release>, String> {
    let releases: Vec<Release> =
        serde_json::from_slice(bytes).map_err(|_| "Invalid GitHub release response")?;
    Ok(releases
        .into_iter()
        .filter(|r| !r.draft && r.tag_name.starts_with('v'))
        .filter_map(|r| Version::parse(&r.tag_name[1..]).ok().map(|v| (v, r)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, r)| r))
}
pub fn is_newer(candidate: &str, current: &str) -> Result<bool, String> {
    Ok(
        Version::parse(candidate.strip_prefix('v').unwrap_or(candidate))
            .map_err(|_| "Invalid update version")?
            > Version::parse(current).map_err(|_| "Invalid installed version")?,
    )
}
pub fn asset_url(tag: &str, name: &str) -> Result<String, String> {
    if !tag.starts_with('v')
        || Version::parse(&tag[1..]).is_err()
        || name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("Invalid release asset name".into());
    }
    Ok(format!(
        "https://github.com/{REPOSITORY}/releases/download/{tag}/{name}"
    ))
}
pub fn verify_manifest(
    bytes: &[u8],
    signature: &[u8],
    tag: &str,
    current: &str,
    kind: &str,
) -> Result<VerifiedRelease, String> {
    verify_with_key(bytes, signature, tag, current, kind, PUBLIC_KEY)
}
fn verify_with_key(
    bytes: &[u8],
    signature: &[u8],
    tag: &str,
    current: &str,
    kind: &str,
    key: &[u8; 32],
) -> Result<VerifiedRelease, String> {
    if bytes.len() > 65536 {
        return Err("Update manifest is too large".into());
    }
    let key = VerifyingKey::from_bytes(key).map_err(|_| "Invalid update verification key")?;
    let sig = Signature::from_slice(signature).map_err(|_| "Invalid update signature")?;
    key.verify_strict(bytes, &sig)
        .map_err(|_| "Update signature could not be verified")?;
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| "Invalid signed update manifest")?;
    if manifest.schema != 1
        || manifest.repository != REPOSITORY
        || manifest.tag != tag
        || tag != format!("v{}", manifest.version)
    {
        return Err("Update manifest does not match this application/release".into());
    }
    if !is_newer(&manifest.version, current)? {
        return Err("Update is not newer than this application".into());
    }
    if !["deb", "appimage"].contains(&kind) {
        return Err("Unsupported update package type".into());
    }
    let key = format!("linux-x86_64-{kind}");
    let asset = manifest
        .assets
        .get(&key)
        .ok_or("No update for this Linux architecture/package")?
        .clone();
    let extension = if kind == "deb" { "deb" } else { "AppImage" };
    if asset.name != format!("Matrix-Play_{}_amd64.{extension}", manifest.version)
        || asset.size == 0
        || asset.size > MAX_PACKAGE
        || asset.sha256.len() != 64
        || !asset
            .sha256
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err("Invalid signed update package".into());
    }
    asset_url(tag, &asset.name)?;
    Ok(VerifiedRelease {
        manifest,
        asset,
        kind: kind.into(),
        signed_manifest: bytes.to_vec(),
        signature: signature.to_vec(),
    })
}

pub trait Transport {
    fn small(&mut self, url: &str, limit: usize) -> Result<Vec<u8>, String>;
    /// Stream a package, observing cancellation during connection and body waits.
    /// Stop immediately if the consumer rejects a chunk (for example, its size).
    fn package(
        &mut self,
        url: &str,
        cancel: &AtomicBool,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<(), String>;
}
pub struct Http {
    client: reqwest::Client,
    runtime: Option<tokio::runtime::Runtime>,
}
impl Drop for Http {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            // The OS DNS resolver can be a blocking task that cannot be aborted.
            // Do not let runtime shutdown delay a cancelled download while that
            // lookup finishes. Dropped HTTP futures no longer consume its result.
            runtime.shutdown_background();
        }
    }
}
impl Http {
    pub fn new() -> Result<Self, String> {
        let policy = reqwest::redirect::Policy::custom(|attempt| {
            let allowed = [
                "github.com",
                "api.github.com",
                "release-assets.githubusercontent.com",
                "objects.githubusercontent.com",
            ];
            if attempt.previous().len() >= 5
                || attempt.url().scheme() != "https"
                || !attempt
                    .url()
                    .host_str()
                    .is_some_and(|h| allowed.contains(&h))
            {
                attempt.error("Untrusted update redirect")
            } else {
                attempt.follow()
            }
        });
        let client = reqwest::Client::builder()
            .https_only(true)
            .redirect(policy)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(900))
            .user_agent("Matrix-Play-Updater/1")
            .build()
            .map_err(|e| e.to_string())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            runtime: Some(runtime),
        })
    }
    async fn response(&self, url: &str, timeout: u64) -> Result<reqwest::Response, String> {
        let r = self
            .client
            .get(url)
            .timeout(Duration::from_secs(timeout))
            .send()
            .await
            .map_err(|e| format!("Could not reach GitHub: {e}"))?;
        if !r.status().is_success() {
            return Err(format!(
                "GitHub returned HTTP {}. Try again later.",
                r.status().as_u16()
            ));
        }
        Ok(r)
    }
}
impl Transport for Http {
    fn small(&mut self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        self.runtime
            .as_ref()
            .expect("Http runtime is alive")
            .block_on(async {
                let mut response = self.response(url, 45).await?;
                if response.content_length().is_some_and(|n| n > limit as u64) {
                    return Err("Update metadata is too large".into());
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                    if chunk.len() > limit.saturating_sub(bytes.len()) {
                        return Err("Update metadata is too large".into());
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Ok(bytes)
            })
    }
    fn package(
        &mut self,
        url: &str,
        cancel: &AtomicBool,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        self.runtime
            .as_ref()
            .expect("Http runtime is alive")
            .block_on(until_cancelled(cancel, async {
                let mut response = self.response(url, 900).await?;
                while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                    check_cancelled(cancel)?;
                    consume(&chunk)?;
                }
                Ok(())
            }))
    }
}

fn check_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Update download cancelled".into())
    } else {
        Ok(())
    }
}

async fn until_cancelled<T>(
    cancel: &AtomicBool,
    operation: impl Future<Output = Result<T, String>>,
) -> Result<T, String> {
    let mut operation = std::pin::pin!(operation);
    loop {
        check_cancelled(cancel)?;
        // Timeout borrows the pinned future, so ordinary polling does not restart
        // the request. Cancellation drops it, aborting header/body network waits
        // without a blocked response reader or partially written installer.
        if let Ok(result) = tokio::time::timeout(Duration::from_millis(100), &mut operation).await {
            check_cancelled(cancel)?;
            return result;
        }
    }
}

pub fn check(
    transport: &mut impl Transport,
    current: &str,
    kind: &str,
) -> Result<(Option<VerifiedRelease>, String), String> {
    if std::env::consts::ARCH != "x86_64" {
        return Err("Updates are currently available for Linux x86-64 only".into());
    }
    let Some(release) = latest_release(&transport.small(API_URL, 2 * 1024 * 1024)?)? else {
        return Ok((None, current.into()));
    };
    let version = release.tag_name.trim_start_matches('v').to_string();
    if !is_newer(&version, current)? {
        return Ok((None, version));
    }
    let bytes = transport.small(
        &asset_url(&release.tag_name, "update-manifest.json")?,
        65536,
    )?;
    let sig = transport.small(&asset_url(&release.tag_name, "update-manifest.sig")?, 64)?;
    Ok((
        Some(verify_manifest(
            &bytes,
            &sig,
            &release.tag_name,
            current,
            kind,
        )?),
        version,
    ))
}

pub fn validate_file(path: &Path, asset: &Asset, kind: &str) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() != asset.size {
        return Err("Downloaded update size does not match its signed manifest".into());
    }
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut prefix = Vec::new();
    let mut buf = [0; 65536];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if prefix.len() < 16 {
            prefix.extend_from_slice(&buf[..n.min(16 - prefix.len())]);
        }
        hash.update(&buf[..n]);
    }
    if format!("{:x}", hash.finalize()) != asset.sha256 {
        return Err("Downloaded update checksum does not match its signed manifest".into());
    }
    if (kind == "deb" && !prefix.starts_with(b"!<arch>\n"))
        || (kind == "appimage"
            && (!prefix.starts_with(b"\x7fELF") || prefix.get(8..11) != Some(b"AI\x02")))
    {
        return Err("Downloaded update is not the expected Linux package".into());
    }
    Ok(())
}
pub fn download(
    transport: &mut impl Transport,
    release: &VerifiedRelease,
    root: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<Downloaded, String> {
    check_cancelled(cancel)?;
    let dir = root.join(&release.manifest.version);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(&release.asset.name);
    if validate_file(&target, &release.asset, &release.kind).is_ok() {
        check_cancelled(cancel)?;
        progress(release.asset.size, release.asset.size);
        return Ok(Downloaded {
            release: release.clone(),
            path: target,
        });
    }
    let mut temp = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
    let mut count = 0;
    transport.package(
        &asset_url(&release.manifest.tag, &release.asset.name)?,
        cancel,
        &mut |chunk| {
            check_cancelled(cancel)?;
            if chunk.len() as u64 > release.asset.size.saturating_sub(count) {
                return Err("Update download exceeded its signed size".into());
            }
            count += chunk.len() as u64;
            temp.write_all(chunk).map_err(|e| e.to_string())?;
            progress(count, release.asset.size);
            Ok(())
        },
    )?;
    check_cancelled(cancel)?;
    temp.flush().map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    validate_file(temp.path(), &release.asset, &release.kind)?;
    check_cancelled(cancel)?;
    temp.persist(&target).map_err(|e| e.to_string())?;
    Ok(Downloaded {
        release: release.clone(),
        path: target,
    })
}
pub fn install_appimage(download: &Downloaded, target: &Path) -> Result<PathBuf, String> {
    if download.release.kind != "appimage" {
        return Err("An AppImage update is required".into());
    }
    validate_file(&download.path, &download.release.asset, "appimage")?;
    let target = target.canonicalize().map_err(|e| e.to_string())?;
    let parent = target.parent().ok_or("AppImage location is invalid")?;
    let mut old = File::open(&target).map_err(|e| e.to_string())?;
    let mut header = [0; 11];
    old.read_exact(&mut header).map_err(|e| e.to_string())?;
    if !header.starts_with(b"\x7fELF") || &header[8..11] != b"AI\x02" {
        return Err("The running AppImage could not be identified".into());
    }
    let mut staged=tempfile::NamedTempFile::new_in(parent).map_err(|_|"Cannot write the AppImage folder. The update is downloaded; move it to a writable location.")?;
    std::io::copy(
        &mut File::open(&download.path).map_err(|e| e.to_string())?,
        &mut staged,
    )
    .map_err(|e| e.to_string())?;
    staged.flush().map_err(|e| e.to_string())?;
    validate_file(staged.path(), &download.release.asset, "appimage")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    let backup = parent.join(format!(
        "{}.previous",
        target
            .file_name()
            .ok_or("Invalid AppImage name")?
            .to_string_lossy()
    ));
    let backup_tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    fs::copy(&target, backup_tmp.path()).map_err(|e| e.to_string())?;
    backup_tmp
        .as_file()
        .set_permissions(
            fs::metadata(&target)
                .map_err(|e| e.to_string())?
                .permissions(),
        )
        .map_err(|e| e.to_string())?;
    backup_tmp.as_file().sync_all().map_err(|e| e.to_string())?;
    staged.as_file().sync_all().map_err(|e| e.to_string())?;
    backup_tmp.persist(&backup).map_err(|e| e.to_string())?;
    staged.persist(&target).map_err(|e| e.to_string())?;
    Ok(target)
}

#[cfg(test)]
mod tests;
