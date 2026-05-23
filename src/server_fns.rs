use std::collections::HashMap;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use crate::config::Config;
#[cfg(feature = "ssr")]
use axum::Extension;

use crate::models::Schedule;

#[cfg(feature = "ssr")]
use crate::setup::IuSetup;

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

#[cfg(feature = "ssr")]
fn schedule_path(config_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(config_dir).join("iu-schedule.json")
}

#[cfg(feature = "ssr")]
fn yaml_path(config_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(config_dir).join("irrigation_unlimited.yaml")
}

#[cfg(feature = "ssr")]
async fn load_or_seed_schedule(
    config_dir: &str,
    setup: &IuSetup,
) -> Result<Schedule, ServerFnError> {
    let path = schedule_path(config_dir);

    if path.exists() {
        tracing::info!(path = %path.display(), "loading schedule from file");
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to read iu-schedule.json: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| ServerFnError::new(format!("Failed to parse iu-schedule.json: {e}")))
    } else {
        tracing::info!("no schedule file found, returning defaults");
        Ok(Schedule::default_seed_from(setup))
    }
}

#[cfg(feature = "ssr")]
async fn persist_schedule_and_yaml(
    config_dir: &str,
    schedule: &Schedule,
    setup: &IuSetup,
) -> Result<(), ServerFnError> {
    use crate::yaml_gen::generate_yaml;

    let config_path = std::path::PathBuf::from(config_dir);

    tokio::fs::create_dir_all(&config_path)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to create config dir: {e}")))?;

    let json = serde_json::to_string_pretty(schedule)
        .map_err(|e| ServerFnError::new(format!("Failed to serialise schedule: {e}")))?;
    tokio::fs::write(schedule_path(config_dir), json)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to write iu-schedule.json: {e}")))?;

    let yaml = generate_yaml(schedule, setup)
        .map_err(|e| ServerFnError::new(format!("Failed to generate YAML: {e}")))?;
    tokio::fs::write(yaml_path(config_dir), yaml)
        .await
        .map_err(|e| {
            ServerFnError::new(format!("Failed to write irrigation_unlimited.yaml: {e}"))
        })?;

    Ok(())
}

#[cfg(feature = "ssr")]
async fn save_manual_schedule(
    config_dir: &str,
    setup: &IuSetup,
    manual_zones: HashMap<String, u32>,
) -> Result<(), ServerFnError> {
    let mut schedule = load_or_seed_schedule(config_dir, setup).await?;
    schedule.manual_zones = manual_zones;
    persist_schedule_and_yaml(config_dir, &schedule, setup).await
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

/// Return the subset of iu-setup.yaml that the WASM client needs.
#[server]
pub async fn get_client_setup() -> Result<ClientSetupInfo, ServerFnError> {
    let Extension(setup) = leptos_axum::extract::<Extension<IuSetup>>()
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
pub async fn get_schedule() -> Result<Schedule, ServerFnError> {
    let Extension(config) = leptos_axum::extract::<Extension<Config>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IuSetup>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

    load_or_seed_schedule(&config.config_dir, &setup).await
}

/// Persist the schedule, regenerate the IU YAML, then call the HA reload endpoint.
///
/// HA reload is best-effort: if `HA_URL` or `HA_TOKEN` are absent (e.g., during
/// local development) the files are still written and `Ok(())` is returned.
#[server]
pub async fn save_schedule(schedule: Schedule) -> Result<(), ServerFnError> {
    let Extension(config) = leptos_axum::extract::<Extension<Config>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IuSetup>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    persist_schedule_and_yaml(&config.config_dir, &schedule, &setup).await?;

    tracing::info!("schedule saved, files written");

    // ── HA reload (best-effort) ────────────────────────────────────────────
    if let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) {
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
    use crate::setup::IuSetup;

    let Extension(config) = leptos_axum::extract::<Extension<Config>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IuSetup>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

    let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) else {
        return Ok(IrrigationStatus::Unknown(
            "HA_URL or HA_TOKEN not configured".into(),
        ));
    };

    let base = ha_url.trim_end_matches('/');
    let client = reqwest::Client::new();

    for controller in &setup.controllers {
        let url = format!("{}/api/states/{}", base, &controller.ha_master_entity);

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
                )));
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
    let has_zones = manual_zones.values().any(|&s| s > 0);
    if !has_zones {
        tracing::warn!("run_manual called with no zones enabled");
        return Err(ServerFnError::new("No zones selected for manual run"));
    }
    tracing::info!(zones = ?manual_zones, "starting manual run");

    let Extension(config) = leptos_axum::extract::<Extension<Config>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IuSetup>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    save_manual_schedule(&config.config_dir, &setup, manual_zones).await?;

    // HA calls are best-effort when env vars are absent (local dev).
    if let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) {
        let entity_id = setup
            .controllers
            .first()
            .map(|c| c.ha_master_entity.clone())
            .unwrap_or_default();
        reload_ha_config(&ha_url, &ha_token).await?;
        trigger_manual_run(&ha_url, &ha_token, &entity_id).await?;
        tracing::info!("manual run triggered in HA");
    } else {
        tracing::warn!("HA_URL/HA_TOKEN not set — skipping HA manual run");
    }

    Ok(())
}

/// Cancel any currently running irrigation sequence on the main controller.
#[server]
pub async fn cancel_run() -> Result<(), ServerFnError> {
    use crate::setup::IuSetup;

    tracing::info!("cancel_run requested");
    let Extension(config) = leptos_axum::extract::<Extension<Config>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    let Extension(setup) = leptos_axum::extract::<Extension<IuSetup>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    if let (Some(ha_url), Some(ha_token)) = (config.ha_url, config.ha_token) {
        let entity_id = setup
            .controllers
            .first()
            .map(|c| c.ha_master_entity.clone())
            .unwrap_or_default();
        cancel_ha_run(&ha_url, &ha_token, &entity_id).await?;
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
async fn trigger_manual_run(
    ha_url: &str,
    ha_token: &str,
    entity_id: &str,
) -> Result<(), ServerFnError> {
    let url = format!(
        "{}/api/services/irrigation_unlimited/manual_run",
        ha_url.trim_end_matches('/')
    );

    let body = serde_json::json!({
        "entity_id": entity_id,
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
async fn cancel_ha_run(ha_url: &str, ha_token: &str, entity_id: &str) -> Result<(), ServerFnError> {
    let url = format!(
        "{}/api/services/irrigation_unlimited/cancel",
        ha_url.trim_end_matches('/')
    );

    let body = serde_json::json!({ "entity_id": entity_id });

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
    use chrono::{DateTime, Datelike, Duration, Local, Weekday};

    let Extension(config) = leptos_axum::extract::<Extension<Config>>()
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

    let (Some(ha_url), Some(ha_token), Some(weather_entity)) =
        (config.ha_url, config.ha_token, config.ha_weather_entity)
    else {
        return Ok(HashMap::new());
    };

    let url = format!(
        "{}/api/services/weather/get_forecasts?return_response",
        ha_url.trim_end_matches('/')
    );

    let body = serde_json::json!({
        "entity_id": weather_entity,
        "type": "daily"
    });

    let response = match reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {ha_token}"))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("weather forecast request failed: {e}");
            return Ok(HashMap::new());
        }
    };

    if !response.status().is_success() {
        tracing::warn!("weather forecast returned HTTP {}", response.status());
        return Ok(HashMap::new());
    }

    let json: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to parse weather forecast response: {e}");
            return Ok(HashMap::new());
        }
    };

    // The REST API with ?return_response returns:
    // { "changed_states": [], "service_response": { "<entity_id>": { "forecast": [...] } } }
    let forecast_list = json
        .get("service_response")
        .and_then(|r| r.get(&weather_entity))
        .and_then(|e| e.get("forecast"))
        .and_then(|f| f.as_array());

    let Some(forecast_list) = forecast_list else {
        tracing::warn!(response = %json, "unexpected weather forecast response shape");
        return Ok(HashMap::new());
    };

    let today = Local::now().date_naive();
    let window_end = today + Duration::days(7);

    let mut result = HashMap::new();

    for entry in forecast_list {
        let Some(datetime_str) = entry.get("datetime").and_then(|d| d.as_str()) else {
            continue;
        };
        let Some(condition) = entry.get("condition").and_then(|c| c.as_str()) else {
            continue;
        };

        // Parse ISO 8601 — BOM returns timezone-aware strings.
        let date = if let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str) {
            dt.date_naive()
        } else if let Ok(d) = chrono::NaiveDate::parse_from_str(datetime_str, "%Y-%m-%d") {
            d
        } else {
            tracing::warn!("could not parse forecast datetime: {datetime_str}");
            continue;
        };

        if date < today || date >= window_end {
            continue;
        }

        let day_key = match date.weekday() {
            Weekday::Mon => "mon",
            Weekday::Tue => "tue",
            Weekday::Wed => "wed",
            Weekday::Thu => "thu",
            Weekday::Fri => "fri",
            Weekday::Sat => "sat",
            Weekday::Sun => "sun",
        };

        let emoji = condition_to_emoji(condition);
        if let Some(e) = emoji {
            result.insert(day_key.to_string(), e.to_string());
        }
    }

    Ok(result)
}

#[cfg(feature = "ssr")]
fn condition_to_emoji(condition: &str) -> Option<&'static str> {
    match condition {
        "sunny" | "clear-night" => Some("☀️"),
        "partlycloudy" => Some("⛅"),
        "cloudy" => Some("☁️"),
        "fog" => Some("🌫️"),
        "rainy" => Some("🌦️"),
        "pouring" => Some("🌧️"),
        "snowy" => Some("❄️"),
        "snowy-rainy" | "hail" => Some("🌨️"),
        "lightning" | "lightning-rainy" => Some("⛈️"),
        "windy" | "windy-variant" => Some("💨"),
        "exceptional" => Some("⚠️"),
        _ => None,
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    fn test_setup() -> IuSetup {
        let yaml = include_str!("../dev/config/iu-setup.yaml");
        serde_yaml::from_str(yaml).expect("dev/config/iu-setup.yaml failed to parse")
    }

    fn test_dir(prefix: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir();
        let unique = format!(
            "iu-configurator-{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock went backwards")
                .as_nanos()
        );
        base.join(unique)
    }

    #[test]
    fn load_or_seed_returns_defaults_when_file_missing() {
        let setup = test_setup();
        let config_dir = test_dir("seed-missing");
        std::fs::create_dir_all(&config_dir).expect("failed to create temp dir");

        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        let schedule = rt
            .block_on(load_or_seed_schedule(
                config_dir.to_str().unwrap_or(""),
                &setup,
            ))
            .expect("load_or_seed_schedule failed");

        let expected = Schedule::default_seed_from(&setup);
        assert_eq!(schedule.morning_time, expected.morning_time);
        assert_eq!(schedule.afternoon_time, expected.afternoon_time);
        assert_eq!(schedule.zones.len(), expected.zones.len());
        assert!(schedule.zones.contains_key("zone_1"));

        let _ = std::fs::remove_dir_all(config_dir);
    }

    #[test]
    fn persist_schedule_and_yaml_writes_both_files() {
        let setup = test_setup();
        let config_dir = test_dir("persist");
        let schedule = Schedule::default_seed_from(&setup);

        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(persist_schedule_and_yaml(
            config_dir.to_str().unwrap_or(""),
            &schedule,
            &setup,
        ))
        .expect("persist_schedule_and_yaml failed");

        let schedule_file = schedule_path(config_dir.to_str().unwrap_or(""));
        let yaml_file = yaml_path(config_dir.to_str().unwrap_or(""));

        assert!(schedule_file.exists(), "expected iu-schedule.json to exist");
        assert!(
            yaml_file.exists(),
            "expected irrigation_unlimited.yaml to exist"
        );

        let json_text = std::fs::read_to_string(&schedule_file).expect("read schedule json failed");
        let parsed: Schedule =
            serde_json::from_str(&json_text).expect("parse schedule json failed");
        assert_eq!(parsed.morning_time, schedule.morning_time);
        assert_eq!(parsed.zones.len(), schedule.zones.len());
        assert!(parsed.zones.contains_key("zone_1"));

        let yaml_text = std::fs::read_to_string(&yaml_file).expect("read yaml failed");
        assert!(yaml_text.contains("controllers:"));
        assert!(yaml_text.contains("zones:"));

        let _ = std::fs::remove_dir_all(config_dir);
    }

    #[test]
    fn save_manual_schedule_updates_manual_zones() {
        let setup = test_setup();
        let config_dir = test_dir("manual");

        let mut manual = HashMap::new();
        manual.insert("zone_1".to_string(), 120);
        manual.insert("zone_2".to_string(), 0);

        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(save_manual_schedule(
            config_dir.to_str().unwrap_or(""),
            &setup,
            manual.clone(),
        ))
        .expect("save_manual_schedule failed");

        let loaded = rt
            .block_on(load_or_seed_schedule(
                config_dir.to_str().unwrap_or(""),
                &setup,
            ))
            .expect("load_or_seed_schedule after save failed");

        assert_eq!(loaded.manual_zones, manual);

        let _ = std::fs::remove_dir_all(config_dir);
    }
}
