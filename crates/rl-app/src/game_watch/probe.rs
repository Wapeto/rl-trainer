//! The real process probe (sysinfo; works on every OS, used on Windows).

use std::ffi::OsStr;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::game_watch::locator::GAME_EXE;
use crate::game_watch::{GameProcess, ProcessProbe};

pub struct SysinfoProbe {
    system: System,
}

impl SysinfoProbe {
    pub fn new() -> Self {
        Self {
            system: System::new(),
        }
    }
}

impl Default for SysinfoProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessProbe for SysinfoProbe {
    fn find_game(&mut self) -> Option<GameProcess> {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        self.system
            .processes_by_exact_name(OsStr::new(GAME_EXE))
            .next()
            .map(|p| GameProcess {
                exe: p.exe().map(std::path::Path::to_path_buf),
            })
    }
}
