pub mod identity;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const MAX_BYTES: usize = 4 * 1024 * 1024;
const INTERVAL: u64 = 3; // Integer-second clock: three ticks ensure at least two elapsed seconds.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn game_key(kind: &str, path: &Path) -> String {
    hash(&format!("{kind}\0{}", path.to_string_lossy()))
}
fn stamp(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let mut value = format!(
        "{}:{:?}",
        meta.len(),
        meta.modified().map_err(|e| e.to_string())?
    );
    // A CUE's track data can change without modifying its descriptor.
    if path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("cue"))
        && meta.len() <= 65536
    {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        for line in text
            .lines()
            .filter(|l| l.trim().to_ascii_uppercase().starts_with("FILE "))
        {
            if let Some(name) = line.split('"').nth(1) {
                let root = path
                    .parent()
                    .ok_or("Missing parent")?
                    .canonicalize()
                    .map_err(|e| e.to_string())?;
                let track = root.join(name).canonicalize().map_err(|e| e.to_string())?;
                if !track.starts_with(&root) {
                    return Err("CUE track outside game folder".into());
                }
                let m = fs::metadata(track).map_err(|e| e.to_string())?;
                value.push_str(&format!(
                    ":{}:{:?}",
                    m.len(),
                    m.modified().map_err(|e| e.to_string())?
                ));
            }
        }
    }
    Ok(value)
}

pub fn supports(kind: &str) -> bool {
    matches!(kind, "ps1" | "ps2" | "gamecube" | "wii")
}

pub fn url(kind: &str, serial: &str, style: &str) -> Result<String, String> {
    let serial = identity::normalize_for(kind, serial)?;
    if matches!(kind, "gamecube" | "wii") {
        let folder = match style {
            "default" => "cover",
            "3d" => "cover3D",
            _ => return Err("Unsupported cover style".into()),
        };
        let region = identity::nintendo_region(&serial)?;
        return Ok(format!(
            "https://art.gametdb.com/wii/{folder}/{region}/{serial}.png"
        ));
    }
    let repo = match kind {
        "ps1" => "psx-covers",
        "ps2" => "ps2-covers",
        _ => return Err("Unsupported cover console".into()),
    };
    let ext = match style {
        "default" => "jpg",
        "3d" => "png",
        _ => return Err("Unsupported cover style".into()),
    };
    Ok(format!(
        "https://raw.githubusercontent.com/xlenore/{repo}/main/covers/{style}/{serial}.{ext}"
    ))
}

/// Decode content with strict dimension bounds; extensions and Content-Type are not trusted.
pub fn validate_image(bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("Cover exceeds the 4 MB limit".into());
    }
    let format = image::guess_format(bytes).map_err(|_| "Response is not an image")?;
    let ext = match format {
        image::ImageFormat::Png => "png",
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::WebP => "webp",
        _ => return Err("Unsupported image format".into()),
    };
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(80 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|e| format!("Invalid cover image: {e}"))?;
    Ok(ext)
}

pub struct Reply {
    pub status: u16,
    pub bytes: Vec<u8>,
    pub retry_after: Option<u64>,
}
pub trait Transport {
    fn get(&mut self, url: &str) -> Result<Reply, String>;
}
pub struct Http;
impl Transport for Http {
    fn get(&mut self, url: &str) -> Result<Reply, String> {
        // URL comes exclusively from url(); arbitrary remote URLs are never accepted.
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Matrix/0.5 cover-downloader")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;
        let response = client.get(url).send().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| {
                s.parse::<u64>().ok().or_else(|| {
                    httpdate::parse_http_date(s).ok().map(|t| {
                        t.duration_since(SystemTime::now())
                            .unwrap_or_default()
                            .as_secs()
                    })
                })
            });
        if response
            .content_length()
            .is_some_and(|n| n > MAX_BYTES as u64)
        {
            return Err("Cover exceeds the 4 MB limit".into());
        }
        let mut bytes = Vec::new();
        if status == 200 {
            response
                .take((MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
        }
        Ok(Reply {
            status,
            bytes,
            retry_after,
        })
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub status: String,
    pub message: String,
    pub serial: Option<String>,
    pub source: Option<String>,
    pub url: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Identity {
    stamp: String,
    serial: String,
    source: String,
}
#[derive(Serialize, Deserialize)]
struct Cached {
    outcome: Outcome,
    retry_at: u64,
    extension: Option<String>,
}
#[derive(Serialize, Deserialize, Default)]
struct Index {
    #[serde(default)]
    identities: BTreeMap<String, Identity>,
    #[serde(default)]
    entries: BTreeMap<String, Cached>,
    #[serde(default)]
    next_request: u64,
}
fn load(root: &Path) -> Result<Index, String> {
    match fs::read(root.join("index.json")) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| format!("Invalid artwork index: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Index::default()),
        Err(e) => Err(e.to_string()),
    }
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("Invalid cover path")?).map_err(|e| e.to_string())?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}
fn save(root: &Path, index: &Index) -> Result<(), String> {
    atomic(
        &root.join("index.json"),
        &serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?,
    )
}
fn entry_key(kind: &str, serial: &str, style: &str) -> String {
    format!("{kind}/{serial}/{style}")
}
fn image_path(root: &Path, key: &str, ext: &str) -> PathBuf {
    root.join(format!("{}.{}", hash(key), ext))
}
fn valid_cached_image(path: &Path) -> bool {
    fs::metadata(path)
        .ok()
        .is_some_and(|m| m.is_file() && m.len() <= MAX_BYTES as u64)
        && fs::read(path)
            .ok()
            .is_some_and(|b| validate_image(&b).is_ok())
}
fn valid_identity<'a>(index: &'a Index, kind: &str, path: &Path) -> Option<&'a Identity> {
    let i = index.identities.get(&game_key(kind, path))?;
    if i.stamp != stamp(path).ok()? {
        return None;
    }
    Some(i)
}
/// Lookup only: never performs networking, even while online artwork is disabled.
pub fn cached_cover(root: &Path, kind: &str, path: &Path, style: &str) -> Option<PathBuf> {
    let index = load(root).ok()?;
    let i = valid_identity(&index, kind, path)?;
    url(kind, &i.serial, style).ok()?;
    let key = entry_key(kind, &i.serial, style);
    let e = index.entries.get(&key)?;
    let ext = e.extension.as_deref()?;
    if !["png", "jpg", "webp"].contains(&ext) {
        return None;
    }
    let path = image_path(root, &key, ext);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

pub fn fetch(
    root: &Path,
    kind: &str,
    path: &Path,
    style: &str,
    confirmed: Option<&str>,
    transport: &mut impl Transport,
) -> Result<Outcome, String> {
    // Reject unsupported console/style before creating files or doing any I/O with the provider.
    if !supports(kind) || !matches!(style, "default" | "3d") {
        return Err("Unsupported cover console or style".into());
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("index.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock_exclusive().map_err(|e| e.to_string())?; // Also serializes multiple Matrix processes.
    let mut index = load(root)?;
    let stamp = stamp(path)?;
    let (serial, source) = if let Some(input) = confirmed {
        (
            identity::normalize_for(kind, input)?,
            "user-confirmed".to_string(),
        )
    } else if let Some(i) =
        valid_identity(&index, kind, path).filter(|i| i.source == "user-confirmed")
    {
        (i.serial.clone(), i.source.clone())
    } else {
        match identity::detect(path, kind) {
            Ok(i) => i,
            Err(message) => {
                return Ok(Outcome {
                    status: "needs-serial".into(),
                    message,
                    ..Default::default()
                })
            }
        }
    };
    let source_url = url(kind, &serial, style)?;
    index.identities.insert(
        game_key(kind, path),
        Identity {
            stamp,
            serial: serial.clone(),
            source: source.clone(),
        },
    );
    let key = entry_key(kind, &serial, style);
    if let Some(cached) = index.entries.get(&key) {
        if let Some(ext) = &cached.extension {
            if ["png", "jpg", "webp"].contains(&ext.as_str())
                && valid_cached_image(&image_path(root, &key, ext))
            {
                let outcome = Outcome {
                    status: "cached".into(),
                    message: "Saved cover available offline".into(),
                    source: Some(source),
                    ..cached.outcome.clone()
                };
                save(root, &index)?;
                return Ok(outcome);
            }
        } else if cached.retry_at > now() {
            let out = Outcome {
                source: Some(source),
                ..cached.outcome.clone()
            };
            save(root, &index)?;
            return Ok(out);
        }
    }
    let mut outcome = Outcome {
        status: "downloaded".into(),
        message: "Cover saved for offline use".into(),
        serial: Some(serial),
        source: Some(source),
        url: Some(source_url.clone()),
    };
    let current = now();
    if index.next_request > current + INTERVAL {
        outcome.status = "deferred".into();
        outcome.message = "Provider cooling down; try a later library refresh".into();
        save(root, &index)?;
        return Ok(outcome);
    }
    if index.next_request > current {
        std::thread::sleep(Duration::from_secs(index.next_request - current));
    }
    index.next_request = now() + INTERVAL;
    save(root, &index)?;
    let response = transport.get(&source_url);
    let mut extension = None;
    let mut retry_at = now() + 900;
    match response {
        Ok(reply) if reply.status == 200 => match validate_image(&reply.bytes) {
            Ok(ext) => {
                atomic(&image_path(root, &key, ext), &reply.bytes)?;
                extension = Some(ext.into());
                retry_at = 0;
            }
            Err(message) => {
                outcome.status = "invalid-image".into();
                outcome.message = message;
            }
        },
        Ok(reply) => {
            outcome.status = if reply.status == 404 {
                "not-found"
            } else {
                "deferred"
            }
            .into();
            outcome.message = if reply.status == 404 {
                "No cover for this exact console, serial and style. Choose a local cover or another style.".into()
            } else {
                format!(
                    "Provider returned HTTP {}; try later or choose a local cover",
                    reply.status
                )
            };
            if reply.status == 404 {
                retry_at = now() + 7 * 86400;
            }
            if [403, 429, 503].contains(&reply.status) {
                let delay = reply
                    .retry_after
                    .unwrap_or(if reply.status == 403 { 86400 } else { 3600 })
                    .max(60);
                index.next_request = now().saturating_add(delay);
                retry_at = index.next_request;
            }
        }
        Err(error) => {
            outcome.status = "offline".into();
            outcome.message = format!("Cover download unavailable: {error}");
            index.next_request = now() + 60;
        }
    }
    index.entries.insert(
        key,
        Cached {
            outcome: outcome.clone(),
            retry_at,
            extension,
        },
    );
    save(root, &index)?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        reply: Option<Result<Reply, String>>,
        urls: Vec<String>,
    }
    impl Transport for Fake {
        fn get(&mut self, url: &str) -> Result<Reply, String> {
            self.urls.push(url.into());
            self.reply.take().expect("Unexpected network request")
        }
    }
    fn fake(status: u16) -> Fake {
        Fake {
            reply: Some(Ok(Reply {
                status,
                bytes: include_bytes!("../../tests/fixtures/demo-cover.png").to_vec(),
                retry_after: None,
            })),
            urls: vec![],
        }
    }
    fn game(tmp: &Path, name: &str) -> PathBuf {
        let p = tmp.join(name);
        fs::write(&p, b"non-proprietary fixture").unwrap();
        p
    }
    fn release_cooldown(root: &Path) {
        let mut index = load(root).unwrap();
        index.next_request = 0;
        save(root, &index).unwrap();
    }
    #[test]
    fn exact_templates_console_and_style_boundaries() {
        assert_eq!(url("ps1","SLUS-00664","default").unwrap(),"https://raw.githubusercontent.com/xlenore/psx-covers/main/covers/default/SLUS-00664.jpg");
        assert_eq!(
            url("ps2", "slus_203.12", "3d").unwrap(),
            "https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/3d/SLUS-20312.png"
        );
        assert!(url("steam", "SLUS-00664", "default").is_err());
        assert!(url("ps2", "../../SLUS-00664", "3d").is_err());
        assert!(url("ps1", "SLUS-00664", "../default").is_err());
    }
    #[test]
    fn gametdb_templates_preserve_console_region_and_style() {
        assert_eq!(
            url("gamecube", "GMSE01", "default").unwrap(),
            "https://art.gametdb.com/wii/cover/US/GMSE01.png"
        );
        assert_eq!(
            url("wii", "RMGJ01", "3d").unwrap(),
            "https://art.gametdb.com/wii/cover3D/JA/RMGJ01.png"
        );
        assert_eq!(
            url("wii", "RMGP01", "default").unwrap(),
            "https://art.gametdb.com/wii/cover/EN/RMGP01.png"
        );
        assert!(url("gamecube", "RMGE01", "default").is_err());
        assert!(url("wii", "GMSE01", "default").is_err());
        assert!(url("wii", "RMGE01", "../../").is_err());
        assert!(url("wii", "RMGZ01", "default").is_err());
    }
    #[test]
    fn nintendo_download_confirmation_cache_and_invalidation() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        let path = game(tmp.path(), "Unknown.rvz");
        let mut absent = Fake {
            reply: None,
            urls: vec![],
        };
        assert_eq!(
            fetch(&root, "wii", &path, "default", None, &mut absent)
                .unwrap()
                .status,
            "needs-serial"
        );
        assert!(fetch(&root, "wii", &path, "default", Some("GMSE01"), &mut absent).is_err());
        let mut http = fake(200);
        assert_eq!(
            fetch(&root, "wii", &path, "default", Some("RMGE01"), &mut http)
                .unwrap()
                .status,
            "downloaded"
        );
        assert!(cached_cover(&root, "wii", &path, "default").is_some());
        assert!(cached_cover(&root, "gamecube", &path, "default").is_none());
        assert!(cached_cover(&root, "wii", &path, "3d").is_none());
        assert_eq!(
            fetch(&root, "wii", &path, "default", None, &mut absent)
                .unwrap()
                .status,
            "cached"
        );
        fs::write(&path, b"changed fixture").unwrap();
        assert!(cached_cover(&root, "wii", &path, "default").is_none());
        assert_eq!(
            fetch(&root, "wii", &path, "default", None, &mut absent)
                .unwrap()
                .status,
            "needs-serial"
        );
        assert!(absent.urls.is_empty());
    }
    #[test]
    #[ignore = "Explicit live GameTDB smoke test; downloads four temporary covers"]
    fn live_gametdb_templates() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        for (kind, serial) in [("gamecube", "GMSE01"), ("wii", "RMGE01")] {
            let path = game(tmp.path(), &format!("{serial}.rvz"));
            for style in ["default", "3d"] {
                let out = fetch(&root, kind, &path, style, None, &mut Http).unwrap();
                assert_eq!(out.status, "downloaded", "{kind} {style}: {}", out.message);
                assert!(cached_cover(&root, kind, &path, style).is_some());
            }
        }
    }
    #[test]
    fn download_persists_and_restart_needs_no_network() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        let path = game(tmp.path(), "Game [SLUS-20312].chd");
        let mut http = fake(200);
        let result = fetch(&root, "ps2", &path, "default", None, &mut http).unwrap();
        assert_eq!(result.status, "downloaded");
        assert!(cached_cover(&root, "ps2", &path, "default")
            .unwrap()
            .is_file());
        let mut offline = Fake {
            reply: None,
            urls: vec![],
        };
        assert_eq!(
            fetch(&root, "ps2", &path, "default", None, &mut offline)
                .unwrap()
                .status,
            "cached"
        );
        assert!(offline.urls.is_empty());
        assert!(cached_cover(&root, "ps1", &path, "default").is_none());
        assert!(cached_cover(&root, "ps2", &path, "3d").is_none());
        fs::write(&path, b"changed game fixture length").unwrap();
        assert!(cached_cover(&root, "ps2", &path, "default").is_none());
    }
    #[test]
    fn no_request_without_unique_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let mut http = Fake {
            reply: None,
            urls: vec![],
        };
        let root = tmp.path().join("covers");
        for name in ["Game.iso", "SLUS-20312 SLES-12345.chd"] {
            let p = game(tmp.path(), name);
            assert_eq!(
                fetch(&root, "ps2", &p, "default", None, &mut http)
                    .unwrap()
                    .status,
                "needs-serial"
            );
        }
        assert!(http.urls.is_empty());
    }
    #[test]
    fn negative_cache_is_persistent_and_style_specific() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        let path = game(tmp.path(), "SLUS-20312.chd");
        let mut http = fake(404);
        assert_eq!(
            fetch(&root, "ps2", &path, "3d", None, &mut http)
                .unwrap()
                .status,
            "not-found"
        );
        let mut absent = Fake {
            reply: None,
            urls: vec![],
        };
        assert_eq!(
            fetch(&root, "ps2", &path, "3d", None, &mut absent)
                .unwrap()
                .status,
            "not-found"
        );
        assert!(cached_cover(&root, "ps2", &path, "3d").is_none());
        release_cooldown(&root);
        let mut found = fake(200);
        assert_eq!(
            fetch(&root, "ps2", &path, "default", None, &mut found)
                .unwrap()
                .status,
            "downloaded"
        );
    }
    #[test]
    fn retry_after_stops_requests_across_games_and_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        let path = game(tmp.path(), "SLUS-20312.chd");
        let mut http = fake(429);
        http.reply.as_mut().unwrap().as_mut().unwrap().retry_after = Some(7200);
        assert_eq!(
            fetch(&root, "ps2", &path, "default", None, &mut http)
                .unwrap()
                .status,
            "deferred"
        );
        assert!(load(&root).unwrap().next_request >= now() + 7199);
        let other = game(tmp.path(), "SLUS-00664.chd");
        let mut none = Fake {
            reply: None,
            urls: vec![],
        };
        assert_eq!(
            fetch(&root, "ps1", &other, "default", None, &mut none)
                .unwrap()
                .status,
            "deferred"
        );
        assert!(none.urls.is_empty());
    }
    #[test]
    fn invalid_and_oversized_images_never_enter_cache() {
        for bytes in [
            b"<html>challenge</html>".to_vec(),
            b"\x89PNG\r\n\x1a\ntruncated".to_vec(),
            vec![0; MAX_BYTES + 1],
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path().join("covers");
            let path = game(tmp.path(), "SLUS-20312.chd");
            let mut http = fake(200);
            http.reply.as_mut().unwrap().as_mut().unwrap().bytes = bytes;
            assert_eq!(
                fetch(&root, "ps2", &path, "default", None, &mut http)
                    .unwrap()
                    .status,
                "invalid-image"
            );
            assert!(cached_cover(&root, "ps2", &path, "default").is_none());
        }
        let mut oversized = vec![0u8; 24];
        oversized[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        oversized[12..16].copy_from_slice(b"IHDR");
        oversized[16..20].copy_from_slice(&9000u32.to_be_bytes());
        oversized[20..24].copy_from_slice(&9000u32.to_be_bytes());
        assert!(validate_image(&oversized).is_err());
    }
    #[test]
    fn manual_confirmation_is_persistent_but_expires_on_file_change() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        let path = game(tmp.path(), "Unknown.chd");
        let mut http = fake(200);
        let out = fetch(
            &root,
            "ps2",
            &path,
            "default",
            Some("SLUS-20312"),
            &mut http,
        )
        .unwrap();
        assert_eq!(out.source.as_deref(), Some("user-confirmed"));
        let mut none = Fake {
            reply: None,
            urls: vec![],
        };
        assert_eq!(
            fetch(&root, "ps2", &path, "default", None, &mut none)
                .unwrap()
                .status,
            "cached"
        );
        fs::write(&path, b"different game").unwrap();
        assert_eq!(
            fetch(&root, "ps2", &path, "default", None, &mut none)
                .unwrap()
                .status,
            "needs-serial"
        );
    }
    #[test]
    fn offline_error_is_bounded_and_keeps_existing_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        let path = game(tmp.path(), "SLUS-20312.chd");
        let mut http = Fake {
            reply: Some(Err("offline".into())),
            urls: vec![],
        };
        assert_eq!(
            fetch(&root, "ps2", &path, "default", None, &mut http)
                .unwrap()
                .status,
            "offline"
        );
        assert!(load(&root).unwrap().next_request >= now() + 59);
    }
    #[test]
    fn corrupted_cached_image_is_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        let path = game(tmp.path(), "SLUS-20312.chd");
        fetch(&root, "ps2", &path, "default", None, &mut fake(200)).unwrap();
        fs::write(
            cached_cover(&root, "ps2", &path, "default").unwrap(),
            b"broken",
        )
        .unwrap();
        release_cooldown(&root);
        let mut http = fake(200);
        assert_eq!(
            fetch(&root, "ps2", &path, "default", None, &mut http)
                .unwrap()
                .status,
            "downloaded"
        );
        assert_eq!(http.urls.len(), 1);
    }
    #[test]
    #[ignore = "Explicit live GitHub smoke test; downloads four temporary covers"]
    fn live_provider_templates() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        for (kind, serial) in [("ps1", "SLUS-00664"), ("ps2", "SLUS-20312")] {
            let path = game(tmp.path(), &format!("{serial}.chd"));
            for style in ["default", "3d"] {
                let out = fetch(&root, kind, &path, style, None, &mut Http).unwrap();
                assert_eq!(out.status, "downloaded", "{kind} {style}: {}", out.message);
                assert!(cached_cover(&root, kind, &path, style).is_some());
            }
        }
    }
}
