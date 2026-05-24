/// Wrap bare `time:` scalar values in single quotes.
/// TODO: There has to be a better way than this!
pub fn quote_time_fields(yaml: String) -> String {
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
