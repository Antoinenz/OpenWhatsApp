use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime,
};
use tauri_plugin_autostart::ManagerExt;

/// Build the system-tray icon and its context menu.
pub fn setup<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Open OpenWhatsApp", true, None::<&str>)?;

    // Reflect whatever the OS actually has registered right now (e.g. if the
    // user removed the Startup shortcut by hand) rather than assuming.
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart_item = CheckMenuItem::with_id(
        app,
        "autostart",
        "Start with Windows",
        true,
        autostart_enabled,
        None::<&str>,
    )?;

    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &autostart_item, &separator, &quit_item])?;

    // Cloned into the menu-event closure below so we can flip its checkmark
    // after a successful toggle (the closure is `Fn`, called repeatedly, so
    // it needs its own owned handle rather than borrowing `autostart_item`).
    let autostart_item_for_toggle = autostart_item.clone();

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("OpenWhatsApp")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
            "autostart" => {
                let currently_enabled = app.autolaunch().is_enabled().unwrap_or(false);
                let toggled = if currently_enabled {
                    app.autolaunch().disable()
                } else {
                    app.autolaunch().enable()
                };
                if toggled.is_ok() {
                    let _ = autostart_item_for_toggle.set_checked(!currently_enabled);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Single left-click on the tray icon → show / focus the window.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
        });

    // Load the 32 px icon directly from the embedded bytes so the tray always
    // shows the correct image regardless of build mode or install state.
    const ICON_32: &[u8] = include_bytes!("../icons/32x32.png");
    let icon = Image::from_bytes(ICON_32).expect("bundled tray icon is valid");
    builder = builder.icon(icon);

    builder.build(app)?;
    Ok(())
}
