//! Watches for Rocket League starting and stopping.

pub mod ini;
pub mod locator;
pub mod probe;

use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{mpsc, watch};

pub const POLL_EVERY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameProcess {
    pub exe: Option<PathBuf>,
}

pub trait ProcessProbe: Send {
    fn find_game(&mut self) -> Option<GameProcess>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameWatchEvent {
    Started { exe: Option<PathBuf> },
    Stopped,
}

#[derive(Debug, Default)]
pub struct GameWatcher {
    running: bool,
}

impl GameWatcher {
    /// The first observation of a running game reports `Started`, so starting the app while
    /// the game is already open begins recording.
    pub fn observe(&mut self, seen: Option<GameProcess>) -> Option<GameWatchEvent> {
        match (self.running, seen) {
            (false, Some(game)) => {
                self.running = true;
                Some(GameWatchEvent::Started { exe: game.exe })
            }
            (true, None) => {
                self.running = false;
                Some(GameWatchEvent::Stopped)
            }
            _ => None,
        }
    }
}

/// Polls `probe` every `every` until `shutdown` turns true or the receiver is dropped.
pub async fn watch(
    mut probe: Box<dyn ProcessProbe>,
    every: Duration,
    tx: mpsc::Sender<GameWatchEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut watcher = GameWatcher::default();
    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.changed() => return,
        }
        if *shutdown.borrow() {
            return;
        }
        // A process snapshot takes a few milliseconds; acceptable on a 2 s cadence.
        if let Some(event) = watcher.observe(probe.find_game()) {
            tracing::info!(?event, "game process");
            if tx.send(event).await.is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    fn running() -> Option<GameProcess> {
        Some(GameProcess {
            exe: Some(PathBuf::from("C:/RL/Binaries/Win64/RocketLeague.exe")),
        })
    }

    #[test]
    fn a_game_already_running_at_startup_is_reported_once() {
        let mut w = GameWatcher::default();
        assert!(matches!(
            w.observe(running()),
            Some(GameWatchEvent::Started { exe: Some(_) })
        ));
        assert_eq!(w.observe(running()), None);
        assert_eq!(w.observe(None), Some(GameWatchEvent::Stopped));
        assert_eq!(w.observe(None), None);
    }

    struct Scripted(Arc<Mutex<VecDeque<Option<GameProcess>>>>);

    impl ProcessProbe for Scripted {
        fn find_game(&mut self) -> Option<GameProcess> {
            let mut q = self.0.lock().unwrap();
            if q.len() > 1 {
                q.pop_front().flatten()
            } else {
                q.front().cloned().flatten()
            }
        }
    }

    #[tokio::test]
    async fn watch_loop_reports_transitions_and_stops_on_shutdown() {
        let script = VecDeque::from([None, running(), running(), None]);
        let probe = Box::new(Scripted(Arc::new(Mutex::new(script))));
        let (tx, mut rx) = mpsc::channel(8);
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = tokio::spawn(watch(probe, Duration::from_millis(10), tx, stop_rx));
        let wait = Duration::from_secs(5);
        assert!(matches!(
            tokio::time::timeout(wait, rx.recv()).await.unwrap(),
            Some(GameWatchEvent::Started { .. })
        ));
        assert_eq!(
            tokio::time::timeout(wait, rx.recv()).await.unwrap(),
            Some(GameWatchEvent::Stopped)
        );
        stop_tx.send(true).unwrap();
        tokio::time::timeout(wait, task).await.unwrap().unwrap();
    }

    #[test]
    #[ignore = "needs Rocket League running"]
    fn finds_the_running_game() {
        assert!(probe::SysinfoProbe::new().find_game().is_some());
    }
}
