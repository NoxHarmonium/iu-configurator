use std::collections::HashMap;

use super::irrigation_unlimited_config::generate_yaml;
use crate::models::AppState;
use crate::models::IUCConfig;
use crate::repositories::app_state;
use crate::repositories::irrigation_unlimited_config;

pub async fn persist_app_state_and_yaml(
    config_dir: &str,
    app_state: &AppState,
    system_config: &IUCConfig,
) -> Result<(), String> {
    app_state::write_app_state(config_dir, app_state).await?;
    let yaml = generate_yaml(app_state, system_config)
        .map_err(|e| format!("Failed to generate YAML: {e}"))?;
    irrigation_unlimited_config::write(config_dir, &yaml).await
}

pub async fn save_manual_schedule(
    config_dir: &str,
    system_config: &IUCConfig,
    manual_zones: HashMap<String, u32>,
) -> Result<(), String> {
    let mut app_state = app_state::load_or_seed_app_state(config_dir, system_config).await?;
    app_state.manual_zones = manual_zones;
    persist_app_state_and_yaml(config_dir, &app_state, system_config).await
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
            "iu-configurator-service-{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock went backwards")
                .as_nanos()
        );
        base.join(unique)
    }

    #[test]
    fn persist_app_state_and_yaml_writes_both_files() {
        let system_config = test_system_config();
        let config_dir = test_dir("persist");
        let app_state = AppState::default_seed_from(&system_config);

        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(persist_app_state_and_yaml(
            config_dir.to_str().unwrap_or(""),
            &app_state,
            &system_config,
        ))
        .expect("persist_app_state_and_yaml failed");

        let app_state_file = std::path::PathBuf::from(&config_dir).join("iu-schedule.json");
        let yaml_file = std::path::PathBuf::from(&config_dir).join("irrigation_unlimited.yaml");

        assert!(
            app_state_file.exists(),
            "expected iu-schedule.json to exist"
        );
        assert!(
            yaml_file.exists(),
            "expected irrigation_unlimited.yaml to exist"
        );

        let _ = std::fs::remove_dir_all(config_dir);
    }

    #[test]
    fn save_manual_schedule_updates_manual_zones() {
        let system_config = test_system_config();
        let config_dir = test_dir("manual");

        let mut manual = HashMap::new();
        manual.insert("zone_1".to_string(), 120);
        manual.insert("zone_2".to_string(), 0);

        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(save_manual_schedule(
            config_dir.to_str().unwrap_or(""),
            &system_config,
            manual.clone(),
        ))
        .expect("save_manual_schedule failed");

        let loaded = rt
            .block_on(app_state::load_or_seed_app_state(
                config_dir.to_str().unwrap_or(""),
                &system_config,
            ))
            .expect("load_or_seed_app_state after save failed");

        assert_eq!(loaded.manual_zones, manual);

        let _ = std::fs::remove_dir_all(config_dir);
    }
}
