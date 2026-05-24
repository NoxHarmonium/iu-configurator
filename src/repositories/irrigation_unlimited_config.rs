fn yaml_path(config_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(config_dir).join("irrigation_unlimited.yaml")
}

pub async fn write(config_dir: &str, yaml: &str) -> Result<(), String> {
    tokio::fs::write(yaml_path(config_dir), yaml)
        .await
        .map_err(|e| format!("Failed to write irrigation_unlimited.yaml: {e}"))
}
