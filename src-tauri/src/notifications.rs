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
        .map(|_| ())
        .map_err(|e| format!("notification error: {e}"))
}
