use leptos::prelude::*;

use super::use_status_polling;
use crate::{
    models::PeriodicSchedule,
    server_fns::{get_irrigation_status, get_schedule, save_schedule, IrrigationStatus},
};

const DAYS: &[(&str, &str)] = &[
    ("mon", "Monday"),
    ("tue", "Tuesday"),
    ("wed", "Wednesday"),
    ("thu", "Thursday"),
    ("fri", "Friday"),
    ("sat", "Saturday"),
    ("sun", "Sunday"),
];

/// Returns `true` if `s` is a well-formed `YYYY-MM-DD` date string.
/// (The HTML date input always produces this format when non-empty.)
fn is_valid_date(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let bytes = s.as_bytes();
    bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..].iter().all(u8::is_ascii_digit)
}

/// Which method is being used to specify the watering schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScheduleMode {
    WeekDays,
    Periodic,
}

#[component]
pub fn SchedulePage() -> impl IntoView {
    // ── Remote data ──────────────────────────────────────────────────────────
    let schedule_res = Resource::new(|| (), |_| get_schedule());
    let status_res = Resource::new(|| (), |_| get_irrigation_status());
    use_status_polling(move || status_res.refetch());

    // ── Local reactive state (populated once schedule loads) ─────────────────
    let morning_days: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let afternoon_days: RwSignal<Vec<String>> = RwSignal::new(vec![]);

    // Schedule mode — derived from loaded schedule, then user-controlled.
    let schedule_mode: RwSignal<ScheduleMode> = RwSignal::new(ScheduleMode::WeekDays);

    // Periodic schedule inputs: an absolute start date (YYYY-MM-DD) and repeat interval.
    let morning_start_date: RwSignal<String> = RwSignal::new("".into());
    let morning_repeat_days: RwSignal<String> = RwSignal::new("3".into());
    let afternoon_start_date: RwSignal<String> = RwSignal::new("".into());
    let afternoon_repeat_days: RwSignal<String> = RwSignal::new("3".into());

    // Populate signals when schedule data arrives
    Effect::new(move |_| {
        if let Some(Ok(s)) = schedule_res.get() {
            morning_days.set(s.morning_days.clone());
            afternoon_days.set(s.afternoon_days.clone());

            // Determine mode from whichever periodic schedule exists.
            // Both sessions share a single mode toggle; if either is periodic,
            // switch to Periodic and populate whichever inputs are available.
            // If only one session has a periodic value, mirror it to the other.
            match (&s.morning_periodic, &s.afternoon_periodic) {
                (Some(m), Some(a)) => {
                    schedule_mode.set(ScheduleMode::Periodic);
                    morning_start_date.set(m.start_date.clone());
                    morning_repeat_days.set(m.repeat_days.to_string());
                    afternoon_start_date.set(a.start_date.clone());
                    afternoon_repeat_days.set(a.repeat_days.to_string());
                }
                (Some(m), None) => {
                    schedule_mode.set(ScheduleMode::Periodic);
                    morning_start_date.set(m.start_date.clone());
                    morning_repeat_days.set(m.repeat_days.to_string());
                    // Mirror morning values to afternoon as a sensible default.
                    afternoon_start_date.set(m.start_date.clone());
                    afternoon_repeat_days.set(m.repeat_days.to_string());
                }
                (None, Some(a)) => {
                    schedule_mode.set(ScheduleMode::Periodic);
                    afternoon_start_date.set(a.start_date.clone());
                    afternoon_repeat_days.set(a.repeat_days.to_string());
                    // Mirror afternoon values to morning as a sensible default.
                    morning_start_date.set(a.start_date.clone());
                    morning_repeat_days.set(a.repeat_days.to_string());
                }
                (None, None) => {}
            }
        }
    });

    // ── Validation ────────────────────────────────────────────────────────────
    // Returns `Some(error_message)` if the periodic inputs are invalid.
    let validate_periodic = move || -> Option<String> {
        let m_date = morning_start_date.get();
        let m_repeat = morning_repeat_days.get();
        let a_date = afternoon_start_date.get();
        let a_repeat = afternoon_repeat_days.get();

        if m_date.trim().is_empty() {
            return Some("Morning start date must be set.".into());
        }
        if !is_valid_date(m_date.trim()) {
            return Some("Morning start date must be a valid date (YYYY-MM-DD).".into());
        }
        match m_repeat.parse::<u32>() {
            Ok(0) => return Some("Morning repeat interval must be at least 1 day.".into()),
            Err(_) => {
                return Some("Morning repeat interval must be a positive whole number.".into())
            }
            Ok(_) => {}
        }
        if a_date.trim().is_empty() {
            return Some("Afternoon start date must be set.".into());
        }
        if !is_valid_date(a_date.trim()) {
            return Some("Afternoon start date must be a valid date (YYYY-MM-DD).".into());
        }
        match a_repeat.parse::<u32>() {
            Ok(0) => return Some("Afternoon repeat interval must be at least 1 day.".into()),
            Err(_) => {
                return Some("Afternoon repeat interval must be a positive whole number.".into())
            }
            Ok(_) => {}
        }
        None
    };

    // ── Save action ───────────────────────────────────────────────────────────
    let save_action = Action::new(move |_: &()| {
        let mode = schedule_mode.get();
        let m = morning_days.get();
        let a = afternoon_days.get();
        let m_date = morning_start_date.get();
        let m_repeat = morning_repeat_days.get();
        let a_date = afternoon_start_date.get();
        let a_repeat = afternoon_repeat_days.get();
        async move {
            // Read the full schedule from server, then patch just the day fields.
            match get_schedule().await {
                Ok(mut s) => {
                    match mode {
                        ScheduleMode::WeekDays => {
                            s.morning_days = m;
                            s.afternoon_days = a;
                            s.morning_periodic = None;
                            s.afternoon_periodic = None;
                        }
                        ScheduleMode::Periodic => {
                            let m_start = m_date.trim().to_string();
                            if m_start.is_empty() {
                                return Err(ServerFnError::new("Morning start date must be set."));
                            }
                            if !is_valid_date(&m_start) {
                                return Err(ServerFnError::new(
                                    "Morning start date must be a valid date (YYYY-MM-DD).",
                                ));
                            }
                            let m_repeat_val: u32 = m_repeat.parse().map_err(|_| {
                                ServerFnError::new(
                                    "Morning repeat interval must be a positive whole number.",
                                )
                            })?;
                            if m_repeat_val == 0 {
                                return Err(ServerFnError::new(
                                    "Morning repeat interval must be at least 1 day.",
                                ));
                            }
                            let a_start = a_date.trim().to_string();
                            if a_start.is_empty() {
                                return Err(ServerFnError::new(
                                    "Afternoon start date must be set.",
                                ));
                            }
                            if !is_valid_date(&a_start) {
                                return Err(ServerFnError::new(
                                    "Afternoon start date must be a valid date (YYYY-MM-DD).",
                                ));
                            }
                            let a_repeat_val: u32 = a_repeat.parse().map_err(|_| {
                                ServerFnError::new(
                                    "Afternoon repeat interval must be a positive whole number.",
                                )
                            })?;
                            if a_repeat_val == 0 {
                                return Err(ServerFnError::new(
                                    "Afternoon repeat interval must be at least 1 day.",
                                ));
                            }
                            s.morning_days = vec![];
                            s.afternoon_days = vec![];
                            s.morning_periodic = Some(PeriodicSchedule {
                                start_date: m_start,
                                repeat_days: m_repeat_val,
                            });
                            s.afternoon_periodic = Some(PeriodicSchedule {
                                start_date: a_start,
                                repeat_days: a_repeat_val,
                            });
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

            // ── Schedule content ──────────────────────────────────────────────
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
                                <div class="field-row schedule-mode">
                                    <label class="field-row__label" for="schedule-mode-select">
                                        "Schedule type"
                                    </label>
                                    <select
                                        id="schedule-mode-select"
                                        class="field-row__input"
                                        prop:disabled=move || is_active() || is_saving.get()
                                        on:change=move |ev| {
                                            let val = event_target_value(&ev);
                                            let new_mode = if val == "periodic" {
                                                ScheduleMode::Periodic
                                            } else {
                                                ScheduleMode::WeekDays
                                            };
                                            schedule_mode.set(new_mode);
                                            save_ok.set(false);
                                        }
                                    >
                                        <option
                                            value="weekdays"
                                            prop:selected=move || schedule_mode.get() == ScheduleMode::WeekDays
                                        >
                                            "Days of week"
                                        </option>
                                        <option
                                            value="periodic"
                                            prop:selected=move || schedule_mode.get() == ScheduleMode::Periodic
                                        >
                                            "Every N days"
                                        </option>
                                    </select>
                                </div>

                                // ── Days-of-week grid ────────────────────────
                                {move || (schedule_mode.get() == ScheduleMode::WeekDays).then(|| view! {
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
                                })}

                                // ── Periodic schedule inputs ─────────────────
                                {move || (schedule_mode.get() == ScheduleMode::Periodic).then(|| view! {
                                    <div class="periodic-form">
                                        <div class="config-section">
                                            <h3 class="config-section__title">"Morning"</h3>
                                            <div class="field-row">
                                                <label class="field-row__label">"Start date"</label>
                                                <input
                                                    type="date"
                                                    class="field-row__input field-row__input--date"
                                                    prop:value=move || morning_start_date.get()
                                                    prop:disabled=move || is_active() || is_saving.get()
                                                    on:input=move |ev| {
                                                        morning_start_date.set(event_target_value(&ev));
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
                                                    prop:value=move || morning_repeat_days.get()
                                                    prop:disabled=move || is_active() || is_saving.get()
                                                    on:input=move |ev| {
                                                        morning_repeat_days.set(event_target_value(&ev));
                                                        save_ok.set(false);
                                                    }
                                                />
                                            </div>
                                            {move || {
                                                let date = morning_start_date.get();
                                                let repeat = morning_repeat_days.get();
                                                let repeat_days = repeat.parse::<u32>().unwrap_or(0);
                                                (!date.is_empty() && repeat_days > 0).then(|| view! {
                                                    <p class="hint">
                                                        "Starting " {date} ", then every " {repeat_days.to_string()} " days."
                                                    </p>
                                                })
                                            }}
                                        </div>

                                        <div class="config-section">
                                            <h3 class="config-section__title">"Afternoon"</h3>
                                            <div class="field-row">
                                                <label class="field-row__label">"Start date"</label>
                                                <input
                                                    type="date"
                                                    class="field-row__input field-row__input--date"
                                                    prop:value=move || afternoon_start_date.get()
                                                    prop:disabled=move || is_active() || is_saving.get()
                                                    on:input=move |ev| {
                                                        afternoon_start_date.set(event_target_value(&ev));
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
                                                    prop:value=move || afternoon_repeat_days.get()
                                                    prop:disabled=move || is_active() || is_saving.get()
                                                    on:input=move |ev| {
                                                        afternoon_repeat_days.set(event_target_value(&ev));
                                                        save_ok.set(false);
                                                    }
                                                />
                                            </div>
                                            {move || {
                                                let date = afternoon_start_date.get();
                                                let repeat = afternoon_repeat_days.get();
                                                let repeat_days = repeat.parse::<u32>().unwrap_or(0);
                                                (!date.is_empty() && repeat_days > 0).then(|| view! {
                                                    <p class="hint">
                                                        "Starting " {date} ", then every " {repeat_days.to_string()} " days."
                                                    </p>
                                                })
                                            }}
                                        </div>
                                    </div>
                                })}

                                // ── Save button ──────────────────────────────
                                <div class="form-actions">
                                    <button
                                        class="btn btn--primary"
                                        prop:disabled=move || {
                                            is_active() || is_saving.get()
                                                || (schedule_mode.get() == ScheduleMode::Periodic
                                                    && validate_periodic().is_some())
                                        }
                                        on:click=move |_| { save_action.dispatch(()); }
                                    >
                                        {move || if is_saving.get() { "Saving…" } else { "Save" }}
                                    </button>
                                    {move || {
                                        if schedule_mode.get() == ScheduleMode::Periodic {
                                            validate_periodic().map(|e| view! {
                                                <p class="error">{e}</p>
                                            })
                                        } else {
                                            None
                                        }
                                    }}
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
