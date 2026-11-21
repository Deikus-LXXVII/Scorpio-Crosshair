//! Games installed through launchers, found in the launchers' own files.
//! Reference behaviour: `reference/js/launchers.js` and its tests.
//! Parsers are pure; `scan()` reads the disk (files and, on Windows, `reg.exe query`).

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InstalledGame {
    /// "Steam", "Epic", "GOG", "Ubisoft", "Xbox", "Riot", "EA", "Battle.net", "Heroic", "Lutris".
    pub source: String,
    pub name: String,
    /// Install folder as a prefix: lower case, '/' separators, trailing '/'.
    pub key: String,
}

pub fn folder_key(folder: &str) -> String {
    todo!()
}

/// The installed game an executable belongs to: the deepest folder holding it.
pub fn game_at<'a>(executable: &str, games: &'a [InstalledGame]) -> Option<&'a InstalledGame> {
    todo!()
}

/// Every installed game Scorpio can find on this machine.
pub fn scan() -> Vec<InstalledGame> {
    todo!()
}
