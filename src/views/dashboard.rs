use crate::{app::ClientGlobalState, types::shortened_address};

use super::{
    GB_CONVERTION,
    earnings::RewardsEarningsCard,
    format_disk_usage,
    helpers::truncated_balance_str,
    icons::{
        IconActivity, IconArrowUpRight, IconDisk, IconFile, IconPeers, IconServer, IconWallet,
    },
};

use alloy_primitives::{U256, utils::format_units};
use leptos::prelude::*;

// Number of nodes to display as the top most connected nodes
const NUMBER_OF_TOP_NODES: usize = 10;

#[component]
pub fn DashboardView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    let sorted_nodes = Memo::new(move |_| {
        let mut sorted = context
            .nodes
            .get()
            .1
            .into_iter()
            .filter(|(_, n)| n.read().status.is_active())
            .collect::<Vec<_>>();
        sorted.sort_by(|a, b| b.1.read().connected_peers.cmp(&a.1.read().connected_peers));
        sorted.truncate(NUMBER_OF_TOP_NODES);
        sorted
    });

    view! {
        <div class="p-4 lg:p-8 space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
            // Stats Grid
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
                <StatCard
                    title="Total Nodes"
                    value=Signal::derive(move || context.stats.read().total_nodes.to_string())
                    sub_value=Signal::derive(move || {
                        context
                            .stats
                            .with(|s| {
                                format!("{} Active | {} Inactive", s.active_nodes, s.inactive_nodes)
                            })
                    })
                    icon=view! { <IconServer class="text-indigo-400 w-8 h-8" /> }.into_any()
                />
                <BalanceCard
                    total=Signal::derive(move || truncated_balance_str(
                        context.stats.read().total_balance,
                    ))
                    sub_value=Signal::derive(move || {
                        format_units(context.stats.read().total_balance, "ether")
                            .unwrap_or_default()
                    })
                    base_url=Signal::derive(move || {
                        format!(
                            "https://arbiscan.io/token/{}",
                            context.app_settings.read().token_contract_address,
                        )
                    })
                    balances=Signal::derive(move || context.stats.read().balances.clone())
                />
                <StatCard
                    title="Estimated Network Size"
                    value=Signal::derive(move || {
                        context.stats.read().estimated_net_size.to_string()
                    })
                    icon=view! { <IconActivity class="text-rose-400 w-7 h-7" /> }.into_any()
                />
                <StatCard
                    title="Stored Records"
                    value=Signal::derive(move || context.stats.read().stored_records.to_string())
                    sub_value=Signal::derive(move || {
                        format!("{} Relevant", context.stats.read().relevant_records)
                    })
                    icon=view! { <IconFile class="text-amber-400 w-8 h-8" /> }.into_any()
                />
                <StatCard
                    title="Total Connected Peers"
                    value=Signal::derive(move || context.stats.read().connected_peers.to_string())
                    sub_value=Signal::derive(move || {
                        format!("Shunned by {}", context.stats.read().shunned_count)
                    })
                    icon=view! { <IconPeers class="text-cyan-400 w-8 h-8" /> }.into_any()
                />
                <DiskUsageCard
                    available=Signal::derive(move || context.stats.read().available_disk_space)
                    node_used=Signal::derive(move || context.stats.read().used_disk_space)
                    total=Signal::derive(move || context.stats.read().total_disk_space)
                />

            </div>

            // Analytics Card - Comprehensive Rewards Breakdown
            <div class="grid grid-cols-1 gap-6">
                <RewardsEarningsCard />
            </div>

            // Current Nodes Activity
            <div class="bg-slate-900 border border-slate-800 rounded-2xl overflow-hidden shadow-xl">
                <div class="p-6 border-b border-slate-800 flex items-center justify-between">
                    <h3 class="text-lg font-bold">
                        "Top " {NUMBER_OF_TOP_NODES} " Most Connected Nodes"
                    </h3>
                </div>
                <div class="overflow-x-auto">
                    <table class="w-full text-left">
                        <thead>
                            <tr class="bg-slate-800/50 text-slate-400 text-xs uppercase tracking-wider">
                                <th class="px-6 py-4 font-semibold">Node Id</th>
                                <th class="px-6 py-4 font-semibold">Status</th>
                                <th class="px-6 py-4 text-center font-semibold">Stored Records</th>
                                <th class="px-6 py-4 text-center font-semibold">
                                    Estimated Network Size
                                </th>
                                <th class="px-6 py-4 font-semibold text-center">Peers</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800">
                            <For
                                each=move || sorted_nodes.get()
                                key=|(node_id, _)| node_id.clone()
                                let:child
                            >
                                <tr class="hover:bg-slate-800/30 transition-colors group">
                                    <td class="px-6 py-4">
                                        <div class="flex items-center gap-3">
                                            <div class="w-8 h-8 rounded bg-slate-800 flex items-center justify-center text-slate-400">
                                                <IconActivity />
                                            </div>
                                            <div>
                                                <div class="font-medium text-slate-200">
                                                    {move || child.1.read().short_node_id()}
                                                </div>
                                            </div>
                                        </div>
                                    </td>
                                    <td class="px-6 py-4">
                                        <span class=move || {
                                            format!(
                                                "px-2.5 py-1 rounded-full text-[10px] font-bold uppercase tracking-wide {}",
                                                if child.1.read().status.is_active() {
                                                    "bg-emerald-500/10 text-emerald-500 border border-emerald-500/20"
                                                } else if child.1.read().status.is_stopped() {
                                                    "bg-rose-500/10 text-rose-500 border border-rose-500/20"
                                                } else {
                                                    "bg-amber-500/10 text-amber-500 border border-amber-500/20"
                                                },
                                            )
                                        }>{move || child.1.read().status_summary()}</span>
                                    </td>
                                    <td class="px-6 py-4 text-center font-mono text-cyan-400">
                                        {move || child.1.read().records}
                                    </td>
                                    <td class="px-6 py-4 text-center font-mono text-cyan-400">
                                        {move || child.1.read().net_size}
                                    </td>
                                    <td class="px-6 py-4 text-center font-mono text-cyan-400">
                                        {move || child.1.read().connected_peers}
                                    </td>
                                </tr>
                            </For>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
}

#[component]
fn StatCard(
    title: &'static str,
    value: Signal<String>,
    #[prop(default=Signal::stored("".to_string()))] sub_value: Signal<String>,
    icon: AnyView,
) -> impl IntoView {
    view! {
        <div class="bg-slate-900 border border-slate-800 p-6 rounded-2xl hover:border-indigo-500/50 transition-all duration-300 group shadow-lg flex flex-col justify-between min-h-[140px]">
            <div>
                <div class="flex items-center gap-4 mb-4">
                    <div class="p-2.5 bg-slate-800 rounded-xl group-hover:scale-110 transition-transform duration-300">
                        {icon}
                    </div>
                    <div class="text-slate-400 text-xs font-bold uppercase tracking-wider">
                        {title}
                    </div>
                </div>
                <div class="text-3xl font-bold tracking-tight text-white">
                    {move || value.get()}
                </div>
            </div>
            <div class="text-slate-400 text-sm mt-1 font-medium">{move || sub_value.get()}</div>
        </div>
    }
}

#[component]
fn DiskUsageCard(
    available: Signal<u64>,
    node_used: Signal<u64>,
    total: Signal<u64>,
) -> impl IntoView {
    let percentage = move || {
        if total.get() > 0 {
            ((total.get() - available.get()) as f64 * 100.0) / total.get() as f64
        } else {
            0f64
        }
    };

    let total_gb = move || total.get() as f64 / GB_CONVERTION;
    let free_gb = move || available.get() as f64 / GB_CONVERTION;
    let used = move || format_disk_usage(node_used.get());

    let colors = move || {
        let percentage = percentage();
        if percentage >= 95.0 {
            ("bg-rose-500", "text-rose-400")
        } else if percentage >= 80.0 {
            ("bg-amber-500", "text-amber-400")
        } else if percentage > 0.0 {
            ("bg-indigo-500", "text-indigo-400")
        } else {
            ("bg-slate-500", "text-slate-400")
        }
    };

    view! {
        <div class="bg-slate-900 border border-slate-800 p-6 rounded-2xl hover:border-indigo-500/50 transition-all duration-300 group shadow-lg flex flex-col">
            // Header
            <div class="flex items-center gap-4 mb-4">
                <div class="p-2.5 bg-slate-800 rounded-xl group-hover:scale-110 transition-transform duration-300">
                    <IconDisk class="text-indigo-400 w-8 h-8" />
                </div>
                <div class="text-slate-400 text-xs font-bold uppercase tracking-wider">
                    Nodes Disk Usage
                </div>
            </div>

            // Content
            <div class="space-y-4 flex-grow flex flex-col justify-center">
                // Value 1: Total Node Data
                <div class="text-3xl font-bold tracking-tight text-white">{move || used()}</div>

                // Value 2: Disk Status
                <Show when=move || 0 < node_used.get()>
                    <div>
                        <div class="flex justify-between items-baseline mb-1">
                            <span class="text-slate-400 text-xs uppercase font-bold tracking-wider">
                                Disk Status
                            </span>
                            <span class=move || {
                                format!("text-sm font-bold {}", colors().1)
                            }>{move || format!("{:.2}", percentage())}% Used</span>

                        </div>
                        <div class="w-full bg-slate-700 rounded-full h-2.5">
                            <div
                                class=move || {
                                    format!(
                                        "{} h-2.5 rounded-full transition-all duration-500",
                                        colors().0,
                                    )
                                }
                                style=move || format!("width: {}%", percentage())
                            />
                        </div>
                        <div class="text-sm font-medium text-slate-400 mt-1.5 text-right">
                            <span class="font-bold text-white">
                                {move || format!("{:.2} GB", free_gb())}
                            </span>
                            {move || format!(" free of {:.2} GB", total_gb())}
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}

#[component]
fn BalanceCard(
    total: Signal<String>,
    sub_value: Signal<String>,
    base_url: Signal<String>,
    balances: Signal<Vec<(String, U256)>>,
) -> impl IntoView {
    view! {
        <div class="relative group">
            <div class="bg-slate-900 border border-slate-800 p-6 rounded-2xl hover:border-emerald-500/50 transition-all duration-300 group-hover:border-emerald-500/50 shadow-lg flex flex-col justify-between min-h-[140px]">
                <div>
                    <div class="flex items-center gap-4 mb-4">
                        <div class="p-2.5 bg-slate-800 rounded-xl group-hover:scale-110 transition-transform duration-300">
                            <IconWallet class="text-emerald-400 w-8 h-8" />
                        </div>
                        <div class="text-slate-400 text-xs font-bold uppercase tracking-wider">
                            Current Total Balance
                        </div>
                    </div>
                    <div class="text-3xl font-bold tracking-tight text-white">
                        {move || total.get()}
                    </div>
                </div>
                <div class="text-slate-400 text-sm mt-1 font-medium">{move || sub_value.get()}</div>
            </div>

            <Show when=move || !balances.read().is_empty()>
                <div class="absolute top-full left-0 w-full pt-2 opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none group-hover:pointer-events-auto z-10">
                    <div class="bg-slate-950 border border-slate-700 rounded-2xl p-4 shadow-2xl max-h-60 overflow-y-auto no-scrollbar">
                        <div class="flex justify-between items-center mb-2 px-1 border-b border-slate-800 pb-2">
                            <h4 class="text-xs font-bold text-slate-400 uppercase tracking-wider">
                                "Address"
                            </h4>
                            <h4 class="text-xs pr-6 font-bold text-slate-400 uppercase tracking-wider">
                                "Balance"
                            </h4>
                        </div>
                        <ul class="space-y-2">
                            <For each=move || balances.get() key=|(addr, _)| addr.clone() let:child>
                                <li prop:key=child.0 class="text-xs font-mono">
                                    <a
                                        href=format!("{}?a={}", base_url.read(), child.0)
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        class="flex items-center justify-between text-slate-400 hover:text-white group/link p-1 rounded-md hover:bg-slate-800/50"
                                    >
                                        <span>{shortened_address(&child.0)}</span>
                                        <div class="flex items-center gap-2">
                                            <span class="text-emerald-400 font-sans font-bold">
                                                {truncated_balance_str(child.1)}
                                            </span>
                                            <IconArrowUpRight class="h-4 w-4 opacity-0 group-hover/link:opacity-100 transition-opacity" />
                                        </div>
                                    </a>
                                </li>
                            </For>
                        </ul>
                    </div>
                </div>
            </Show>
        </div>
    }
}
