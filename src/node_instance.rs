use super::{
    helpers::{node_logs_stream, remove_node_instance},
    server_api::{start_node_instance, stop_node_instance},
};

use chrono::{DateTime, Utc};
use leptos::*;
use serde::{Deserialize, Serialize};

// Length of nodes PeerIds' prefix and suffix to be displayed
const PEER_ID_PREFIX_SUFFIX_LEN: usize = 10;
// Length of nodes Docker container ids' prefix to be displayed
const CONTAINER_ID_PREFIX_LEN: usize = 12;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum NodeStatus {
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
}

impl NodeStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
    pub fn is_inactive(&self) -> bool {
        matches!(self, Self::Inactive)
    }
    pub fn is_shunned(&self) -> bool {
        matches!(self, Self::Shunned)
    }
    pub fn is_changing(&self) -> bool {
        match self {
            Self::Restarting | Self::Stopping | Self::Removing => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInstanceInfo {
    pub container_id: String,
    pub created: u64,
    pub peer_id: Option<String>, // base58-encoded Peer Id bytes
    pub status: NodeStatus,
    pub status_info: String,
    pub bin_version: Option<String>,
    pub rewards: Option<u64>,
    pub balance: Option<u64>,
    pub chunks: Option<u64>,
    pub connected_peers: Option<usize>,
}

#[component]
pub fn NodeInstanceView(
    info: RwSignal<NodeInstanceInfo>,
    nodes: RwSignal<Vec<RwSignal<NodeInstanceInfo>>>,
) -> impl IntoView {
    let peer_id = info
        .get_untracked()
        .peer_id
        .map_or("unknown".to_string(), |id| {
            format!(
                "{}...{}",
                &id[..PEER_ID_PREFIX_SUFFIX_LEN],
                &id[id.len() - PEER_ID_PREFIX_SUFFIX_LEN..]
            )
        });
    let container_id = info.get_untracked().container_id[..CONTAINER_ID_PREFIX_LEN].to_string();

    view! {
        <div class="m-2 p-4 card card-normal bg-neutral text-neutral-content card-bordered shadow-2xl">
            <div class="card-actions justify-end">
                <NodeLogs container_id=info.get_untracked().container_id />
                <Show
                    when=move || info.get().status.is_changing()
                    fallback=move || view! { <ButtonStopStart info /> }.into_view()
                >
                    <button class="btn btn-square btn-sm">
                        <span class="loading loading-spinner" />
                    </button>
                </Show>
                <ButtonRemove info nodes />
            </div>
            <p>"Node Id: " {container_id.clone()}</p>
            <p>"Peer Id: " {peer_id}</p>
            <p>
                "Status: " {move || format!("{:?} - {}", info.get().status, info.get().status_info)}
            </p>
            <p>
                "Version: "
                {move || info.get().bin_version.unwrap_or_else(|| "unknown".to_string())}
            </p>
            <p>
                "Balance: "
                {move || info.get().balance.map_or("unknown".to_string(), |v| v.to_string())}
            </p>
            <p>
                "Connected peers: "
                {move || {
                    info.get().connected_peers.map_or("unknown".to_string(), |v| v.to_string())
                }}
            </p>
            <p>
                "Created: "
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
fn NodeLogs(container_id: String) -> impl IntoView {
    // we use the context to switch on/off the streaming of logs
    let logs_stream_is_on = expect_context::<RwSignal<bool>>();
    // this signal keeps the reactive list of log entries
    let (logs, set_logs) = create_signal(Vec::new());

    // action to trigger the streaming of logs from the node to the 'set_logs' signal
    let start_logs_stream =
        create_action(move |(id, signal): &(String, WriteSignal<Vec<String>>)| {
            logs_stream_is_on.set(true);
            let id = id.clone();
            let signal = signal.clone();
            async move {
                let _ = node_logs_stream(id, signal).await;
            }
        });

    view! {
        <label
            for="logs_stream_modal"
            class="btn btn-square btn-sm"
            on:click=move |_| start_logs_stream.dispatch((container_id.clone(), set_logs))
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                class="h-6 w-6"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
            >
                <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M2 2 L15 2 L22 9 L15 9 L15 2 M22 9 L22 22 L2 22 L2 2 M6 9 L11 9 M6 13 L17 13 M6 17 L17 17"
                />
            </svg>
        </label>

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
                            <li>">> " {child.1}</li>
                        </For>
                    </ul>
                </div>

                <div class="modal-action">
                    <label
                        for="logs_stream_modal"
                        class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2"
                        on:click=move |_| logs_stream_is_on.set(false)
                    >
                        X
                    </label>
                </div>
            </div>
        </div>
    }
}

#[component]
fn ButtonStopStart(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    view! {
        <button
            class="btn btn-square btn-sm"
            on:click=move |_| {
                let container_id = info.get().container_id.clone();
                let previous_status = info.get().status;
                if previous_status.is_inactive() {
                    info.update(|node| node.status = NodeStatus::Restarting);
                    spawn_local(async move {
                        match start_node_instance(container_id).await {
                            Ok(()) => info.update(|node| node.status = NodeStatus::Active),
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
                            Ok(()) => info.update(|node| node.status = NodeStatus::Inactive),
                            Err(err) => {
                                logging::log!("Failed to stop node: {err:?}");
                                info.update(|node| node.status = previous_status);
                            }
                        }
                    });
                }
            }
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                class="h-6 w-6"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
            >
                <Show
                    when=move || info.get().status.is_inactive()
                    fallback=|| {
                        view! {
                            <rect
                                width="13"
                                height="13"
                                x="5"
                                y="5"
                                fill="currentColor"
                                stroke-width="2"
                            />
                        }
                    }
                >
                    <polygon points="6,6 18,12 6,18" fill="currentColor" stroke-width="2" />
                </Show>
            </svg>
        </button>
    }
}

#[component]
fn ButtonRemove(
    info: RwSignal<NodeInstanceInfo>,
    nodes: RwSignal<Vec<RwSignal<NodeInstanceInfo>>>,
) -> impl IntoView {
    view! {
        <button
            class=move || {
                if info.get().status.is_changing() {
                    "btn btn-square btn-sm btn-disabled"
                } else {
                    "btn btn-square btn-sm"
                }
            }
            on:click=move |_| spawn_local({
                info.update(|info| info.status = NodeStatus::Removing);
                let container_id = info.get().container_id.clone();
                async move {
                    let _ = remove_node_instance(container_id, nodes).await;
                }
            })
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                class="h-6 w-6"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
            >
                <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M6 18L18 6M6 6l12 12"
                />
            </svg>
        </button>
    }
}
