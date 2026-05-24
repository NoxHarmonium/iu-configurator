//! I/O functions for reading/writing `AppState` models

use crate::models::AppState;
use crate::models::IUCConfig;

fn app_state_path(config_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(config_dir).join("iu-schedule.json")
}

pub async fn load_or_seed_app_state(
    config_dir: &str,
    system_config: &IUCConfig,
) -> Result<AppState, String> {
    let path = app_state_path(config_dir);

    if path.exists() {
        tracing::info!(path = %path.display(), "loading app state from file");
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| format!("Failed to read iu-schedule.json: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse iu-schedule.json: {e}"))
    } else {
        tracing::info!("no app state file found, returning defaults");
        Ok(AppState::default_seed_from(system_config))
    }
}

pub async fn write_app_state(config_dir: &str, app_state: &AppState) -> Result<(), String> {
    let config_path = std::path::PathBuf::from(config_dir);

    tokio::fs::create_dir_all(&config_path)
        .await
        .map_err(|e| format!("Failed to create config dir: {e}"))?;

    let json = serde_json::to_string_pretty(app_state)
        .map_err(|e| format!("Failed to serialise app state: {e}"))?;
    tokio::fs::write(app_state_path(config_dir), json)
        .await
        .map_err(|e| format!("Failed to write iu-schedule.json: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_system_config() -> IUCConfig {
        let yaml = include_str!("../../dev/config/iuc-config.yaml");
        serde_yaml::from_str(yaml).expect("dev/config/iuc-config.yaml failed to parse")
    }

    fn test_dir(prefix: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir();
        let unique = format!(
            "iu-configurator-repo-{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock went backwards")
                .as_nanos()
        );
        base.join(unique)
    }

    #[test]
    fn load_or_seed_returns_defaults_when_missing() {
        let system_config = test_system_config();
        let config_dir = test_dir("seed");
        std::fs::create_dir_all(&config_dir).expect("failed to create temp dir");

        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        let app_state = rt
            .block_on(load_or_seed_app_state(
                config_dir.to_str().unwrap_or(""),
                &system_config,
            ))
            .expect("load_or_seed_app_state failed");

        assert_eq!(app_state.morning_time, system_config.defaults.morning_time);
        assert_eq!(
            app_state.afternoon_time,
            system_config.defaults.afternoon_time
        );

        let _ = std::fs::remove_dir_all(config_dir);
    }

    #[test]
    fn write_app_state_writes_json_file() {
        let system_config = test_system_config();
        let config_dir = test_dir("write-json");
        let app_state = AppState::default_seed_from(&system_config);

        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(write_app_state(
            config_dir.to_str().unwrap_or(""),
            &app_state,
        ))
        .expect("write_app_state failed");

        let app_state_file = app_state_path(config_dir.to_str().unwrap_or(""));
        assert!(
            app_state_file.exists(),
            "expected iu-schedule.json to exist"
        );

        let _ = std::fs::remove_dir_all(config_dir);
    }
}
