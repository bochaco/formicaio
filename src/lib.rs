mod add_node;
pub mod app;
pub mod error_template;
#[cfg(feature = "ssr")]
pub mod fileserv;
mod helpers;
mod icons;
#[cfg(feature = "ssr")]
pub mod metadata_db;
mod node_instance;
#[cfg(feature = "ssr")]
mod node_rpc_client;
#[cfg(feature = "ssr")]
pub mod portainer_client;
mod server_api;
mod stats;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount_to_body(App);
}
