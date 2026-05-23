pub(super) fn parse_hhmm_to_secs(hhmm: &str) -> u32 {
    let mut parts = hhmm.splitn(2, ':');
    let h: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let m: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    h * 3600 + m * 60
}

pub(super) fn format_secs_to_hhmm(secs: u32) -> String {
    let secs = secs % 86400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    format!("{h:02}:{m:02}")
}

pub(super) fn weekday_filter(days: &[String]) -> Option<Vec<String>> {
    if days.len() >= 7 {
        None
    } else {
        Some(days.to_vec())
    }
}

pub(super) fn format_duration(secs: u32) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

pub(super) fn days_label(days: &[String]) -> String {
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

pub(super) fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
