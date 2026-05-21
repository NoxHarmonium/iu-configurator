use std::collections::HashMap;

use serde::Serialize;

use crate::models::{Schedule, ScheduleMode, ZoneSchedule};
use crate::setup::IuSetup;

/// `(sorted_active_days, zones_in_group)` where each zone is `(zone_id, duration_secs)`.
/// Used internally by `build_weekday_sequences` to accumulate day-pattern groups
/// before assigning start times.
type DayGroup = (Vec<String>, Vec<(String, u32)>);

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
    day: Option<IuEveryNDays>,
}

#[derive(Serialize)]
struct IuEveryNDays {
    every_n_days: u32,
    start_n_days: String,
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
pub fn generate_yaml(schedule: &Schedule, setup: &IuSetup) -> Result<String, serde_yaml::Error> {
    let controllers = build_controllers(schedule, setup);
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

fn build_controllers(schedule: &Schedule, setup: &IuSetup) -> Vec<IuController> {
    setup
        .controllers
        .iter()
        .map(|ctrl| {
            // All physical zone definitions for this controller (always included).
            let zones: Vec<IuZone> = setup
                .zones
                .iter()
                .filter(|z| z.controller_id == ctrl.id)
                .map(|z| IuZone {
                    zone_id: z.id.clone(),
                    name: z.name.clone(),
                    entity_id: z.entity_id.clone(),
                })
                .collect();

            let mut sequences = Vec::new();

            let is_periodic = schedule.schedule_mode == ScheduleMode::Periodic;
            let periodic_active =
                is_periodic && !schedule.period_anchor.is_empty() && schedule.period_days > 0;

            if is_periodic {
                // Periodic mode: one morning and one afternoon sequence for all enabled zones.
                if periodic_active {
                    let seq_zones = build_seq_zones(
                        setup,
                        ctrl.id.as_str(),
                        &schedule.zones,
                        |zs| zs.morning_enabled,
                        |zs| zs.morning_secs,
                    );
                    if !seq_zones.is_empty() {
                        sequences.push(IuSequence {
                            name: "Morning".into(),
                            sequence_id: format!("{}_morning", ctrl.id),
                            delay: format_duration(ctrl.delay_secs),
                            schedules: vec![IuSchedule {
                                name: "Morning".into(),
                                time: schedule.morning_time.clone(),
                                weekday: None,
                                day: Some(IuEveryNDays {
                                    every_n_days: schedule.period_days,
                                    start_n_days: schedule.period_anchor.clone(),
                                }),
                            }],
                            zones: seq_zones,
                        });
                    }

                    let seq_zones = build_seq_zones(
                        setup,
                        ctrl.id.as_str(),
                        &schedule.zones,
                        |zs| zs.afternoon_enabled,
                        |zs| zs.afternoon_secs,
                    );
                    if !seq_zones.is_empty() {
                        sequences.push(IuSequence {
                            name: "Afternoon".into(),
                            sequence_id: format!("{}_afternoon", ctrl.id),
                            delay: format_duration(ctrl.delay_secs),
                            schedules: vec![IuSchedule {
                                name: "Afternoon".into(),
                                time: schedule.afternoon_time.clone(),
                                weekday: None,
                                day: Some(IuEveryNDays {
                                    every_n_days: schedule.period_days,
                                    start_n_days: schedule.period_anchor.clone(),
                                }),
                            }],
                            zones: seq_zones,
                        });
                    }
                }
            } else {
                // Weekday mode: group zones by their day pattern, one sequence per group.
                sequences.extend(build_weekday_sequences(
                    setup,
                    ctrl.id.as_str(),
                    &schedule.zones,
                    &schedule.zone_active_days,
                    &schedule.morning_time,
                    "morning",
                    ctrl.delay_secs,
                    ctrl.preamble_secs,
                    ctrl.postamble_secs,
                    |zs| zs.morning_enabled,
                    |zs| zs.morning_secs,
                ));
                sequences.extend(build_weekday_sequences(
                    setup,
                    ctrl.id.as_str(),
                    &schedule.zones,
                    &schedule.zone_active_days,
                    &schedule.afternoon_time,
                    "afternoon",
                    ctrl.delay_secs,
                    ctrl.preamble_secs,
                    ctrl.postamble_secs,
                    |zs| zs.afternoon_enabled,
                    |zs| zs.afternoon_secs,
                ));
            }

            // Manual sequence — no schedules, triggered via HA API only.
            // Only emitted when at least one zone has a non-zero duration selected.
            let manual_seq_zones =
                build_manual_seq_zones(setup, ctrl.id.as_str(), &schedule.manual_zones);
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
                name: ctrl.name.clone(),
                preamble: format_duration(ctrl.preamble_secs),
                postamble: format_duration(ctrl.postamble_secs),
                zones,
                sequences,
            }
        })
        .collect()
}
/// Group zones for a single controller session by their active-day pattern and
/// emit one `IuSequence` per unique pattern.  Zones without any active days,
/// or that are disabled / have zero duration for this session, are skipped.
///
/// Sequences are assigned start times so that sequences that would share the
/// same wall-clock time do not compete for water.  Two sequences may share a
/// start time only if every zone in both belongs to the same non-empty
/// `zone_group`.  All other sequences are serialised: each slot starts after
/// the previous one's longest sequence has finished.
/// TODO: Address this clippy issue
#[allow(clippy::too_many_arguments)]
fn build_weekday_sequences<F, G>(
    setup: &IuSetup,
    controller_id: &str,
    zone_schedules: &HashMap<String, ZoneSchedule>,
    zone_active_days: &HashMap<String, Vec<String>>,
    session_time: &str,
    session: &str,
    delay_secs: u32,
    preamble_secs: u32,
    postamble_secs: u32,
    is_enabled: F,
    get_secs: G,
) -> Vec<IuSequence>
where
    F: Fn(&ZoneSchedule) -> bool,
    G: Fn(&ZoneSchedule) -> u32,
{
    const DAY_ORDER: &[&str] = &["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

    // Build groups: (sorted-day-set, zones-in-that-group) storing raw secs so
    // we can compute slot durations without re-parsing formatted strings.
    let mut groups: Vec<DayGroup> = Vec::new();

    for zone in setup
        .zones
        .iter()
        .filter(|z| z.controller_id == controller_id)
    {
        let days = match zone_active_days.get(zone.id.as_str()) {
            Some(d) if !d.is_empty() => {
                let mut sorted = d.clone();
                sorted.sort_by_key(|d| DAY_ORDER.iter().position(|&o| o == d).unwrap_or(7));
                sorted
            }
            _ => continue,
        };

        let secs = match zone_schedules.get(zone.id.as_str()) {
            Some(zs) if is_enabled(zs) && get_secs(zs) > 0 => get_secs(zs),
            _ => continue,
        };

        if let Some(group) = groups.iter_mut().find(|(d, _)| *d == days) {
            group.1.push((zone.id.clone(), secs));
        } else {
            groups.push((days, vec![(zone.id.clone(), secs)]));
        }
    }

    if groups.is_empty() {
        return Vec::new();
    }

    // Determine the concurrent key for each group.  A group's key is the
    // shared `zone_group` of all its zones, or `None` when zones have no group
    // or belong to different groups.  Only groups with the same non-None key
    // are allowed to start at the same time.
    let concurrent_keys: Vec<Option<String>> = groups
        .iter()
        .map(|(_, zone_list)| {
            let first_group = setup
                .zones
                .iter()
                .find(|z| z.id == zone_list[0].0)
                .and_then(|z| z.zone_group.clone());
            let first_group = match first_group {
                Some(g) => g,
                None => return None,
            };
            for (zone_id, _) in &zone_list[1..] {
                let zg = setup
                    .zones
                    .iter()
                    .find(|z| &z.id == zone_id)
                    .and_then(|z| z.zone_group.as_deref());
                if zg != Some(&first_group) {
                    return None;
                }
            }
            Some(first_group)
        })
        .collect();

    // Build run slots.  Groups sharing the same non-None concurrent key form
    // one slot (same start time).  Groups with no key each get their own
    // singleton slot and run after the previous slot finishes.
    let mut slots: Vec<Vec<usize>> = Vec::new();
    let mut assigned = vec![false; groups.len()];
    for (i, key_i) in concurrent_keys.iter().enumerate() {
        if assigned[i] {
            continue;
        }
        assigned[i] = true;
        let mut slot = vec![i];
        if let Some(g) = key_i {
            for (j, key_j) in concurrent_keys.iter().enumerate().skip(i + 1) {
                if key_j.as_deref() == Some(g.as_str()) && !assigned[j] {
                    assigned[j] = true;
                    slot.push(j);
                }
            }
        }
        slots.push(slot);
    }

    // Emit sequences, computing cumulative start times across slots.
    // Within a slot all sequences get the same start time; the slot advances
    // by the longest sequence duration in that slot.
    let total = groups.len();
    let mut current_secs = parse_hhmm_to_secs(session_time);
    let mut result: Vec<IuSequence> = Vec::new();
    let mut seq_num: usize = 0;

    for slot in &slots {
        let slot_start = format_secs_to_hhmm(current_secs);

        let slot_duration = slot
            .iter()
            .map(|&gi| {
                let zone_secs: u32 = groups[gi].1.iter().map(|(_, s)| s).sum();
                let n = groups[gi].1.len() as u32;
                preamble_secs + zone_secs + delay_secs * n.saturating_sub(1) + postamble_secs
            })
            .max()
            .unwrap_or(0);

        for &gi in slot {
            seq_num += 1;
            let (days, zone_list) = &groups[gi];
            let seq_id = if total == 1 {
                format!("{}_{}", controller_id, session)
            } else {
                format!("{}_{}_{}", controller_id, session, seq_num)
            };
            let seq_zones: Vec<IuSeqZone> = zone_list
                .iter()
                .map(|(zone_id, secs)| IuSeqZone {
                    zone_id: zone_id.clone(),
                    duration: format_duration(*secs),
                })
                .collect();
            result.push(IuSequence {
                name: format!("{} ({})", capitalize_first(session), days_label(days)),
                sequence_id: seq_id,
                delay: format_duration(delay_secs),
                schedules: vec![IuSchedule {
                    name: capitalize_first(session),
                    time: slot_start.clone(),
                    weekday: weekday_filter(days),
                    day: None,
                }],
                zones: seq_zones,
            });
        }

        current_secs += slot_duration;
    }

    result
}

/// Returns a human-readable label for a set of active days.
/// Examples: "All Week", "Weekdays", "Weekends", "Mon, Wed, Fri"
fn days_label(days: &[String]) -> String {
    const WEEKDAYS: &[&str] = &["mon", "tue", "wed", "thu", "fri"];
    const WEEKEND: &[&str] = &["sat", "sun"];

    if days.len() == 7 {
        return "All Week".to_string();
    }
    let day_strs: Vec<&str> = days.iter().map(String::as_str).collect();
    if day_strs == WEEKDAYS {
        return "Weekdays".to_string();
    }
    if day_strs == WEEKEND {
        return "Weekends".to_string();
    }
    days.iter()
        .map(|d| capitalize_first(d))
        .collect::<Vec<_>>()
        .join(", ")
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
/// Build the sequence zone list for a single controller & session, including
/// only zones that are enabled and have a non-zero duration.
fn build_seq_zones(
    setup: &IuSetup,
    controller_id: &str,
    zone_schedules: &HashMap<String, ZoneSchedule>,
    is_enabled: impl Fn(&ZoneSchedule) -> bool,
    get_secs: impl Fn(&ZoneSchedule) -> u32,
) -> Vec<IuSeqZone> {
    setup
        .zones
        .iter()
        .filter(|z| z.controller_id == controller_id)
        .filter_map(|z| {
            zone_schedules.get(z.id.as_str()).and_then(|zs| {
                let secs = get_secs(zs);
                if is_enabled(zs) && secs > 0 {
                    Some(IuSeqZone {
                        zone_id: z.id.clone(),
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
    setup: &IuSetup,
    controller_id: &str,
    manual_zones: &HashMap<String, u32>,
) -> Vec<IuSeqZone> {
    setup
        .zones
        .iter()
        .filter(|z| z.controller_id == controller_id)
        .filter_map(|z| {
            manual_zones.get(z.id.as_str()).and_then(|&secs| {
                if secs > 0 {
                    Some(IuSeqZone {
                        zone_id: z.id.clone(),
                        duration: format_duration(secs),
                    })
                } else {
                    None
                }
            })
        })
        .collect()
}

/// Parse `"HH:MM"` into a total number of seconds.
fn parse_hhmm_to_secs(hhmm: &str) -> u32 {
    let mut parts = hhmm.splitn(2, ':');
    let h: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let m: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    h * 3600 + m * 60
}

/// Format a total number of seconds as `"HH:MM"`, wrapping at midnight.
fn format_secs_to_hhmm(secs: u32) -> String {
    let secs = secs % 86400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    format!("{h:02}:{m:02}")
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
    use crate::setup::IuSetup;

    /// Parse the dev config fixture at compile time.  This also ensures
    /// `dev/config/iu-setup.yaml` is tracked in the repository — the compiler
    /// will refuse to build if the file is missing.
    fn test_setup() -> IuSetup {
        let yaml = include_str!("../dev/config/iu-setup.yaml");
        serde_yaml::from_str(yaml).expect("dev/config/iu-setup.yaml failed to parse")
    }

    fn set_all_zone_days(schedule: &mut Schedule, setup: &IuSetup, days: Vec<String>) {
        for zone in &setup.zones {
            schedule
                .zone_active_days
                .insert(zone.id.clone(), days.clone());
        }
    }

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
        let setup = test_setup();
        let schedule = Schedule::default_seed_from(&setup); // zone_active_days is empty
        let yaml = generate_yaml(&schedule, &setup).unwrap();
        // sequences field should be absent when empty (skip_serializing_if)
        assert!(!yaml.contains("sequences"));
        assert!(yaml.contains("controllers"));
        assert!(yaml.contains("zone_1"));
    }

    #[test]
    fn test_morning_sequence_produced() {
        let setup = test_setup();
        let mut schedule = Schedule::default_seed_from(&setup);
        set_all_zone_days(
            &mut schedule,
            &setup,
            vec!["mon".into(), "wed".into(), "fri".into()],
        );

        let yaml = generate_yaml(&schedule, &setup).unwrap();

        assert!(
            yaml.contains("main_morning"),
            "missing main_morning sequence_id"
        );
        assert!(yaml.contains("07:00"), "missing morning time");
        assert!(yaml.contains("mon"), "missing weekday filter");
    }

    #[test]
    fn test_afternoon_sequence_produced() {
        let setup = test_setup();
        let mut schedule = Schedule::default_seed_from(&setup);
        set_all_zone_days(&mut schedule, &setup, vec!["sat".into(), "sun".into()]);

        let yaml = generate_yaml(&schedule, &setup).unwrap();

        assert!(yaml.contains("main_afternoon"));
        assert!(yaml.contains("15:00"));
    }

    #[test]
    fn test_all_seven_days_omits_weekday_field() {
        let setup = test_setup();
        let mut schedule = Schedule::default_seed_from(&setup);
        set_all_zone_days(
            &mut schedule,
            &setup,
            vec![
                "mon".into(),
                "tue".into(),
                "wed".into(),
                "thu".into(),
                "fri".into(),
                "sat".into(),
                "sun".into(),
            ],
        );

        let yaml = generate_yaml(&schedule, &setup).unwrap();

        // weekday filter should be omitted when all 7 days selected
        assert!(
            !yaml.contains("weekday:"),
            "weekday should be absent for all-7-days"
        );
    }

    #[test]
    fn test_disabled_zone_excluded_from_sequence() {
        let setup = test_setup();
        let mut schedule = Schedule::default_seed_from(&setup);
        set_all_zone_days(&mut schedule, &setup, vec!["mon".into()]);
        // Explicitly disable zone_4 for both sessions.
        if let Some(zs) = schedule.zones.get_mut("zone_4") {
            zs.morning_enabled = false;
            zs.afternoon_enabled = false;
        }

        let yaml = generate_yaml(&schedule, &setup).unwrap();

        // zone_4 must appear in the controller zones list (the physical definitions)
        assert!(
            yaml.contains("zone_4"),
            "zone_4 should be in zone definitions"
        );

        // But zone_4 should NOT appear inside the sequences block.
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
        let setup = test_setup();
        let mut schedule = Schedule::default_seed_from(&setup);
        set_all_zone_days(&mut schedule, &setup, vec!["mon".into()]);

        let yaml = generate_yaml(&schedule, &setup).unwrap();

        assert!(yaml.contains("main_morning"));
        assert!(yaml.contains("main_afternoon"));
    }

    #[test]
    fn test_manual_sequence_emitted_when_zones_selected() {
        let setup = test_setup();
        let mut schedule = Schedule::default_seed_from(&setup);
        schedule.manual_zones.insert("zone_1".into(), 120);
        schedule.manual_zones.insert("zone_5".into(), 300);

        let yaml = generate_yaml(&schedule, &setup).unwrap();

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
        let setup = test_setup();
        let schedule = Schedule::default_seed_from(&setup); // manual_zones is empty
        let yaml = generate_yaml(&schedule, &setup).unwrap();
        assert!(
            !yaml.contains("manual"),
            "manual sequence should not appear when no zones selected"
        );
    }

    // ---------------------------------------------------------------------------
    // zone_group / slot-scheduling tests
    // ---------------------------------------------------------------------------

    /// Build a minimal IuSetup from a YAML string — useful for zone_group tests.
    fn setup_from_yaml(yaml: &str) -> IuSetup {
        serde_yaml::from_str(yaml).expect("test setup YAML failed to parse")
    }

    /// Minimal controller block shared across zone_group tests.
    /// Afternoon is disabled so the tests only need to reason about morning schedules.
    const CTRL_YAML: &str = r"
poll_interval_ms: 1000
controllers:
  - id: main
    name: Test Controller
    preamble_secs: 10
    postamble_secs: 10
    delay_secs: 5
    ha_master_entity: binary_sensor.irrigation_unlimited_c1_m
defaults:
  morning_time: '08:00'
  afternoon_time: '15:00'
  zone_morning_secs: 600
  zone_afternoon_secs: 600
  zone_morning_enabled: true
  zone_afternoon_enabled: false
";

    #[test]
    fn test_no_zone_group_serialises_sequences() {
        // Two zones with different active days and no zone_group → two singleton
        // slots → second sequence's start time must be offset from the first.
        let setup = setup_from_yaml(&format!(
            "{CTRL_YAML}
zones:
  - id: zone_a
    controller_id: main
    name: Zone A
    entity_id: switch.a
  - id: zone_b
    controller_id: main
    name: Zone B
    entity_id: switch.b
"
        ));

        let mut schedule = Schedule::default_seed_from(&setup);
        // zone_a runs Mon, zone_b runs Tue → two separate day-pattern groups.
        schedule
            .zone_active_days
            .insert("zone_a".into(), vec!["mon".into()]);
        schedule
            .zone_active_days
            .insert("zone_b".into(), vec!["tue".into()]);

        let yaml = generate_yaml(&schedule, &setup).unwrap();

        // Both sequences must exist but with different start times.
        assert!(yaml.contains("main_morning_1"), "missing first sequence");
        assert!(yaml.contains("main_morning_2"), "missing second sequence");
        // The first slot starts at 08:00; the second must NOT also be 08:00.
        let times: Vec<&str> = yaml
            .lines()
            .filter(|l| l.trim().starts_with("time:"))
            .collect();
        let morning_times: Vec<&str> = times.iter().filter(|l| l.contains(':')).copied().collect();
        // There should be two distinct morning schedule times.
        assert!(
            morning_times.len() >= 2,
            "expected at least two schedule time entries"
        );
        let unique_times: std::collections::HashSet<&str> = morning_times.iter().copied().collect();
        assert!(
            unique_times.len() >= 2,
            "sequences without zone_group should have different start times, got: {morning_times:?}"
        );
    }

    #[test]
    fn test_same_zone_group_allows_concurrent_start() {
        // Two zones with the same zone_group and different active days → one
        // slot → both sequences start at the session time.
        let setup = setup_from_yaml(&format!(
            "{CTRL_YAML}
zones:
  - id: zone_a
    controller_id: main
    name: Zone A
    entity_id: switch.a
    zone_group: pots
  - id: zone_b
    controller_id: main
    name: Zone B
    entity_id: switch.b
    zone_group: pots
"
        ));

        let mut schedule = Schedule::default_seed_from(&setup);
        schedule
            .zone_active_days
            .insert("zone_a".into(), vec!["mon".into()]);
        schedule
            .zone_active_days
            .insert("zone_b".into(), vec!["tue".into()]);

        let yaml = generate_yaml(&schedule, &setup).unwrap();

        // Both sequences must exist.
        assert!(yaml.contains("main_morning_1"), "missing first sequence");
        assert!(yaml.contains("main_morning_2"), "missing second sequence");
        // All morning schedule times should be identical (both start at 08:00).
        let morning_times: Vec<&str> = yaml
            .lines()
            .filter(|l| l.trim().starts_with("time:"))
            .collect();
        let unique_times: std::collections::HashSet<&str> = morning_times.iter().copied().collect();
        assert_eq!(
            unique_times.len(),
            1,
            "sequences in the same zone_group should share a start time, got: {morning_times:?}"
        );
    }

    #[test]
    fn test_mixed_zone_groups_serialises_correctly() {
        // Three zones: zone_a and zone_b share zone_group "pots"; zone_c has none.
        // Expected: zone_a and zone_b form one concurrent slot (start at 08:00),
        // zone_c forms a second slot (starts after the first slot completes).
        let setup = setup_from_yaml(&format!(
            "{CTRL_YAML}
zones:
  - id: zone_a
    controller_id: main
    name: Zone A
    entity_id: switch.a
    zone_group: pots
  - id: zone_b
    controller_id: main
    name: Zone B
    entity_id: switch.b
    zone_group: pots
  - id: zone_c
    controller_id: main
    name: Zone C
    entity_id: switch.c
"
        ));

        let mut schedule = Schedule::default_seed_from(&setup);
        schedule
            .zone_active_days
            .insert("zone_a".into(), vec!["mon".into()]);
        schedule
            .zone_active_days
            .insert("zone_b".into(), vec!["tue".into()]);
        schedule
            .zone_active_days
            .insert("zone_c".into(), vec!["wed".into()]);

        let yaml = generate_yaml(&schedule, &setup).unwrap();

        // All three sequences must be present.
        assert!(yaml.contains("main_morning_1"), "missing sequence 1");
        assert!(yaml.contains("main_morning_2"), "missing sequence 2");
        assert!(yaml.contains("main_morning_3"), "missing sequence 3");

        // Extract schedule time values from the YAML.
        let times: Vec<&str> = yaml
            .lines()
            .filter(|l| l.trim().starts_with("time:"))
            .map(|l| l.trim())
            .collect();
        assert_eq!(
            times.len(),
            3,
            "expected 3 schedule entries, got: {times:?}"
        );

        // The first two must share the same time (both in the "pots" slot).
        assert_eq!(
            times[0], times[1],
            "zone_a and zone_b should share a start time"
        );
        // The third must differ (zone_c is serialised after the first slot).
        assert_ne!(
            times[0], times[2],
            "zone_c should have a later start time than the 'pots' slot"
        );
    }

    #[test]
    fn test_parse_and_format_hhmm_helpers() {
        assert_eq!(parse_hhmm_to_secs("08:00"), 28800);
        assert_eq!(parse_hhmm_to_secs("00:00"), 0);
        assert_eq!(parse_hhmm_to_secs("23:59"), 86340);
        assert_eq!(format_secs_to_hhmm(28800), "08:00");
        assert_eq!(format_secs_to_hhmm(0), "00:00");
        // Wraps at midnight.
        assert_eq!(format_secs_to_hhmm(86400), "00:00");
        assert_eq!(format_secs_to_hhmm(86460), "00:01");
    }

    #[test]
    fn print_sample_yaml() {
        // Not a real assertion — useful for manual inspection during development.
        let setup = test_setup();
        let mut schedule = Schedule::default_seed_from(&setup);
        set_all_zone_days(
            &mut schedule,
            &setup,
            vec![
                "mon".into(),
                "tue".into(),
                "wed".into(),
                "thu".into(),
                "fri".into(),
                "sat".into(),
                "sun".into(),
            ],
        );

        let yaml = generate_yaml(&schedule, &setup).unwrap();
        println!("\n--- Sample YAML ---\n{yaml}\n---");
    }
}
