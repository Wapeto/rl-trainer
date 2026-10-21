//! Polls the replay folders (every 5 s) instead of a file watcher: no extra dependency, and it
//! behaves the same on OneDrive folders. Spec §4.1.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::app::App;

/// Files modified more than this before the first recorded match cannot belong to one.
pub const SKIP_BEFORE_MS: i64 = 3_600_000;
/// A file must be this old, with the same size at two polls, before it is read.
pub const STABLE_MS: i64 = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub path: PathBuf,
    pub modified_at: i64,
    pub size: i64,
}

/// The files to import now, and the sizes to compare with at the next poll.
pub fn ready(
    entries: &[FileInfo],
    earliest_match: Option<i64>,
    seen: &HashMap<String, (i64, i64)>,
    last_sizes: &HashMap<String, i64>,
    now_ms: i64,
) -> (Vec<FileInfo>, HashMap<String, i64>) {
    let Some(earliest) = earliest_match else {
        return (Vec::new(), HashMap::new());
    };
    let mut sizes = HashMap::new();
    let mut out = Vec::new();
    for f in entries {
        if f.modified_at < earliest - SKIP_BEFORE_MS {
            continue;
        }
        let key = f.path.to_string_lossy().into_owned();
        if seen.get(&key) == Some(&(f.modified_at, f.size)) {
            continue;
        }
        let stable = last_sizes.get(&key) == Some(&f.size) && now_ms - f.modified_at >= STABLE_MS;
        if stable {
            out.push(f.clone());
        } else {
            sizes.insert(key, f.size);
        }
    }
    (out, sizes)
}

/// `seen` without the files to try again in this run.
pub fn without(
    mut seen: HashMap<String, (i64, i64)>,
    retry: &HashSet<String>,
) -> HashMap<String, (i64, i64)> {
    seen.retain(|path, _| !retry.contains(path));
    seen
}

/// What the poller remembers between ticks.
#[derive(Default)]
struct PollState {
    /// Sizes seen at the last tick, to wait for a stable size.
    sizes: HashMap<String, i64>,
    /// Files to read once more at this start: read failures, and imported replays with no player
    /// rows (loaded at the first tick).
    retry: Option<HashSet<String>>,
}

/// The `.replay` files in `dirs` with their time and size (unreadable entries are skipped).
pub fn list(dirs: &[PathBuf]) -> Vec<FileInfo> {
    dirs.iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("replay"))
        })
        .filter_map(|e| info(&e.path()))
        .collect()
}

fn info(path: &Path) -> Option<FileInfo> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let ms = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some(FileInfo {
        path: path.to_path_buf(),
        modified_at: i64::try_from(ms).ok()?,
        size: i64::try_from(meta.len()).ok()?,
    })
}

impl App {
    pub(crate) fn spawn_replay_poller(&self, every: Duration) {
        let app = self.clone();
        let mut stop = self.stop_receiver();
        let task = tokio::spawn(async move {
            let mut state = PollState::default();
            let mut tick = tokio::time::interval(every);
            loop {
                tokio::select! {
                    _ = tick.tick() => {}
                    _ = stop.changed() => return,
                }
                if let Err(err) = app.poll_replays(&mut state).await {
                    tracing::warn!(error = %err, "replay folder check failed");
                }
                app.run_first_backfill().await;
            }
        });
        crate::app::push_background(self, task);
    }

    async fn poll_replays(&self, state: &mut PollState) -> Result<(), crate::app::AppError> {
        let dirs = self.replay_dirs()?;
        let (seen, earliest) = {
            let conn = self.read_db()?;
            if state.retry.is_none() {
                let mut retry = rl_core::store::replays::read_failures(&conn)?;
                // Replays read before the match page existed: read once more for every player's stats.
                retry.extend(rl_core::store::replays::missing_players(&conn)?);
                state.retry = Some(retry);
            }
            let retry = state.retry.as_ref().map_or_else(HashSet::new, Clone::clone);
            (
                without(rl_core::store::replays::seen_files(&conn)?, &retry),
                rl_core::store::replays::earliest_match_at(&conn)?,
            )
        };
        let entries = tokio::task::spawn_blocking(move || list(&dirs))
            .await
            .map_err(|e| crate::app::AppError::Join(e.to_string()))?;
        let (now_ready, next) = ready(
            &entries,
            earliest,
            &seen,
            &state.sizes,
            rl_core::ingest::now_ms(),
        );
        state.sizes = next;
        for file in now_ready {
            if let Some(retry) = state.retry.as_mut() {
                retry.remove(file.path.to_string_lossy().as_ref());
            }
            let path = file.path.display().to_string();
            match self.import_replay(file).await {
                Ok(status) => tracing::info!(%path, status = status.as_str(), "replay looked at"),
                Err(err) => tracing::error!(%path, error = %err, "replay import failed"),
            }
        }
        Ok(())
    }

    /// The folders to watch now (the setting can change while running).
    pub fn replay_dirs(&self) -> Result<Vec<PathBuf>, crate::app::AppError> {
        let manual = self
            .inner
            .replay_dir_override
            .clone()
            .or(self.settings()?.replay_dir);
        let docs = self.inner.platform.documents_dir();
        Ok(super::folder::replay_dirs(
            manual.as_deref(),
            docs.as_deref(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000_000;

    fn file(name: &str, modified_at: i64, size: i64) -> FileInfo {
        FileInfo {
            path: PathBuf::from(format!("C:/r/{name}.replay")),
            modified_at,
            size,
        }
    }

    fn key(f: &FileInfo) -> String {
        f.path.to_string_lossy().into_owned()
    }

    #[test]
    fn old_files_are_skipped_without_reading() {
        let old = file("old", NOW - 10 * SKIP_BEFORE_MS, 100);
        let sizes = HashMap::from([(key(&old), 100)]);
        let (ready_now, _) = ready(
            &[old],
            Some(NOW - SKIP_BEFORE_MS),
            &HashMap::new(),
            &sizes,
            NOW,
        );
        assert!(ready_now.is_empty());
        let (none, _) = ready(
            &[file("new", NOW - 60_000, 1)],
            None,
            &HashMap::new(),
            &HashMap::new(),
            NOW,
        );
        assert!(none.is_empty(), "with no recorded match, nothing can match");
    }

    #[test]
    fn a_growing_file_waits_until_its_size_is_stable() {
        let f1 = file("a", NOW - 10_000, 100);
        let (first, sizes) = ready(
            std::slice::from_ref(&f1),
            Some(0),
            &HashMap::new(),
            &HashMap::new(),
            NOW,
        );
        assert!(first.is_empty(), "first sighting: remember the size");
        let f2 = file("a", NOW - 5_000, 200);
        let (second, sizes) = ready(
            std::slice::from_ref(&f2),
            Some(0),
            &HashMap::new(),
            &sizes,
            NOW,
        );
        assert!(second.is_empty(), "still growing");
        let (third, _) = ready(
            std::slice::from_ref(&f2),
            Some(0),
            &HashMap::new(),
            &sizes,
            NOW,
        );
        assert_eq!(third, [f2]);
        let fresh = file("b", NOW - 500, 50);
        let sizes = HashMap::from([(key(&fresh), 50)]);
        assert!(
            ready(&[fresh], Some(0), &HashMap::new(), &sizes, NOW)
                .0
                .is_empty(),
            "under 2 s old"
        );
    }

    #[test]
    fn a_file_seen_with_the_same_size_and_time_is_not_read_again() {
        let f = file("a", NOW - 10_000, 100);
        let seen = HashMap::from([(key(&f), (f.modified_at, f.size))]);
        let sizes = HashMap::from([(key(&f), 100)]);
        assert!(ready(std::slice::from_ref(&f), Some(0), &seen, &sizes, NOW)
            .0
            .is_empty());
        let changed = file("a", NOW - 9_000, 100);
        assert_eq!(
            ready(std::slice::from_ref(&changed), Some(0), &seen, &sizes, NOW).0,
            [changed]
        );
    }

    #[test]
    fn a_file_that_could_not_be_read_is_tried_again_after_a_start() {
        let f = file("a", NOW - 10_000, 100);
        let seen = HashMap::from([(key(&f), (f.modified_at, f.size))]);
        let retry = HashSet::from([key(&f)]);
        let sizes = HashMap::from([(key(&f), 100)]);
        let unseen = without(seen.clone(), &retry);
        assert_eq!(
            ready(std::slice::from_ref(&f), Some(0), &unseen, &sizes, NOW).0,
            std::slice::from_ref(&f)
        );
        assert!(ready(std::slice::from_ref(&f), Some(0), &seen, &sizes, NOW)
            .0
            .is_empty());
    }
}
