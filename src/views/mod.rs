pub mod about;
mod actions_batch;
pub mod alerts;
pub mod chart;
pub mod icons;
pub mod navbar;
pub mod node_actions;
mod node_instance;
pub mod nodes_list;
pub mod pagination;
pub mod settings;
pub mod sort_nodes;
pub mod stats;
pub mod terminal;

use self::{
    alerts::{AlertMsg, OfflineMsg},
    node_actions::NodesActionsView,
    nodes_list::NodesListView,
    pagination::PaginationView,
    stats::AggregatedStatsView,
};

use leptos::prelude::*;

#[component]
pub fn HomeScreenView() -> impl IntoView {
    view! {
        <AlertMsg />

        <AggregatedStatsView />
        <OfflineMsg />
        <NodesActionsView />

        <PaginationView />
        <NodesListView />
    }
}
