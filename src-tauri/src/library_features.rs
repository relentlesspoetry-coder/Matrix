//! Library preferences and offline inventory. No game files are modified.
use super::*;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Preferences {
    pub setup_completed: Option<bool>,
    pub hide_unconfigured: Option<bool>,
    pub theme: Option<String>,
    pub ui_scale: Option<u16>,
    pub cover_size: Option<String>,
    pub controller_prompts: Option<String>,
    pub start_fullscreen: Option<bool>,
    pub ready_only: bool,
    pub hidden: Vec<String>,
    pub pinned: Vec<String>,
    pub order: Vec<String>,
    pub views: BTreeMap<String, View>,
    pub metadata: BTreeMap<String, Metadata>,
    pub auto_refresh: Option<bool>,
    pub grouping: Option<bool>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct View {
    pub layout: String,
    pub sort: String,
    pub selected: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Metadata {
    pub display: String,
    pub cover_title: String,
    pub group: String,
    pub progress: String,
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn set_library_preference(
    section: String,
    key: String,
    value: serde_json::Value,
) -> Result<Preferences, String> {
    update_settings(|s| {
        let p = &mut s.library_preferences;
        match section.as_str() {
            "theme" => {
                let theme = value.as_str().ok_or("Invalid theme")?;
                if !["arctic-current", "white-frost"].contains(&theme) {
                    return Err("Unknown theme".into());
                }
                p.theme = Some(theme.into());
            }
            "uiScale" => {let n=value.as_u64().ok_or("Invalid UI scale")?;if ![85,100,115,130,150].contains(&n){return Err("Unsupported UI scale".into());}p.ui_scale=Some(n as u16);}
            "coverSize" => {let v=value.as_str().ok_or("Invalid cover size")?;if !["small","medium","large"].contains(&v){return Err("Invalid cover size".into());}p.cover_size=Some(v.into());}
            "controllerPrompts" => {let v=value.as_str().ok_or("Invalid button prompts")?;if !["auto","xbox","playstation"].contains(&v){return Err("Invalid button prompts".into());}p.controller_prompts=Some(v.into());}
            "startFullscreen" => p.start_fullscreen=Some(value.as_bool().ok_or("Invalid fullscreen setting")?),
            "readyOnly" => p.ready_only = value.as_bool().ok_or("Invalid filter")?,
            "setupCompleted" => p.setup_completed = Some(value.as_bool().ok_or("Invalid setup state")?),
            "hideUnconfigured" => p.hide_unconfigured = Some(value.as_bool().ok_or("Invalid emulator filter")?),
            "autoRefresh" => {
                p.auto_refresh = Some(value.as_bool().ok_or("Invalid refresh setting")?)
            }
            "grouping" => p.grouping = Some(value.as_bool().ok_or("Invalid grouping setting")?),
            "hidden" | "pinned" | "order" => {
                let ids: Vec<String> =
                    serde_json::from_value(value).map_err(|_| "Invalid console list")?;
                if ids.len() > 24
                    || ids.iter().any(|id| spec(id).is_err())
                    || ids.iter().collect::<HashSet<_>>().len() != ids.len()
                {
                    return Err("Invalid console list".into());
                }
                match section.as_str() {
                    "hidden" => p.hidden = ids,
                    "pinned" => p.pinned = ids,
                    _ => p.order = ids,
                }
            }
            "view" => {
                spec(&key)?;
                let v: View = serde_json::from_value(value).map_err(|_| "Invalid view")?;
                if !["grid", "list"].contains(&v.layout.as_str())
                    || !["default", "az", "recent"].contains(&v.sort.as_str())
                    || v.selected.len() > 8192
                {
                    return Err("Invalid view".into());
                }
                p.views.insert(key, v);
            }
            "metadata" => {
                let m: Metadata = serde_json::from_value(value).map_err(|_| "Invalid metadata")?;
                if key.len() > 8192
                    || [&m.display, &m.cover_title, &m.group]
                        .iter()
                        .any(|v| v.len() > 240 || v.chars().any(char::is_control))
                {
                    return Err("Use titles up to 240 bytes without control characters".into());
                }
                if !["", "backlog", "playing", "completed", "dropped"].contains(&m.progress.as_str()){return Err("Invalid game progress label".into());}
                p.metadata.insert(key, m);
            }
            _ => return Err("Unknown library preference".into()),
        };
        Ok(p.clone())
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    games: BTreeMap<String, Vec<Game>>,
    folders: BTreeMap<String, Vec<libraries::Folder>>,
}
pub(crate) fn snapshot_impl() -> Result<Snapshot, String> {
    let mut games = BTreeMap::new();
    let mut folders = BTreeMap::new();
    let previous = load_settings()?.library_index;
    for s in SPECS {
        let fs = libraries::folders(s.id)?;
        let mut rows = scan_console_impl(s.id)?;
        for old in previous.get(s.id).into_iter().flatten() {
            if fs
                .iter()
                .any(|f| !f.available && Path::new(&old.path).starts_with(&f.path))
                && !rows.iter().any(|g| g.uid == old.uid)
            {
                rows.push(old.clone());
            }
        }
        games.insert(s.id.into(), rows);
        folders.insert(s.id.into(), fs);
    }
    if games != previous {
        update_settings(|s| {
            s.library_index = games.clone();
            Ok(())
        })?;
    }
    Ok(Snapshot { games, folders })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub(crate) async fn library_snapshot(background:Option<bool>) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move||{if background==Some(true)&&gaming::active(){return Err("Gaming mode pauses background scans".into());}snapshot_impl()})
        .await
        .map_err(|e| e.to_string())?
}
pub(crate) fn offline_cover(id: &str, uid: &str) -> Result<Option<String>, String> {
    use std::io::Read;
    spec(id)?;
    let settings = load_settings()?;
    let game = settings
        .library_index
        .get(id)
        .into_iter()
        .flatten()
        .find(|g| g.uid == uid)
        .ok_or("Game is not in the library index")?;
    if !libraries::folders(id)?
        .iter()
        .any(|f| !f.available && Path::new(&game.path).starts_with(&f.path))
    {
        return Err("Game is not on a registered offline drive".into());
    }
    let mut paths = console_cover_candidates(id, uid)?;
    for ext in ["png", "jpg", "webp"] {
        paths.push(legacy_stored_cover_path(id, uid, ext)?);
    }
    for path in paths {
        let Ok(file) = fs::File::open(path) else {
            continue;
        };
        if !file
            .metadata()
            .is_ok_and(|m| m.is_file() && m.len() <= MAX_COVER_BYTES as u64)
        {
            continue;
        }
        let mut bytes = Vec::new();
        if file
            .take(MAX_COVER_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > MAX_COVER_BYTES
        {
            continue;
        }
        if let Some((_, mime)) = cover_format(&bytes) {
            return Ok(Some(format!("data:{mime};base64,{}", BASE64.encode(bytes))));
        }
    }
    Ok(None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_library_features() {
        let temp = tempfile::tempdir().unwrap();
        let name = format!("{}::fixture", module_path!().split_once("::").unwrap().1);
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env("MATRIX_FEATURE_TEST", "1")
            .env("HOME", temp.path())
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .env("XDG_DATA_HOME", temp.path().join("data"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    #[test]
    fn fixture() {
        if std::env::var_os("MATRIX_FEATURE_TEST").is_none() {
            return;
        }
        create_layout().unwrap();
        let legacy = serde_json::json!({"windows":[{"uid":"/keep.desktop","runner":"desktop"}],"steam_libraries":[{"path":"/keep/steamapps","launcher":"native"}]});
        let settings: Settings = serde_json::from_value(legacy.clone()).unwrap();
        update_settings(|s| {
            *s = settings;
            Ok(())
        })
        .unwrap();
        assert!(launches::get_launch_status("steam".into()).is_err());
        assert!(launches::get_launch_status("windows".into()).is_err());

        set_library_preference("theme".into(), "".into(), serde_json::json!("white-frost"))
            .unwrap();
        assert_eq!(
            load_settings()
                .unwrap()
                .library_preferences
                .theme
                .as_deref(),
            Some("white-frost")
        );
        assert_eq!(
            load_settings().unwrap().legacy_fields["windows"],
            legacy["windows"]
        );
        assert_eq!(
            load_settings().unwrap().legacy_fields["steam_libraries"],
            legacy["steam_libraries"]
        );
        assert!(set_library_preference(
            "theme".into(),
            "".into(),
            serde_json::json!("not-a-theme")
        )
        .is_err());

        let view = serde_json::json!({"layout":"grid","sort":"az","selected":"game"});
        set_library_preference("view".into(), "ps2".into(), view).unwrap();
        assert_eq!(
            load_settings().unwrap().library_preferences.views["ps2"].layout,
            "grid"
        );
        assert!(set_library_preference(
            "hidden".into(),
            "".into(),
            serde_json::json!(["ps2", "ps2"])
        )
        .is_err());
        set_library_preference("metadata".into(),"ps2|game".into(),serde_json::json!({"display":"Clean title","coverTitle":"Catalog (USA)","group":"Series"})).unwrap();
        assert_eq!(
            load_settings().unwrap().library_preferences.metadata["ps2|game"].cover_title,
            "Catalog (USA)"
        );
        let external = dirs::home_dir().unwrap().join("external-games");
        fs::create_dir(&external).unwrap();
        let game = external.join("Example.iso");
        fs::write(&game, b"synthetic game").unwrap();
        libraries::add_library_folder("ps2".into(), external.to_str().unwrap().into()).unwrap();
        assert!(snapshot_impl().unwrap().games["ps2"]
            .iter()
            .any(|g| g.path == game.to_str().unwrap()));
        let unplugged = external.with_extension("unplugged");
        assert!(get_cover("ps1".into(), game.to_str().unwrap().into())
            .unwrap_err()
            .contains("outside"));
        let png = include_bytes!("../../tests/fixtures/demo-cover.png");
        let uid = game.to_str().unwrap();
        save_cover("ps2".into(), uid.into(), png.to_vec()).unwrap();
        fs::rename(&external, &unplugged).unwrap();
        let offline = snapshot_impl().unwrap();
        assert!(get_cover("ps2".into(), uid.into())
            .unwrap()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(offline.folders["ps2"]
            .iter()
            .any(|f| !f.managed && !f.available));
        assert!(offline.games["ps2"]
            .iter()
            .any(|g| g.path == game.to_str().unwrap()));
        fs::remove_file(unplugged.join("Example.iso")).unwrap();
        fs::rename(&unplugged, &external).unwrap();
        assert!(!snapshot_impl().unwrap().games["ps2"]
            .iter()
            .any(|g| g.path == game.to_str().unwrap()));
        // Ordered sets preserve images, reject foreign members and never overwrite.
        let discroot=games_dir("ps1").unwrap();
        let d1=discroot.join("Adventure (Disc 1).chd");let d2=discroot.join("Adventure (Disc 2).chd");
        fs::write(&d1,b"disc one").unwrap();fs::write(&d2,b"disc two").unwrap();
        let uids=vec![d2.to_str().unwrap().into(),d1.to_str().unwrap().into()];
        let collection=super::disc_sets::create("ps1","Adventure",&uids).unwrap();
        assert_eq!(super::disc_sets::details("ps1",&collection.uid).unwrap().discs[0].path,d2.to_str().unwrap());
        assert!(super::disc_sets::create("ps1","Adventure",&uids).is_err());
        assert!(super::disc_sets::create("ps2","Adventure",&uids).is_err());
        assert!(super::disc_sets::create("ps1","../Escape",&uids).is_err());
        let p=EmulatorProfile{emulator_path:"/bin/duckstation".into(),..Default::default()};
        assert_eq!(super::disc_sets::launch_path("ps1",&collection,&p,Some(d1.to_str().unwrap())).unwrap(),d1);
        assert!(super::disc_sets::launch_path("ps1",&collection,&p,Some("/outside/Disc.chd")).is_err());
        let custom=EmulatorProfile{emulator_path:"/bin/custom-launcher".into(),launch_args:vec!["--own-playlist".into(),"{game}".into()],..Default::default()};
        assert_eq!(super::disc_sets::launch_path("ps1",&collection,&custom,None).unwrap(),PathBuf::from(&collection.path));
        assert_eq!(fs::read(&d1).unwrap(),b"disc one");
        // Display preferences are validated and survive settings reload.
        for (section,value) in [("uiScale",serde_json::json!(130)),("coverSize",serde_json::json!("large")),("controllerPrompts",serde_json::json!("playstation")),("startFullscreen",serde_json::json!(true))] {
            set_library_preference(section.into(),"".into(),value).unwrap();
        }
        assert_eq!(load_settings().unwrap().library_preferences.ui_scale,Some(130));
        assert!(set_library_preference("uiScale".into(),"".into(),serde_json::json!(900)).is_err());
        assert!(set_library_preference("coverSize".into(),"".into(),serde_json::json!("huge")).is_err());
        set_library_preference("metadata".into(),"ps1|disc".into(),serde_json::json!({"display":"Keep title","progress":"completed"})).unwrap();
        assert_eq!(load_settings().unwrap().library_preferences.metadata["ps1|disc"].progress,"completed");
        assert!(set_library_preference("metadata".into(),"ps1|disc".into(),serde_json::json!({"progress":"unknown"})).is_err());
        assert_eq!(load_settings().unwrap().library_preferences.metadata["ps1|disc"].display,"Keep title");
        let secret="fixture-retro-web-key-000000000000";
        let status=super::retroachievements::save_account("FixtureUser".into(),secret.into()).unwrap();
        assert!(!serde_json::to_string(&status).unwrap().contains(secret));
        super::retroachievements::save_account("FixtureUser".into(),String::new()).unwrap();
        assert_eq!(load_settings().unwrap().retroachievements.web_api_key,secret);
        assert!(super::retroachievements::save_account("ChangedUser".into(),String::new()).is_err());
        super::retroachievements::save_account(String::new(),String::new()).unwrap();
        assert!(load_settings().unwrap().retroachievements.web_api_key.is_empty());
        super::save_data::test_restore_roundtrip();
    }
}
