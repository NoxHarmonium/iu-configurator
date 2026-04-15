use leptos::prelude::*;
use std::collections::HashMap;

use super::use_status_polling;
use crate::server_fns::{
    ClientSetupInfo, IrrigationStatus, get_client_setup, get_irrigation_status, get_schedule,
    save_schedule,
};

#[component]
pub fn ConfigPage() -> impl IntoView {
    let schedule_res = Resource::new(|| (), |_| get_schedule());
    let status_res = Resource::new(|| (), |_| get_irrigation_status());
    let setup_res = Resource::new(|| (), |_| get_client_setup());
    let poll_ms = Signal::derive(move || {
        setup_res
            .get()
            .and_then(|r| r.ok())
            .map(|s: ClientSetupInfo| s.poll_interval_ms)
            .unwrap_or(5000)
    });
    use_status_polling(move || status_res.refetch(), poll_ms);

    // Local signals for form fields — populated from schedule_res
    let morning_time = RwSignal::new("07:00".to_string());
    let afternoon_time = RwSignal::new("15:00".to_string());

    // Per-zone signals keyed by zone id
    let zone_morning_enabled: RwSignal<HashMap<String, bool>> = RwSignal::new(HashMap::new());
    let zone_afternoon_enabled: RwSignal<HashMap<String, bool>> = RwSignal::new(HashMap::new());
    let zone_morning: RwSignal<HashMap<String, String>> = RwSignal::new(HashMap::new());
    let zone_afternoon: RwSignal<HashMap<String, String>> = RwSignal::new(HashMap::new());

    // Populate once both resources arrive; track both so effect re-runs when either changes
    Effect::new(move |_| {
        let setup_opt = setup_res.get();
        let schedule_opt = schedule_res.get();

        let Some(Ok(setup)) = setup_opt else {
            return;
        };

        zone_morning_enabled.update(|m| {
            for z in &setup.zones {
                m.entry(z.id.clone()).or_insert(true);
            }
        });
        zone_afternoon_enabled.update(|m| {
            for z in &setup.zones {
                m.entry(z.id.clone()).or_insert(true);
            }
        });
        zone_morning.update(|m| {
            for z in &setup.zones {
                m.entry(z.id.clone()).or_insert_with(|| "00:00".to_string());
            }
        });
        zone_afternoon.update(|m| {
            for z in &setup.zones {
                m.entry(z.id.clone()).or_insert_with(|| "00:00".to_string());
            }
        });

        if let Some(Ok(s)) = schedule_opt {
            morning_time.set(s.morning_time.clone());
            afternoon_time.set(s.afternoon_time.clone());
            for z in &setup.zones {
                if let Some(zs) = s.zones.get(z.id.as_str()) {
                    zone_morning_enabled.update(|m| {
                        m.insert(z.id.clone(), zs.morning_enabled);
                    });
                    zone_afternoon_enabled.update(|m| {
                        m.insert(z.id.clone(), zs.afternoon_enabled);
                    });
                    zone_morning.update(|m| {
                        m.insert(z.id.clone(), secs_to_mmss(zs.morning_secs));
                    });
                    zone_afternoon.update(|m| {
                        m.insert(z.id.clone(), secs_to_mmss(zs.afternoon_secs));
                    });
                }
            }
        }
    });

    // Save action
    let save_action = Action::new(move |_: &()| {
        let mt = morning_time.get();
        let at = afternoon_time.get();
        let morning_enabled_snap = zone_morning_enabled.get();
        let afternoon_enabled_snap = zone_afternoon_enabled.get();
        let morning_snap = zone_morning.get();
        let afternoon_snap = zone_afternoon.get();

        async move {
            match get_schedule().await {
                Ok(mut s) => {
                    s.morning_time = mt;
                    s.afternoon_time = at;
                    let zone_ids: Vec<String> = morning_enabled_snap.keys().cloned().collect();
                    for zid in &zone_ids {
                        if let Some(zs) = s.zones.get_mut(zid.as_str()) {
                            zs.morning_enabled =
                                morning_enabled_snap.get(zid).copied().unwrap_or(true);
                            zs.afternoon_enabled =
                                afternoon_enabled_snap.get(zid).copied().unwrap_or(true);
                            zs.morning_secs = mmss_to_secs(
                                morning_snap.get(zid).map(|s| s.as_str()).unwrap_or("00:00"),
                            );
                            zs.afternoon_secs = mmss_to_secs(
                                afternoon_snap
                                    .get(zid)
                                    .map(|s| s.as_str())
                                    .unwrap_or("00:00"),
                            );
                        }
                    }
                    save_schedule(s).await
                }
                Err(e) => Err(e),
            }
        }
    });

    let is_saving = save_action.pending();
    let save_error = RwSignal::new(Option::<String>::None);
    let save_ok = RwSignal::new(false);

    Effect::new(move |_| {
        if let Some(result) = save_action.value().get() {
            match result {
                Ok(_) => {
                    save_error.set(None);
                    save_ok.set(true);
                }
                Err(e) => {
                    save_error.set(Some(e.to_string()));
                    save_ok.set(false);
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
            <h1 class="page__title">"Configuration"</h1>

            // ── Status banner ────────────────────────────────────────────────
            <Transition fallback=|| ()>
                {move || {
                    status_res.get().map(|result| {
                        match result {
                            Ok(IrrigationStatus::Active) => view! {
                                <div class="banner banner--active">
                                    "💧 Watering is currently active — saving is disabled."
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

            <Transition fallback=|| view! { <p class="loading">"Loading configuration…"</p> }>
                {move || {
                    schedule_res.get().map(|result| match result {
                        Err(e) => view! {
                            <p class="error">{format!("Failed to load configuration: {e}")}</p>
                        }.into_any(),
                        Ok(_) => {
                            let Some(Ok(setup)) = setup_res.get() else {
                                return view! {
                                    <p class="loading">"Loading zone information…"</p>
                                }.into_any();
                            };

                            view! {
                                <form class="config-form" on:submit=|ev| ev.prevent_default()>

                                    // ── Session times ────────────────────────
                                    <section class="config-section">
                                        <h2 class="config-section__title">"Session Times"</h2>
                                        <div class="field-row">
                                            <label class="field-row__label" for="morning-time">
                                                "Morning start"
                                            </label>
                                            <input
                                                id="morning-time"
                                                type="time"
                                                class="field-row__input"
                                                prop:value=move || morning_time.get()
                                                prop:disabled=move || is_active() || is_saving.get()
                                                on:input=move |ev| {
                                                    morning_time.set(event_target_value(&ev));
                                                    save_ok.set(false);
                                                }
                                            />
                                        </div>
                                        <div class="field-row">
                                            <label class="field-row__label" for="afternoon-time">
                                                "Afternoon start"
                                            </label>
                                            <input
                                                id="afternoon-time"
                                                type="time"
                                                class="field-row__input"
                                                prop:value=move || afternoon_time.get()
                                                prop:disabled=move || is_active() || is_saving.get()
                                                on:input=move |ev| {
                                                    afternoon_time.set(event_target_value(&ev));
                                                    save_ok.set(false);
                                                }
                                            />
                                        </div>
                                    </section>

                                    // ── Zone durations ───────────────────────
                                    <section class="config-section">
                                        <h2 class="config-section__title">"Zone Durations"</h2>
                                        <div class="zone-table">
                                            <div class="zone-table__header">
                                                <span>"Zone"</span>
                                                <span>"Morning"</span>
                                                <span>"Morning (MM:SS)"</span>
                                                <span>"Afternoon"</span>
                                                <span>"Afternoon (MM:SS)"</span>
                                            </div>
                                            {setup.zones.into_iter().map(|zone| {
                                                let zid1 = zone.id.clone();
                                                let zid2 = zone.id.clone();
                                                let zid3 = zone.id.clone();
                                                let zid4 = zone.id.clone();
                                                let zid5 = zone.id.clone();
                                                let zid6 = zone.id.clone();
                                                let zid7 = zone.id.clone();
                                                let zid8 = zone.id.clone();
                                                let name = zone.name.clone();

                                                view! {
                                                    <div class="zone-table__row">
                                                        <span class="zone-table__name">{name}</span>

                                                        // Morning enabled toggle
                                                        <label class="toggle">
                                                            <input
                                                                type="checkbox"
                                                                class="toggle__input"
                                                                prop:checked=move || zone_morning_enabled.get().get(&zid1).copied().unwrap_or(true)
                                                                prop:disabled=move || is_active() || is_saving.get()
                                                                on:change=move |ev| {
                                                                    let v = event_target_checked(&ev);
                                                                    zone_morning_enabled.update(|m| { m.insert(zid2.clone(), v); });
                                                                    save_ok.set(false);
                                                                }
                                                            />
                                                            <span class="toggle__slider"></span>
                                                        </label>

                                                        // Morning duration
                                                        <input
                                                            type="text"
                                                            class="field-row__input field-row__input--duration"
                                                            placeholder="MM:SS"
                                                            prop:value=move || zone_morning.get().get(&zid3).cloned().unwrap_or_default()
                                                            prop:disabled=move || is_active() || is_saving.get()
                                                            on:input=move |ev| {
                                                                let v = event_target_value(&ev);
                                                                zone_morning.update(|m| { m.insert(zid4.clone(), v); });
                                                                save_ok.set(false);
                                                            }
                                                        />

                                                        // Afternoon enabled toggle
                                                        <label class="toggle">
                                                            <input
                                                                type="checkbox"
                                                                class="toggle__input"
                                                                prop:checked=move || zone_afternoon_enabled.get().get(&zid5).copied().unwrap_or(true)
                                                                prop:disabled=move || is_active() || is_saving.get()
                                                                on:change=move |ev| {
                                                                    let v = event_target_checked(&ev);
                                                                    zone_afternoon_enabled.update(|m| { m.insert(zid6.clone(), v); });
                                                                    save_ok.set(false);
                                                                }
                                                            />
                                                            <span class="toggle__slider"></span>
                                                        </label>

                                                        // Afternoon duration
                                                        <input
                                                            type="text"
                                                            class="field-row__input field-row__input--duration"
                                                            placeholder="MM:SS"
                                                            prop:value=move || zone_afternoon.get().get(&zid7).cloned().unwrap_or_default()
                                                            prop:disabled=move || is_active() || is_saving.get()
                                                            on:input=move |ev| {
                                                                let v = event_target_value(&ev);
                                                                zone_afternoon.update(|m| { m.insert(zid8.clone(), v); });
                                                                save_ok.set(false);
                                                            }
                                                        />
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </section>

                                    // ── Save button ──────────────────────────
                                    <div class="form-actions">
                                        <button
                                            type="button"
                                            class="btn btn--primary"
                                            prop:disabled=move || is_active() || is_saving.get()
                                            on:click=move |_| { save_action.dispatch(()); }
                                        >
                                            {move || if is_saving.get() { "Saving…" } else { "Save" }}
                                        </button>
                                        {move || save_error.get().map(|e| view! {
                                            <p class="error">"Error: " {e}</p>
                                        })}
                                        {move || save_ok.get().then(|| view! {
                                            <p class="success">"✓ Configuration saved."</p>
                                        })}
                                    </div>
                                </form>
                            }.into_any()
                        }
                    })
                }}
            </Transition>
        </div>
    }
}

// ── Duration conversion helpers ───────────────────────────────────────────────

/// Convert seconds to `"MM:SS"` display format.
pub fn secs_to_mmss(secs: u32) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{m:02}:{s:02}")
}

/// Parse `"MM:SS"` (or `"HH:MM:SS"`, or plain seconds) back to total seconds.
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
