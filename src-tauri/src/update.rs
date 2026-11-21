//! Updates: signed packages from the releases page, checked at start and
//! every few hours. Automatic installs wait until no game is in front, so a
//! restart never lands in the middle of a match. Only Scorpio's own HTTPS
//! request is involved; nothing touches a game.

use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST_CHECK: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available { version: String, notes: Option<String> },
    Downloading { version: String, percent: u8 },
    /// Downloaded and waiting for a moment without a game in front.
    Ready { version: String },
    Installing { version: String },
    Failed { message: String },
    /// This install is updated by something else (a Linux package).
    Unsupported,
}

#[derive(Default)]
pub struct Updates {
    state: Mutex<Option<UpdateState>>,
    found: Mutex<Option<Update>>,
    downloaded: Mutex<Option<(Update, Vec<u8>)>>,
    busy: Mutex<bool>,
}

/// The updater replaces an AppImage or runs the Windows installer; a .deb
/// install belongs to the package manager.
pub fn supported() -> bool {
    !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some()
}

pub fn state(handle: &AppHandle) -> UpdateState {
    let updates = handle.state::<Updates>();
    let current = updates.state.lock().unwrap().clone();
    current.unwrap_or(if supported() { UpdateState::Idle } else { UpdateState::Unsupported })
}

fn set(handle: &AppHandle, state: UpdateState) {
    *handle.state::<Updates>().state.lock().unwrap() = Some(state.clone());
    let _ = handle.emit_to(crate::SETTINGS_WINDOW, "update", &state);
}

/// Looks for a newer version. Returns what it found.
pub async fn check(handle: &AppHandle) -> UpdateState {
    if !supported() {
        return UpdateState::Unsupported;
    }
    set(handle, UpdateState::Checking);
    let result = match handle.updater() {
        Ok(updater) => updater.check().await,
        Err(error) => Err(error),
    };
    let state = match result {
        Ok(Some(update)) => {
            let state = UpdateState::Available { version: update.version.clone(), notes: update.body.clone() };
            *handle.state::<Updates>().found.lock().unwrap() = Some(update);
            state
        }
        Ok(None) => UpdateState::UpToDate,
        Err(error) => UpdateState::Failed { message: plain(&error.to_string()) },
    };
    set(handle, state.clone());
    state
}

fn plain(error: &str) -> String {
    if error.contains("404") || error.to_lowercase().contains("could not fetch") {
        "The update server could not be reached. Try again later.".into()
    } else {
        format!("The update could not be checked: {error}")
    }
}

/// Downloads the update found by `check`, keeping it for `install`.
async fn download(handle: &AppHandle) -> bool {
    let update = handle.state::<Updates>().found.lock().unwrap().clone();
    let Some(update) = update else { return false };
    let version = update.version.clone();
    let progress_handle = handle.clone();
    let progress_version = version.clone();
    let mut received = 0usize;
    let result = update
        .download(
            move |chunk, total| {
                received += chunk;
                if let Some(total) = total.filter(|t| *t > 0) {
                    let percent = ((received as u64 * 100) / total).min(100) as u8;
                    set(&progress_handle, UpdateState::Downloading { version: progress_version.clone(), percent });
                }
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            *handle.state::<Updates>().downloaded.lock().unwrap() = Some((update, bytes));
            set(handle, UpdateState::Ready { version });
            true
        }
        Err(error) => {
            set(handle, UpdateState::Failed { message: format!("The download failed: {error}") });
            false
        }
    }
}

/// Installs a downloaded update and restarts Scorpio. On Windows the
/// installer closes Scorpio itself and starts the new version.
fn install(handle: &AppHandle) {
    let ready = handle.state::<Updates>().downloaded.lock().unwrap().take();
    let Some((update, bytes)) = ready else { return };
    set(handle, UpdateState::Installing { version: update.version.clone() });
    crate::before_exit(handle);
    match update.install(bytes) {
        Ok(()) => handle.restart(),
        Err(error) => set(handle, UpdateState::Failed { message: format!("The update could not be installed: {error}") }),
    }
}

/// The button: check, download and install now (the player is in the
/// settings window, not in a game).
pub async fn install_now(handle: AppHandle) {
    if std::mem::replace(&mut *handle.state::<Updates>().busy.lock().unwrap(), true) {
        return;
    }
    let ready = handle.state::<Updates>().downloaded.lock().unwrap().is_some();
    let ok = ready || (matches!(check(&handle).await, UpdateState::Available { .. }) && download(&handle).await);
    *handle.state::<Updates>().busy.lock().unwrap() = false;
    if ok {
        install(&handle);
    }
}

/// Called whenever the overlay state changes: an automatic update that is
/// ready installs only while no game is in front.
pub fn install_if_idle(handle: &AppHandle, game_in_front: bool, manual: bool) {
    if manual || game_in_front {
        return;
    }
    if handle.state::<Updates>().downloaded.lock().unwrap().is_some() {
        install(handle);
    }
}

/// Starts the background checks on a thread of their own that sleeps
/// between them.
pub fn start(handle: &AppHandle) {
    if !supported() {
        return;
    }
    let handle = handle.clone();
    let spawned = std::thread::Builder::new().name("scorpio-updates".into()).spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            let manual = crate::settings_snapshot(&handle).update_manually;
            let busy = *handle.state::<Updates>().busy.lock().unwrap();
            if !manual && !busy {
                tauri::async_runtime::block_on(async {
                    if let UpdateState::Available { .. } = check(&handle).await {
                        if download(&handle).await {
                            install_if_idle(&handle, crate::game_in_front(&handle), false);
                        }
                    }
                });
            }
            std::thread::sleep(EVERY);
        }
    });
    if let Err(error) = spawned {
        log::warn!("update checks not started: {error}");
    }
}
