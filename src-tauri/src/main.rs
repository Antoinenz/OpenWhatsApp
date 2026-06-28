// Prevents a console window appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod notifications;
mod session;
mod tray;
mod tweaks;

use tauri::{utils::config::Color, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::MacosLauncher;

fn main() {
    tauri::Builder::default()
        // Single-instance: focus existing window if user launches us twice.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // ── Persistent WebView2 profile (the "stays logged in" trick) ─────
            let data_dir = session::profile_dir(app.handle());
            std::fs::create_dir_all(&data_dir)?;

            // ── Main window: load WhatsApp Web directly ──────────────────────
            let mut window_builder = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(
                    "https://web.whatsapp.com"
                        .parse()
                        .expect("invalid WhatsApp URL"),
                ),
            )
            .title("OpenWhatsApp")
            .inner_size(1280.0, 800.0)
            .min_inner_size(800.0, 600.0)
            .decorations(true)
            .visible(true)
            // NB: we deliberately do *not* hard-code a User-Agent here.
            // Doing so pinned a Chrome version into the HTTP header sent to
            // WhatsApp's server, and that version goes stale → server starts
            // returning an "Update Google Chrome" page. Instead we let the
            // real, current WebView2/Edge UA flow over the wire, and we
            // override `navigator.userAgent` *from JS* (see tweaks.rs) so the
            // client-side desktop-app check still sees an Electron-flavoured
            // UA based on the live Chrome version.
            // Persistent profile dir → cookies + IndexedDB survive restarts.
            .data_directory(data_dir)
            // Two scripts injected before any page script runs.
            .initialization_script(notifications::INJECTION_SCRIPT)
            .initialization_script(tweaks::INJECTION_SCRIPT)
            // Match WhatsApp's dark-theme background so the unrendered strip
            // during a resize doesn't flash white — that's the resize "jank".
            .background_color(Color(17, 27, 33, 255));

            // WebView2-specific perf hints (Windows-only API; cfg-gated to avoid
            // breaking Linux/macOS dev builds if anyone tries them).
            #[cfg(target_os = "windows")]
            {
                window_builder = window_builder.additional_browser_args(
                    "--disable-features=msSmartScreenProtection,MicrosoftEdgeAutoUpdater \
                     --enable-features=msWebView2EnableDraggableRegions",
                );
            }

            let _window = window_builder.build()?;

            tray::setup(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            notifications::send_notification,
            quit_app,
        ])
        .on_window_event(|window, event| {
            // Close button → hide to tray instead of quitting.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running OpenWhatsApp");
}

/// Invoked by the in-page Ctrl+Q shortcut to fully terminate the app
/// (bypasses close-to-tray behaviour).
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
