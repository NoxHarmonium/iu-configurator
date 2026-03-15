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

/// Load the current schedule from `$CONFIG_DIR/schedule.json`.
/// Returns seeded defaults if the file does not yet exist.
#[server]
pub async fn get_schedule() -> Result<Schedule, ServerFnError> {
    use std::path::PathBuf;

    let config_dir = std::env::var("CONFIG_DIR").unwrap_or_else(|_| "/config".into());
    let path = PathBuf::from(&config_dir).join("schedule.json");

    if path.exists() {
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to read schedule.json: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| ServerFnError::new(format!("Failed to parse schedule.json: {e}")))
    } else {
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

    // ── schedule.json ──────────────────────────────────────────────────────
    let json = serde_json::to_string_pretty(&schedule)
        .map_err(|e| ServerFnError::new(format!("Failed to serialise schedule: {e}")))?;
    tokio::fs::write(config_path.join("schedule.json"), json)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to write schedule.json: {e}")))?;

    // ── irrigation_unlimited.yaml ──────────────────────────────────────────
    let yaml = generate_yaml(&schedule)
        .map_err(|e| ServerFnError::new(format!("Failed to generate YAML: {e}")))?;
    tokio::fs::write(config_path.join("irrigation_unlimited.yaml"), yaml)
        .await
        .map_err(|e| {
            ServerFnError::new(format!("Failed to write irrigation_unlimited.yaml: {e}"))
        })?;

    // ── HA reload (best-effort) ────────────────────────────────────────────
    if let (Ok(ha_url), Ok(ha_token)) = (std::env::var("HA_URL"), std::env::var("HA_TOKEN")) {
        reload_ha_config(&ha_url, &ha_token).await?;
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
