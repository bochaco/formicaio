use super::{
    chart::{ChartSeriesData, node_metrics_update},
    helpers::{node_logs_stream, show_alert_msg, truncated_balance_str},
    icons::{
        IconRecycle, IconRemove, IconShowChart, IconShowLogs, IconStartNode, IconStopNode,
        IconUpgradeNode,
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

// Helper which converts a value to string or a dash sign if it's None
fn value_or_dash<T: ToString>(val: Option<T>) -> String {
    val.map_or(" -".to_string(), |v| v.to_string())
}

#[component]
pub(super) fn NodeInstanceView(
    info: RwSignal<NodeInstanceInfo>,
    set_logs: WriteSignal<Vec<String>>,
    set_render_chart: WriteSignal<bool>,
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
    let is_show_node_select = move || {
        let (is_selecting_nodes, _) = *context.selecting_nodes.read();
        is_selecting_nodes || info.read().is_status_locked
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

    let custom_data_dir = move || info.get().data_dir_path.map(|p| p.display().to_string());
    let peer_id = move || value_or_dash(info.read().short_peer_id());

    let rewards_addr = move || value_or_dash(info.read().short_rewards_addr());

    let display_if_active = move |v| {
        if info.read().status.is_active() {
            value_or_dash(v)
        } else {
            " -".to_string()
        }
    };

    let node_card_clicked = move || {
        let (is_selecting, _) = *context.selecting_nodes.read_untracked();
        if is_selecting && !info.read_untracked().is_status_locked {
            if is_selected() {
                context.selecting_nodes.update(|(_, selected)| {
                    selected.remove(&info.read_untracked().node_id);
                })
            } else {
                context.selecting_nodes.update(|(_, selected)| {
                    selected.insert(info.read_untracked().node_id.clone());
                })
            }
        }
    };

    // warn the user when there is no connected peers for more than 2 minutes
    let mut innactivity_started = 0i64;
    let mut warn_conn_peers = move || {
        if info.read().status.is_active() {
            if matches!(info.read().connected_peers, Some(0)) {
                if innactivity_started == 0 {
                    innactivity_started = Utc::now().timestamp();
                } else {
                    let elapsed_secs = Utc::now().timestamp() - innactivity_started;
                    if elapsed_secs > 120 {
                        return true;
                    }
                }
            } else {
                innactivity_started = 0i64;
            }
        }

        false
    };

    view! {
        <div
            on:click=move |_| node_card_clicked()
            class=move || match (is_selected(), info.read().status.is_active()) {
                (true, true) => "node-card-selected node-card-active",
                (true, false) => "node-card-selected node-card-inactive",
                (false, true) => "node-card node-card-active",
                (false, false) => "node-card node-card-inactive",
            }
        >

            <div class="flex justify-end">
                <Show when=move || is_show_node_select()>
                    <NodeSelection info />
                </Show>

                <Show when=move || is_transitioning()>
                    <div>
                        <span class="loading loading-spinner absolute left-4"></span>
                    </div>
                </Show>

                <Show when=move || info.read().upgradeable()>
                    <ButtonUpgrade info />
                </Show>

                <NodeLogs info set_logs />
                <NodeChartShow info set_render_chart set_chart_data />
                <ButtonStopStart info />
                <ButtonRecycle info />
                <ButtonRemove info />
            </div>
            <div class="mt-2">
                <p>
                    <span class="node-info-item">"Status: "</span>
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
                </p>
                <span class=move || { if is_transitioning() { "opacity-60" } else { "" } }>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-2/3">
                                <span class="node-info-item">"Node Id: "</span>
                                {info.read_untracked().short_node_id()}
                            </div>
                            <div class="basis-1/3">
                                <span class="node-info-item">"PID: "</span>
                                {move || display_if_active(info.read().pid)}
                            </div>
                        </div>
                    </p>
                    <p>
                        <span class="node-info-item">"Peer Id: "</span>
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
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-2/5">
                                <span class="node-info-item">"Version: "</span>
                                {move || value_or_dash(info.get().bin_version)}
                            </div>
                            <div class="basis-3/12">
                                <span class="node-info-item mr-2">"Listen IP:"</span>
                            </div>
                            <div class="basis-2/5 overflow-hidden relative">
                                <div class=move || {
                                    if info
                                        .read()
                                        .node_ip
                                        .is_none_or(|ip| ip.to_string().len() < 15)
                                    {
                                        ""
                                    } else {
                                        "absolute whitespace-nowrap animate-slide"
                                    }
                                }>{move || value_or_dash(info.get().node_ip)}</div>
                            </div>
                        </div>
                    </p>
                    <Show
                        when=move || { !custom_data_dir().is_none_or(|p| p.is_empty()) }
                        fallback=|| view! { "" }
                    >
                        <p>
                            <div class="flex flex-row">
                                <div class="basis-4/12">
                                    <span class="node-info-item">"Custom dir: "</span>
                                </div>
                                <div class="basis-7/12 overflow-hidden relative">
                                    <div class=move || {
                                        if custom_data_dir().is_none_or(|p| p.len() < 20) {
                                            ""
                                        } else {
                                            "absolute whitespace-nowrap animate-slide"
                                        }
                                    }>{move || value_or_dash(custom_data_dir())}</div>
                                </div>
                            </div>
                        </p>
                    </Show>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-1/2">
                                <span class="node-info-item">"Balance: "</span>
                                <div
                                    class="tooltip tooltip-bottom tooltip-info"
                                    data-tip=move || {
                                        info.read()
                                            .balance
                                            .map_or(
                                                "".to_string(),
                                                |v| format_units(v, "ether").unwrap_or_default(),
                                            )
                                    }
                                >
                                    <span class="underline decoration-dotted">
                                        {move || {
                                            value_or_dash(
                                                info.read().balance.map(truncated_balance_str),
                                            )
                                        }}
                                    </span>
                                </div>
                            </div>
                            <div class="basis-1/2">
                                <span class="node-info-item">"Rewards: "</span>
                                <div
                                    class="tooltip tooltip-bottom tooltip-info"
                                    data-tip=move || {
                                        info.read()
                                            .rewards
                                            .map_or(
                                                "".to_string(),
                                                |v| format_units(v, "ether").unwrap_or_default(),
                                            )
                                    }
                                >
                                    <span class="underline decoration-dotted">
                                        {move || {
                                            value_or_dash(
                                                info.read().rewards.map(truncated_balance_str),
                                            )
                                        }}
                                    </span>
                                </div>
                            </div>
                        </div>
                    </p>
                    <p>
                        <span class="node-info-item">"Rewards addr: "</span>
                        {move || rewards_addr}
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-1/3">
                                <span class="node-info-item">"Port: "</span>
                                {move || { value_or_dash(info.read().port) }}
                            </div>
                            <div class="basis-2/3">
                                <span class="node-info-item">"Node metrics Port: "</span>
                                {move || { value_or_dash(info.read().metrics_port) }}
                            </div>
                        </div>
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-1/2">
                                <span class="node-info-item">"Records: "</span>
                                {move || { value_or_dash(info.read().records) }}
                            </div>
                            <div class="basis-1/2">
                                <span class="node-info-item">"Relevant: "</span>
                                {move || { value_or_dash(info.read().relevant_records) }}
                            </div>
                        </div>
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-1/2">
                                <span class=move || {
                                    if warn_conn_peers() {
                                        "node-info-item-warn"
                                    } else {
                                        "node-info-item"
                                    }
                                }>"Conn. peers: "</span>
                                <span class=move || {
                                    if warn_conn_peers() { "node-info-item-warn" } else { "" }
                                }>{move || { value_or_dash(info.read().connected_peers) }}</span>
                            </div>
                            <div class="basis-1/2">
                                <span class="node-info-item">"Shunned by: "</span>
                                {move || { value_or_dash(info.read().shunned_count) }}
                            </div>
                        </div>
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-1/2">
                                <span class="node-info-item">"kBuckets peers: "</span>
                                {move || { value_or_dash(info.read().kbuckets_peers) }}
                            </div>
                            <div class="basis-1/2">
                                <span class="node-info-item">"Network size: "</span>
                                {move || { value_or_dash(info.read().net_size) }}
                            </div>
                        </div>
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-2/3">
                                <span class="node-info-item">"Memory used: "</span>
                                {move || {
                                    value_or_dash(
                                        info.read().mem_used.map(|v| format!("{v:.2} MB")),
                                    )
                                }}
                            </div>
                            <div class="basis-1/3">
                                <span class="node-info-item">"CPU: "</span>
                                {move || {
                                    value_or_dash(info.get().cpu_usage.map(|v| format!("{v:.2}%")))
                                }}
                            </div>
                        </div>
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-1/3">
                                <span class="node-info-item">"UPnP: "</span>
                                {move || if info.read().upnp { "On" } else { "Off" }}
                            </div>
                            <div class="basis-3/12">
                                <span class="node-info-item">"Host IPs:"</span>
                            </div>
                            <div class="basis-2/5 overflow-hidden relative">
                                <div class="absolute whitespace-nowrap animate-slide">
                                    {move || info.get().ips.unwrap_or_default()}
                                </div>
                            </div>
                        </div>
                    </p>
                    <p>
                        <span class="node-info-item">"Created: "</span>
                        {move || {
                            DateTime::<Utc>::from_timestamp(info.read().created as i64, 0)
                                .unwrap_or_default()
                                .with_timezone(&Local)
                                .to_string()
                        }}
                    </p>
                </span>
            </div>
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

    view! {
        <div>
            <span class="absolute left-4">
                <input
                    type="checkbox"
                    prop:checked=is_selected
                    prop:disabled=move || info.read().is_status_locked
                    class=move || {
                        if info.read().status.is_transitioning() {
                            "hidden"
                        } else {
                            "w-5 h-5 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
                        }
                    }
                />
            </span>
        </div>
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
        context.logs_stream_on_for.set(Some(id.clone()));
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
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="view logs">
            <button
                prop:disabled=is_btn_disabled
                class=move || {
                    if is_btn_disabled() { "btn-disabled-node-action" } else { "btn-node-action" }
                }
                on:click=move |_| {
                    set_logs.set(vec![]);
                    start_logs_stream.dispatch(info.read_untracked().node_id.clone());
                }
            >
                <IconShowLogs />
            </button>
        </div>
    }
}

#[component]
fn NodeChartShow(
    info: RwSignal<NodeInstanceInfo>,
    set_render_chart: WriteSignal<bool>,
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
        context.metrics_update_on_for.set(Some(id.clone()));
        leptos::task::spawn_local(async move {
            if let Err(err) = node_metrics_update(id.clone(), set_chart_data).await {
                let msg = format!("Failed to start updating metrics chart for node {id}: {err:?}");
                logging::log!("{msg}");
                show_alert_msg(msg);
            }
        });
    };

    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="mem & cpu">
            <button
                prop:disabled=is_btn_disabled
                class=move || {
                    if is_btn_disabled() { "btn-disabled-node-action" } else { "btn-node-action" }
                }
                on:click=move |_| {
                    start_metrics_update(info.read_untracked().node_id.clone());
                }
            >
                <IconShowChart />
            </button>
        </div>
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
        <div class="tooltip tooltip-bottom tooltip-info" data-tip=tip>
            <button
                prop:disabled=is_btn_disabled
                class=move || match (is_btn_disabled(), info.read().status.is_active()) {
                    (true, true) => "btn-disabled-node-action btn-node-action-active",
                    (true, false) => "btn-disabled-node-action btn-node-action-inactive",
                    (false, true) => "btn-node-action btn-node-action-active",
                    (false, false) => "btn-node-action btn-node-action-inactive",
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
        </div>
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
                class=move || {
                    if is_btn_disabled() { "btn-disabled-node-action" } else { "btn-node-action" }
                }
                on:click=move |_| spawn_local(async move {
                    NodeAction::Upgrade.apply(&info, &context.stats).await;
                })
            >
                <IconUpgradeNode color="green".to_string() />
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
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="recycle">
            <button
                prop:disabled=move || !is_btn_enabled()
                class=move || {
                    if is_btn_enabled() { "btn-node-action" } else { "btn-disabled-node-action" }
                }
                on:click=move |_| spawn_local(async move {
                    NodeAction::Recycle.apply(&info, &context.stats).await;
                })
            >
                <IconRecycle />
            </button>
        </div>
    }
}

#[component]
fn ButtonRemove(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="remove">
            <button
                prop:disabled=move || context.selecting_nodes.read().0
                class=move || {
                    if context.selecting_nodes.read().0 {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| spawn_local(async move {
                    NodeAction::Remove.apply(&info, &context.stats).await;
                })
            >

                <IconRemove />
            </button>
        </div>
    }
}
