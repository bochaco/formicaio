use leptos::*;

use crate::node_instance::NodeInstanceInfo;

#[component]
pub fn AggregatedStatsView(nodes: RwSignal<Vec<RwSignal<NodeInstanceInfo>>>) -> impl IntoView {
    let total_nodes = move || nodes.get().len();
    let active_nodes = move || {
        nodes
            .get()
            .iter()
            .filter(|n| n.get().status.is_active())
            .count()
    };
    let inactive_nodes = move || {
        nodes
            .get()
            .iter()
            .filter(|n| n.get().status.is_inactive())
            .count()
    };
    let shunned_nodes = move || {
        nodes
            .get()
            .iter()
            .filter(|n| n.get().status.is_shunned())
            .count()
    };
    let rewards = move || nodes.get().iter().map(|n| n.get().rewards).sum::<u64>();
    let balance = move || nodes.get().iter().map(|n| n.get().balance).sum::<u64>();
    let chunks = move || nodes.get().iter().map(|n| n.get().chunks).sum::<u64>();

    view! {
        <div class="stat place-items-center">
            <div class="stat-title">Total rewards</div>
            <div class="stat-value text-primary">{rewards}</div>
            <div class="stat-desc">21% more than last month</div>
        </div>

        <div class="stat place-items-center">
            <div class="stat-title">Total current balance</div>
            <div class="stat-value text-primary">{balance}</div>
        </div>

        <div class="stat place-items-center">
            <div class="stat-title">Stored chunks</div>
            <div class="stat-value text-secondary">{chunks}</div>
            <div class="stat-desc">10% more than last month</div>
        </div>

        <div class="stat place-items-center">
            <div class="stat-title">Active nodes</div>
            <div class="stat-value">{active_nodes} " / " {total_nodes}</div>
            <div class="stat-desc text-secondary">
                {shunned_nodes} " shunned | " {inactive_nodes} " inactive"
            </div>
        </div>
    }
}
