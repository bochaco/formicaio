use super::{
    app::ClientGlobalState,
    node_instance::{NodeId, NodeInstanceInfo},
};

use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodesSortStrategy {
    CreationDate(bool),
    PortNumber(bool),
    Rewards(bool),
    ShunnedCount(bool),
    NumRecords(bool),
    NumConnPeers(bool),
}

impl std::fmt::Display for NodesSortStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let text = match self {
            Self::CreationDate(true) => "creation date ↓",
            Self::CreationDate(false) => "creation date ↑",
            Self::PortNumber(true) => "port number ↓",
            Self::PortNumber(false) => "port number ↑",
            Self::Rewards(true) => "rewards count ↓",
            Self::Rewards(false) => "rewards count ↑",
            Self::ShunnedCount(true) => "shunned count ↓",
            Self::ShunnedCount(false) => "shunned count ↑",
            Self::NumRecords(true) => "number of records ↓",
            Self::NumRecords(false) => "number of records ↑",
            Self::NumConnPeers(true) => "connected peers ↓",
            Self::NumConnPeers(false) => "connected peers ↑",
        };
        write!(f, "{text}")
    }
}

impl NodesSortStrategy {
    pub fn variants() -> Vec<Self> {
        vec![
            Self::CreationDate(true),
            Self::CreationDate(false),
            Self::PortNumber(true),
            Self::PortNumber(false),
            Self::Rewards(true),
            Self::Rewards(false),
            Self::ShunnedCount(true),
            Self::ShunnedCount(false),
            Self::NumRecords(true),
            Self::NumRecords(false),
            Self::NumConnPeers(true),
            Self::NumConnPeers(false),
        ]
    }

    pub fn from_str(str: &str) -> Option<Self> {
        match str {
            "CreationDate(true)" => Some(Self::CreationDate(true)),
            "CreationDate(false)" => Some(Self::CreationDate(false)),
            "PortNumber(true)" => Some(Self::PortNumber(true)),
            "PortNumber(false)" => Some(Self::PortNumber(false)),
            "Rewards(true)" => Some(Self::Rewards(true)),
            "Rewards(false)" => Some(Self::Rewards(false)),
            "ShunnedCount(true)" => Some(Self::ShunnedCount(true)),
            "ShunnedCount(false)" => Some(Self::ShunnedCount(false)),
            "NumRecords(true)" => Some(Self::NumRecords(true)),
            "NumRecords(false)" => Some(Self::NumRecords(false)),
            "NumConnPeers(true)" => Some(Self::NumConnPeers(true)),
            "NumConnPeers(false)" => Some(Self::NumConnPeers(false)),
            _ => None,
        }
    }

    pub fn sort_items(&self, items: &mut [(NodeId, RwSignal<NodeInstanceInfo>)]) {
        match self {
            NodesSortStrategy::CreationDate(true) => {
                items.sort_by(|a, b| b.1.read().created.cmp(&a.1.read().created));
            }
            NodesSortStrategy::CreationDate(false) => {
                items.sort_by(|b, a| b.1.read().created.cmp(&a.1.read().created));
            }
            NodesSortStrategy::PortNumber(true) => {
                items.sort_by(|a, b| b.1.read().port.cmp(&a.1.read().port));
            }
            NodesSortStrategy::PortNumber(false) => {
                items.sort_by(|b, a| b.1.read().port.cmp(&a.1.read().port));
            }
            NodesSortStrategy::Rewards(true) => {
                items.sort_by(|a, b| b.1.read().rewards.cmp(&a.1.read().rewards));
            }
            NodesSortStrategy::Rewards(false) => {
                items.sort_by(|b, a| b.1.read().rewards.cmp(&a.1.read().rewards));
            }
            NodesSortStrategy::ShunnedCount(true) => {
                items.sort_by(|a, b| b.1.read().shunned_count.cmp(&a.1.read().shunned_count));
            }
            NodesSortStrategy::ShunnedCount(false) => {
                items.sort_by(|b, a| b.1.read().shunned_count.cmp(&a.1.read().shunned_count));
            }
            NodesSortStrategy::NumRecords(true) => {
                items.sort_by(|a, b| b.1.read().records.cmp(&a.1.read().records));
            }
            NodesSortStrategy::NumRecords(false) => {
                items.sort_by(|b, a| b.1.read().records.cmp(&a.1.read().records));
            }
            NodesSortStrategy::NumConnPeers(true) => {
                items.sort_by(|a, b| b.1.read().connected_peers.cmp(&a.1.read().connected_peers));
            }
            NodesSortStrategy::NumConnPeers(false) => {
                items.sort_by(|b, a| b.1.read().connected_peers.cmp(&a.1.read().connected_peers));
            }
        }
    }
}

#[component]
pub fn SortStrategyView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    view! {
        <div class="flex w-full flex-col">
            <div class="divider">
                <select
                    class="block py-2.5 px-0 text-sm text-gray-500 bg-transparent border-0 appearance-none dark:text-gray-400 focus:outline-none focus:ring-0 focus:border-gray-200"
                    on:change:target=move |ev| {
                        if let Some(v) = NodesSortStrategy::from_str(&ev.target().value()) {
                            context.nodes_sort_strategy.set(v);
                        }
                    }
                >
                    {NodesSortStrategy::variants()
                        .into_iter()
                        .map(|variant| {
                            view! {
                                <option
                                    selected=move || {
                                        context.nodes_sort_strategy.read() == variant
                                    }
                                    value=format!("{variant:?}")
                                >
                                    "Sort by "
                                    {variant.to_string()}
                                </option>
                            }
                                .into_view()
                        })
                        .collect::<Vec<_>>()}
                </select>
            </div>
        </div>
    }
}
