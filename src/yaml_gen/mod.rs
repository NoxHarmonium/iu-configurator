mod builders;
mod quote;
mod schema;
mod time;

#[cfg(test)]
mod tests;

use crate::models::Schedule;
use crate::setup::IuSetup;

use builders::build_controllers;
use quote::quote_time_fields;
use schema::IuConfig;

#[cfg(test)]
use time::{format_duration, format_secs_to_hhmm, parse_hhmm_to_secs};

/// Generate an `irrigation_unlimited` YAML string from the active schedule.
pub fn generate_yaml(schedule: &Schedule, setup: &IuSetup) -> Result<String, serde_yaml::Error> {
    let controllers = build_controllers(schedule, setup);
    let yaml = serde_yaml::to_string(&IuConfig { controllers })?;
    // serde_yaml targets YAML 1.2 and leaves "HH:MM" unquoted, but Home
    // Assistant uses PyYAML which defaults to YAML 1.1 where bare "HH:MM"
    // scalars are interpreted as sexagesimal numbers. Quote them explicitly.
    Ok(quote_time_fields(yaml))
}
