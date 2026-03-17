pub mod config;
pub mod run;
pub mod schedule;

/// Starts a recurring poll that calls `refetch()` on `status_res` every
/// [`crate::definitions::STATUS_POLL_INTERVAL_MS`] milliseconds, keeping the status
/// banner and button states in sync with Home Assistant without requiring a page refresh.
///
/// This is a no-op on the server (SSR) — the interval only runs in the browser.
pub fn use_status_polling(on_tick: impl Fn() + Clone + 'static) {
    #[cfg(not(feature = "ssr"))]
    {
        use crate::definitions::STATUS_POLL_INTERVAL_MS;
        use leptos_use::use_interval_fn;
        let _ = use_interval_fn(on_tick, STATUS_POLL_INTERVAL_MS);
    }
    #[cfg(feature = "ssr")]
    let _ = on_tick; // suppress unused variable warning
}
