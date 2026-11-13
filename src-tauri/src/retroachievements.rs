//! Read-only RetroAchievements web dashboard. Credentials never reach the UI,
//! logs, image hosts or exports. Emulator achievement unlocking is separate.
use super::*;
use std::{
    io::Read,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Account {
    pub username: String,
    pub web_api_key: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    username: String,
    connected: bool,
}
pub(crate) fn status() -> Result<Status, String> {
    let a = load_settings()?.retroachievements;
    Ok(Status {
        connected: !a.username.is_empty() && !a.web_api_key.is_empty(),
        username: a.username,
    })
}
pub(crate) fn save_account(username: String, key: String) -> Result<Status, String> {
    let username = username.trim();
    let key = key.trim();
    if username.len() > 64 || username.chars().any(|c| c.is_control()) {
        return Err("Use a RetroAchievements username up to 64 bytes".into());
    }
    if !key.is_empty()
        && (!(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c)))
    {
        return Err("Paste a valid RetroAchievements Web API key".into());
    }
    update_settings(|s| {
        if username.is_empty() {
            if !key.is_empty() {
                return Err("Enter a username with the Web API key".into());
            }
            s.retroachievements = Account::default();
        } else {
            let key = if key.is_empty() && s.retroachievements.username == username {
                s.retroachievements.web_api_key.clone()
            } else {
                key.into()
            };
            if key.is_empty() {
                return Err("Add your RetroAchievements Web API key".into());
            }
            s.retroachievements = Account {
                username: username.into(),
                web_api_key: key,
            };
        }
        Ok(())
    })?;
    status()
}
#[cfg_attr(feature = "desktop", tauri::command)]
pub(crate) fn ra_open_key() -> Result<(), String> {
    launch_command("xdg-open")
        .arg("https://retroachievements.org/controlpanel.php")
        .spawn()
        .map(|_| ())
        .map_err(|_| "Cannot open RetroAchievements account settings".into())
}
fn account_id(a: &Account) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{}\0{}", a.username, a.web_api_key).as_bytes())
    )
}
struct Cached {
    at: Instant,
    value: serde_json::Value,
}
#[derive(Default)]
struct Cache {
    values: BTreeMap<String, Cached>,
    last: Option<Instant>,
    cooldown: Option<Instant>,
}
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
fn get(
    endpoint: &str,
    params: &[(&str, String)],
) -> Result<(serde_json::Value, bool, String), String> {
    let a = load_settings()?.retroachievements;
    if a.username.is_empty() || a.web_api_key.is_empty() {
        return Err("Connect RetroAchievements in Settings first".into());
    }
    let account = account_id(&a);
    let cache_key = format!("{account}|{endpoint}|{params:?}");
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(Cache::default()))
        .lock()
        .map_err(|_| "Achievements worker unavailable")?;
    if let Some(c) = cache.values.get(&cache_key) {
        if c.at.elapsed() < Duration::from_secs(300) {
            return Ok((c.value.clone(), true, String::new()));
        }
    }
    let fallback = cache.values.get(&cache_key).map(|c| c.value.clone());
    let request = (|| {
        if gaming::active() {
            return Err("Gaming mode pauses achievement refreshes".into());
        }
        if cache.cooldown.is_some_and(|t| Instant::now() < t) {
            return Err("RetroAchievements rate limit: try again later".into());
        }
        if let Some(last) = cache.last {
            let delay = Duration::from_millis(750).saturating_sub(last.elapsed());
            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
        }
        cache.last = Some(Instant::now());
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .user_agent("Matrix-Play/RetroAchievements-dashboard")
            .build()
            .map_err(|_| "Cannot start achievements client")?;
        let mut response = client
            .get(format!("https://retroachievements.org/API/{endpoint}"))
            .query(&[("y", a.web_api_key.as_str()), ("u", a.username.as_str())])
            .query(params)
            .send()
            .map_err(|_| "Cannot reach RetroAchievements. Check your connection.")?;
        let code = response.status().as_u16();
        if code == 429 {
            cache.cooldown = Some(Instant::now() + Duration::from_secs(60));
        }
        if !response.status().is_success() {
            return Err(match code {
                401 | 403 => "RetroAchievements rejected the account or Web API key".into(),
                429 => "RetroAchievements rate limit: try again later".into(),
                _ => format!("RetroAchievements returned HTTP {code}"),
            });
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut response)
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "RetroAchievements response interrupted")?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("RetroAchievements response exceeds 2 MB".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid RetroAchievements response")?;
        if value.get("Success").and_then(|v| v.as_bool()) == Some(false)
            || value
                .get("Error")
                .is_some_and(|e| !e.is_null() && e.as_str() != Some(""))
        {
            return Err("RetroAchievements could not load this account or game".into());
        }
        if account_id(&load_settings()?.retroachievements) != account {
            return Err("Account changed during refresh; try again".into());
        }
        Ok(value)
    })();
    if account_id(&load_settings()?.retroachievements) != account {
        return Err("Account changed during refresh; try again".into());
    }
    match request {
        Ok(value) => {
            if cache.values.len() >= 64 {
                if let Some(old) = cache
                    .values
                    .iter()
                    .min_by_key(|(_, v)| v.at)
                    .map(|(k, _)| k.clone())
                {
                    cache.values.remove(&old);
                }
            }
            cache.values.insert(
                cache_key,
                Cached {
                    at: Instant::now(),
                    value: value.clone(),
                },
            );
            Ok((value, false, String::new()))
        }
        Err(error) => {
            if let Some(value) = fallback {
                Ok((value, true, format!("{error}. Showing cached progress.")))
            } else {
                Err(error)
            }
        }
    }
}
fn number(v: &serde_json::Value, key: &str) -> u64 {
    v.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}
fn string(v: &serde_json::Value, key: &str, max: usize) -> String {
    v.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect()
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameProgress {
    game_id: u64,
    title: String,
    console_name: String,
    possible: u64,
    earned: u64,
    hardcore: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Page {
    total: u64,
    offset: u64,
    games: Vec<GameProgress>,
    cached: bool,
    warning: String,
}
fn parse_page(
    v: &serde_json::Value,
    offset: u64,
    cached: bool,
    warning: String,
) -> Result<Page, String> {
    let rows = v
        .get("Results")
        .and_then(|v| v.as_array())
        .ok_or("Invalid achievement progress response")?;
    let games = rows
        .iter()
        .take(100)
        .filter(|v| number(v, "GameID") > 0)
        .map(|v| GameProgress {
            game_id: number(v, "GameID"),
            title: string(v, "Title", 240),
            console_name: string(v, "ConsoleName", 120),
            possible: number(v, "MaxPossible"),
            earned: number(v, "NumAwarded"),
            hardcore: number(v, "NumAwardedHardcore"),
        })
        .collect();
    Ok(Page {
        total: number(v, "Total"),
        offset,
        games,
        cached,
        warning,
    })
}
pub(crate) fn progress(offset: u64) -> Result<Page, String> {
    if offset > 100000 {
        return Err("Achievement page offset is too large".into());
    }
    let (v, cached, warning) = get(
        "API_GetUserCompletionProgress.php",
        &[("c", "100".into()), ("o", offset.to_string())],
    )?;
    parse_page(&v, offset, cached, warning)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Achievement {
    title: String,
    description: String,
    points: u64,
    earned: bool,
    hardcore: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameDetail {
    game_id: u64,
    title: String,
    console_name: String,
    possible: u64,
    earned: u64,
    hardcore: u64,
    achievements: Vec<Achievement>,
    cached: bool,
    warning: String,
}
fn parse_game(v: &serde_json::Value, cached: bool, warning: String) -> Result<GameDetail, String> {
    let id = number(v, "ID");
    if id == 0 {
        return Err("RetroAchievements game was not found".into());
    }
    let rows = v
        .get("Achievements")
        .and_then(|a| a.as_object())
        .map(|m| m.values().collect::<Vec<_>>())
        .unwrap_or_default();
    let achievements = rows
        .into_iter()
        .take(5000)
        .map(|a| Achievement {
            title: string(a, "Title", 240),
            description: string(a, "Description", 2000),
            points: number(a, "Points"),
            earned: !string(a, "DateEarned", 64).is_empty()
                || !string(a, "DateEarnedHardcore", 64).is_empty(),
            hardcore: !string(a, "DateEarnedHardcore", 64).is_empty(),
        })
        .collect();
    Ok(GameDetail {
        game_id: id,
        title: string(v, "Title", 240),
        console_name: string(v, "ConsoleName", 120),
        possible: number(v, "NumAchievements"),
        earned: number(v, "NumAwardedToUser"),
        hardcore: number(v, "NumAwardedToUserHardcore"),
        achievements,
        cached,
        warning,
    })
}
pub(crate) fn game(game_id: u64) -> Result<GameDetail, String> {
    if game_id == 0 || game_id > 10000000 {
        return Err("Invalid achievement game ID".into());
    }
    let (v, cached, warning) = get(
        "API_GetGameInfoAndUserProgress.php",
        &[("g", game_id.to_string())],
    )?;
    let result = parse_game(&v, cached, warning)?;
    if result.game_id != game_id {
        return Err("Unexpected achievement game response".into());
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documented_progress_schema_is_sanitized_without_exposing_other_fields() {
        let p=parse_page(&serde_json::json!({"Total":1,"Results":[{"GameID":1,"Title":"Red <game>","ConsoleName":"Game Boy","MaxPossible":10,"NumAwarded":4,"NumAwardedHardcore":2,"Ignored":"not returned"}]}),0,false,String::new()).unwrap();
        assert_eq!(p.games[0].earned, 4);
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("Ignored"));
        assert!(!json.contains("not returned"));
        assert!(parse_page(
            &serde_json::json!({"Success":false}),
            0,
            false,
            String::new()
        )
        .is_err());
    }
    #[test]
    fn achievements_have_separate_normal_and_hardcore_unlocks() {
        let d=parse_game(&serde_json::json!({"ID":1,"Title":"Game","NumAchievements":2,"NumAwardedToUser":1,"Achievements":{"1":{"Title":"First","Description":"Play","Points":5,"DateEarned":"2026-11-12"},"2":{"Title":"Second","DateEarnedHardcore":"2026-11-12"}}}),true,String::new()).unwrap();
        assert_eq!(d.achievements.len(), 2);
        assert!(d.achievements.iter().all(|a| a.earned));
        assert_eq!(d.achievements.iter().filter(|a| a.hardcore).count(), 1);
    }
}
