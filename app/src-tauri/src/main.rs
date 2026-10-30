// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // started by the copy it replaces (see lib.rs `switch_copy`): give that one time to quit,
    // otherwise the single-instance guard would hand this launch straight back to it
    if std::env::args().any(|a| a == "--wait-old") {
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    idea_autopilot_lib::run()
}
