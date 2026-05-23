use crate::models::IuSetup;
use crate::models::Schedule;

fn schedule_path(config_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(config_dir).join("iu-schedule.json")
}

pub fn yaml_path(config_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(config_dir).join("irrigation_unlimited.yaml")
}

pub async fn load_or_seed_schedule(config_dir: &str, setup: &IuSetup) -> Result<Schedule, String> {
    let path = schedule_path(config_dir);

    if path.exists() {
        tracing::info!(path = %path.display(), "loading schedule from file");
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| format!("Failed to read iu-schedule.json: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse iu-schedule.json: {e}"))
    } else {
        tracing::info!("no schedule file found, returning defaults");
        Ok(Schedule::default_seed_from(setup))
    }
}

pub async fn write_schedule(config_dir: &str, schedule: &Schedule) -> Result<(), String> {
    let config_path = std::path::PathBuf::from(config_dir);

    tokio::fs::create_dir_all(&config_path)
        .await
        .map_err(|e| format!("Failed to create config dir: {e}"))?;

    let json = serde_json::to_string_pretty(schedule)
        .map_err(|e| format!("Failed to serialise schedule: {e}"))?;
    tokio::fs::write(schedule_path(config_dir), json)
        .await
        .map_err(|e| format!("Failed to write iu-schedule.json: {e}"))
}

pub async fn write_yaml(config_dir: &str, yaml: &str) -> Result<(), String> {
    tokio::fs::write(yaml_path(config_dir), yaml)
        .await
        .map_err(|e| format!("Failed to write irrigation_unlimited.yaml: {e}"))
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
        let setup = test_setup();
        let config_dir = test_dir("seed");
        std::fs::create_dir_all(&config_dir).expect("failed to create temp dir");

        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        let schedule = rt
            .block_on(load_or_seed_schedule(
                config_dir.to_str().unwrap_or(""),
                &setup,
            ))
            .expect("load_or_seed_schedule failed");

        assert_eq!(schedule.morning_time, setup.defaults.morning_time);
        assert_eq!(schedule.afternoon_time, setup.defaults.afternoon_time);

        let _ = std::fs::remove_dir_all(config_dir);
    }

    #[test]
    fn write_schedule_writes_json_file() {
        let setup = test_setup();
        let config_dir = test_dir("write-json");
        let schedule = Schedule::default_seed_from(&setup);

        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(write_schedule(config_dir.to_str().unwrap_or(""), &schedule))
            .expect("write_schedule failed");

        let schedule_file = schedule_path(config_dir.to_str().unwrap_or(""));
        assert!(schedule_file.exists(), "expected iu-schedule.json to exist");

        let _ = std::fs::remove_dir_all(config_dir);
    }
}
