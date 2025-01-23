pub use super::metrics::*;

#[cfg(all(feature = "hydrate", not(feature = "native")))]
use super::server_api::nodes_instances;
#[cfg(all(feature = "hydrate", feature = "native"))]
use super::server_api_native::nodes_instances;
#[cfg(feature = "ssr")]
use super::server_api_types::AppSettings;

use super::{
    about::AboutView,
    alerts::AlertMsg,
    error_template::{AppError, ErrorTemplate},
    navbar::NavBar,
    node_actions::NodesActionsView,
    node_instance::{ContainerId, NodeInstanceInfo},
    nodes_list_view::NodesListView,
    server_api_types::{BatchInProgress, Stats},
    sort_nodes::{NodesSortStrategy, SortStrategyView},
    stats::AggregatedStatsView,
};

#[cfg(feature = "ssr")]
use axum::extract::FromRef;
#[cfg(feature = "hydrate")]
use leptos::{logging, task::spawn_local};
#[cfg(feature = "ssr")]
use std::sync::Arc;
#[cfg(feature = "ssr")]
use tokio::{
    sync::{broadcast, Mutex},
    time::Instant,
};

#[cfg(feature = "hydrate")]
use gloo_timers::future::sleep;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Script, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use wasm_bindgen::{prelude::*, JsValue};

#[wasm_bindgen(module = "/public/metamask.js")]
extern "C" {
    pub async fn get_addr_from_metamask() -> JsValue;
}

// Maximum number of metrics data points to be kept per node on DB cache.
pub const METRICS_MAX_SIZE_PER_CONTAINER: usize = 5_000;
// How often we poll the backedn to retrieve an up to date list of node instances.
pub const NODES_LIST_POLLING_FREQ_MILLIS: u64 = 5_500;

// Env var to restrict the nodes to be run only with home-network mode on,
// i.e. the home-network cannot be disabled for any node instantiated in current deployment.
const HOME_NETWORK_ONLY: &str = "HOME_NETWORK_ONLY";

// Type of actions that can be requested to the bg jobs.
#[cfg(feature = "ssr")]
#[derive(Clone, Debug)]
pub enum BgTasksCmds {
    ApplySettings(AppSettings),
    CheckBalanceFor(NodeInstanceInfo),
    DeleteBalanceFor(NodeInstanceInfo),
    CheckAllBalances,
}

#[cfg(feature = "ssr")]
#[derive(Clone, FromRef, Debug)]
pub struct ServerGlobalState {
    pub leptos_options: LeptosOptions,
    pub db_client: super::db_client::DbClient,
    #[cfg(not(feature = "native"))]
    pub docker_client: super::docker_client::DockerClient,
    #[cfg(feature = "native")]
    pub node_manager: super::node_manager::NodeManager,
    pub latest_bin_version: Arc<Mutex<Option<String>>>,
    pub server_api_hit: Arc<Mutex<bool>>,
    pub nodes_metrics: Arc<Mutex<super::metrics_client::NodesMetrics>>,
    pub node_status_locked: ImmutableNodeStatus,
    pub bg_tasks_cmds_tx: broadcast::Sender<BgTasksCmds>,
    pub node_instaces_batches: Arc<
        Mutex<(
            broadcast::Sender<()>,
            Vec<super::node_instance::NodeInstancesBatch>,
        )>,
    >,
    pub stats: Arc<Mutex<Stats>>,
}

// List of nodes which status is temporarily immutable/locked,
// along with expiration information for when it should be unlocked.
#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Default)]
pub struct ImmutableNodeStatus(
    Arc<Mutex<HashMap<super::node_instance::ContainerId, (Instant, Duration)>>>,
);

#[cfg(feature = "ssr")]
impl ImmutableNodeStatus {
    pub async fn insert(&self, container_id: ContainerId, expiration: Duration) {
        self.0
            .lock()
            .await
            .insert(container_id, (Instant::now(), expiration));
    }

    pub async fn remove(&self, container_id: &ContainerId) {
        self.0.lock().await.remove(container_id);
    }

    // Check if the container id is still in the list, but also check if
    // its expiration has already passed and therefore has to be removed from the list.
    pub async fn is_still_locked(&self, container_id: &ContainerId) -> bool {
        let info = self.0.lock().await.get(container_id).cloned();
        match info {
            None => false,
            Some((instant, expiration)) => {
                if instant.elapsed() >= expiration {
                    self.remove(container_id).await;
                    false
                } else {
                    true
                }
            }
        }
    }
}

// Struct to use client side as a global context/state
#[derive(Clone, Copy, Debug)]
pub struct ClientGlobalState {
    // List of nodes instances and their info/state
    pub nodes: RwSignal<(bool, HashMap<String, RwSignal<NodeInstanceInfo>>)>,
    // Node global stats
    pub stats: RwSignal<Stats>,
    // Flag to enable/disable nodes' logs stream
    pub logs_stream_on_for: RwSignal<Option<ContainerId>>,
    // Flag to enable/disable nodes' metrics charts update
    pub metrics_update_on_for: RwSignal<Option<ContainerId>>,
    // Lastest version of the node binary available
    pub latest_bin_version: RwSignal<Option<String>>,
    // List of alerts to be shown in the UI
    pub alerts: RwSignal<Vec<(u64, String)>>,
    // Information about node instances batch currently in progress
    pub batch_in_progress: RwSignal<Option<BatchInProgress>>,
    // Keep track of nodes being selected and if selection is on/off
    pub selecting_nodes: RwSignal<(bool, bool, HashSet<ContainerId>)>,
    // How to sort nodes to display them on the list
    pub nodes_sort_strategy: RwSignal<NodesSortStrategy>,
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>

            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    // Provide context to manage all client side states that need to be used globally
    provide_context(ClientGlobalState {
        nodes: RwSignal::new((false, HashMap::default())),
        stats: RwSignal::new(Stats::default()),
        logs_stream_on_for: RwSignal::new(None),
        metrics_update_on_for: RwSignal::new(None),
        latest_bin_version: RwSignal::new(None),
        alerts: RwSignal::new(vec![]),
        batch_in_progress: RwSignal::new(None),
        selecting_nodes: RwSignal::new((false, false, HashSet::new())),
        nodes_sort_strategy: RwSignal::new(NodesSortStrategy::CreationDate(true)),
    });

    // spawn poller task only on client side
    #[cfg(feature = "hydrate")]
    spawn_nodes_list_polling();

    view! {
        <Router>
            <main>
                <Stylesheet id="leptos" href="/pkg/formicaio.css" />
                <Script src="https://cdn.jsdelivr.net/npm/flowbite@2.5.1/dist/flowbite.min.js" />

                <Title text="Formicaio" />

                <NavBar />
                <Routes fallback=|| {
                    let mut outside_errors = Errors::default();
                    outside_errors.insert_with_default_key(AppError::NotFound);
                    view! { <ErrorTemplate outside_errors /> }.into_view()
                }>
                    <Route path=StaticSegment("/") view=HomeScreenView />
                    <Route path=StaticSegment("/about") view=AboutView />
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomeScreenView() -> impl IntoView {
    let home_net_only = std::env::var(HOME_NETWORK_ONLY)
        .map(|v| v.parse().unwrap_or_default())
        .unwrap_or_default();

    if home_net_only {
        leptos::logging::log!("'{HOME_NETWORK_ONLY}' env var set to 'true', thus home-network mode cannot be disabled in this deployment.");
    }

    view! {
        <AlertMsg />

        <AggregatedStatsView />
        <NodesActionsView home_net_only />

        <SortStrategyView />
        <NodesListView />
    }
}

// Spawns a task which polls the server to obtain up to date information of nodes instances.
#[cfg(feature = "hydrate")]
fn spawn_nodes_list_polling() {
    spawn_local(async {
        logging::log!("Polling server every {NODES_LIST_POLLING_FREQ_MILLIS}ms. ...");
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

                    context.stats.update(|s| *s = info.stats);

                    // update info about node instances batch in progress
                    context
                        .batch_in_progress
                        .update(|b| *b = info.batch_in_progress);

                    // first let's get rid of those removed remotely
                    context.nodes.update(|(loaded, cx_nodes)| {
                        *loaded = true;
                        cx_nodes.retain(|id, node_info| {
                            node_info.read_untracked().status.is_creating()
                                || info.nodes.contains_key(id)
                        })
                    });
                    // let's now update those with new values
                    context.nodes.with_untracked(|(_, cx_nodes)| {
                        for (id, cn) in cx_nodes {
                            if let Some(updated) = info.nodes.get(id) {
                                if cn.read_untracked() != *updated {
                                    cn.update(|cn| *cn = updated.clone());
                                }
                            }
                        }
                    });
                    // we can add any new node created remotely, perhaps by another instance of the app
                    info.nodes
                        .into_iter()
                        .filter(|(id, _)| !context.nodes.read_untracked().1.contains_key(id))
                        .for_each(|(id, new_node)| {
                            context.nodes.update(|(_, nodes)| {
                                let _ = nodes.insert(id.clone(), RwSignal::new(new_node));
                            })
                        });
                }
            }

            sleep(Duration::from_millis(NODES_LIST_POLLING_FREQ_MILLIS)).await;
        }
    });
}
