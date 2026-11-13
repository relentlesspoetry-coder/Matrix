//! Explicit ordered disc sets. Create only a new local M3U; never rename discs.
use super::*;
use std::io::Write;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiscSet {
    pub discs: Vec<Game>,
    pub playlist_supported: bool,
    pub custom_playlist: bool,
}

fn supported(profile: &EmulatorProfile, id: &str) -> bool {
    matches!(
        (id, discovery::identity(profile, id)),
        ("ps1", Some("DuckStation")) | ("saturn", Some("Mednafen"))
    )
}
fn custom_playlist(profile: &EmulatorProfile, id: &str) -> bool {
    // PS1 already accepted M3U paths with explicitly configured custom launchers.
    // Keep that contract without inventing flags or claiming engine support.
    id == "ps1" && discovery::identity(profile, id).is_none()
}
pub(crate) fn details(id: &str, uid: &str) -> Result<DiscSet, String> {
    let game = validate_game(id, uid)?;
    let discs = if libraries::is_playlist(id, Path::new(&game.path)) {
        libraries::playlist_members(id, Path::new(&game.path), &libraries::roots(id)?)?
            .into_iter()
            .enumerate()
            .map(|(n, p)| {
                let uid = actionable_path(&p)?;
                Ok(Game {
                    title: format!(
                        "Disc {} · {}",
                        n + 1,
                        p.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    uid: uid.clone(),
                    path: uid,
                    kind: id.into(),
                    app_id: None,
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        Vec::new()
    };
    let settings = load_settings()?;
    let custom_playlist = settings
        .profiles
        .get(id)
        .is_some_and(|p| custom_playlist(p, id));
    let playlist_supported =
        custom_playlist || settings.profiles.get(id).is_some_and(|p| supported(p, id));
    Ok(DiscSet {
        discs,
        playlist_supported,
        custom_playlist,
    })
}
pub(crate) fn launch_path(
    id: &str,
    game: &Game,
    profile: &EmulatorProfile,
    disc: Option<&str>,
) -> Result<PathBuf, String> {
    let path = Path::new(&game.path);
    if !libraries::is_playlist(id, path) {
        if disc.is_some() {
            return Err("Choose a disc from a multi-disc game".into());
        }
        return Ok(path.into());
    }
    let members = libraries::playlist_members(id, path, &libraries::roots(id)?)?;
    if let Some(disc) = disc {
        return members
            .into_iter()
            .find(|p| p.to_str() == Some(disc))
            .ok_or("Disc is not part of this game".into());
    }
    if !supported(profile, id) && !custom_playlist(profile, id) {
        return Err("This emulator has no verified playlist adapter. Open game details and choose a disc to launch individually.".into());
    }
    Ok(path.into())
}

pub(crate) fn create(id: &str, title: &str, uids: &[String]) -> Result<Game, String> {
    if !["ps1", "saturn"].contains(&id) {
        return Err("Disc sets currently support PlayStation 1 and Sega Saturn".into());
    }
    let title = title.trim();
    if title.is_empty()
        || title.len() > 160
        || title.starts_with('.')
        || title.chars().any(|c| c.is_control() || "/\\:".contains(c))
    {
        return Err(
            "Use a title up to 160 bytes without path separators or control characters".into(),
        );
    }
    if !(2..=64).contains(&uids.len()) || uids.iter().collect::<HashSet<_>>().len() != uids.len() {
        return Err("Select 2–64 distinct discs in playback order".into());
    }
    let roots = libraries::roots(id)?;
    let games = uids
        .iter()
        .map(|uid| validate_game(id, uid))
        .collect::<Result<Vec<_>, _>>()?;
    let parent = Path::new(&games[0].path)
        .parent()
        .ok_or("Invalid disc path")?;
    let mut text = String::from("#EXTM3U\n");
    for game in &games {
        let p = Path::new(&game.path);
        if libraries::is_playlist(id, p) || p.parent() != Some(parent) {
            return Err("Choose individual discs from the same folder".into());
        }
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid disc filename")?;
        if name.starts_with('#')
            || name.trim() != name
            || name.chars().any(|c| c.is_control() || ":\\".contains(c))
        {
            return Err("A disc filename cannot be represented safely in a playlist".into());
        }
        text.push_str(name);
        text.push('\n');
    }
    let path = parent.join(format!("{title}.m3u"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| format!("Create playlist (existing files are kept): {e}"))?;
    let result = (|| {
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        libraries::playlist_members(id, &path, &roots)?;
        fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        let uid = actionable_path(&path.canonicalize().map_err(|e| e.to_string())?)?;
        Ok(Game {
            uid: uid.clone(),
            path: uid,
            title: title.into(),
            kind: id.into(),
            app_id: None,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_verified_engines_receive_playlists() {
        for (id, path, expected) in [
            ("ps1", "/bin/duckstation", true),
            ("saturn", "/bin/mednafen", true),
            ("ps1", "/bin/custom", false),
            ("dreamcast", "/bin/flycast", false),
        ] {
            let p = EmulatorProfile {
                emulator_path: path.into(),
                ..Default::default()
            };
            assert_eq!(supported(&p, id), expected);
        }
    }
}
