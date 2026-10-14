//! A scriptable `Platform` for tests and demos.

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::elevated;
use crate::game_watch::{GameProcess, ProcessProbe};
use crate::platform::{LanAddresses, Platform, PlatformError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevationMode {
    /// Runs the elevated command in this process (the firewall command only records success).
    RunInProcess,
    Decline,
    ExitCode(i32),
}

pub struct FakePlatform {
    game: Arc<Mutex<Option<GameProcess>>>,
    elevation: Mutex<ElevationMode>,
    calls: Mutex<Vec<Vec<String>>>,
    firewall: AtomicBool,
    opened: Mutex<Vec<PathBuf>>,
    documents: Mutex<Option<PathBuf>>,
}

struct SharedProbe(Arc<Mutex<Option<GameProcess>>>);

impl ProcessProbe for SharedProbe {
    fn find_game(&mut self) -> Option<GameProcess> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl FakePlatform {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            game: Arc::new(Mutex::new(None)),
            elevation: Mutex::new(ElevationMode::RunInProcess),
            calls: Mutex::new(Vec::new()),
            firewall: AtomicBool::new(false),
            opened: Mutex::new(Vec::new()),
            documents: Mutex::new(None),
        })
    }

    pub fn set_documents(&self, dir: Option<PathBuf>) {
        *self
            .documents
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = dir;
    }

    pub fn opened_folders(&self) -> Vec<PathBuf> {
        self.opened
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_game(&self, game: Option<GameProcess>) {
        *self.game.lock().unwrap_or_else(PoisonError::into_inner) = game;
    }

    pub fn set_elevation(&self, mode: ElevationMode) {
        *self
            .elevation
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = mode;
    }

    pub fn elevated_calls(&self) -> Vec<Vec<String>> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Platform for FakePlatform {
    fn open_folder(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        self.opened
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(path.to_path_buf());
        Ok(())
    }

    fn open_url(&self, url: &str) -> Result<(), PlatformError> {
        self.open_folder(std::path::Path::new(url))
    }

    fn current_exe(&self) -> PathBuf {
        PathBuf::from("C:/Program Files/RL Trainer/rl-trainer.exe")
    }

    fn run_elevated(&self, args: &[String]) -> Result<i32, PlatformError> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(args.to_vec());
        match *self
            .elevation
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
        {
            ElevationMode::Decline => Err(PlatformError::Cancelled),
            ElevationMode::ExitCode(code) => Ok(code),
            ElevationMode::RunInProcess
                if args.get(1).is_some_and(|a| a == "add-firewall-rule") =>
            {
                self.firewall.store(true, Ordering::SeqCst);
                Ok(elevated::EXIT_OK)
            }
            ElevationMode::RunInProcess => Ok(elevated::run(&args[1..])),
        }
    }

    fn firewall_rule_present(&self) -> bool {
        self.firewall.load(Ordering::SeqCst)
    }

    /// Loopback, so tests of the LAN path do not listen on every interface.
    fn lan_bind_ip(&self) -> IpAddr {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    }

    fn epic_manifests_dir(&self) -> Option<PathBuf> {
        None
    }

    fn steam_root(&self) -> Option<PathBuf> {
        None
    }

    fn process_probe(&self) -> Box<dyn ProcessProbe> {
        Box::new(SharedProbe(self.game.clone()))
    }

    fn documents_dir(&self) -> Option<PathBuf> {
        self.documents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn lan_addresses(&self) -> LanAddresses {
        let local = IpAddr::V4(Ipv4Addr::LOCALHOST);
        LanAddresses {
            preferred: Some(local),
            all: vec![local],
        }
    }
}
