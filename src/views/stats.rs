use crate::app::ClientGlobalState;

use super::{
    helpers::truncated_balance_str,
    icons::{IconActivity, IconDisk, IconFile, IconPeers, IconServer, IconWallet},
};

use alloy_primitives::utils::format_units;
use leptos::prelude::*;

// Number of nodes to display as the top most connected nodes
const NUMBER_OF_TOP_NODES: usize = 10;

const GB_CONVERTION: f64 = 1_073_741_824.0;

#[component]
pub fn AggregatedStatsView() -> impl IntoView {
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
                <StatCard
                    title="Current Total Balance"
                    value=Signal::derive(move || truncated_balance_str(
                        context.stats.read().total_balance,
                    ))
                    sub_value=Signal::derive(move || {
                        format_units(context.stats.read().total_balance, "ether")
                            .unwrap_or_default()
                    })
                    icon=view! { <IconWallet class="text-emerald-400 w-8 h-8" /> }.into_any()
                />
                <StatCard
                    title="Total Connected Peers"
                    value=Signal::derive(move || context.stats.read().connected_peers.to_string())
                    sub_value=Signal::derive(move || {
                        format!("Shunned by {}", context.stats.read().shunned_count)
                    })
                    icon=view! { <IconPeers class="text-cyan-400 w-8 h-8" /> }.into_any()
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
                    title="Estimated Network Size"
                    value=Signal::derive(move || {
                        context.stats.read().estimated_net_size.to_string()
                    })
                    icon=view! { <IconActivity class="text-rose-400 w-7 h-7" /> }.into_any()
                />
                <DiskUsageCard
                    available=Signal::derive(move || context.stats.read().available_disk_space)
                    total=Signal::derive(move || context.stats.read().total_disk_space)
                />

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
                                    Observed Network Size
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
fn DiskUsageCard(available: Signal<u64>, total: Signal<u64>) -> impl IntoView {
    let used = move || {
        if total.get() > available.get() {
            total.get() - available.get()
        } else {
            0
        }
    };
    let percentage = move || {
        if total.get() > 0 {
            (used() as f64 * 100.0) / total.get() as f64
        } else {
            0f64
        }
    };

    let total_gb = move || total.get() as f64 / GB_CONVERTION;
    let free_gb = move || available.get() as f64 / GB_CONVERTION;
    let used_gb = move || used() as f64 / GB_CONVERTION;

    let colors = move || {
        let percentage = percentage();
        if percentage >= 90.0 {
            ("bg-rose-500", "text-rose-400")
        } else if percentage >= 75.0 {
            ("bg-amber-500", "text-amber-400")
        } else if percentage > 0.0 {
            ("bg-indigo-500", "text-indigo-400")
        } else {
            ("bg-slate-500", "text-slate-400")
        }
    };

    view! {
        <div class="bg-slate-900 border border-slate-800 p-6 rounded-2xl hover:border-indigo-500/50 transition-all duration-300 group shadow-lg flex flex-col justify-between min-h-[140px]">
            <div>
                <div class="flex items-center gap-4 mb-4">
                    <div class="p-2.5 bg-slate-800 rounded-xl group-hover:scale-110 transition-transform duration-300">
                        <IconDisk class="text-indigo-400 w-8 h-8" />
                    </div>
                    <div class="text-slate-400 text-xs font-bold uppercase tracking-wider">
                        Disk Usage
                    </div>
                </div>
                <div class="space-y-2">
                    <div class="flex justify-between items-baseline">
                        <span class="text-3xl font-bold tracking-tight text-white">
                            {move || format!("{:.2} GB", used_gb())}
                        </span>
                        <span class=move || {
                            format!("text-lg font-bold {}", colors().1)
                        }>{move || format!("{:.2}", percentage())}%</span>
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
                </div>
            </div>
            <div class="text-sm font-medium text-slate-400 mt-1">
                <span class="font-bold text-white">{move || format!("{:.2} GB", free_gb())}</span>
                {move || format!(" free of {:.2} GB", total_gb())}
            </div>
        </div>
    }
}
