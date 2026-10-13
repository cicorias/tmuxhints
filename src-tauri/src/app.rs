//! The tray app: tray icon and menu, global hotkey, and the cheat-sheet popup.

use crate::cheatsheet::{self, Section};
use crate::cli::{self, Action};
use crate::settings::{self, Settings};
use crate::{autostart, hotkey};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, RunEvent, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const POPUP: &str = "popup";
const TRAY: &str = "main";

/// Whether the popup has had keyboard focus since it was last shown.
static HAD_FOCUS: AtomicBool = AtomicBool::new(false);

pub fn run(action: Action) {
    // A second launch never gets here: the single-instance plugin hands its arguments
    // to the running app and exits. Reaching setup with --reload means nothing was running.
    if action == Action::Reload {
        return;
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            handle(app, cli::forwarded_action(&argv));
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        handle(app, Action::Toggle);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![get_cheatsheet, hide_popup])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            build_tray(app.handle())?;
            apply_hotkey(app.handle());

            let popup = app
                .get_webview_window(POPUP)
                .expect("popup window is defined in tauri.conf.json");
            let window = popup.clone();
            popup.on_window_event(move |event| match event {
                WindowEvent::Focused(true) => HAD_FOCUS.store(true, Ordering::Relaxed),
                // Click-away closes the popup, but only once it actually had focus: when
                // Windows refuses to hand focus to a background request, the popup gets a
                // focus-lost event right away and would vanish before anyone saw it.
                WindowEvent::Focused(false) if HAD_FOCUS.load(Ordering::Relaxed) => {
                    let _ = window.hide();
                }
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                _ => {}
            });

            handle(app.handle(), action);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start tmuxhints");

    app.run(|_app, event| {
        // Tray app: keep running with no visible windows; only Quit exits.
        if let RunEvent::ExitRequested { api, code, .. } = event
            && code.is_none()
        {
            api.prevent_exit();
        }
    });
}

fn handle(app: &AppHandle, action: Action) {
    let Some(popup) = app.get_webview_window(POPUP) else {
        return;
    };
    match action {
        Action::Start => {}
        Action::Show => show(&popup),
        Action::Hide => {
            let _ = popup.hide();
        }
        Action::Toggle => {
            if popup.is_visible().unwrap_or(false) {
                let _ = popup.hide();
            } else {
                show(&popup);
            }
        }
        Action::Reload => {
            apply_hotkey(app);
            let _ = app.emit("reload", ());
        }
    }
}

fn show(popup: &WebviewWindow) {
    HAD_FOCUS.store(false, Ordering::Relaxed);
    center_on_cursor_monitor(popup);
    let _ = popup.show();
    let _ = popup.set_focus();
}

/// Centers the popup on the monitor under the mouse, so it opens where you're looking.
fn center_on_cursor_monitor(popup: &WebviewWindow) {
    let placed = (|| -> tauri::Result<bool> {
        let cursor = popup.cursor_position()?;
        let Some(monitor) = popup.monitor_from_point(cursor.x, cursor.y)? else {
            return Ok(false);
        };
        let size = popup.outer_size()?;
        let (origin, area) = (monitor.position(), monitor.size());
        let x = origin.x + (area.width as i32 - size.width as i32) / 2;
        let y = origin.y + (area.height as i32 - size.height as i32) / 2;
        popup.set_position(PhysicalPosition::new(x, y))?;
        Ok(true)
    })();
    if !matches!(placed, Ok(true)) {
        let _ = popup.center();
    }
}

/// (Re)registers the hotkey from settings and reflects the result in the tray tooltip.
fn apply_hotkey(app: &AppHandle) {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();

    let status = Settings::load()
        .and_then(|s| hotkey::parse(&s.hotkey))
        .and_then(|hk| {
            let label = hotkey::display(&hk);
            if !hotkey::app_registers_hotkey() {
                return Ok(format!("{label} (bound by the compositor)"));
            }
            shortcuts
                .register(hk)
                .map(|()| label.clone())
                .map_err(|e| format!("{label} is unavailable: {e}"))
        });
    let tooltip = match status {
        Ok(label) => format!("tmuxhints — {label}"),
        Err(e) => {
            eprintln!("tmuxhints: {e}");
            format!("tmuxhints — {e}")
        }
    };
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show cheat sheet", true, None::<&str>)?;
    let startup = CheckMenuItem::with_id(
        app,
        "autostart",
        "Start at logon",
        true,
        autostart::is_enabled().unwrap_or(false),
        None::<&str>,
    )?;
    let config = MenuItem::with_id(app, "config", "Open config folder", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &PredefinedMenuItem::separator(app)?,
            &startup,
            &config,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut tray = TrayIconBuilder::with_id(TRAY)
        .tooltip("tmuxhints")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => handle(app, Action::Show),
            "autostart" => {
                let want = !autostart::is_enabled().unwrap_or(false);
                if let Err(e) = autostart::set(want) {
                    eprintln!("tmuxhints: autostart: {e}");
                }
                let _ = startup.set_checked(autostart::is_enabled().unwrap_or(false));
            }
            "config" => open_config_folder(),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                handle(tray.app_handle(), Action::Toggle);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn open_config_folder() {
    let dir = settings::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(dir).spawn();
}

#[derive(Serialize)]
struct Payload {
    title: String,
    subtitle: String,
    prefix: String,
    hotkey: String,
    sections: Vec<Section>,
    warning: Option<String>,
}

#[tauri::command]
fn get_cheatsheet() -> Payload {
    let (settings, settings_warning) = match Settings::load() {
        Ok(s) => (s, None),
        Err(e) => (Settings::default(), Some(format!("using defaults: {e}"))),
    };
    let (sheet, sheet_warning) = cheatsheet::load();
    let sheet = cheatsheet::with_prefix(sheet, &settings.prefix);
    let hotkey = hotkey::parse(&settings.hotkey)
        .map(|hk| hotkey::display(&hk))
        .unwrap_or(settings.hotkey);
    Payload {
        title: sheet.title,
        subtitle: sheet.subtitle,
        prefix: settings.prefix,
        hotkey,
        sections: sheet.sections,
        warning: settings_warning.or(sheet_warning),
    }
}

#[tauri::command]
fn hide_popup(window: WebviewWindow) {
    let _ = window.hide();
}
