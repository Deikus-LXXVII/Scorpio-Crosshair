pub mod core;
pub mod platform;

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("Scorpio failed to start");
}
