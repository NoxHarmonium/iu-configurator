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
                morning_enabled: true,
                afternoon_enabled: true,
                morning_secs: 60,
                afternoon_secs: 60,
            },
        );
        zones.insert(
            "zone_2".into(),
            ZoneSchedule {
                morning_enabled: true,
                afternoon_enabled: true,
                morning_secs: 30,
                afternoon_secs: 30,
            },
        );
        zones.insert(
            "zone_3".into(),
            ZoneSchedule {
                morning_enabled: true,
                afternoon_enabled: true,
                morning_secs: 1200,
                afternoon_secs: 1200,
            },
        );
        // Disabled by default — no sprinklers installed yet
        zones.insert(
            "zone_4".into(),
            ZoneSchedule {
                morning_enabled: false,
                afternoon_enabled: false,
                morning_secs: 300,
                afternoon_secs: 300,
            },
        );

        // Controller "back" — Winter Regular durations
        zones.insert(
            "zone_5".into(),
            ZoneSchedule {
                morning_enabled: true,
                afternoon_enabled: true,
                morning_secs: 180,
                afternoon_secs: 180,
            },
        );
        zones.insert(
            "zone_6".into(),
            ZoneSchedule {
                morning_enabled: true,
                afternoon_enabled: true,
                morning_secs: 600,
                afternoon_secs: 600,
            },
        );
        zones.insert(
            "zone_7".into(),
            ZoneSchedule {
                morning_enabled: true,
                afternoon_enabled: true,
                morning_secs: 600,
                afternoon_secs: 600,
            },
        );
        zones.insert(
            "zone_8".into(),
            ZoneSchedule {
                morning_enabled: true,
                afternoon_enabled: true,
                morning_secs: 600,
                afternoon_secs: 600,
            },
        );

        Schedule {
            morning_time: "07:00".into(),
            afternoon_time: "15:00".into(),
            zone_active_days: HashMap::new(),
            zones,
            manual_zones: HashMap::new(),
            schedule_mode: ScheduleMode::Weekday,
            period_anchor: String::new(),
            period_days: 2,
        }
    }
}
