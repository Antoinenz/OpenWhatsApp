use tauri::Manager;

/// Returns the path where WebView2 will store its user data (cookies, local
/// storage, IndexedDB, etc.).  Keeping this in a stable, app-specific location
/// is the key reason OpenWhatsApp never logs you out — the session is simply
/// never wiped between restarts.
pub fn profile_dir(app: &tauri::AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .expect("could not resolve app data dir")
        .join("webview-profile")
}
