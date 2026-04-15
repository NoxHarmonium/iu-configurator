pub mod config;
pub mod run;
pub mod schedule;

/// Starts a recurring poll that calls `refetch()` on `status_res` every
/// `interval_ms` milliseconds, keeping the status banner and button states
/// in sync with Home Assistant without requiring a page refresh.
///
/// This is a no-op on the server (SSR) — the interval only runs in the browser.
pub fn use_status_polling(
    on_tick: impl Fn() + Clone + 'static,
    interval_ms: leptos::prelude::Signal<u64>,
) {
    #[cfg(not(feature = "ssr"))]
    {
        use leptos_use::use_interval_fn;
        let _ = use_interval_fn(on_tick, interval_ms);
    }
    #[cfg(feature = "ssr")]
    let _ = (on_tick, interval_ms); // suppress unused variable warnings
}
