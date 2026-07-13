#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> Result<(), desktop_app::StartupError> {
    desktop_app::run()
}
