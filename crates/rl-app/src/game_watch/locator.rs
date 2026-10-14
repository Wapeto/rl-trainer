//! Finding the Rocket League install: running process, Epic, Steam, then a manual folder.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub const GAME_EXE: &str = "RocketLeague.exe";
const EPIC_APP_NAME: &str = "Sugar";
const STEAM_APP_ID: &str = "252950";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallSource {
    RunningProcess,
    Epic,
    Steam,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Install {
    pub root: PathBuf,
    pub source: InstallSource,
}

impl Install {
    pub fn stats_ini(&self) -> PathBuf {
        stats_ini_path(&self.root)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InstallHints {
    pub running_exe: Option<PathBuf>,
    pub epic_manifests: Option<PathBuf>,
    pub steam_root: Option<PathBuf>,
    pub manual: Option<PathBuf>,
}

pub fn stats_ini_path(root: &Path) -> PathBuf {
    root.join("TAGame")
        .join("Config")
        .join("DefaultStatsAPI.ini")
}

fn has_ini(root: &Path) -> bool {
    stats_ini_path(root).is_file()
}

/// `<root>\Binaries\Win64\RocketLeague.exe` → `<root>`.
pub fn root_from_exe(exe: &Path) -> Option<PathBuf> {
    exe.parent()?.parent()?.parent().map(Path::to_path_buf)
}

pub fn epic_install(manifests_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(manifests_dir).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "item"))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .filter_map(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .find(|item| item["AppName"] == EPIC_APP_NAME)
        .and_then(|item| item["InstallLocation"].as_str().map(PathBuf::from))
}

/// Every `"path" "<dir>"` pair in `libraryfolders.vdf`.
pub fn steam_libraries(vdf: &str) -> Vec<PathBuf> {
    let tokens: Vec<String> = vdf
        .split('"')
        .skip(1)
        .step_by(2)
        .map(|t| t.replace("\\\\", "\\"))
        .collect();
    tokens
        .windows(2)
        .filter(|pair| pair[0].eq_ignore_ascii_case("path"))
        .map(|pair| PathBuf::from(&pair[1]))
        .collect()
}

pub fn steam_install(steam_root: &Path) -> Option<PathBuf> {
    let vdf =
        std::fs::read_to_string(steam_root.join("steamapps").join("libraryfolders.vdf")).ok()?;
    steam_libraries(&vdf)
        .into_iter()
        .find(|lib| {
            lib.join("steamapps")
                .join(format!("appmanifest_{STEAM_APP_ID}.acf"))
                .is_file()
        })
        .map(|lib| lib.join("steamapps").join("common").join("rocketleague"))
}

pub fn locate(hints: &InstallHints) -> Option<Install> {
    let candidates = [
        (
            hints.running_exe.as_deref().and_then(root_from_exe),
            InstallSource::RunningProcess,
        ),
        (
            hints.epic_manifests.as_deref().and_then(epic_install),
            InstallSource::Epic,
        ),
        (
            hints.steam_root.as_deref().and_then(steam_install),
            InstallSource::Steam,
        ),
        (hints.manual.clone(), InstallSource::Manual),
    ];
    candidates.into_iter().find_map(|(root, source)| {
        root.filter(|r| has_ini(r))
            .map(|root| Install { root, source })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_install(root: &Path) {
        let config = root.join("TAGame").join("Config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("DefaultStatsAPI.ini"),
            "[TAGame.MatchStatsExporter_TA]\n",
        )
        .unwrap();
    }

    #[test]
    fn running_process_path_wins() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("rocketleague");
        make_install(&root);
        let exe = root.join("Binaries").join("Win64").join(GAME_EXE);
        assert_eq!(root_from_exe(&exe), Some(root.clone()));
        let found = locate(&InstallHints {
            running_exe: Some(exe),
            manual: Some(dir.path().join("elsewhere")),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            (found.root, found.source),
            (root.clone(), InstallSource::RunningProcess)
        );
    }

    #[test]
    fn epic_manifest_is_found_by_app_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("rocketleague");
        make_install(&root);
        let manifests = dir.path().join("Manifests");
        std::fs::create_dir_all(manifests.join("Pending")).unwrap();
        std::fs::write(
            manifests.join("A.item"),
            r#"{"AppName":"Fortnite","InstallLocation":"C:\\nope"}"#,
        )
        .unwrap();
        std::fs::write(manifests.join("broken.item"), "{not json").unwrap();
        let item = serde_json::json!({"AppName": "Sugar", "InstallLocation": root});
        std::fs::write(manifests.join("B.item"), item.to_string()).unwrap();
        assert_eq!(epic_install(&manifests), Some(root.clone()));
        let found = locate(&InstallHints {
            epic_manifests: Some(manifests),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(found.source, InstallSource::Epic);
    }

    #[test]
    fn steam_library_with_the_app_manifest_is_used() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}\n";
        assert_eq!(
            steam_libraries(vdf),
            [
                PathBuf::from("C:\\Program Files (x86)\\Steam"),
                PathBuf::from("D:\\SteamLibrary")
            ]
        );
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        let lib = dir.path().join("Lib");
        std::fs::create_dir_all(steam.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib.join("steamapps")).unwrap();
        let vdf = format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} }}",
            steam.display().to_string().replace('\\', "\\\\"),
            lib.display().to_string().replace('\\', "\\\\")
        );
        std::fs::write(steam.join("steamapps").join("libraryfolders.vdf"), vdf).unwrap();
        std::fs::write(lib.join("steamapps").join("appmanifest_252950.acf"), "x").unwrap();
        let root = lib.join("steamapps").join("common").join("rocketleague");
        make_install(&root);
        assert_eq!(steam_install(&steam), Some(root));
    }

    #[test]
    fn manual_folder_is_the_last_resort_and_must_contain_the_ini() {
        let dir = tempfile::tempdir().unwrap();
        assert!(locate(&InstallHints {
            manual: Some(dir.path().to_path_buf()),
            ..Default::default()
        })
        .is_none());
        make_install(dir.path());
        let found = locate(&InstallHints {
            manual: Some(dir.path().to_path_buf()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(found.source, InstallSource::Manual);
        assert!(found
            .stats_ini()
            .ends_with("TAGame/Config/DefaultStatsAPI.ini"));
    }
}
