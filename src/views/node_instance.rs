use super::{
    chart::{ChartSeriesData, node_metrics_update},
    helpers::{node_logs_stream, show_alert_msg, truncated_balance_str},
    icons::{
        IconChevronDown, IconRecycle, IconRemove, IconShowChart, IconShowLogs, IconStartNode,
        IconStopNode, IconUpgradeNode,
    },
    node_actions::NodeAction,
};
use crate::{
    app::ClientGlobalState,
    types::{InactiveReason, NodeId, NodeInstanceInfo, NodeStatus},
};

use alloy_primitives::utils::format_units;
use chrono::{DateTime, Local, Utc};
use leptos::{logging, prelude::*, task::spawn_local};

// Number of elapsed seconds to warn the user of an active node with 0 connected peers
const WARN_ZERO_CONN_PEERS_SECS: i64 = 120;

// Helper which converts a value to string or a dash sign if it's None
fn value_or_dash<T: ToString>(val: Option<T>) -> String {
    val.map_or("-".to_string(), |v| v.to_string())
}

#[component]
pub(super) fn NodeInstanceView(
    info: RwSignal<NodeInstanceInfo>,
    set_logs: WriteSignal<Vec<String>>,
    set_render_chart: RwSignal<bool>,
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selected = move || {
        info.read().is_status_locked
            || context
                .selecting_nodes
                .read()
                .1
                .contains(&info.read_untracked().node_id)
    };
    let node_card_clicked = move |always: bool| {
        let (is_selecting, _) = *context.selecting_nodes.read_untracked();
        if (always || is_selecting) && !info.read_untracked().is_status_locked {
            let node_id = &info.read_untracked().node_id;
            context.selecting_nodes.update(|(is_selecting, selected)| {
                if selected.contains(node_id) {
                    selected.remove(node_id);
                } else {
                    selected.insert(node_id.clone());
                }
                *is_selecting = !selected.is_empty();
            });
        }
    };

    let is_transitioning = move || info.read().status.is_transitioning();
    let is_locked = move || info.read().is_status_locked;
    let is_warn_node_status = move || {
        info.read().is_status_unknown
            || matches!(
                info.read().status,
                NodeStatus::Inactive(
                    InactiveReason::Unknown
                        | InactiveReason::Exited(_)
                        | InactiveReason::StartFailed(_)
                )
            )
    };

    let is_expanded = move || {
        context
            .expanded_nodes
            .read()
            .contains(&info.read_untracked().node_id)
    };

    // warn the user when there is no connected peers for more than 2 minutes
    let innactivity_started = RwSignal::new(0i64);
    let reachability_check_running = move || {
        info.get()
            .reachability
            .map(|r| r.in_progress())
            .unwrap_or(false)
    };
    let warn_conn_peers = move || {
        if info.read().status.is_active() && !reachability_check_running() {
            if matches!(info.read().connected_peers, Some(0)) {
                if innactivity_started.get_untracked() == 0 {
                    innactivity_started.set(Utc::now().timestamp());
                } else {
                    let elapsed_secs = Utc::now().timestamp() - innactivity_started.get_untracked();
                    if elapsed_secs > WARN_ZERO_CONN_PEERS_SECS {
                        return true;
                    }
                }
            } else {
                innactivity_started.set(0i64);
            }
        }

        false
    };

    view! {
        <Show
            when=move || context.tile_mode.get()
            fallback=move || {
                view! {
                    <div
                        on:click=move |_| node_card_clicked(!is_transitioning())
                        prop:id=info.read_untracked().short_node_id()
                        class=move || {
                            format!(
                                "{} border rounded-2xl transition-all duration-300 hover:shadow-2xl hover:shadow-indigo-500/5 backdrop-blur-sm flex flex-col {} {}",
                                if info.read().status.is_active() {
                                    "bg-slate-900/70"
                                } else {
                                    "bg-slate-950/70"
                                },
                                if is_selected() {
                                    "border-indigo-500/50 ring-2 ring-indigo-500/20"
                                } else {
                                    "border-slate-800"
                                },
                                if is_transitioning() { "opacity-60" } else { "" },
                            )
                        }
                    >
                        <div class="grid grid-cols-1 md:grid-cols-15 gap-x-4 gap-y-2 items-center md:px-6 cursor-pointer">
                            <div class="md:col-span-1 flex items-center gap-4">
                                <NodeSelection info />
                                <Show when=move || is_transitioning()>
                                    <div class="w-5 h-5 border-2 border-slate-500 rounded-full animate-spin border-t-transparent" />
                                </Show>
                            </div>
                            <div class="md:col-span-2 flex items-center gap-4">
                                <div class="font-mono text-sm text-white">
                                    {info.read_untracked().short_node_id().to_string()}
                                </div>
                            </div>
                            <div class="md:col-span-5 flex items-center justify-between md:justify-start gap-4">
                                <span class="md:hidden text-xs font-bold text-slate-500 uppercase w-20">
                                    Status
                                </span>
                                <span class=move || {
                                    info.read().status.status_color()
                                }>
                                    {move || info.read().status_summary()}
                                    {move || {
                                        if is_transitioning() {
                                            " ...".to_string()
                                        } else if info.get().status_info.is_empty() {
                                            "".to_string()
                                        } else {
                                            format!(", {}", info.read().status_info)
                                        }
                                    }}
                                </span>
                            </div>
                            <div class="md:col-span-1 flex items-center justify-between md:justify-center gap-4">
                                <span class="md:hidden text-xs font-bold text-slate-500 uppercase w-20">
                                    CPU
                                </span>
                                <div>
                                    {move || value_or_dash(
                                        info.get().cpu_usage.map(|v| format!("{v:.2}%")),
                                    )}
                                </div>
                            </div>
                            <div class="md:col-span-2 flex items-center justify-between md:justify-center gap-4">
                                <span class="md:hidden text-xs font-bold text-slate-500 uppercase w-20">
                                    Memory
                                </span>
                                <div>
                                    {move || value_or_dash(
                                        info.read().mem_used.map(|v| format!("{v:.2} MB")),
                                    )}
                                </div>
                            </div>
                            <div class="md:col-span-1 flex items-center justify-between md:justify-center gap-4">
                                <span class="md:hidden text-xs font-bold text-slate-500 uppercase w-20">
                                    Records
                                </span>
                                <span class="font-mono text-white">
                                    {move || value_or_dash(info.read().records)}
                                </span>
                            </div>
                            <div class="md:col-span-1 flex items-center justify-between md:justify-center gap-4">
                                <span class="md:hidden text-xs font-bold text-slate-500 uppercase w-20">
                                    Peers
                                </span>
                                <span class=move || {
                                    if warn_conn_peers() {
                                        "text-rose-500"
                                    } else {
                                        "font-mono text-cyan-400"
                                    }
                                }>{move || value_or_dash(info.read().connected_peers)}</span>
                            </div>
                            <div
                                class="md:col-span-2 flex flex-wrap items-center justify-center gap-1 text-slate-400"
                                on:click=move |e| e.stop_propagation()
                            >
                                <NodeLogs info set_logs />
                                <NodeChartShow info set_render_chart set_chart_data />
                                <button
                                    on:click=move |_| {
                                        if is_expanded() {
                                            context
                                                .expanded_nodes
                                                .update(|expanded| {
                                                    expanded.remove(&info.read_untracked().node_id);
                                                });
                                        } else {
                                            context
                                                .expanded_nodes
                                                .update(|expanded| {
                                                    expanded.insert(info.get_untracked().node_id);
                                                });
                                        }
                                    }
                                    class="p-2 hover:bg-slate-800 rounded-lg transition-colors"
                                    title=move || {
                                        if is_expanded() { "Show Less" } else { "Show More" }
                                    }
                                >
                                    <IconChevronDown is_down=Signal::derive(is_expanded) />
                                </button>
                            </div>
                        </div>
                        <div class=move || {
                            format!(
                                "transition-all duration-500 ease-in-out overflow-hidden {}",
                                if is_expanded() {
                                    "max-h-[600px] opacity-100"
                                } else {
                                    "max-h-0 opacity-0"
                                },
                            )
                        }>
                            <div class="p-6 pt-4 border-t border-slate-800">
                                <ExpandedNodeDetails info />
                            </div>
                        </div>
                    </div>
                }
            }
        >
            <div
                on:click=move |_| node_card_clicked(false)
                prop:id=info.read_untracked().short_node_id()
                class=move || {
                    format!(
                        "{} border rounded-2xl p-3 transition-all duration-300 hover:shadow-2xl hover:shadow-indigo-500/5 backdrop-blur-sm flex flex-col {} {}",
                        if info.read().status.is_active() {
                            "bg-slate-900/70"
                        } else {
                            "bg-slate-950/70"
                        },
                        if is_selected() {
                            "border-indigo-500/50 ring-2 ring-indigo-500/20"
                        } else {
                            "border-slate-800"
                        },
                        if is_transitioning() { "opacity-60" } else { "" },
                    )
                }
            >
                // Card Header
                <div class="flex items-start justify-between">
                    <div class="flex items-center gap-3">
                        <NodeSelection info />
                    </div>
                    <Show when=move || is_transitioning()>
                        <div>
                            <span class="loading loading-spinner absolute left-4"></span>
                        </div>
                    </Show>
                    <div class="flex items-center gap-0 text-slate-400">
                        <NodeLogs info set_logs />
                        <NodeChartShow info set_render_chart set_chart_data />
                        <Show when=move || info.read().upgradeable()>
                            <ButtonUpgrade info />
                        </Show>
                        <ButtonStopStart info />
                        <ButtonRecycle info />
                        <ButtonRemove info />
                    </div>
                </div>

                // Collapsed Summary
                <div class="mt-2 grid grid-cols-2 gap-x-4 gap-y-1">
                    <DetailItemView
                        label="Status"
                        full_width=true
                        children_class=Signal::derive(move || info.read().status.status_color())
                    >
                        <span class=move || {
                            if is_locked() {
                                "node-info-item-highlight"
                            } else if is_warn_node_status() {
                                "node-info-item-warn"
                            } else {
                                ""
                            }
                        }>{move || info.get().status_summary()}</span>
                        {move || {
                            if is_transitioning() {
                                " ...".to_string()
                            } else if info.get().status_info.is_empty() {
                                "".to_string()
                            } else {
                                format!(", {}", info.read().status_info)
                            }
                        }}
                    </DetailItemView>
                    <DetailItemView label="Node ID" full_width=true>
                        {info.read_untracked().short_node_id()}
                    </DetailItemView>
                    <DetailItemView label="CPU">
                        {move || value_or_dash(info.get().cpu_usage.map(|v| format!("{v:.2}%")))}
                    </DetailItemView>
                    <DetailItemView label="Memory Used">
                        {move || value_or_dash(info.read().mem_used.map(|v| format!("{v:.2} MB")))}
                    </DetailItemView>
                    <DetailItemView label="Records">
                        {move || value_or_dash(info.read().records)}
                    </DetailItemView>

                    <DetailItemView
                        label="Connected Peers"
                        children_class=Signal::derive(move || {
                            if warn_conn_peers() { "text-rose-500" } else { "text-cyan-400" }
                        })
                    >
                        {move || value_or_dash(info.read().connected_peers)}
                    </DetailItemView>
                    <DetailItemView
                        label="Network size"
                        children_class=Signal::stored("text-cyan-400")
                    >
                        {move || { value_or_dash(info.read().net_size) }}
                    </DetailItemView>

                </div>

                // Expanded Details
                <div class=move || {
                    format!(
                        "transition-all duration-500 ease-in-out overflow-hidden {}",
                        if is_expanded() {
                            "max-h-[600px] opacity-100 pt-3 mt-3 border-t border-slate-800"
                        } else {
                            "max-h-0 opacity-0"
                        },
                    )
                }>
                    <ExpandedNodeDetails info />
                </div>

                // Card Footer with Toggle
                <div class="mt-auto pt-2 border-t border-slate-800/50 flex justify-center">
                    <button
                        on:click=move |_| {
                            if is_expanded() {
                                context
                                    .expanded_nodes
                                    .update(|expanded| {
                                        expanded.remove(&info.read_untracked().node_id);
                                    });
                            } else {
                                context
                                    .expanded_nodes
                                    .update(|expanded| {
                                        expanded.insert(info.get_untracked().node_id);
                                    });
                            }
                        }
                        class="w-full flex items-center justify-center gap-2 text-xs text-slate-500 hover:text-indigo-400 font-semibold transition-colors py-1"
                    >
                        <span>{move || if is_expanded() { "Show Less" } else { "Show More" }}</span>
                        <IconChevronDown is_down=Signal::derive(is_expanded) />
                    </button>
                </div>
            </div>
        </Show>
    }
}

#[component]
pub(super) fn DetailItemView(
    label: &'static str,
    #[prop(default = Signal::stored("text-slate-300"))] children_class: Signal<&'static str>,
    #[prop(default = false)] full_width: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <div class=if full_width { "col-span-full" } else { "" }>
            <span class="text-xs font-semibold text-slate-500 uppercase tracking-wider">
                {label}
            </span>
            <div class=move || {
                format!("text-sm font-mono mt-0.5 break-words {}", children_class.get())
            }>{children()}</div>
        </div>
    }
}

#[component]
fn ExpandedNodeDetails(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let custom_data_dir = move || {
        info.get_untracked()
            .data_dir_path
            .map(|p| p.display().to_string())
    };
    let peer_id = move || value_or_dash(info.read().short_peer_id());

    let display_if_active = move |v| {
        if info.read().status.is_active() {
            value_or_dash(v)
        } else {
            " -".to_string()
        }
    };

    let reachability_check_running = move || {
        info.get()
            .reachability
            .map(|r| r.in_progress())
            .unwrap_or(false)
    };

    view! {
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-x-4 gap-y-0">
            <DetailItemView label="Peer ID" full_width=true>
                {move || {
                    if info.read().status.is_recycling() {
                        view! {
                            <span class="bg-indigo-100 text-indigo-800 text-xs font-medium me-2 px-2.5 py-0.5 rounded dark:bg-indigo-900 dark:text-indigo-300">
                                "...regenerating peer-id..."
                            </span>
                        }
                            .into_any()
                    } else {
                        peer_id().into_any()
                    }
                }}
            </DetailItemView>
            <DetailItemView label="PID">
                {move || display_if_active(info.read().pid)}
            </DetailItemView>
            <DetailItemView label="Version">
                {move || value_or_dash(info.get().bin_version)}
            </DetailItemView>
            <DetailItemView label="Balance" children_class=Signal::stored("text-emerald-400")>
                <div class="relative group">
                    <span class="cursor-help">
                        {move || { value_or_dash(info.read().balance.map(truncated_balance_str)) }}
                    </span>
                    <div class="absolute bottom-full mb-2 left-1/2 -translate-x-1/3 w-max max-w-xs bg-slate-950 text-emerald-400 text-xs font-mono break-all text-center px-3 py-1.5 rounded-lg border border-slate-700 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 shadow-lg">
                        {move || {
                            info.read()
                                .balance
                                .map_or(
                                    "".to_string(),
                                    |v| format_units(v, "ether").unwrap_or_default(),
                                )
                        }}
                    </div>
                </div>
            </DetailItemView>
            <DetailItemView label="kBuckets Peers">
                {move || value_or_dash(info.read().kbuckets_peers)}
            </DetailItemView>
            <DetailItemView label="Shunned By" children_class=Signal::stored("text-amber-400")>
                {move || value_or_dash(info.read().shunned_count)}
            </DetailItemView>
            <DetailItemView label="Relevant Records">
                {move || value_or_dash(info.read().relevant_records)}
            </DetailItemView>

            <div class="col-span-full">
                <span class="text-xs font-semibold text-slate-500 uppercase tracking-wider">
                    "Listen IPs"
                </span>
                <div class="text-sm font-mono mt-0.5 text-slate-300 relative flex overflow-x-hidden">
                    <div class=move || {
                        if info.read().node_ip.is_none_or(|ip| ip.to_string().len() < 15) {
                            ""
                        } else {
                            "flex whitespace-nowrap animate-slide"
                        }
                    }>{move || value_or_dash(info.get().node_ip)}</div>
                </div>
            </div>

            <DetailItemView label="Port">
                {value_or_dash(info.read_untracked().port)}
            </DetailItemView>
            <DetailItemView label="Metrics Port">
                {value_or_dash(info.read_untracked().metrics_port)}
            </DetailItemView>
            <DetailItemView label="UPnP">
                {if info.read_untracked().upnp { "On" } else { "Off" }}
            </DetailItemView>
            <DetailItemView label="Reachability Check">
                <Show
                    when=move || { info.read().reachability.is_some() }
                    fallback=move || {
                        view! {
                            {move || { if info.read().reachability_check { "On" } else { "Off" } }}
                        }
                    }
                >
                    <span class=move || {
                        if reachability_check_running() { "node-info-item-info" } else { "" }
                    }>{move || { value_or_dash(info.read().reachability.clone()) }}</span>
                </Show>
            </DetailItemView>

            <div class="col-span-full">
                <span class="text-xs font-semibold text-slate-500 uppercase tracking-wider">
                    "Host IPs"
                </span>
                <div class="text-sm font-mono mt-0.5 text-slate-300 relative flex overflow-x-hidden">
                    <div class="flex whitespace-nowrap animate-slide">
                        {move || value_or_dash(info.get().ips)}
                    </div>
                </div>
            </div>

            <DetailItemView label="Rewards Address" full_width=true>
                {value_or_dash(info.read_untracked().short_rewards_addr())}
            </DetailItemView>
            <Show when=move || { !custom_data_dir().is_none_or(|p| p.is_empty()) }>
                <div class="col-span-full">
                    <span class="text-xs font-semibold text-slate-500 uppercase tracking-wider">
                        "Custom Directory"
                    </span>
                    <div class="text-sm font-mono mt-0.5 text-slate-300 relative flex overflow-x-hidden">
                        <div class=move || {
                            if custom_data_dir().is_none_or(|p| p.len() < 20) {
                                ""
                            } else {
                                "flex whitespace-nowrap animate-slide"
                            }
                        }>{move || value_or_dash(custom_data_dir())}</div>
                    </div>
                </div>
            </Show>
            <DetailItemView label="Created" full_width=true>
                {DateTime::<Utc>::from_timestamp(info.read_untracked().created as i64, 0)
                    .unwrap_or_default()
                    .with_timezone(&Local)
                    .to_string()}
            </DetailItemView>
        </div>
    }
}

#[component]
fn NodeSelection(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selected = move || {
        info.read().is_status_locked
            || context
                .selecting_nodes
                .read()
                .1
                .contains(&info.read_untracked().node_id)
    };
    let select_node_clicked = move || {
        if !info.read_untracked().is_status_locked {
            let node_id = &info.read_untracked().node_id;
            context.selecting_nodes.update(|(is_selecting, selected)| {
                if selected.contains(node_id) {
                    selected.remove(node_id);
                } else {
                    selected.insert(node_id.clone());
                }
                *is_selecting = !selected.is_empty();
            });
        }
    };

    view! {
        <input
            type="checkbox"
            prop:checked=is_selected
            prop:disabled=move || info.read().is_status_locked
            prop:hidden=move || info.read().status.is_transitioning()
            on:click=move |event| event.stop_propagation()
            on:change=move |_| select_node_clicked()
            class="w-5 h-5 mt-0.5 rounded-md border-slate-700 bg-slate-800 text-indigo-600 focus:ring-indigo-500/20 shrink-0"
            aria-label=format!("Select node {}", info.read_untracked().short_node_id())
        />
    }
}

#[component]
fn NodeLogs(info: RwSignal<NodeInstanceInfo>, set_logs: WriteSignal<Vec<String>>) -> impl IntoView {
    // we use the context to switch on/off the streaming of logs
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;
    let is_btn_disabled = move || {
        !info.read_untracked().node_logs
            || is_selecting_nodes()
            || info.read().status.is_transitioning()
            || (info.read().status.is_inactive() && !cfg!(feature = "native"))
    };

    // action to trigger the streaming of logs from the node to the 'set_logs' signal
    let start_logs_stream = Action::new(move |id: &NodeId| {
        context.logs_stream_on_for.set(Some(info));
        let id = id.clone();
        async move {
            if let Err(err) = node_logs_stream(id.clone(), set_logs).await {
                let msg = format!("Failed to start logs stream for node {id}: {err:?}");
                logging::log!("{msg}");
                show_alert_msg(msg);
            }
        }
    });

    view! {
        <button
            prop:disabled=is_btn_disabled
            class="p-2 hover:bg-slate-800 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent"
            title="view logs"
            on:click=move |_| {
                set_logs.set(vec![]);
                start_logs_stream.dispatch(info.read_untracked().node_id.clone());
            }
        >
            <IconShowLogs />
        </button>
    }
}

#[component]
fn NodeChartShow(
    info: RwSignal<NodeInstanceInfo>,
    set_render_chart: RwSignal<bool>,
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> impl IntoView {
    // we use the context to switch on/off the update of metrics charts
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;
    let is_btn_disabled = move || {
        is_selecting_nodes()
            || info.read().status.is_transitioning()
            || info.read().status.is_inactive()
    };

    // action to trigger the update of nodes metrics charts
    let start_metrics_update = move |id: NodeId| {
        set_render_chart.set(true);
        context.metrics_update_on_for.set(Some(info));
        leptos::task::spawn_local(async move {
            if let Err(err) = node_metrics_update(id.clone(), set_chart_data).await {
                let msg = format!("Failed to start updating metrics chart for node {id}: {err:?}");
                logging::log!("{msg}");
                show_alert_msg(msg);
            }
        });
    };

    view! {
        <button
            prop:disabled=is_btn_disabled
            on:click=move |_| {
                start_metrics_update(info.read_untracked().node_id.clone());
            }
            class="p-2 hover:bg-slate-800 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent"
            title="mem & cpu"
        >
            <IconShowChart />
        </button>
    }
}

#[component]
fn ButtonStopStart(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;
    let is_btn_disabled = move || {
        is_selecting_nodes()
            || info.read().is_status_locked
            || info.read().status.is_transitioning()
    };
    let tip = move || {
        if info.read().status.is_inactive() {
            "start"
        } else {
            "stop"
        }
    };

    view! {
        <button
            title=tip
            prop:disabled=is_btn_disabled
            class=move || {
                format!(
                    "p-2 {} rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent",
                    if info.read().status.is_inactive() {
                        "hover:bg-emerald-500/10 text-emerald-500"
                    } else {
                        "hover:bg-rose-500/10 text-rose-700"
                    },
                )
            }
            on:click=move |_| spawn_local(async move {
                if info.read_untracked().status.is_inactive() {
                    NodeAction::Start.apply(&info, &context.stats).await;
                } else {
                    NodeAction::Stop.apply(&info, &context.stats).await;
                }
            })
        >
            <Show
                when=move || info.read().status.is_inactive()
                fallback=|| view! { <IconStopNode /> }
            >
                <IconStartNode />
            </Show>
        </button>
    }
}

#[component]
fn ButtonUpgrade(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;
    let is_btn_disabled = move || {
        is_selecting_nodes()
            || info.read().is_status_locked
            || info.read().status.is_transitioning()
    };
    let tip = move || {
        if let Some(v) = context.latest_bin_version.get() {
            format!("upgrade to v{v} and restart")
        } else {
            "upgrade and restart".to_string()
        }
    };

    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip=tip>
            <button
                type="button"
                prop:disabled=is_btn_disabled
                class="p-2 hover:bg-cyan-500/10 text-cyan-500 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent"
                on:click=move |_| spawn_local(async move {
                    NodeAction::Upgrade.apply(&info, &context.stats).await;
                })
            >
                <IconUpgradeNode />
            </button>
        </div>
    }
}

#[component]
fn ButtonRecycle(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;
    let is_btn_enabled = move || {
        !is_selecting_nodes()
            && !info.read().status.is_transitioning()
            && !info.read().is_status_locked
            && info.read().peer_id.is_some()
    };

    view! {
        <button
            prop:disabled=move || !is_btn_enabled()
            class="p-2 hover:bg-slate-800 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent"
            title="recycle"
            on:click=move |_| spawn_local(async move {
                NodeAction::Recycle.apply(&info, &context.stats).await;
            })
        >
            <IconRecycle />
        </button>
    }
}

#[component]
fn ButtonRemove(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    view! {
        <button
            prop:disabled=move || context.selecting_nodes.read().0
            class="p-2 hover:bg-rose-500/10 text-rose-700 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent"
            title="remove"
            on:click=move |_| spawn_local(async move {
                NodeAction::Remove.apply(&info, &context.stats).await;
            })
        >
            <IconRemove />
        </button>
    }
}
