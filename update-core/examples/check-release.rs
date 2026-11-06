//! Real public-release smoke check; never installs or launches a package.
use matrix_update_core::{check, download, Http};
use std::sync::atomic::AtomicBool;
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let current = args.get(1).map(String::as_str).unwrap_or("0.5.2");
    let kind = args.get(2).map(String::as_str).unwrap_or("deb");
    let mut http = Http::new()?;
    let (release, latest) = check(&mut http, current, kind)?;
    if let Some(release) = release {
        println!(
            "Verified signed manifest for {} ({})",
            release.manifest.version, kind
        );
        if args.get(3).is_some_and(|s| s == "--download") {
            let root = tempfile::tempdir().map_err(|e| e.to_string())?;
            let package = download(
                &mut http,
                &release,
                root.path(),
                &AtomicBool::new(false),
                |_, _| {},
            )?;
            println!(
                "Verified {} bytes and SHA-256: {}",
                release.asset.size,
                package.path.file_name().unwrap().to_string_lossy()
            );
        }
    } else {
        println!("No newer release (latest {latest}, current {current})");
    }
    Ok(())
}
