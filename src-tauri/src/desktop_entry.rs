//! On Linux, an AppImage is just a file: nothing tells the desktop it is
//! there, so the app menu and its search can't find Scorpio. When run as an
//! AppImage, Scorpio writes its own desktop entry and icon into the user's
//! data folder (the .deb package installs its own entry instead).

use std::fs;
use std::path::{Path, PathBuf};

const ICON: &[u8] = include_bytes!("../icons/128x128@2x.png");

pub fn install() {
    let Some(appimage) = std::env::var_os("APPIMAGE").map(PathBuf::from) else { return };
    let Some(data) = data_home() else { return };
    if let Err(error) = write(&data, &appimage) {
        log::warn!("desktop entry not written: {error}");
    }
}

fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")))
}

/// The entry, pointing at the AppImage wherever it is now. Written only when
/// it changed, so a normal start touches nothing.
fn entry(appimage: &Path) -> String {
    let exec = appimage.display().to_string().replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "[Desktop Entry]\nType=Application\nName=Scorpio\nGenericName=Crosshair Overlay\n\
         Comment=A CS2-style crosshair that shows only while your game is in front\n\
         Exec=\"{exec}\"\nIcon=scorpio\nTerminal=false\nCategories=Utility;Game;\n\
         Keywords=crosshair;overlay;cs2;counter-strike;reticle;aim;scorpio;\nStartupWMClass=scorpio\n"
    )
}

fn write(data: &Path, appimage: &Path) -> std::io::Result<()> {
    let desktop = data.join("applications/scorpio.desktop");
    let icon = data.join("icons/hicolor/256x256/apps/scorpio.png");
    let text = entry(appimage);
    if fs::read_to_string(&desktop).ok().as_deref() != Some(text.as_str()) {
        fs::create_dir_all(desktop.parent().expect("has a parent"))?;
        fs::write(&desktop, text)?;
    }
    if fs::metadata(&icon).map(|m| m.len()).ok() != Some(ICON.len() as u64) {
        fs::create_dir_all(icon.parent().expect("has a parent"))?;
        fs::write(&icon, ICON)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_searchable_entry_once_and_follows_a_moved_appimage() {
        let data = std::env::temp_dir().join(format!("scorpio-desktop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data);
        write(&data, Path::new("/home/p/Apps/Scorpio.AppImage")).unwrap();
        let text = fs::read_to_string(data.join("applications/scorpio.desktop")).unwrap();
        assert!(text.contains("Exec=\"/home/p/Apps/Scorpio.AppImage\"") && text.contains("Keywords=crosshair;"));
        assert!(data.join("icons/hicolor/256x256/apps/scorpio.png").exists());
        write(&data, Path::new("/home/p/Games/Scorpio.AppImage")).unwrap();
        assert!(fs::read_to_string(data.join("applications/scorpio.desktop")).unwrap().contains("/home/p/Games/"));
    }
}
