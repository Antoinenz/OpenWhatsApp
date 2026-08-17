// Prevents a console window appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod notifications;
mod session;
mod tray;
mod tweaks;

use tauri::{
    image::Image, utils::config::Color, webview::NewWindowResponse, Manager, Url, WebviewUrl,
    WebviewWindowBuilder,
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_opener::OpenerExt;

// Embed the 32 px icon at compile time so the tray always has the right image,
// even in dev builds where the bundle icons aren't "installed" anywhere.
const ICON_32: &[u8] = include_bytes!("../icons/32x32.png");

// Small red dot drawn over the taskbar icon when there are unread messages.
const BADGE_DOT: &[u8] = include_bytes!("../icons/badge-dot.png");

/// True for web.whatsapp.com itself and its CDN/subdomains — i.e. anything
/// that's a legitimate in-app destination rather than a link a user clicked.
/// `None` (e.g. "about:blank", used as a window.open() placeholder target)
/// is treated as internal too, since it's never itself an external site.
fn is_whatsapp_host(url: &Url) -> bool {
    match url.host_str() {
        Some(host) => {
            host == "whatsapp.com"
                || host == "whatsapp.net"
                || host.ends_with(".whatsapp.com")
                || host.ends_with(".whatsapp.net")
        }
        None => true,
    }
}

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
            // Tag the process launched at login so .setup() below can start
            // it hidden-to-tray instead of popping the window up — matching
            // how Discord/Slack/Spotify behave when "start with system" is on.
            Some(vec!["--autostart"]),
        ))
        // Lets links clicked inside WhatsApp Web open in the user's default
        // browser instead of doing nothing (see tweaks.rs for the JS side).
        .plugin(tauri_plugin_opener::init())
        // Remembers window size/position/maximized state across restarts —
        // most native Windows apps do this and its absence stands out.
        // Deliberately NOT persisting visibility: the window should always
        // appear on launch regardless of whether it was hidden-to-tray when
        // the app last quit.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .setup(|app| {
            // ── Persistent WebView2 profile (the "stays logged in" trick) ─────
            let data_dir = session::profile_dir(app.handle());
            std::fs::create_dir_all(&data_dir)?;

            // Windows passes our own --autostart arg back to us when it
            // launches OpenWhatsApp at login (see the autostart plugin
            // registration above) — start hidden-to-tray in that case rather
            // than popping a window up before the user has even reached the
            // desktop.
            let launched_at_startup = std::env::args().any(|a| a == "--autostart");

            // Cloned once per hook below — each is its own `move` closure so
            // each needs its own owned handle.
            let opener_for_navigation = app.handle().clone();
            let opener_for_new_window = app.handle().clone();

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
            .visible(!launched_at_startup)
            .icon(Image::from_bytes(ICON_32).expect("bundled icon is valid"))?
            // Required for HTML5 drag-and-drop (file uploads) to work on
            // Windows — disables Tauri's own handler so events reach the WebView.
            .disable_drag_drop_handler()
            // Ctrl+= / Ctrl+- / Ctrl+0 page zoom, like every browser and most
            // desktop apps — off by default in WebView2.
            .zoom_hotkeys_enabled(true)
            // Lets WhatsApp's "copy image" buttons and Ctrl+V paste-image work
            // through the async Clipboard API, not just plain text.
            .enable_clipboard_access()
            // WhatsApp Web has no password/payment fields; Edge's autofill
            // dropdown popping up over the UI would look out of place in a
            // dedicated chat client, so we turn it off.
            .general_autofill_enabled(false)
            // Clicking a link in a chat is a real same-window navigation
            // (WhatsApp's own message links don't set target="_blank", so
            // the opener plugin's built-in click-handler — which only fires
            // for target="_blank" or a Ctrl/Shift-click — never sees them).
            // This hook catches every other way the WebView could be told to
            // navigate to a new URL, regardless of what triggered it: plain
            // link clicks, `location.href = ...`, form submissions, etc.
            // WhatsApp itself is allowed through; everything else is handed
            // to the OS default browser instead and the in-app navigation is
            // blocked, so the chat window never actually leaves whatsapp.com.
            .on_navigation(move |url| {
                if is_whatsapp_host(url) {
                    return true;
                }
                let _ = opener_for_navigation
                    .opener()
                    .open_url(url.to_string(), None::<String>);
                false
            })
            // Same idea for window.open()-style "new window" requests that
            // the opener plugin's click-handler doesn't end up preventing
            // (e.g. triggered by something other than a plain click).
            .on_new_window(move |url, _features| {
                if is_whatsapp_host(&url) {
                    return NewWindowResponse::Allow;
                }
                let _ = opener_for_new_window
                    .opener()
                    .open_url(url.to_string(), None::<String>);
                NewWindowResponse::Deny
            })
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

            // Release-only: kill the WebView2 native right-click menu. We keep
            // it in debug builds so we can still hit "Inspect" while iterating.
            #[cfg(not(debug_assertions))]
            {
                window_builder =
                    window_builder.initialization_script(tweaks::PROD_INJECTION_SCRIPT);
            }

            // WebView2-specific perf hints (Windows-only API; cfg-gated to avoid
            // breaking Linux/macOS dev builds if anyone tries them).
            //
            // The three --disable-background-timer-throttling / occluded-windows /
            // renderer-backgrounding flags plus CalculateNativeWinOcclusion are
            // the same combination Slack/Discord/Electron apps use to stop
            // Chromium from suspending timers on a minimized/hidden window.
            // Without them, closing OpenWhatsApp to the tray lets Chromium
            // throttle the page after ~1 minute hidden, which can delay
            // WhatsApp's WebSocket-driven message delivery and notifications
            // until the window is brought back to the foreground.
            #[cfg(target_os = "windows")]
            {
                window_builder = window_builder.additional_browser_args(
                    "--disable-features=msSmartScreenProtection,MicrosoftEdgeAutoUpdater,CalculateNativeWinOcclusion \
                     --enable-features=msWebView2EnableDraggableRegions \
                     --disable-background-timer-throttling \
                     --disable-backgrounding-occluded-windows \
                     --disable-renderer-backgrounding",
                );
            }

            let _window = window_builder.build()?;

            tray::setup(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            notifications::send_notification,
            quit_app,
            set_unread_badge,
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

/// Called from the JS shim whenever the "(N) " unread-count title prefix
/// appears or disappears (see tweaks.rs). Draws — or clears — a small red
/// dot over the taskbar icon, matching how Discord/Slack/Teams surface
/// "you have something waiting" without the user having to alt-tab back to
/// OpenWhatsApp first. We show a plain dot rather than an exact count since
/// Windows only supports badge counts via a hand-drawn overlay icon, not a
/// number (`set_badge_count` is explicitly unsupported on Windows).
#[tauri::command]
fn set_unread_badge(window: tauri::WebviewWindow, has_unread: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let icon = if has_unread {
            Some(Image::from_bytes(BADGE_DOT).map_err(|e| e.to_string())?)
        } else {
            None
        };
        window.set_overlay_icon(icon).map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, has_unread);
        Ok(())
    }
}
