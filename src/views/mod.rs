pub mod about;
mod actions_batch;
mod add_nodes;
mod alerts;
mod chart;
mod form_inputs;
mod helpers;
mod icons;
pub mod navbar;
mod node_actions;
mod node_instance;
mod nodes_list;
mod pagination;
mod settings;
mod sort_nodes;
mod stats;
pub mod terminal;

pub use helpers::truncated_balance_str;

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
