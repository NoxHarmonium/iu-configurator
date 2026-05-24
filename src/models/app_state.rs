//! Models to do with the current state of the application (e.g. what schedules are selected)
//!
//! The zones/controllers are not specified here, you'll want IUCConfig for that.
//!
//! It is not required for the application to start up. If it is the first time the application is
//! being run, this state will be generated with sensible defaults from the IUCConfig.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// How the watering schedule is expressed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppStateMode {
    /// Run on specific days of the week (existing behaviour).
    #[default]
    Weekday,
    /// Run every N days starting from an anchor date.
    Periodic,
}

fn default_period_days() -> u32 {
    2
}

/// Per-zone dynamic schedule data — stored in iu-schedule.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneAppState {
    /// Whether this zone runs in the morning session.
    pub morning_enabled: bool,
    /// Whether this zone runs in the afternoon session.
    pub afternoon_enabled: bool,
    /// Duration in seconds for the morning watering session.
    pub morning_secs: u32,
    /// Duration in seconds for the afternoon watering session.
    pub afternoon_secs: u32,
}

/// The full application state — the only thing persisted to disk and sent over
/// the wire between client and server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    /// Local time for morning watering, e.g. "07:00".
    pub morning_time: String,
    /// Local time for afternoon watering, e.g. "15:00".
    pub afternoon_time: String,
    /// Per-zone active days for Weekday mode.
    /// Maps zone_id → subset of ["mon","tue","wed","thu","fri","sat","sun"].
    #[serde(default)]
    pub zone_active_days: HashMap<String, Vec<String>>,
    /// Per-zone config keyed by zone_id (e.g. "zone_1").
    pub zones: HashMap<String, ZoneAppState>,
    /// Zones selected for the next manual run, keyed by zone_id → duration in seconds.
    /// Empty map means no manual sequence is emitted in the YAML.
    #[serde(default)]
    pub manual_zones: HashMap<String, u32>,
    /// Whether to use weekday or periodic scheduling.
    #[serde(default)]
    pub schedule_mode: AppStateMode,
    /// Anchor date for periodic mode (ISO 8601, e.g. "2026-02-05").
    #[serde(default)]
    pub period_anchor: String,
    /// Days between each watering cycle in periodic mode.
    #[serde(default = "default_period_days")]
    pub period_days: u32,
}

impl AppState {
    /// Build a seed app state from the irrigation system configuration.
    /// All zone active days start empty; the user configures them via the UI.
    #[cfg(feature = "ssr")]
    pub fn default_seed_from(system_config: &super::iuc_config::IUCConfig) -> Self {
        let mut zones = HashMap::new();
        for zone in &system_config.zones {
            zones.insert(
                zone.id.clone(),
                ZoneAppState {
                    morning_enabled: system_config.defaults.zone_morning_enabled,
                    afternoon_enabled: system_config.defaults.zone_afternoon_enabled,
                    morning_secs: system_config.defaults.zone_morning_secs,
                    afternoon_secs: system_config.defaults.zone_afternoon_secs,
                },
            );
        }
        AppState {
            morning_time: system_config.defaults.morning_time.clone(),
            afternoon_time: system_config.defaults.afternoon_time.clone(),
            zone_active_days: HashMap::new(),
            zones,
            manual_zones: HashMap::new(),
            schedule_mode: AppStateMode::Weekday,
            period_anchor: String::new(),
            period_days: 2,
        }
    }
}
