//! Debug builds only: Settings → Restart rebuilds the app from its sources first, so a change
//! is one click away. The app keeps recording during the build; a failed build leaves it running.
//!
//! Windows locks a running exe, but lets it be renamed: the running file (and its hard link in
//! `deps/`) move to `*.old`, so cargo can link the new exe in their place. The restart then
//! launches the original path, which now holds the new build.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ASIDE_EXT: &str = "old";
const PACKAGE: &str = "rl-trainer";
const ERROR_LINES: usize = 3;

/// The workspace this debug exe was built from; `None` in a release build or once the sources
/// are gone.
pub fn workspace() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    root.join("Cargo.toml").is_file().then_some(root)
}

/// Builds the app (rl-app's build script rebuilds the UI too). On success the new exe is in
/// place for the restart; on failure everything is put back and the error is returned.
pub fn rebuild(root: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot find the running app: {e}"))?;
    let moved = move_aside(&locked_files(&exe))?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    tracing::info!(root = %root.display(), "rebuilding before the restart");
    let output = Command::new(cargo)
        .args(["build", "-p", PACKAGE])
        .current_dir(root)
        .output();
    match output {
        Ok(out) if out.status.success() => Ok(()),
        result => {
            put_back(&moved);
            Err(failure(result))
        }
    }
}

/// The running exe and, when cargo built it, its hard link in `deps/` (same file, same lock).
fn locked_files(exe: &Path) -> Vec<PathBuf> {
    let mut files = vec![exe.to_path_buf()];
    if let (Some(dir), Some(name)) = (exe.parent(), exe.file_name()) {
        let link = dir
            .join("deps")
            .join(name.to_string_lossy().replace('-', "_"));
        if link.is_file() {
            files.push(link);
        }
    }
    files
}

fn aside(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".");
    name.push(ASIDE_EXT);
    PathBuf::from(name)
}

/// Renames each file to `*.old` (replacing the copy a previous rebuild left); returns the moves
/// made. On an error the moves already made are undone.
fn move_aside(files: &[PathBuf]) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut moved = Vec::new();
    for file in files {
        let to = aside(file);
        let _ = std::fs::remove_file(&to);
        if let Err(err) = std::fs::rename(file, &to) {
            put_back(&moved);
            return Err(format!(
                "Cannot move {} aside to rebuild: {err}",
                file.display()
            ));
        }
        moved.push((file.clone(), to));
    }
    Ok(moved)
}

/// Undoes `move_aside` wherever the build left no new file.
fn put_back(moved: &[(PathBuf, PathBuf)]) {
    for (original, aside) in moved {
        if !original.exists() {
            if let Err(err) = std::fs::rename(aside, original) {
                tracing::warn!(file = %original.display(), error = %err, "could not put the exe back");
            }
        }
    }
}

/// A short message for the window; the full cargo output goes to the terminal and the log.
fn failure(result: std::io::Result<Output>) -> String {
    match result {
        Err(err) => format!("Cannot run cargo to rebuild: {err}"),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("{stderr}");
            tracing::warn!(output = %stderr, "rebuild failed");
            format!(
                "Rebuild failed, still running the old build: {}",
                summary(&stderr)
            )
        }
    }
}

/// The first few `error` lines of cargo's output, or its last line when there are none.
fn summary(stderr: &str) -> String {
    let errors: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("error"))
        .take(ERROR_LINES)
        .collect();
    if errors.is_empty() {
        stderr
            .lines()
            .rev()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("no output")
            .to_owned()
    } else {
        errors.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_the_exe_and_its_hard_link_in_deps() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("rl-trainer.exe");
        fs::write(&exe, "exe").unwrap();
        assert_eq!(locked_files(&exe), vec![exe.clone()], "no deps copy");
        fs::create_dir(dir.path().join("deps")).unwrap();
        let link = dir.path().join("deps").join("rl_trainer.exe");
        fs::write(&link, "exe").unwrap();
        assert_eq!(locked_files(&exe), vec![exe, link]);
    }

    #[test]
    fn moves_files_aside_replacing_an_old_copy_and_puts_back_what_the_build_did_not_replace() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.exe");
        let b = dir.path().join("b.exe");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        fs::write(aside(&a), "stale").unwrap();

        let moved = move_aside(&[a.clone(), b.clone()]).unwrap();
        assert!(!a.exists() && !b.exists());
        assert_eq!(
            fs::read_to_string(aside(&a)).unwrap(),
            "a",
            "the stale copy is replaced"
        );

        fs::write(&a, "new a").unwrap(); // the build linked a new `a` before failing
        put_back(&moved);
        assert_eq!(
            fs::read_to_string(&a).unwrap(),
            "new a",
            "a new file is kept"
        );
        assert_eq!(
            fs::read_to_string(&b).unwrap(),
            "b",
            "a missing file comes back"
        );
    }

    #[test]
    fn a_failed_move_undoes_the_moves_already_made() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.exe");
        fs::write(&a, "a").unwrap();
        let missing = dir.path().join("missing.exe");
        assert!(move_aside(&[a.clone(), missing]).is_err());
        assert_eq!(fs::read_to_string(&a).unwrap(), "a");
    }

    #[test]
    fn summarises_cargo_errors() {
        let out = "   Compiling rl-app v0.1.0\nerror[E0425]: cannot find value `x`\n  --> src/a.rs:1:1\nerror: could not compile `rl-app`\n";
        assert_eq!(
            summary(out),
            "error[E0425]: cannot find value `x` · error: could not compile `rl-app`"
        );
        assert_eq!(summary("warning: x\n  npm failed\n\n"), "npm failed");
        assert_eq!(summary(""), "no output");
    }
}
