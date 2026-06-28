use serde::Deserialize;
use tauri_plugin_notification::NotificationExt;

/// JavaScript injected into *every* page the WebView loads (including
/// web.whatsapp.com).  It replaces the browser's built-in `Notification` API
/// with a shim that tunnels notification data through Tauri's IPC bridge to
/// the `send_notification` Rust command below.
///
/// This means:
///  • WhatsApp Web thinks it is sending a normal browser notification.
///  • We receive it as a native Windows toast via tauri-plugin-notification.
///  • We never need notification permission to be granted by the OS for a
///    browser — it goes through the app's own permission, which is always on.
pub const INJECTION_SCRIPT: &str = r#"
(function () {
  "use strict";

  // Keep a reference to the real Notification in case we need it later.
  const _RealNotification = window.Notification;

  class TauriNotification {
    constructor(title, options = {}) {
      // Fire-and-forget — route to Rust backend.
      window.__TAURI_INTERNALS__.invoke("send_notification", {
        title: String(title),
        body: options.body ? String(options.body) : "",
        icon: options.icon ? String(options.icon) : "",
      }).catch(() => {
        // Fallback to real notification if IPC fails (e.g. dev mode without Tauri).
        new _RealNotification(title, options);
      });

      // WhatsApp Web checks .permission and calls .requestPermission(); we
      // stub those out so it never shows a permission dialog.
    }

    static get permission() { return "granted"; }
    static requestPermission() { return Promise.resolve("granted"); }

    // Lifecycle stubs — WhatsApp Web may call these.
    addEventListener() {}
    removeEventListener() {}
    close() {}
  }

  Object.defineProperty(window, "Notification", {
    value: TauriNotification,
    writable: false,
    configurable: false,
  });
})();
"#;

/// Payload sent from the JS shim via Tauri IPC.
#[derive(Debug, Deserialize)]
pub struct NotificationPayload {
    title: String,
    body: String,
    #[allow(dead_code)]
    icon: String,
}

/// Receives a notification from the JS shim and fires it as a native Windows
/// toast via tauri-plugin-notification.
#[tauri::command]
pub fn send_notification(
    app: tauri::AppHandle,
    title: String,
    body: String,
    icon: String,
) -> Result<(), String> {
    let _ = icon; // reserved for future use (download & cache the avatar)
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| format!("notification error: {e}"))
}
