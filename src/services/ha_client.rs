//! Functions to call Home Assistant via its HTTP API

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Duration, Local, Weekday};

pub async fn reload_ha_config(ha_url: &str, ha_token: &str) -> Result<(), String> {
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
        .map_err(|e| format!("HA reload request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HA reload returned HTTP {}", response.status()));
    }

    Ok(())
}

pub async fn trigger_manual_run(
    ha_url: &str,
    ha_token: &str,
    entity_id: &str,
) -> Result<(), String> {
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
        .map_err(|e| format!("HA manual_run request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HA manual_run returned HTTP {}", response.status()));
    }

    Ok(())
}

pub async fn cancel_ha_run(ha_url: &str, ha_token: &str, entity_id: &str) -> Result<(), String> {
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
        .map_err(|e| format!("HA cancel request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HA cancel returned HTTP {}", response.status()));
    }

    Ok(())
}

pub async fn any_controller_active(
    ha_url: &str,
    ha_token: &str,
    entity_ids: &[String],
) -> Result<bool, String> {
    let base = ha_url.trim_end_matches('/');
    let client = reqwest::Client::new();

    for entity_id in entity_ids {
        let url = format!("{}/api/states/{}", base, entity_id);

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {ha_token}"))
            .send()
            .await
            .map_err(|e| format!("HA request failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("HA returned HTTP {}", response.status()));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse HA response: {e}"))?;

        if body.get("state").and_then(|s| s.as_str()) == Some("on") {
            return Ok(true);
        }
    }

    Ok(false)
}

pub async fn get_weather_forecast(
    ha_url: &str,
    ha_token: &str,
    weather_entity: &str,
) -> Result<HashMap<String, String>, String> {
    let url = format!(
        "{}/api/services/weather/get_forecasts?return_response",
        ha_url.trim_end_matches('/')
    );

    let body = serde_json::json!({
        "entity_id": weather_entity,
        "type": "daily"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {ha_token}"))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| format!("weather forecast request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "weather forecast returned HTTP {}",
            response.status()
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse weather forecast response: {e}"))?;

    let forecast_list = json
        .get("service_response")
        .and_then(|r| r.get(weather_entity))
        .and_then(|e| e.get("forecast"))
        .and_then(|f| f.as_array())
        .ok_or_else(|| format!("unexpected weather forecast response shape: {json}"))?;

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

        if let Some(e) = condition_to_emoji(condition) {
            result.insert(day_key.to_string(), e.to_string());
        }
    }

    Ok(result)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condition_to_emoji_maps_known_and_unknown() {
        assert_eq!(condition_to_emoji("sunny"), Some("☀️"));
        assert_eq!(condition_to_emoji("rainy"), Some("🌦️"));
        assert_eq!(condition_to_emoji("unknown_condition"), None);
    }
}
