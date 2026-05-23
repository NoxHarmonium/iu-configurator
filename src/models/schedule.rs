use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// How the watering schedule is expressed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleMode {
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
pub struct ZoneSchedule {
    /// Whether this zone runs in the morning session.
    pub morning_enabled: bool,
    /// Whether this zone runs in the afternoon session.
    pub afternoon_enabled: bool,
    /// Duration in seconds for the morning watering session.
    pub morning_secs: u32,
    /// Duration in seconds for the afternoon watering session.
    pub afternoon_secs: u32,
}

/// The full schedule state — the only thing persisted to disk and sent over
/// the wire between client and server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    /// Local time for morning watering, e.g. "07:00".
    pub morning_time: String,
    /// Local time for afternoon watering, e.g. "15:00".
    pub afternoon_time: String,
    /// Per-zone active days for Weekday mode.
    /// Maps zone_id → subset of ["mon","tue","wed","thu","fri","sat","sun"].
    #[serde(default)]
    pub zone_active_days: HashMap<String, Vec<String>>,
    /// Per-zone config keyed by zone_id (e.g. "zone_1").
    pub zones: HashMap<String, ZoneSchedule>,
    /// Zones selected for the next manual run, keyed by zone_id → duration in seconds.
    /// Empty map means no manual sequence is emitted in the YAML.
    #[serde(default)]
    pub manual_zones: HashMap<String, u32>,
    /// Whether to use weekday or periodic scheduling.
    #[serde(default)]
    pub schedule_mode: ScheduleMode,
    /// Anchor date for periodic mode (ISO 8601, e.g. "2026-02-05").
    #[serde(default)]
    pub period_anchor: String,
    /// Days between each watering cycle in periodic mode.
    #[serde(default = "default_period_days")]
    pub period_days: u32,
}

impl Schedule {
    /// Build a seed schedule from the user's setup configuration.
    /// All zone active days start empty; the user configures them via the UI.
    #[cfg(feature = "ssr")]
    pub fn default_seed_from(setup: &super::setup::IuSetup) -> Self {
        let mut zones = HashMap::new();
        for zone in &setup.zones {
            zones.insert(
                zone.id.clone(),
                ZoneSchedule {
                    morning_enabled: setup.defaults.zone_morning_enabled,
                    afternoon_enabled: setup.defaults.zone_afternoon_enabled,
                    morning_secs: setup.defaults.zone_morning_secs,
                    afternoon_secs: setup.defaults.zone_afternoon_secs,
                },
            );
        }
        Schedule {
            morning_time: setup.defaults.morning_time.clone(),
            afternoon_time: setup.defaults.afternoon_time.clone(),
            zone_active_days: HashMap::new(),
            zones,
            manual_zones: HashMap::new(),
            schedule_mode: ScheduleMode::Weekday,
            period_anchor: String::new(),
            period_days: 2,
        }
    }
}
