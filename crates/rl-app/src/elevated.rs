//! Commands run by `rl-trainer.exe --elevated <command>` after a UAC prompt. They accept only
//! the exact targets they exist for.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::game_watch::ini;

pub const FIREWALL_RULE: &str = "RL-Trainer-Phone";
pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_REFUSED: i32 = 3;
pub const EXIT_IO: i32 = 4;
pub const EXIT_COMMAND_FAILED: i32 = 5;

pub fn fix_stats_api_args(ini: &Path) -> Vec<String> {
    vec![
        "--elevated".into(),
        "fix-stats-api".into(),
        "--ini".into(),
        ini.to_string_lossy().into_owned(),
    ]
}

pub fn firewall_args(port: u16, program: &Path) -> Vec<String> {
    vec![
        "--elevated".into(),
        "add-firewall-rule".into(),
        "--port".into(),
        port.to_string(),
        "--program".into(),
        program.to_string_lossy().into_owned(),
    ]
}

/// `args` are the words after `--elevated`.
pub fn run(args: &[String]) -> i32 {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["fix-stats-api", "--ini", ini] => fix_stats_api(Path::new(ini)),
        ["add-firewall-rule", "--port", port, "--program", program] => {
            add_firewall_rule(port, Path::new(program))
        }
        _ => {
            eprintln!("rl-trainer: unknown elevated command {words:?}");
            EXIT_USAGE
        }
    }
}

pub fn validate_ini_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("the ini path must be absolute".into());
    }
    let canonical =
        std::fs::canonicalize(path).map_err(|e| format!("cannot resolve the ini path: {e}"))?;
    let tail: Vec<String> = canonical
        .components()
        .rev()
        .take(3)
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    if tail != ["defaultstatsapi.ini", "config", "tagame"] {
        return Err("only TAGame\\Config\\DefaultStatsAPI.ini may be edited".into());
    }
    Ok(canonical)
}

pub fn fix_ini_file(path: &Path) -> io::Result<()> {
    let text = std::fs::read_to_string(path)?;
    let mut backup = path.as_os_str().to_owned();
    backup.push(".bak");
    std::fs::copy(path, PathBuf::from(backup))?;
    std::fs::write(path, ini::fix(&text))
}

fn fix_stats_api(path: &Path) -> i32 {
    let path = match validate_ini_path(path) {
        Ok(path) => path,
        Err(reason) => {
            eprintln!("rl-trainer: refused: {reason}");
            return EXIT_REFUSED;
        }
    };
    match fix_ini_file(&path) {
        Ok(()) => EXIT_OK,
        Err(err) => {
            eprintln!("rl-trainer: could not update {}: {err}", path.display());
            EXIT_IO
        }
    }
}

pub fn netsh_add_args(port: u16, program: &Path) -> Vec<String> {
    vec![
        "advfirewall".into(),
        "firewall".into(),
        "add".into(),
        "rule".into(),
        format!("name={FIREWALL_RULE}"),
        "dir=in".into(),
        "action=allow".into(),
        "protocol=TCP".into(),
        format!("localport={port}"),
        "profile=private".into(),
        format!("program={}", program.display()),
    ]
}

pub fn netsh_delete_args() -> Vec<String> {
    ["advfirewall", "firewall", "delete", "rule"]
        .into_iter()
        .map(String::from)
        .chain([format!("name={FIREWALL_RULE}")])
        .collect()
}

pub fn netsh_show_args() -> Vec<String> {
    ["advfirewall", "firewall", "show", "rule"]
        .into_iter()
        .map(String::from)
        .chain([format!("name={FIREWALL_RULE}")])
        .collect()
}

/// The program the firewall rule may name: this executable, and only when `program` resolves
/// to it.
pub fn firewall_target(program: &Path) -> Option<PathBuf> {
    let own = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .ok()?;
    let given = std::fs::canonicalize(program).ok()?;
    // The resolved path, never the given one: a junction in `program` could be swapped after
    // the check.
    (own == given).then(|| plain_path(&own))
}

/// `\\?\C:\x` → `C:\x` and `\\?\UNC\s\x` → `\\s\x` (netsh does not take verbatim paths).
fn plain_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

fn add_firewall_rule(port: &str, program: &Path) -> i32 {
    let Ok(port) = port.parse::<u16>() else {
        return EXIT_USAGE;
    };
    let Some(program) = firewall_target(program) else {
        eprintln!("rl-trainer: refused: the rule must target rl-trainer itself");
        return EXIT_REFUSED;
    };
    let program = program.as_path();
    // Replacing keeps exactly one rule when the port changes; a missing rule is fine.
    let _ = Command::new("netsh").args(netsh_delete_args()).status();
    match Command::new("netsh")
        .args(netsh_add_args(port, program))
        .status()
    {
        Ok(status) if status.success() => EXIT_OK,
        Ok(status) => {
            eprintln!("rl-trainer: netsh failed: {status}");
            EXIT_COMMAND_FAILED
        }
        Err(err) => {
            eprintln!("rl-trainer: cannot run netsh: {err}");
            EXIT_COMMAND_FAILED
        }
    }
}

/// Quotes one argument so `CommandLineToArgvW` (how the elevated process reads its command
/// line) gives it back unchanged: backslashes count only before a quote or at the end.
pub fn quote_arg(arg: &str) -> String {
    let mut out = String::from('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_layout(root: &Path) -> PathBuf {
        let config = root.join("TAGame").join("Config");
        std::fs::create_dir_all(&config).unwrap();
        let ini = config.join("DefaultStatsAPI.ini");
        std::fs::write(
            &ini,
            "[TAGame.MatchStatsExporter_TA]\nPort=49123\nPacketSendRate=0\n",
        )
        .unwrap();
        ini
    }

    #[test]
    fn only_the_stats_api_ini_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let ini = real_layout(dir.path());
        assert!(validate_ini_path(&ini).is_ok());
        let detour = dir
            .path()
            .join("TAGame")
            .join("Config")
            .join("..")
            .join("Config")
            .join("DefaultStatsAPI.ini");
        assert!(
            validate_ini_path(&detour).is_ok(),
            "canonical form is what counts"
        );
        let other = dir.path().join("TAGame").join("Config").join("Other.ini");
        std::fs::write(&other, "x").unwrap();
        assert!(validate_ini_path(&other).is_err());
        let fake = dir.path().join("evil").join("DefaultStatsAPI.ini");
        std::fs::create_dir_all(fake.parent().unwrap()).unwrap();
        std::fs::write(&fake, "x").unwrap();
        assert!(validate_ini_path(&fake).is_err());
        assert!(
            validate_ini_path(Path::new("TAGame/Config/DefaultStatsAPI.ini")).is_err(),
            "relative"
        );
    }

    #[test]
    fn fix_command_backs_up_then_fixes() {
        let dir = tempfile::tempdir().unwrap();
        let ini = real_layout(dir.path());
        let original = std::fs::read_to_string(&ini).unwrap();
        let args: Vec<String> = fix_stats_api_args(&ini).into_iter().skip(1).collect();
        assert_eq!(run(&args), EXIT_OK);
        let bak = dir
            .path()
            .join("TAGame")
            .join("Config")
            .join("DefaultStatsAPI.ini.bak");
        assert_eq!(std::fs::read_to_string(bak).unwrap(), original);
        assert!(std::fs::read_to_string(&ini)
            .unwrap()
            .contains("PacketSendRate=10"));
    }

    #[test]
    fn bad_invocations_are_refused() {
        assert_eq!(run(&["nonsense".to_owned()]), EXIT_USAGE);
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = dir.path().join("other.exe");
        std::fs::write(&elsewhere, "x").unwrap();
        let args: Vec<String> = firewall_args(49_321, &elsewhere)
            .into_iter()
            .skip(1)
            .collect();
        assert_eq!(
            run(&args),
            EXIT_REFUSED,
            "a rule may only target this program"
        );
    }

    #[test]
    fn the_firewall_rule_names_the_resolved_executable_not_the_given_path() {
        let exe = std::env::current_exe().unwrap();
        let detour = exe
            .parent()
            .unwrap()
            .join(".")
            .join(exe.file_name().unwrap());
        let target = firewall_target(&detour).expect("this executable is accepted");
        assert!(
            !target.to_string_lossy().starts_with(r"\\?\"),
            "{}",
            target.display()
        );
        assert!(
            !target.components().any(|c| c.as_os_str() == "."),
            "resolved, not as given"
        );
        assert_eq!(
            std::fs::canonicalize(&target).unwrap(),
            std::fs::canonicalize(&exe).unwrap()
        );
        assert_eq!(firewall_target(Path::new("C:/Windows/notepad.exe")), None);
    }

    #[test]
    fn netsh_rule_is_private_and_scoped() {
        let args = netsh_add_args(49_321, Path::new("C:/Apps/rl-trainer.exe"));
        for expected in [
            "name=RL-Trainer-Phone",
            "dir=in",
            "protocol=TCP",
            "localport=49321",
            "profile=private",
        ] {
            assert!(args.iter().any(|a| a == expected), "missing {expected}");
        }
        assert!(args.iter().any(|a| a.starts_with("program=")));
    }

    #[test]
    fn arguments_are_quoted_for_command_line_to_argv() {
        assert_eq!(
            quote_arg(r"C:\Program Files\rl"),
            r#""C:\Program Files\rl""#
        );
        assert_eq!(quote_arg(r"C:\dir\"), r#""C:\dir\\""#);
        assert_eq!(quote_arg(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote_arg(r#"a\\"b"#), r#""a\\\\\"b""#);
        assert_eq!(quote_arg(""), r#""""#);
    }
}
