#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

/// Names are intentionally stable: moving or renaming managed folders breaks an existing library.
#[derive(Clone, Copy)]
struct ConsoleSpec {
    id: &'static str,
    dir: &'static str,
    games: &'static str,
    extensions: &'static [&'static str],
}

const SPECS: &[ConsoleSpec] = &[
    ConsoleSpec {
        id: "switch",
        dir: "Switch",
        games: "Switch Games",
        extensions: &["nsp", "xci"],
    },
    ConsoleSpec {
        id: "ps1",
        dir: "PS1",
        games: "PS1 Games",
        extensions: &["cue", "chd", "iso", "pbp", "ccd"],
    },
    ConsoleSpec {
        id: "ps2",
        dir: "PS2",
        games: "PS2 Games",
        extensions: &["iso", "chd", "cso", "gz"],
    },
    ConsoleSpec {
        id: "ps3",
        dir: "PS3",
        games: "PS3 Games",
        extensions: &["iso", "pkg"],
    },
    ConsoleSpec {
        id: "psp",
        dir: "PSP",
        games: "PSP Games",
        extensions: &["iso", "cso", "pbp"],
    },
    ConsoleSpec {
        id: "n64",
        dir: "N64",
        games: "N64 Games",
        extensions: &["z64", "v64", "n64"],
    },
    ConsoleSpec {
        id: "gamecube",
        dir: "GameCube",
        games: "GameCube Games",
        extensions: &["iso", "gcm", "rvz", "gcz"],
    },
    ConsoleSpec {
        id: "dreamcast",
        dir: "Dreamcast",
        games: "Dreamcast Games",
        extensions: &["gdi", "cdi", "chd"],
    },
    ConsoleSpec {
        id: "wii",
        dir: "Wii",
        games: "Wii Games",
        extensions: &["wbfs", "iso", "rvz", "wia"],
    },
    ConsoleSpec {
        id: "wiiu",
        dir: "Wii U",
        games: "Wii U Games",
        extensions: &["wua", "wud", "wux", "rpx"],
    },
    ConsoleSpec {
        id: "gb",
        dir: "Game Boy",
        games: "Game Boy Games",
        extensions: &["gb", "gbc"],
    },
    ConsoleSpec {
        id: "gba",
        dir: "GBA",
        games: "GBA Games",
        extensions: &["gba"],
    },
    ConsoleSpec {
        id: "nds",
        dir: "Nintendo DS",
        games: "Nintendo DS Games",
        extensions: &["nds"],
    },
    ConsoleSpec {
        id: "3ds",
        dir: "Nintendo 3DS",
        games: "Nintendo 3DS Games",
        extensions: &["3ds", "cci", "cxi"],
    },
];

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EmulatorProfile {
    emulator_name: String,
    emulator_path: String,
    /// Each entry is a literal argv token. {game} is the ONLY interpolation token.
    #[serde(default)]
    launch_args: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsGame {
    uid: String,
    title: String,
    target: String,
    runner: String, // "wine" for .exe, "desktop" for .desktop
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ArtworkSettings {
    enabled: bool,
    style: String,
}
impl Default for ArtworkSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            style: "default".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Settings {
    #[serde(default)]
    artwork: ArtworkSettings,
    #[serde(default)]
    profiles: BTreeMap<String, EmulatorProfile>,
    #[serde(default)]
    windows: Vec<WindowsGame>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    artwork: ArtworkSettings,
    root: String,
    folders: BTreeMap<String, String>,
    profiles: BTreeMap<String, EmulatorProfile>,
    windows: Vec<WindowsGame>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Game {
    uid: String,
    title: String,
    path: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_id: Option<String>,
}

fn home() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "Cannot determine Linux home directory".into())
}

fn artwork_root() -> Result<PathBuf, String> {
    let data = dirs::data_local_dir().ok_or("Cannot determine local data directory")?;
    Ok(data.join("matrix").join("covers"))
}

fn matrix_root() -> Result<PathBuf, String> {
    Ok(home()?.join("Games").join("Matrix"))
}

fn config_file() -> Result<PathBuf, String> {
    let config = dirs::config_dir().ok_or("Cannot determine configuration directory")?;
    Ok(config.join("matrix").join("frontend.json"))
}

fn spec(id: &str) -> Result<&'static ConsoleSpec, String> {
    SPECS
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "Unsupported console".into())
}

fn games_dir(id: &str) -> Result<PathBuf, String> {
    let s = spec(id)?;
    Ok(matrix_root()?.join(s.dir).join(s.games))
}

fn guessed_emulator(id: &str) -> &'static str {
    match id {
        "switch" => "Eden",
        "ps1" => "DuckStation",
        "ps2" => "PCSX2",
        "ps3" => "RPCS3",
        "psp" => "PPSSPP",
        "n64" => "Rosalie Mupen GUI",
        "gamecube" | "wii" => "Dolphin",
        "dreamcast" => "Flycast",
        "wiiu" => "Cemu",
        "gb" => "SameBoy",
        "gba" => "mGBA",
        "nds" => "melonDS",
        "3ds" => "Azahar",
        _ => "Emulator",
    }
}

fn default_args(id: &str) -> Vec<String> {
    match id {
        "gamecube" | "wii" => vec!["-b".into(), "-e".into(), "{game}".into()],
        "wiiu" => vec!["-g".into(), "{game}".into()],
        "ps1" => vec!["-batch".into(), "{game}".into()],
        _ => vec!["{game}".into()],
    }
}

/// Detects a single executable in the user-supplied console directory.
/// Multiple candidates are deliberately not guessed: user chooses one in Settings.
fn detect_emulator(s: &ConsoleSpec) -> Option<PathBuf> {
    let dir = matrix_root().ok()?.join(s.dir);
    let mut candidates = Vec::new();
    for file in fs::read_dir(dir).ok()?.flatten() {
        let path = file.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name()?.to_str()?.to_lowercase();
        let recognized = name.ends_with(".appimage")
            || name.ends_with(".sh")
            || name.contains(&guessed_emulator(s.id).to_lowercase().replace(' ', ""));
        if !recognized {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(&path).ok()?.permissions().mode() & 0o111 == 0 {
                continue;
            }
        }
        candidates.push(path);
    }
    if candidates.len() == 1 {
        candidates.pop()
    } else {
        None
    }
}

/// Detect native executables already installed on PATH, without changing their files.
/// Flatpak and atypical installations can be configured manually.
fn detect_installed_emulator(id: &str) -> Option<PathBuf> {
    let names: &[&str] = match id {
        "ps1" => &["duckstation-qt", "duckstation"],
        "ps2" => &["pcsx2-qt", "pcsx2"],
        "ps3" => &["rpcs3"],
        "psp" => &["PPSSPPSDL", "ppsspp"],
        "n64" => &["RMG", "rmg"],
        "gamecube" | "wii" => &["dolphin-emu", "dolphin"],
        "dreamcast" => &["flycast"],
        "wiiu" => &["cemu"],
        "gb" => &["sameboy"],
        "gba" => &["mgba-qt", "mgba"],
        "nds" => &["melonDS", "melonds"],
        "3ds" => &["azahar"],
        _ => &[],
    };
    let paths = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&paths) {
        for name in names {
            let path = directory.join(name);
            if !path.is_file() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if fs::metadata(&path).ok()?.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }
            return Some(path.canonicalize().unwrap_or(path));
        }
    }
    None
}

fn create_layout() -> Result<(), String> {
    for s in SPECS {
        let path = matrix_root()?.join(s.dir).join(s.games);
        fs::create_dir_all(&path).map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
    }
    Ok(())
}

fn load_settings() -> Result<Settings, String> {
    let path = config_file()?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let contents =
        fs::read_to_string(&path).map_err(|e| format!("Read {}: {e}", path.display()))?;
    serde_json::from_str(&contents).map_err(|e| format!("Invalid {}: {e}", path.display()))
}

fn save_settings(settings: &Settings) -> Result<(), String> {
    let file = config_file()?;
    let parent = file.parent().ok_or("Invalid configuration directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    let temporary = file.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    fs::rename(&temporary, &file).map_err(|e| e.to_string())
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn bootstrap() -> Result<Bootstrap, String> {
    create_layout()?;
    let mut settings = load_settings()?;
    let mut auto_detected = false;
    for s in SPECS {
        if settings.profiles.contains_key(s.id) {
            continue;
        }
        if let Some(executable) = detect_emulator(s).or_else(|| detect_installed_emulator(s.id)) {
            settings.profiles.insert(
                s.id.to_string(),
                EmulatorProfile {
                    emulator_name: guessed_emulator(s.id).to_string(),
                    emulator_path: executable.to_string_lossy().into_owned(),
                    launch_args: default_args(s.id),
                },
            );
            auto_detected = true;
        }
    }
    if auto_detected {
        save_settings(&settings)?;
    }
    let root = matrix_root()?;
    let mut folders = BTreeMap::new();
    for s in SPECS {
        folders.insert(
            s.id.into(),
            root.join(s.dir).join(s.games).to_string_lossy().into(),
        );
    }
    Ok(Bootstrap {
        root: root.to_string_lossy().into(),
        folders,
        profiles: settings.profiles,
        windows: settings.windows,
        artwork: settings.artwork,
    })
}

fn prettify(stem: &str) -> String {
    let name = stem.replace('_', " ").replace('.', " ");
    // Remove common bracketed dumps/region tags for display. Never rename the original file.
    let simplified = name
        .split('(')
        .next()
        .unwrap_or(&name)
        .split('[')
        .next()
        .unwrap_or(&name);
    simplified.trim().to_string()
}

fn scan_console_impl(id: &str) -> Result<Vec<Game>, String> {
    let s = spec(id)?;
    let folder = games_dir(id)?;
    if !folder.exists() {
        return Ok(vec![]);
    }
    let mut games = vec![];
    let mut seen = HashSet::new();
    let canonical_folder = folder.canonicalize().map_err(|e| e.to_string())?;
    // Avoid symlink loops, hidden dirs, very deep trees, and accidental unlimited scanning.
    for entry in WalkDir::new(&folder)
        .follow_links(false)
        .max_depth(9)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if games.len() >= 8000 {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_lowercase();
        let ps3_eboot = id == "ps3"
            && path
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("EBOOT.BIN"))
            && path
                .parent()
                .and_then(|v| v.file_name())
                .and_then(|v| v.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("USRDIR"));
        if !ps3_eboot && !s.extensions.contains(&ext.as_str()) {
            continue;
        }
        // Avoid a standalone ISO entry when the same PS1 track is described by a .cue.
        if id == "ps1" && ext == "iso" && path.with_extension("cue").exists() {
            continue;
        }
        // PS3 .pkg is an install package, not a directly launchable game.
        if id == "ps3" && ext == "pkg" {
            continue;
        }
        let launch_path = path.to_path_buf();
        let canonical = match launch_path.canonicalize() {
            Ok(value) => value,
            Err(_) => continue,
        };
        // Canonicalized results must remain inside the managed games directory.
        if !canonical.starts_with(&canonical_folder) {
            continue;
        }
        let uid = canonical.to_string_lossy().to_string();
        if seen.insert(uid.clone()) {
            games.push(Game {
                uid: uid.clone(),
                title: if ps3_eboot {
                    let game_dir = path.ancestors().nth(3).unwrap_or(path);
                    prettify(
                        game_dir
                            .file_name()
                            .and_then(|v| v.to_str())
                            .unwrap_or("PS3 Game"),
                    )
                } else {
                    prettify(
                        path.file_stem()
                            .and_then(|v| v.to_str())
                            .unwrap_or("Untitled"),
                    )
                },
                path: uid,
                kind: id.into(),
                app_id: None,
            });
        }
    }
    games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(games)
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn scan_console(id: String) -> Result<Vec<Game>, String> {
    scan_console_impl(&id)
}

fn quoted_tokens(line: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut inside = false;
    let mut escape = false;
    for ch in line.chars() {
        if !inside {
            if ch == '"' {
                inside = true;
                current.clear();
            }
            continue;
        }
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        match ch {
            '\\' => escape = true,
            '"' => {
                result.push(current.clone());
                inside = false;
            }
            _ => current.push(ch),
        }
    }
    result
}

fn steam_locations() -> Result<Vec<PathBuf>, String> {
    let home = home()?;
    let candidates = [
        home.join(".steam/steam/steamapps"),
        home.join(".steam/root/steamapps"),
        home.join(".local/share/Steam/steamapps"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps"),
    ];
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for original in candidates {
        if !original.is_dir() {
            continue;
        }
        let canonical = original.canonicalize().unwrap_or(original);
        if seen.insert(canonical.clone()) {
            paths.push(canonical);
        }
    }
    let bases = paths.clone();
    for base in bases {
        let config = base.join("libraryfolders.vdf");
        if let Ok(contents) = fs::read_to_string(config) {
            for line in contents.lines() {
                let fields = quoted_tokens(line);
                if fields.len() != 2 || fields[0] != "path" {
                    continue;
                }
                let next = Path::new(&fields[1]).join("steamapps");
                if next.is_dir() {
                    let real = next.canonicalize().unwrap_or(next);
                    if seen.insert(real.clone()) {
                        paths.push(real);
                    }
                }
            }
        }
    }
    Ok(paths)
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn scan_steam() -> Result<Vec<Game>, String> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();
    for dir in steam_locations()? {
        let entries = match fs::read_dir(dir) {
            Ok(items) => items,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let filename = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
            if !filename.starts_with("appmanifest_") || !filename.ends_with(".acf") {
                continue;
            }
            let contents = match fs::read_to_string(&path) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let mut app_id = None;
            let mut title = None;
            for line in contents.lines() {
                let fields = quoted_tokens(line);
                if fields.len() != 2 {
                    continue;
                }
                match fields[0].as_str() {
                    "appid" => app_id = Some(fields[1].clone()),
                    "name" => {
                        if title.is_none() {
                            title = Some(fields[1].clone())
                        }
                    }
                    _ => (),
                }
            }
            if let Some(id) =
                app_id.filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            {
                if seen.insert(id.clone()) {
                    results.push(Game {
                        uid: id.clone(),
                        title: title.unwrap_or_else(|| format!("Steam App {id}")),
                        path: path.to_string_lossy().into(),
                        kind: "steam".into(),
                        app_id: Some(id),
                    });
                }
            }
        }
    }
    results.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(results)
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn save_emulator(
    id: String,
    emulator_name: String,
    emulator_path: String,
    launch_args: Vec<String>,
) -> Result<(), String> {
    let _ = spec(&id)?;
    if emulator_name.trim().is_empty() || emulator_path.trim().is_empty() {
        return Err("Specify both the emulator name and executable path".into());
    }
    let path = Path::new(&emulator_path);
    if !path.is_file() {
        return Err("Emulator executable does not exist at that path".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode();
        if mode & 0o111 == 0 {
            return Err("Emulator is not executable; grant execute permission first".into());
        }
    }
    let mut settings = load_settings()?;
    let effective_args = if launch_args.is_empty() {
        default_args(&id)
    } else {
        launch_args
    };
    settings.profiles.insert(
        id,
        EmulatorProfile {
            emulator_name,
            emulator_path,
            launch_args: effective_args,
        },
    );
    save_settings(&settings)
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn add_windows_game(title: String, target: String, runner: String) -> Result<WindowsGame, String> {
    if title.trim().is_empty() {
        return Err("Game title is required".into());
    }
    let path = Path::new(&target);
    if !path.is_file() {
        return Err("Game or launcher file does not exist".into());
    }
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !((runner == "wine" && ext == "exe") || (runner == "desktop" && ext == "desktop")) {
        return Err("Choose Wine for an .exe or Desktop launcher for a .desktop file".into());
    }
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    let uid = canonical.to_string_lossy().to_string();
    let mut settings = load_settings()?;
    if settings.windows.iter().any(|g| g.uid == uid) {
        return Err("Game already in Windows library".into());
    }
    let game = WindowsGame {
        uid,
        title: title.trim().into(),
        target: canonical.to_string_lossy().into(),
        runner,
    };
    settings.windows.push(game.clone());
    save_settings(&settings)?;
    Ok(game)
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn list_windows_games() -> Result<Vec<WindowsGame>, String> {
    Ok(load_settings()?.windows)
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn remove_windows_game(uid: String) -> Result<(), String> {
    let mut settings = load_settings()?;
    settings.windows.retain(|entry| entry.uid != uid);
    save_settings(&settings)
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn open_console_folder(id: String) -> Result<(), String> {
    let path = games_dir(&id)?;
    Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn command_for_emulator(path: &str) -> Result<Command, String> {
    let binary = Path::new(path);
    if !binary.is_file() {
        return Err("Emulator executable is missing; check Settings".into());
    }
    Ok(Command::new(binary))
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn launch_console_game(id: String, uid: String) -> Result<(), String> {
    let _ = spec(&id)?;
    let game = scan_console_impl(&id)?
        .into_iter()
        .find(|g| g.uid == uid)
        .ok_or("Game is no longer present in this console's library")?;
    let settings = load_settings()?;
    let profile = settings
        .profiles
        .get(&id)
        .ok_or("Configure an emulator for this console first")?;
    let mut command = command_for_emulator(&profile.emulator_path)?;
    let args = if profile.launch_args.is_empty() {
        vec!["{game}".into()]
    } else {
        profile.launch_args.clone()
    };
    let mut substituted = false;
    for arg in args {
        if arg == "{game}" {
            command.arg(&game.path);
            substituted = true;
        } else {
            command.arg(arg);
        }
    }
    if !substituted {
        command.arg(&game.path);
    }
    command.current_dir(Path::new(&game.path).parent().ok_or("Invalid game path")?);
    command
        .spawn()
        .map_err(|e| format!("Could not start emulator: {e}"))?;
    Ok(())
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn launch_steam_game(uid: String) -> Result<(), String> {
    if !uid.bytes().all(|b| b.is_ascii_digit()) || uid.is_empty() {
        return Err("Invalid Steam app ID".into());
    }
    if !scan_steam()?.into_iter().any(|g| g.uid == uid) {
        return Err("Steam app not found in installed libraries".into());
    }
    // Prefer the system Steam client; Flatpak Steam is supported as a fallback.
    let system = Command::new("steam").arg("-applaunch").arg(&uid).spawn();
    if system.is_ok() {
        return Ok(());
    }
    Command::new("flatpak")
        .args(["run", "com.valvesoftware.Steam", "-applaunch", &uid])
        .spawn()
        .map_err(|e| format!("Steam could not be started: {e}"))?;
    Ok(())
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn launch_windows_game(uid: String) -> Result<(), String> {
    let settings = load_settings()?;
    let game = settings
        .windows
        .iter()
        .find(|g| g.uid == uid)
        .ok_or("Windows game not registered")?;
    if !Path::new(&game.target).is_file() {
        return Err("Windows game or launcher is missing".into());
    }
    let mut command = match game.runner.as_str() {
        "wine" => {
            let mut c = Command::new("wine");
            c.arg(&game.target);
            c
        }
        "desktop" => {
            let mut c = Command::new("gio");
            c.arg("launch").arg(&game.target);
            c
        }
        _ => return Err("Unsupported Windows launcher".into()),
    };
    if game.runner == "wine" {
        command.current_dir(
            Path::new(&game.target)
                .parent()
                .ok_or("Invalid Windows game path")?,
        );
    }
    command
        .spawn()
        .map_err(|e| format!("Could not launch Windows game: {e}"))?;
    Ok(())
}

// Artwork is private to Matrix, not stored in emulator / games directories.
// Titles have stable IDs and SHA-256 filenames to prevent traversal or name collisions.
const MAX_COVER_BYTES: usize = 4 * 1024 * 1024;

fn cover_format(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    let extension = matrix_cover_core::validate_image(bytes).ok()?;
    Some((
        extension,
        match extension {
            "png" => "image/png",
            "jpg" => "image/jpeg",
            _ => "image/webp",
        },
    ))
}

fn cover_key(kind: &str, uid: &str) -> String {
    let digest = Sha256::digest(format!("{kind}\0{uid}").as_bytes());
    format!("{digest:x}")
}

fn validate_game(kind: &str, uid: &str) -> Result<Game, String> {
    if kind == "steam" {
        return scan_steam()?
            .into_iter()
            .find(|g| g.uid == uid)
            .ok_or_else(|| "Steam game is no longer installed".into());
    }
    if kind == "windows" {
        let game = load_settings()?
            .windows
            .into_iter()
            .find(|g| g.uid == uid)
            .ok_or("Windows game no longer registered")?;
        return Ok(Game {
            uid: game.uid,
            title: game.title,
            path: game.target,
            kind: "windows".into(),
            app_id: None,
        });
    }
    // Validate console paths directly instead of rescanning thousands of ROM files
    // for each cover requested as the user scrolls the library.
    let s = spec(kind)?;
    let root = games_dir(kind)?
        .canonicalize()
        .map_err(|_| "Games directory missing")?;
    let game_path = Path::new(uid)
        .canonicalize()
        .map_err(|_| "Game file missing")?;
    if !game_path.starts_with(&root) || !game_path.is_file() {
        return Err("Game is outside this console's library".into());
    }
    let extension = game_path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase();
    let ps3_eboot = kind == "ps3"
        && game_path
            .file_name()
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("EBOOT.BIN"))
        && game_path
            .parent()
            .and_then(|v| v.file_name())
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("USRDIR"));
    if !ps3_eboot && !s.extensions.contains(&extension.as_str()) {
        return Err("Unsupported game format".into());
    }
    // Package installers are not games that can be launched directly.
    if kind == "ps3" && extension == "pkg" {
        return Err("PS3 package needs installation first".into());
    }
    let name = game_path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("Untitled");
    let title = if ps3_eboot {
        prettify(
            game_path
                .ancestors()
                .nth(3)
                .and_then(|p| p.file_name())
                .and_then(|v| v.to_str())
                .unwrap_or("PS3 Game"),
        )
    } else {
        prettify(name)
    };
    let uid = game_path.to_string_lossy().into_owned();
    Ok(Game {
        uid: uid.clone(),
        title,
        path: uid,
        kind: kind.into(),
        app_id: None,
    })
}

fn stored_cover_path(kind: &str, uid: &str, extension: &str) -> Result<PathBuf, String> {
    Ok(artwork_root()?
        .join(kind)
        .join(format!("{}.{}", cover_key(kind, uid), extension)))
}

fn candidate_cover_paths(game: &Game) -> Vec<PathBuf> {
    if game.kind == "steam" {
        let id = game.app_id.as_deref().unwrap_or("");
        let Ok(home) = home() else {
            return Vec::new();
        };
        let roots = [
            home.join(".steam/steam/appcache/librarycache"),
            home.join(".local/share/Steam/appcache/librarycache"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/appcache/librarycache"),
        ];
        let mut candidates = Vec::new();
        for root in roots {
            candidates.push(root.join(format!("{id}_library_600x900.jpg")));
            candidates.push(root.join(format!("{id}_library_600x900.png")));
            candidates.push(root.join(id).join("library_600x900.jpg"));
            candidates.push(root.join(id).join("library_600x900.png"));
        }
        return candidates;
    }
    let path = Path::new(&game.path);
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let mut candidates = Vec::new();
    for extension in ["png", "jpg", "webp"] {
        candidates.push(parent.join(format!("{stem}.{extension}")));
    }
    // A game in its own folder can have cover.png / cover.jpg / cover.webp.
    let unique_game_folder =
        spec(&game.kind).ok().is_some_and(|s| {
            let root = games_dir(&game.kind)
                .ok()
                .and_then(|p| p.canonicalize().ok());
            let parent_path = parent.canonicalize().ok();
            root.is_some()
                && root != parent_path
                && fs::read_dir(parent).ok().is_some_and(|entries| {
                    entries
                        .flatten()
                        .filter(|e| {
                            e.path().is_file()
                                && e.path().extension().and_then(|x| x.to_str()).is_some_and(
                                    |ext| s.extensions.contains(&ext.to_ascii_lowercase().as_str()),
                                )
                        })
                        .take(2)
                        .count()
                        == 1
                })
        });
    if unique_game_folder {
        for extension in ["png", "jpg", "webp"] {
            candidates.push(parent.join(format!("cover.{extension}")));
        }
    }
    candidates
}

fn read_local_cover_bytes(kind: &str, uid: &str, game: &Game) -> Result<Option<Vec<u8>>, String> {
    let mut candidates = Vec::new();
    for extension in ["png", "jpg", "webp"] {
        candidates.push(stored_cover_path(kind, uid, extension)?);
    }
    candidates.extend(candidate_cover_paths(game));
    for candidate in candidates {
        let Ok(meta) = fs::metadata(&candidate) else {
            continue;
        };
        if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_COVER_BYTES as u64 {
            continue;
        }
        let Ok(bytes) = fs::read(candidate) else {
            continue;
        };
        if cover_format(&bytes).is_some() {
            return Ok(Some(bytes));
        }
    }
    Ok(None)
}

fn read_cover_bytes(kind: &str, uid: &str, game: &Game) -> Result<Option<Vec<u8>>, String> {
    if let Some(bytes) = read_local_cover_bytes(kind, uid, game)? {
        return Ok(Some(bytes));
    }
    let settings = load_settings()?;
    if let Some(path) = matrix_cover_core::cached_cover(
        &artwork_root()?.join("online"),
        kind,
        Path::new(&game.path),
        &settings.artwork.style,
    ) {
        if let Ok(meta) = fs::metadata(&path) {
            if meta.len() <= MAX_COVER_BYTES as u64 {
                let bytes = fs::read(path).map_err(|e| e.to_string())?;
                if cover_format(&bytes).is_some() {
                    return Ok(Some(bytes));
                }
            }
        }
    }
    Ok(None)
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn save_artwork_settings(enabled: bool, style: String) -> Result<ArtworkSettings, String> {
    if !["default", "3d"].contains(&style.as_str()) {
        return Err("Unsupported cover style".into());
    }
    let mut settings = load_settings()?;
    settings.artwork = ArtworkSettings { enabled, style };
    save_settings(&settings)?;
    Ok(settings.artwork)
}

fn download_cover_impl(
    kind: String,
    uid: String,
    serial: Option<String>,
) -> Result<matrix_cover_core::Outcome, String> {
    if !["ps1", "ps2"].contains(&kind.as_str()) {
        return Err("Automatic artwork supports PS1 and PS2".into());
    }
    let game = validate_game(&kind, &uid)?;
    let settings = load_settings()?;
    if !settings.artwork.enabled && serial.is_none() {
        return Ok(matrix_cover_core::Outcome {
            status: "disabled".into(),
            message: "Automatic covers are disabled".into(),
            ..Default::default()
        });
    }
    if read_local_cover_bytes(&kind, &uid, &game)?.is_some() {
        return Ok(matrix_cover_core::Outcome {
            status: "local".into(),
            message: "Using your local cover".into(),
            ..Default::default()
        });
    }
    matrix_cover_core::fetch(
        &artwork_root()?.join("online"),
        &kind,
        Path::new(&game.path),
        &settings.artwork.style,
        serial.as_deref(),
        &mut matrix_cover_core::Http,
    )
}

#[cfg(feature = "desktop")]
#[tauri::command]
async fn download_cover(
    kind: String,
    uid: String,
    serial: Option<String>,
) -> Result<matrix_cover_core::Outcome, String> {
    // Blocking filesystem, decoding and network work stays off the UI thread.
    tauri::async_runtime::spawn_blocking(move || download_cover_impl(kind, uid, serial))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn open_cover_project() -> Result<(), String> {
    // Explicit user action opens the official site; Matrix never scrapes it or bypasses challenges.
    Command::new("xdg-open")
        .arg("https://www.thecoverproject.net/")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot open TheCoverProject: {e}"))
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn get_cover(kind: String, uid: String) -> Result<Option<String>, String> {
    let game = validate_game(&kind, &uid)?;
    let Some(bytes) = read_cover_bytes(&kind, &uid, &game)? else {
        return Ok(None);
    };
    let Some((_, mime)) = cover_format(&bytes) else {
        return Ok(None);
    };
    Ok(Some(format!("data:{mime};base64,{}", BASE64.encode(bytes))))
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn save_cover(kind: String, uid: String, bytes: Vec<u8>) -> Result<(), String> {
    let _ = validate_game(&kind, &uid)?;
    if bytes.is_empty() || bytes.len() > MAX_COVER_BYTES {
        return Err("Cover must be 4 MB or smaller".into());
    }
    let (extension, _) =
        cover_format(&bytes).ok_or("Only PNG, JPEG, and WebP images are supported")?;
    let target = stored_cover_path(&kind, &uid, extension)?;
    fs::create_dir_all(target.parent().ok_or("Invalid artwork directory")?)
        .map_err(|e| format!("Could not create artwork directory: {e}"))?;
    let temp = target.with_extension(format!("{}.tmp", extension));
    fs::write(&temp, &bytes).map_err(|e| format!("Could not save artwork: {e}"))?;
    fs::rename(&temp, &target).map_err(|e| format!("Could not finalize artwork: {e}"))?;
    for old in ["png", "jpg", "webp"] {
        if old != extension {
            let _ = fs::remove_file(stored_cover_path(&kind, &uid, old)?);
        }
    }
    Ok(())
}

#[cfg(feature = "desktop")]
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            scan_console,
            scan_steam,
            save_emulator,
            add_windows_game,
            list_windows_games,
            remove_windows_game,
            open_console_folder,
            launch_console_game,
            launch_steam_game,
            launch_windows_game,
            get_cover,
            save_cover,
            download_cover,
            save_artwork_settings,
            open_cover_project,
        ])
        .run(tauri::generate_context!())
        .expect("Error running Matrix frontend");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_console_paths_are_unique_and_valid() {
        let mut ids = HashSet::new();
        let mut directories = HashSet::new();
        for s in SPECS {
            assert!(ids.insert(s.id));
            assert!(directories.insert(s.dir));
            assert!(s.games.ends_with(" Games"));
        }
        assert_eq!(SPECS.len(), 14);
    }
    #[test]
    fn parse_vdf_tokens() {
        assert_eq!(
            quoted_tokens("  \"path\" \"/home/bless/Games/Steam\""),
            vec!["path", "/home/bless/Games/Steam"]
        );
        assert_eq!(
            quoted_tokens("\"appid\" \"123456\""),
            vec!["appid", "123456"]
        );
    }
    #[test]
    fn cover_validation_is_content_based() {
        assert!(cover_format(b"\x89PNG\r\n\x1a\nhello").is_none());
        assert!(cover_format(b"\xff\xd8\xfftest").is_none());
        assert_eq!(
            cover_format(include_bytes!("../../tests/fixtures/demo-cover.png"))
                .unwrap()
                .0,
            "png"
        );
        assert!(cover_format(b"not an image").is_none());
        assert_eq!(
            cover_key("ps1", "/games/file.iso"),
            cover_key("ps1", "/games/file.iso")
        );
        assert_ne!(
            cover_key("ps1", "/games/file.iso"),
            cover_key("ps2", "/games/file.iso")
        );
    }
    #[test]
    fn game_title_cleanup_does_not_touch_filename() {
        assert_eq!(prettify("Super_Game_(USA)"), "Super Game");
    }
    #[test]
    fn actual_backend_artwork_lifecycle_in_isolated_home() {
        let temp = tempfile::tempdir().unwrap();
        let names = ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"];
        let previous: Vec<_> = names.iter().map(std::env::var_os).collect();
        struct Restore {
            names: [&'static str; 3],
            values: Vec<Option<std::ffi::OsString>>,
        }
        impl Drop for Restore {
            fn drop(&mut self) {
                for (name, value) in self.names.iter().zip(&self.values) {
                    match value {
                        Some(v) => std::env::set_var(name, v),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
        let _restore = Restore {
            names,
            values: previous,
        };
        std::env::set_var("HOME", temp.path());
        std::env::set_var("XDG_CONFIG_HOME", temp.path().join("config"));
        std::env::set_var("XDG_DATA_HOME", temp.path().join("data"));
        create_layout().unwrap();
        assert_eq!(SPECS.len(), 14);
        let root = games_dir("ps2").unwrap();
        let path = root.join("SLUS-20312.chd");
        fs::write(&path, b"synthetic fixture").unwrap();
        let uid = path.canonicalize().unwrap().to_string_lossy().into_owned();
        assert_eq!(scan_console_impl("ps2").unwrap().len(), 1);
        assert!(scan_console_impl("ps1").unwrap().is_empty());
        assert!(validate_game("ps1", &uid).is_err());
        assert!(get_cover("ps2".into(), uid.clone()).unwrap().is_none());
        let mut settings = Settings::default();
        settings.profiles.insert(
            "ps2".into(),
            EmulatorProfile {
                emulator_name: "Keep profile".into(),
                ..Default::default()
            },
        );
        save_settings(&settings).unwrap();
        save_artwork_settings(false, "3d".into()).unwrap();
        assert_eq!(
            load_settings().unwrap().profiles["ps2"].emulator_name,
            "Keep profile"
        );
        assert_eq!(
            download_cover_impl("ps2".into(), uid.clone(), None)
                .unwrap()
                .status,
            "disabled"
        );
        assert!(save_artwork_settings(true, "bad-style".into()).is_err());
        let bytes = include_bytes!("../../tests/fixtures/demo-cover.png").to_vec();
        // A generic image in the shared library must not label every game.
        fs::write(root.join("cover.png"), &bytes).unwrap();
        assert!(get_cover("ps2".into(), uid.clone()).unwrap().is_none());
        struct OneImage(Vec<u8>);
        impl matrix_cover_core::Transport for OneImage {
            fn get(&mut self, _: &str) -> Result<matrix_cover_core::Reply, String> {
                Ok(matrix_cover_core::Reply {
                    status: 200,
                    bytes: self.0.clone(),
                    retry_after: None,
                })
            }
        }
        matrix_cover_core::fetch(
            &artwork_root().unwrap().join("online"),
            "ps2",
            &path,
            "3d",
            None,
            &mut OneImage(bytes.clone()),
        )
        .unwrap();
        // Actual backend loads the downloaded core cache even with automatic artwork off.
        assert!(get_cover("ps2".into(), uid.clone())
            .unwrap()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(save_cover("ps2".into(), uid.clone(), b"fake PNG".to_vec()).is_err());
        save_cover("ps2".into(), uid.clone(), bytes).unwrap();
        save_artwork_settings(true, "default".into()).unwrap();
        assert_eq!(
            download_cover_impl("ps2".into(), uid.clone(), None)
                .unwrap()
                .status,
            "local"
        );
        assert!(get_cover("ps2".into(), uid).unwrap().is_some());
    }
}
