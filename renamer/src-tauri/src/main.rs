#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // If any CLI args are present, act as CLI; otherwise launch the GUI.
    let has_args = std::env::args().count() > 1
        && std::env::args().nth(1).map(|a| !a.starts_with('-') || a == "--help" || a == "--version" || a == "--apply" || a == "--dry-run").unwrap_or(false)
        && std::env::args().nth(1).map(|a| a == "preview" || a == "undo" || a == "presets" || a == "--help" || a == "--version" || (a.starts_with('-') && a != "--apply" && a != "--dry-run" && !a.starts_with("--preset"))).unwrap_or(false);

    if has_args {
        std::process::exit(renamer_lib::cli::run());
    }
    renamer_lib::run();
}
