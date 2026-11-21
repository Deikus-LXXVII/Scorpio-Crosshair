//! Updates: signed packages from the releases page, checked at start and
//! every few hours. An automatic update is downloaded and installed only
//! after no game has been in front for a few minutes, so a restart never
//! lands in a match and the package never waits in memory. Only Scorpio's
//! own HTTPS request is involved; nothing touches a game.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST_CHECK: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How long no game must be in front before an automatic install.
const QUIET: Duration = Duration::from_secs(3 * 60);

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available { version: String, notes: Option<String> },
    Downloading { version: String, percent: u8 },
    Installing { version: String },
    Failed { message: String },
    /// This install is updated by something else (a Linux package).
    Unsupported,
}

#[derive(Default)]
pub struct Updates {
    state: Mutex<Option<UpdateState>>,
    found: Mutex<Option<Update>>,
    busy: AtomicBool,
    /// Counts the times a game came to the front; a quiet wait that sees it
    /// change was interrupted by play.
    games_seen: AtomicU64,
    waiting: AtomicBool,
    /// The bundled root certificates failed where the system's worked.
    system_roots: AtomicBool,
}

/// The updater replaces an AppImage or runs the Windows installer; a .deb
/// install belongs to the package manager.
pub fn supported() -> bool {
    !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some()
}

pub fn state(handle: &AppHandle) -> UpdateState {
    let current = handle.state::<Updates>().state.lock().unwrap().clone();
    current.unwrap_or(if supported() { UpdateState::Idle } else { UpdateState::Unsupported })
}

fn set(handle: &AppHandle, state: UpdateState) {
    *handle.state::<Updates>().state.lock().unwrap() = Some(state.clone());
    let _ = handle.emit_to(crate::SETTINGS_WINDOW, "update", &state);
}

fn plain(error: &str) -> String {
    if error.contains("404") || error.to_lowercase().contains("could not fetch") {
        "The update server could not be reached. Try again later.".into()
    } else {
        format!("The update could not be checked: {error}")
    }
}

/// Mozilla's root certificates, bundled with Scorpio.
fn bundled_roots() -> Vec<reqwest::Certificate> {
    let roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS.iter();
    roots.filter_map(|der| reqwest::Certificate::from_der(der).ok()).collect()
}

/// Asks for the latest release. With the bundled root certificates unless
/// `system_roots`: checking a server with those is plain Rust, while the
/// system's go through Windows' certificate chain engine, which keeps about
/// 1.5 MB and four threads for the rest of the run.
async fn ask(handle: &AppHandle, system_roots: bool) -> tauri_plugin_updater::Result<Option<Update>> {
    let updater = if system_roots {
        handle.updater()?
    } else {
        let bundled = |client: reqwest::ClientBuilder| client.tls_certs_only(bundled_roots());
        handle.updater_builder().configure_client(bundled).build()?
    };
    updater.check().await
}

/// The bundled roots first. If they fail where the system's work (an
/// antivirus or a company proxy that re-signs HTTPS with its own root), the
/// system's are used from then on.
async fn latest(handle: &AppHandle) -> tauri_plugin_updater::Result<Option<Update>> {
    let updates = handle.state::<Updates>();
    if updates.system_roots.load(Ordering::SeqCst) {
        return ask(handle, true).await;
    }
    let bundled = ask(handle, false).await;
    if bundled.is_ok() {
        return bundled;
    }
    let system = ask(handle, true).await;
    if system.is_ok() {
        updates.system_roots.store(true, Ordering::SeqCst);
    }
    system
}

/// Looks for a newer version and remembers it.
pub async fn check(handle: &AppHandle) -> UpdateState {
    if !supported() {
        return UpdateState::Unsupported;
    }
    set(handle, UpdateState::Checking);
    let state = match latest(handle).await {
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

async fn download(handle: &AppHandle, update: &Update) -> tauri_plugin_updater::Result<Vec<u8>> {
    let progress = handle.clone();
    let version = update.version.clone();
    let mut received = 0usize;
    let on_chunk = move |chunk, total: Option<u64>| {
        received += chunk;
        if let Some(total) = total.filter(|t| *t > 0) {
            let percent = ((received as u64 * 100) / total).min(100) as u8;
            set(&progress, UpdateState::Downloading { version: version.clone(), percent });
        }
    };
    update.download(on_chunk, || {}).await
}

/// Downloads the update found by `check` and installs it; Scorpio then
/// restarts (on Windows the installer closes it and starts the new one).
async fn download_and_install(handle: &AppHandle) {
    let updates = handle.state::<Updates>();
    let found = updates.found.lock().unwrap().clone();
    let Some(mut update) = found else { return };
    let mut bytes = download(handle, &update).await;
    if bytes.is_err() && !updates.system_roots.load(Ordering::SeqCst) {
        // Found with the bundled roots; the download host may need the system's.
        if let Ok(Some(again)) = ask(handle, true).await {
            let retried = download(handle, &again).await;
            if retried.is_ok() {
                updates.system_roots.store(true, Ordering::SeqCst);
                (update, bytes) = (again, retried);
            }
        }
    }
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(error) => return set(handle, UpdateState::Failed { message: format!("The download failed: {error}") }),
    };
    set(handle, UpdateState::Installing { version: update.version.clone() });
    crate::before_exit(handle);
    match update.install(bytes) {
        Ok(()) => handle.restart(),
        Err(error) => set(handle, UpdateState::Failed { message: format!("The update could not be installed: {error}") }),
    }
}

/// Runs `work` unless an update is already being checked or installed.
async fn exclusive(handle: &AppHandle, work: impl std::future::Future<Output = ()>) {
    let updates = handle.state::<Updates>();
    if updates.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    work.await;
    updates.busy.store(false, Ordering::SeqCst);
}

/// The button: the player is in the settings window, not in a game.
pub async fn install_now(handle: AppHandle) {
    let work = async {
        let found = handle.state::<Updates>().found.lock().unwrap().is_some();
        if found || matches!(check(&handle).await, UpdateState::Available { .. }) {
            download_and_install(&handle).await;
        }
    };
    exclusive(&handle, work).await;
}

/// Called after every change of the window in front. Once no game has been
/// in front for `QUIET`, a waiting automatic update installs.
pub fn on_focus(handle: &AppHandle, game_in_front: bool, manual: bool) {
    let updates = handle.state::<Updates>();
    if game_in_front {
        updates.games_seen.fetch_add(1, Ordering::SeqCst);
        return;
    }
    let found = updates.found.lock().unwrap().is_some();
    if manual || !found || updates.waiting.swap(true, Ordering::SeqCst) {
        return;
    }
    let seen = updates.games_seen.load(Ordering::SeqCst);
    let handle = handle.clone();
    let spawned = std::thread::Builder::new().name("scorpio-update-wait".into()).spawn(move || {
        std::thread::sleep(QUIET);
        let updates = handle.state::<Updates>();
        updates.waiting.store(false, Ordering::SeqCst);
        let quiet = updates.games_seen.load(Ordering::SeqCst) == seen && !crate::game_in_front(&handle);
        if quiet && !crate::settings_snapshot(&handle).update_manually {
            tauri::async_runtime::block_on(exclusive(&handle, download_and_install(&handle)));
        }
    });
    if spawned.is_err() {
        updates.waiting.store(false, Ordering::SeqCst);
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
            let already = handle.state::<Updates>().found.lock().unwrap().is_some();
            if !manual && !already {
                tauri::async_runtime::block_on(exclusive(&handle, async {
                    check(&handle).await;
                }));
                on_focus(&handle, crate::game_in_front(&handle), manual);
            }
            std::thread::sleep(EVERY);
        }
    });
    if let Err(error) = spawned {
        log::warn!("update checks not started: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_root_is_read() {
        // A root that fails to parse is dropped silently; all of them failing
        // would send every check to the system's roots and their cost.
        let roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS;
        assert!(roots.len() > 100);
        assert_eq!(bundled_roots().len(), roots.len());
    }
}
