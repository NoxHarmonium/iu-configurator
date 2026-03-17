/// Interval in milliseconds at which the UI polls Home Assistant for the current
/// irrigation status. Lower values give faster UI updates at the cost of more HA API calls.
///
/// Because polling runs in the browser (WASM), this cannot be a runtime environment variable —
/// change this value directly to adjust the poll frequency.
pub const STATUS_POLL_INTERVAL_MS: u64 = 5_000;

/// Static definition of an irrigation controller.
/// These values never change via the UI; they live here as a single source of truth.
pub struct ControllerDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Seconds the master turns on before any zone turns on.
    pub preamble_secs: u32,
    /// Seconds the master stays on after all zones turn off.
    pub postamble_secs: u32,
    /// Seconds between successive zones in a sequence.
    pub delay_secs: u32,
    /// Home Assistant entity_id of the master binary sensor, used to check
    /// whether this controller is currently active.
    pub ha_master_entity: &'static str,
}

/// Static definition of a watering zone.
pub struct ZoneDef {
    /// Stable zone identifier — must be snake_case, matches iu-schedule.json key.
    pub id: &'static str,
    /// Which controller this zone belongs to.
    pub controller_id: &'static str,
    /// Human-readable display name shown in the UI.
    pub name: &'static str,
    /// Home Assistant switch / input_boolean entity to control.
    pub entity_id: &'static str,
}

// TODO: Make controllers and zones configurable via a config file somewhere
pub static CONTROLLERS: &[ControllerDef] = &[ControllerDef {
    id: "main",
    name: "Irrigation",
    preamble_secs: 5,
    postamble_secs: 5,
    delay_secs: 5,
    ha_master_entity: "binary_sensor.irrigation_unlimited_c1_m",
}];

pub static ZONES: &[ZoneDef] = &[
    ZoneDef {
        id: "zone_1",
        controller_id: "main",
        name: "1. Balcony Pots",
        entity_id: "switch.front_irrigation_controller_l1",
    },
    ZoneDef {
        id: "zone_2",
        controller_id: "main",
        name: "2. Driveway Pots",
        entity_id: "switch.front_irrigation_controller_l2",
    },
    ZoneDef {
        id: "zone_3",
        controller_id: "main",
        name: "3. Front Garden Bed",
        entity_id: "switch.front_irrigation_controller_l3",
    },
    ZoneDef {
        id: "zone_4",
        controller_id: "main",
        name: "4. Natives",
        entity_id: "input_boolean.irrigation_zone_4",
    },
    ZoneDef {
        id: "zone_5",
        controller_id: "main",
        name: "5. Deck Planter Boxes",
        entity_id: "switch.irrigation_controller_l1",
    },
    ZoneDef {
        id: "zone_6",
        controller_id: "main",
        name: "6. Around Lemon Tree",
        entity_id: "switch.irrigation_controller_l2",
    },
    ZoneDef {
        id: "zone_7",
        controller_id: "main",
        name: "7. Vege Patch",
        entity_id: "switch.irrigation_controller_l3",
    },
    ZoneDef {
        id: "zone_8",
        controller_id: "main",
        name: "8. Fernararium",
        entity_id: "switch.irrigation_controller_l4",
    },
];
