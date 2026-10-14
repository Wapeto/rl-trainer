//! Where Rocket League saves replays: `<Documents>\My Games\Rocket League\TAGame\DemosEpic`
//! (Epic) and `…\Demos` (Steam), or a folder the user chose in Settings.

use std::path::{Path, PathBuf};

pub fn replay_dirs(manual: Option<&Path>, documents: Option<&Path>) -> Vec<PathBuf> {
    let candidates: Vec<PathBuf> = match (manual, documents) {
        (Some(dir), _) => vec![dir.to_path_buf()],
        (None, Some(docs)) => {
            let tagame = docs.join("My Games").join("Rocket League").join("TAGame");
            vec![tagame.join("DemosEpic"), tagame.join("Demos")]
        }
        (None, None) => Vec::new(),
    };
    candidates.into_iter().filter(|d| d.is_dir()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manual_folder_wins_and_otherwise_both_game_folders_are_used() {
        let dir = tempfile::tempdir().unwrap();
        let tagame = dir
            .path()
            .join("My Games")
            .join("Rocket League")
            .join("TAGame");
        std::fs::create_dir_all(tagame.join("DemosEpic")).unwrap();
        std::fs::create_dir_all(tagame.join("Demos")).unwrap();
        assert_eq!(
            replay_dirs(None, Some(dir.path())),
            [tagame.join("DemosEpic"), tagame.join("Demos")]
        );
        let manual = dir.path().join("mine");
        std::fs::create_dir_all(&manual).unwrap();
        assert_eq!(replay_dirs(Some(&manual), Some(dir.path())), [manual]);
    }

    #[test]
    fn missing_folders_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        assert!(replay_dirs(None, Some(dir.path())).is_empty());
        assert!(replay_dirs(Some(&dir.path().join("gone")), None).is_empty());
        assert!(replay_dirs(None, None).is_empty());
    }
}
