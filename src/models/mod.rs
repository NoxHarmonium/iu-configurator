pub mod app_state;
pub use app_state::{AppState, AppStateMode, ZoneAppState};

#[cfg(feature = "ssr")]
pub mod iuc_config;
#[cfg(feature = "ssr")]
pub use iuc_config::{ControllerConfig, IUCConfig, IrrigationSystemDefaults, ZoneConfig};

#[cfg(feature = "ssr")]
pub mod env;
#[cfg(feature = "ssr")]
pub use env::EnvironmentConfig;

#[cfg(feature = "ssr")]
pub(crate) mod irrigation_unlimited_config;

#[cfg(feature = "ssr")]
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub config: EnvironmentConfig,
    pub setup: IUCConfig,
}
