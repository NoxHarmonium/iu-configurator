pub mod config;
pub mod run;
pub mod schedule;

use leptos_use::use_interval_fn;

use crate::definitions::STATUS_POLL_INTERVAL_MS;

/// Starts a recurring poll that calls `refetch()` on `status_res` every
/// [`STATUS_POLL_INTERVAL_MS`] milliseconds, keeping the status banner and button
/// states in sync with Home Assistant without requiring a page refresh.
pub fn use_status_polling(on_tick: impl Fn() + Clone + 'static) {
    let _ = use_interval_fn(on_tick, STATUS_POLL_INTERVAL_MS);
}
