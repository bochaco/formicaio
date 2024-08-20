use super::server_api::{remove_node_instance, start_node_instance, stop_node_instance};

use chrono::{DateTime, Utc};
use leptos::*;
use serde::{Deserialize, Serialize};

// Length of nodes PeerIds' prefix and suffix to be displayed
const PEER_ID_PREFIX_SUFFIX_LEN: usize = 10;

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
    pub peer_id: Vec<u8>,
    pub status: NodeStatus,
    pub rewards: u64,
    pub balance: u64,
    pub chunks: u64,
}

#[component]
pub fn NodeInstanceView(
    info: RwSignal<NodeInstanceInfo>,
    nodes: RwSignal<Vec<RwSignal<NodeInstanceInfo>>>,
) -> impl IntoView {
    let peer_id_str = bs58::encode(info.get_untracked().peer_id).into_string();
    let peer_id_base58 = format!(
        "{}...{}",
        &peer_id_str[..PEER_ID_PREFIX_SUFFIX_LEN],
        &peer_id_str[peer_id_str.len() - PEER_ID_PREFIX_SUFFIX_LEN..]
    );

    view! {
        <div class="card-normal bg-base-100 w-96 shadow-xl">
            <div class="card-compact">
                <div class="card-actions justify-end">
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
                <p>"Node Id: " {info.get_untracked().container_id}</p>
                <p>"Peer Id: " {peer_id_base58}</p>
                <p>"Status: " {move || format!("{:?}", info.get().status)}</p>
                <p>"Balance: " {move || info.get().balance}</p>
                <p>
                    "Created: "
                    {move || {
                        DateTime::<Utc>::from_timestamp(info.get().created as i64, 0)
                            .unwrap()
                            .to_string()
                    }}
                </p>
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
