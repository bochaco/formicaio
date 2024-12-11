mod about;
mod alerts;
pub mod app;
#[cfg(feature = "ssr")]
pub mod bg_tasks;
mod chart_view;
#[cfg(feature = "ssr")]
pub mod db_client;
#[cfg(feature = "ssr")]
pub mod docker_client;
#[cfg(feature = "ssr")]
mod docker_msgs;
pub mod error_template;
mod helpers;
mod icons;
#[cfg(feature = "ssr")]
mod lcd;
mod metrics;
#[cfg(feature = "ssr")]
pub mod metrics_client;
mod navbar;
mod node_actions;
pub mod node_instance;
mod nodes_list_view;
mod server_api;
mod settings;
mod sort_nodes;
mod stats;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
