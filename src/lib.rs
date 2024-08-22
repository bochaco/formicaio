pub mod app;
pub mod error_template;
#[cfg(feature = "ssr")]
pub mod fileserv;
mod helpers;
mod node_instance;
#[cfg(feature = "ssr")]
mod portainer_client;
mod server_api;
mod stats;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount_to_body(App);
}
