mod about;
mod alerts;
pub mod app;
#[cfg(feature = "ssr")]
pub mod bg_tasks;
mod chart_view;
#[cfg(feature = "ssr")]
pub mod db_client;
#[cfg(all(feature = "ssr", not(feature = "native")))]
pub mod docker_client;
#[cfg(all(feature = "ssr", not(feature = "native")))]
mod docker_msgs;
pub mod error_template;
mod helpers;
mod icons;
#[cfg(all(feature = "ssr", not(feature = "lcd-disabled")))]
mod lcd;
mod metrics;
#[cfg(feature = "ssr")]
pub mod metrics_client;
mod navbar;
mod node_actions;
pub mod node_instance;
#[cfg(all(feature = "ssr", feature = "native"))]
pub mod node_manager;
mod nodes_list_view;
#[cfg(not(feature = "native"))]
mod server_api;
#[cfg(feature = "native")]
mod server_api_native;
pub mod server_api_types;
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
