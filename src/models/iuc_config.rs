use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ControllerConfig {
    pub id: String,
    pub name: String,
    /// Seconds the master turns on before any zone turns on.
    pub preamble_secs: u32,
    /// Seconds the master stays on after all zones turn off.
    pub postamble_secs: u32,
    /// Seconds between successive zones in a sequence.
    pub delay_secs: u32,
    /// Home Assistant entity_id of the master binary sensor.
    pub ha_master_entity: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZoneConfig {
    /// Stable zone identifier — must be snake_case, matches iu-schedule.json key.
    pub id: String,
    /// Which controller this zone belongs to.
    pub controller_id: String,
    /// Human-readable display name shown in the UI.
    pub name: String,
    /// Home Assistant switch / input_boolean entity to control.
    pub entity_id: String,
    /// Optional concurrency group. Sequences whose zones all share the same
    /// non-empty `zone_group` value are allowed to start at the same time.
    /// Sequences with no `zone_group` (or mixed groups) are serialised —
    /// each runs after the previous one finishes.
    #[serde(default)]
    pub zone_group: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IrrigationSystemDefaults {
    /// Local time for morning watering, e.g. "07:00".
    pub morning_time: String,
    /// Local time for afternoon watering, e.g. "15:00".
    pub afternoon_time: String,
    /// Default morning duration in seconds applied to all zones on first seed.
    pub zone_morning_secs: u32,
    /// Default afternoon duration in seconds applied to all zones on first seed.
    pub zone_afternoon_secs: u32,
    /// Whether morning session is enabled for all zones on first seed.
    pub zone_morning_enabled: bool,
    /// Whether afternoon session is enabled for all zones on first seed.
    pub zone_afternoon_enabled: bool,
}

/// Runtime configuration loaded from `$CONFIG_DIR/iuc-config.yaml`.
/// Describes the physical irrigation setup — controllers, zones, and seeding defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct IUCConfig {
    /// How often (ms) the UI polls Home Assistant for irrigation status.
    pub poll_interval_ms: u64,
    pub controllers: Vec<ControllerConfig>,
    pub zones: Vec<ZoneConfig>,
    pub defaults: IrrigationSystemDefaults,
}
