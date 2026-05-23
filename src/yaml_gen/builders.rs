use std::collections::HashMap;

use crate::models::{Schedule, ScheduleMode, ZoneSchedule};
use crate::setup::IuSetup;

use super::schema::{IuController, IuEveryNDays, IuSchedule, IuSeqZone, IuSequence, IuZone};
use super::time::{
    capitalize_first, days_label, format_duration, format_secs_to_hhmm, parse_hhmm_to_secs,
    weekday_filter,
};

type DayGroup = (Vec<String>, Vec<(String, u32)>);

pub(super) fn build_controllers(schedule: &Schedule, setup: &IuSetup) -> Vec<IuController> {
    setup
        .controllers
        .iter()
        .map(|ctrl| {
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
