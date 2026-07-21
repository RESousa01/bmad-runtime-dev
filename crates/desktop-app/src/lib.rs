#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

#[allow(
    dead_code,
    reason = "the Checkpoint 3 bridge is consumed by the Checkpoint 5 coordinator"
)]
mod bmad_capability_host;
pub mod bmad_foundation;
mod bmad_governed_proposal;
mod bmad_model;
mod commands;
mod edits;
#[allow(
    dead_code,
    reason = "the Task 3 recovery host composition is consumed by the Task 4 command boundary"
)]
mod recovery;
mod state;
mod update;
mod wire;

use tauri::Manager as _;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Sapphirus desktop startup failed")]
pub struct StartupError;

/// Starts the native desktop host and its single local renderer window.
///
/// # Errors
///
/// Returns [`StartupError`] when host state, the renderer window, or the
/// Tauri event loop cannot be initialized.
pub fn run() -> Result<(), StartupError> {
    tauri::Builder::default()
        // Registered first so a second launch reaches the callback before
        // any other initialization: the duplicate process exits and the
        // existing window is surfaced instead of racing for the local
        // authority store.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let foundation_root = app
                .path()
                .resource_dir()
                .map_err(|_| Box::new(StartupError) as Box<dyn std::error::Error>)?
                .join("bmad-foundation");
            let foundation = bmad_foundation::load_bmad_foundation(foundation_root)
                .map_err(|_| Box::new(StartupError) as Box<dyn std::error::Error>)?;
            if !app.manage(foundation) {
                return Err(Box::new(StartupError) as Box<dyn std::error::Error>);
            }
            let storage_root = app
                .path()
                .app_local_data_dir()
                .ok()
                .map(|path| path.join("authority-v1"));
            let state = state::HostState::initialize(storage_root)
                .map_err(|_| Box::new(StartupError) as Box<dyn std::error::Error>)?;
            if !app.manage(state) {
                return Err(Box::new(StartupError) as Box<dyn std::error::Error>);
            }

            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Sapphirus")
            .inner_size(1440.0, 900.0)
            .min_inner_size(1100.0, 700.0)
            .center()
            .decorations(false)
            .resizable(true)
            .maximizable(true)
            .minimizable(true)
            .closable(true)
            .fullscreen(false)
            .always_on_top(false)
            .visible(true)
            .on_navigation(is_allowed_navigation)
            .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
            .build()
            .map_err(|_| Box::new(StartupError) as Box<dyn std::error::Error>)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::host_bootstrap,
            commands::host_dispatch,
            commands::host_projection_snapshot,
            commands::host_projection_events,
            update::install_app_update,
        ])
        .run(tauri::generate_context!())
        .map_err(|_| StartupError)
}

fn is_allowed_navigation(url: &tauri::Url) -> bool {
    let packaged = url.scheme() == "tauri"
        || (matches!(url.scheme(), "http" | "https") && url.host_str() == Some("tauri.localhost"));
    let local_development = cfg!(debug_assertions)
        && url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port() == Some(1420);
    packaged || local_development
}

#[cfg(test)]
mod tests {
    use super::is_allowed_navigation;

    #[test]
    fn navigation_guard_allows_only_packaged_or_exact_development_origin() {
        let packaged = tauri::Url::parse("http://tauri.localhost/index.html");
        let remote = tauri::Url::parse("https://example.com/");
        let lookalike = tauri::Url::parse("http://tauri.localhost.example.com/");

        assert!(packaged.is_ok_and(|url| is_allowed_navigation(&url)));
        assert!(remote.is_ok_and(|url| !is_allowed_navigation(&url)));
        assert!(lookalike.is_ok_and(|url| !is_allowed_navigation(&url)));
    }
}
