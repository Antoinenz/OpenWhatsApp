// Only needed on the non-Windows fallback path further down.
#[cfg(not(target_os = "windows"))]
use tauri_plugin_notification::NotificationExt;

/// JavaScript injected into *every* page the WebView loads (including
/// web.whatsapp.com).  It intercepts both browser- and service-worker-level
/// notifications and tunnels them through Tauri IPC to `send_notification`
/// below, which fires a native Windows toast.
///
/// WhatsApp Web actually uses `ServiceWorkerRegistration.showNotification`
/// for most of its notifications (so they can fire when the tab is hidden),
/// so we hook *both* APIs here.
pub const INJECTION_SCRIPT: &str = r#"
(function () {
  "use strict";

  function send(title, options) {
    options = options || {};
    try {
      return window.__TAURI_INTERNALS__.invoke("send_notification", {
        title: String(title == null ? "" : title),
        body: options.body ? String(options.body) : "",
        icon: options.icon ? String(options.icon) : "",
      });
    } catch (e) {
      return Promise.reject(e);
    }
  }

  // ── 1. window.Notification constructor ─────────────────────────────────
  const _RealNotification = window.Notification;
  class TauriNotification {
    constructor(title, options) {
      send(title, options).catch(function () {
        // Fallback only if the real API exists (dev mode, etc.).
        if (_RealNotification) { try { new _RealNotification(title, options); } catch (_) {} }
      });
    }
    static get permission()   { return "granted"; }
    static get maxActions()   { return 0; }
    static requestPermission(cb) {
      if (typeof cb === "function") cb("granted");
      return Promise.resolve("granted");
    }
    addEventListener()    {}
    removeEventListener() {}
    close()               {}
  }
  try {
    Object.defineProperty(window, "Notification", {
      value: TauriNotification, writable: false, configurable: false,
    });
  } catch (_) { window.Notification = TauriNotification; }

  // ── 2. ServiceWorkerRegistration.showNotification ──────────────────────
  // This is the one WhatsApp Web actually uses.
  if (window.ServiceWorkerRegistration &&
      window.ServiceWorkerRegistration.prototype) {
    const proto = window.ServiceWorkerRegistration.prototype;
    const orig  = proto.showNotification;
    proto.showNotification = function (title, options) {
      return send(title, options).catch(function () {
        if (orig) return orig.call(this, title, options);
      });
    };
    // Also stub getNotifications so the page doesn't crash if it calls it.
    if (typeof proto.getNotifications !== "function") {
      proto.getNotifications = function () { return Promise.resolve([]); };
    }
  }

  // ── 3. Permissions API ─────────────────────────────────────────────────
  if (navigator.permissions && navigator.permissions.query) {
    const origQuery = navigator.permissions.query.bind(navigator.permissions);
    navigator.permissions.query = function (descriptor) {
      if (descriptor && descriptor.name === "notifications") {
        return Promise.resolve({
          state: "granted",
          status: "granted",
          onchange: null,
          addEventListener: function () {},
          removeEventListener: function () {},
        });
      }
      return origQuery(descriptor);
    };
  }
})();
"#;

/// Receives a notification from the JS shim and fires it as a native Windows
/// toast.
///
/// On Windows we bypass tauri-plugin-notification's high-level `.show()` and
/// call `notify-rust` directly.  The plugin always sets our own app
/// identifier as the toast's AppUserModelID (AUMID) and fires it from a
/// spawned task whose Result is discarded — so if that AUMID isn't
/// registered on this machine (e.g. a missing/broken Start Menu shortcut, or
/// the exe run from a portable/unregistered location), the toast silently
/// vanishes with zero indication anywhere, and our JS-side fallback never
/// engages because the plugin reports success regardless.
///
/// Here we try our own AUMID first (shows "OpenWhatsApp" as the sender), and
/// if that fails, immediately retry with no AUMID at all — notify-rust then
/// falls back to the PowerShell AUMID, which ships pre-registered on every
/// Windows 10/11 install, guaranteeing the toast actually appears (just with
/// a generic sender name) instead of disappearing outright.
#[tauri::command]
pub fn send_notification(
    app: tauri::AppHandle,
    title: String,
    body: String,
    icon: String,
) -> Result<(), String> {
    let _ = icon; // reserved for future use (download & cache the avatar)

    #[cfg(target_os = "windows")]
    {
        use notify_rust::Notification as WinToast;

        let identifier = tauri::Manager::config(&app).identifier.clone();
        if WinToast::new()
            .summary(&title)
            .body(&body)
            .app_id(&identifier)
            .show()
            .is_ok()
        {
            return Ok(());
        }

        return WinToast::new()
            .summary(&title)
            .body(&body)
            .show()
            .map(|_| ())
            .map_err(|e| format!("notification error: {e}"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        app.notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map(|_| ())
            .map_err(|e| format!("notification error: {e}"))
    }
}
