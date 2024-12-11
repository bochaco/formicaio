use super::{
    app::ClientGlobalState,
    node_instance::{ContainerId, NodeInstanceInfo},
};

use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodesSortStrategy {
    ByCreationDate(bool),
    ByPortNumber(bool),
    ByRewards(bool),
    ByShunnedCount(bool),
    ByNumRecords(bool),
    ByNumConnPeers(bool),
}

impl std::fmt::Display for NodesSortStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let text = match self {
            Self::ByCreationDate(true) => "creation date ↓",
            Self::ByCreationDate(false) => "creation date ↑",
            Self::ByPortNumber(true) => "port number ↓",
            Self::ByPortNumber(false) => "port number ↑",
            Self::ByRewards(true) => "rewards count ↓",
            Self::ByRewards(false) => "rewards count ↑",
            Self::ByShunnedCount(true) => "shunned count ↓",
            Self::ByShunnedCount(false) => "shunned count ↑",
            Self::ByNumRecords(true) => "number of records ↓",
            Self::ByNumRecords(false) => "number of records ↑",
            Self::ByNumConnPeers(true) => "connected peers ↓",
            Self::ByNumConnPeers(false) => "connected peers ↑",
        };
        write!(f, "{text}")
    }
}

impl NodesSortStrategy {
    pub fn variants() -> Vec<Self> {
        vec![
            Self::ByCreationDate(true),
            Self::ByCreationDate(false),
            Self::ByPortNumber(true),
            Self::ByPortNumber(false),
            Self::ByRewards(true),
            Self::ByRewards(false),
            Self::ByShunnedCount(true),
            Self::ByShunnedCount(false),
            Self::ByNumRecords(true),
            Self::ByNumRecords(false),
            Self::ByNumConnPeers(true),
            Self::ByNumConnPeers(false),
        ]
    }

    pub fn from_str(str: &str) -> Option<Self> {
        match str {
            "ByCreationDate(true)" => Some(Self::ByCreationDate(true)),
            "ByCreationDate(false)" => Some(Self::ByCreationDate(false)),
            "ByPortNumber(true)" => Some(Self::ByPortNumber(true)),
            "ByPortNumber(false)" => Some(Self::ByPortNumber(false)),
            "ByRewards(true)" => Some(Self::ByRewards(true)),
            "ByRewards(false)" => Some(Self::ByRewards(false)),
            "ByShunnedCount(true)" => Some(Self::ByShunnedCount(true)),
            "ByShunnedCount(false)" => Some(Self::ByShunnedCount(false)),
            "ByNumRecords(true)" => Some(Self::ByNumRecords(true)),
            "ByNumRecords(false)" => Some(Self::ByNumRecords(false)),
            "ByNumConnPeers(true)" => Some(Self::ByNumConnPeers(true)),
            "ByNumConnPeers(false)" => Some(Self::ByNumConnPeers(false)),
            _ => None,
        }
    }

    pub fn sort_items(&self, items: &mut Vec<(ContainerId, RwSignal<NodeInstanceInfo>)>) {
        match self {
            NodesSortStrategy::ByCreationDate(true) => {
                items.sort_by(|a, b| b.1.read().created.cmp(&a.1.read().created));
            }
            NodesSortStrategy::ByCreationDate(false) => {
                items.sort_by(|b, a| b.1.read().created.cmp(&a.1.read().created));
            }
            NodesSortStrategy::ByPortNumber(true) => {
                items.sort_by(|a, b| b.1.read().port.cmp(&a.1.read().port));
            }
            NodesSortStrategy::ByPortNumber(false) => {
                items.sort_by(|b, a| b.1.read().port.cmp(&a.1.read().port));
            }
            NodesSortStrategy::ByRewards(true) => {
                items.sort_by(|a, b| b.1.read().rewards.cmp(&a.1.read().rewards));
            }
            NodesSortStrategy::ByRewards(false) => {
                items.sort_by(|b, a| b.1.read().rewards.cmp(&a.1.read().rewards));
            }
            NodesSortStrategy::ByShunnedCount(true) => {
                items.sort_by(|a, b| b.1.read().shunned_count.cmp(&a.1.read().shunned_count));
            }
            NodesSortStrategy::ByShunnedCount(false) => {
                items.sort_by(|b, a| b.1.read().shunned_count.cmp(&a.1.read().shunned_count));
            }
            NodesSortStrategy::ByNumRecords(true) => {
                items.sort_by(|a, b| b.1.read().records.cmp(&a.1.read().records));
            }
            NodesSortStrategy::ByNumRecords(false) => {
                items.sort_by(|b, a| b.1.read().records.cmp(&a.1.read().records));
            }
            NodesSortStrategy::ByNumConnPeers(true) => {
                items.sort_by(|a, b| b.1.read().connected_peers.cmp(&a.1.read().connected_peers));
            }
            NodesSortStrategy::ByNumConnPeers(false) => {
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
                        NodesSortStrategy::from_str(&ev.target().value())
                            .map(|v| context.nodes_sort_strategy.set(v));
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
