//! Builds the UI (`ui/`) so rust-embed can include `ui/dist/phone/` and `ui/dist/desktop/`.

#[path = "build_support.rs"]
mod build_support;

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let ui = PathBuf::from(manifest).join("../../ui");
    for path in [
        "phone",
        "desktop",
        "src",
        "package.json",
        "package-lock.json",
        "tsconfig.json",
        "vite.config.ts",
        "vite.desktop.config.ts",
    ] {
        println!("cargo:rerun-if-changed={}", ui.join(path).display());
    }
    println!("cargo:rerun-if-changed=build_support.rs");
    println!("cargo:rerun-if-env-changed=RL_TRAINER_SKIP_UI_BUILD");
    println!("cargo:rerun-if-env-changed=RL_REPORT_RELAY_URL");
    let built = ["phone", "desktop"]
        .iter()
        .all(|app| ui.join("dist").join(app).join("index.html").exists());
    if std::env::var_os("RL_TRAINER_SKIP_UI_BUILD").is_some() && built {
        return;
    }
    if build_support::needs_install(&ui) {
        npm(&ui, &["ci"]);
    }
    npm(&ui, &["run", "build"]);
}

fn npm(dir: &Path, args: &[&str]) {
    let program = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|err| {
            panic!(
                "building the UI needs Node.js and npm (`{program} {}` in {}): {err}",
                args.join(" "),
                dir.display()
            )
        });
    assert!(
        status.success(),
        "`npm {}` failed in {}; run it there to see why",
        args.join(" "),
        dir.display()
    );
}
