/// Convert seconds to "MM:SS" display format.
pub fn secs_to_mmss(secs: u32) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{m:02}:{s:02}")
}

/// Parse "MM:SS" (or "HH:MM:SS", or plain seconds) back to total seconds.
/// Returns 0 on any parse failure so malformed input is treated as zero duration.
pub fn mmss_to_secs(s: &str) -> u32 {
    let parts: Vec<&str> = s.trim().splitn(3, ':').collect();
    match parts.as_slice() {
        [mm, ss] => {
            let m = mm.parse::<u32>().unwrap_or(0);
            let s = ss.parse::<u32>().unwrap_or(0);
            m * 60 + s
        }
        [hh, mm, ss] => {
            let h = hh.parse::<u32>().unwrap_or(0);
            let m = mm.parse::<u32>().unwrap_or(0);
            let s = ss.parse::<u32>().unwrap_or(0);
            h * 3600 + m * 60 + s
        }
        [plain] => plain.parse::<u32>().unwrap_or(0),
        _ => 0,
    }
}

pub fn parse_hhmm_to_secs(hhmm: &str) -> u32 {
    let mut parts = hhmm.splitn(2, ':');
    let h: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let m: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    h * 3600 + m * 60
}

pub fn format_secs_to_hhmm(secs: u32) -> String {
    let secs = secs % 86400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    format!("{h:02}:{m:02}")
}

pub fn format_duration(secs: u32) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

pub fn weekday_filter(days: &[String]) -> Option<Vec<String>> {
    if days.len() >= 7 {
        None
    } else {
        Some(days.to_vec())
    }
}

pub fn days_label(days: &[String]) -> String {
    use super::string::capitalize_first;

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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secs_to_mmss_formats_values() {
        assert_eq!(secs_to_mmss(0), "00:00");
        assert_eq!(secs_to_mmss(75), "01:15");
        assert_eq!(secs_to_mmss(600), "10:00");
    }

    #[test]
    fn mmss_to_secs_parses_supported_formats() {
        assert_eq!(mmss_to_secs("01:15"), 75);
        assert_eq!(mmss_to_secs("01:02:03"), 3723);
        assert_eq!(mmss_to_secs("45"), 45);
    }

    #[test]
    fn mmss_to_secs_invalid_inputs_return_zero() {
        assert_eq!(mmss_to_secs("bad"), 0);
        assert_eq!(mmss_to_secs(""), 0);
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
    fn test_parse_and_format_hhmm_helpers() {
        assert_eq!(parse_hhmm_to_secs("08:00"), 28800);
        assert_eq!(parse_hhmm_to_secs("00:00"), 0);
        assert_eq!(parse_hhmm_to_secs("23:59"), 86340);
        assert_eq!(format_secs_to_hhmm(28800), "08:00");
        assert_eq!(format_secs_to_hhmm(0), "00:00");
        assert_eq!(format_secs_to_hhmm(86400), "00:00");
        assert_eq!(format_secs_to_hhmm(86460), "00:01");
    }
}
