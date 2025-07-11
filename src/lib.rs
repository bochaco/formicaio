mod about;
mod alerts;
pub mod app;
#[cfg(feature = "ssr")]
pub mod bg_tasks;
mod chart_view;
pub mod cli_cmds;
#[cfg(feature = "ssr")]
pub mod db_client;
#[cfg(all(feature = "ssr", not(feature = "native")))]
pub mod docker_client;
#[cfg(all(feature = "ssr", not(feature = "native")))]
mod docker_msgs;
pub mod error_template;
mod helpers;
mod icons;
#[cfg(feature = "ssr")]
pub mod metrics_client;
mod navbar;
mod node_actions;
#[cfg(feature = "ssr")]
pub mod node_manager;
mod nodes_list_view;
mod pagination;
pub mod server_api;
mod settings;
mod sort_nodes;
mod stats;
mod terminal;
pub mod types;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
