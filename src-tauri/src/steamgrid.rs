use super::*;
use matrix_cover_core::steamgriddb as api;

#[cfg_attr(feature="desktop",tauri::command)]
pub(crate) fn steamgrid_save_key(key: String) -> Result<bool,String> {
    let key=key.trim();
    if !key.is_empty() && (!(16..=128).contains(&key.len()) || !key.bytes().all(|c|c.is_ascii_alphanumeric()||b"-_".contains(&c))) {
        return Err("Paste a valid SteamGridDB API key.".into());
    }
    let enabled=update_settings(|settings|{settings.steamgriddb_api_key=key.into();Ok(!key.is_empty())})?;
    Ok(enabled)
}
#[cfg_attr(feature="desktop",tauri::command)]
pub(crate) fn steamgrid_open_key() -> Result<(),String> {
    launch_command("xdg-open").arg("https://www.steamgriddb.com/profile/preferences/api")
        .spawn().map(|_|()).map_err(|_|"Cannot open SteamGridDB preferences".into())
}
#[cfg(feature="desktop")]
#[tauri::command]
pub(crate) async fn steamgrid_search(query:String)->Result<Vec<api::Game>,String> {
    tauri::async_runtime::spawn_blocking(move||api::search(&load_settings()?.steamgriddb_api_key,&query)).await.map_err(|e|e.to_string())?
}
#[cfg(feature="desktop")]
#[tauri::command]
pub(crate) async fn steamgrid_grids(game_id:u64)->Result<Vec<api::Grid>,String> {
    tauri::async_runtime::spawn_blocking(move||api::grids(&load_settings()?.steamgriddb_api_key,game_id)).await.map_err(|e|e.to_string())?
}
fn save_grid(kind:&str,uid:&str,key:&str,game_id:u64,grid_id:u64)->Result<(),String> {
    validate_game(kind,uid)?;
    let grid=api::grids(key,game_id)?.into_iter().find(|g|g.id==grid_id).ok_or("Cover is no longer available on SteamGridDB")?;
    let bytes=api::download(&grid)?;
    artwork::run_cover_operation(||save_cover(kind.into(),uid.into(),bytes))
}
#[cfg(feature="desktop")]
#[tauri::command]
pub(crate) async fn steamgrid_save(kind:String,uid:String,game_id:u64,grid_id:u64)->Result<(),String> {
    tauri::async_runtime::spawn_blocking(move||save_grid(&kind,&uid,&load_settings()?.steamgriddb_api_key,game_id,grid_id)).await.map_err(|e|e.to_string())?
}
pub(crate) fn automatic(kind:&str,uid:&str,title:&str,key:&str)->Result<matrix_cover_core::Outcome,String> {
    // Never cache failures or successful saves: Retry must recover immediately,
    // and a deleted image must be downloaded again.
    let result=automatic_lookup(kind,uid,title,key);
    result
}
fn automatic_lookup(kind:&str,uid:&str,title:&str,key:&str)->Result<matrix_cover_core::Outcome,String> {
    // SGDB titles can cover several platforms/editions. Do not guess on ambiguity.
    let matches:Vec<_>=api::search(key,title)?.into_iter().filter(|g|g.name.eq_ignore_ascii_case(title)).collect();
    if matches.len()!=1 {
        return Ok(matrix_cover_core::Outcome{status:"needs-title".into(),message:"Choose the correct game and cover with Find on SteamGridDB.".into(),..Default::default()});
    }
    let grids=api::grids(key,matches[0].id)?;
    let Some(grid)=grids.first() else {return Ok(matrix_cover_core::Outcome{status:"not-found".into(),message:"SteamGridDB has no static portrait covers for this game.".into(),..Default::default()});};
    let bytes=api::download(grid)?;
    artwork::run_cover_operation(|| {
        let game=validate_game(kind,uid)?;
        if read_local_cover_bytes(kind,uid,&game)?.is_none() {
            let (extension,_) = cover_format(&bytes).ok_or("Invalid downloaded cover")?;
            artwork::save_without_replacing(&stored_cover_path(kind,uid,extension)?, &bytes)?;
        }
        Ok(())
    })?;
    Ok(matrix_cover_core::Outcome{status:"downloaded".into(),message:"SteamGridDB cover saved in this console’s Covers folder.".into(),source:Some("SteamGridDB".into()),..Default::default()})
}
