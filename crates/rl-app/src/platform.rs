//! What the app needs from the operating system, behind a trait so tests can fake it.

use std::net::IpAddr;
use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;

use crate::game_watch::{GameProcess, ProcessProbe};

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("the administrator prompt was declined")]
    Cancelled,
    #[error("{0}")]
    Failed(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LanAddresses {
    pub preferred: Option<IpAddr>,
    pub all: Vec<IpAddr>,
}

/// Private IPv4 addresses of this PC; `preferred` is the one carrying the default route.
pub fn lan_addresses() -> LanAddresses {
    let preferred = local_ip_address::local_ip().ok();
    let mut all: Vec<IpAddr> = local_ip_address::list_afinet_netifas()
        .map(|list| {
            list.into_iter()
                .map(|(_, ip)| ip)
                .filter(|ip| matches!(ip, IpAddr::V4(v4) if v4.is_private()))
                .collect()
        })
        .unwrap_or_default();
    all.sort();
    all.dedup();
    LanAddresses { preferred, all }
}

pub trait Platform: Send + Sync + 'static {
    fn current_exe(&self) -> PathBuf;
    /// Runs this executable elevated with `args` (starting with `--elevated`) and waits.
    fn run_elevated(&self, args: &[String]) -> Result<i32, PlatformError>;
    fn firewall_rule_present(&self) -> bool;
    fn epic_manifests_dir(&self) -> Option<PathBuf>;
    fn steam_root(&self) -> Option<PathBuf>;
    fn process_probe(&self) -> Box<dyn ProcessProbe>;

    /// The user's Documents folder, following a OneDrive or other redirect.
    fn documents_dir(&self) -> Option<PathBuf> {
        None
    }
    fn lan_addresses(&self) -> LanAddresses {
        lan_addresses()
    }

    /// Where the phone page listens for the LAN: every interface.
    fn lan_bind_ip(&self) -> std::net::IpAddr {
        std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
    }

    /// Shows a folder in the file manager. Does nothing where there is none (headless).
    fn open_folder(&self, _path: &std::path::Path) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Opens a web page in the default browser. Does nothing where there is none (headless).
    fn open_url(&self, _url: &str) -> Result<(), PlatformError> {
        Ok(())
    }
}

/// `rl-replay serve`: the Stats API address is given explicitly, so the game counts as running.
pub struct HeadlessPlatform;

struct AlwaysRunning;

impl ProcessProbe for AlwaysRunning {
    fn find_game(&mut self) -> Option<GameProcess> {
        Some(GameProcess { exe: None })
    }
}

impl Platform for HeadlessPlatform {
    fn current_exe(&self) -> PathBuf {
        std::env::current_exe().unwrap_or_default()
    }
    fn run_elevated(&self, _args: &[String]) -> Result<i32, PlatformError> {
        Err(PlatformError::Failed(
            "administrator actions are not available in serve mode".into(),
        ))
    }
    fn firewall_rule_present(&self) -> bool {
        true // left to Windows' own prompt in serve mode
    }
    fn epic_manifests_dir(&self) -> Option<PathBuf> {
        None
    }
    fn steam_root(&self) -> Option<PathBuf> {
        None
    }
    fn process_probe(&self) -> Box<dyn ProcessProbe> {
        Box::new(AlwaysRunning)
    }
}

#[cfg(windows)]
pub use windows_impl::WindowsPlatform;

#[cfg(windows)]
mod windows_impl {
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};

    use super::{Platform, PlatformError};
    use crate::elevated::netsh_show_args;
    use crate::game_watch::probe::SysinfoProbe;
    use crate::game_watch::ProcessProbe;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub struct WindowsPlatform;

    fn shell_execute_runas(exe: &Path, args: &[String]) -> Result<i32, PlatformError> {
        use windows::core::{w, HSTRING, PCWSTR};
        use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, WaitForSingleObject, INFINITE,
        };
        use windows::Win32::UI::Shell::{
            ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
        };
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

        let file = HSTRING::from(exe.as_os_str());
        let params = HSTRING::from(
            args.iter()
                .map(|a| crate::elevated::quote_arg(a))
                .collect::<Vec<_>>()
                .join(" "),
        );
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        // SAFETY: `info` is fully initialised and its strings (`file`, `params`, the static
        // verb) outlive the call.
        unsafe { ShellExecuteExW(&mut info) }.map_err(|e| {
            if e.code() == ERROR_CANCELLED.to_hresult() {
                PlatformError::Cancelled
            } else {
                PlatformError::Failed(e.message().to_string())
            }
        })?;
        let mut code = 0_u32;
        // SAFETY: SEE_MASK_NOCLOSEPROCESS was set and the call succeeded, so `hProcess` is a
        // valid process handle that we own and close exactly once.
        let waited = unsafe {
            WaitForSingleObject(info.hProcess, INFINITE);
            let result = GetExitCodeProcess(info.hProcess, &mut code);
            let _ = CloseHandle(info.hProcess);
            result
        };
        waited.map_err(|e| PlatformError::Failed(e.message().to_string()))?;
        Ok(code as i32)
    }

    impl Platform for WindowsPlatform {
        fn open_folder(&self, path: &std::path::Path) -> Result<(), PlatformError> {
            std::process::Command::new("explorer.exe")
                .arg(path)
                .spawn()
                .map(|_| ())
                .map_err(|e| PlatformError::Failed(e.to_string()))
        }

        fn open_url(&self, url: &str) -> Result<(), PlatformError> {
            // Explorer hands an http(s) URL to the default browser.
            self.open_folder(std::path::Path::new(url))
        }

        fn current_exe(&self) -> PathBuf {
            std::env::current_exe().unwrap_or_default()
        }

        fn run_elevated(&self, args: &[String]) -> Result<i32, PlatformError> {
            shell_execute_runas(&self.current_exe(), args)
        }

        fn firewall_rule_present(&self) -> bool {
            std::process::Command::new("netsh")
                .args(netsh_show_args())
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .is_ok_and(|out| out.status.success())
        }

        fn epic_manifests_dir(&self) -> Option<PathBuf> {
            std::env::var_os("ProgramData").map(|d| {
                PathBuf::from(d)
                    .join("Epic")
                    .join("EpicGamesLauncher")
                    .join("Data")
                    .join("Manifests")
            })
        }

        fn steam_root(&self) -> Option<PathBuf> {
            use winreg::enums::HKEY_CURRENT_USER;
            let key = winreg::RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey("Software\\Valve\\Steam")
                .ok()?;
            key.get_value::<String, _>("SteamPath")
                .ok()
                .map(PathBuf::from)
        }

        fn documents_dir(&self) -> Option<PathBuf> {
            use winreg::enums::HKEY_CURRENT_USER;
            // "Shell Folders" holds the expanded path, redirects included.
            let key = winreg::RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(
                    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders",
                )
                .ok();
            key.and_then(|k| k.get_value::<String, _>("Personal").ok())
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Documents"))
                })
        }

        fn process_probe(&self) -> Box<dyn ProcessProbe> {
            Box::new(SysinfoProbe::new())
        }
    }
}
