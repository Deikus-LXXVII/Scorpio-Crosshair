pub mod app;
pub mod core;
pub mod logging;
#[cfg(target_os = "linux")]
mod desktop_entry;
pub mod platform;
pub mod obs;
pub mod picture;
pub mod report;
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
use std::sync::{Arc, Mutex};
use std::time::Duration;
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
    /// The player's image files (`picture`).
    images: PathBuf,
    /// Registered shortcut id -> what it does.
    hotkeys: Mutex<HashMap<u32, Hotkey>>,
    lock_item: MenuItem<tauri::Wry>,
    /// Held from reading the settings to writing them, so two threads saving
    /// at once can't write an older copy last.
    saving: Mutex<()>,
    obs: obs::Obs,
    /// The settings were read from the file, or there was none. False when
    /// defaults stand in for a file that could not be read: then no image
    /// is removed for this whole run, since the settings naming it are lost.
    trusted: bool,
    /// When this run started reading the settings, to find its own copy.
    started: u64,
}

fn shared(handle: &AppHandle) -> tauri::State<'_, Shared> {
    handle.state::<Shared>()
}

/// What is left of an app step once the overlay and the stream were told.
struct Rest {
    settings_changed: bool,
    status_changed: bool,
}

/// Sends an app step's overlay commands and the stream's view. Called with
/// the app lock held, so steps taken on two threads reach the overlay and
/// OBS in the order the app decided them.
fn deliver(state: &Shared, app: &App, effects: Effects) -> Rest {
    let overlay_changed = !effects.commands.is_empty();
    for command in effects.commands {
        state.platform.send(command);
    }
    if overlay_changed || effects.settings_changed {
        state.obs.set_picture(app.stream_png());
        state.obs.publish(serde_json::to_string(&app.stream_view()).unwrap_or_default());
    }
    Rest { settings_changed: effects.settings_changed, status_changed: effects.status_changed }
}

/// Saving and telling the settings window, after the app lock is let go.
fn finish(handle: &AppHandle, rest: Rest) {
    let state = shared(handle);
    if rest.settings_changed {
        let settings = persist(handle);
        let _ = handle.emit_to(SETTINGS_WINDOW, "settings", &settings);
    }
    if rest.status_changed || rest.settings_changed {
        let status = state.app.lock().unwrap().status();
        let _ = state.lock_item.set_text(if status.locked { "Unlock position" } else { "Lock position" });
        let _ = handle.emit_to(SETTINGS_WINDOW, "status", &status);
    }
}

/// Takes one app step and carries out what it decided.
fn step(handle: &AppHandle, decide: impl FnOnce(&mut App) -> Effects) {
    let state = shared(handle);
    let rest = {
        let mut app = state.app.lock().unwrap();
        let effects = decide(&mut app);
        deliver(&state, &app, effects)
    };
    finish(handle, rest);
}

/// Writes the current settings and returns what was written.
fn persist(handle: &AppHandle) -> Settings {
    let state = shared(handle);
    let _saving = state.saving.lock().unwrap();
    let settings = state.app.lock().unwrap().settings.clone();
    // Defaults stand in for a file that could not be used: never write over
    // it before a copy of it is kept aside, or what it held is gone.
    if !state.trusted && state.path.exists() && !store::kept_aside(&state.path, state.started) {
        if let Err(error) = store::keep_aside(&state.path) {
            log::error!("settings not saved: the unreadable file could not be kept aside first ({error})");
            return settings;
        }
    }
    if let Err(error) = store::save(&state.path, &settings) {
        log::error!("could not save settings: {error}");
    }
    settings
}

/// Removes image files older than `age` that neither the settings nor a
/// kept-aside copy of unreadable settings name.
fn tidy_images(images: &std::path::Path, path: &std::path::Path, settings: &Settings, age: Duration) {
    let mut used = store::images_kept_aside(path);
    used.extend(settings.image_files().into_iter().map(str::to_string));
    picture::prune(images, &used, age);
}

/// Reads the images the settings use that are not decoded yet. A file that
/// is gone or broken is logged; the CS2 crosshair shows instead.
fn load_pictures(handle: &AppHandle) {
    let state = shared(handle);
    let missing = state.app.lock().unwrap().missing_pictures();
    for name in missing {
        let decoded = picture::read(&state.images, &name).map_err(|error| error.to_string()).and_then(picture::decode);
        match decoded {
            Ok(decoded) => {
                step(handle, |app| app.add_picture(name, Arc::new(decoded)));
            }
            Err(error) => log::warn!("image {name} not shown: {error}"),
        }
    }
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
    if !refused.is_empty() {
        log::warn!("shortcuts refused by the system: {}", refused.join(", "));
    }
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
/// A game may be in front: one is, or Always show is on, which draws over
/// games Scorpio does not recognise, so an automatic update must wait.
pub fn game_in_front(handle: &AppHandle) -> bool {
    shared(handle).app.lock().unwrap().maybe_playing()
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
    if let Some(error) = &error {
        log::warn!("OBS source could not start: {error}");
    }
    if error.is_some() {
        state.app.lock().unwrap().settings.obs = false;
    }
    error
}

/// Saves new settings and returns them validated.
#[tauri::command]
fn save_settings(handle: AppHandle, settings: Settings) -> Saved {
    let state = shared(&handle);
    let (rest, keys_changed, settings) = {
        let mut app = state.app.lock().unwrap();
        let keys_changed = app.settings.keys != settings.keys;
        let effects = app.replace_settings(settings);
        // The window already has these settings: saved below, not echoed back.
        let effects = Effects { settings_changed: false, status_changed: true, ..effects };
        (deliver(&state, &app, effects), keys_changed, app.settings.clone())
    };
    let refused = if keys_changed { register_hotkeys(&handle, &settings) } else { Vec::new() };
    sync_autostart(&handle, settings.start_with_system);
    let obs_error = sync_obs(&handle);
    let settings = persist(&handle);
    if state.trusted {
        // Under the save lock, which storing an image takes too, and older
        // than a minute: a file just kept for a setting on its way stays.
        let _saving = state.saving.lock().unwrap();
        tidy_images(&state.images, &state.path, &settings, Duration::from_secs(60));
    }
    finish(&handle, rest);
    load_pictures(&handle);
    Saved { settings, refused, obs_error }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredImage {
    /// Scorpio's name for the file, for the settings.
    name: String,
    /// The bitmap's size as shown (a rendered image only).
    width: u32,
    height: u32,
}

/// Keeps a file the settings window sends as raw bytes. The `x-image`
/// header says what it is: the kind of an original the player chose
/// (`svg`, `png`...), or `render` for the PNG made from it, which is checked
/// by decoding it here and shown as soon as the settings point at it.
#[tauri::command]
async fn store_image(handle: AppHandle, request: tauri::ipc::Request<'_>) -> Result<StoredImage, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else { return Err("No image was sent.".into()) };
    let kind = request.headers().get("x-image").and_then(|value| value.to_str().ok()).unwrap_or_default().to_string();
    if bytes.len() > picture::FILE_LIMIT {
        return Err("The file is larger than 5 MB. Choose a smaller one.".into());
    }
    let state = shared(&handle);
    let keep = |bytes: &[u8], kind: &str| {
        let _saving = state.saving.lock().unwrap();
        picture::store(&state.images, bytes, kind).map_err(|error| {
            log::error!("image could not be kept: {error}");
            format!("The image could not be kept: {error}")
        })
    };
    if kind == "render" {
        let decoded = picture::decode(bytes.clone())?;
        let (width, height) = (decoded.width, decoded.height);
        let name = keep(bytes, "png")?;
        log::info!("image rendered: {width}x{height}");
        step(&handle, |app| app.add_picture(name.clone(), Arc::new(decoded)));
        return Ok(StoredImage { name, width, height });
    }
    let kind = picture::kind_of(&format!("file.{kind}")).ok_or("Scorpio takes PNG, JPG, WebP, GIF and SVG images.")?;
    let name = keep(bytes, kind)?;
    log::info!("image chosen: {kind}, {} bytes", bytes.len());
    Ok(StoredImage { name, width: 0, height: 0 })
}

/// A kept image file, as raw bytes, to draw it again at a new size.
#[tauri::command]
async fn image_file(handle: AppHandle, name: String) -> Result<tauri::ipc::Response, String> {
    picture::read(&shared(&handle).images, &name)
        .map(tauri::ipc::Response::new)
        .map_err(|_| "The image file is missing. Choose the image again.".to_string())
}

/// The problem report, for the player to read before anything leaves.
#[tauri::command]
async fn problem_report(handle: AppHandle) -> String {
    let (settings, state) = {
        let shared = shared(&handle);
        let app = shared.app.lock().unwrap();
        let status = app.status();
        let display = app.target_area();
        let state = format!(
            "Crosshair: {}; locked: {}; hidden by shortcut: {}; always show: {}\n\
             Centred on: {}x{} at {},{}; primary display {}x{}\n\
             Window in front: {}\n\
             Launcher games found: {}; OBS source: {}",
            if status.visible || status.always { "shown" } else { "hidden" },
            status.locked,
            status.hidden,
            status.always,
            display.width,
            display.height,
            display.x,
            display.y,
            app.display.width,
            app.display.height,
            status.front.map_or("none".to_string(), |w| format!("{} (class {}, fullscreen {})", w.name, w.window_class, w.fullscreen)),
            app.installed.len(),
            if shared.obs.enabled() { "on" } else { "off" },
        );
        (app.settings.clone(), state)
    };
    report::text(&report::gather(state, Ok(settings)))
}

/// Saves the report the player read, and says where.
#[tauri::command]
async fn save_report(text: String) -> Result<String, String> {
    if text.len() > report::TEXT_LIMIT {
        return Err("The report is too long to save.".into());
    }
    let path = report::save(&text).map_err(|error| format!("The report could not be saved: {error}"))?;
    log::info!("problem report saved");
    Ok(path.to_string_lossy().into_owned())
}

/// Opens a new GitHub issue in the browser, prefilled with a short body that
/// asks for the saved file. The address is made here, never taken from the
/// web view.
#[tauri::command]
fn open_issue(file: String) -> Result<(), String> {
    let file: String = file.rsplit(['/', '\\']).next().unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.')).take(64).collect();
    let file = if file.starts_with("scorpio-report-") && file.ends_with(".txt") { file } else { "scorpio-report.txt".into() };
    platform::open_url(&report::issue_url(env!("CARGO_PKG_VERSION"), &report::system(), &file))
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
        finish(&handle, Rest { settings_changed: true, status_changed: true });
    }
    added
}

#[tauri::command]
fn set_locked(handle: AppHandle, locked: bool) {
    step(&handle, |app| app.set_locked(locked));
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
    let config = handle.path().app_config_dir()?;
    let path = config.join("settings.json");
    let started = store::now_seconds();
    let (settings, trusted) = store::load(&path);
    let images = picture::folder(&config);
    if trusted {
        tidy_images(&images, &path, &settings, Duration::ZERO);
    }

    let (events, received) = channel();
    let platform = platform::start(events).map_err(|unsupported| {
        log::error!("platform could not start: {}", unsupported.0);
        unsupported.0
    })?;
    log::info!("platform started");

    let open = MenuItem::with_id(handle, "open", "Settings", true, None::<&str>)?;
    let lock_item = MenuItem::with_id(handle, "lock", "Unlock position", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(handle, "quit", "Quit Scorpio", true, None::<&str>)?;
    let menu = Menu::with_items(handle, &[&open, &lock_item, &quit_item])?;

    let mut app = App::new(settings.clone(), std::process::id());
    if let Some(display) = primary_display(handle) {
        app.display = display;
    }
    handle.manage(Shared { app: Mutex::new(app), platform, path, images, hotkeys: Mutex::new(HashMap::new()), lock_item, saving: Mutex::new(()), obs: obs::Obs::default(), trusted, started });

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
                step(handle, |app| app.hotkey(Hotkey::Lock));
            }
            "quit" => quit(handle),
            _ => {}
        })
        .build(handle)?;

    handle.manage(update::Updates::default());
    register_hotkeys(handle, &settings);
    sync_autostart(handle, settings.start_with_system);
    update::start(handle);
    sync_obs(handle);

    // Platform events, one at a time, for as long as Scorpio runs.
    let events_handle = handle.clone();
    std::thread::Builder::new().name("scorpio-events".into()).spawn(move || {
        for event in received {
            let display_check = matches!(event, platform::Event::Foreground(_));
            let display = display_check.then(|| primary_display(&events_handle)).flatten();
            step(&events_handle, |app| {
                let mut effects = display.map(|d| app.set_display(d)).unwrap_or_default();
                let next = app.event(event);
                effects.commands.extend(next.commands);
                effects.settings_changed |= next.settings_changed;
                effects.status_changed |= next.status_changed;
                effects
            });
            if display_check {
                let (visible, manual) = {
                    let state = shared(&events_handle);
                    let app = state.app.lock().unwrap();
                    (app.maybe_playing(), app.settings.update_manually)
                };
                update::on_focus(&events_handle, visible, manual);
            }
        }
    })?;

    // Launcher folders are read once, off the main thread.
    let scan_handle = handle.clone();
    std::thread::Builder::new().name("scorpio-launchers".into()).spawn(move || {
        let installed = core::launchers::scan();
        step(&scan_handle, |app| app.set_installed(installed));
    })?;

    // Show the crosshair state right away (centred, hidden until a game).
    let display = primary_display(handle).unwrap_or_default();
    step(handle, |app| app.set_display(display));
    load_pictures(handle);
    Ok(())
}

pub fn run() {
    // `scorpio --report` prints the problem report and ends; nothing starts.
    if std::env::args_os().skip(1).any(|arg| arg == "--report") {
        platform::attach_console();
        println!("{}", report::from_files());
        return;
    }
    if let Some(folder) = logging::paths::log_dir() {
        logging::start(&folder);
    }
    log::info!("Scorpio {} starting on {}", env!("CARGO_PKG_VERSION"), report::system());

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
                step(handle, |app| app.hotkey(hotkey));
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
            quit_app,
            store_image,
            image_file,
            problem_report,
            save_report,
            open_issue
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
