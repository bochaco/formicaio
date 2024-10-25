use super::app::ClientGlobalState;

use leptos::*;

#[component]
pub fn AggregatedStatsView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    let total_nodes = move || context.nodes.get().len();
    let active_nodes = move || {
        context
            .nodes
            .get()
            .values()
            .filter(|n| n.get().status.is_active())
            .count()
    };
    let inactive_nodes = move || {
        context
            .nodes
            .get()
            .values()
            .filter(|n| n.get().status.is_inactive())
            .count()
    };
    let balance = move || {
        context
            .nodes
            .get()
            .values()
            .map(|n| n.get().balance.unwrap_or_default())
            .sum::<u64>()
    };
    let connected_peers = move || {
        context
            .nodes
            .get()
            .values()
            .map(|n| n.get().connected_peers.unwrap_or_default())
            .sum::<usize>()
    };
    let stored_records = move || {
        context
            .nodes
            .get()
            .values()
            .map(|n| {
                if n.get().status.is_active() {
                    n.get().records.unwrap_or_default()
                } else {
                    0
                }
            })
            .sum::<usize>()
    };
    let inactive_records = move || {
        context
            .nodes
            .get()
            .values()
            .map(|n| {
                if n.get().status.is_inactive() {
                    n.get().records.unwrap_or_default()
                } else {
                    0
                }
            })
            .sum::<usize>()
    };
    let relevant_records = move || {
        context
            .nodes
            .get()
            .values()
            .map(|n| {
                if n.get().status.is_active() {
                    n.get().relevant_records.unwrap_or_default()
                } else {
                    0
                }
            })
            .sum::<usize>()
    };

    view! {
        <div class="stats flex">
            <div class="stat place-items-center">
                <div class="stat-title">Current total balance</div>
                <div class="stat-value text-primary">{balance}</div>
            </div>

            <div class="stat place-items-center">
                <div class="stat-title">Total connected peers</div>
                <div class="stat-value text-primary">{connected_peers}</div>
            </div>

            <div class="stat place-items-center">
                <div class="stat-title">Active nodes</div>
                <div class="stat-value">{active_nodes} " / " {total_nodes}</div>
                <div class="stat-desc text-secondary">{inactive_nodes} " inactive"</div>
            </div>

            <div class="stat place-items-center">
                <div class="stat-title">Stored records</div>
                <div class="stat-value">{stored_records}</div>
                <div class="stat-desc text-secondary">
                    {relevant_records} " are relevant | " {inactive_records}
                    " are in inactive nodes"
                </div>
            </div>
        </div>
    }
}
