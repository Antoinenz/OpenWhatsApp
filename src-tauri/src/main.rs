// Prevents a console window appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;
mod notifications;
mod session;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::MacosLauncher;

fn main() {
    tauri::Builder::default()
        // Ensure only one instance runs; focus the existing window if the user
        // tries to open a second one.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // ── Persistent WebView2 data directory ────────────────────────────
            // Storing the profile here means WhatsApp Web session cookies and
            // encryption keys survive restarts — the user never gets logged out.
            let data_dir = session::profile_dir(app.handle());
            std::fs::create_dir_all(&data_dir)?;

            // ── Main window ───────────────────────────────────────────────────
            let _window = WebviewWindowBuilder::new(
                app,
                "main",
                // Start on the splash page; navigate_to_whatsapp() takes over
                // immediately after the JS invoke lands.
                WebviewUrl::App("index.html".into()),
            )
            .title("OpenWhatsApp")
            .inner_size(1280.0, 800.0)
            .min_inner_size(800.0, 600.0)
            .decorations(true)
            .visible(true)
            // Point WebView2 at our persistent profile directory.
            .data_directory(data_dir)
            // Inject the notification bridge before any page script runs.
            .initialization_script(notifications::INJECTION_SCRIPT)
            .build()?;

            // ── System tray ───────────────────────────────────────────────────
            tray::setup(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            navigate_to_whatsapp,
            notifications::send_notification,
        ])
        .on_window_event(|window, event| {
            // Minimise to tray instead of quitting when the user closes the window.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running OpenWhatsApp");
}

/// Called by the splash-screen JS the moment the webview is ready.
/// We navigate straight to WhatsApp Web so WebView2 uses the persistent session.
#[tauri::command]
fn navigate_to_whatsapp(window: tauri::WebviewWindow) -> Result<(), String> {
    window
        .navigate("https://web.whatsapp.com".parse().map_err(|e| format!("{e}"))?)
        .map_err(|e| format!("{e}"))
}
