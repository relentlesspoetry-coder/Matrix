//! SteamGridDB v2: bounded authenticated metadata, public CDN image downloads.
use serde::{Deserialize, Serialize};
use std::{io::Read, time::{Duration, Instant}, sync::Mutex};
use reqwest::{blocking::Client, Url};
static LAST_REQUEST: Mutex<Option<Instant>> = Mutex::new(None);
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Game { pub id: u64, pub name: String }
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Grid { pub id: u64, pub url: String }
fn client() -> Result<Client, String> {
    Client::builder().https_only(true).redirect(reqwest::redirect::Policy::none())
        .user_agent("Matrix-Play/SteamGridDB").connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15)).build().map_err(|_| "Cannot start SteamGridDB client".into())
}
fn bounded(mut response: reqwest::blocking::Response, limit: usize) -> Result<Vec<u8>, String> {
    let status=response.status();
    if !status.is_success() { return Err(match status.as_u16() {401|403=>"SteamGridDB API key was rejected. Update it in Settings.".into(),429=>"SteamGridDB rate limit reached. Try again later.".into(),_=>format!("SteamGridDB returned HTTP {}",status.as_u16())}); }
    let mut bytes=Vec::new();
    Read::by_ref(&mut response).take((limit+1) as u64).read_to_end(&mut bytes).map_err(|_|"SteamGridDB download interrupted")?;
    if bytes.len()>limit {return Err("SteamGridDB response is too large".into());} Ok(bytes)
}
fn api<T: serde::de::DeserializeOwned>(key: &str, path: &str) -> Result<Vec<T>,String> {
    if key.is_empty() {return Err("Add your SteamGridDB API key in Settings first.".into());}
    let mut guard=LAST_REQUEST.lock().map_err(|_|"SteamGridDB worker unavailable")?;
    if let Some(last)=*guard { if last>Instant::now(){return Err("SteamGridDB is cooling down after a rate limit. Try later.".into());} let elapsed=last.elapsed();if elapsed<Duration::from_millis(500) {std::thread::sleep(Duration::from_millis(500)-elapsed);} }
    *guard=Some(Instant::now());
    let response=client()?.get(format!("https://www.steamgriddb.com/api/v2/{path}"))
        .bearer_auth(key).send().map_err(|_|"Cannot reach SteamGridDB. Check your connection.")?;
    if response.status().as_u16()==429 {
        let delay=response.headers().get("Retry-After").and_then(|v|v.to_str().ok()).and_then(|v|v.parse::<u64>().ok()).unwrap_or(60).clamp(1,86400);
        *guard=Some(Instant::now()+Duration::from_secs(delay));
    }
    #[derive(Deserialize)] struct Envelope<T> { success: bool, data: Vec<T> }
    let value:Envelope<T>=serde_json::from_slice(&bounded(response,1024*1024)?).map_err(|_|"Invalid SteamGridDB response")?;
    if !value.success {return Err("SteamGridDB lookup failed".into());} Ok(value.data)
}
pub fn search(key:&str,query:&str)->Result<Vec<Game>,String> {
    let query=query.trim();if query.is_empty()||query.len()>240{return Err("Enter a game title (up to 240 characters).".into());}
    let mut url=Url::parse("https://www.steamgriddb.com/api/v2/search/autocomplete/").unwrap();
    url.path_segments_mut().unwrap().pop_if_empty().push(query);
    api(key,url.as_str().strip_prefix("https://www.steamgriddb.com/api/v2/").unwrap())
}
pub fn allowed_image(value:&str)->bool {
    Url::parse(value).ok().is_some_and(|url| url.scheme()=="https" && url.username().is_empty() && url.password().is_none() && url.port().is_none() && matches!(url.host_str(),Some("cdn2.steamgriddb.com")|Some("cdn.steamgriddb.com")) && url.path().starts_with("/"))
}
pub fn grids(key:&str,id:u64)->Result<Vec<Grid>,String> {
    let mut values:Vec<Grid>=api(key,&format!("grids/game/{id}?dimensions=600x900&types=static&nsfw=false&humor=false"))?;
    values.retain(|g|allowed_image(&g.url)); values.truncate(20);Ok(values)
}
pub fn download(grid:&Grid)->Result<Vec<u8>,String> {
    if !allowed_image(&grid.url){return Err("Unsupported SteamGridDB image host".into());}
    // Credentials never leave the API origin, including image requests.
    let response=client()?.get(&grid.url).send().map_err(|_|"Cannot download SteamGridDB image")?;
    let bytes=bounded(response,super::MAX_BYTES)?;super::validate_image(&bytes)?;Ok(bytes)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn image_origins_are_exact_and_https_only() {
        assert!(allowed_image("https://cdn2.steamgriddb.com/grid/a.png"));
        for url in ["http://cdn2.steamgriddb.com/a","https://cdn2.steamgriddb.com.evil.test/a","https://user@cdn2.steamgriddb.com/a","https://cdn2.steamgriddb.com:444/a","file:///tmp/a"]{assert!(!allowed_image(url));}
    }
    #[test] fn missing_keys_and_empty_titles_do_not_send_requests() {
        assert!(search("", "Game").unwrap_err().contains("Settings"));
        assert!(search("key", "").is_err());
    }
}
