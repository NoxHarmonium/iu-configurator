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

/// The full schedule state — the only thing persisted to disk and sent over
/// the wire between client and server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    /// Local time for morning watering, e.g. "07:00".
    pub morning_time: String,
    /// Local time for afternoon watering, e.g. "15:00".
    pub afternoon_time: String,
    /// Days of week morning watering runs: subset of ["mon","tue","wed","thu","fri","sat","sun"].
    pub morning_days: Vec<String>,
    /// Days of week afternoon watering runs.
    pub afternoon_days: Vec<String>,
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
            zones,
            manual_zones: HashMap::new(),
        }
    }
}
