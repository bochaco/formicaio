#[cfg(feature = "ssr")]
pub use super::app_context::AppContext;

#[cfg(feature = "ssr")]
use super::node_mgr::NodeManager;
#[cfg(feature = "hydrate")]
use super::server_api::nodes_instances;
use super::{
    error_template::{AppError, ErrorTemplate},
    types::{NodeId, NodeInstanceInfo, NodesActionsBatch, NodesSortStrategy, Stats},
    views::{HomeScreenView, about::AboutView, terminal::TerminalView},
};

#[cfg(feature = "ssr")]
use axum::extract::FromRef;
#[cfg(feature = "hydrate")]
use leptos::{logging, task::spawn_local};

#[cfg(feature = "hydrate")]
use gloo_timers::future::sleep;
use leptos::prelude::*;
use leptos_meta::{MetaTags, Script, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{Route, Router, Routes},
};
use std::collections::{HashMap, HashSet};
use wasm_bindgen::{JsValue, prelude::*};

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

/// Global server-side state shared across the application, available only when running with SSR (server-side rendering).
#[cfg(feature = "ssr")]
#[derive(Clone, FromRef, Debug)]
pub struct ServerGlobalState {
    /// Leptos framework options and configuration.
    pub leptos_options: LeptosOptions,
    /// Node manager responsible for managing node instances (either native or on Docker).
    pub node_manager: NodeManager,
    /// Main application context holding shared state and resources.
    pub app_ctx: AppContext,
}

// Struct to use client side as a global context/state
#[derive(Clone, Copy, Debug)]
pub struct ClientGlobalState {
    // Flag which tells the frontend when the connection to the backend is lost.
    pub is_online: RwSignal<bool>,
    // List of nodes instances and their info/state
    pub nodes: RwSignal<(bool, HashMap<NodeId, RwSignal<NodeInstanceInfo>>)>,
    // Node global stats
    pub stats: RwSignal<Stats>,
    // Flag to enable/disable nodes' logs stream
    pub logs_stream_on_for: RwSignal<Option<RwSignal<NodeInstanceInfo>>>,
    // Flag to enable/disable nodes' metrics charts update
    pub metrics_update_on_for: RwSignal<Option<RwSignal<NodeInstanceInfo>>>,
    // Lastest version of the node binary available
    pub latest_bin_version: RwSignal<Option<String>>,
    // List of alerts to be shown in the UI
    pub alerts: RwSignal<Vec<(u64, String)>>,
    // Information about node instances batch currently in progress
    pub scheduled_batches: RwSignal<Vec<RwSignal<NodesActionsBatch>>>,
    // Keep track of nodes being selected and if selection is on/off
    pub selecting_nodes: RwSignal<(bool, HashSet<NodeId>)>,
    // Keep track of nodes info being expanded and collapsed
    pub expanded_nodes: RwSignal<HashSet<NodeId>>,
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
        expanded_nodes: RwSignal::new(HashSet::new()),
        nodes_sort_strategy: RwSignal::new(NodesSortStrategy::default()),
        current_page: RwSignal::new(0usize),
    });

    // spawn poller task only on client side
    #[cfg(feature = "hydrate")]
    spawn_nodes_list_polling();

    view! {
        <Router>
            <main>
                <Stylesheet id="leptos" href="/pkg/formicaio.css" />
                <Script src="https://cdn.jsdelivr.net/npm/flowbite@3.1.2/dist/flowbite.min.js" />
                <link
                    href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=Fira+Code:wght@400;500&display=swap"
                    rel="stylesheet"
                />

                <Title text="Formicaio" />

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

// Spawns a task which polls the server to obtain up to date information of nodes instances.
#[cfg(feature = "hydrate")]
fn spawn_nodes_list_polling() {
    spawn_local(async {
        let context = expect_context::<ClientGlobalState>();
        loop {
            let delay_millis = match nodes_instances(None).await {
                Err(err) => {
                    context.is_online.set(false);
                    logging::log!("Failed to get updated node information from server: {err}");
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
                            if let Some(updated) = info.nodes.get(id)
                                && cn.read_untracked() != *updated
                            {
                                cn.update(|cn| *cn = updated.clone());
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

            let delay =
                std::time::Duration::from_millis(NODES_LIST_POLLING_FREQ_MILLIS + delay_millis);
            logging::log!("Polling server again in {delay:?} ...");
            sleep(delay).await;
        }
    });
}
