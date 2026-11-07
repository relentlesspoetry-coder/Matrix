//! Explicit library registration and bounded local multidisc playlists.
//! Never moves, rewrites, or removes game data.
use super::*;
use std::io::Read;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Folder {
    path: String,
    managed: bool,
    available: bool,
}

fn folders_in(id: &str, managed: &Path, settings: &Settings) -> Vec<Folder> {
    let mut result = vec![Folder {
        path: managed.to_string_lossy().into_owned(),
        managed: true,
        available: managed.is_dir(),
    }];
    let mut seen = HashSet::new();
    seen.insert(
        managed
            .canonicalize()
            .unwrap_or_else(|_| managed.to_owned()),
    );
    for path in settings.library_folders.get(id).into_iter().flatten() {
        let path = Path::new(path);
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            continue;
        }
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        if seen.insert(canonical) {
            result.push(Folder {
                path: path.to_string_lossy().into_owned(),
                managed: false,
                available: path.is_dir(),
            });
        }
    }
    result
}

pub(crate) fn folders(id: &str) -> Result<Vec<Folder>, String> {
    Ok(folders_in(id, &games_dir(id)?, &load_settings()?))
}

pub(crate) fn roots(id: &str) -> Result<Vec<PathBuf>, String> {
    Ok(folders(id)?
        .into_iter()
        .filter(|f| f.available)
        .filter_map(|f| Path::new(&f.path).canonicalize().ok())
        .collect())
}

pub(crate) fn containing_root(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    // Prefer the narrowest registered folder for sandbox grants.
    roots
        .iter()
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.components().count())
        .cloned()
}

pub(crate) fn game_root(id: &str, path: &Path) -> Result<PathBuf, String> {
    let path = path.canonicalize().map_err(|_| "Game file missing")?;
    if !path.is_file() {
        return Err("Game file missing".into());
    }
    containing_root(&path, &roots(id)?)
        .ok_or_else(|| "Game is outside this console's library".into())
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn list_library_folders(id: String) -> Result<Vec<Folder>, String> {
    folders(&id)
}

fn register(settings: &mut Settings, id: &str, path: &Path, managed: &Path) -> Result<(), String> {
    spec(id)?;
    if !path.is_absolute() {
        return Err("Choose an absolute game library folder path.".into());
    }
    let path = path
        .canonicalize()
        .map_err(|_| "Choose an existing game library folder.")?;
    if !path.is_dir() {
        return Err("Choose a folder, not a game file.".into());
    }
    if path.parent().is_none() {
        return Err("Choose a game library folder, not the filesystem root.".into());
    }
    if folders_in(id, managed, settings)
        .iter()
        .any(|f| Path::new(&f.path).canonicalize().ok().as_ref() == Some(&path))
    {
        return Ok(());
    }
    let paths = settings.library_folders.entry(id.into()).or_default();
    if paths.len() >= 32 {
        return Err("At most 32 external folders can be registered per console.".into());
    }
    paths.push(path.to_string_lossy().into_owned());
    Ok(())
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn add_library_folder(id: String, path: String) -> Result<Vec<Folder>, String> {
    let managed = games_dir(&id)?;
    update_settings(|settings| {
        register(settings, &id, Path::new(&path), &managed)?;
        Ok(folders_in(&id, &managed, settings))
    })
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn remove_library_folder(id: String, path: String) -> Result<Vec<Folder>, String> {
    let managed = games_dir(&id)?;
    update_settings(|settings| {
        // Exact saved spelling also permits removing an unplugged drive.
        if let Some(paths) = settings.library_folders.get_mut(&id) {
            paths.retain(|saved| saved != &path);
        }
        Ok(folders_in(&id, &managed, settings))
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) async fn browse_library_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Choose game library folder")
            .blocking_pick_folder()
            .map(|file| {
                file.into_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|e| e.to_string())
            })
            .transpose()
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) fn is_playlist(id: &str, path: &Path) -> bool {
    id == "ps1"
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("m3u"))
}

pub(crate) fn playlist_members(
    id: &str,
    path: &Path,
    roots: &[PathBuf],
) -> Result<Vec<PathBuf>, String> {
    if !is_playlist(id, path) {
        return Err("Unsupported playlist format".into());
    }
    let canonical = path.canonicalize().map_err(|_| "Playlist file missing")?;
    let playlist_root =
        containing_root(&canonical, roots).ok_or("Playlist is outside this console's library")?;
    let parent = canonical.parent().ok_or("Invalid playlist path")?;
    let mut bytes = Vec::new();
    fs::File::open(&canonical)
        .map_err(|e| e.to_string())?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Playlist exceeds 64 KiB".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "Playlist must use UTF-8 text")?;
    let mut members = Vec::new();
    let mut seen = HashSet::new();
    let s = spec(id)?;
    // DuckStation strips one UTF-8 BOM and ASCII whitespace, not arbitrary
    // Unicode spaces that can legitimately be part of a Linux filename.
    for line in text.strip_prefix('\u{feff}').unwrap_or(text).lines() {
        let line = line.trim_matches(|c: char| c == ' ' || ('\u{9}'..='\u{d}').contains(&c));
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let relative = Path::new(line);
        if line.contains(['\0', ':', '\\'])
            || relative.is_absolute()
            || relative.components().any(|c| {
                !matches!(
                    c,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            })
        {
            return Err(
                "Playlist entries must be local relative paths without parent traversal".into(),
            );
        }
        let member = parent
            .join(relative)
            .canonicalize()
            .map_err(|_| "A playlist disc is missing")?;
        if !member.is_file() || !member.starts_with(&playlist_root) {
            return Err("A playlist disc is outside this console's library".into());
        }
        if is_playlist(id, &member) || !supported_console_file(s, &member) {
            return Err("Playlist contains an unsupported disc format".into());
        }
        if !seen.insert(member.clone()) {
            return Err("Playlist contains a duplicate disc".into());
        }
        members.push(member);
        if members.len() > 64 {
            return Err("Playlist exceeds 64 discs".into());
        }
    }
    if members.is_empty() {
        return Err("Playlist has no discs".into());
    }
    Ok(members)
}

pub(crate) fn has_manual_cover(root: &Path, id: &str, uid: &str) -> bool {
    let key = cover_key(id, uid);
    ["png", "jpg", "webp"]
        .iter()
        .any(|ext| root.join(id).join(format!("{key}.{ext}")).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_preserves_managed_folders_and_unrelated_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let managed = tmp.path().join("managed");
        let external = tmp.path().join("external");
        fs::create_dir_all(&managed).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(external.join("keep.iso"), b"keep").unwrap();
        let mut settings = Settings::default();
        settings.artwork.enabled = false;
        settings.favorites.insert("ps1".into(), vec!["keep".into()]);
        register(&mut settings, "ps1", &external, &managed).unwrap();
        register(&mut settings, "ps1", &external, &managed).unwrap();
        register(&mut settings, "ps1", &managed, &managed).unwrap();
        assert_eq!(settings.library_folders["ps1"].len(), 1);
        let rows = folders_in("ps1", &managed, &settings);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].managed);
        assert!(rows[1].available);
        assert!(register(&mut settings, "ps1", Path::new("relative"), &managed).is_err());
        assert!(register(&mut settings, "ps1", &external.join("keep.iso"), &managed).is_err());
        assert!(!settings.artwork.enabled);
        assert_eq!(settings.favorites["ps1"], ["keep"]);
        settings.library_folders.get_mut("ps1").unwrap().clear();
        assert_eq!(fs::read(external.join("keep.iso")).unwrap(), b"keep");
        assert_eq!(folders_in("ps1", &managed, &settings).len(), 1);
    }
    #[test]
    fn unavailable_external_folder_is_retained_and_legacy_settings_default() {
        let tmp = tempfile::tempdir().unwrap();
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        settings.library_folders.insert(
            "ps1".into(),
            vec![tmp.path().join("offline").to_string_lossy().into_owned()],
        );
        let rows = folders_in("ps1", &tmp.path().join("managed"), &settings);
        assert_eq!(rows.len(), 2);
        assert!(!rows[1].available);
        assert!(!rows[1].managed);
    }
    #[test]
    fn local_playlists_are_bounded_and_reject_unsafe_or_missing_discs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        fs::write(root.join("disc1.chd"), b"fixture").unwrap();
        fs::write(root.join("disc2.chd"), b"fixture").unwrap();
        let playlist = root.join("Collection.m3u");
        fs::write(&playlist, "\u{feff}#EXTM3U\r\ndisc1.chd\r\ndisc2.chd\r\n").unwrap();
        assert_eq!(
            playlist_members("ps1", &playlist, &[root.clone()])
                .unwrap()
                .len(),
            2
        );
        for text in [
            "\u{3000}disc1.chd",
            "../disc1.chd",
            "/disc1.chd",
            "https://example.com/disc1.chd",
            "C:\\disc1.chd",
            "missing.chd",
            "disc1.chd\ndisc1.chd",
            "Collection.m3u",
            "# Empty",
            "disc1.chd\0",
        ] {
            fs::write(&playlist, text).unwrap();
            assert!(
                playlist_members("ps1", &playlist, &[root.clone()]).is_err(),
                "accepted {text}"
            );
        }
        fs::write(&playlist, vec![b'x'; 65537]).unwrap();
        assert!(playlist_members("ps1", &playlist, &[root.clone()]).is_err());
        fs::write(&playlist, [0xff, 0xfe]).unwrap();
        assert!(playlist_members("ps1", &playlist, &[root]).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn playlist_symlink_cannot_escape_registered_roots() {
        let inside = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("disc.chd"), b"fixture").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("disc.chd"),
            inside.path().join("disc.chd"),
        )
        .unwrap();
        let playlist = inside.path().join("title.m3u");
        fs::write(&playlist, "disc.chd").unwrap();
        assert!(
            playlist_members("ps1", &playlist, &[inside.path().canonicalize().unwrap()]).is_err()
        );
    }
    #[test]
    fn scans_multiple_folders_deduplicate_and_group_only_valid_playlists() {
        let tmp = tempfile::tempdir().unwrap();
        let first = tmp.path().join("managed");
        let external = tmp.path().join("external");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(first.join("Standalone.chd"), b"fixture").unwrap();
        for disc in ["Disc 1.chd", "Disc 2.chd"] {
            fs::write(external.join(disc), b"fixture").unwrap();
        }
        let playlist = external.join("Adventure.m3u");
        fs::write(&playlist, "#EXTM3U\nDisc 1.chd\nDisc 2.chd").unwrap();
        let roots = [
            first.canonicalize().unwrap(),
            external.canonicalize().unwrap(),
            external.canonicalize().unwrap(),
        ];
        let games = scan_console_in("ps1", &roots).unwrap();
        assert_eq!(games.len(), 2);
        assert_eq!(
            games.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(),
            ["Adventure", "Standalone"]
        );
        // Invalid lists never conceal working discs and cannot become launch targets.
        fs::write(&playlist, "Disc 1.chd\nMissing.chd").unwrap();
        let games = scan_console_in("ps1", &roots).unwrap();
        assert_eq!(games.len(), 3);
        assert!(!games.iter().any(|g| g.path.ends_with(".m3u")));
        assert_eq!(fs::read(external.join("Disc 1.chd")).unwrap(), b"fixture");
        // Unregistering a root removes it from scans, not from disk.
        assert_eq!(scan_console_in("ps1", &roots[..1]).unwrap().len(), 1);
        assert!(external.join("Disc 2.chd").is_file());
    }
    #[test]
    fn unverified_playlist_formats_never_hide_individual_discs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        fs::write(root.join("One.gdi"), b"fixture").unwrap();
        fs::write(root.join("Two.chd"), b"fixture").unwrap();
        fs::write(root.join("Title.m3u"), "One.gdi\nTwo.chd").unwrap();
        assert_eq!(
            scan_console_in("dreamcast", &[root.clone()]).unwrap().len(),
            2
        );
        fs::remove_file(root.join("Title.m3u")).unwrap();
        fs::write(root.join("Title.m3u8"), "Two.chd").unwrap();
        assert_eq!(scan_console_in("ps1", &[root.clone()]).unwrap().len(), 1);
        assert!(!is_playlist("ps1", &root.join("Title.m3u8")));
        assert!(!is_playlist("ps2", &root.join("Title.m3u")));
        assert!(!supported_console_file(
            spec("ps2").unwrap(),
            &root.join("Title.m3u")
        ));
    }
    #[test]
    fn installed_rpcs3_folder_uses_existing_eboot_adapter() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("dev_hdd0/game");
        let boot = root.join("Example_Digital_Title/USRDIR/EBOOT.BIN");
        fs::create_dir_all(boot.parent().unwrap()).unwrap();
        fs::write(&boot, b"fixture").unwrap();
        let games = scan_console_in("ps3", &[root.canonicalize().unwrap()]).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Example Digital Title");
        assert_eq!(
            games[0].path,
            boot.canonicalize().unwrap().to_string_lossy()
        );
    }
    #[test]
    fn grouping_preserves_existing_disc_favorites_and_imported_covers() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("games");
        let covers = tmp.path().join("covers");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(covers.join("ps1")).unwrap();
        let paths: Vec<_> = ["Favorite.chd", "Custom.chd", "Plain.chd"]
            .iter()
            .map(|name| root.join(name))
            .collect();
        for path in &paths {
            fs::write(path, b"synthetic").unwrap();
        }
        fs::write(
            root.join("Collection.m3u"),
            "Favorite.chd\nCustom.chd\nPlain.chd",
        )
        .unwrap();
        let uid = |path: &Path| path.canonicalize().unwrap().to_string_lossy().into_owned();
        let favorite = uid(&paths[0]);
        let custom = uid(&paths[1]);
        let cover = covers
            .join("ps1")
            .join(format!("{}.png", cover_key("ps1", &custom)));
        fs::write(&cover, b"existing manual cover fixture").unwrap();
        let favorites = HashSet::from([favorite.clone()]);
        let games = scan_console_with_customizations(
            "ps1",
            &[root.canonicalize().unwrap()],
            &favorites,
            Some(&covers),
        )
        .unwrap();
        assert_eq!(games.len(), 3);
        assert!(games.iter().any(|g| g.uid == favorite));
        assert!(games.iter().any(|g| g.uid == custom));
        assert!(!games.iter().any(|g| g.uid == uid(&paths[2])));
        assert!(games.iter().any(|g| g.path.ends_with("Collection.m3u")));
        assert_eq!(fs::read(cover).unwrap(), b"existing manual cover fixture");
        assert_eq!(favorites, HashSet::from([favorite]));
    }
    #[test]
    fn playlist_whitespace_matches_duckstation_ascii_range() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let disc = root.join("disc.chd");
        fs::write(&disc, b"fixture").unwrap();
        let playlist = root.join("Game.m3u");
        fs::write(&playlist, "\u{b}disc.chd\u{b}\n").unwrap();
        assert_eq!(playlist_members("ps1", &playlist, &[root]).unwrap(), [disc]);
    }

    #[test]
    fn playlist_preserves_non_ascii_whitespace_in_filenames() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let disc = root.join("\u{3000}Disc.chd");
        fs::write(&disc, b"fixture").unwrap();
        let playlist = root.join("Title.m3u");
        fs::write(&playlist, "\u{feff}#EXTM3U\n\t \u{3000}Disc.chd \r\n").unwrap();
        assert_eq!(playlist_members("ps1", &playlist, &[root]).unwrap(), [disc]);
    }
}
