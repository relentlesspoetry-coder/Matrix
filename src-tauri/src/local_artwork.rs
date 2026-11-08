//! Fixed local PS5 artwork only: no metadata paths, URLs or game-content reads.
use super::*;
use std::{
    ffi::CString,
    io::Read,
    os::fd::{AsRawFd, FromRawFd},
    os::unix::ffi::OsStrExt,
    path::Component,
};

pub(crate) fn unique_game_folder(game: &Game) -> bool {
    let Some(parent) = Path::new(&game.path).parent() else {
        return false;
    };
    let Ok(s) = spec(&game.kind) else {
        return false;
    };
    let Ok(roots) = libraries::roots(&game.kind) else {
        return false;
    };
    let Ok(parent_path) = parent.canonicalize() else {
        return false;
    };
    if game.kind != "ps5" {
        // Preserve established non-PS5 eligibility exactly. In particular, PS1
        // playlists must not add a second target beside their referenced CHD.
        return !roots.is_empty()
            && !roots.contains(&parent_path)
            && fs::read_dir(parent).ok().is_some_and(|entries| {
                entries
                    .flatten()
                    .filter(|entry| {
                        entry.path().is_file()
                            && entry
                                .path()
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .is_some_and(|ext| {
                                    s.extensions.contains(&ext.to_ascii_lowercase().as_str())
                                })
                    })
                    .take(2)
                    .count()
                    == 1
            });
    }
    !roots.contains(&parent_path)
        && libraries::containing_root(&parent_path, &roots).is_some()
        && fs::read_dir(parent).ok().is_some_and(|entries| {
            // For PS5 only, count actual boot targets including extracted eboots.
            // Do not group away ELF or other independent targets in this folder.
            let mut count = 0;
            for entry in entries {
                let Ok(entry) = entry else {
                    return false;
                };
                let Ok(kind) = entry.file_type() else {
                    return false;
                };
                if kind.is_file() && supported_console_file(s, &entry.path()) {
                    count += 1;
                    if count > 1 {
                        return false;
                    }
                }
            }
            count == 1
        })
}

pub(crate) fn ps5_icon(game: &Game) -> Option<Vec<u8>> {
    let path = Path::new(&game.path);
    if !is_ps5_eboot(&game.kind, path) || !unique_game_folder(game) {
        return None;
    }
    let icon = path.parent()?.join("sce_sys/icon0.png");
    let file = open_regular_without_symlinks(&icon)?;
    let size = file.metadata().ok()?.len();
    if size == 0 || size > MAX_COVER_BYTES as u64 {
        return None;
    }
    let mut bytes = Vec::new();
    // Bound the read itself, not only the earlier metadata check.
    (&file)
        .take(MAX_COVER_BYTES as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if file.metadata().ok()?.len() > MAX_COVER_BYTES as u64 {
        return None;
    }
    cover_format(&bytes).map(|_| bytes)
}

fn open_regular_without_symlinks(path: &Path) -> Option<fs::File> {
    if !path.is_absolute() {
        return None;
    }
    let mut directory = fs::File::open("/").ok()?;
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            if component == Component::RootDir {
                continue;
            }
            return None;
        };
        let name = CString::new(name.as_bytes()).ok()?;
        let last = components.peek().is_none();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | libc::O_NOCTTY
            | if last { 0 } else { libc::O_DIRECTORY };
        // Each component is opened relative to a pinned directory descriptor.
        // O_NOFOLLOW applies to both the icon and all ancestor directories.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return None;
        }
        let opened = unsafe { fs::File::from_raw_fd(fd) };
        if last {
            return opened.metadata().ok()?.is_file().then_some(opened);
        }
        directory = opened;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn ps5_artwork_in_isolated_home() {
        let temp = tempfile::tempdir().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!(
                    "{}::ps5_artwork_fixture",
                    module_path!().split_once("::").unwrap().1
                ),
                "--nocapture",
            ])
            .env("MATRIX_PS5_ARTWORK_FIXTURE", "1")
            .env("HOME", temp.path())
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .env("XDG_DATA_HOME", temp.path().join("data"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }

    #[test]
    fn ps5_artwork_fixture() {
        if std::env::var_os("MATRIX_PS5_ARTWORK_FIXTURE").is_none() {
            return;
        }
        create_layout().unwrap();
        // Existing PS1 folder-cover eligibility counts extension files, not
        // playlists. A valid playlist groups its CHD; an invalid one is omitted.
        let ps1_title = libraries::roots("ps1")
            .unwrap()
            .remove(0)
            .join("Playlist title");
        fs::create_dir_all(&ps1_title).unwrap();
        let disc = ps1_title.join("Game.chd");
        let playlist = ps1_title.join("Game.m3u");
        let png = include_bytes!("../../tests/fixtures/demo-cover.png").to_vec();
        fs::write(&disc, b"synthetic disc").unwrap();
        fs::write(ps1_title.join("cover.png"), &png).unwrap();
        fs::write(&playlist, b"Game.chd\n").unwrap();
        let games = scan_console_impl("ps1").unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].path, playlist.to_string_lossy());
        assert_eq!(
            read_local_cover_bytes("ps1", &games[0].uid, &games[0]).unwrap(),
            Some(png.clone())
        );
        fs::write(&playlist, b"Missing.chd\n").unwrap();
        let games = scan_console_impl("ps1").unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].path, disc.to_string_lossy());
        assert_eq!(
            read_local_cover_bytes("ps1", &games[0].uid, &games[0]).unwrap(),
            Some(png)
        );
        let root = libraries::roots("ps5").unwrap().remove(0);
        let title = root.join("Extracted title");
        let metadata = title.join("sce_sys");
        fs::create_dir_all(&metadata).unwrap();
        let boot = title.join("eboot.bin");
        fs::write(&boot, b"synthetic boot, not playable").unwrap();
        let game = validate_game("ps5", boot.to_str().unwrap()).unwrap();
        let uid = game.uid.clone();
        let read = || read_local_cover_bytes("ps5", &uid, &game).unwrap();
        let png = include_bytes!("../../tests/fixtures/demo-cover.png").to_vec();
        let icon = metadata.join("icon0.png");
        assert_eq!(read(), None);
        fs::write(&icon, &png).unwrap();
        assert_eq!(read(), Some(png.clone()));
        // Sidecar then folder cover then fixed icon; imported covers always win.
        let mut sidecar = png.clone();
        sidecar.extend(b"sidecar");
        let mut folder = png.clone();
        folder.extend(b"folder");
        let mut manual = png.clone();
        manual.extend(b"manual");
        fs::write(title.join("cover.png"), &folder).unwrap();
        assert_eq!(read(), Some(folder.clone()));
        fs::write(title.join("eboot.png"), &sidecar).unwrap();
        assert_eq!(read(), Some(sidecar));
        save_cover("ps5".into(), uid.clone(), manual.clone()).unwrap();
        assert_eq!(read(), Some(manual));
        fs::remove_file(stored_cover_path("ps5", &uid, "png").unwrap()).unwrap();
        fs::remove_file(title.join("eboot.png")).unwrap();
        fs::remove_file(title.join("cover.png")).unwrap();
        favorites::set_favorite("ps5".into(), uid.clone(), true).unwrap();
        // JSON URLs/paths are never interpreted, including when the icon is absent.
        fs::write(
            metadata.join("param.json"),
            br#"{"icon":"/outside.png","url":"https://example.invalid/icon"}"#,
        )
        .unwrap();
        for bad in [
            Vec::new(),
            b"corrupt PNG".to_vec(),
            vec![0; MAX_COVER_BYTES + 1],
        ] {
            fs::write(&icon, bad).unwrap();
            assert_eq!(read(), None);
        }
        fs::remove_file(&icon).unwrap();
        fs::create_dir(&icon).unwrap();
        assert_eq!(read(), None);
        fs::remove_dir(&icon).unwrap();
        let cpath = CString::new(icon.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
        assert_eq!(read(), None);
        fs::remove_file(&icon).unwrap();
        let outside = home().unwrap().join("outside.png");
        fs::write(&outside, &png).unwrap();
        symlink(&outside, &icon).unwrap();
        assert_eq!(read(), None);
        fs::remove_file(&icon).unwrap();
        let original = title.join("original_sce_sys");
        fs::rename(&metadata, &original).unwrap();
        fs::write(original.join("icon0.png"), &png).unwrap();
        symlink(&original, &metadata).unwrap();
        assert_eq!(read(), None);
        fs::remove_file(&metadata).unwrap();
        fs::rename(&original, &metadata).unwrap();
        let outside_dir = home().unwrap().join("outside_sce_sys");
        fs::rename(&metadata, &outside_dir).unwrap();
        symlink(&outside_dir, &metadata).unwrap();
        assert_eq!(read(), None);
        fs::remove_file(&metadata).unwrap();
        fs::rename(&outside_dir, &metadata).unwrap();
        let mut max_png = png.clone();
        max_png.resize(MAX_COVER_BYTES, 0);
        fs::write(&icon, &max_png).unwrap();
        assert_eq!(read(), Some(max_png.clone()));
        max_png.push(0);
        fs::write(&icon, &max_png).unwrap();
        assert_eq!(read(), None);
        fs::write(&icon, &png).unwrap();
        // Multiple actual boot targets must not share a folder/icon fallback.
        fs::write(title.join("other.elf"), b"other synthetic game").unwrap();
        fs::write(title.join("cover.png"), &folder).unwrap();
        assert_eq!(read(), None);
        assert_eq!(scan_console_impl("ps5").unwrap().len(), 2);
        fs::remove_file(title.join("other.elf")).unwrap();
        assert_eq!(read(), Some(folder));
        fs::remove_file(title.join("cover.png")).unwrap();
        // Exact platform and boot filename isolation, never ELF/ZAR/package artwork.
        for kind in SPECS.iter().map(|s| s.id).filter(|id| *id != "ps5") {
            let mut other = game.clone();
            other.kind = kind.into();
            assert_eq!(ps5_icon(&other), None, "{kind}");
        }
        for name in ["game.elf", "game.zar", "unrelated.bin", "game.pkg"] {
            let mut other = game.clone();
            other.path = title.join(name).to_string_lossy().into();
            assert_eq!(ps5_icon(&other), None, "{name}");
        }
        assert_eq!(read(), Some(png.clone()));
        let rescanned = scan_console_impl("ps5").unwrap();
        assert_eq!(rescanned.len(), 1);
        assert_eq!(rescanned[0].uid, uid);
        assert!(load_settings().unwrap().favorites["ps5"].contains(&uid));
        assert_eq!(fs::read(&boot).unwrap(), b"synthetic boot, not playable");
        // Even one game at a shared registered root does not inherit generic art.
        let root_boot = root.join("EBOOT.BIN");
        fs::write(&root_boot, b"synthetic").unwrap();
        fs::create_dir_all(root.join("sce_sys")).unwrap();
        fs::write(root.join("sce_sys/icon0.png"), &png).unwrap();
        let root_game = validate_game("ps5", root_boot.to_str().unwrap()).unwrap();
        assert_eq!(ps5_icon(&root_game), None);
    }
}
