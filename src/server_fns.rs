use std::collections::HashMap;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::AppState;

#[cfg(feature = "ssr")]
use crate::models::ServerConfig;

/// Whether any irrigation controller is currently running.
///
/// This type crosses the wire (server → client) so it must be de/serializable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrrigationStatus {
    /// At least one controller master sensor reports state = "on".
    Active,
    /// All controller master sensors report state = "off".
    Idle,
    /// Status could not be determined — HA unreachable or env vars absent.
    Unknown(String),
}

/// Minimal zone info sent to the WASM client — no HA entity IDs exposed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientZoneInfo {
    pub id: String,
    pub name: String,
}

/// Setup data the client needs to render the UI dynamically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSetupInfo {
    pub zones: Vec<ClientZoneInfo>,
    pub poll_interval_ms: u64,
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

/// Return the subset of iuc-config.yaml that the WASM client needs.
#[allow(clippy::unused_async)] // #[server] requires `async`; no await points needed here
#[server]
pub async fn get_client_setup() -> Result<ClientSetupInfo, ServerFnError> {
    let ServerConfig { setup, .. } = expect_context::<ServerConfig>();
    Ok(ClientSetupInfo {
        zones: setup
            .zones
            .iter()
            .map(|z| ClientZoneInfo {
                id: z.id.clone(),
                name: z.name.clone(),
            })
            .collect(),
        poll_interval_ms: setup.poll_interval_ms,
    })
}

/// Load the current schedule from `$CONFIG_DIR/iu-schedule.json`.
/// Returns seeded defaults if the file does not yet exist.
#[server]
pub async fn get_schedule() -> Result<AppState, ServerFnError> {
    let ServerConfig { config, setup } = expect_context::<ServerConfig>();

    crate::repositories::app_state::load_or_seed_app_state(&config.config_dir, &setup)
        .await
        .map_err(ServerFnError::new)
}

/// Persist the schedule, regenerate the IU YAML, then call the HA reload endpoint.
///
/// HA reload is best-effort: if `HA_URL` or `HA_TOKEN` are absent (e.g., during
/// local development) the files are still written and `Ok(())` is returned.
#[server]
pub async fn save_schedule(schedule: AppState) -> Result<(), ServerFnError> {
    // TODO: Validate schedule before persisting — morning_time/afternoon_time must match
    // HH:MM format; zone morning_secs/afternoon_secs must be in [0, 86400].
    let ServerConfig { config, setup } = expect_context::<ServerConfig>();
    crate::services::schedule::persist_app_state_and_yaml(&config.config_dir, &schedule, &setup)
        .await
        .map_err(ServerFnError::new)?;

    tracing::info!("schedule saved, files written");

    // ── HA reload (best-effort) ────────────────────────────────────────────
    // TODO: Extract ha_credentials(config: &EnvironmentConfig) -> Option<(String, String)> —
    // this let-else pattern is repeated in save_schedule, get_irrigation_status,
    // run_manual, cancel_run, and get_weather_forecast.
    let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) else {
        tracing::warn!("HA_URL/HA_TOKEN not set — skipping HA reload");
        return Ok(());
    };
    crate::services::ha_client::reload_ha_config(&ha_url, &ha_token)
        .await
        .map_err(ServerFnError::new)?;

    Ok(())
}

/// Check whether any controller master sensor is currently reporting "on".
///
/// Never returns `Err` — connectivity or configuration issues are surfaced as
/// `IrrigationStatus::Unknown` so the UI degrades gracefully.
#[server]
pub async fn get_irrigation_status() -> Result<IrrigationStatus, ServerFnError> {
    let ServerConfig { config, setup } = expect_context::<ServerConfig>();

    let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) else {
        return Ok(IrrigationStatus::Unknown(
            "HA_URL or HA_TOKEN not configured".into(),
        ));
    };

    let entity_ids: Vec<String> = setup
        .controllers
        .iter()
        .map(|c| c.ha_master_entity.clone())
        .collect();

    match crate::services::ha_client::any_controller_active(&ha_url, &ha_token, &entity_ids).await {
        Ok(true) => Ok(IrrigationStatus::Active),
        Ok(false) => Ok(IrrigationStatus::Idle),
        Err(e) => Ok(IrrigationStatus::Unknown(e)),
    }
}

/// Update manual_zones in the persisted schedule, regenerate YAML, reload HA,
/// then trigger the manual sequence via the irrigation_unlimited.manual_run service.
#[server]
pub async fn run_manual(manual_zones: HashMap<String, u32>) -> Result<(), ServerFnError> {
    let has_zones = manual_zones.values().any(|&s| s > 0);
    if !has_zones {
        tracing::warn!("run_manual called with no zones enabled");
        return Err(ServerFnError::new("No zones selected for manual run"));
    }
    tracing::info!(zones = ?manual_zones, "starting manual run");

    let ServerConfig { config, setup } = expect_context::<ServerConfig>();
    crate::services::schedule::save_manual_schedule(&config.config_dir, &setup, manual_zones)
        .await
        .map_err(ServerFnError::new)?;

    // HA calls are best-effort when env vars are absent (local dev).
    let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) else {
        tracing::warn!("HA_URL/HA_TOKEN not set — skipping HA manual run");
        return Ok(());
    };
    // TODO: Extract primary_entity(setup: &IUCConfig) -> String helper —
    // this first-controller lookup is duplicated in run_manual and cancel_run.
    let entity_id = setup
        .controllers
        .first()
        .map(|c| c.ha_master_entity.clone())
        .unwrap_or_default();
    crate::services::ha_client::reload_ha_config(&ha_url, &ha_token)
        .await
        .map_err(ServerFnError::new)?;
    crate::services::ha_client::trigger_manual_run(&ha_url, &ha_token, &entity_id)
        .await
        .map_err(ServerFnError::new)?;
    tracing::info!("manual run triggered in HA");

    Ok(())
}

/// Cancel any currently running irrigation sequence on the main controller.
#[server]
pub async fn cancel_run() -> Result<(), ServerFnError> {
    tracing::info!("cancel_run requested");
    let ServerConfig { config, setup } = expect_context::<ServerConfig>();
    let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) else {
        tracing::warn!("HA_URL/HA_TOKEN not set — cancel is a no-op");
        return Ok(());
    };
    let entity_id = setup
        .controllers
        .first()
        .map(|c| c.ha_master_entity.clone())
        .unwrap_or_default();
    crate::services::ha_client::cancel_ha_run(&ha_url, &ha_token, &entity_id)
        .await
        .map_err(ServerFnError::new)?;
    tracing::info!("irrigation cancelled in HA");
    Ok(())
}

// ---------------------------------------------------------------------------
// Weather forecast
// ---------------------------------------------------------------------------

/// Fetch the daily weather forecast from Home Assistant and return a mapping of
/// weekday key (e.g. `"mon"`) to a Unicode weather emoji for the next 7 days.
///
/// Returns an empty map — never `Err` — when `HA_WEATHER_ENTITY` is not set,
/// HA is unreachable, or any parsing step fails, so the UI degrades silently.
#[server]
pub async fn get_weather_forecast() -> Result<HashMap<String, String>, ServerFnError> {
    let ServerConfig { config, .. } = expect_context::<ServerConfig>();

    let (Some(ha_url), Some(ha_token), Some(weather_entity)) =
        (config.ha_url, config.ha_token, config.ha_weather_entity)
    else {
        return Ok(HashMap::new());
    };

    match crate::services::ha_client::get_weather_forecast(&ha_url, &ha_token, &weather_entity)
        .await
    {
        Ok(map) => Ok(map),
        Err(e) => {
            tracing::warn!("{e}");
            Ok(HashMap::new())
        }
    }
}
