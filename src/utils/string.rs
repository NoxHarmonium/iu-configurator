pub fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalize_first_empty() {
        assert_eq!(capitalize_first(""), "");
    }

    #[test]
    fn capitalize_first_lowercase() {
        assert_eq!(capitalize_first("morning"), "Morning");
    }

    #[test]
    fn capitalize_first_already_uppercase() {
        assert_eq!(capitalize_first("Morning"), "Morning");
    }
}
