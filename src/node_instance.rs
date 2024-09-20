use super::{
    app::ClientGlobalState,
    helpers::{node_logs_stream, remove_node_instance},
    icons::{IconRemoveNode, IconShowLogs, IconStartNode, IconStopNode, IconUpgradeNode},
    server_api::{start_node_instance, stop_node_instance, upgrade_node_instance},
};

use chrono::{DateTime, Utc};
use leptos::*;
use serde::{Deserialize, Serialize};
use std::fmt;

// Length of nodes PeerIds' prefix and suffix to be displayed
const PEER_ID_PREFIX_SUFFIX_LEN: usize = 12;
// Length of nodes Docker container ids' prefix to be displayed
const CONTAINER_ID_PREFIX_LEN: usize = 12;

#[derive(Clone, Default, Debug, Deserialize, PartialEq, Serialize)]
pub enum NodeStatus {
    #[default]
    Creating,
    // A running node connected to peers on the network is considered Active.
    Active,
    Restarting,
    Stopping,
    // A node not connected to any peer on the network is considered Inactive.
    Inactive,
    // When a node is running and connected to peers on the network but it's
    // being considered a bad node by them, then this node is considered Shunned.
    Shunned, // TODO: include suspected reason as to why others shunned it ...??
    Removing,
    Upgrading,
    // This is a special state just to provide a good UX, after going thru some status
    // change, e.g. Restarting, Upgrading, we set to this state till we get actual state
    // from the server during our polling cycle. The string describes the type of transition.
    Transitioned(String),
}

impl NodeStatus {
    pub fn is_creating(&self) -> bool {
        matches!(self, Self::Creating)
    }
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
    pub fn is_inactive(&self) -> bool {
        matches!(self, Self::Inactive)
    }
    pub fn is_shunned(&self) -> bool {
        matches!(self, Self::Shunned)
    }
    pub fn is_upgrading(&self) -> bool {
        matches!(self, Self::Upgrading)
    }
    pub fn is_transitioning(&self) -> bool {
        match self {
            Self::Creating
            | Self::Restarting
            | Self::Stopping
            | Self::Removing
            | Self::Upgrading
            | Self::Transitioned(_) => true,
            _ => false,
        }
    }
    pub fn is_transitioned(&self) -> bool {
        matches!(self, Self::Transitioned(_))
    }
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Transitioned(s) => write!(f, "{s}"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[derive(Clone, Default, Debug, Deserialize, PartialEq, Serialize)]
pub struct NodeInstanceInfo {
    pub container_id: String,
    pub created: u64,
    pub peer_id: Option<String>, // base58-encoded Peer Id bytes
    pub status: NodeStatus,
    pub status_info: String,
    pub bin_version: Option<String>,
    pub port: Option<u16>,
    pub rpc_api_port: Option<u16>,
    pub rpc_api_ip: Option<String>,
    pub rewards: Option<u64>,
    pub balance: Option<u64>,           // nanos
    pub forwarded_balance: Option<u64>, // nanos, only during beta
    pub records: Option<usize>,
    pub connected_peers: Option<usize>,
    pub kbuckets_peers: Option<usize>,
}

impl NodeInstanceInfo {
    pub fn upgrade_available(&self) -> bool {
        let context = expect_context::<ClientGlobalState>();
        context.latest_bin_version.get().is_some()
            && self.bin_version.is_some()
            && context.latest_bin_version.get() != self.bin_version
    }

    pub fn upgradeable(&self) -> bool {
        self.status.is_active() && self.upgrade_available()
    }
}

#[component]
pub fn NodesListView() -> impl IntoView {
    // we use the context to switch on/off the streaming of logs
    let context = expect_context::<ClientGlobalState>();
    // this signal keeps the reactive list of log entries
    let (logs, set_logs) = create_signal(Vec::new());

    view! {
        <div class="flex flex-wrap">
            <For
                each=move || context.nodes.get()
                key=|(container_id, _)| container_id.clone()
                let:child
            >
                <Show
                    when=move || !child.1.get().status.is_creating()
                    fallback=move || { view! { <CreatingNodeInstanceView /> }.into_view() }
                >
                    <NodeInstanceView info=child.1 set_logs />
                </Show>

            </For>
        </div>

        <input type="checkbox" id="logs_stream_modal" class="modal-toggle" />
        <div class="modal" role="dialog">
            <div class="modal-box border border-solid border-slate-50 max-w-full h-full overflow-hidden">
                <h3 class="text-lg font-bold">Node logs</h3>
                <div class="p-2.5 border-transparent overflow-y-auto h-full">
                    <ul>
                        <For
                            each=move || logs.get().into_iter().enumerate()
                            key=|(i, _)| *i
                            let:child
                        >
                            <li>{child.1}</li>
                        </For>
                    </ul>
                </div>

                <div class="modal-action">
                    <label
                        for="logs_stream_modal"
                        class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2"
                        on:click=move |_| context.logs_stream_is_on.set(false)
                    >
                        X
                    </label>
                </div>
            </div>
        </div>
    }
}

#[component]
fn CreatingNodeInstanceView() -> impl IntoView {
    view! {
        <div class="w-1/4 m-2 p-4 overflow-x-auto card card-normal bg-neutral text-neutral-content card-bordered shadow-2xl">
            <div class="flex flex-col gap-4">
                <div class="skeleton h-16 w-full"></div>
                <div class="skeleton h-4 w-28"></div>
                <div class="skeleton h-4 w-56"></div>
                <div class="skeleton h-4 w-28"></div>
                <div class="skeleton h-4 w-20"></div>
                <div class="skeleton h-4 w-28"></div>
                <div class="skeleton h-4 w-40"></div>
                <div class="skeleton h-4 w-40"></div>
                <div class="skeleton h-4 w-56"></div>
            </div>
        </div>
    }
}

#[component]
fn NodeInstanceView(
    info: RwSignal<NodeInstanceInfo>,
    set_logs: WriteSignal<Vec<String>>,
) -> impl IntoView {
    let peer_id = move || {
        info.get().peer_id.map_or("unknown".to_string(), |id| {
            format!(
                "{}...{}",
                &id[..PEER_ID_PREFIX_SUFFIX_LEN],
                &id[id.len() - PEER_ID_PREFIX_SUFFIX_LEN..]
            )
        })
    };
    let container_id = info.get_untracked().container_id[..CONTAINER_ID_PREFIX_LEN].to_string();

    let spinner_msg = move || {
        let status = info.get().status;
        if status.is_transitioning() {
            format!("{status}")
        } else {
            "".to_string()
        }
    };

    view! {
        <div class="w-1/4 m-2 p-4 overflow-x-auto card card-normal bg-neutral text-neutral-content card-bordered shadow-2xl">
            <div class="card-actions justify-end">
                <Show
                    when=move || info.get().status.is_transitioning()
                    fallback=move || view! { "" }.into_view()
                >
                    <span class="loading loading-spinner mr-2"></span>
                    <div class="mr-10">{spinner_msg}</div>
                </Show>
                <Show
                    when=move || info.get().upgradeable()
                    fallback=move || view! { "" }.into_view()
                >
                    <ButtonUpgrade info />
                </Show>
                <NodeLogs info set_logs />
                <ButtonStopStart info />
                <ButtonRemove info />
            </div>
            <p>
                <span class="text-info">"Node Id: "</span>
                {container_id.clone()}
            </p>
            <p>
                <span class="text-info">"Peer Id: "</span>
                {peer_id}
            </p>
            <p>
                <span class="text-info">"Status: "</span>
                {move || format!("{} - {}", info.get().status, info.get().status_info)}
            </p>
            <p>
                <span class="text-info">"Version: "</span>
                {move || info.get().bin_version.unwrap_or_else(|| "unknown".to_string())}
            </p>
            <p>
                <span class="text-info">"Port: "</span>
                {move || info.get().port.map_or("unknown".to_string(), |v| v.to_string())}
            </p>
            <p>
                <span class="text-info">"RPC API Port: "</span>
                {move || info.get().rpc_api_port.map_or("unknown".to_string(), |v| v.to_string())}
            </p>
            <p>
                <span class="text-info">"Balance: "</span>
                {move || info.get().balance.map_or("unknown".to_string(), |v| v.to_string())}
            </p>
            <p>
                <span class="text-info">"Forwarded balance (Beta): "</span>
                {move || {
                    info.get().forwarded_balance.map_or("unknown".to_string(), |v| v.to_string())
                }}
            </p>
            <p>
                <span class="text-info">"Records: "</span>
                {move || info.get().records.map_or("unknown".to_string(), |v| v.to_string())}
            </p>
            <p>
                <span class="text-info">"Connected peers: "</span>
                {move || {
                    info.get().connected_peers.map_or("unknown".to_string(), |v| v.to_string())
                }}
            </p>
            <p>
                <span class="text-info">"kBuckets peers: "</span>
                {move || {
                    info.get().kbuckets_peers.map_or("unknown".to_string(), |v| v.to_string())
                }}
            </p>
            <p>
                <span class="text-info">"Created: "</span>
                {move || {
                    DateTime::<Utc>::from_timestamp(info.get().created as i64, 0)
                        .unwrap()
                        .to_string()
                }}
            </p>
        </div>
    }
}

#[component]
fn NodeLogs(info: RwSignal<NodeInstanceInfo>, set_logs: WriteSignal<Vec<String>>) -> impl IntoView {
    // we use the context to switch on/off the streaming of logs
    let context = expect_context::<ClientGlobalState>();

    // action to trigger the streaming of logs from the node to the 'set_logs' signal
    let start_logs_stream = create_action(move |id: &String| {
        context.logs_stream_is_on.set(true);
        let id = id.clone();
        let signal = set_logs.clone();
        async move {
            let _ = node_logs_stream(id, signal).await;
        }
    });

    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="view logs">
            <label
                for="logs_stream_modal"
                class=move || {
                    if info.get().status.is_transitioning() || info.get().status.is_inactive() {
                        "btn btn-square btn-sm btn-disabled"
                    } else {
                        "btn btn-square btn-sm"
                    }
                }
                on:click=move |_| {
                    set_logs.set(vec![]);
                    start_logs_stream.dispatch(info.get_untracked().container_id.clone());
                }
            >
                <IconShowLogs />
            </label>
        </div>
    }
}

#[component]
fn ButtonStopStart(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    view! {
        <button
            class=move || {
                if info.get().status.is_transitioning() {
                    "btn btn-square btn-sm btn-disabled"
                } else {
                    "btn btn-square btn-sm"
                }
            }
            on:click=move |_| {
                let container_id = info.get().container_id.clone();
                let previous_status = info.get().status;
                if previous_status.is_inactive() {
                    info.update(|node| node.status = NodeStatus::Restarting);
                    spawn_local(async move {
                        match start_node_instance(container_id).await {
                            Ok(()) => {
                                info.update(|node| {
                                    node.status = NodeStatus::Transitioned("Restarted".to_string());
                                })
                            }
                            Err(err) => {
                                logging::log!("Failed to start node: {err:?}");
                                info.update(|node| node.status = previous_status);
                            }
                        }
                    });
                } else {
                    info.update(|node| node.status = NodeStatus::Stopping);
                    spawn_local(async move {
                        match stop_node_instance(container_id).await {
                            Ok(()) => {
                                info.update(|node| {
                                    node.connected_peers = Some(0);
                                    node.kbuckets_peers = Some(0);
                                    node.status = NodeStatus::Transitioned("Stopped".to_string());
                                })
                            }
                            Err(err) => {
                                logging::log!("Failed to stop node: {err:?}");
                                info.update(|node| node.status = previous_status);
                            }
                        }
                    });
                }
            }
        >
            <Show
                when=move || info.get().status.is_inactive()
                fallback=|| {
                    view! {
                        <div class="tooltip tooltip-bottom tooltip-info" data-tip="stop">
                            <IconStopNode />
                        </div>
                    }
                }
            >
                <div class="tooltip tooltip-bottom tooltip-info" data-tip="start">

                    <IconStartNode />
                </div>
            </Show>
        </button>
    }
}

#[component]
fn ButtonUpgrade(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let tip = move || {
        if let Some(v) = context.latest_bin_version.get() {
            format!("upgrade to v{v} and restart")
        } else {
            "upgrade and restart".to_string()
        }
    };
    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip=tip>
            <button
                class=move || {
                    if info.get().status.is_transitioning() {
                        "btn btn-square btn-sm btn-disabled"
                    } else {
                        "btn btn-square btn-sm"
                    }
                }
                on:click=move |_| spawn_local({
                    let previous_status = info.get().status;
                    info.update(|info| info.status = NodeStatus::Upgrading);
                    let container_id = info.get().container_id.clone();
                    async move {
                        match upgrade_node_instance(container_id).await {
                            Ok(()) => {
                                info.update(|node| {
                                    node.status = NodeStatus::Transitioned("Upgraded".to_string());
                                })
                            }
                            Err(err) => {
                                logging::log!("Failed to upgrade node: {err:?}");
                                info.update(|node| node.status = previous_status);
                            }
                        }
                    }
                })
            >
                <IconUpgradeNode />
            </button>
        </div>
    }
}

#[component]
fn ButtonRemove(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="remove">

            <button
                class=move || {
                    if info.get().status.is_transitioning() {
                        "btn btn-square btn-sm btn-disabled"
                    } else {
                        "btn btn-square btn-sm"
                    }
                }
                on:click=move |_| spawn_local({
                    info.update(|info| info.status = NodeStatus::Removing);
                    let container_id = info.get().container_id.clone();
                    async move {
                        let _ = remove_node_instance(container_id).await;
                    }
                })
            >
                <IconRemoveNode />
            </button>
        </div>
    }
}
