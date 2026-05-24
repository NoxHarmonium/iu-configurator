use super::*;
use crate::models::AppState;
use crate::models::IUCConfig;

fn test_setup() -> IUCConfig {
    let yaml = include_str!("../../dev/config/iuc-config.yaml");
    serde_yaml::from_str(yaml).expect("dev/config/iuc-config.yaml failed to parse")
}

fn set_all_zone_days(schedule: &mut AppState, setup: &IUCConfig, days: Vec<String>) {
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
    let schedule = AppState::default_seed_from(&setup);
    let yaml = generate_yaml(&schedule, &setup).unwrap();
    assert!(!yaml.contains("sequences"));
    assert!(yaml.contains("controllers"));
    assert!(yaml.contains("zone_1"));
}

#[test]
fn test_morning_sequence_produced() {
    let setup = test_setup();
    let mut schedule = AppState::default_seed_from(&setup);
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
    let mut schedule = AppState::default_seed_from(&setup);
    set_all_zone_days(&mut schedule, &setup, vec!["sat".into(), "sun".into()]);

    let yaml = generate_yaml(&schedule, &setup).unwrap();

    assert!(yaml.contains("main_afternoon"));
    assert!(yaml.contains("15:00"));
}

#[test]
fn test_all_seven_days_omits_weekday_field() {
    let setup = test_setup();
    let mut schedule = AppState::default_seed_from(&setup);
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

    assert!(
        !yaml.contains("weekday:"),
        "weekday should be absent for all-7-days"
    );
}

#[test]
fn test_disabled_zone_excluded_from_sequence() {
    let setup = test_setup();
    let mut schedule = AppState::default_seed_from(&setup);
    set_all_zone_days(&mut schedule, &setup, vec!["mon".into()]);
    if let Some(zs) = schedule.zones.get_mut("zone_4") {
        zs.morning_enabled = false;
        zs.afternoon_enabled = false;
    }

    let yaml = generate_yaml(&schedule, &setup).unwrap();

    assert!(
        yaml.contains("zone_4"),
        "zone_4 should be in zone definitions"
    );

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
    let mut schedule = AppState::default_seed_from(&setup);
    set_all_zone_days(&mut schedule, &setup, vec!["mon".into()]);

    let yaml = generate_yaml(&schedule, &setup).unwrap();

    assert!(yaml.contains("main_morning"));
    assert!(yaml.contains("main_afternoon"));
}

#[test]
fn test_manual_sequence_emitted_when_zones_selected() {
    let setup = test_setup();
    let mut schedule = AppState::default_seed_from(&setup);
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
    assert!(
        !yaml.contains("schedules:"),
        "manual sequence should have no schedules block"
    );
}

#[test]
fn test_manual_sequence_absent_when_no_zones() {
    let setup = test_setup();
    let schedule = AppState::default_seed_from(&setup);
    let yaml = generate_yaml(&schedule, &setup).unwrap();
    assert!(
        !yaml.contains("manual"),
        "manual sequence should not appear when no zones selected"
    );
}

fn setup_from_yaml(yaml: &str) -> IUCConfig {
    serde_yaml::from_str(yaml).expect("test setup YAML failed to parse")
}

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

    let mut schedule = AppState::default_seed_from(&setup);
    schedule
        .zone_active_days
        .insert("zone_a".into(), vec!["mon".into()]);
    schedule
        .zone_active_days
        .insert("zone_b".into(), vec!["tue".into()]);

    let yaml = generate_yaml(&schedule, &setup).unwrap();

    assert!(yaml.contains("main_morning_1"), "missing first sequence");
    assert!(yaml.contains("main_morning_2"), "missing second sequence");
    let times: Vec<&str> = yaml
        .lines()
        .filter(|l| l.trim().starts_with("time:"))
        .collect();
    let morning_times: Vec<&str> = times.iter().filter(|l| l.contains(':')).copied().collect();
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

    let mut schedule = AppState::default_seed_from(&setup);
    schedule
        .zone_active_days
        .insert("zone_a".into(), vec!["mon".into()]);
    schedule
        .zone_active_days
        .insert("zone_b".into(), vec!["tue".into()]);

    let yaml = generate_yaml(&schedule, &setup).unwrap();

    assert!(yaml.contains("main_morning_1"), "missing first sequence");
    assert!(yaml.contains("main_morning_2"), "missing second sequence");
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

    let mut schedule = AppState::default_seed_from(&setup);
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

    assert!(yaml.contains("main_morning_1"), "missing sequence 1");
    assert!(yaml.contains("main_morning_2"), "missing sequence 2");
    assert!(yaml.contains("main_morning_3"), "missing sequence 3");

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

    assert_eq!(
        times[0], times[1],
        "zone_a and zone_b should share a start time"
    );
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
    assert_eq!(format_secs_to_hhmm(86400), "00:00");
    assert_eq!(format_secs_to_hhmm(86460), "00:01");
}

#[test]
fn print_sample_yaml() {
    let setup = test_setup();
    let mut schedule = AppState::default_seed_from(&setup);
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
