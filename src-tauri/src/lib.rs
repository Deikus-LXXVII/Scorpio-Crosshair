pub mod app;
pub mod core;
#[cfg(target_os = "linux")]
mod desktop_entry;
pub mod platform;
pub mod obs;
pub mod store;
pub mod update;

use app::{App, Display, Effects, Hotkey, Status};
use core::crosshair::{self, Crosshair};
use core::focus::WindowInfo;
use platform::{Command, Platform};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Mutex;
use store::Settings;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const SETTINGS_WINDOW: &str = "settings";

struct Shared {
    app: Mutex<App>,
    platform: Platform,
    path: PathBuf,
    /// Registered shortcut id -> what it does.
    hotkeys: Mutex<HashMap<u32, Hotkey>>,
    lock_item: MenuItem<tauri::Wry>,
    /// Held from reading the settings to writing them, so two threads saving
    /// at once can't write an older copy last.
    saving: Mutex<()>,
    obs: obs::Obs,
}

fn shared(handle: &AppHandle) -> tauri::State<'_, Shared> {
    handle.state::<Shared>()
}

/// Carries out what an app step decided: platform commands, saving, telling
/// the settings window.
fn apply(handle: &AppHandle, effects: Effects) {
    let state = shared(handle);
    let overlay_changed = !effects.commands.is_empty();
    for command in effects.commands {
        state.platform.send(command);
    }
    if overlay_changed || effects.settings_changed {
        let view = state.app.lock().unwrap().stream_view();
        state.obs.publish(serde_json::to_string(&view).unwrap_or_default());
    }
    if effects.settings_changed {
        let settings = persist(handle);
        let _ = handle.emit_to(SETTINGS_WINDOW, "settings", &settings);
    }
    if effects.status_changed || effects.settings_changed {
        let status = state.app.lock().unwrap().status();
        let _ = state.lock_item.set_text(if status.locked { "Unlock position" } else { "Lock position" });
        let _ = handle.emit_to(SETTINGS_WINDOW, "status", &status);
    }
}

/// Writes the current settings and returns what was written.
fn persist(handle: &AppHandle) -> Settings {
    let state = shared(handle);
    let _saving = state.saving.lock().unwrap();
    let settings = state.app.lock().unwrap().settings.clone();
    if let Err(error) = store::save(&state.path, &settings) {
        log::error!("could not save settings: {error}");
    }
    settings
}

fn primary_display(handle: &AppHandle) -> Option<Display> {
    let monitor = handle.primary_monitor().ok().flatten()?;
    let (position, size) = (monitor.position(), monitor.size());
    Some(Display { x: position.x, y: position.y, width: size.width, height: size.height })
}

/// Registers the player's shortcuts; returns the ones the system refused
/// (taken by another program, or not a valid key).
fn register_hotkeys(handle: &AppHandle, settings: &Settings) -> Vec<String> {
    let state = shared(handle);
    let shortcuts = handle.global_shortcut();
    let _ = shortcuts.unregister_all();
    // Built without the lock: registering waits on the main thread, and the
    // shortcut handler takes this lock there.
    let mut map = HashMap::new();
    let mut refused = Vec::new();
    for (id, accelerator) in settings.keys.all() {
        if accelerator.trim().is_empty() {
            continue;
        }
        match accelerator.parse::<Shortcut>() {
            Ok(shortcut) if !map.contains_key(&shortcut.id()) && shortcuts.register(shortcut).is_ok() => {
                map.insert(shortcut.id(), Hotkey::from_id(id).expect("known id"));
            }
            _ => refused.push(id.to_string()),
        }
    }
    *state.hotkeys.lock().unwrap() = map;
    refused
}

fn sync_autostart(handle: &AppHandle, wanted: bool) {
    let launcher = handle.autolaunch();
    if launcher.is_enabled().unwrap_or(false) != wanted {
        let result = if wanted { launcher.enable() } else { launcher.disable() };
        if let Err(error) = result {
            log::warn!("autostart: {error}");
        }
    }
}

/// The settings window exists only while it is open: closing it destroys
/// the web view, so nothing of it stays in memory during play.
fn open_settings(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window(SETTINGS_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(handle, SETTINGS_WINDOW, WebviewUrl::App("index.html".into()))
        .title(app::SETTINGS_TITLE)
        .inner_size(980.0, 700.0)
        .min_inner_size(760.0, 560.0)
        .decorations(false)
        .background_color(tauri::window::Color(11, 10, 9, 255))
        .center()
        .build();
    if let Err(error) = built {
        log::error!("could not open settings: {error}");
    }
}

/// The overlay goes away before Scorpio does (quit or update).
pub fn before_exit(handle: &AppHandle) {
    shared(handle).platform.send(Command::Hide);
}

pub fn settings_snapshot(handle: &AppHandle) -> Settings {
    shared(handle).app.lock().unwrap().settings.clone()
}

/// True while the crosshair is over a game (or a detected window).
pub fn game_in_front(handle: &AppHandle) -> bool {
    shared(handle).app.lock().unwrap().focus.visible
}

fn quit(handle: &AppHandle) {
    shared(handle).platform.send(Command::Quit);
    handle.exit(0);
}

/* Commands for the settings window */

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    settings: Settings,
    status: Status,
    styles: Vec<(u8, &'static str)>,
    display_height: u32,
    installed: usize,
    platform: &'static str,
    version: &'static str,
    update: update::UpdateState,
    obs_address: String,
}

#[tauri::command]
fn view(handle: AppHandle) -> View {
    let update = update::state(&handle);
    let state = shared(&handle);
    let app = state.app.lock().unwrap();
    View {
        settings: app.settings.clone(),
        status: app.status(),
        styles: crosshair::STYLES.iter().map(|(id, name, _)| (*id, *name)).collect(),
        display_height: app.display.height,
        installed: app.installed.len(),
        platform: std::env::consts::OS,
        version: env!("CARGO_PKG_VERSION"),
        update,
        obs_address: obs::address(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    settings: Settings,
    /// Shortcuts that could not be registered.
    refused: Vec<String>,
    /// Why the OBS source could not start, if it couldn't.
    obs_error: Option<String>,
}

/// Turns the OBS source on or off as the settings say; on failure the
/// setting is turned back off.
fn sync_obs(handle: &AppHandle) -> Option<String> {
    let state = shared(handle);
    let wanted = state.app.lock().unwrap().settings.obs;
    let error = state.obs.set_enabled(wanted).err();
    if error.is_some() {
        state.app.lock().unwrap().settings.obs = false;
    }
    error
}

/// Saves new settings and returns them validated.
#[tauri::command]
fn save_settings(handle: AppHandle, settings: Settings) -> Saved {
    let state = shared(&handle);
    let (effects, keys_changed, settings) = {
        let mut app = state.app.lock().unwrap();
        let keys_changed = app.settings.keys != settings.keys;
        let effects = app.replace_settings(settings);
        (effects, keys_changed, app.settings.clone())
    };
    let refused = if keys_changed { register_hotkeys(&handle, &settings) } else { Vec::new() };
    sync_autostart(&handle, settings.start_with_system);
    let obs_error = sync_obs(&handle);
    // The window already has these settings: saved, but not echoed back.
    let settings = persist(&handle);
    apply(&handle, Effects { settings_changed: false, status_changed: true, ..effects });
    Saved { settings, refused, obs_error }
}

/// The crosshair as SVG, sized for the display it will be drawn on.
#[tauri::command]
fn preview(handle: AppHandle, crosshair: Crosshair) -> String {
    let height = shared(&handle).app.lock().unwrap().display.height;
    crosshair::to_svg(&crosshair::draw(&crosshair.normalized(), height))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadCode {
    format: &'static str,
    code_style: u8,
    crosshair: Crosshair,
}

#[tauri::command]
fn read_code(handle: AppHandle, code: String) -> Option<ReadCode> {
    let height = shared(&handle).app.lock().unwrap().display.height;
    crosshair::read_code(&code, height)
        .ok()
        .map(|decoded| ReadCode { format: decoded.format, code_style: decoded.code_style, crosshair: decoded.crosshair })
}

#[tauri::command]
fn write_code(crosshair: Crosshair) -> String {
    crosshair::write_code(&crosshair.normalized())
}

/// Windows open now, for "add a running game". Async so it runs off the
/// main thread.
#[tauri::command]
async fn running_windows(handle: AppHandle) -> Vec<WindowInfo> {
    let state = shared(&handle);
    let own = std::process::id();
    let mut windows: Vec<WindowInfo> = (state.platform.list_windows)().into_iter().filter(|w| w.pid != own).collect();
    windows.sort_by_key(|w| w.name.to_lowercase());
    windows
}

#[tauri::command]
fn add_window(handle: AppHandle, window: WindowInfo) -> bool {
    let state = shared(&handle);
    let added = state.app.lock().unwrap().add_game(&window);
    if added {
        let effects = Effects { settings_changed: true, status_changed: true, ..Effects::default() };
        apply(&handle, effects);
    }
    added
}

#[tauri::command]
fn set_locked(handle: AppHandle, locked: bool) {
    let effects = shared(&handle).app.lock().unwrap().set_locked(locked);
    apply(&handle, effects);
}

#[tauri::command]
fn pro_crosshairs() -> serde_json::Value {
    serde_json::from_str(include_str!("../pro-crosshairs.json")).unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
async fn check_updates(handle: AppHandle) -> update::UpdateState {
    update::check(&handle).await
}

#[tauri::command]
async fn install_update(handle: AppHandle) {
    update::install_now(handle).await;
}

#[tauri::command]
fn quit_app(handle: AppHandle) {
    quit(&handle);
}

fn setup(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    desktop_entry::install();
    let path = handle.path().app_config_dir()?.join("settings.json");
    let settings = store::load(&path);

    let (events, received) = channel();
    let platform = platform::start(events).map_err(|unsupported| unsupported.0)?;

    let open = MenuItem::with_id(handle, "open", "Settings", true, None::<&str>)?;
    let lock_item = MenuItem::with_id(handle, "lock", "Unlock position", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(handle, "quit", "Quit Scorpio", true, None::<&str>)?;
    let menu = Menu::with_items(handle, &[&open, &lock_item, &quit_item])?;

    let mut app = App::new(settings.clone(), std::process::id());
    if let Some(display) = primary_display(handle) {
        app.display = display;
    }
    handle.manage(Shared { app: Mutex::new(app), platform, path, hotkeys: Mutex::new(HashMap::new()), lock_item, saving: Mutex::new(()), obs: obs::Obs::default() });

    TrayIconBuilder::with_id("scorpio")
        .icon(handle.default_window_icon().cloned().ok_or("no icon")?)
        .tooltip("Scorpio")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                open_settings(tray.app_handle());
            }
        })
        .on_menu_event(|handle, event| match event.id.as_ref() {
            "open" => open_settings(handle),
            "lock" => {
                let effects = shared(handle).app.lock().unwrap().hotkey(Hotkey::Lock);
                apply(handle, effects);
            }
            "quit" => quit(handle),
            _ => {}
        })
        .build(handle)?;

    handle.manage(update::Updates::default());
    register_hotkeys(handle, &settings);
    sync_autostart(handle, settings.start_with_system);
    update::start(handle);
    if let Some(error) = sync_obs(handle) {
        log::warn!("OBS source not started: {error}");
    }

    // Platform events, one at a time, for as long as Scorpio runs.
    let events_handle = handle.clone();
    std::thread::Builder::new().name("scorpio-events".into()).spawn(move || {
        for event in received {
            let display_check = matches!(event, platform::Event::Foreground(_));
            let display = display_check.then(|| primary_display(&events_handle)).flatten();
            let effects = {
                let state = shared(&events_handle);
                let mut app = state.app.lock().unwrap();
                let mut effects = display.map(|d| app.set_display(d)).unwrap_or_default();
                let step = app.event(event);
                effects.commands.extend(step.commands);
                effects.settings_changed |= step.settings_changed;
                effects.status_changed |= step.status_changed;
                effects
            };
            apply(&events_handle, effects);
            if display_check {
                let (visible, manual) = {
                    let state = shared(&events_handle);
                    let app = state.app.lock().unwrap();
                    (app.focus.visible, app.settings.update_manually)
                };
                update::on_focus(&events_handle, visible, manual);
            }
        }
    })?;

    // Launcher folders are read once, off the main thread.
    let scan_handle = handle.clone();
    std::thread::Builder::new().name("scorpio-launchers".into()).spawn(move || {
        let installed = core::launchers::scan();
        let effects = shared(&scan_handle).app.lock().unwrap().set_installed(installed);
        apply(&scan_handle, effects);
    })?;

    // Show the crosshair state right away (centred, hidden until a game).
    let effects = shared(handle).app.lock().unwrap().set_display(primary_display(handle).unwrap_or_default());
    apply(handle, effects);
    Ok(())
}

pub fn run() {
    // The overlay needs X11; under Wayland the settings window joins it on
    // XWayland so both live on the same display server.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DISPLAY").is_some() {
        std::env::set_var("GDK_BACKEND", "x11");
    }

    // Tauri's async work (the settings window's files and calls, update
    // checks) on one worker thread. Its default runtime starts one per
    // logical processor (20 on the owner's PC, 1.5 MB), and they stay for
    // the rest of the run after the first update check or settings window.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("scorpio-async")
        .enable_all()
        .build()
        .expect("Scorpio's async runtime failed to start");
    tauri::async_runtime::set(runtime.handle().clone());

    let shortcuts = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|handle, shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let state = shared(handle);
            let hotkey = state.hotkeys.lock().unwrap().get(&shortcut.id()).copied();
            if let Some(hotkey) = hotkey {
                let effects = state.app.lock().unwrap().hotkey(hotkey);
                apply(handle, effects);
            }
        })
        .build();

    tauri::Builder::default()
        // The window library would otherwise register for raw input itself;
        // one process gets one raw input target, and it must be Scorpio's own.
        .device_event_filter(tauri::DeviceEventFilter::Always)
        .plugin(tauri_plugin_single_instance::init(|handle, _args, _cwd| open_settings(handle)))
        .plugin(shortcuts)
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            view,
            save_settings,
            preview,
            read_code,
            write_code,
            running_windows,
            add_window,
            set_locked,
            pro_crosshairs,
            check_updates,
            install_update,
            quit_app
        ])
        .setup(|app| {
            setup(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Scorpio failed to start")
        .run(|_, event| {
            // Closing the settings window must not end Scorpio.
            if let RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
