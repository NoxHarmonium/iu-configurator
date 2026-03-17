use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Per-zone dynamic schedule data — stored in iu-schedule.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneSchedule {
    pub enabled: bool,
    /// Duration in seconds for the morning watering session.
    pub morning_secs: u32,
    /// Duration in seconds for the afternoon watering session.
    pub afternoon_secs: u32,
}

/// A "every N days" periodic schedule — an alternative to days-of-week scheduling.
///
/// `start_date` is the absolute first-run date in `YYYY-MM-DD` format.  Storing
/// it as an absolute date (rather than a relative offset) ensures the value
/// remains stable when the user reopens the editor on a different day.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeriodicSchedule {
    /// Absolute start date in `YYYY-MM-DD` format (e.g. `"2026-03-20"`).
    pub start_date: String,
    /// How many days between each watering cycle (must be >= 1).
    pub repeat_days: u32,
}

/// The full schedule state — the only thing persisted to disk and sent over
/// the wire between client and server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    /// Local time for morning watering, e.g. "07:00".
    pub morning_time: String,
    /// Local time for afternoon watering, e.g. "15:00".
    pub afternoon_time: String,
    /// Days of week morning watering runs: subset of ["mon","tue","wed","thu","fri","sat","sun"].
    /// Used when `morning_periodic` is `None`.
    #[serde(default)]
    pub morning_days: Vec<String>,
    /// Days of week afternoon watering runs.
    /// Used when `afternoon_periodic` is `None`.
    #[serde(default)]
    pub afternoon_days: Vec<String>,
    /// Periodic schedule for morning watering. When `Some`, overrides `morning_days`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morning_periodic: Option<PeriodicSchedule>,
    /// Periodic schedule for afternoon watering. When `Some`, overrides `afternoon_days`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub afternoon_periodic: Option<PeriodicSchedule>,
    /// Per-zone config keyed by zone_id (e.g. "zone_1").
    pub zones: HashMap<String, ZoneSchedule>,
    /// Zones selected for the next manual run, keyed by zone_id → duration in seconds.
    /// Empty map means no manual sequence is emitted in the YAML.
    #[serde(default)]
    pub manual_zones: HashMap<String, u32>,
}

// TODO: Make default schedules configurable via config file
impl Schedule {
    /// Seed defaults based on the Winter – Regular schedule.
    /// All days start as OFF; user enables them via the UI.
    pub fn default_seed() -> Self {
        let mut zones = HashMap::new();

        // Controller "front" — Winter Regular durations
        zones.insert(
            "zone_1".into(),
            ZoneSchedule {
                enabled: true,
                morning_secs: 60,
                afternoon_secs: 60,
            },
        );
        zones.insert(
            "zone_2".into(),
            ZoneSchedule {
                enabled: true,
                morning_secs: 30,
                afternoon_secs: 30,
            },
        );
        zones.insert(
            "zone_3".into(),
            ZoneSchedule {
                enabled: true,
                morning_secs: 1200,
                afternoon_secs: 1200,
            },
        );
        // Disabled by default — no sprinklers installed yet
        zones.insert(
            "zone_4".into(),
            ZoneSchedule {
                enabled: false,
                morning_secs: 300,
                afternoon_secs: 300,
            },
        );

        // Controller "back" — Winter Regular durations
        zones.insert(
            "zone_5".into(),
            ZoneSchedule {
                enabled: true,
                morning_secs: 180,
                afternoon_secs: 180,
            },
        );
        zones.insert(
            "zone_6".into(),
            ZoneSchedule {
                enabled: true,
                morning_secs: 600,
                afternoon_secs: 600,
            },
        );
        zones.insert(
            "zone_7".into(),
            ZoneSchedule {
                enabled: true,
                morning_secs: 600,
                afternoon_secs: 600,
            },
        );
        zones.insert(
            "zone_8".into(),
            ZoneSchedule {
                enabled: true,
                morning_secs: 600,
                afternoon_secs: 600,
            },
        );

        Schedule {
            morning_time: "07:00".into(),
            afternoon_time: "15:00".into(),
            morning_days: Vec::new(),   // all off until user enables
            afternoon_days: Vec::new(), // all off until user enables
            morning_periodic: None,
            afternoon_periodic: None,
            zones,
            manual_zones: HashMap::new(),
        }
    }
}
