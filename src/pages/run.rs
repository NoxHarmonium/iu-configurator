use std::collections::HashMap;

use leptos::prelude::*;

use super::use_status_polling;
use crate::{
    server_fns::{
        IrrigationStatus, cancel_run, get_client_setup, get_irrigation_status, get_schedule,
        run_manual,
    },
    utils::time::{mmss_to_secs, secs_to_mmss},
};

#[component]
pub fn RunPage() -> impl IntoView {
    let schedule_res = Resource::new(|| (), |_| get_schedule());
    let status_res = Resource::new(|| (), |_| get_irrigation_status());
    let setup_res = Resource::new(|| (), |_| get_client_setup());

    let poll_ms = Signal::derive(move || {
        setup_res
            .get()
            .and_then(|r| r.ok())
            .map(|s| s.poll_interval_ms)
            .unwrap_or(5000)
    });
    use_status_polling(move || status_res.refetch(), poll_ms);

    // Per-zone state keyed by zone_id (populated once setup + schedule load).
    let zone_enabled: RwSignal<HashMap<String, bool>> = RwSignal::new(HashMap::new());
    let zone_duration: RwSignal<HashMap<String, String>> = RwSignal::new(HashMap::new());

    // Initialise slots from setup (so all zones appear even before schedule loads).
    Effect::new(move |_| {
        if let Some(Ok(setup)) = setup_res.get() {
            zone_enabled.update(|m| {
                for z in &setup.zones {
                    m.entry(z.id.clone()).or_insert(false);
                }
            });
            zone_duration.update(|m| {
                for z in &setup.zones {
                    m.entry(z.id.clone()).or_insert_with(|| "00:00".to_string());
                }
            });
        }
    });

    // Populate duration defaults (afternoon_secs) once schedule loads.
    Effect::new(move |_| {
        if let Some(Ok(s)) = schedule_res.get() {
            zone_duration.update(|m| {
                for (id, zs) in &s.zones {
                    m.insert(id.clone(), secs_to_mmss(zs.afternoon_secs));
                }
            });
        }
    });

    // Run action — iterates zone_enabled keys so it needs no zone list reference.
    let run_action = Action::new(move |_: &()| {
        let enabled_snap = zone_enabled.get();
        let duration_snap = zone_duration.get();
        async move {
            let mut manual_zones: HashMap<String, u32> = HashMap::new();
            for (id, &is_enabled) in &enabled_snap {
                if is_enabled {
                    let secs = duration_snap.get(id).map(|d| mmss_to_secs(d)).unwrap_or(0);
                    if secs > 0 {
                        manual_zones.insert(id.clone(), secs);
                    }
                }
            }
            run_manual(manual_zones).await
        }
    });

    let is_running = run_action.pending();
    let run_error = RwSignal::new(Option::<String>::None);
    let run_ok = RwSignal::new(false);

    let cancel_action = Action::new(move |_: &()| async move { cancel_run().await });
    let is_cancelling = cancel_action.pending();
    let cancel_error = RwSignal::new(Option::<String>::None);

    Effect::new(move |_| {
        if let Some(result) = cancel_action.value().get() {
            if let Err(e) = result {
                cancel_error.set(Some(e.to_string()));
            } else {
                cancel_error.set(None);
                status_res.refetch();
            }
        }
    });

    Effect::new(move |_| {
        if let Some(result) = run_action.value().get() {
            match result {
                Ok(_) => {
                    run_error.set(None);
                    run_ok.set(true);
                    status_res.refetch();
                }
                Err(e) => {
                    run_error.set(Some(e.to_string()));
                    run_ok.set(false);
                }
            }
        }
    });

    let is_active = move || {
        status_res
            .get()
            .and_then(|r| r.ok())
            .map(|s| s == IrrigationStatus::Active)
            .unwrap_or(false)
    };

    view! {
        <div class="page">
            <h1 class="page__title">"Force Run"</h1>

            // ── Status banner ────────────────────────────────────────────────
            <Transition fallback=|| ()>
                {move || {
                    status_res.get().map(|result| {
                        match result {
                            Ok(IrrigationStatus::Active) => view! {
                                <div class="banner banner--active">
                                    "💧 Watering is currently active — force run is disabled."
                                </div>
                            }.into_any(),
                            Ok(IrrigationStatus::Unknown(ref msg)) => view! {
                                <div class="banner banner--warn">
                                    {format!("⚠️ Irrigation status unknown: {msg}")}
                                </div>
                            }.into_any(),
                            _ => view! { <div></div> }.into_any(),
                        }
                    })
                }}
            </Transition>

            // ── Zone table ───────────────────────────────────────────────────
            <Transition fallback=|| view! { <p class="loading">"Loading zones…"</p> }>
                {move || {
                    let setup = match (setup_res.get(), schedule_res.get()) {
                        (None, _) | (_, None) => return None,
                        (Some(Err(e)), _) => return Some(
                            view! { <p class="error">{format!("Failed to load setup: {e}")}</p> }
                                .into_any(),
                        ),
                        (_, Some(Err(e))) => return Some(
                            view! {
                                <p class="error">{format!("Failed to load schedule: {e}")}</p>
                            }
                            .into_any(),
                        ),
                        (Some(Ok(s)), Some(Ok(_))) => s,
                    };

                    Some(view! {
                        <div class="zone-table">
                            <div class="zone-table__header">
                                <span>"Zone"</span>
                                <span>"Run"</span>
                                <span>"Duration (MM:SS)"</span>
                            </div>
                            {setup.zones.into_iter().map(|zone| {
                                let zid1 = zone.id.clone();
                                let zid2 = zone.id.clone();
                                let zid3 = zone.id.clone();
                                let zid4 = zone.id;
                                let zone_name = zone.name;

                                view! {
                                    <div class="zone-table__row">
                                        <span class="zone-table__name">{zone_name}</span>

                                        // Enable toggle
                                        <label class="toggle">
                                            <input
                                                type="checkbox"
                                                class="toggle__input"
                                                prop:checked=move || {
                                                    zone_enabled.get().get(&zid1).copied().unwrap_or(false)
                                                }
                                                prop:disabled=move || is_active() || is_running.get()
                                                on:change=move |ev| {
                                                    zone_enabled.update(|m| {
                                                        m.insert(zid2.clone(), event_target_checked(&ev));
                                                    });
                                                    run_ok.set(false);
                                                }
                                            />
                                            <span class="toggle__slider"></span>
                                        </label>

                                        // Duration input
                                        <input
                                            type="text"
                                            class="field-row__input field-row__input--duration"
                                            placeholder="MM:SS"
                                            prop:value=move || {
                                                zone_duration
                                                    .get()
                                                    .get(&zid3)
                                                    .cloned()
                                                    .unwrap_or_default()
                                            }
                                            prop:disabled=move || is_active() || is_running.get()
                                            on:input=move |ev| {
                                                zone_duration.update(|m| {
                                                    m.insert(zid4.clone(), event_target_value(&ev));
                                                });
                                                run_ok.set(false);
                                            }
                                        />
                                    </div>
                                }
                            }).collect_view()}
                        </div>

                        // ── Run / stop buttons ───────────────────────
                        <div class="form-actions">
                            <button
                                type="button"
                                class="btn btn--primary"
                                prop:disabled=move || is_active() || is_running.get()
                                on:click=move |_| { run_action.dispatch(()); }
                            >
                                {move || if is_running.get() { "Starting…" } else { "Force Run" }}
                            </button>
                            <button
                                type="button"
                                class="btn btn--danger"
                                prop:disabled=move || !is_active() || is_cancelling.get()
                                on:click=move |_| { cancel_action.dispatch(()); }
                            >
                                {move || if is_cancelling.get() { "Stopping…" } else { "⏹ Emergency Stop" }}
                            </button>
                            {move || run_error.get().map(|e| view! {
                                <p class="error">"Error: " {e}</p>
                            })}
                            {move || cancel_error.get().map(|e| view! {
                                <p class="error">"Stop error: " {e}</p>
                            })}
                            {move || run_ok.get().then(|| view! {
                                <p class="success">"✓ Manual run triggered."</p>
                            })}
                        </div>
                    }.into_any())
                }}
            </Transition>
        </div>
    }
}
