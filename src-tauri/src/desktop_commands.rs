//! Heavy synchronous backend operations run on the blocking pool, not GTK's
//! main thread or the async executor. Keep their public IPC names unchanged.
use super::{artwork, emulators, profile_saves, Bootstrap, Game};

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn bootstrap() -> Result<Bootstrap, String> {
    blocking(super::bootstrap).await
}
#[tauri::command]
pub(crate) async fn refresh_managed_emulators() -> Result<Bootstrap, String> {
    blocking(|| super::bootstrap_inventory(true)).await
}
#[tauri::command]
pub(crate) async fn import_emulator(
    id: String,
    path: String,
) -> Result<super::emulator_import::Imported, String> {
    let request = profile_saves::begin()?;
    blocking(move || super::emulator_import::import(request, &id, &path)).await
}

#[tauri::command]
pub(crate) async fn scan_console(id: String) -> Result<Vec<Game>, String> {
    blocking(move || super::scan_console(id)).await
}

#[tauri::command]
pub(crate) async fn list_flatpaks() -> Result<Vec<emulators::FlatpakApp>, String> {
    blocking(emulators::list_flatpaks).await
}

#[tauri::command]
pub(crate) async fn get_cover(kind: String, uid: String) -> Result<Option<String>, String> {
    blocking(move || artwork::run_cover_operation(|| super::get_cover(kind, uid))).await
}

#[tauri::command]
pub(crate) async fn save_cover(kind: String, uid: String, bytes: Vec<u8>) -> Result<(), String> {
    blocking(move || artwork::run_cover_operation(|| super::save_cover(kind, uid, bytes))).await
}

#[tauri::command]
pub(crate) async fn save_emulator(
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
    // Reserve order before dispatching to the pool: validation workers may
    // finish in any order, and reopened dialogs share this process-wide order.
    let request = profile_saves::begin()?;
    blocking(move || {
        super::save_emulator(
            request,
            id,
            emulator_name,
            emulator_path,
            launch_args,
            launcher,
            flatpak_id,
            flatpak_installation,
            allow_execute,
            extract_appimage,
            flatpak_runtime_fonts,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn disc_set(id: String, uid: String) -> Result<super::disc_sets::DiscSet, String> {
    blocking(move || super::disc_sets::details(&id, &uid)).await
}
#[tauri::command]
pub(crate) async fn create_disc_set(
    id: String,
    title: String,
    uids: Vec<String>,
) -> Result<Game, String> {
    blocking(move || super::disc_sets::create(&id, &title, &uids)).await
}
#[tauri::command]
pub(crate) async fn inspect_drop(
    paths: Vec<String>,
) -> Result<Vec<super::drop_setup::DropItem>, String> {
    blocking(move || super::drop_setup::inspect(paths)).await
}
#[tauri::command]
pub(crate) async fn import_dropped_cover(
    id: String,
    uid: String,
    path: String,
) -> Result<(), String> {
    blocking(move || super::drop_setup::cover(&id, &uid, &path)).await
}
#[tauri::command]
pub(crate) fn set_app_fullscreen(
    window: tauri::Window,
    fullscreen: Option<bool>,
) -> Result<bool, String> {
    let fullscreen = fullscreen.unwrap_or(!window.is_fullscreen().map_err(|e| e.to_string())?);
    window
        .set_fullscreen(fullscreen)
        .map_err(|e| e.to_string())?;
    Ok(fullscreen)
}
