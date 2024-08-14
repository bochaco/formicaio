use leptos::*;
use serde::{Deserialize, Serialize};

// Length of nodes PeerIds' prefix and suffix to be displayed
const PEER_ID_PREFIX_SUFFIX_LEN: usize = 10;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum NodeStatus {
    // A running node connected to peers on the network is considered Active.
    Active,
    // A node not connected to any peer on the network is considered Inactive.
    Inactive,
    // When a node is running and connected to peers on the network but it's
    // being considered a bad node by them, then this node is considered Shunned.
    Shunned, // TODO: include suspected reason as to why others shunned it ...??
}

impl NodeStatus {
    pub fn is_active(&self) -> bool {
        match self {
            Self::Active => true,
            Self::Inactive | Self::Shunned => false,
        }
    }
    pub fn is_inactive(&self) -> bool {
        match self {
            Self::Inactive => true,
            Self::Active | Self::Shunned => false,
        }
    }
    pub fn is_shunned(&self) -> bool {
        match self {
            Self::Shunned => true,
            Self::Active | Self::Inactive => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInstanceInfo {
    pub name: String,
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
    let peer_id_str = bs58::encode(info.get().peer_id).into_string();
    let peer_id = format!(
        "{}...{}",
        &peer_id_str[..PEER_ID_PREFIX_SUFFIX_LEN],
        &peer_id_str[peer_id_str.len() - PEER_ID_PREFIX_SUFFIX_LEN..]
    );

    view! {
        <div class="card-normal bg-base-100 w-96 shadow-xl">
            <div class="card-compact">
                <div class="card-actions justify-end">
                    <ButtonStopStart info nodes />
                    <ButtonRemove peer_id=info.get().peer_id nodes />
                </div>
                <p>"Name: " {info.get().name}</p>
                <p>"Peer Id: " {peer_id}</p>
                <p>"Status: " {move || format!("{:?}", info.get().status)}</p>
                <p>"Balance: " {move || info.get().balance}</p>
            </div>
        </div>
    }
}

#[component]
fn ButtonStopStart(
    info: RwSignal<NodeInstanceInfo>,
    nodes: RwSignal<Vec<RwSignal<NodeInstanceInfo>>>,
) -> impl IntoView {
    view! {
        <button
            class="btn btn-square btn-sm"
            on:click=move |_| {
                nodes
                    .with(|nodes| {
                        nodes
                            .iter()
                            .find(|node| node.get().peer_id == info.get().peer_id)
                            .map(|node| {
                                if node.get().status.is_inactive() {
                                    node.update(|info| info.status = NodeStatus::Active);
                                } else {
                                    node.update(|info| info.status = NodeStatus::Inactive);
                                }
                            });
                    })
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
    peer_id: Vec<u8>,
    nodes: RwSignal<Vec<RwSignal<NodeInstanceInfo>>>,
) -> impl IntoView {
    view! {
        <button
            class="btn btn-square btn-sm"
            on:click=move |_| {
                nodes
                    .update(|nodes| {
                        nodes.retain(|node| node.get().peer_id != peer_id);
                    })
            }
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
