use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_updater::UpdaterExt as _;

use crate::state::HostState;

const UPDATE_ENDPOINT: Option<&str> = option_env!("SAPPHIRUS_UPDATE_ENDPOINT");
const UPDATE_PUBLIC_KEY: Option<&str> = option_env!("SAPPHIRUS_UPDATE_PUBLIC_KEY");
static UPDATE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

struct UpdateGuard;

impl UpdateGuard {
    fn acquire() -> Result<Self, String> {
        UPDATE_IN_PROGRESS
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| "An app update is already in progress.".to_owned())
    }
}

impl Drop for UpdateGuard {
    fn drop(&mut self) {
        UPDATE_IN_PROGRESS.store(false, Ordering::Release);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum AppUpdateResult {
    Disabled,
    Current { version: String },
    Installed { version: String },
}

#[tauri::command]
pub async fn install_app_update(
    app: AppHandle,
    state: State<'_, HostState>,
) -> Result<AppUpdateResult, String> {
    let _update_guard = UpdateGuard::acquire()?;
    let Some(endpoint) = UPDATE_ENDPOINT.filter(|value| !value.trim().is_empty()) else {
        return Ok(AppUpdateResult::Disabled);
    };
    let Some(public_key) = UPDATE_PUBLIC_KEY.filter(|value| !value.trim().is_empty()) else {
        return Ok(AppUpdateResult::Disabled);
    };

    ensure_update_is_safe(&state)?;

    let endpoint = endpoint
        .parse::<tauri::Url>()
        .map_err(|_| update_failed())?;
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|_| update_failed())?
        .pubkey(public_key)
        .build()
        .map_err(|_| update_failed())?;
    let Some(update) = updater.check().await.map_err(|_| update_failed())? else {
        return Ok(AppUpdateResult::Current {
            version: app.package_info().version.to_string(),
        });
    };

    let version = update.version.clone();
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|_| update_failed())?;

    // The download may take long enough for recovery or a governed effect to
    // begin, so authorization must be refreshed immediately before handoff.
    ensure_update_is_safe(&state)?;
    update.install(bytes).map_err(|_| update_failed())?;

    Ok(AppUpdateResult::Installed { version })
}

fn ensure_update_is_safe(state: &HostState) -> Result<(), String> {
    let authority = state.ready_authority().map_err(|_| update_blocked())?;
    let store = state
        .local_store(&authority)
        .map_err(|_| update_blocked())?;
    let open_journals = store
        .list_open_effect_journals()
        .map_err(|_| update_blocked())?;
    if open_journals.is_empty() {
        Ok(())
    } else {
        Err(update_blocked())
    }
}

fn update_blocked() -> String {
    "The app cannot update while local work or recovery is active.".to_owned()
}

fn update_failed() -> String {
    "The app update could not be completed. Try again later.".to_owned()
}

#[cfg(test)]
mod tests {
    use super::{update_blocked, update_failed, UpdateGuard};

    #[test]
    fn update_errors_are_safe_and_actionable() {
        assert_eq!(
            update_blocked(),
            "The app cannot update while local work or recovery is active."
        );
        assert_eq!(
            update_failed(),
            "The app update could not be completed. Try again later."
        );
    }

    #[test]
    fn only_one_update_can_run_at_a_time() -> Result<(), String> {
        let guard = UpdateGuard::acquire()?;
        assert_eq!(
            UpdateGuard::acquire().err().as_deref(),
            Some("An app update is already in progress.")
        );
        drop(guard);
        assert!(UpdateGuard::acquire().is_ok());
        Ok(())
    }
}
