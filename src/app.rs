pub use super::metrics::*;

#[cfg(feature = "hydrate")]
use super::server_api::nodes_instances;
#[cfg(feature = "ssr")]
use super::server_api_types::AppSettings;

use super::{
    about::AboutView,
    alerts::{AlertMsg, OfflineMsg},
    error_template::{AppError, ErrorTemplate},
    navbar::NavBar,
    node_actions::NodesActionsView,
    node_instance::{NodeId, NodeInstanceInfo},
    nodes_list_view::NodesListView,
    pagination::PaginationView,
    server_api_types::{NodesActionsBatch, Stats},
    sort_nodes::NodesSortStrategy,
    stats::AggregatedStatsView,
    terminal::TerminalView,
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
pub const METRICS_MAX_SIZE_PER_NODE: usize = 5_000;
// How often we poll the backend to retrieve an up to date list of node instances.
pub const NODES_LIST_POLLING_FREQ_MILLIS: u64 = 5_500;
// Size of nodes list pages
pub const PAGE_SIZE: usize = 100;

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
    pub latest_bin_version: Arc<Mutex<Option<semver::Version>>>,
    pub server_api_hit: Arc<Mutex<bool>>,
    pub nodes_metrics: Arc<Mutex<super::metrics_client::NodesMetrics>>,
    pub node_status_locked: ImmutableNodeStatus,
    pub bg_tasks_cmds_tx: broadcast::Sender<BgTasksCmds>,
    pub node_action_batches: Arc<
        Mutex<(
            broadcast::Sender<u16>,
            Vec<super::server_api_types::NodesActionsBatch>,
        )>,
    >,
    pub stats: Arc<Mutex<Stats>>,
}

// List of nodes which status is temporarily immutable/locked.
#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Default)]
pub struct ImmutableNodeStatus(Arc<Mutex<HashMap<super::node_instance::NodeId, LockedStatus>>>);

#[cfg(feature = "ssr")]
#[derive(Clone, Debug)]
struct LockedStatus {
    // Timestamp when the status has been locked.
    timestamp: Instant,
    // Expiration information for when it should be unlocked.
    expiration_time: Duration,
    // If this flag is set to 'true' , and the current node status
    // is `Exited` then it has priority over the lock and the
    // status in such case is considered unlocked.
    exited_takes_priority: bool,
}

#[cfg(feature = "ssr")]
impl ImmutableNodeStatus {
    pub async fn lock(&self, node_id: NodeId, expiration_time: Duration) {
        self.0.lock().await.insert(
            node_id,
            LockedStatus {
                timestamp: Instant::now(),
                expiration_time,
                exited_takes_priority: false,
            },
        );
    }

    pub async fn lock_unless_exited(&self, node_id: NodeId, expiration_time: Duration) {
        self.0.lock().await.insert(
            node_id,
            LockedStatus {
                timestamp: Instant::now(),
                expiration_time,
                exited_takes_priority: true,
            },
        );
    }

    pub async fn remove(&self, node_id: &NodeId) {
        self.0.lock().await.remove(node_id);
    }

    // Check if the node id is still in the list, but also check if
    // its expiration has already passed and therefore has to be removed from the list.
    pub async fn is_still_locked(&self, node_info: &NodeInstanceInfo) -> bool {
        let info = self.0.lock().await.get(&node_info.node_id).cloned();
        match info {
            None => false,
            Some(LockedStatus {
                timestamp,
                expiration_time,
                exited_takes_priority,
            }) => {
                if timestamp.elapsed() >= expiration_time {
                    self.remove(&node_info.node_id).await;
                    false
                } else {
                    !(exited_takes_priority && node_info.status.is_exited())
                }
            }
        }
    }
}

// Struct to use client side as a global context/state
#[derive(Clone, Copy, Debug)]
pub struct ClientGlobalState {
    // Flag which tells the frontend when the connection to the backend is lost.
    pub is_online: RwSignal<bool>,
    // List of nodes instances and their info/state
    pub nodes: RwSignal<(bool, HashMap<String, RwSignal<NodeInstanceInfo>>)>,
    // Node global stats
    pub stats: RwSignal<Stats>,
    // Flag to enable/disable nodes' logs stream
    pub logs_stream_on_for: RwSignal<Option<NodeId>>,
    // Flag to enable/disable nodes' metrics charts update
    pub metrics_update_on_for: RwSignal<Option<NodeId>>,
    // Lastest version of the node binary available
    pub latest_bin_version: RwSignal<Option<String>>,
    // List of alerts to be shown in the UI
    pub alerts: RwSignal<Vec<(u64, String)>>,
    // Information about node instances batch currently in progress
    pub scheduled_batches: RwSignal<Vec<RwSignal<NodesActionsBatch>>>,
    // Keep track of nodes being selected and if selection is on/off
    pub selecting_nodes: RwSignal<(bool, HashSet<NodeId>)>,
    // How to sort nodes to display them on the list
    pub nodes_sort_strategy: RwSignal<NodesSortStrategy>,
    // Currently selected page of nodes list (0-based index)
    pub current_page: RwSignal<usize>,
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
        is_online: RwSignal::new(true),
        nodes: RwSignal::new((false, HashMap::default())),
        stats: RwSignal::new(Stats::default()),
        logs_stream_on_for: RwSignal::new(None),
        metrics_update_on_for: RwSignal::new(None),
        latest_bin_version: RwSignal::new(None),
        alerts: RwSignal::new(vec![]),
        scheduled_batches: RwSignal::new(vec![]),
        selecting_nodes: RwSignal::new((false, HashSet::new())),
        nodes_sort_strategy: RwSignal::new(NodesSortStrategy::CreationDate(true)),
        current_page: RwSignal::new(0usize),
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
                    <Route path=StaticSegment("/terminal") view=TerminalView />
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
        <OfflineMsg />
        <NodesActionsView home_net_only />

        <PaginationView />
        <NodesListView />
    }
}

// Spawns a task which polls the server to obtain up to date information of nodes instances.
#[cfg(feature = "hydrate")]
fn spawn_nodes_list_polling() {
    spawn_local(async {
        let context = expect_context::<ClientGlobalState>();
        loop {
            let delay_millis = match nodes_instances(None).await {
                Err(err) => {
                    context.is_online.set(false);
                    logging::log!("Failed to get up to date nodes info from server: {err}");
                    0u64
                }
                Ok(info) => {
                    context.is_online.set(true);

                    // if we received info about new binary version then update context
                    if info.latest_bin_version.is_some() {
                        context.latest_bin_version.set(info.latest_bin_version);
                    }

                    context.stats.update(|s| *s = info.stats);

                    // update info about node action batches in progress
                    context.scheduled_batches.update(|cx_batches| {
                        // first let's get rid of those removed remotely
                        cx_batches.retain(|cx_b| {
                            info.scheduled_batches
                                .iter()
                                .any(|b| b.id == cx_b.read_untracked().id)
                        });
                        // now update and/or add those which are new
                        info.scheduled_batches.into_iter().enumerate().for_each(
                            |(index, batch)| {
                                if let Some(cx_batch) = cx_batches.get(index) {
                                    if cx_batch.read_untracked().id == batch.id {
                                        cx_batch.update(|b| *b = batch);
                                    } else {
                                        cx_batches.insert(index, RwSignal::new(batch));
                                    }
                                } else {
                                    cx_batches.insert(index, RwSignal::new(batch));
                                }
                            },
                        );
                    });

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

                    // make sure our pagination is not overflowing the number of nodes
                    let count = context.nodes.with_untracked(|(_, nodes)| nodes.len());
                    if count > 0 && context.current_page.get_untracked() >= count {
                        context.current_page.update(|c| *c = count - 1);
                    }

                    // the larger the number of nodes, the longer the delay
                    (count / PAGE_SIZE) as u64 * 1_000
                }
            };

            let delay = Duration::from_millis(NODES_LIST_POLLING_FREQ_MILLIS + delay_millis);
            logging::log!("Polling server again in {delay:?} ...");
            sleep(delay).await;
        }
    });
}
