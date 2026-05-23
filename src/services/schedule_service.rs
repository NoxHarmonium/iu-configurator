use std::collections::HashMap;

use crate::models::IuSetup;
use crate::models::Schedule;
use crate::repositories::schedule_repository;
use crate::yaml_gen::generate_yaml;

pub async fn persist_schedule_and_yaml(
    config_dir: &str,
    schedule: &Schedule,
    setup: &IuSetup,
) -> Result<(), String> {
    schedule_repository::write_schedule(config_dir, schedule).await?;
    let yaml =
        generate_yaml(schedule, setup).map_err(|e| format!("Failed to generate YAML: {e}"))?;
    schedule_repository::write_yaml(config_dir, &yaml).await
}

pub async fn save_manual_schedule(
    config_dir: &str,
    setup: &IuSetup,
    manual_zones: HashMap<String, u32>,
) -> Result<(), String> {
    let mut schedule = schedule_repository::load_or_seed_schedule(config_dir, setup).await?;
    schedule.manual_zones = manual_zones;
    persist_schedule_and_yaml(config_dir, &schedule, setup).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_setup() -> IuSetup {
        let yaml = include_str!("../../dev/config/iu-setup.yaml");
        serde_yaml::from_str(yaml).expect("dev/config/iu-setup.yaml failed to parse")
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

        let schedule_file = std::path::PathBuf::from(&config_dir).join("iu-schedule.json");
        let yaml_file = std::path::PathBuf::from(&config_dir).join("irrigation_unlimited.yaml");

        assert!(schedule_file.exists(), "expected iu-schedule.json to exist");
        assert!(
            yaml_file.exists(),
            "expected irrigation_unlimited.yaml to exist"
        );

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
            .block_on(schedule_repository::load_or_seed_schedule(
                config_dir.to_str().unwrap_or(""),
                &setup,
            ))
            .expect("load_or_seed_schedule after save failed");

        assert_eq!(loaded.manual_zones, manual);

        let _ = std::fs::remove_dir_all(config_dir);
    }
}
