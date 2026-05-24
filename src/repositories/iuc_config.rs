//! I/O functions for reading `IUCConfig` models

use crate::models::IUCConfig;

fn config_path(config_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(config_dir).join("iuc-config.yaml")
}

pub async fn load(config_dir: &str) -> Result<IUCConfig, String> {
    let path = config_path(config_dir);
    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        format!(
            "Failed to read iuc-config.yaml at {}: {}. Create this file in CONFIG_DIR to configure your controllers and zones.",
            path.display(),
            e
        )
    })?;

    serde_yaml::from_str(&content).map_err(|e| format!("Failed to parse iuc-config.yaml: {e}"))
}
