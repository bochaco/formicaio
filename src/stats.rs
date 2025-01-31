use super::{app::ClientGlobalState, helpers::truncated_balance_str};

use alloy_primitives::utils::format_units;
use leptos::prelude::*;

#[component]
pub fn AggregatedStatsView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    let total_nodes = move || context.nodes.read().1.len();
    let active_nodes = move || {
        context
            .nodes
            .read()
            .1
            .values()
            .filter(|n| n.read().status.is_active())
            .count()
    };
    let inactive_nodes = move || {
        context
            .nodes
            .read()
            .1
            .values()
            .filter(|n| n.read().status.is_inactive())
            .count()
    };
    let total_balance = move || context.stats.read().total_balance;
    let connected_peers = move || context.stats.read().connected_peers;
    let shunned_count = move || context.stats.read().shunned_count;
    let estimated_net_size = move || context.stats.read().estimated_net_size;
    let stored_records = move || context.stats.read().stored_records;
    let relevant_records = move || context.stats.read().relevant_records;

    view! {
        <div class="stats flex">
            <div class="stat place-items-center">
                <div class="stat-title">Current total balance</div>
                <div class="stat-value text-primary">
                    {move || truncated_balance_str(total_balance())}
                </div>
                <div class="stat-desc text-secondary">
                    {move || format_units(total_balance(), "ether").unwrap_or_default()}
                </div>
            </div>

            <div class="stat place-items-center">
                <div class="stat-title">Total connected peers</div>
                <div class="stat-value text-primary">{connected_peers}</div>
                <div class="stat-desc text-secondary">"shunned by " {shunned_count} " peers"</div>
            </div>

            <div class="stat place-items-center">
                <div class="stat-title">Active nodes</div>
                <div class="stat-value">{active_nodes} " / " {total_nodes}</div>
                <div class="stat-desc text-secondary">{inactive_nodes} " inactive"</div>
            </div>

            <div class="stat place-items-center">
                <div class="stat-title">Stored records</div>
                <div class="stat-value">{stored_records}</div>
                <div class="stat-desc text-secondary">{relevant_records} " are relevant"</div>
            </div>
            <div class="stat place-items-center">
                <div class="stat-title">Estimated network size</div>
                <div class="stat-value text-primary">{estimated_net_size}</div>
            </div>
        </div>
    }
}
