use leptos::prelude::*;

use super::use_status_polling;
use crate::models::ScheduleMode;
use crate::server_fns::{get_irrigation_status, get_schedule, save_schedule, IrrigationStatus};

const DAYS: &[(&str, &str)] = &[
    ("mon", "Monday"),
    ("tue", "Tuesday"),
    ("wed", "Wednesday"),
    ("thu", "Thursday"),
    ("fri", "Friday"),
    ("sat", "Saturday"),
    ("sun", "Sunday"),
];

#[component]
pub fn SchedulePage() -> impl IntoView {
    // ── Remote data ──────────────────────────────────────────────────────────
    let schedule_res = Resource::new(|| (), |_| get_schedule());
    let status_res = Resource::new(|| (), |_| get_irrigation_status());
    use_status_polling(move || status_res.refetch());

    // ── Local reactive state (populated once schedule loads) ─────────────────
    let morning_days: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let afternoon_days: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let schedule_mode: RwSignal<ScheduleMode> = RwSignal::new(ScheduleMode::Weekday);
    let period_anchor: RwSignal<String> = RwSignal::new(String::new());
    let period_days: RwSignal<u32> = RwSignal::new(2);

    // Populate signals when schedule data arrives
    Effect::new(move |_| {
        if let Some(Ok(s)) = schedule_res.get() {
            morning_days.set(s.morning_days.clone());
            afternoon_days.set(s.afternoon_days.clone());
            schedule_mode.set(s.schedule_mode.clone());
            period_anchor.set(s.period_anchor.clone());
            period_days.set(s.period_days);
        }
    });

    // ── Save action ───────────────────────────────────────────────────────────
    let save_action = Action::new(move |_: &()| {
        let m = morning_days.get();
        let a = afternoon_days.get();
        let mode = schedule_mode.get();
        let anchor = period_anchor.get();
        let days = period_days.get();
        async move {
            // Read the full schedule from server, then patch just the day fields.
            match get_schedule().await {
                Ok(mut s) => {
                    s.morning_days = m;
                    s.afternoon_days = a;
                    s.schedule_mode = mode;
                    s.period_anchor = anchor;
                    s.period_days = days;
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

    // ── Derived validation ─────────────────────────────────────────────────────
    let validation_error = move || -> Option<&'static str> {
        if schedule_mode.get() == ScheduleMode::Periodic && period_anchor.get().trim().is_empty() {
            Some("A start date is required for periodic mode.")
        } else {
            None
        }
    };

    // ── Day toggle helper ─────────────────────────────────────────────────────
    let toggle_day = move |signal: RwSignal<Vec<String>>, day: &'static str, checked: bool| {
        signal.update(|days| {
            if checked {
                if !days.contains(&day.to_string()) {
                    days.push(day.to_string());
                }
            } else {
                days.retain(|d| d != day);
            }
        });
        save_ok.set(false);
    };

    view! {
        <div class="page">
            <h1 class="page__title">"Schedule"</h1>

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

            // ── Day grid ─────────────────────────────────────────────────────
            <Transition fallback=|| view! { <p class="loading">"Loading schedule…"</p> }>
                {move || {
                    schedule_res.get().map(|result| match result {
                        Err(e) => view! {
                            <p class="error">{format!("Failed to load schedule: {e}")}</p>
                        }.into_any(),
                        Ok(_) => {
                            let is_active = move || {
                                status_res.get()
                                    .and_then(|r| r.ok())
                                    .map(|s| s == IrrigationStatus::Active)
                                    .unwrap_or(false)
                            };

                            view! {
                                // ── Mode selector ────────────────────────────
                                <div class="mode-selector">
                                    <label class="mode-selector__label">"Schedule mode"</label>
                                    <select
                                        class="mode-selector__select"
                                        prop:value=move || if schedule_mode.get() == ScheduleMode::Periodic { "periodic" } else { "weekday" }
                                        prop:disabled=move || is_active() || is_saving.get()
                                        on:change=move |ev| {
                                            let value = event_target_value(&ev);
                                            schedule_mode.set(
                                                if value == "periodic" { ScheduleMode::Periodic } else { ScheduleMode::Weekday }
                                            );
                                            save_ok.set(false);
                                        }
                                    >
                                        <option value="weekday">"Weekly (days of week)"</option>
                                        <option value="periodic">"Periodic (every N days)"</option>
                                    </select>
                                </div>

                                // ── Weekday or Periodic content ──────────────
                                {move || match schedule_mode.get() {
                                    ScheduleMode::Weekday => view! {
                                        <div class="day-grid">
                                            <div class="day-grid__header">
                                                <span></span>
                                                <span class="day-grid__session-label">"Morning"</span>
                                                <span class="day-grid__session-label">"Afternoon"</span>
                                            </div>
                                            {DAYS.iter().map(|(key, label)| {
                                                let key = *key;
                                                let label = *label;
                                                view! {
                                                    <div class="day-grid__row">
                                                        <span class="day-grid__day-label">{label}</span>

                                                        // Morning toggle
                                                        <label class="toggle">
                                                            <input
                                                                type="checkbox"
                                                                class="toggle__input"
                                                                prop:checked=move || morning_days.get().contains(&key.to_string())
                                                                prop:disabled=move || is_active() || is_saving.get()
                                                                on:change=move |ev| {
                                                                    let checked = event_target_checked(&ev);
                                                                    toggle_day(morning_days, key, checked);
                                                                }
                                                            />
                                                            <span class="toggle__slider"></span>
                                                        </label>

                                                        // Afternoon toggle
                                                        <label class="toggle">
                                                            <input
                                                                type="checkbox"
                                                                class="toggle__input"
                                                                prop:checked=move || afternoon_days.get().contains(&key.to_string())
                                                                prop:disabled=move || is_active() || is_saving.get()
                                                                on:change=move |ev| {
                                                                    let checked = event_target_checked(&ev);
                                                                    toggle_day(afternoon_days, key, checked);
                                                                }
                                                            />
                                                            <span class="toggle__slider"></span>
                                                        </label>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any(),
                                    ScheduleMode::Periodic => view! {
                                        <div class="periodic-form">
                                            <div class="field-row">
                                                <label class="field-row__label">"Start date (anchor)"</label>
                                                <input
                                                    type="date"
                                                    class="field-row__input"
                                                    prop:value=move || period_anchor.get()
                                                    prop:disabled=move || is_active() || is_saving.get()
                                                    on:change=move |ev| {
                                                        period_anchor.set(event_target_value(&ev));
                                                        save_ok.set(false);
                                                    }
                                                />
                                            </div>
                                            <div class="field-row">
                                                <label class="field-row__label">"Repeat every (days)"</label>
                                                <input
                                                    type="number"
                                                    class="field-row__input"
                                                    min="1"
                                                    prop:value=move || period_days.get().to_string()
                                                    prop:disabled=move || is_active() || is_saving.get()
                                                    on:change=move |ev| {
                                                        let v = event_target_value(&ev)
                                                            .parse::<u32>()
                                                            .unwrap_or(2)
                                                            .max(1);
                                                        period_days.set(v);
                                                        save_ok.set(false);
                                                    }
                                                />
                                            </div>
                                        </div>
                                    }.into_any(),
                                }}

                                // ── Save button ──────────────────────────────
                                <div class="form-actions">
                                    <button
                                        class="btn btn--primary"
                                        prop:disabled=move || is_active() || is_saving.get() || validation_error().is_some()
                                        on:click=move |_| { save_action.dispatch(()); }
                                    >
                                        {move || if is_saving.get() { "Saving…" } else { "Save" }}
                                    </button>
                                    {move || validation_error().map(|msg| view! {
                                        <p class="error">{msg}</p>
                                    })}
                                    {move || save_error.get().map(|e| view! {
                                        <p class="error">"Error: " {e}</p>
                                    })}
                                    {move || save_ok.get().then(|| view! {
                                        <p class="success">"✓ Schedule saved."</p>
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
