use std::collections::HashMap;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use crate::models::env::EnvironmentConfig;
#[cfg(feature = "ssr")]
use axum::Extension;

use crate::models::AppState;

#[cfg(feature = "ssr")]
use crate::models::IUCConfig;

/// Whether any irrigation controller is currently running.
///
/// This type crosses the wire (server → client) so it must be de/serializable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrrigationStatus {
    /// At least one controller master sensor reports state = "on".
    Active,
    /// All controller master sensors report state = "off".
    Idle,
    /// Status could not be determined — HA unreachable or env vars absent.
    Unknown(String),
}

/// Minimal zone info sent to the WASM client — no HA entity IDs exposed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientZoneInfo {
    pub id: String,
    pub name: String,
}

/// Setup data the client needs to render the UI dynamically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientSetupInfo {
    pub zones: Vec<ClientZoneInfo>,
    pub poll_interval_ms: u64,
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

/// Return the subset of iuc-config.yaml that the WASM client needs.
#[server]
pub async fn get_client_setup() -> Result<ClientSetupInfo, ServerFnError> {
    let Extension(setup) = leptos_axum::extract::<Extension<IUCConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
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
    let Extension(config) = leptos_axum::extract::<Extension<EnvironmentConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IUCConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

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
    let Extension(config) = leptos_axum::extract::<Extension<EnvironmentConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IUCConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    crate::services::schedule::persist_app_state_and_yaml(&config.config_dir, &schedule, &setup)
        .await
        .map_err(ServerFnError::new)?;

    tracing::info!("schedule saved, files written");

    // ── HA reload (best-effort) ────────────────────────────────────────────
    if let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) {
        crate::services::ha_client::reload_ha_config(&ha_url, &ha_token)
            .await
            .map_err(ServerFnError::new)?;
    } else {
        tracing::warn!("HA_URL/HA_TOKEN not set — skipping HA reload");
    }

    Ok(())
}

/// Check whether any controller master sensor is currently reporting "on".
///
/// Never returns `Err` — connectivity or configuration issues are surfaced as
/// `IrrigationStatus::Unknown` so the UI degrades gracefully.
#[server]
pub async fn get_irrigation_status() -> Result<IrrigationStatus, ServerFnError> {
    let Extension(config) = leptos_axum::extract::<Extension<EnvironmentConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IUCConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

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

    let Extension(config) = leptos_axum::extract::<Extension<EnvironmentConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IUCConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    crate::services::schedule::save_manual_schedule(&config.config_dir, &setup, manual_zones)
        .await
        .map_err(ServerFnError::new)?;

    // HA calls are best-effort when env vars are absent (local dev).
    if let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) {
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
    } else {
        tracing::warn!("HA_URL/HA_TOKEN not set — skipping HA manual run");
    }

    Ok(())
}

/// Cancel any currently running irrigation sequence on the main controller.
#[server]
pub async fn cancel_run() -> Result<(), ServerFnError> {
    tracing::info!("cancel_run requested");
    let Extension(config) = leptos_axum::extract::<Extension<EnvironmentConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IUCConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    if let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) {
        let entity_id = setup
            .controllers
            .first()
            .map(|c| c.ha_master_entity.clone())
            .unwrap_or_default();
        crate::services::ha_client::cancel_ha_run(&ha_url, &ha_token, &entity_id)
            .await
            .map_err(ServerFnError::new)?;
        tracing::info!("irrigation cancelled in HA");
    } else {
        tracing::warn!("HA_URL/HA_TOKEN not set — cancel is a no-op");
    }
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
    let Extension(config) = leptos_axum::extract::<Extension<EnvironmentConfig>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

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
