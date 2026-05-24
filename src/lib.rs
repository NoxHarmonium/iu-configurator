#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(warnings)]
#![allow(
    clippy::missing_errors_doc, // app, not a published library
    clippy::implicit_hasher,    // app, not a published library
    clippy::too_many_lines,     // Leptos view fns are declaratively long
)]

pub mod app;
pub mod models;
pub mod pages;
pub mod server_fns;
pub mod utils;

#[cfg(feature = "ssr")]
pub mod handlers;
#[cfg(feature = "ssr")]
pub mod repositories;
#[cfg(feature = "ssr")]
pub mod services;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
