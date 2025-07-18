use crate::app::ClientGlobalState;

use super::helpers::truncated_balance_str;

use alloy_primitives::utils::format_units;
use leptos::prelude::*;

#[component]
pub fn AggregatedStatsView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    view! {
        <div class="stats flex">
            <div class="stat place-items-center">
                <div class="stat-title">Current total balance</div>
                <div class="stat-value text-primary">
                    {move || truncated_balance_str(context.stats.read().total_balance)}
                </div>
                <div class="stat-desc text-secondary">
                    {move || {
                        format_units(context.stats.read().total_balance, "ether")
                            .unwrap_or_default()
                    }}
                </div>
            </div>

            <div class="stat place-items-center">
                <div class="stat-title">Total connected peers</div>
                <div class="stat-value text-primary">
                    {move || context.stats.read().connected_peers}
                </div>
                <div class="stat-desc text-secondary">
                    "shunned by " {move || context.stats.read().shunned_count} " peers"
                </div>
            </div>

            <div class="stat place-items-center">
                <div class="stat-title">Active nodes</div>
                <div class="stat-value">
                    {move || context.stats.read().active_nodes} " / "
                    {move || context.stats.read().total_nodes}
                </div>
                <div class="stat-desc text-secondary">
                    {move || context.stats.read().inactive_nodes} " inactive"
                </div>
            </div>

            <div class="stat place-items-center">
                <div class="stat-title">Stored records</div>
                <div class="stat-value">{move || context.stats.read().stored_records}</div>
                <div class="stat-desc text-secondary">
                    {move || context.stats.read().relevant_records} " are relevant"
                </div>
            </div>
            <div class="stat place-items-center">
                <div class="stat-title">Estimated network size</div>
                <div class="stat-value text-primary">
                    {move || context.stats.read().estimated_net_size}
                </div>
            </div>
        </div>
    }
}
