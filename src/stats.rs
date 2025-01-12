use super::{app::ClientGlobalState, helpers::truncated_balance_str};

use alloy::primitives::{utils::format_units, U256};
use leptos::prelude::*;
use std::collections::HashMap;

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
    let total_balance = move || {
        let mut total = U256::ZERO;
        let seen = context
            .nodes
            .read()
            .1
            .values()
            .filter_map(|n| {
                n.read()
                    .balance
                    .map(|v| (n.get_untracked().rewards_addr, v))
            })
            .fold(HashMap::new(), |mut acc, (addr, v)| {
                acc.insert(addr, v);
                acc
            });

        for (_, balance) in seen.iter() {
            total += balance;
        }

        total
    };

    let connected_peers = move || {
        context
            .nodes
            .read()
            .1
            .values()
            .map(|n| n.read().connected_peers.unwrap_or_default())
            .sum::<usize>()
    };
    let shunned_by = move || {
        context
            .nodes
            .read()
            .1
            .values()
            .map(|n| n.read().shunned_count.unwrap_or_default())
            .sum::<usize>()
    };
    let estimated_net_size = move || {
        let weighted_estimations = context
            .nodes
            .read()
            .1
            .values()
            .filter(|n| n.read().status.is_active())
            .map(|n| {
                n.read().connected_peers.unwrap_or_default() * n.read().net_size.unwrap_or_default()
            })
            .sum::<usize>();
        let weights = context
            .nodes
            .read()
            .1
            .values()
            .filter(|n| n.read().status.is_active())
            .map(|n| n.read().connected_peers.unwrap_or_default())
            .sum::<usize>();

        if weights > 0 {
            weighted_estimations / weights
        } else {
            0
        }
    };
    let stored_records = move || {
        context
            .nodes
            .read()
            .1
            .values()
            .map(|n| {
                if n.read().status.is_active() {
                    n.read().records.unwrap_or_default()
                } else {
                    0
                }
            })
            .sum::<usize>()
    };
    let inactive_records = move || {
        context
            .nodes
            .read()
            .1
            .values()
            .map(|n| {
                if n.read().status.is_inactive() {
                    n.read().records.unwrap_or_default()
                } else {
                    0
                }
            })
            .sum::<usize>()
    };
    let relevant_records = move || {
        context
            .nodes
            .read()
            .1
            .values()
            .map(|n| {
                if n.read().status.is_active() {
                    n.read().relevant_records.unwrap_or_default()
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
                <div class="stat-desc text-secondary">"shunned by " {shunned_by} " peers"</div>
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
            <div class="stat place-items-center">
                <div class="stat-title">Estimated network size</div>
                <div class="stat-value text-primary">{estimated_net_size}</div>
            </div>
        </div>
    }
}
