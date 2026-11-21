//! Games installed through launchers, found in the launchers' own files.
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

/// A folder as a comparable prefix: lower case, one separator style, trailing separator.
pub fn folder_key(folder: &str) -> String {
    let key = folder.trim().replace('\\', "/").trim_end_matches('/').to_lowercase();
    if key.is_empty() { key } else { key + "/" }
}

/// The installed game an executable belongs to: the deepest folder holding it.
pub fn game_at<'a>(executable: &str, games: &'a [InstalledGame]) -> Option<&'a InstalledGame> {
    let file = executable.trim().replace('\\', "/").to_lowercase();
    if file.is_empty() {
        return None;
    }
    games.iter().filter(|game| !game.key.is_empty() && file.starts_with(&game.key)).max_by_key(|game| game.key.len())
}

/// Every installed game Scorpio can find on this machine.
pub fn scan() -> Vec<InstalledGame> {
    let found = if cfg!(windows) {
        windows_scan()
    } else if cfg!(target_os = "linux") {
        linux_scan()
    } else {
        Vec::new()
    };
    unique(found)
}

/// The first game found for each folder; games without a folder dropped.
fn unique(found: Vec<InstalledGame>) -> Vec<InstalledGame> {
    let mut seen = std::collections::HashSet::new();
    found.into_iter().filter(|game| !game.key.is_empty() && seen.insert(game.key.clone())).collect()
}

fn game(source: &str, name: &str, folder: &str) -> InstalledGame {
    InstalledGame { source: source.to_owned(), name: name.trim().to_owned(), key: folder_key(folder) }
}

fn last_part(folder: &str) -> &str {
    folder.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("")
}

/// Steam installs tools and runtimes as apps too.
const NOT_GAMES: [&str; 14] = [
    "steamworks shared",
    "steamworks common redistributables",
    "wallpaper engine",
    "wallpaper_engine",
    "lossless scaling",
    "borderless gaming",
    "steamvr",
    "steam linux runtime",
    "steam linux runtime - soldier",
    "steam linux runtime - sniper",
    "proton experimental",
    "proton hotfix",
    "proton easyanticheat runtime",
    "proton battleye runtime",
];

fn is_tool(name: &str) -> bool {
    let name = name.trim().to_lowercase();
    let proton_version = name.strip_prefix("proton ").is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()));
    NOT_GAMES.contains(&name.as_str()) || proton_version
}

/* Parsers */

/// Every value stored under `key` (any case) in a Valve KeyValues text.
fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut pending: Option<String> = None;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let token = vdf_string(&mut chars);
                match pending.take() {
                    Some(k) if k.eq_ignore_ascii_case(key) => values.push(token),
                    Some(_) => {}
                    None => pending = Some(token),
                }
            }
            '{' | '}' => pending = None,
            _ => {}
        }
    }
    values
}

/// The rest of a quoted string, with `\\` and `\"` unescaped.
fn vdf_string(chars: &mut std::str::Chars) -> String {
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => match chars.next() {
                Some(next @ ('\\' | '"')) => out.push(next),
                Some(other) => out.extend(['\\', other]),
                None => out.push('\\'),
            },
            _ => out.push(c),
        }
    }
    out
}

fn steam_libraries(vdf: &str) -> Vec<String> {
    vdf_values(vdf, "path")
}

fn steam_game(acf: &str, library: &str) -> Option<InstalledGame> {
    let name = vdf_values(acf, "name").into_iter().next().unwrap_or_default();
    let dir = vdf_values(acf, "installdir").into_iter().next().unwrap_or_default();
    if dir.is_empty() || is_tool(&name) || is_tool(&dir) {
        return None;
    }
    let shown = if name.is_empty() { &dir } else { &name };
    Some(game("Steam", shown, &format!("{library}/steamapps/common/{dir}")))
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value.get(key).and_then(serde_json::Value::as_str).unwrap_or("")
}

fn epic_game(text: &str) -> Option<InstalledGame> {
    let item: serde_json::Value = serde_json::from_str(text).ok()?;
    let folder = json_str(&item, "InstallLocation");
    let name = json_str(&item, "DisplayName");
    let categories = item.get("AppCategories").and_then(serde_json::Value::as_array);
    let is_game = categories.is_some_and(|list| list.iter().any(|c| c.as_str() == Some("games")));
    if folder.is_empty() || !is_game || is_tool(name) {
        return None;
    }
    Some(game("Epic", if name.is_empty() { last_part(folder) } else { name }, folder))
}

/// `reg query <key> /s /v <value>` output: one folder per subkey.
fn registry_folders(text: &str, value: &str, source: &str) -> Vec<InstalledGame> {
    text.lines()
        .filter_map(|line| registry_value(line, value))
        .filter(|folder| !is_tool(last_part(folder)))
        .map(|folder| game(source, last_part(folder), folder))
        .collect()
}

/// "    <value>    REG_SZ    <data>" -> data.
fn registry_value<'a>(line: &'a str, value: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(char::is_whitespace)?.trim_start();
    let after_name = strip_word(rest, value)?;
    let data = strip_word(after_name, "REG_SZ")?.trim();
    (!data.is_empty()).then_some(data)
}

/// `text` without a leading `word` (any case) and the whitespace after it.
fn strip_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let head = text.get(..word.len())?;
    let rest = &text[word.len()..];
    (head.eq_ignore_ascii_case(word) && rest.starts_with(char::is_whitespace)).then(|| rest.trim_start())
}

/// Battle.net's product.db is protobuf with the install folders as plain text;
/// it also names their parents and its own folders, which are not games.
fn battle_net_games(db: &[u8]) -> Vec<InstalledGame> {
    let mut found: Vec<(String, String)> = Vec::new();
    for folder in printable_paths(db) {
        let key = folder_key(&folder);
        if !key.contains("battle.net") && !key.contains("programdata") && !is_bare_root(&key) && !found.iter().any(|(k, _)| *k == key) {
            found.push((key, last_part(&folder).to_owned()));
        }
    }
    let keys: Vec<String> = found.iter().map(|(k, _)| k.clone()).collect();
    found
        .into_iter()
        .filter(|(key, _)| !keys.iter().any(|other| other != key && other.starts_with(key.as_str())))
        .map(|(key, name)| InstalledGame { source: "Battle.net".to_owned(), name, key })
        .collect()
}

/// A drive, or one of the shared folders on it, that holds many programs.
fn is_bare_root(key: &str) -> bool {
    let rest = match key.as_bytes() {
        [drive, b':', b'/', ..] if drive.is_ascii_alphabetic() => &key[3..],
        _ => return false,
    };
    ["", "program files/", "program files (x86)/", "games/", "users/", "windows/"].contains(&rest)
}

/// Runs like `C:\x...` or `d:/x...`: a drive, a separator, then 2 to 200
/// printable ASCII characters up to the next unprintable byte.
fn printable_paths(db: &[u8]) -> Vec<String> {
    let printable = |b: u8| (0x20..=0x7e).contains(&b);
    let mut paths = Vec::new();
    let mut i = 0;
    while i + 3 <= db.len() {
        let starts = db[i].is_ascii_alphabetic() && db[i + 1] == b':' && matches!(db[i + 2], b'\\' | b'/');
        let end = (i + 3..db.len()).find(|&j| !printable(db[j])).unwrap_or(db.len());
        if starts && (2..=200).contains(&(end - i - 3)) {
            paths.push(String::from_utf8_lossy(&db[i..end]).into_owned());
            i = end;
        } else {
            i += 1;
        }
    }
    paths
}

/// X:\.GamingRoot: "RGBX", a little-endian u32 count, then UTF-16 folder names on that drive.
fn xbox_folders(bytes: &[u8]) -> Vec<String> {
    if bytes.len() < 8 || &bytes[..4] != b"RGBX" {
        return Vec::new();
    }
    let count = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let units: Vec<u16> = bytes[8..].chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
    String::from_utf16_lossy(&units)
        .split('\0')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .take(count)
        .map(str::to_owned)
        .collect()
}

/// RiotClientInstalls.json: each game's folder is a key of associated_client.
fn riot_games(text: &str) -> Vec<InstalledGame> {
    let Ok(data) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(clients) = data.get("associated_client").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    clients
        .keys()
        .filter_map(|folder| {
            let parts: Vec<&str> = folder.split(['/', '\\']).filter(|part| !part.is_empty()).collect();
            let last = *parts.last()?;
            let is_channel = last.eq_ignore_ascii_case("live") || last.eq_ignore_ascii_case("pbe");
            let name = if is_channel { parts.len().checked_sub(2).map(|i| parts[i])? } else { last };
            Some(game("Riot", name, folder))
        })
        .collect()
}

/// Heroic (Linux): legendaryConfig/installed.json and gog_store/installed.json.
fn heroic_games(text: &str, source: &str) -> Vec<InstalledGame> {
    let Ok(data) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let entries: Vec<&serde_json::Value> = match (data.get("installed").and_then(serde_json::Value::as_array), data.as_object()) {
        (Some(list), _) => list.iter().collect(),
        (None, Some(map)) => map.values().collect(),
        _ => Vec::new(),
    };
    entries
        .into_iter()
        .filter_map(|entry| {
            let folder = json_str(entry, "install_path");
            let name = [json_str(entry, "title"), json_str(entry, "app_name"), last_part(folder)].into_iter().find(|n| !n.is_empty()).unwrap_or("");
            (!folder.is_empty() && !is_tool(name)).then(|| game(source, name, folder))
        })
        .collect()
}

/* Reading the disk: only the launchers' own files, never another process. */

use std::path::{Path, PathBuf};

fn read_text(file: &Path) -> String {
    std::fs::read_to_string(file).unwrap_or_default()
}

fn entries(dir: &Path, folders: bool) -> Vec<String> {
    let Ok(listing) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    listing
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir() == folders))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

fn is_app_manifest(name: &str) -> bool {
    let lower = name.to_lowercase();
    let id = lower.strip_prefix("appmanifest_").and_then(|rest| rest.strip_suffix(".acf"));
    id.is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
}

fn steam_from(roots: &[PathBuf]) -> Vec<InstalledGame> {
    let mut libraries: Vec<(String, PathBuf)> = Vec::new();
    for root in roots {
        let listed = steam_libraries(&read_text(&root.join("steamapps").join("libraryfolders.vdf")));
        for library in std::iter::once(root.clone()).chain(listed.into_iter().map(PathBuf::from)) {
            let key = folder_key(&library.to_string_lossy());
            if !libraries.iter().any(|(k, _)| *k == key) {
                libraries.push((key, library));
            }
        }
    }
    libraries
        .iter()
        .flat_map(|(_, library)| {
            let steamapps = library.join("steamapps");
            entries(&steamapps, false)
                .into_iter()
                .filter(|name| is_app_manifest(name))
                .filter_map(move |name| steam_game(&read_text(&steamapps.join(name)), &library.to_string_lossy()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn subfolder_games(source: &str, parents: &[PathBuf], skip: &[&str]) -> Vec<InstalledGame> {
    parents
        .iter()
        .flat_map(|parent| {
            entries(parent, true)
                .into_iter()
                .filter(|name| !skip.contains(&name.to_lowercase().as_str()) && !is_tool(name))
                .map(|name| game(source, &name, &parent.join(&name).to_string_lossy()))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(windows)]
fn reg_query(args: &[&str]) -> String {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("reg.exe")
        .arg("query")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn reg_query(_args: &[&str]) -> String {
    String::new()
}

const DRIVES: [char; 6] = ['C', 'D', 'E', 'F', 'G', 'H'];

fn on_drives(folder: &str) -> Vec<PathBuf> {
    DRIVES.iter().map(|drive| PathBuf::from(format!("{drive}:\\{folder}"))).collect()
}

fn windows_scan() -> Vec<InstalledGame> {
    let steam = registry_value_in(&reg_query(&["HKCU\\Software\\Valve\\Steam", "/v", "SteamPath"]), "SteamPath")
        .map_or_else(|| "C:\\Program Files (x86)\\Steam".to_owned(), |path| path.replace('/', "\\"));
    let mut found = steam_from(&[PathBuf::from(steam)]);

    let epic = Path::new("C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests");
    let items = entries(epic, false).into_iter().filter(|name| name.to_lowercase().ends_with(".item"));
    found.extend(items.filter_map(|name| epic_game(&read_text(&epic.join(name)))));

    let gog = reg_query(&["HKLM\\SOFTWARE\\WOW6432Node\\GOG.com\\Games", "/s", "/v", "path"]);
    found.extend(registry_folders(&gog, "path", "GOG"));
    let ubisoft = reg_query(&["HKLM\\SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher\\Installs", "/s", "/v", "InstallDir"]);
    found.extend(registry_folders(&ubisoft, "InstallDir", "Ubisoft"));

    let xbox: Vec<PathBuf> = DRIVES
        .iter()
        .flat_map(|drive| {
            let root = format!("{drive}:\\");
            let listed = xbox_folders(&std::fs::read(format!("{root}.GamingRoot")).unwrap_or_default());
            listed.into_iter().map(move |name| Path::new(&root).join(name))
        })
        .collect();
    found.extend(subfolder_games("Xbox", &xbox, &["gamesave"]));

    found.extend(riot_games(&read_text(Path::new("C:\\ProgramData\\Riot Games\\RiotClientInstalls.json"))));
    found.extend(subfolder_games("Riot", &on_drives("Riot Games"), &["riot client"]));

    let mut ea = vec![PathBuf::from("C:\\Program Files\\EA Games"), PathBuf::from("C:\\Program Files (x86)\\Origin Games")];
    ea.extend(on_drives("EA Games"));
    found.extend(subfolder_games("EA", &ea, &[]));

    found.extend(battle_net_games(&std::fs::read("C:\\ProgramData\\Battle.net\\Agent\\product.db").unwrap_or_default()));
    found
}

/// The first `name` value in `reg query` output.
fn registry_value_in(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| registry_value(line, name)).map(str::to_owned)
}

fn linux_scan() -> Vec<InstalledGame> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let mut found = steam_from(&[
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ]);
    let heroic = home.join(".config/heroic");
    found.extend(heroic_games(&read_text(&heroic.join("legendaryConfig/legendary/installed.json")), "Heroic"));
    found.extend(heroic_games(&read_text(&heroic.join("gog_store/installed.json")), "Heroic"));
    // Lutris keeps its games under ~/Games by default
    found.extend(subfolder_games("Lutris", &[home.join("Games")], &[]));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(source: &str, name: &str, key: &str) -> InstalledGame {
        InstalledGame { source: source.into(), name: name.into(), key: key.into() }
    }

    fn names(games: &[InstalledGame]) -> Vec<&str> {
        games.iter().map(|g| g.name.as_str()).collect()
    }

    #[test]
    fn steam_libraries_then_games_from_app_manifests_tools_left_out() {
        let vdf = r#""0" { "path"  "C:\\Program Files (x86)\\Steam" } "1" { "path" "/home/me/.local/share/Steam" }"#;
        assert_eq!(steam_libraries(vdf), [r"C:\Program Files (x86)\Steam", "/home/me/.local/share/Steam"]);

        let acf = |name: &str, dir: &str| format!(r#""AppState" {{ "name" "{name}" "installdir" "{dir}" }}"#);
        assert_eq!(steam_game(&acf("THE FINALS", "The Finals"), r"C:\Steam"), Some(installed("Steam", "THE FINALS", "c:/steam/steamapps/common/the finals/")));
        assert_eq!(steam_game(&acf("Proton 9.0", "Proton 9.0"), "/s"), None);
        assert_eq!(steam_game(&acf("Steam Linux Runtime - Sniper", "SteamLinuxRuntime_sniper"), "/s"), None);
        assert_eq!(steam_game(&acf("Wallpaper Engine", "wallpaper_engine"), "/s"), None);
    }

    #[test]
    fn a_value_that_reads_like_a_key_is_not_one() {
        let acf = r#""AppState" { "name" "path" "installdir" "Odd" "UserConfig" { "path" "x" } }"#;
        assert_eq!(vdf_values(acf, "path"), ["x"]);
        assert_eq!(vdf_values(acf, "INSTALLDIR"), ["Odd"]);
    }

    #[test]
    fn epic_counts_only_games_gog_and_ubisoft_come_from_the_registry() {
        let fortnite = r#"{"DisplayName":"Fortnite","InstallLocation":"C:\\Epic\\Fortnite","AppCategories":["games"]}"#;
        assert_eq!(epic_game(fortnite).map(|g| g.name), Some("Fortnite".into()));
        assert_eq!(epic_game(r#"{"DisplayName":"Unreal Engine","InstallLocation":"C:\\UE","AppCategories":["engines"]}"#), None);
        assert_eq!(epic_game("{"), None);
        let reg = "    path    REG_SZ    C:\\GOG Games\\Witcher 3\r\n";
        assert_eq!(registry_folders(reg, "path", "GOG"), [installed("GOG", "Witcher 3", "c:/gog games/witcher 3/")]);
    }

    #[test]
    fn registry_output_is_read_line_by_line() {
        let text = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher\\Installs\\635\r\n    InstallDir    REG_SZ    D:/Ubisoft/Far Cry 5/\r\n\r\nEnd of search: 1 match(es) found.\r\n";
        assert_eq!(registry_folders(text, "installdir", "Ubisoft"), [installed("Ubisoft", "Far Cry 5", "d:/ubisoft/far cry 5/")]);
        assert_eq!(registry_value_in("    SteamPath    REG_SZ    c:/program files (x86)/steam\r\n", "SteamPath").as_deref(), Some("c:/program files (x86)/steam"));
        assert_eq!(registry_folders("    pathology    REG_SZ    C:\\x\r\n", "path", "GOG"), []);
    }

    #[test]
    fn battle_net_keeps_install_folders_and_drops_their_parents_and_its_own() {
        let db = b"\x02C:/Program Files (x86)\x03C:/Program Files (x86)/StarCraft II\x04C:/ProgramData/Battle.net/Agent\x05D:/Games\x06D:/Games/Overwatch\x07";
        let found = battle_net_games(db);
        assert_eq!(names(&found), ["StarCraft II", "Overwatch"]);
        assert_eq!(found[0].source, "Battle.net");
        assert_eq!(game_at(r"C:\Program Files (x86)\Google\Chrome\chrome.exe", &found), None);
    }

    #[test]
    fn xbox_folders_from_gaming_root_riot_games_from_its_install_list() {
        let root: Vec<u8> = (0..56).step_by(2).map(|i| u8::from_str_radix(&"5247425801000000580062006f007800470061006d00650073000000"[i..i + 2], 16).unwrap()).collect();
        assert_eq!(xbox_folders(&root), ["XboxGames"]);
        assert!(xbox_folders(b"nope").is_empty());

        let riot = r#"{"associated_client":{"D:/Riot Games/VALORANT/live/":"x","C:\\Riot Games\\League of Legends":"y"}}"#;
        let found = riot_games(riot);
        let mut riot_names = names(&found);
        riot_names.sort_unstable();
        assert_eq!(riot_names, ["League of Legends", "VALORANT"]);
        assert!(riot_games("[").is_empty());
    }

    #[test]
    fn heroic_lists_its_installed_games_on_linux() {
        let legendary = r#"{"Fortnite":{"title":"Fortnite","install_path":"/home/me/Games/Heroic/Fortnite"}}"#;
        let gog = r#"{"installed":[{"appName":"1","install_path":"/home/me/Games/Heroic/Cyberpunk 2077","title":"Cyberpunk 2077"}]}"#;
        assert_eq!(heroic_games(legendary, "Heroic"), [installed("Heroic", "Fortnite", "/home/me/games/heroic/fortnite/")]);
        assert_eq!(heroic_games(gog, "Heroic")[0].name, "Cyberpunk 2077");
    }

    #[test]
    fn an_executable_belongs_to_the_deepest_folder_that_holds_it_never_a_sibling() {
        let games = [
            installed("Steam", "Deadlock", "c:/steam/steamapps/common/deadlock/"),
            installed("X", "Outer", "d:/games/"),
            installed("Y", "Inner", "d:/games/inner/"),
        ];
        assert_eq!(game_at(r"C:\Steam\steamapps\common\Deadlock\bin\deadlock.exe", &games).map(|g| g.name.as_str()), Some("Deadlock"));
        assert_eq!(game_at("D:/Games/Inner/x.exe", &games).map(|g| g.name.as_str()), Some("Inner"));
        assert_eq!(game_at(r"C:\Steam\steamapps\common\Deadlock-other\x.exe", &games), None);
        assert_eq!(game_at(r"C:\Steam\steam.exe", &games), None);
        assert_eq!(game_at("  ", &games), None);
    }

    #[test]
    fn folder_keys_and_duplicates() {
        assert_eq!(folder_key(r" C:\Games\Foo\\ "), "c:/games/foo/");
        assert_eq!(folder_key(""), "");
        let found = vec![installed("Steam", "A", "c:/a/"), installed("Epic", "A again", "c:/a/"), installed("EA", "none", ""), installed("EA", "B", "c:/b/")];
        assert_eq!(names(&unique(found)), ["A", "B"]);
    }

    #[test]
    fn steam_and_subfolders_are_read_from_disk() {
        let root = std::env::temp_dir().join(format!("scorpio-launchers-{}", std::process::id()));
        let steamapps = root.join("Steam").join("steamapps");
        let second = root.join("Library2");
        std::fs::create_dir_all(&steamapps).unwrap();
        std::fs::create_dir_all(second.join("steamapps")).unwrap();
        std::fs::create_dir_all(root.join("Games").join("Celeste")).unwrap();
        std::fs::create_dir_all(root.join("Games").join("Proton 8.0")).unwrap();
        let library = second.to_string_lossy().replace('\\', "\\\\");
        std::fs::write(steamapps.join("libraryfolders.vdf"), format!(r#""libraryfolders" {{ "0" {{ "path" "{library}" }} }}"#)).unwrap();
        std::fs::write(steamapps.join("appmanifest_730.acf"), r#""AppState" { "name" "Counter-Strike 2" "installdir" "Counter-Strike Global Offensive" }"#).unwrap();
        std::fs::write(steamapps.join("appmanifest_x.acf"), r#""AppState" { "name" "Not a manifest" "installdir" "x" }"#).unwrap();
        std::fs::write(second.join("steamapps").join("appmanifest_1.acf"), r#""AppState" { "name" "Deadlock" "installdir" "Deadlock" }"#).unwrap();

        let mut steam = names(&steam_from(&[root.join("Steam")])).into_iter().map(str::to_owned).collect::<Vec<_>>();
        steam.sort();
        let lutris = subfolder_games("Lutris", &[root.join("Games")], &[]);
        std::fs::remove_dir_all(&root).unwrap();

        assert_eq!(steam, ["Counter-Strike 2", "Deadlock"]);
        assert_eq!(names(&lutris), ["Celeste"]);
        assert!(lutris[0].key.ends_with("/games/celeste/"));
    }
}
