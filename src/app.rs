pub use super::metrics::{ContainerId, Metrics, NodeMetric, NodesMetrics};

#[cfg(feature = "hydrate")]
use super::server_api::nodes_instances;
use super::{
    about::AboutView,
    add_node::AddNodeView,
    alerts::AlertMsg,
    error_template::{AppError, ErrorTemplate},
    navbar::NavBar,
    node_instance::{NodeInstanceInfo, NodesListView},
    stats::AggregatedStatsView,
};

#[cfg(feature = "ssr")]
use axum::extract::FromRef;
#[cfg(feature = "ssr")]
use std::sync::Arc;
#[cfg(feature = "ssr")]
use tokio::sync::Mutex;

#[cfg(feature = "hydrate")]
use gloo_timers::future::TimeoutFuture;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use std::collections::HashMap;
use wasm_bindgen::{prelude::*, JsValue};

#[wasm_bindgen(module = "/public/metamask.js")]
extern "C" {
    pub async fn get_addr_from_metamask() -> JsValue;
}

#[cfg(feature = "hydrate")]
const POLLING_FREQ_MILLIS: u32 = 5_500;

#[cfg(feature = "ssr")]
#[derive(Clone, FromRef, Debug)]
pub struct ServerGlobalState {
    pub leptos_options: LeptosOptions,
    pub db_client: super::metadata_db::DbClient,
    pub docker_client: super::docker_client::DockerClient,
    pub latest_bin_version: Arc<Mutex<Option<String>>>,
    pub nodes_metrics: Arc<Mutex<NodesMetrics>>,
}

// Struct to use client side as a global context/state
#[derive(Clone, Copy, Debug)]
pub struct ClientGlobalState {
    // List of nodes instances and their info/state
    pub nodes: RwSignal<HashMap<String, RwSignal<NodeInstanceInfo>>>,
    // Flag to enable/disable nodes' logs stream
    pub logs_stream_is_on: RwSignal<bool>,
    // Lastest version of the node binary available
    pub latest_bin_version: RwSignal<Option<String>>,
    // List of alerts to be shown in the UI
    pub alerts: RwSignal<Vec<(u64, String)>>,
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    // Provide context to manage all client side states that need to be used globally
    provide_context(ClientGlobalState {
        nodes: create_rw_signal(HashMap::default()),
        logs_stream_is_on: create_rw_signal(false),
        latest_bin_version: create_rw_signal(None),
        alerts: create_rw_signal(vec![]),
    });

    // spawn poller task only on client side
    #[cfg(feature = "hydrate")]
    spawn_nodes_list_polling();

    view! {
        <html>
            <Stylesheet id="leptos" href="/pkg/formicaio.css" />
            <Script src="https://cdn.jsdelivr.net/npm/flowbite@2.5.1/dist/flowbite.min.js" />
            // <Script src="/flowbite.min.js" />

            <Title text="Formicaio" />

            <Router fallback=|| {
                let mut outside_errors = Errors::default();
                outside_errors.insert_with_default_key(AppError::NotFound);
                view! { <ErrorTemplate outside_errors /> }.into_view()
            }>
                <NavBar />
                <main>
                    <Routes>
                        <Route path="/" view=HomeScreenView />
                        <Route path="/about" view=AboutView />
                    </Routes>
                </main>
            </Router>
        </html>
    }
}

#[component]
fn HomeScreenView() -> impl IntoView {
    view! {
        <AlertMsg />

        <AggregatedStatsView />
        <AddNodeView />
        <NodesListView />
    }
}

// Spawns a task which polls the server to obtain up to date information of nodes instances.
#[cfg(feature = "hydrate")]
fn spawn_nodes_list_polling() {
    spawn_local(async {
        logging::log!("Polling server every {POLLING_FREQ_MILLIS}ms. ...");
        let context = expect_context::<ClientGlobalState>();
        loop {
            // TODO: poll only when nodes list screen is active
            match nodes_instances().await {
                Err(err) => {
                    logging::log!("Failed to get up to date nodes info from server: {err}")
                }
                Ok(info) => {
                    // if we received info about new binary version then update context
                    if info.latest_bin_version.is_some() {
                        context.latest_bin_version.set(info.latest_bin_version);
                    }

                    // first let's get rid of those removed remotely
                    context.nodes.update(|cx_nodes| {
                        cx_nodes.retain(|id, node_info| {
                            node_info.get_untracked().status.is_creating()
                                || info.nodes.contains_key(id)
                        })
                    });
                    // let's now update those with new values
                    context.nodes.with_untracked(|cx_nodes| {
                        for (id, cn) in cx_nodes {
                            if let Some(updated) = info.nodes.get(id) {
                                if cn.get_untracked() != *updated {
                                    cn.update(|cn| {
                                        if !cn.status.is_transitioning()
                                            || cn.status.is_transitioned()
                                        {
                                            cn.status = updated.status.clone();
                                        }
                                        cn.peer_id = updated.peer_id.clone();
                                        cn.status_info = updated.status_info.clone();
                                        cn.bin_version = updated.bin_version.clone();
                                        cn.balance = updated.balance;
                                        cn.rewards = updated.rewards;
                                        cn.records = updated.records;
                                        cn.relevant_records = updated.relevant_records;
                                        cn.store_cost = updated.store_cost;
                                        cn.mem_used = updated.mem_used;
                                        cn.cpu_usage = updated.cpu_usage.clone();
                                        cn.connected_peers = updated.connected_peers;
                                        cn.kbuckets_peers = updated.kbuckets_peers;
                                    });
                                }
                            }
                        }
                    });
                    // we can add any new node created remotely, perhaps by another instance of the app
                    info.nodes
                        .into_iter()
                        .filter(|(id, _)| !context.nodes.get_untracked().contains_key(id))
                        .for_each(|(id, new_node)| {
                            context.nodes.update(|nodes| {
                                let _ = nodes.insert(id.clone(), create_rw_signal(new_node));
                            })
                        });
                }
            }

            TimeoutFuture::new(POLLING_FREQ_MILLIS).await;
        }
    });
}
