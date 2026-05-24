use std::collections::HashMap;

use crate::models::IUCConfig;
use crate::models::{AppState, AppStateMode, ZoneAppState};

use super::schema::{IuController, IuEveryNDays, IuSchedule, IuSeqZone, IuSequence, IuZone};
use super::time::{
    capitalize_first, days_label, format_duration, format_secs_to_hhmm, parse_hhmm_to_secs,
    weekday_filter,
};

struct DayGroup {
    days: Vec<String>,
    zone_durations: Vec<(String, u32)>,
}

struct WeekdayBuildCtx<'a> {
    setup: &'a IUCConfig,
    controller_id: &'a str,
    zone_schedules: &'a HashMap<String, ZoneAppState>,
    zone_active_days: &'a HashMap<String, Vec<String>>,
    session_time: &'a str,
    session: &'a str,
    delay_secs: u32,
    preamble_secs: u32,
    postamble_secs: u32,
}

pub(super) fn build_controllers(schedule: &AppState, setup: &IUCConfig) -> Vec<IuController> {
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

            let is_periodic = schedule.schedule_mode == AppStateMode::Periodic;
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
                let morning_ctx = WeekdayBuildCtx {
                    setup,
                    controller_id: ctrl.id.as_str(),
                    zone_schedules: &schedule.zones,
                    zone_active_days: &schedule.zone_active_days,
                    session_time: &schedule.morning_time,
                    session: "morning",
                    delay_secs: ctrl.delay_secs,
                    preamble_secs: ctrl.preamble_secs,
                    postamble_secs: ctrl.postamble_secs,
                };
                sequences.extend(build_weekday_sequences(
                    &morning_ctx,
                    |zs| zs.morning_enabled,
                    |zs| zs.morning_secs,
                ));

                let afternoon_ctx = WeekdayBuildCtx {
                    setup,
                    controller_id: ctrl.id.as_str(),
                    zone_schedules: &schedule.zones,
                    zone_active_days: &schedule.zone_active_days,
                    session_time: &schedule.afternoon_time,
                    session: "afternoon",
                    delay_secs: ctrl.delay_secs,
                    preamble_secs: ctrl.preamble_secs,
                    postamble_secs: ctrl.postamble_secs,
                };
                sequences.extend(build_weekday_sequences(
                    &afternoon_ctx,
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

fn build_weekday_sequences<F, G>(
    ctx: &WeekdayBuildCtx<'_>,
    is_enabled: F,
    get_secs: G,
) -> Vec<IuSequence>
where
    F: Fn(&ZoneAppState) -> bool,
    G: Fn(&ZoneAppState) -> u32,
{
    let groups = build_day_groups(ctx, &is_enabled, &get_secs);

    if groups.is_empty() {
        return Vec::new();
    }

    let concurrent_keys = build_concurrent_keys(ctx.setup, &groups);
    let slots = build_slots(&concurrent_keys);

    let total = groups.len();
    let mut current_secs = parse_hhmm_to_secs(ctx.session_time);
    let mut result: Vec<IuSequence> = Vec::new();
    let mut seq_num: usize = 0;

    for slot in &slots {
        let slot_start = format_secs_to_hhmm(current_secs);

        let slot_duration = slot
            .iter()
            .map(|&gi| group_runtime_secs(&groups[gi], ctx))
            .max()
            .unwrap_or(0);

        for &gi in slot {
            seq_num += 1;
            let group = &groups[gi];
            let seq_id = if total == 1 {
                format!("{}_{}", ctx.controller_id, ctx.session)
            } else {
                format!("{}_{}_{}", ctx.controller_id, ctx.session, seq_num)
            };
            let seq_zones: Vec<IuSeqZone> = group
                .zone_durations
                .iter()
                .map(|(zone_id, secs)| IuSeqZone {
                    zone_id: zone_id.clone(),
                    duration: format_duration(*secs),
                })
                .collect();
            result.push(IuSequence {
                name: format!(
                    "{} ({})",
                    capitalize_first(ctx.session),
                    days_label(&group.days)
                ),
                sequence_id: seq_id,
                delay: format_duration(ctx.delay_secs),
                schedules: vec![IuSchedule {
                    name: capitalize_first(ctx.session),
                    time: slot_start.clone(),
                    weekday: weekday_filter(&group.days),
                    day: None,
                }],
                zones: seq_zones,
            });
        }

        current_secs += slot_duration;
    }

    result
}

fn build_day_groups<F, G>(ctx: &WeekdayBuildCtx<'_>, is_enabled: &F, get_secs: &G) -> Vec<DayGroup>
where
    F: Fn(&ZoneAppState) -> bool,
    G: Fn(&ZoneAppState) -> u32,
{
    let mut groups: Vec<DayGroup> = Vec::new();

    for zone in ctx
        .setup
        .zones
        .iter()
        .filter(|z| z.controller_id == ctx.controller_id)
    {
        let days = match sorted_days(ctx.zone_active_days.get(zone.id.as_str())) {
            Some(d) => d,
            None => continue,
        };

        let secs = match ctx.zone_schedules.get(zone.id.as_str()) {
            Some(zs) if is_enabled(zs) && get_secs(zs) > 0 => get_secs(zs),
            _ => continue,
        };

        if let Some(group) = groups.iter_mut().find(|g| g.days == days) {
            group.zone_durations.push((zone.id.clone(), secs));
        } else {
            groups.push(DayGroup {
                days,
                zone_durations: vec![(zone.id.clone(), secs)],
            });
        }
    }

    groups
}

fn sorted_days(days: Option<&Vec<String>>) -> Option<Vec<String>> {
    const DAY_ORDER: &[&str] = &["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

    match days {
        Some(d) if !d.is_empty() => {
            let mut sorted = d.clone();
            sorted.sort_by_key(|d| DAY_ORDER.iter().position(|&o| o == d).unwrap_or(7));
            Some(sorted)
        }
        _ => None,
    }
}

fn build_concurrent_keys(setup: &IUCConfig, groups: &[DayGroup]) -> Vec<Option<String>> {
    let zone_group_by_id: HashMap<&str, Option<&str>> = setup
        .zones
        .iter()
        .map(|z| (z.id.as_str(), z.zone_group.as_deref()))
        .collect();

    groups
        .iter()
        .map(|group| {
            let first_group = match zone_group_by_id
                .get(group.zone_durations[0].0.as_str())
                .copied()
                .flatten()
            {
                Some(g) => g,
                None => return None,
            };

            for (zone_id, _) in &group.zone_durations[1..] {
                if zone_group_by_id.get(zone_id.as_str()).copied().flatten() != Some(first_group) {
                    return None;
                }
            }

            Some(first_group.to_string())
        })
        .collect()
}

fn build_slots(concurrent_keys: &[Option<String>]) -> Vec<Vec<usize>> {
    let mut slots: Vec<Vec<usize>> = Vec::new();
    let mut assigned = vec![false; concurrent_keys.len()];

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

    slots
}

fn group_runtime_secs(group: &DayGroup, ctx: &WeekdayBuildCtx<'_>) -> u32 {
    let zone_secs: u32 = group.zone_durations.iter().map(|(_, s)| s).sum();
    let n = group.zone_durations.len() as u32;
    ctx.preamble_secs + zone_secs + ctx.delay_secs * n.saturating_sub(1) + ctx.postamble_secs
}

fn build_seq_zones(
    setup: &IUCConfig,
    controller_id: &str,
    zone_schedules: &HashMap<String, ZoneAppState>,
    is_enabled: impl Fn(&ZoneAppState) -> bool,
    get_secs: impl Fn(&ZoneAppState) -> u32,
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
    setup: &IUCConfig,
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
