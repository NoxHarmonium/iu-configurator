use serde::Deserialize;

fn default_config_dir() -> String {
    "/config".to_string()
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_config_dir")]
    pub config_dir: String,
    pub ha_url: Option<String>,
    pub ha_token: Option<String>,
    pub ha_weather_entity: Option<String>,
}
