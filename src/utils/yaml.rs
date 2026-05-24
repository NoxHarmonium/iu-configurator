/// Wrap bare `time:` scalar values in single quotes.
// TODO: serde_yaml (YAML 1.2) leaves bare "HH:MM" scalars unquoted, but Home Assistant's
// PyYAML parser (YAML 1.1) interprets them as sexagesimal numbers. A proper fix would be
// a custom serde serializer that quotes time-format strings natively; this line-by-line
// post-processing is the pragmatic workaround until that is implemented.
#[must_use]
pub fn quote_time_fields(yaml: &str) -> String {
    let trailing_newline = yaml.ends_with('\n');
    let mut result = yaml
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if let Some(value_part) = trimmed.strip_prefix("time:") {
                let value = value_part.trim();
                if !value.is_empty() && !value.starts_with('\'') && !value.starts_with('"') {
                    let indent = &line[..line.len() - trimmed.len()];
                    return format!("{indent}time: '{value}'");
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
