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

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.config_dir.trim().is_empty() {
            return Err("CONFIG_DIR cannot be empty".to_string());
        }

        if let Some(url) = &self.ha_url {
            let trimmed = url.trim();
            if trimmed.is_empty() {
                return Err("HA_URL cannot be empty when provided".to_string());
            }

            let parsed = reqwest::Url::parse(trimmed)
                .map_err(|e| format!("HA_URL is not a valid URL: {e}"))?;

            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                return Err("HA_URL must start with http:// or https://".to_string());
            }
        }

        if let Some(token) = &self.ha_token
            && token.trim().is_empty()
        {
            return Err("HA_TOKEN cannot be empty when provided".to_string());
        }

        if let Some(entity) = &self.ha_weather_entity
            && entity.trim().is_empty()
        {
            return Err("HA_WEATHER_ENTITY cannot be empty when provided".to_string());
        }

        Ok(())
    }
}
