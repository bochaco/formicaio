mod about;
mod add_node;
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
#[cfg(feature = "ssr")]
pub mod fileserv;
mod helpers;
mod icons;
mod metrics;
#[cfg(feature = "ssr")]
pub mod metrics_client;
mod navbar;
pub mod node_instance;
#[cfg(feature = "ssr")]
pub mod node_rpc_client;
mod nodes_list_view;
mod server_api;
mod settings;
mod stats;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount_to_body(App);
}
