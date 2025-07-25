pub mod app;
#[cfg(feature = "ssr")]
pub mod bg_tasks;
pub mod cli_cmds;
#[cfg(feature = "ssr")]
pub mod db_client;
#[cfg(all(feature = "ssr", not(feature = "native")))]
pub mod docker_client;
#[cfg(all(feature = "ssr", not(feature = "native")))]
mod docker_msgs;
pub mod error_template;
#[cfg(feature = "ssr")]
pub mod metrics_client;
#[cfg(feature = "ssr")]
pub mod node_manager;
pub mod server_api;
pub mod types;
mod views;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
