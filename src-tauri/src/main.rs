#![cfg_attr(
    all(not(debug_assertions), target_os = "windows", feature = "desktop"),
    windows_subsystem = "windows"
)]

mod appimage_environment;
mod artwork;
mod steamgrid;
mod library_tools;
mod library_features;
mod save_data;
mod bounded_io;
#[cfg(feature = "desktop")]
mod desktop_commands;
#[cfg(test)]
mod console_library_tests;
mod discovery;
mod emulators;
mod favorites;
mod immersive;
mod installed_titles;
mod launch_log;
mod launches;
mod libraries;
mod local_artwork;
mod profile_saves;
mod settings;
mod steam_libraries;
mod update_lifecycle;
mod update_worker;
mod updates;

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
        extensions: &["iso", "cso", "pbp", "chd"],
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
        extensions: &["iso", "gcm", "rvz", "gcz", "wia", "ciso"],
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
        extensions: &["wbfs", "iso", "rvz", "wia", "gcz", "ciso"],
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
    ConsoleSpec {
        id: "ps4",
        dir: "PS4",
        games: "PS4 Games",
        extensions: &["elf"],
    },
    ConsoleSpec {
        id: "ps5",
        dir: "PS5",
        games: "PS5 Games",
        // Keep legacy collection formats; a custom emulator decides what it can load.
        extensions: &["pkg", "iso", "elf", "zar"],
    },
    ConsoleSpec {
        id: "vita",
        dir: "PS Vita",
        games: "PS Vita Games",
        extensions: &["vpk", "zip"],
    },
    ConsoleSpec {
        id: "switch2",
        dir: "Switch 2",
        games: "Switch 2 Games",
        // Collection recognition only; a custom emulator decides what it can load.
        extensions: &[
            "nsp", "xci", "nup", "nsz", "xcz", "nx", "nro", "nso", "nca", "kip",
        ],
    },
    ConsoleSpec { id: "genesis", dir: "Sega Genesis", games: "Sega Genesis Games", extensions: &["md", "gen", "smd", "bin", "zip"] },
    ConsoleSpec { id: "saturn", dir: "Sega Saturn", games: "Sega Saturn Games", extensions: &["cue", "ccd", "toc"] },
    ConsoleSpec { id: "snes", dir: "Super Nintendo", games: "Super Nintendo Games", extensions: &["sfc", "smc", "zip"] },
    ConsoleSpec { id: "nes", dir: "Nintendo NES", games: "Nintendo NES Games", extensions: &["nes", "unf", "unif", "fds", "zip"] },
    ConsoleSpec { id: "xbox", dir: "Xbox", games: "Xbox Games", extensions: &["iso"] },
    ConsoleSpec { id: "xbox360", dir: "Xbox 360", games: "Xbox 360 Games", extensions: &["iso"] },

];

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EmulatorProfile {
    emulator_name: String,
    #[serde(default)]
    auto_detected: bool,
    emulator_path: String,
    #[serde(default)]
    launcher: emulators::Launcher,
    #[serde(default)]
    flatpak_id: String,
    #[serde(default)]
    flatpak_installation: String,
    #[serde(default)]
    flatpak_runtime_fonts: Option<bool>,
    #[serde(default)]
    extract_appimage: bool,
    /// Each entry is a literal argv token. Custom PS5/Switch 2 profiles also
    /// support {gameDir}; placeholders must occupy a whole token, never shell text.
    #[serde(default)]
    launch_args: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsGame {
    uid: String,
    title: String,
    target: String,
    // Retain unknown legacy values for settings compatibility; only desktop is launchable.
    runner: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ArtworkSettings {
    enabled: bool,
    style: String,
    // Unlike ID providers, title lookups disclose a game filename stem.
    // Existing installations opt in before automatic scans expand scope.
    title_enabled: bool,
}
impl Default for ArtworkSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            style: "default".into(),
            title_enabled: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
struct Settings {
    #[serde(default)]
    library_preferences: library_features::Preferences,
    #[serde(default)]
    library_index: BTreeMap<String,Vec<Game>>,
    #[serde(default)]
    recent: Vec<library_tools::Recent>,
    #[serde(default)]
    game_options: BTreeMap<String,library_tools::GameOptions>,
    #[serde(default)]
    steamgriddb_api_key: String,
    #[serde(default)]
    steam_libraries: Vec<steam_libraries::RegisteredLibrary>,
    #[serde(default)]
    favorites: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    favorites_migrated: bool,
    #[serde(default)]
    library_folders: BTreeMap<String, Vec<String>>,
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
    library_preferences: library_features::Preferences,
    recent: Vec<library_tools::Recent>,
    ready_profiles: BTreeMap<String,bool>,
    steamgrid_enabled: bool,
    emulator_discovery: discovery::Report,
    favorites: BTreeMap<String, Vec<String>>,
    favorites_migrated: bool,
    artwork: ArtworkSettings,
    root: String,
    folders: BTreeMap<String, String>,
    library_folders: BTreeMap<String, Vec<libraries::Folder>>,
    profiles: BTreeMap<String, EmulatorProfile>,
    windows: Vec<WindowsGame>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
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
    let games = home()?.join("Games");
    let legacy = games.join("Matrix");
    // Existing libraries stay in place across the product rename.
    Ok(if legacy.is_dir() {
        legacy
    } else {
        games.join("Matrix Play")
    })
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

// JSON paths must round-trip to exactly the same filesystem object. Never use
// replacement characters for identifiers or paths that can later be launched.
fn actionable_path(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| "This path contains non-UTF-8 filename bytes and cannot be used safely. Choose a UTF-8 path; no files have been changed.".into())
}

fn games_dir(id: &str) -> Result<PathBuf, String> {
    let s = spec(id)?;
    Ok(matrix_root()?.join(s.dir).join(s.games))
}

fn console_covers_dir(id: &str) -> Result<PathBuf, String> {
    Ok(matrix_root()?.join(spec(id)?.dir).join("Covers"))
}

fn custom_emulator_only(id: &str) -> bool {
    matches!(id, "ps5" | "switch2" | "xbox360")
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
        "ps4" => "shadPS4",
        "vita" => "Vita3K",
        "genesis" | "saturn" | "nes" | "snes" => "Mednafen",
        "xbox" => "xemu",
        "xbox360" => "Custom Xbox 360 emulator",
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

/// List executable candidates without guessing when several are available.
fn detect_emulators(s: &ConsoleSpec) -> Vec<PathBuf> {
    matrix_root()
        .map(|root| detect_emulators_in(s, &root.join(s.dir)))
        .unwrap_or_default()
}
fn detect_emulators_in(s: &ConsoleSpec, dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for file in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = file.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let name = name.to_lowercase();
        let recognized = name.ends_with(".appimage")
            || name.ends_with(".sh")
            || name.contains(&guessed_emulator(s.id).to_lowercase().replace(' ', ""));
        if !recognized {
            continue;
        }
        if !emulators::executable_file(&path) {
            continue;
        }
        candidates.push(path);
    }
    candidates.sort();
    candidates
}

/// List all installed native choices, without executing or changing their files.
fn detect_installed_emulators(id: &str) -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|paths| {
            detect_installed_emulators_in(id, &std::env::split_paths(&paths).collect::<Vec<_>>())
        })
        .unwrap_or_default()
}
fn detect_installed_emulators_in(id: &str, paths: &[PathBuf]) -> Vec<PathBuf> {
    let names: &[&str] = match id {
        "switch" => &["eden", "ryujinx"],
        "ps1" => &["duckstation-qt", "duckstation"],
        "ps2" => &["pcsx2-qt", "pcsx2"],
        "ps3" => &["rpcs3"],
        "psp" => &["PPSSPPSDL", "ppsspp"],
        "n64" => &["RMG", "rmg"],
        "gamecube" | "wii" => &["dolphin-emu"],
        "dreamcast" => &["flycast"],
        "wiiu" => &["cemu"],
        "gb" => &["sameboy", "mgba-qt", "mgba"],
        "gba" => &["mgba-qt", "mgba"],
        "nds" => &["melonDS", "melonds"],
        "3ds" => &["azahar"],
        "ps4" => &["shadPS4QtLauncher", "shadPS4"],
        "vita" => &["Vita3K", "vita3k"],
        "nes" | "snes" | "genesis" | "saturn" => &["mednafen"],
        "xbox" => &["xemu"],
        _ => &[],
    };
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for directory in paths {
        for name in names {
            let path = directory.join(name);
            if !path.is_file() {
                continue;
            }
            if !emulators::executable_file(&path) {
                continue;
            }
            let canonical = path.canonicalize().unwrap_or(path);
            if seen.insert(canonical.clone()) {
                candidates.push(canonical);
            }
        }
    }
    candidates
}

fn create_layout() -> Result<(), String> {
    for s in SPECS {
        let path = matrix_root()?.join(s.dir).join(s.games);
        fs::create_dir_all(&path).map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
        let covers = console_covers_dir(s.id)?;
        fs::create_dir_all(&covers)
            .map_err(|e| format!("Cannot create {}: {e}", covers.display()))?;
    }
    Ok(())
}

fn load_settings() -> Result<Settings, String> {
    settings::load(&config_file()?)
}

/// Every writer must read and mutate the latest document while holding this lock.
fn update_settings<T>(
    change: impl FnOnce(&mut Settings) -> Result<T, String>,
) -> Result<T, String> {
    settings::update(&config_file()?, change)
}

fn bootstrap() -> Result<Bootstrap, String> {
    create_layout()?;
    // Discovery may be slow. Merge its candidates into the latest settings only
    // after it finishes, so another window's saves and manual choices win.
    let mut emulator_discovery = discovery::discover()?;
    let settings = update_settings(|settings| {
        discovery::auto_link(settings, &mut emulator_discovery);
        Ok(settings.clone())
    })?;
    let root = matrix_root()?;
    let mut folders = BTreeMap::new();
    for s in SPECS {
        folders.insert(
            s.id.into(),
            root.join(s.dir).join(s.games).to_string_lossy().into(),
        );
    }
    let mut library_folders = BTreeMap::new();
    for s in SPECS {
        library_folders.insert(s.id.into(), libraries::folders(s.id)?);
    }
    let ready_profiles=settings.profiles.iter().map(|(id,p)|(id.clone(),if p.launcher==emulators::Launcher::Flatpak {emulator_discovery.flatpak_inventory.as_ref().is_some_and(|apps|apps.iter().any(|a|a.app_id==p.flatpak_id&&a.installation==p.flatpak_installation))} else {emulators::executable_file(Path::new(&p.emulator_path))})).collect();
    Ok(Bootstrap {
        library_preferences: settings.library_preferences,
        ready_profiles,
        recent: settings.recent,
        steamgrid_enabled: !settings.steamgriddb_api_key.is_empty(),
        library_folders,
        emulator_discovery,
        favorites: settings.favorites,
        favorites_migrated: settings.favorites_migrated,
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

fn is_ps3_eboot(id: &str, path: &Path) -> bool {
    id == "ps3"
        && path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("EBOOT.BIN"))
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n.eq_ignore_ascii_case("USRDIR"))
}

// Disc dumps wrap the title in PS3_GAME; installed digital titles do not.
fn ps3_title(path: &Path) -> String {
    let content = path.ancestors().nth(2);
    let title_dir = content.and_then(|dir| {
        if dir
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("PS3_GAME"))
        {
            dir.parent()
        } else {
            Some(dir)
        }
    });
    prettify(
        title_dir
            .and_then(Path::file_name)
            .and_then(|v| v.to_str())
            .unwrap_or("PS3 Game"),
    )
}

fn is_ps4_eboot(id: &str, path: &Path) -> bool {
    id == "ps4"
        && path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("eboot.bin"))
        && path
            .parent()
            .is_some_and(|p| p.join("sce_sys/param.sfo").is_file())
}

fn is_ps5_eboot(id: &str, path: &Path) -> bool {
    id == "ps5"
        && path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("eboot.bin"))
}

// An extracted Switch 2 title is represented by its actual ExeFS boot file.
// Keep each base/update path independently addressable; never pick an update by
// traversal order or change existing package identities, favorites or covers.
fn switch2_extracted_title(path: &Path) -> Option<String> {
    if !path.file_name()?.eq_ignore_ascii_case("main") {
        return None;
    }
    let exefs = path.parent()?;
    if !exefs.file_name()?.eq_ignore_ascii_case("exefs") {
        return None;
    }
    let content = exefs.parent()?;
    let name = content.file_name()?;
    let storage = content.parent()?;
    let (title, suffix) = if storage
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("updates"))
    {
        (
            storage.parent()?,
            format!(" — Update {}", name.to_string_lossy()),
        )
    } else if name.eq_ignore_ascii_case("base") {
        (storage, " — Base".into())
    } else {
        (content, String::new())
    };
    Some(format!(
        "{}{}",
        prettify(title.file_name()?.to_str()?),
        suffix
    ))
}

// Scanning and direct validation must present the same title without changing
// the canonical path used by launches, favorites or artwork.
fn console_game_title(id: &str, path: &Path, roots: &[PathBuf]) -> String {
    if let Some(title) = installed_titles::title(id, path, roots) {
        return title;
    }
    if is_ps3_eboot(id, path) {
        return ps3_title(path);
    }
    if id == "xbox360" && path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("default.xex")) {
        return prettify(path.parent().and_then(Path::file_name).and_then(|n| n.to_str()).unwrap_or("Xbox 360 Game"));
    }
    if id == "switch2" {
        if let Some(title) = switch2_extracted_title(path) {
            return title;
        }
    }
    let name = if is_ps4_eboot(id, path) || is_ps5_eboot(id, path) {
        path.parent().and_then(Path::file_name)
    } else {
        path.file_stem()
    };
    prettify(name.and_then(|v| v.to_str()).unwrap_or("Untitled"))
}

fn supported_console_file(s: &ConsoleSpec, path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase();
    // PS3 packages need installation, and PS4 packages are never boot targets.
    (s.extensions.contains(&extension.as_str()) && !(s.id == "ps3" && extension == "pkg"))
        || is_ps3_eboot(s.id, path)
        || is_ps4_eboot(s.id, path)
        || is_ps5_eboot(s.id, path)
        || (s.id == "xbox360" && path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("default.xex")))
        || (s.id == "switch2" && switch2_extracted_title(path).is_some())
        || libraries::is_playlist(s.id, path)
}

fn scan_console_impl(id: &str) -> Result<Vec<Game>, String> {
    let settings = load_settings()?;
    let favorites = settings
        .favorites
        .get(id)
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    scan_console_with_customizations(
        id,
        &libraries::roots(id)?,
        &favorites,
        Some(&artwork_root()?),
    )
}

fn scan_console_in(id: &str, roots: &[PathBuf]) -> Result<Vec<Game>, String> {
    scan_console_with_customizations(id, roots, &HashSet::new(), None)
}

fn scan_console_with_customizations(
    id: &str,
    roots: &[PathBuf],
    favorites: &HashSet<String>,
    covers: Option<&Path>,
) -> Result<Vec<Game>, String> {
    let s = spec(id)?;
    let mut games = vec![];
    let mut seen = HashSet::new();
    let mut grouped_members = HashSet::new();
    for folder in roots {
        let canonical_folder = match folder.canonicalize() {
            Ok(path) => path,
            Err(_) => continue,
        };
        // Avoid symlink loops, hidden dirs, very deep trees, and accidental unlimited scanning.
        for entry in WalkDir::new(&folder)
            .follow_links(false)
            .max_depth(9)
            .into_iter()
            .filter_entry(|e| {
                e.depth() == 0
                    || (!e.file_name().to_string_lossy().starts_with('.')
                        // Extracted RomFS trees contain assets, not launch targets.
                        // Avoid walking hundreds of thousands of payload files.
                        && !(id == "switch2"
                            && e.file_type().is_dir()
                            && e.file_name().eq_ignore_ascii_case("romfs")))
            })
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            // PS1 ISO/CUE companions consume two temporary rows but only one
            // logical game. Keep the previous 8,000-game capacity after grouping.
            if games.len() >= if id == "ps1" { 16000 } else { 8000 } {
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
            if !supported_console_file(s, path) {
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
            let Ok(uid) = actionable_path(&canonical) else {
                continue;
            };
            if libraries::is_playlist(id, &canonical) {
                match libraries::playlist_members(id, &canonical, roots) {
                    Ok(members) => grouped_members.extend(members),
                    Err(_) => continue,
                }
            }
            if seen.insert(uid.clone()) {
                games.push(Game {
                    uid: uid.clone(),
                    title: console_game_title(id, &canonical, roots),
                    path: uid,
                    kind: id.into(),
                    app_id: None,
                });
            }
        }
    }
    // Only scanned, regular, in-scope CUE files can suppress their ISO companion.
    // Extension spelling is case-insensitive, but Linux filename stems are not.
    let cue_stems: HashSet<PathBuf> = if id == "ps1" {
        games
            .iter()
            .map(|game| Path::new(&game.path))
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("cue"))
            })
            .map(|path| path.with_extension(""))
            .collect()
    } else {
        HashSet::new()
    };
    // Keep customized ISO/disc IDs directly reachable after CUE/playlist grouping.
    // Do not migrate favorites or overwrite covers with a different identity.
    games.retain(|game| {
        let path = Path::new(&game.path);
        let cue_companion = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("iso"))
            && cue_stems.contains(&path.with_extension(""));
        (!grouped_members.contains(path) && !cue_companion)
            || favorites.contains(&game.uid)
            || covers.is_some_and(|root| libraries::has_manual_cover(root, id, &game.uid))
    });
    games.truncate(8000);
    games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(games)
}

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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SteamLauncher {
    Native,
    Flatpak,
    Snap,
}
#[derive(Debug)]
struct SteamLibrary {
    path: PathBuf,
    launcher: SteamLauncher,
}
struct SteamGame {
    game: Game,
    launcher: SteamLauncher,
}
fn steam_locations_in(
    home: &Path,
    registered: &[steam_libraries::RegisteredLibrary],
) -> Vec<SteamLibrary> {
    let flatpak_root = home.join(".var/app/com.valvesoftware.Steam");
    let flatpak_root = flatpak_root.canonicalize().unwrap_or(flatpak_root);
    let snap_root = home.join("snap/steam");
    let snap_root = snap_root.canonicalize().unwrap_or(snap_root);
    let candidates = [
        (home.join(".steam/steam/steamapps"), SteamLauncher::Native),
        (home.join(".steam/root/steamapps"), SteamLauncher::Native),
        (
            home.join(".local/share/Steam/steamapps"),
            SteamLauncher::Native,
        ),
        (
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps"),
            SteamLauncher::Flatpak,
        ),
        (
            home.join("snap/steam/common/.local/share/Steam/steamapps"),
            SteamLauncher::Snap,
        ),
        (
            home.join("snap/steam/common/.steam/steam/steamapps"),
            SteamLauncher::Snap,
        ),
    ];
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    // Explicit choices win over automatic roots, including different clients
    // sharing a library. Keep saved registrations even while a drive is offline.
    for entry in registered.iter().take(32) {
        let original = PathBuf::from(&entry.path);
        if let Ok(canonical) = original.canonicalize() {
            if canonical.is_dir() && seen.insert(canonical.clone()) {
                paths.push(SteamLibrary {
                    path: canonical,
                    launcher: entry.launcher,
                });
            }
        }
    }
    for (original, mut launcher) in candidates {
        if !original.is_dir() {
            continue;
        }
        let canonical = original.canonicalize().unwrap_or(original);
        // A legacy ~/.steam alias can point into Flatpak's data directory.
        if canonical.starts_with(&flatpak_root) {
            launcher = SteamLauncher::Flatpak;
        } else if canonical.starts_with(&snap_root) {
            launcher = SteamLauncher::Snap;
        }
        if seen.insert(canonical.clone()) {
            paths.push(SteamLibrary {
                path: canonical,
                launcher,
            });
        }
    }
    let mut additional = Vec::new();
    for base in &paths {
        if let Some(contents) = steam_libraries::bounded_text(&base.path.join("libraryfolders.vdf"))
        {
            for line in contents.lines() {
                let fields = quoted_tokens(line);
                if fields.len() != 2 || fields[0] != "path" {
                    continue;
                }
                if paths.len() + additional.len() >= steam_libraries::MAX_LIBRARIES {
                    break;
                }
                let library_path = Path::new(&fields[1]);
                if !library_path.is_absolute() {
                    continue;
                }
                let next = library_path.join("steamapps");
                if next.is_dir() {
                    let real = next.canonicalize().unwrap_or(next);
                    if seen.insert(real.clone()) {
                        additional.push(SteamLibrary {
                            path: real,
                            launcher: base.launcher,
                        });
                    }
                }
            }
        }
    }
    paths.extend(additional);
    paths
}
fn scan_steam_in(home: &Path, registered: &[steam_libraries::RegisteredLibrary]) -> Vec<SteamGame> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();
    for library in steam_locations_in(home, registered) {
        let entries = match fs::read_dir(&library.path) {
            Ok(items) => items,
            Err(_) => continue,
        };
        for entry in entries.flatten().take(10_000) {
            let path = entry.path();
            let filename = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
            if !filename.starts_with("appmanifest_") || !filename.ends_with(".acf") {
                continue;
            }
            let Some(contents) = steam_libraries::bounded_text(&path) else {
                continue;
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
                    "name" if title.is_none() => title = Some(fields[1].clone()),
                    _ => (),
                }
            }
            if let Some(id) =
                app_id.filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            {
                if seen.insert(id.clone()) {
                    results.push(SteamGame {
                        game: Game {
                            uid: id.clone(),
                            title: title.unwrap_or_else(|| format!("Steam App {id}")),
                            path: path.to_string_lossy().into(),
                            kind: "steam".into(),
                            app_id: Some(id),
                        },
                        launcher: library.launcher,
                    });
                }
            }
        }
    }
    results.sort_by(|a, b| {
        a.game
            .title
            .to_lowercase()
            .cmp(&b.game.title.to_lowercase())
    });
    results
}
fn scan_steam() -> Result<Vec<Game>, String> {
    Ok(scan_steam_in(&home()?, &load_settings()?.steam_libraries)
        .into_iter()
        .map(|g| g.game)
        .collect())
}
fn steam_command(uid: &str, launcher: SteamLauncher) -> Command {
    let mut command = match launcher {
        SteamLauncher::Native => launch_command("steam"),
        SteamLauncher::Flatpak => {
            let mut c = launch_command("flatpak");
            c.args(["run", "com.valvesoftware.Steam"]);
            c
        }
        SteamLauncher::Snap => {
            let mut c = launch_command("snap");
            c.args(["run", "steam"]);
            c
        }
    };
    command.args(["-applaunch", uid]);
    command
}

// Setup refresh is read-only: unlike bootstrap, it must never auto-link or
// rewrite a profile while the user is editing another choice.
#[cfg(feature = "desktop")]
#[tauri::command]
async fn discover_emulators() -> Result<discovery::Report, String> {
    // Filesystem/Flatpak scans must not block the window's event loop. Browse,
    // editing and Cancel remain usable while a slow installation is checked.
    tauri::async_runtime::spawn_blocking(discovery::discover)
        .await
        .map_err(|error| error.to_string())?
}

fn save_emulator(
    request: profile_saves::Request,
    id: String,
    emulator_name: String,
    emulator_path: String,
    launch_args: Vec<String>,
    launcher: Option<String>,
    flatpak_id: Option<String>,
    flatpak_installation: Option<String>,
    allow_execute: Option<bool>,
    extract_appimage: Option<bool>,
    flatpak_runtime_fonts: Option<bool>,
) -> Result<(), String> {
    spec(&id)?;
    let launcher = match launcher.as_deref().unwrap_or("executable") {
        "executable" => emulators::Launcher::Executable,
        "appimage" => emulators::Launcher::Appimage,
        "flatpak" => emulators::Launcher::Flatpak,
        _ => return Err("Choose AppImage, Flatpak or executable.".into()),
    };
    let profile = emulators::prepare(
        EmulatorProfile {
            emulator_name,
            auto_detected: false,
            emulator_path,
            flatpak_runtime_fonts,
            launcher,
            flatpak_id: flatpak_id.unwrap_or_default(),
            flatpak_installation: flatpak_installation.unwrap_or_default(),
            extract_appimage: extract_appimage.unwrap_or(false),
            launch_args: if launch_args.is_empty() {
                default_args(&id)
            } else {
                launch_args
            },
        },
        allow_execute.unwrap_or(false),
    )?;
    immersive::validate_profile_paths(&profile, &id)?;
    profile_saves::commit(request, &id, || {
        update_settings(|settings| {
            settings.profiles.insert(id.clone(), profile);
            Ok(())
        })
    })
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
    validate_windows_shortcut(path, &runner)?;
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    validate_windows_shortcut(&canonical, &runner)?;
    let uid = actionable_path(&canonical)?;
    let game = WindowsGame {
        uid: uid.clone(),
        title: title.trim().into(),
        target: uid,
        runner,
    };
    update_settings(|settings| {
        if settings.windows.iter().any(|g| g.uid == game.uid) {
            return Err("Game already in Windows library".into());
        }
        settings.windows.push(game.clone());
        Ok(game)
    })
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn list_windows_games() -> Result<Vec<WindowsGame>, String> {
    Ok(load_settings()?.windows)
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn remove_windows_game(uid: String) -> Result<(), String> {
    update_settings(|settings| {
        settings.windows.retain(|entry| entry.uid != uid);
        Ok(())
    })
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn open_console_folder(id: String) -> Result<(), String> {
    let path = games_dir(&id)?;
    launch_command("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn console_launch_args(
    profile: &EmulatorProfile,
    id: &str,
    game: &Path,
) -> Result<Vec<String>, String> {
    let mut substituted = false;
    let mut args = Vec::new();
    for arg in immersive::game_args(profile, id) {
        let path = if arg == "{game}" {
            Some(game)
        } else if custom_emulator_only(id) && arg == "{gameDir}" {
            Some(game.parent().ok_or("Invalid game path")?)
        } else {
            None
        };
        if let Some(path) = path {
            args.push(actionable_path(path)?);
            substituted = true;
        } else {
            args.push(arg);
        }
    }
    if !substituted {
        args.push(actionable_path(game)?);
    }
    Ok(args)
}

fn launch_console_impl(id: String, uid: String) -> Result<launches::Info, String> {
    spec(&id)?;
    let game = scan_console_impl(&id)?
        .into_iter()
        .find(|g| g.uid == uid)
        .ok_or("Game is no longer present in this console's library")?;
    let settings = load_settings()?;
    let mut selected=settings.profiles.get(&id).ok_or("Configure an emulator for this console first")?.clone();
    if let Some(options)=settings.game_options.get(&cover_key(&id,&uid)){selected.launch_args=options.args.clone();}
    let profile=&selected;
    launches::preflight(profile)?;
    let mut command = emulators::command(profile, &id, Some(Path::new(&game.path)))?;
    command.args(console_launch_args(profile, &id, Path::new(&game.path))?);
    command.current_dir(Path::new(&game.path).parent().ok_or("Invalid game path")?);
    let info=launches::start(command, &id)?;
    let _=library_tools::record(&game);
    Ok(info)
}

#[cfg(feature = "desktop")]
#[tauri::command]
async fn launch_console_game(id: String, uid: String) -> Result<launches::Info, String> {
    tauri::async_runtime::spawn_blocking(move || launch_console_impl(id, uid))
        .await
        .map_err(|e| e.to_string())?
}
fn open_emulator_impl(id: String) -> Result<launches::Info, String> {
    spec(&id)?;
    let settings = load_settings()?;
    let profile = settings
        .profiles
        .get(&id)
        .ok_or("Set up an emulator for this console first.")?;
    let mut command = emulators::command(profile, &id, None)?;
    command.args(immersive::setup_args(profile, &id));
    command.current_dir(home()?);
    launches::start(command, &id)
}
#[cfg(feature = "desktop")]
#[tauri::command]
async fn open_emulator(id: String) -> Result<launches::Info, String> {
    tauri::async_runtime::spawn_blocking(move || open_emulator_impl(id))
        .await
        .map_err(|e| e.to_string())?
}

fn launch_steam_impl(uid: String) -> Result<launches::Info, String> {
    if !uid.bytes().all(|b| b.is_ascii_digit()) || uid.is_empty() {
        return Err("Invalid Steam app ID".into());
    }
    let game = scan_steam_in(&home()?, &load_settings()?.steam_libraries)
        .into_iter()
        .find(|g| g.game.uid == uid)
        .ok_or("Steam app not found in installed libraries")?;
    // Never silently switch installations: their libraries and settings differ.
    let info=launches::start(steam_libraries::command_for_launch(&uid, game.launcher)?, "steam")?;
    let _=library_tools::record(&game.game);
    Ok(info)
}
#[cfg(feature = "desktop")]
#[tauri::command]
async fn launch_steam_game(uid: String) -> Result<launches::Info, String> {
    tauri::async_runtime::spawn_blocking(move || launch_steam_impl(uid))
        .await
        .map_err(|e| e.to_string())?
}
fn validate_windows_shortcut(path: &Path, runner: &str) -> Result<(), String> {
    if runner != "desktop"
        || !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("desktop"))
    {
        return Err("Only existing .desktop shortcuts are supported. Direct .exe launching is unavailable. Add a working desktop shortcut instead; your installed game files are unchanged.".into());
    }
    Ok(())
}
fn windows_command(game: &WindowsGame) -> Result<Command, String> {
    let path = Path::new(&game.target);
    // Validate saved entries too, including registrations made by older versions.
    validate_windows_shortcut(path, &game.runner)?;
    if !path.is_file() {
        return Err("Windows desktop shortcut is missing".into());
    }
    let mut command = launch_command("gio");
    command.arg("launch").arg(path);
    Ok(command)
}
fn launch_windows_impl(uid: String) -> Result<launches::Info, String> {
    let settings = load_settings()?;
    let game = settings
        .windows
        .iter()
        .find(|g| g.uid == uid)
        .ok_or("Windows game not registered")?;
    let info=launches::start(windows_command(game)?, "windows")?;
    let record=Game{uid:game.uid.clone(),title:game.title.clone(),path:game.target.clone(),kind:"windows".into(),app_id:None};
    let _=library_tools::record(&record);
    Ok(info)
}
#[cfg(feature = "desktop")]
#[tauri::command]
async fn launch_windows_game(uid: String) -> Result<launches::Info, String> {
    tauri::async_runtime::spawn_blocking(move || launch_windows_impl(uid))
        .await
        .map_err(|e| e.to_string())?
}

// Console artwork is stored in managed Covers folders, separately from game files.
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
    let roots = libraries::roots(kind)?;
    let game_path = Path::new(uid)
        .canonicalize()
        .map_err(|_| "Game file missing")?;
    if libraries::containing_root(&game_path, &roots).is_none() || !game_path.is_file() {
        return Err("Game is outside this console's library".into());
    }
    if !supported_console_file(s, &game_path) {
        return Err("Unsupported game format".into());
    }
    if libraries::is_playlist(kind, &game_path) {
        libraries::playlist_members(kind, &game_path, &roots)?;
    }
    let title = console_game_title(kind, &game_path, &roots);
    let uid = actionable_path(&game_path)?;
    Ok(Game {
        uid: uid.clone(),
        title,
        path: uid,
        kind: kind.into(),
        app_id: None,
    })
}

fn stored_cover_path(kind: &str, uid: &str, extension: &str) -> Result<PathBuf, String> {
    if spec(kind).is_ok() {
        return Ok(console_covers_dir(kind)?
            .join(format!("{}.{}", cover_key(kind, uid), extension)));
    }
    legacy_stored_cover_path(kind, uid, extension)
}

fn legacy_stored_cover_path(kind: &str, uid: &str, extension: &str) -> Result<PathBuf, String> {
    Ok(artwork_root()?
        .join(kind)
        .join(format!("{}.{}", cover_key(kind, uid), extension)))
}

fn console_cover_candidates(kind: &str, uid: &str) -> Result<Vec<PathBuf>, String> {
    let covers = console_covers_dir(kind)?;
    let mut candidates = Vec::new();
    for extension in ["png", "jpg", "webp"] {
        candidates.push(stored_cover_path(kind, uid, extension)?);
    }
    if let Some(stem) = Path::new(uid).file_stem().and_then(|s| s.to_str()) {
        for extension in ["png", "jpg", "jpeg", "webp", "PNG", "JPG", "JPEG", "WEBP"] {
            candidates.push(covers.join(format!("{stem}.{extension}")));
        }
    }
    Ok(candidates)
}

fn candidate_cover_paths(game: &Game) -> Vec<PathBuf> {
    if game.kind == "steam" {
        let id = game.app_id.as_deref().unwrap_or("");
        let Ok(home) = home() else {
            return Vec::new();
        };
        let mut roots = vec![
            home.join(".steam/steam/appcache/librarycache"),
            home.join(".local/share/Steam/appcache/librarycache"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/appcache/librarycache"),
            home.join("snap/steam/common/.local/share/Steam/appcache/librarycache"),
            home.join("snap/steam/common/.steam/steam/appcache/librarycache"),
        ];
        if let Ok(settings) = load_settings() {
            for library in steam_locations_in(&home, &settings.steam_libraries) {
                if let Some(parent) = library.path.parent() {
                    roots.push(parent.join("appcache/librarycache"));
                }
            }
        }
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
    let unique_game_folder = local_artwork::unique_game_folder(game);
    if unique_game_folder {
        for extension in ["png", "jpg", "webp"] {
            candidates.push(parent.join(format!("cover.{extension}")));
        }
    }
    candidates
}

fn read_local_cover_bytes(kind: &str, uid: &str, game: &Game) -> Result<Option<Vec<u8>>, String> {
    let mut candidates = Vec::new();
    if spec(kind).is_ok() {
        candidates.extend(console_cover_candidates(kind, uid)?);
    }
    // Keep pre-upgrade imports readable without moving or deleting them.
    for extension in ["png", "jpg", "webp"] {
        candidates.push(legacy_stored_cover_path(kind, uid, extension)?);
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
    Ok(local_artwork::ps5_icon(game))
}

fn read_cover_bytes(kind: &str, uid: &str, game: &Game) -> Result<Option<Vec<u8>>, String> {
    if let Some(bytes) = read_local_cover_bytes(kind, uid, game)? {
        return Ok(Some(bytes));
    }
    let settings = load_settings()?;
    let cache = artwork_root()?.join("online");
    let mut downloaded = Vec::new();
    if spec(kind).is_ok() {
        downloaded.extend(matrix_cover_core::cached_cover_in(
            &cache, &console_covers_dir(kind)?, kind, Path::new(&game.path), &settings.artwork.style,
        ));
    }
    downloaded.extend(matrix_cover_core::cached_cover(
        &cache, kind, Path::new(&game.path), &settings.artwork.style,
    ));
    for path in downloaded {
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
fn save_artwork_settings(
    enabled: bool,
    style: String,
    title_enabled: Option<bool>,
) -> Result<ArtworkSettings, String> {
    if !["default", "3d"].contains(&style.as_str()) {
        return Err("Unsupported cover style".into());
    }
    update_settings(|settings| {
        settings.artwork = ArtworkSettings {
            enabled,
            style,
            title_enabled: title_enabled.unwrap_or(settings.artwork.title_enabled),
        };
        Ok(settings.artwork.clone())
    })
}

fn download_cover_impl(
    kind: String,
    uid: String,
    serial: Option<String>,
) -> Result<matrix_cover_core::Outcome, String> {
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
    if !settings.steamgriddb_api_key.is_empty() {
        let override_title=settings.library_preferences.metadata.get(&format!("{kind}|{uid}")).map(|m|m.cover_title.as_str()).filter(|t|!t.is_empty()).unwrap_or(&game.title);
        return steamgrid::automatic(&kind, &uid, override_title, &settings.steamgriddb_api_key);
    }
    if !matrix_cover_core::supports(&kind) {
        return Err("Add a SteamGridDB API key in Settings or choose a local cover".into());
    }
    if matrix_cover_core::title::system(&kind).is_some()
        && !settings.artwork.title_enabled
        && serial.is_none()
    {
        return Ok(matrix_cover_core::Outcome {
            status: "disabled".into(),
            message: "Enable title-based covers in Settings, or confirm an exact title here".into(),
            ..Default::default()
        });
    }
    let serial=serial.or_else(|| if matrix_cover_core::title::system(&kind).is_some()&&settings.artwork.title_enabled{settings.library_preferences.metadata.get(&format!("{kind}|{uid}")).map(|m|m.cover_title.clone()).filter(|t|!t.is_empty())}else{None});
    matrix_cover_core::fetch_into(
        &artwork_root()?.join("online"),
        &console_covers_dir(&kind)?,
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
    launch_command("xdg-open")
        .arg("https://www.thecoverproject.net/")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot open TheCoverProject: {e}"))
}

fn get_cover(kind: String, uid: String) -> Result<Option<String>, String> {
    let game = match validate_game(&kind, &uid){Ok(game)=>game,Err(error)=>return library_features::offline_cover(&kind,&uid).map_err(|_|error)};
    let Some(bytes) = read_cover_bytes(&kind, &uid, &game)? else {
        return Ok(None);
    };
    let Some((_, mime)) = cover_format(&bytes) else {
        return Ok(None);
    };
    Ok(Some(format!("data:{mime};base64,{}", BASE64.encode(bytes))))
}

fn save_cover(kind: String, uid: String, bytes: Vec<u8>) -> Result<(), String> {
    let _ = validate_game(&kind, &uid)?;
    if bytes.is_empty() || bytes.len() > MAX_COVER_BYTES {
        return Err("Cover must be 4 MB or smaller".into());
    }
    let (extension, _) =
        cover_format(&bytes).ok_or("Only PNG, JPEG, and WebP images are supported")?;
    let target = stored_cover_path(&kind, &uid, extension)?;
    artwork::save(&target, &bytes)
}

// WebKit/GStreamer/Mesa cache initialization can stall before any page loads.
// Keep Matrix's runtime caches separate, without moving or deleting existing caches.
static ORIGINAL_CACHE_HOME: std::sync::OnceLock<Option<std::ffi::OsString>> =
    std::sync::OnceLock::new();
fn isolate_runtime_cache() {
    #[cfg(target_os = "linux")]
    {
        let original = std::env::var_os("XDG_CACHE_HOME");
        if let Some(cache) = dirs::cache_dir() {
            let runtime = cache.join("matrix").join("runtime-v1");
            match fs::create_dir_all(&runtime) {
                Ok(()) => {
                    let _ = ORIGINAL_CACHE_HOME.set(original);
                    // Called once from main(), before GTK/WebKit or background threads start.
                    std::env::set_var("XDG_CACHE_HOME", &runtime);
                    startup_log(&format!("Isolated runtime cache: {}", runtime.display()));
                }
                Err(error) => startup_log(&format!("Could not isolate runtime cache: {error}")),
            }
        }
    }
}

fn launch_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = appimage_environment::command(program);
    // The cache override belongs to Matrix, not to games or external applications.
    if let Some(original) = ORIGINAL_CACHE_HOME.get() {
        match original {
            Some(value) => {
                command.env("XDG_CACHE_HOME", value);
            }
            None => {
                command.env_remove("XDG_CACHE_HOME");
            }
        }
    }
    command
}

static STARTUP_LOG: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
fn startup_log(message: &str) {
    use std::io::Write;
    eprintln!("[Matrix Play] {message}");
    if let Some(path) = STARTUP_LOG.get() {
        if fs::metadata(path).is_ok_and(|m| m.len() > 65536) {
            return;
        }
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{message}");
        }
    }
}
fn initialize_startup_log() {
    if let Some(data) = dirs::data_local_dir() {
        let path = data.join("matrix").join("startup.log");
        if fs::create_dir_all(path.parent().unwrap()).is_ok() {
            let _ = fs::write(&path, "");
            let _ = STARTUP_LOG.set(path);
        }
    }
    startup_log(&format!(
        "Starting Matrix Play {}",
        env!("CARGO_PKG_VERSION")
    ));
    startup_log(&format!("Graphics: {:?}", graphics_diagnostics()));
}
#[cfg_attr(feature = "desktop", tauri::command)]
fn report_startup(message: String) {
    startup_log(&message.chars().take(4000).collect::<String>());
}

#[cfg_attr(feature = "desktop", tauri::command)]
fn graphics_diagnostics() -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    values.insert("version".into(), env!("CARGO_PKG_VERSION").into());
    for key in [
        "XDG_SESSION_TYPE",
        "GDK_BACKEND",
        "WAYLAND_DISPLAY",
        "WEBKIT_DISABLE_DMABUF_RENDERER",
        "WEBKIT_DISABLE_COMPOSITING_MODE",
        "XDG_CACHE_HOME",
    ] {
        if let Ok(value) = std::env::var(key) {
            values.insert(key.into(), value);
        }
    }
    values
}

#[cfg(feature = "desktop")]
fn main() {
    static SYSTEM_GUARD: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();
    let installed =
        std::env::current_exe().is_ok_and(|p| p == Path::new("/usr/bin/matrix-frontend"));
    match update_lifecycle::system_startup_guard(installed) {
        Ok(Some(guard)) => {
            let _ = SYSTEM_GUARD.set(guard);
        }
        Ok(None) => (),
        Err(error) => {
            eprintln!("Matrix Play: {error}");
            return;
        }
    }
    // The log reader is a separate process that must outlive the GUI. Dispatch
    // before initializing WebKit, application state or startup/cache files.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--matrix-launch-log-sink")
    {
        std::process::exit(if launch_log::run().is_ok() { 0 } else { 1 });
    }
    // The private lease lives until actual process exit, not merely window close.
    static INSTANCE: std::sync::OnceLock<update_lifecycle::Instance> = std::sync::OnceLock::new();
    let registered =
        update_lifecycle::directory().and_then(|root| update_lifecycle::Instance::register(&root));
    match registered {
        Ok(instance) => {
            std::env::set_var("MATRIX_INSTANCE_TOKEN", instance.token());
            let _ = INSTANCE.set(instance);
        }
        Err(error) => {
            eprintln!("Matrix Play: {error}");
            return;
        }
    }
    initialize_startup_log();
    isolate_runtime_cache();
    tauri::Builder::default()
        .manage(updates::Service::default())
        .plugin(tauri_plugin_dialog::init())
        .on_page_load(|_, page| {
            startup_log(&format!("Page {:?}: {}", page.event(), page.url()));
        })
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                if INSTANCE
                    .get()
                    .is_some_and(|instance| instance.stop_requested())
                {
                    handle.exit(0);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            });
            #[cfg(target_os = "linux")]
            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    window.with_webview(|webview| {
                        use webkit2gtk::WebViewExt;
                        let view = webview.inner();
                        view.connect_load_failed(|_, stage, url, error| {
                            startup_log(&format!(
                                "WebKit load failed at {stage:?}, {url}: {error}"
                            ));
                            false
                        });
                        view.connect_web_process_terminated(|_, reason| {
                            startup_log(&format!("WebKit process terminated: {reason:?}"));
                        });
                    })?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_commands::bootstrap,
            updates::update_status,
            updates::check_updates,
            updates::download_update,
            updates::install_update,
            updates::cancel_update,
            updates::restart_updated,
            updates::open_update_releases,
            graphics_diagnostics,
            report_startup,
            desktop_commands::scan_console,
            libraries::list_library_folders,
            libraries::add_library_folder,
            libraries::remove_library_folder,
            libraries::browse_library_folder,
            desktop_commands::scan_steam,
            steam_libraries::list_steam_libraries,
            steam_libraries::add_steam_library,
            steam_libraries::remove_steam_library,
            desktop_commands::save_emulator,
            discover_emulators,
            desktop_commands::list_flatpaks,
            emulators::browse_emulator,
            favorites::set_favorite,
            favorites::import_favorites,
            add_windows_game,
            list_windows_games,
            remove_windows_game,
            open_console_folder,
            launch_console_game,
            open_emulator,
            launches::get_launch_status,
            launch_steam_game,
            launch_windows_game,
            desktop_commands::get_cover,
            desktop_commands::save_cover,
            download_cover,
            save_artwork_settings,
            open_cover_project,
            library_features::set_library_preference,
            library_features::library_snapshot,
            save_data::save_data_dialog,
            save_data::apply_save_restore,
            library_tools::save_game_options,
            library_tools::game_options,
            library_tools::backup_library,
            steamgrid::steamgrid_save_key,
            steamgrid::steamgrid_search,
            steamgrid::steamgrid_grids,
            steamgrid::steamgrid_save,
            steamgrid::steamgrid_open_key,
        ])
        .run(tauri::generate_context!())
        .expect("Error running Matrix Play frontend");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn console_paths_never_alias_invalid_bytes_to_replacement_characters() {
        use std::os::unix::ffi::OsStringExt;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut originals = Vec::new();
        for byte in [0xff, 0xfe] {
            let path = root.join(std::ffi::OsString::from_vec(
                [b"Title-".as_slice(), &[byte], b".iso"].concat(),
            ));
            fs::write(&path, [byte]).unwrap();
            originals.push((path, vec![byte]));
        }
        let invalid_parent = root.join(std::ffi::OsString::from_vec(b"folder-\xff".to_vec()));
        fs::create_dir(&invalid_parent).unwrap();
        let nested = invalid_parent.join("Valid.iso");
        fs::write(&nested, b"nested").unwrap();
        originals.push((nested.clone(), b"nested".to_vec()));
        let lookalike = root.join("Title-\u{fffd}.iso");
        fs::write(&lookalike, b"lookalike").unwrap();
        let rows = scan_console_in("ps2", &[root.clone()]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uid, lookalike.to_str().unwrap());
        assert_eq!(rows[0].path, rows[0].uid);
        assert_eq!(fs::read(&rows[0].path).unwrap(), b"lookalike");
        let profile = EmulatorProfile::default();
        assert_eq!(console_launch_args(&profile, "ps2", &lookalike).unwrap(),
            [lookalike.to_str().unwrap()]);
        for (path, bytes) in originals {
            assert!(actionable_path(&path).is_err());
            assert!(console_launch_args(&profile, "ps2", &path).is_err());
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        assert!(scan_console_in("ps2", &[invalid_parent]).unwrap().is_empty());
    }

    #[test]
    fn windows_shortcut_rejects_non_utf8_canonical_symlink_target() {
        use std::os::unix::ffi::OsStringExt;
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join(std::ffi::OsString::from_vec(b"Game-\xff.desktop".to_vec()));
        fs::write(&target, b"original shortcut").unwrap();
        let alias = temp.path().join("alias.desktop");
        std::os::unix::fs::symlink(&target, &alias).unwrap();
        let lookalike = temp.path().join("Game-\u{fffd}.desktop");
        fs::write(&lookalike, b"different shortcut").unwrap();
        let error = add_windows_game("Game".into(), alias.to_str().unwrap().into(), "desktop".into())
            .err().expect("must reject before changing settings");
        assert!(error.contains("non-UTF-8"));
        assert_eq!(fs::read(target).unwrap(), b"original shortcut");
        assert_eq!(fs::read(lookalike).unwrap(), b"different shortcut");
    }

    #[test]
    fn all_console_paths_are_unique_and_valid() {
        let mut ids = HashSet::new();
        let mut directories = HashSet::new();
        for s in SPECS {
            assert!(ids.insert(s.id));
            assert!(directories.insert(s.dir));
            assert!(s.games.ends_with(" Games"));
        }
        assert_eq!(SPECS.len(), 24);
    }
    #[test]
    fn steam_preserves_launcher_for_primary_and_external_libraries() {
        let root = tempfile::tempdir().unwrap();
        let native = root.path().join(".local/share/Steam/steamapps");
        let flatpak = root
            .path()
            .join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps");
        let external = root.path().join("External Steam Library/steamapps");
        for (directory, id) in [(&native, "11"), (&flatpak, "22"), (&external, "33")] {
            fs::create_dir_all(directory).unwrap();
            fs::write(
                directory.join(format!("appmanifest_{id}.acf")),
                format!("\"appid\" \"{id}\"\n\"name\" \"Game {id}\"\n"),
            )
            .unwrap();
        }
        fs::write(
            flatpak.join("libraryfolders.vdf"),
            format!("\"path\" \"{}\"\n", external.parent().unwrap().display()),
        )
        .unwrap();
        let games = scan_steam_in(root.path(), &[]);
        assert_eq!(games.len(), 3);
        for game in games {
            let flat = game.game.uid != "11";
            assert_eq!(
                game.launcher,
                if flat {
                    SteamLauncher::Flatpak
                } else {
                    SteamLauncher::Native
                }
            );
            let command = steam_command(&game.game.uid, game.launcher);
            assert_eq!(
                command.get_program(),
                if flat { "flatpak" } else { "steam" }
            );
            let args: Vec<_> = command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            let mut expected: Vec<String> = if flat {
                vec!["run".into(), "com.valvesoftware.Steam".into()]
            } else {
                vec![]
            };
            expected.extend(["-applaunch".into(), game.game.uid]);
            assert_eq!(args, expected);
        }
        // Legacy native aliases into the Flatpak root must not lose provenance.
        fs::create_dir_all(root.path().join(".steam")).unwrap();
        std::os::unix::fs::symlink(flatpak.parent().unwrap(), root.path().join(".steam/root"))
            .unwrap();
        let games = scan_steam_in(root.path(), &[]);
        assert_eq!(games.len(), 3);
        assert_eq!(
            games.iter().find(|g| g.game.uid == "22").unwrap().launcher,
            SteamLauncher::Flatpak
        );
    }
    #[test]
    fn ps3_titles_support_disc_and_digital_layouts() {
        assert_eq!(
            ps3_title(Path::new("/Games/PS3 Games/Digital_Title/USRDIR/EBOOT.BIN")),
            "Digital Title"
        );
        assert_eq!(
            ps3_title(Path::new(
                "/Games/PS3 Games/Disc_Title/PS3_GAME/USRDIR/EBOOT.BIN"
            )),
            "Disc Title"
        );
        assert_eq!(
            ps3_title(Path::new(
                "/Games/PS3 Games/Disc_Title/ps3_game/usrdir/eboot.bin"
            )),
            "Disc Title"
        );
    }
    #[test]
    fn windows_shortcuts_reject_direct_executables_and_unknown_runners() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("Game with spaces.exe");
        fs::write(&target, b"synthetic game fixture").unwrap();
        for runner in ["desktop", "legacy-runtime", ""] {
            let error = add_windows_game(
                "Unsupported entry".into(),
                target.to_string_lossy().into(),
                runner.into(),
            )
            .err()
            .expect("direct executable registration must fail");
            assert!(error.contains("Direct .exe launching is unavailable"));
            let game = WindowsGame {
                uid: "legacy".into(),
                title: "Legacy game".into(),
                target: target.to_string_lossy().into(),
                runner: runner.into(),
            };
            assert!(windows_command(&game).is_err());
        }
        let disguised = root.path().join("Disguised.desktop");
        std::os::unix::fs::symlink(&target, &disguised).unwrap();
        assert!(add_windows_game(
            "Disguised executable".into(),
            disguised.to_string_lossy().into(),
            "desktop".into()
        )
        .is_err());
        let desktop = root.path().join("Game.DESKTOP");
        fs::write(&desktop, b"[Desktop Entry]\nType=Application\nExec=true\n").unwrap();
        let game = WindowsGame {
            uid: "shortcut".into(),
            title: "Shortcut game".into(),
            target: desktop.to_string_lossy().into(),
            runner: "desktop".into(),
        };
        let command = windows_command(&game).unwrap();
        assert_eq!(command.get_program(), "gio");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [std::ffi::OsStr::new("launch"), desktop.as_os_str()]
        );
        assert!(validate_windows_shortcut(&desktop, "legacy-runtime").is_err());
    }
    #[test]
    fn legacy_windows_entries_are_preserved_but_not_launchable() {
        let value = serde_json::json!({
            "uid": "/games/Legacy.exe", "title": "Legacy", "target": "/games/Legacy.exe",
            "runner": "legacy-runtime"
        });
        let game: WindowsGame = serde_json::from_value(value.clone()).unwrap();
        assert!(windows_command(&game)
            .unwrap_err()
            .contains("Direct .exe launching is unavailable"));
        assert_eq!(serde_json::to_value(game).unwrap(), value);
    }
    #[test]
    fn external_launch_fixture() {
        let Some(root) = std::env::var_os("MATRIX_EXTERNAL_FIXTURE") else {
            return;
        };
        use std::os::unix::fs::PermissionsExt;
        let root = PathBuf::from(root);
        fs::create_dir_all(root.join("bin")).unwrap();
        {
            let (program, ext, runner) = ("gio", "desktop", "desktop");
            let script = root.join("bin").join(program);
            fs::write(&script, format!("#!/bin/sh\nprintf 'early {program} diagnostic\\n' >&2\nsleep 0.7\nprintf 'late {program} diagnostic\\n' >&2\nexit 17\n")).unwrap();
            fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
            let target = root.join(format!("Game with spaces.{ext}"));
            fs::write(&target, b"synthetic fixture, not executable game data").unwrap();
            let game = add_windows_game(
                "Fixture".into(),
                target.to_string_lossy().into(),
                runner.into(),
            )
            .unwrap();
            let command = windows_command(&game).unwrap();
            let args: Vec<_> = command.get_args().collect();
            assert_eq!(args.last().unwrap(), &target.as_os_str());
            assert_eq!(args.len(), 2);
            assert_eq!(args[0], "launch");
            assert_eq!(command.get_program(), "gio");
            let info = launch_windows_impl(game.uid).unwrap();
            assert_eq!(info.phase, "started");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                let info = launches::get_launch_status("windows".into())
                    .unwrap()
                    .unwrap();
                if info.phase == "error" {
                    assert!(info.message.contains("17"));
                    assert!(info.message.contains("Windows launcher"));
                    assert!(info.log.contains(&format!("early {program} diagnostic")));
                    assert!(info.log.contains(&format!("late {program} diagnostic")));
                    assert!(Path::new(&info.log_path).is_file());
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "launcher failure was not reported"
                );
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        assert!(launches::get_launch_status("../escape".into()).is_err());
    }
    #[test]
    fn desktop_shortcut_failures_are_captured_after_spawn() {
        let root = tempfile::tempdir().unwrap();
        let (_, module) = module_path!().split_once("::").unwrap_or(("", "tests"));
        let fixture = format!("{module}::external_launch_fixture");
        let path = std::env::join_paths(std::iter::once(root.path().join("bin")).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &fixture, "--nocapture"])
            .env("MATRIX_EXTERNAL_FIXTURE", root.path())
            .env("HOME", root.path())
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("XDG_DATA_HOME", root.path().join("data"))
            .env("PATH", path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        assert!(root.path().join("data/matrix/launches").is_dir());
    }
    #[test]
    fn custom_nextgen_launch_arguments_are_literal_and_console_scoped() {
        let game = Path::new("/games/A title; $(never run)/eboot.bin");
        let profile = EmulatorProfile {
            emulator_path: "/apps/custom emulator".into(),
            launch_args: vec![
                "--game".into(),
                "{gameDir}".into(),
                "--literal=$(no)".into(),
            ],
            ..Default::default()
        };
        for id in ["ps5", "switch2"] {
            assert_eq!(
                console_launch_args(&profile, id, game).unwrap(),
                ["--game", "/games/A title; $(never run)", "--literal=$(no)"]
            );
            assert!(immersive::setup_args(&profile, id).is_empty());
        }
        // The new token cannot reinterpret an older console's saved argv.
        assert_eq!(
            console_launch_args(&profile, "ps4", game).unwrap(),
            [
                "--game",
                "{gameDir}",
                "--literal=$(no)",
                game.to_str().unwrap()
            ]
        );
        let profile = EmulatorProfile {
            launch_args: vec!["--file={game}".into(), "{game}".into()],
            ..profile
        };
        assert_eq!(
            console_launch_args(&profile, "ps5", game).unwrap(),
            ["--file={game}", game.to_str().unwrap()]
        );
    }

    #[test]
    fn ps5_custom_scan_keeps_collection_files_and_adds_exact_boot_targets() {
        let root = tempfile::tempdir().unwrap();
        let title = root.path().join("An Extracted Title");
        fs::create_dir_all(&title).unwrap();
        for name in ["eboot.bin", "unrelated.bin"] {
            fs::write(title.join(name), b"synthetic").unwrap();
        }
        for name in ["Archive.zar", "Homebrew.elf", "Legacy.pkg", "Legacy.iso"] {
            fs::write(root.path().join(name), b"synthetic").unwrap();
        }
        let games = scan_console_in("ps5", &[root.path().into()]).unwrap();
        assert_eq!(games.len(), 5);
        assert!(games.iter().any(|g| g.title == "An Extracted Title"));
        assert!(!games.iter().any(|g| g.path.ends_with("unrelated.bin")));
        assert!(!supported_console_file(
            spec("ps4").unwrap(),
            &title.join("eboot.bin")
        ));
        assert!(scan_console_in("switch2", &[root.path().into()])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn switch2_nup_scan_is_case_insensitive_and_console_scoped() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let games = root.path().join("Switch 2 Games");
        let nested = games.join("Nested Title");
        fs::create_dir_all(&nested).unwrap();
        let payload = b"synthetic collection entry, not a verified game format";
        for name in [
            "Lower.nup",
            "Upper.NUP",
            "Mixed.NuP",
            "Existing.nsp",
            "Existing.xci",
        ] {
            fs::write(nested.join(name), payload).unwrap();
        }
        fs::write(nested.join("Not a package.nup.txt"), payload).unwrap();
        fs::write(nested.join(".Hidden.nup"), payload).unwrap();
        fs::create_dir(games.join("Directory.nup")).unwrap();
        let outside = root.path().join("Outside.nup");
        fs::write(&outside, payload).unwrap();
        symlink(&outside, games.join("Linked.nup")).unwrap();
        let rows = scan_console_in("switch2", &[games.clone(), nested.clone()]).unwrap();
        assert_eq!(rows.len(), 5, "nested roots must not duplicate entries");
        for row in &rows {
            assert_eq!(row.kind, "switch2");
            assert!(Path::new(&row.path).starts_with(&nested));
            assert_eq!(row.uid, row.path);
            assert_eq!(fs::read(&row.path).unwrap(), payload);
        }
        assert_eq!(scan_console_in("switch", &[games]).unwrap().len(), 2);
        for id in SPECS.iter().map(|s| s.id).filter(|id| *id != "switch2") {
            assert!(!supported_console_file(
                spec(id).unwrap(),
                &nested.join("Upper.NUP")
            ));
        }
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
    fn artwork_migration_requires_opt_in_for_title_providers() {
        let previous: ArtworkSettings =
            serde_json::from_str(r#"{"enabled":true,"style":"3d"}"#).unwrap();
        assert!(previous.enabled);
        assert!(!previous.title_enabled);
        assert_eq!(previous.style, "3d");
        let explicit: ArtworkSettings =
            serde_json::from_str(r#"{"enabled":true,"style":"default","titleEnabled":true}"#)
                .unwrap();
        assert!(explicit.title_enabled);
        assert!(serde_json::to_string(&explicit)
            .unwrap()
            .contains("\"titleEnabled\":true"));
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
        assert_eq!(SPECS.len(), 24);
        for s in SPECS {
            let covers = console_covers_dir(s.id).unwrap();
            let games = games_dir(s.id).unwrap();
            assert!(covers.is_dir());
            assert_eq!(covers.parent(), games.parent());
        }
        let root = games_dir("ps2").unwrap();
        let path = root.join("SLUS-20312.chd");
        fs::write(&path, b"synthetic fixture").unwrap();
        let uid = path.canonicalize().unwrap().to_string_lossy().into_owned();
        assert_eq!(scan_console_impl("ps2").unwrap().len(), 1);
        assert!(scan_console_impl("ps1").unwrap().is_empty());
        let ps3 = games_dir("ps3").unwrap();
        for layout in ["Digital_Title/USRDIR", "Disc_Title/PS3_GAME/USRDIR"] {
            fs::create_dir_all(ps3.join(layout)).unwrap();
            fs::write(ps3.join(layout).join("EBOOT.BIN"), b"synthetic").unwrap();
        }
        let ps3_games = scan_console_impl("ps3").unwrap();
        assert_eq!(ps3_games.len(), 2);
        assert_eq!(
            ps3_games
                .iter()
                .map(|g| g.title.as_str())
                .collect::<Vec<_>>(),
            ["Digital Title", "Disc Title"]
        );
        for game in ps3_games {
            assert_eq!(validate_game("ps3", &game.uid).unwrap().title, game.title);
        }
        let ps4 = games_dir("ps4").unwrap();
        fs::create_dir_all(ps4.join("Example PS4/sce_sys")).unwrap();
        fs::write(ps4.join("Example PS4/eboot.bin"), b"synthetic").unwrap();
        fs::write(ps4.join("Example PS4/sce_sys/param.sfo"), b"synthetic").unwrap();
        fs::write(ps4.join("install.pkg"), b"not bootable").unwrap();
        fs::write(ps4.join("unrelated.bin"), b"not bootable").unwrap();
        let ps4games = scan_console_impl("ps4").unwrap();
        assert_eq!(ps4games.len(), 1);
        assert_eq!(ps4games[0].title, "Example PS4");
        let ps4_uid = ps4games[0].uid.clone();
        assert_eq!(validate_game("ps4", &ps4_uid).unwrap().title, "Example PS4");
        assert_eq!(
            favorites::set_favorite("ps4".into(), ps4_uid.clone(), true).unwrap(),
            [ps4_uid.clone()]
        );
        assert!(get_cover("ps4".into(), ps4_uid.clone()).unwrap().is_none());
        let ps4_cover = include_bytes!("../../tests/fixtures/demo-cover.png").to_vec();
        save_cover("ps4".into(), ps4_uid.clone(), ps4_cover).unwrap();
        assert!(get_cover("ps4".into(), ps4_uid.clone())
            .unwrap()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(validate_game("ps3", &ps4_uid).is_err());
        assert!(validate_game("ps4", ps4.join("unrelated.bin").to_str().unwrap()).is_err());
        assert!(validate_game("ps4", ps4.join("install.pkg").to_str().unwrap()).is_err());
        fs::remove_file(ps4.join("Example PS4/sce_sys/param.sfo")).unwrap();
        assert!(validate_game("ps4", &ps4_uid).is_err());
        assert!(scan_console_impl("ps4").unwrap().is_empty());
        fs::write(ps4.join("Example PS4/sce_sys/param.sfo"), b"synthetic").unwrap();
        fs::write(games_dir("vita").unwrap().join("Example.vpk"), b"synthetic").unwrap();
        assert_eq!(scan_console_impl("vita").unwrap().len(), 1);
        for (id, filename) in [("ps5", "Collection.pkg"), ("switch2", "Collection.xci")] {
            fs::write(games_dir(id).unwrap().join(filename), b"stored file only").unwrap();
            let stored = scan_console_impl(id).unwrap().remove(0);
            assert!(launch_console_impl(id.into(), stored.uid.clone())
                .err()
                .unwrap()
                .contains("Configure an emulator"));
            assert_eq!(
                favorites::set_favorite(id.into(), stored.uid, true)
                    .unwrap()
                    .len(),
                1
            );
            assert!(open_emulator_impl(id.into())
                .err()
                .unwrap()
                .contains("Set up an emulator"));
            save_cover(
                id.into(),
                stored.path.clone(),
                include_bytes!("../../tests/fixtures/demo-cover.png").to_vec(),
            )
            .unwrap();
            // Registration never runs the selected file. Arbitrary Linux
            // filenames work, without borrowing another console's engine.
            let launcher = games_dir(id).unwrap().parent().unwrap().join("My Emulator");
            fs::write(&launcher, b"#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
            save_emulator(
                profile_saves::begin().unwrap(),
                id.into(),
                "My custom emulator".into(),
                launcher.to_string_lossy().into(),
                vec!["--load".into(), "{game}".into()],
                Some("executable".into()),
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let settings = load_settings().unwrap();
            let profile = &settings.profiles[id];
            assert!(!profile.auto_detected);
            assert!(settings.favorites[id].contains(&stored.path));
            assert!(get_cover(id.into(), stored.path.clone()).unwrap().is_some());
            assert_eq!(fs::read(&stored.path).unwrap(), b"stored file only");
            assert_eq!(profile.emulator_path, launcher.to_string_lossy());
            assert!(discovery::identity(profile, id).is_none());
            let mut command =
                emulators::command(profile, id, Some(Path::new(&stored.path))).unwrap();
            command.args(console_launch_args(profile, id, Path::new(&stored.path)).unwrap());
            let output = command.output().unwrap();
            assert!(output.status.success());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!("--load\n{}\n", stored.path)
            );
            assert!(immersive::setup_args(profile, id).is_empty());
        }

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
        update_settings(|current| {
            *current = settings;
            Ok(())
        })
        .unwrap();
        let legacy = BTreeMap::from([
            ("ps2".into(), vec![uid.clone(), "demo-0".into()]),
            ("ps1".into(), vec![uid.clone()]),
        ]);
        let migrated = favorites::import_favorites(legacy.clone()).unwrap();
        assert_eq!(migrated["ps2"], vec![uid.clone()]);
        assert!(migrated["ps1"].is_empty());
        assert!(favorites::set_favorite("ps1".into(), uid.clone(), true).is_err());
        assert!(favorites::set_favorite("ps2".into(), uid.clone(), false)
            .unwrap()
            .is_empty());
        assert!(favorites::import_favorites(legacy).unwrap()["ps2"].is_empty());
        assert_eq!(
            favorites::set_favorite("ps2".into(), uid.clone(), true)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            favorites::set_favorite("ps2".into(), uid.clone(), true)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(bootstrap().unwrap().favorites["ps2"], vec![uid.clone()]);
        save_artwork_settings(false, "3d".into(), None).unwrap();
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
        assert!(save_artwork_settings(true, "bad-style".into(), None).is_err());
        let bytes = include_bytes!("../../tests/fixtures/demo-cover.png").to_vec();
        let game = validate_game("ps2", &uid).unwrap();
        let covers = console_covers_dir("ps2").unwrap();
        let named = covers.join("SLUS-20312.PNG");
        fs::write(&named, &bytes).unwrap();
        assert_eq!(read_local_cover_bytes("ps2", &uid, &game).unwrap(), Some(bytes.clone()));
        assert!(scan_console_impl("ps2").unwrap().iter().all(|g| !Path::new(&g.path).starts_with(&covers)));
        fs::remove_file(&named).unwrap();
        let legacy = legacy_stored_cover_path("ps2", &uid, "png").unwrap();
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, &bytes).unwrap();
        assert_eq!(read_local_cover_bytes("ps2", &uid, &game).unwrap(), Some(bytes.clone()));
        let mut replacement = bytes.clone();
        replacement.extend(b"replacement");
        save_cover("ps2".into(), uid.clone(), replacement.clone()).unwrap();
        let saved = stored_cover_path("ps2", &uid, "png").unwrap();
        assert_eq!(saved.parent(), Some(covers.as_path()));
        assert_eq!(read_local_cover_bytes("ps2", &uid, &game).unwrap(), Some(replacement));
        assert_eq!(fs::read(&legacy).unwrap(), bytes);
        fs::remove_file(saved).unwrap();
        fs::remove_file(legacy).unwrap();
        create_layout().unwrap();
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
        save_artwork_settings(true, "default".into(), None).unwrap();
        assert_eq!(
            download_cover_impl("ps2".into(), uid.clone(), None)
                .unwrap()
                .status,
            "local"
        );
        assert!(get_cover("ps2".into(), uid).unwrap().is_some());
        // Every newly supported catalog keeps explicit user artwork ahead of
        // network lookup, even when the caller confirms a different title.
        for (kind, extension) in [
            ("ps3", "iso"),
            ("ps4", "elf"),
            ("psp", "chd"),
            ("vita", "vpk"),
            ("n64", "z64"),
            ("dreamcast", "chd"),
            ("wiiu", "wua"),
            ("gb", "gb"),
            ("gb", "gbc"),
            ("gba", "gba"),
            ("nds", "nds"),
            ("3ds", "3ds"),
        ] {
            let file = games_dir(kind)
                .unwrap()
                .join(format!("Keep cover (USA).{extension}"));
            fs::write(&file, b"unchanged synthetic game").unwrap();
            let uid = file.canonicalize().unwrap().to_string_lossy().into_owned();
            assert_eq!(
                download_cover_impl(kind.into(), uid.clone(), None)
                    .unwrap()
                    .status,
                "disabled"
            );
            let png = include_bytes!("../../tests/fixtures/demo-cover.png").to_vec();
            save_cover(kind.into(), uid.clone(), png.clone()).unwrap();
            save_artwork_settings(true, "3d".into(), None).unwrap();
            for confirmed in [None, Some("Different Title (USA)".into())] {
                assert_eq!(
                    download_cover_impl(kind.into(), uid.clone(), confirmed)
                        .unwrap()
                        .status,
                    "local"
                );
            }
            let game = validate_game(kind, &uid).unwrap();
            assert_eq!(read_cover_bytes(kind, &uid, &game).unwrap(), Some(png));
            assert_eq!(fs::read(&file).unwrap(), b"unchanged synthetic game");
        }
        // Switch identity is constrained, and manual images remain first priority.
        let switch_file = games_dir("switch")
            .unwrap()
            .join("Game [0100000000010000].nsp");
        fs::write(&switch_file, b"synthetic unchanged Switch file").unwrap();
        let switch_uid = switch_file
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            download_cover_impl("switch".into(), switch_uid.clone(), None)
                .unwrap()
                .status,
            "needs-serial"
        );
        let png = include_bytes!("../../tests/fixtures/demo-cover.png").to_vec();
        save_cover("switch".into(), switch_uid.clone(), png.clone()).unwrap();
        for confirmed in [None, Some("AAACA-US".into())] {
            assert_eq!(
                download_cover_impl("switch".into(), switch_uid.clone(), confirmed)
                    .unwrap()
                    .status,
                "local"
            );
        }
        let game = validate_game("switch", &switch_uid).unwrap();
        assert_eq!(
            read_cover_bytes("switch", &switch_uid, &game).unwrap(),
            Some(png)
        );
        assert_eq!(
            fs::read(&switch_file).unwrap(),
            b"synthetic unchanged Switch file"
        );
        assert!(
            save_artwork_settings(true, "3d".into(), Some(true))
                .unwrap()
                .title_enabled
        );
        assert!(
            save_artwork_settings(false, "default".into(), None)
                .unwrap()
                .title_enabled
        );
        assert!(
            !save_artwork_settings(true, "default".into(), Some(false))
                .unwrap()
                .title_enabled
        );
    }
}
