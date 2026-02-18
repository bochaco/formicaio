use crate::{
    app::ClientGlobalState,
    types::{PeriodStats, shortened_address},
};

use super::{
    helpers::{human_readable_percent, truncated_balance_str},
    icons::{IconChevronDown, IconTrendingUp, IconWallet},
};

use alloy_primitives::U256;
use leptos::prelude::*;

#[component]
pub fn RewardsEarningsCard() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let selected_address = RwSignal::<Option<String>>::new(None);
    let balances = move || context.stats.read().balances.clone();
    let earnings_syncing = move || context.stats.read().earnings_syncing;
    Effect::new(move |_| {
        if let Some(addr) = selected_address.get() {
            let addr_upper = addr.to_uppercase();
            if !context
                .stats
                .read()
                .earnings
                .iter()
                .any(|(a, _)| a.to_uppercase() == addr_upper)
            {
                selected_address.set(None);
            }
        }
    });

    // Fetch earnings statistics based on selected address
    let earnings_stats = move || {
        let addr = selected_address.get().unwrap_or_default().to_uppercase();
        context
            .stats
            .read()
            .earnings
            .iter()
            .find(|(a, _)| a.to_uppercase() == addr)
            .cloned()
    };

    view! {
        <div class="bg-slate-900 border border-slate-800 rounded-3xl overflow-hidden shadow-2xl lg:col-span-3">
            <header class="p-6 border-b border-slate-800 bg-slate-800/10 backdrop-blur-md flex flex-col md:flex-row md:items-center justify-between gap-4">
                <div class="flex items-center gap-4">
                    <div class="p-3 bg-indigo-500/10 rounded-2xl text-indigo-400">
                        <IconTrendingUp />
                    </div>
                    <div>
                        <div class="flex items-center gap-3">
                            <h3 class="text-xl font-bold text-white tracking-tight">
                                Earnings Stats
                            </h3>
                            {move || {
                                earnings_syncing()
                                    .then(|| {
                                        view! {
                                            <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider bg-amber-500/10 text-amber-400 border border-amber-500/20 animate-pulse">
                                                <span class="w-1.5 h-1.5 bg-amber-400 rounded-full"></span>
                                                "Syncing"
                                            </span>
                                        }
                                    })
                            }}
                        </div>
                        <p class="text-xs text-slate-500 font-medium uppercase tracking-widest mt-1">
                            Rewards Performance Analytics
                        </p>
                    </div>
                </div>

                <div class="flex items-center gap-3">
                    <div class="relative group/select">
                        <IconWallet class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none group-focus-within/select:text-indigo-400 transition-colors" />
                        <select
                            prop:value=move || {
                                selected_address.get().unwrap_or("All Reward Addresses".to_string())
                            }
                            on:change=move |e| {
                                e.prevent_default();
                                let v = event_target_value(&e);
                                if v == "All Reward Addresses" {
                                    selected_address.set(None);
                                } else {
                                    selected_address.set(Some(v));
                                }
                            }
                            class="bg-slate-950 border border-slate-700 text-xs font-mono text-indigo-400 py-2.5 pl-9 pr-10 rounded-xl focus:outline-none focus:ring-2 focus:ring-indigo-500/50 appearance-none min-w-[240px] hover:border-slate-600 transition-colors shadow-inner"
                        >
                            <option prop:value="All Reward Addresses">All Reward Addresses</option>
                            <For each=move || balances() key=|(addr, _)| addr.clone() let:child>
                                <option prop:key=child.0.clone() value=child.0.clone()>
                                    {shortened_address(&child.0)}
                                </option>
                            </For>
                        </select>
                        <IconChevronDown
                            is_down=Signal::stored(true)
                            class="w-4 h-4 absolute right-3 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none"
                        />
                    </div>
                </div>
            </header>

            {move || match earnings_stats() {
                Some(stats) => {
                    view! {
                        <div class="p-6 grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-6 bg-slate-950/20">
                            <PeriodStatCard stats=stats.1.period_1 />
                            <PeriodStatCard stats=stats.1.period_2 />
                            <PeriodStatCard stats=stats.1.period_3 />
                            <PeriodStatCard stats=stats.1.period_4 />
                        </div>
                    }
                        .into_any()
                }
                None if balances().is_empty() => {
                    view! {
                        <div class="p-6 text-center text-slate-400">
                            "No addresses to retrieve earnings from"
                        </div>
                    }
                        .into_any()
                }
                None => {
                    view! {
                        <div class="p-6 text-center text-amber-400">
                            "Earnings history not fully retrieved yet for all addresses..."
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn PeriodStatCard(stats: PeriodStats) -> impl IntoView {
    let is_positive = move || {
        stats.change_percent.is_none() || matches!(stats.change_percent, Some(v) if v >= 0.0)
    };
    let sign = move || {
        if stats.change_percent.is_none() {
            ""
        } else if is_positive() {
            "+"
        } else {
            "-"
        }
    };

    view! {
        <div class="bg-slate-900/60 border border-slate-800/80 rounded-2xl p-6 hover:border-indigo-500/50 hover:bg-slate-800/40 transition-all duration-500 group shadow-lg flex flex-col">
            <div class="flex items-center justify-between mb-8">
                <h4 class="text-lg font-bold text-white tracking-tight group-hover:text-indigo-300 transition-colors">
                    {stats.label}
                </h4>
                <span class="text-[10px] font-bold text-slate-500 uppercase tracking-widest px-2 py-0.5 bg-slate-950 rounded border border-slate-800 group-hover:border-indigo-500/20 transition-colors">
                    {stats.length_hours}H
                </span>
            </div>

            <div class="space-y-4 flex-1">
                <div class="flex flex-col gap-1 group/row">
                    <span class="text-[11px] font-bold text-slate-500 uppercase tracking-widest">
                        Accumulated Rewards
                    </span>
                    <span class=move || {
                        format!(
                            "text-2xl font-bold tracking-tight transition-colors {}",
                            if is_positive() { "text-white" } else { "text-slate-200" },
                        )
                    }>{move || truncated_balance_str(U256::from(stats.total_earned))}</span>
                </div>

                <div class="flex flex-col gap-2 pb-4 border-b border-slate-800/60 group/row text-center sm:text-left">
                    <div class="flex justify-between items-center">
                        <span class="text-[11px] font-bold text-slate-500 uppercase tracking-widest">
                            vs Prior
                        </span>
                        <span class="text-[11px] font-bold text-slate-500/80 uppercase tracking-widest">
                            "Prior: "
                            <span class="text-slate-300 font-mono">
                                {move || truncated_balance_str(stats.total_earned_prev)}
                            </span>
                        </span>
                    </div>

                    <div class=move || {
                        format!(
                            "inline-flex items-center justify-center gap-1.5 px-3 py-2 rounded-xl text-sm font-bold border transition-all duration-300 w-full {}",
                            if is_positive() {
                                "bg-emerald-500/10 text-emerald-400 border-emerald-500/20 shadow-[0_0_12px_rgba(16,185,129,0.08)]"
                            } else {
                                "bg-rose-500/10 text-rose-400 border-rose-500/20 shadow-[0_0_12px_rgba(244,63,94,0.08)]"
                            },
                        )
                    }>
                        {move || {
                            view! {
                                <IconTrendingUp class=if is_positive() {
                                    "w-4 h-4"
                                } else {
                                    "w-4 h-4 rotate-45"
                                } />
                            }
                                .into_any()
                        }}
                        {move || {
                            format!(
                                "{} {}{}",
                                stats
                                    .change_percent
                                    .map_or_else(|| "".to_string(), human_readable_percent),
                                sign(),
                                truncated_balance_str(U256::from(stats.change_amount.abs())),
                            )
                        }}
                    </div>
                </div>

                <div class="space-y-3.5 pt-2">
                    <div class="flex justify-between items-center">
                        <span class="text-sm font-medium text-slate-400">Payments:</span>
                        <span class="text-sm font-bold text-slate-200">
                            {move || stats.num_payments}
                        </span>
                    </div>

                    <div class="flex justify-between items-center">
                        <span class="text-sm font-medium text-slate-400">Average amount:</span>
                        <span class="text-sm font-bold text-slate-200">
                            {move || truncated_balance_str(U256::from(stats.average_payment))}
                        </span>
                    </div>

                    <div class="flex justify-between items-center">
                        <span class="text-sm font-medium text-slate-400">Median amount:</span>
                        <span class="text-sm font-bold text-slate-200">
                            {move || truncated_balance_str(U256::from(stats.median_payment))}
                        </span>
                    </div>

                    <div class="flex justify-between items-center pt-1 mt-1">
                        <span class="text-sm font-medium text-slate-400">Largest amount:</span>
                        <span class="text-sm font-bold text-emerald-400 bg-emerald-500/5 px-2 py-0.5 rounded border border-emerald-500/10">
                            {move || truncated_balance_str(U256::from(stats.largest_payment))}
                        </span>
                    </div>
                </div>
            </div>
        </div>
    }
}
