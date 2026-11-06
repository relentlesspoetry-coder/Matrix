use super::*;

#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn set_favorite(id: String, uid: String, favorite: bool) -> Result<Vec<String>, String> {
    spec(&id)?;
    if favorite {
        validate_game(&id, &uid)?;
    }
    update_settings(|settings| {
        let list = settings.favorites.entry(id).or_default();
        if favorite {
            if !list.contains(&uid) {
                list.push(uid);
            }
        } else {
            list.retain(|entry| entry != &uid);
        }
        let result = list.clone();
        settings.favorites_migrated = true;
        Ok(result)
    })
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn import_favorites(
    entries: BTreeMap<String, Vec<String>>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    update_settings(|settings| {
        if !settings.favorites_migrated {
            for (id, uids) in entries {
                if spec(&id).is_err() {
                    continue;
                }
                let root = games_dir(&id)?;
                let root = root.canonicalize().unwrap_or(root);
                let list = settings.favorites.entry(id).or_default();
                for uid in uids {
                    let path = Path::new(&uid);
                    // Preserve unavailable legacy entries under their own console,
                    // while excluding preview IDs, parent traversal and other libraries.
                    if path.is_absolute()
                        && path.starts_with(&root)
                        && !path
                            .components()
                            .any(|c| c == std::path::Component::ParentDir)
                        && !list.contains(&uid)
                    {
                        list.push(uid);
                    }
                }
            }
            settings.favorites_migrated = true;
        }
        Ok(settings.favorites.clone())
    })
}
