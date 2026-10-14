fn main() {
    // The icons are baked into the exe (Windows resource, window and tray icon), but
    // tauri-build only watches tauri.conf.json: without this, new icons never land.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build();
}
