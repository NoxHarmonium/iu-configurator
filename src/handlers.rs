use axum::http::StatusCode;
use serde_json::json;

pub async fn health() -> (StatusCode, axum::Json<serde_json::Value>) {
    let mut checks: Vec<serde_json::Value> = Vec::new();
    let mut ok = true;

    // ── Check 1: CONFIG_DIR is accessible ─────────────────────────────────
    let config_dir = std::env::var("CONFIG_DIR").unwrap_or_else(|_| "/config".into());
    match tokio::fs::metadata(&config_dir).await {
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
    if let (Ok(ha_url), Ok(ha_token)) = (std::env::var("HA_URL"), std::env::var("HA_TOKEN")) {
        let url = format!("{}/api/", ha_url.trim_end_matches('/'));
        let result = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
            .get(&url)
            .header("Authorization", format!("Bearer {ha_token}"))
            .send()
            .await;

        match result {
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
