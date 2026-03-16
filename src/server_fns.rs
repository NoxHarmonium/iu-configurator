use std::collections::HashMap;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::Schedule;

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

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

/// Load the current schedule from `$CONFIG_DIR/iu-schedule.json`.
/// Returns seeded defaults if the file does not yet exist.
#[server]
pub async fn get_schedule() -> Result<Schedule, ServerFnError> {
    use std::path::PathBuf;

    let config_dir = std::env::var("CONFIG_DIR").unwrap_or_else(|_| "/config".into());
    let path = PathBuf::from(&config_dir).join("iu-schedule.json");

    if path.exists() {
        tracing::info!(path = %path.display(), "loading schedule from file");
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to read iu-schedule.json: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| ServerFnError::new(format!("Failed to parse iu-schedule.json: {e}")))
    } else {
        tracing::info!("no schedule file found, returning defaults");
        Ok(Schedule::default_seed())
    }
}

/// Persist the schedule, regenerate the IU YAML, then call the HA reload endpoint.
///
/// HA reload is best-effort: if `HA_URL` or `HA_TOKEN` are absent (e.g., during
/// local development) the files are still written and `Ok(())` is returned.
#[server]
pub async fn save_schedule(schedule: Schedule) -> Result<(), ServerFnError> {
    use std::path::PathBuf;

    use crate::yaml_gen::generate_yaml;

    let config_dir = std::env::var("CONFIG_DIR").unwrap_or_else(|_| "/config".into());
    let config_path = PathBuf::from(&config_dir);

    // Ensure the target directory exists (important on first run in a new container).
    tokio::fs::create_dir_all(&config_path)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to create config dir: {e}")))?;

    // ── iu-schedule.json ──────────────────────────────────────────────────────
    let json = serde_json::to_string_pretty(&schedule)
        .map_err(|e| ServerFnError::new(format!("Failed to serialise schedule: {e}")))?;
    tokio::fs::write(config_path.join("iu-schedule.json"), json)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to write iu-schedule.json: {e}")))?;

    // ── irrigation_unlimited.yaml ──────────────────────────────────────────
    let yaml = generate_yaml(&schedule)
        .map_err(|e| ServerFnError::new(format!("Failed to generate YAML: {e}")))?;
    tokio::fs::write(config_path.join("irrigation_unlimited.yaml"), yaml)
        .await
        .map_err(|e| {
            ServerFnError::new(format!("Failed to write irrigation_unlimited.yaml: {e}"))
        })?;

    tracing::info!("schedule saved, files written");

    // ── HA reload (best-effort) ────────────────────────────────────────────
    if let (Ok(ha_url), Ok(ha_token)) = (std::env::var("HA_URL"), std::env::var("HA_TOKEN")) {
        reload_ha_config(&ha_url, &ha_token).await?;
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
    use crate::definitions::CONTROLLERS;

    let (ha_url, ha_token) = match (std::env::var("HA_URL"), std::env::var("HA_TOKEN")) {
        (Ok(u), Ok(t)) => (u, t),
        _ => {
            return Ok(IrrigationStatus::Unknown(
                "HA_URL or HA_TOKEN not configured".into(),
            ))
        }
    };

    let base = ha_url.trim_end_matches('/');
    let client = reqwest::Client::new();

    for controller in CONTROLLERS {
        let url = format!("{}/api/states/{}", base, controller.ha_master_entity);

        let response = match client
            .get(&url)
            .header("Authorization", format!("Bearer {ha_token}"))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return Ok(IrrigationStatus::Unknown(format!("HA request failed: {e}"))),
        };

        if !response.status().is_success() {
            return Ok(IrrigationStatus::Unknown(format!(
                "HA returned HTTP {}",
                response.status()
            )));
        }

        let body: serde_json::Value = match response.json().await {
            Ok(v) => v,
            Err(e) => {
                return Ok(IrrigationStatus::Unknown(format!(
                    "Failed to parse HA response: {e}"
                )))
            }
        };

        if body.get("state").and_then(|s| s.as_str()) == Some("on") {
            return Ok(IrrigationStatus::Active);
        }
    }

    Ok(IrrigationStatus::Idle)
}

/// Update manual_zones in the persisted schedule, regenerate YAML, reload HA,
/// then trigger the manual sequence via the irrigation_unlimited.manual_run service.
#[server]
pub async fn run_manual(manual_zones: HashMap<String, u32>) -> Result<(), ServerFnError> {
    use std::path::PathBuf;

    use crate::yaml_gen::generate_yaml;

    let has_zones = manual_zones.values().any(|&s| s > 0);
    if !has_zones {
        tracing::warn!("run_manual called with no zones enabled");
        return Err(ServerFnError::new("No zones selected for manual run"));
    }
    tracing::info!(zones = ?manual_zones, "starting manual run");

    let config_dir = std::env::var("CONFIG_DIR").unwrap_or_else(|_| "/config".into());
    let config_path = PathBuf::from(&config_dir);

    tokio::fs::create_dir_all(&config_path)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to create config dir: {e}")))?;

    // Read current schedule and update manual_zones.
    let path = config_path.join("iu-schedule.json");
    let mut schedule: Schedule = if path.exists() {
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to read iu-schedule.json: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| ServerFnError::new(format!("Failed to parse iu-schedule.json: {e}")))?
    } else {
        Schedule::default_seed()
    };

    schedule.manual_zones = manual_zones;

    let json = serde_json::to_string_pretty(&schedule)
        .map_err(|e| ServerFnError::new(format!("Failed to serialise schedule: {e}")))?;
    tokio::fs::write(config_path.join("iu-schedule.json"), json)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to write iu-schedule.json: {e}")))?;

    let yaml = generate_yaml(&schedule)
        .map_err(|e| ServerFnError::new(format!("Failed to generate YAML: {e}")))?;
    tokio::fs::write(config_path.join("irrigation_unlimited.yaml"), yaml)
        .await
        .map_err(|e| {
            ServerFnError::new(format!("Failed to write irrigation_unlimited.yaml: {e}"))
        })?;

    // HA calls are best-effort when env vars are absent (local dev).
    if let (Ok(ha_url), Ok(ha_token)) = (std::env::var("HA_URL"), std::env::var("HA_TOKEN")) {
        reload_ha_config(&ha_url, &ha_token).await?;
        trigger_manual_run(&ha_url, &ha_token).await?;
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
    if let (Ok(ha_url), Ok(ha_token)) = (std::env::var("HA_URL"), std::env::var("HA_TOKEN")) {
        cancel_ha_run(&ha_url, &ha_token).await?;
        tracing::info!("irrigation cancelled in HA");
    } else {
        tracing::warn!("HA_URL/HA_TOKEN not set — cancel is a no-op");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// SSR-only helpers (not server functions — called from within server fn bodies)
// ---------------------------------------------------------------------------

#[cfg(feature = "ssr")]
async fn reload_ha_config(ha_url: &str, ha_token: &str) -> Result<(), ServerFnError> {
    let url = format!(
        "{}/api/services/irrigation_unlimited/reload",
        ha_url.trim_end_matches('/')
    );

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {ha_token}"))
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .map_err(|e| ServerFnError::new(format!("HA reload request failed: {e}")))?;

    // HA returns 200 with a JSON body on success; anything else is an error.
    if !response.status().is_success() {
        return Err(ServerFnError::new(format!(
            "HA reload returned HTTP {}",
            response.status()
        )));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
async fn trigger_manual_run(ha_url: &str, ha_token: &str) -> Result<(), ServerFnError> {
    let url = format!(
        "{}/api/services/irrigation_unlimited/manual_run",
        ha_url.trim_end_matches('/')
    );

    // TODO: Don't hardcode entity IDs
    let body = serde_json::json!({
        "entity_id": "binary_sensor.irrigation_unlimited_c1_m",
        "sequence_id": "manual"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {ha_token}"))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| ServerFnError::new(format!("HA manual_run request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(ServerFnError::new(format!(
            "HA manual_run returned HTTP {}",
            response.status()
        )));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
async fn cancel_ha_run(ha_url: &str, ha_token: &str) -> Result<(), ServerFnError> {
    let url = format!(
        "{}/api/services/irrigation_unlimited/cancel",
        ha_url.trim_end_matches('/')
    );

    let body = serde_json::json!({
        "entity_id": "binary_sensor.irrigation_unlimited_c1_m"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {ha_token}"))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| ServerFnError::new(format!("HA cancel request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(ServerFnError::new(format!(
            "HA cancel returned HTTP {}",
            response.status()
        )));
    }

    Ok(())
}
