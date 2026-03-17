use std::collections::HashMap;

use serde::Serialize;

use crate::definitions::{CONTROLLERS, ZONES};
use crate::models::{Schedule, ScheduleMode, ZoneSchedule};

// ---------------------------------------------------------------------------
// Structures that mirror the irrigation_unlimited YAML schema.
// These are only used for serialisation — they never leave the server.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct IuConfig {
    controllers: Vec<IuController>,
}

#[derive(Serialize)]
struct IuController {
    name: String,
    preamble: String,
    postamble: String,
    zones: Vec<IuZone>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sequences: Vec<IuSequence>,
}

#[derive(Serialize)]
struct IuZone {
    zone_id: String,
    name: String,
    entity_id: String,
}

#[derive(Serialize)]
struct IuSequence {
    name: String,
    sequence_id: String,
    delay: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    schedules: Vec<IuSchedule>,
    zones: Vec<IuSeqZone>,
}

#[derive(Serialize)]
struct IuSchedule {
    name: String,
    time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    weekday: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    every_n_days: Option<u32>,
}

#[derive(Serialize)]
struct IuSeqZone {
    zone_id: String,
    duration: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate an `irrigation_unlimited` YAML string from the active schedule.
pub fn generate_yaml(schedule: &Schedule) -> Result<String, serde_yaml::Error> {
    let controllers = build_controllers(schedule);
    let yaml = serde_yaml::to_string(&IuConfig { controllers })?;
    // serde_yaml targets YAML 1.2 and leaves "HH:MM" unquoted, but Home
    // Assistant uses PyYAML which defaults to YAML 1.1 where bare "HH:MM"
    // scalars are interpreted as sexagesimal numbers.  Quote them explicitly.
    Ok(quote_time_fields(yaml))
}

/// Wrap bare `time:` scalar values in single quotes.
/// TODO: There has to be a better way than this!
fn quote_time_fields(yaml: String) -> String {
    let trailing_newline = yaml.ends_with('\n');
    let mut result = yaml
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if let Some(value_part) = trimmed.strip_prefix("time:") {
                let value = value_part.trim();
                if !value.is_empty() && !value.starts_with('\'') && !value.starts_with('"') {
                    let indent = &line[..line.len() - trimmed.len()];
                    return format!("{}time: '{}'", indent, value);
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    if trailing_newline {
        result.push('\n');
    }
    result
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_controllers(schedule: &Schedule) -> Vec<IuController> {
    CONTROLLERS
        .iter()
        .map(|ctrl| {
            // All physical zone definitions for this controller (always included).
            let zones: Vec<IuZone> = ZONES
                .iter()
                .filter(|z| z.controller_id == ctrl.id)
                .map(|z| IuZone {
                    zone_id: z.id.to_string(),
                    name: z.name.to_string(),
                    entity_id: z.entity_id.to_string(),
                })
                .collect();

            let mut sequences = Vec::new();

            let is_periodic = schedule.schedule_mode == ScheduleMode::Periodic;
            let periodic_active =
                is_periodic && !schedule.period_anchor.is_empty() && schedule.period_days > 0;

            // Morning sequence — emitted when days are selected (weekday) or
            // a valid periodic anchor+period is configured.
            let morning_active = if is_periodic {
                periodic_active
            } else {
                !schedule.active_days.is_empty()
            };

            if morning_active {
                let seq_zones = build_seq_zones(
                    ctrl.id,
                    &schedule.zones,
                    |zs| zs.morning_enabled,
                    |zs| zs.morning_secs,
                );
                if !seq_zones.is_empty() {
                    let morning_sched = if is_periodic {
                        IuSchedule {
                            name: "Morning".into(),
                            time: schedule.morning_time.clone(),
                            weekday: None,
                            anchor: Some(schedule.period_anchor.clone()),
                            every_n_days: Some(schedule.period_days),
                        }
                    } else {
                        IuSchedule {
                            name: "Morning".into(),
                            time: schedule.morning_time.clone(),
                            weekday: weekday_filter(&schedule.active_days),
                            anchor: None,
                            every_n_days: None,
                        }
                    };
                    sequences.push(IuSequence {
                        name: "Morning".into(),
                        sequence_id: format!("{}_morning", ctrl.id),
                        delay: format_duration(ctrl.delay_secs),
                        schedules: vec![morning_sched],
                        zones: seq_zones,
                    });
                }
            }

            // Afternoon sequence
            let afternoon_active = if is_periodic {
                periodic_active
            } else {
                !schedule.active_days.is_empty()
            };

            if afternoon_active {
                let seq_zones = build_seq_zones(
                    ctrl.id,
                    &schedule.zones,
                    |zs| zs.afternoon_enabled,
                    |zs| zs.afternoon_secs,
                );
                if !seq_zones.is_empty() {
                    let afternoon_sched = if is_periodic {
                        IuSchedule {
                            name: "Afternoon".into(),
                            time: schedule.afternoon_time.clone(),
                            weekday: None,
                            anchor: Some(schedule.period_anchor.clone()),
                            every_n_days: Some(schedule.period_days),
                        }
                    } else {
                        IuSchedule {
                            name: "Afternoon".into(),
                            time: schedule.afternoon_time.clone(),
                            weekday: weekday_filter(&schedule.active_days),
                            anchor: None,
                            every_n_days: None,
                        }
                    };
                    sequences.push(IuSequence {
                        name: "Afternoon".into(),
                        sequence_id: format!("{}_afternoon", ctrl.id),
                        delay: format_duration(ctrl.delay_secs),
                        schedules: vec![afternoon_sched],
                        zones: seq_zones,
                    });
                }
            }

            // Manual sequence — no schedules, triggered via HA API only.
            // Only emitted when at least one zone has a non-zero duration selected.
            let manual_seq_zones = build_manual_seq_zones(ctrl.id, &schedule.manual_zones);
            if !manual_seq_zones.is_empty() {
                sequences.push(IuSequence {
                    name: "Manual".into(),
                    sequence_id: "manual".into(),
                    delay: format_duration(ctrl.delay_secs),
                    schedules: vec![],
                    zones: manual_seq_zones,
                });
            }

            IuController {
                name: ctrl.name.to_string(),
                preamble: format_duration(ctrl.preamble_secs),
                postamble: format_duration(ctrl.postamble_secs),
                zones,
                sequences,
            }
        })
        .collect()
}

/// Build the sequence zone list for a single controller & session, including
/// only zones that are enabled and have a non-zero duration.
fn build_seq_zones(
    controller_id: &str,
    zone_schedules: &HashMap<String, ZoneSchedule>,
    is_enabled: impl Fn(&ZoneSchedule) -> bool,
    get_secs: impl Fn(&ZoneSchedule) -> u32,
) -> Vec<IuSeqZone> {
    ZONES
        .iter()
        .filter(|z| z.controller_id == controller_id)
        .filter_map(|z| {
            zone_schedules.get(z.id).and_then(|zs| {
                let secs = get_secs(zs);
                if is_enabled(zs) && secs > 0 {
                    Some(IuSeqZone {
                        zone_id: z.id.to_string(),
                        duration: format_duration(secs),
                    })
                } else {
                    None
                }
            })
        })
        .collect()
}

/// Build the sequence zone list for a manual run, including only zones that
/// appear in `manual_zones` with a non-zero duration.
fn build_manual_seq_zones(
    controller_id: &str,
    manual_zones: &HashMap<String, u32>,
) -> Vec<IuSeqZone> {
    ZONES
        .iter()
        .filter(|z| z.controller_id == controller_id)
        .filter_map(|z| {
            manual_zones.get(z.id).and_then(|&secs| {
                if secs > 0 {
                    Some(IuSeqZone {
                        zone_id: z.id.to_string(),
                        duration: format_duration(secs),
                    })
                } else {
                    None
                }
            })
        })
        .collect()
}

/// Returns `None` (omitting the YAML field) when all seven days are selected,
/// since IU's default is to run every day when weekday is absent.
fn weekday_filter(days: &[String]) -> Option<Vec<String>> {
    if days.len() >= 7 {
        None
    } else {
        Some(days.to_vec())
    }
}

/// Format seconds as `"HH:MM:SS"` for irrigation_unlimited duration fields.
fn format_duration(secs: u32) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

// ---------------------------------------------------------------------------
// Tests — run with: cargo test --features ssr
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Schedule;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "00:00:00");
        assert_eq!(format_duration(30), "00:00:30");
        assert_eq!(format_duration(60), "00:01:00");
        assert_eq!(format_duration(1200), "00:20:00");
        assert_eq!(format_duration(3661), "01:01:01");
    }

    #[test]
    fn test_no_days_produces_no_sequences() {
        let schedule = Schedule::default_seed(); // active_days is empty
        let yaml = generate_yaml(&schedule).unwrap();
        // sequences field should be absent when empty (skip_serializing_if)
        assert!(!yaml.contains("sequences"));
        assert!(yaml.contains("controllers"));
        assert!(yaml.contains("zone_1"));
    }

    #[test]
    fn test_morning_sequence_produced() {
        let mut schedule = Schedule::default_seed();
        schedule.active_days = vec!["mon".into(), "wed".into(), "fri".into()];

        let yaml = generate_yaml(&schedule).unwrap();

        assert!(
            yaml.contains("main_morning"),
            "missing main_morning sequence_id"
        );
        assert!(yaml.contains("07:00"), "missing morning time");
        assert!(yaml.contains("mon"), "missing weekday filter");
    }

    #[test]
    fn test_afternoon_sequence_produced() {
        let mut schedule = Schedule::default_seed();
        schedule.active_days = vec!["sat".into(), "sun".into()];

        let yaml = generate_yaml(&schedule).unwrap();

        assert!(yaml.contains("main_afternoon"));
        assert!(yaml.contains("15:00"));
    }

    #[test]
    fn test_all_seven_days_omits_weekday_field() {
        let mut schedule = Schedule::default_seed();
        schedule.active_days = vec![
            "mon".into(),
            "tue".into(),
            "wed".into(),
            "thu".into(),
            "fri".into(),
            "sat".into(),
            "sun".into(),
        ];

        let yaml = generate_yaml(&schedule).unwrap();

        // weekday filter should be omitted when all 7 days selected
        assert!(
            !yaml.contains("weekday:"),
            "weekday should be absent for all-7-days"
        );
    }

    #[test]
    fn test_disabled_zone_excluded_from_sequence() {
        let mut schedule = Schedule::default_seed();
        schedule.active_days = vec!["mon".into()];
        // zone_4 has morning_enabled: false, afternoon_enabled: false in default_seed

        let yaml = generate_yaml(&schedule).unwrap();

        // zone_4 must appear in the controller zones list (the physical definitions)
        assert!(
            yaml.contains("zone_4"),
            "zone_4 should be in zone definitions"
        );

        // But the sequence zones for front controller should only list zone_1,2,3
        // We check that zone_4 does NOT appear as a duration entry by ensuring
        // the duration lines are for zones 1-3 only.
        // (spot-check: zone_3 enabled, zone_4 disabled)
        let lines: Vec<&str> = yaml.lines().collect();
        let mut in_sequences = false;
        for line in &lines {
            if line.contains("sequences") {
                in_sequences = true;
            }
            if in_sequences && line.contains("zone_id: zone_4") {
                panic!("zone_4 should not appear in sequences but found: {line}");
            }
        }
    }

    #[test]
    fn test_both_sessions_produced() {
        let mut schedule = Schedule::default_seed();
        schedule.active_days = vec!["mon".into()];

        let yaml = generate_yaml(&schedule).unwrap();

        assert!(yaml.contains("main_morning"));
        assert!(yaml.contains("main_afternoon"));
    }

    #[test]
    fn test_manual_sequence_emitted_when_zones_selected() {
        let mut schedule = Schedule::default_seed();
        schedule.manual_zones.insert("zone_1".into(), 120);
        schedule.manual_zones.insert("zone_5".into(), 300);

        let yaml = generate_yaml(&schedule).unwrap();

        assert!(
            yaml.contains("sequence_id: manual"),
            "missing manual sequence_id"
        );
        assert!(
            yaml.contains("zone_id: zone_1"),
            "missing zone_1 in manual sequence"
        );
        assert!(
            yaml.contains("zone_id: zone_5"),
            "missing zone_5 in manual sequence"
        );
        // schedules key should be absent for the manual sequence
        assert!(
            !yaml.contains("schedules:"),
            "manual sequence should have no schedules block"
        );
    }

    #[test]
    fn test_manual_sequence_absent_when_no_zones() {
        let schedule = Schedule::default_seed(); // manual_zones is empty
        let yaml = generate_yaml(&schedule).unwrap();
        assert!(
            !yaml.contains("manual"),
            "manual sequence should not appear when no zones selected"
        );
    }

    #[test]
    fn print_sample_yaml() {
        // Not a real assertion — useful for manual inspection during development.
        let mut schedule = Schedule::default_seed();
        schedule.active_days = vec![
            "mon".into(),
            "tue".into(),
            "wed".into(),
            "thu".into(),
            "fri".into(),
            "sat".into(),
            "sun".into(),
        ];

        let yaml = generate_yaml(&schedule).unwrap();
        println!("\n--- Sample YAML ---\n{yaml}\n---");
    }
}
