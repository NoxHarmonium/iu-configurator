use leptos::prelude::*;

use super::use_status_polling;
use crate::{
    definitions::ZONES,
    server_fns::{IrrigationStatus, get_irrigation_status, get_schedule, save_schedule},
};

#[component]
pub fn ConfigPage() -> impl IntoView {
    let schedule_res = Resource::new(|| (), |_| get_schedule());
    let status_res = Resource::new(|| (), |_| get_irrigation_status());
    use_status_polling(move || status_res.refetch());

    // Local signals for form fields — populated from schedule_res
    let morning_time = RwSignal::new("07:00".to_string());
    let afternoon_time = RwSignal::new("15:00".to_string());

    // Per-zone signals: (morning_enabled, afternoon_enabled, morning_secs, afternoon_secs)
    // Stored as Vec indexed to match ZONES order.
    let zone_morning_enabled: Vec<RwSignal<bool>> =
        ZONES.iter().map(|_| RwSignal::new(true)).collect();
    let zone_afternoon_enabled: Vec<RwSignal<bool>> =
        ZONES.iter().map(|_| RwSignal::new(true)).collect();
    let zone_morning: Vec<RwSignal<String>> = ZONES
        .iter()
        .map(|_| RwSignal::new("00:00".to_string()))
        .collect();
    let zone_afternoon: Vec<RwSignal<String>> = ZONES
        .iter()
        .map(|_| RwSignal::new("00:00".to_string()))
        .collect();

    // Populate once data arrives
    let zone_morning_enabled_init = zone_morning_enabled.clone();
    let zone_afternoon_enabled_init = zone_afternoon_enabled.clone();
    let zone_morning_init = zone_morning.clone();
    let zone_afternoon_init = zone_afternoon.clone();
    Effect::new(move |_| {
        if let Some(Ok(s)) = schedule_res.get() {
            morning_time.set(s.morning_time.clone());
            afternoon_time.set(s.afternoon_time.clone());
            for (i, zone_def) in ZONES.iter().enumerate() {
                if let Some(zs) = s.zones.get(zone_def.id) {
                    zone_morning_enabled_init[i].set(zs.morning_enabled);
                    zone_afternoon_enabled_init[i].set(zs.afternoon_enabled);
                    zone_morning_init[i].set(secs_to_mmss(zs.morning_secs));
                    zone_afternoon_init[i].set(secs_to_mmss(zs.afternoon_secs));
                }
            }
        }
    });

    // Save action
    let zone_morning_enabled_save = zone_morning_enabled.clone();
    let zone_afternoon_enabled_save = zone_afternoon_enabled.clone();
    let zone_morning_save = zone_morning.clone();
    let zone_afternoon_save = zone_afternoon.clone();

    let save_action = Action::new(move |_: &()| {
        let mt = morning_time.get();
        let at = afternoon_time.get();
        let morning_enabled_snap: Vec<bool> =
            zone_morning_enabled_save.iter().map(|s| s.get()).collect();
        let afternoon_enabled_snap: Vec<bool> = zone_afternoon_enabled_save
            .iter()
            .map(|s| s.get())
            .collect();
        let morning_snap: Vec<String> = zone_morning_save.iter().map(|s| s.get()).collect();
        let afternoon_snap: Vec<String> = zone_afternoon_save.iter().map(|s| s.get()).collect();

        async move {
            match get_schedule().await {
                Ok(mut s) => {
                    s.morning_time = mt;
                    s.afternoon_time = at;
                    for (i, zone_def) in ZONES.iter().enumerate() {
                        if let Some(zs) = s.zones.get_mut(zone_def.id) {
                            zs.morning_enabled = morning_enabled_snap[i];
                            zs.afternoon_enabled = afternoon_enabled_snap[i];
                            zs.morning_secs = mmss_to_secs(&morning_snap[i]);
                            zs.afternoon_secs = mmss_to_secs(&afternoon_snap[i]);
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
                            let zone_morning_enabled_view = zone_morning_enabled.clone();
                            let zone_afternoon_enabled_view = zone_afternoon_enabled.clone();
                            let zone_morning_view = zone_morning.clone();
                            let zone_afternoon_view = zone_afternoon.clone();

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
                                            {ZONES.iter().enumerate().map(|(i, zone_def)| {
                                                let morning_enabled_sig = zone_morning_enabled_view[i];
                                                let afternoon_enabled_sig = zone_afternoon_enabled_view[i];
                                                let morning_sig = zone_morning_view[i];
                                                let afternoon_sig = zone_afternoon_view[i];
                                                let name = zone_def.name;

                                                view! {
                                                    <div class="zone-table__row">
                                                        <span class="zone-table__name">{name}</span>

                                                        // Morning enabled toggle
                                                        <label class="toggle">
                                                            <input
                                                                type="checkbox"
                                                                class="toggle__input"
                                                                prop:checked=move || morning_enabled_sig.get()
                                                                prop:disabled=move || is_active() || is_saving.get()
                                                                on:change=move |ev| {
                                                                    morning_enabled_sig.set(event_target_checked(&ev));
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
                                                            prop:value=move || morning_sig.get()
                                                            prop:disabled=move || is_active() || is_saving.get()
                                                            on:input=move |ev| {
                                                                morning_sig.set(event_target_value(&ev));
                                                                save_ok.set(false);
                                                            }
                                                        />

                                                        // Afternoon enabled toggle
                                                        <label class="toggle">
                                                            <input
                                                                type="checkbox"
                                                                class="toggle__input"
                                                                prop:checked=move || afternoon_enabled_sig.get()
                                                                prop:disabled=move || is_active() || is_saving.get()
                                                                on:change=move |ev| {
                                                                    afternoon_enabled_sig.set(event_target_checked(&ev));
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
                                                            prop:value=move || afternoon_sig.get()
                                                            prop:disabled=move || is_active() || is_saving.get()
                                                            on:input=move |ev| {
                                                                afternoon_sig.set(event_target_value(&ev));
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
