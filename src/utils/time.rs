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
}
