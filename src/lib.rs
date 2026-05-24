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
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
