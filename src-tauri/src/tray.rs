use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime,
};

/// Build the system-tray icon and its context menu.
pub fn setup<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Open OpenWhatsApp", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("OpenWhatsApp")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
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
