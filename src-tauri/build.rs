fn main() {
    // Tauri's default manifest plus the segment heap (Windows 10 2004 and
    // later), which keeps 0.1 to 0.2 MB less at idle than the old heap.
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("windows-manifest.xml"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri-build failed");
}
