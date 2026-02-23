use crate::types::AppSettings;

#[cfg(feature = "ssr")]
pub use super::app_context::AppContext;

#[cfg(feature = "ssr")]
use super::node_mgr::NodeManager;
#[cfg(feature = "hydrate")]
use super::server_api::{get_new_agent_events, get_settings, nodes_instances};
use super::{
    error_template::{AppError, ErrorTemplate},
    types::{NodeId, NodeInstanceInfo, NodesActionsBatch, NodesSortStrategy, Stats},
    views::{HomeScreenView, Notification, about::AboutView, terminal::TerminalView},
};

#[cfg(feature = "hydrate")]
use super::{
    types::AgentEventType,
    views::{show_error_alert_msg, show_warning_alert_msg},
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
/// Number of days to retain agent events in the DB before pruning.
pub const AGENT_EVENTS_MAX_AGE_DAYS: u32 = 30;
// How often we poll the backend to retrieve an up to date list of node instances.
pub const NODES_LIST_POLLING_FREQ_MILLIS: u64 = 5_500;

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

#[derive(Clone, Debug)]
pub enum ActionTriggered {
    None,
    CreatingNode,
    BatchCreatingNodes,
}

// Struct to use client side as a global context/state
#[derive(Clone, Copy, Debug)]
pub struct ClientGlobalState {
    // Flag which tells the frontend when the connection to the backend is lost.
    pub is_online: RwSignal<bool>,
    // List of nodes instances and their info/state
    pub nodes: RwSignal<(bool, HashMap<NodeId, RwSignal<NodeInstanceInfo>>)>,
    // Indicates whether there is a pending action whose status will be confirmed when synced with backend
    pub is_action_triggered: RwSignal<ActionTriggered>,
    // Layout for the list of nodes
    pub tile_mode: RwSignal<bool>,
    // Node global stats
    pub stats: RwSignal<Stats>,
    // Flag to enable/disable nodes' logs stream
    pub logs_stream_on_for: RwSignal<Option<RwSignal<NodeInstanceInfo>>>,
    // Flag to enable/disable nodes' metrics charts update
    pub metrics_update_on_for: RwSignal<Option<RwSignal<NodeInstanceInfo>>>,
    // Lastest version of the node binary available
    pub latest_bin_version: RwSignal<Option<String>>,
    // List of alerts to be shown in the UI
    pub alerts: RwSignal<Vec<Notification>>,
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
    // Current values of the app settings
    pub app_settings: RwSignal<AppSettings>,
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
        is_action_triggered: RwSignal::new(ActionTriggered::None),
        tile_mode: RwSignal::new(true),
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
        app_settings: RwSignal::new(AppSettings::default()),
    });

    // spawn poller tasks only on client side
    #[cfg(feature = "hydrate")]
    spawn_nodes_list_polling();
    #[cfg(feature = "hydrate")]
    spawn_agent_events_polling();

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
            let app_settings = get_settings().await.unwrap_or_default();
            let node_list_page_size = app_settings.node_list_page_size as usize;

            context.app_settings.update(|s| {
                // update node list mode only if it was changed on the backend
                context.tile_mode.update(|m| {
                    let updated_tile_mode = app_settings.node_list_mode == 0;
                    let context_tile_mode = s.node_list_mode == 0;
                    if updated_tile_mode != context_tile_mode && m != &updated_tile_mode {
                        *m = updated_tile_mode;
                    }
                });

                // update app settings in context only if there were changes
                if s != &app_settings {
                    *s = app_settings;
                }
            });

            let delay_millis = match nodes_instances(None).await {
                Err(err) => {
                    context.is_online.set(false);
                    logging::log!(
                        "[Task] Failed to get updated node information from server: {err}"
                    );
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

                    context.is_action_triggered.set(ActionTriggered::None);

                    // first let's get rid of those removed remotely
                    context.nodes.update(|(loaded, cx_nodes)| {
                        *loaded = true;
                        cx_nodes.retain(|id, _| info.nodes.contains_key(id))
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
                    (count / node_list_page_size) as u64 * 1_000
                }
            };

            let delay =
                std::time::Duration::from_millis(NODES_LIST_POLLING_FREQ_MILLIS + delay_millis);
            logging::log!("[Task] Polling server again in {delay:?} ...");
            sleep(delay).await;
        }
    });
}

// Spawns a task which polls for new autonomous agent events and pushes them to the alert bell.
// The first poll is used only to establish the baseline timestamp so that historical events
// are not replayed as notifications on page load.
#[cfg(feature = "hydrate")]
fn spawn_agent_events_polling() {
    const AGENT_EVENTS_POLL_SECS: u64 = 60;

    spawn_local(async {
        let mut last_ts: i64 = 0;
        let mut is_first_poll = true;
        // Track the last error message shown to suppress repeated identical errors
        // (e.g. Ollama unreachable across multiple consecutive autonomous cycles).
        let mut last_error_shown: Option<String> = None;

        loop {
            sleep(std::time::Duration::from_secs(AGENT_EVENTS_POLL_SECS)).await;

            match get_new_agent_events(last_ts).await {
                Ok(events) => {
                    // Advance the watermark regardless
                    for event in &events {
                        if event.timestamp > last_ts {
                            last_ts = event.timestamp;
                        }
                    }

                    // Skip notifications on the very first poll — it just syncs the watermark
                    if !is_first_poll {
                        for event in events {
                            let msg =
                                agent_event_notification(&event.event_type, &event.description);
                            match &event.event_type {
                                AgentEventType::AnomalyDetected => {
                                    last_error_shown = None; // reset dedup on successful actions
                                    show_warning_alert_msg(msg);
                                }
                                AgentEventType::Error => {
                                    // Suppress consecutive identical errors so a persistent
                                    // backend outage doesn't spam the bell every 15 seconds.
                                    if last_error_shown.as_deref() != Some(&msg) {
                                        last_error_shown = Some(msg.clone());
                                        show_error_alert_msg(msg);
                                    }
                                }
                                AgentEventType::ActionTaken | AgentEventType::Info => {} // skip — informational only
                            }
                        }
                    }

                    is_first_poll = false;
                }
                Err(err) => {
                    logging::log!("[Task] Failed to poll agent events: {err}");
                }
            }
        }
    });
}

// Produce a concise, human-readable notification string for an autonomous agent event.
// ActionTaken descriptions have the format "Called {tool}: {json_result}" which can be
// very long — we map the tool name to a short label and drop the result payload.
// All other descriptions are truncated to one line / max 100 chars.
#[cfg(feature = "hydrate")]
fn agent_event_notification(event_type: &AgentEventType, description: &str) -> String {
    match event_type {
        AgentEventType::ActionTaken => {
            // "Called start_node_instance: {...}" → "[Agent] Restarted a node"
            if let Some(rest) = description.strip_prefix("Called ") {
                let tool = rest.split(':').next().unwrap_or(rest).trim();
                let label = match tool {
                    "start_node_instance" => "Restarted a node",
                    "stop_node_instance" => "Stopped a node",
                    "recycle_node_instance" => "Recycled a node",
                    "delete_node_instance" => "Deleted a node",
                    "create_node_instance" => "Created a new node",
                    "upgrade_node_instance" => "Upgraded a node",
                    other => other,
                };
                return format!("[Agent] {label}");
            }
            truncate_notification("[Agent]", description, 100)
        }
        AgentEventType::Error => {
            // "LLM error during monitoring: <reqwest details>" → "[Agent] Cannot reach LLM"
            // Strip the verbose prefix and show just the first meaningful line.
            let core = description
                .strip_prefix("LLM error during monitoring: ")
                .unwrap_or(description);
            // Connection-refused-style errors get a plain summary
            if core.contains("Connection refused") || core.contains("connection refused") {
                "[Agent] Cannot reach LLM backend (connection refused)".to_string()
            } else {
                truncate_notification("[Agent] Error:", core, 100)
            }
        }
        AgentEventType::AnomalyDetected | AgentEventType::Info => {
            truncate_notification("[Agent]", description, 100)
        }
    }
}

// Formats a notification string with a prefix, capping at max_chars and adding "…" if truncated.
// Only the first line of text is used so multi-line LLM summaries stay compact.
#[cfg(feature = "hydrate")]
fn truncate_notification(prefix: &str, text: &str, max_chars: usize) -> String {
    let text = text.trim();
    let text = text.split('\n').next().unwrap_or(text);
    if text.len() > max_chars {
        format!("{prefix} {}…", &text[..max_chars].trim_end())
    } else {
        format!("{prefix} {text}")
    }
}
