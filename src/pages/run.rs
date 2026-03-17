use std::collections::HashMap;

use leptos::prelude::*;

use super::use_status_polling;
use crate::{
    definitions::ZONES,
    pages::config::{mmss_to_secs, secs_to_mmss},
    server_fns::{cancel_run, get_irrigation_status, get_schedule, run_manual, IrrigationStatus},
};

#[component]
pub fn RunPage() -> impl IntoView {
    let schedule_res = Resource::new(|| (), |_| get_schedule());
    let status_res = Resource::new(|| (), |_| get_irrigation_status());
    use_status_polling(move || status_res.refetch());

    // Per-zone signals: enabled (toggle) and duration string (MM:SS)
    let zone_enabled: Vec<RwSignal<bool>> = ZONES.iter().map(|_| RwSignal::new(false)).collect();
    let zone_duration: Vec<RwSignal<String>> = ZONES
        .iter()
        .map(|_| RwSignal::new("00:00".to_string()))
        .collect();

    // Populate duration defaults (afternoon_secs) once schedule loads
    let zone_duration_init = zone_duration.clone();
    Effect::new(move |_| {
        if let Some(Ok(s)) = schedule_res.get() {
            for (i, zone_def) in ZONES.iter().enumerate() {
                if let Some(zs) = s.zones.get(zone_def.id) {
                    zone_duration_init[i].set(secs_to_mmss(zs.afternoon_secs));
                }
            }
        }
    });

    // Run action
    let zone_enabled_run = zone_enabled.clone();
    let zone_duration_run = zone_duration.clone();
    let run_action = Action::new(move |_: &()| {
        let enabled_snap: Vec<bool> = zone_enabled_run.iter().map(|s| s.get()).collect();
        let duration_snap: Vec<String> = zone_duration_run.iter().map(|s| s.get()).collect();
        async move {
            let mut manual_zones: HashMap<String, u32> = HashMap::new();
            for (i, zone_def) in ZONES.iter().enumerate() {
                if enabled_snap[i] {
                    let secs = mmss_to_secs(&duration_snap[i]);
                    if secs > 0 {
                        manual_zones.insert(zone_def.id.to_string(), secs);
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
                    schedule_res.get().map(|result| match result {
                        Err(e) => view! {
                            <p class="error">{format!("Failed to load schedule: {e}")}</p>
                        }.into_any(),
                        Ok(_) => {
                            let zone_enabled_view = zone_enabled.clone();
                            let zone_duration_view = zone_duration.clone();

                            view! {
                                <div class="zone-table">
                                    <div class="zone-table__header">
                                        <span>"Zone"</span>
                                        <span>"Run"</span>
                                        <span>"Duration (MM:SS)"</span>
                                    </div>
                                    {ZONES.iter().enumerate().map(|(i, zone_def)| {
                                        let enabled_sig = zone_enabled_view[i];
                                        let duration_sig = zone_duration_view[i];
                                        let name = zone_def.name;

                                        view! {
                                            <div class="zone-table__row">
                                                <span class="zone-table__name">{name}</span>

                                                // Enable toggle
                                                <label class="toggle">
                                                    <input
                                                        type="checkbox"
                                                        class="toggle__input"
                                                        prop:checked=move || enabled_sig.get()
                                                        prop:disabled=move || is_active() || is_running.get()
                                                        on:change=move |ev| {
                                                            enabled_sig.set(event_target_checked(&ev));
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
                                                    prop:value=move || duration_sig.get()
                                                    prop:disabled=move || is_active() || is_running.get()
                                                    on:input=move |ev| {
                                                        duration_sig.set(event_target_value(&ev));
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
                            }.into_any()
                        }
                    })
                }}
            </Transition>
        </div>
    }
}
