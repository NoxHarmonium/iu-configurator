use axum::{Extension, http::StatusCode};
use serde_json::json;

use crate::models::env::EnvironmentConfig;

pub async fn health(
    Extension(config): Extension<EnvironmentConfig>,
) -> (StatusCode, axum::Json<serde_json::Value>) {
    let mut checks: Vec<serde_json::Value> = Vec::new();
    let mut ok = true;

    // ── Check 1: CONFIG_DIR is accessible ─────────────────────────────────
    let config_dir = &config.config_dir;
    match tokio::fs::metadata(config_dir).await {
        Ok(m) if m.is_dir() => {
            checks.push(json!({ "name": "config_dir", "status": "ok", "path": config_dir }));
        }
        Ok(_) => {
            ok = false;
            checks.push(json!({
                "name": "config_dir",
                "status": "error",
                "path": config_dir,
                "detail": "path exists but is not a directory"
            }));
        }
        Err(e) => {
            ok = false;
            checks.push(json!({
                "name": "config_dir",
                "status": "error",
                "path": config_dir,
                "detail": e.to_string()
            }));
        }
    }

    // ── Check 2: Home Assistant reachable (only when token is set) ────────
    if let (Some(ha_url), Some(ha_token)) = (&config.ha_url, &config.ha_token) {
        let url = format!("{}/api/", ha_url.trim_end_matches('/'));

        match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            Ok(client) => {
                match client
                    .get(&url)
                    .header("Authorization", format!("Bearer {ha_token}"))
                    .send()
                    .await
                {
                    Ok(r) if r.status().is_success() => {
                        checks.push(json!({ "name": "home_assistant", "status": "ok" }));
                    }
                    Ok(r) => {
                        ok = false;
                        checks.push(json!({
                            "name": "home_assistant",
                            "status": "error",
                            "detail": format!("HTTP {}", r.status())
                        }));
                    }
                    Err(e) => {
                        ok = false;
                        checks.push(json!({
                            "name": "home_assistant",
                            "status": "error",
                            "detail": e.to_string()
                        }));
                    }
                }
            }
            Err(e) => {
                ok = false;
                checks.push(json!({
                    "name": "home_assistant",
                    "status": "error",
                    "detail": format!("failed to build HTTP client: {e}")
                }));
            }
        }
    }

    if !ok {
        // Failure detail otherwise only reaches the probe's HTTP response body,
        // which kubelet reads but never surfaces in `kubectl logs`.
        tracing::warn!(checks = %json!(checks), "healthz check failed");
    }

    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        axum::Json(json!({ "status": if ok { "ok" } else { "degraded" }, "checks": checks })),
    )
}
