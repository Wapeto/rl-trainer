//! Shared by `build.rs` and the crate's tests: when the UI's npm packages need installing.

use std::path::Path;

/// `npm ci` is needed when nothing is installed or `package-lock.json` changed since the last
/// install (npm writes `node_modules/.package-lock.json` on every install).
pub fn needs_install(ui: &Path) -> bool {
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let lock = modified(&ui.join("package-lock.json"));
    let installed = modified(&ui.join("node_modules").join(".package-lock.json"));
    match (lock, installed) {
        (Some(lock), Some(installed)) => lock > installed,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::needs_install;
    use std::fs::{self, File};
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    fn touch(path: &Path, at: SystemTime) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        File::create(path).unwrap().set_modified(at).unwrap();
    }

    #[test]
    fn npm_ci_runs_when_the_lockfile_is_newer_than_the_install() {
        let dir = tempfile::tempdir().unwrap();
        let ui = dir.path();
        assert!(needs_install(ui), "nothing installed");
        let then = SystemTime::now() - Duration::from_secs(3600);
        touch(&ui.join("package-lock.json"), then);
        touch(
            &ui.join("node_modules").join(".package-lock.json"),
            then + Duration::from_secs(60),
        );
        assert!(
            !needs_install(ui),
            "installed after the lockfile last changed"
        );
        touch(
            &ui.join("package-lock.json"),
            then + Duration::from_secs(120),
        );
        assert!(needs_install(ui), "the lockfile changed since the install");
    }
}
