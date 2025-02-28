use super::{
    app::ClientGlobalState,
    chart_view::{node_metrics_update, ChartSeriesData, NodeChartView},
    helpers::{node_logs_stream, show_alert_msg, truncated_balance_str},
    icons::{
        IconCancel, IconRecycle, IconRemove, IconShowChart, IconShowLogs, IconStartNode,
        IconStopNode, IconUpgradeNode,
    },
    node_actions::NodeAction,
    node_instance::NodeInstanceInfo,
    server_api::cancel_batch,
    server_api_types::{BatchType, NodesActionsBatch},
};

use alloy_primitives::utils::format_units;
use chrono::{DateTime, Local, Utc};
use leptos::{logging, prelude::*, task::spawn_local};

// Helper which converts a value to string or a dash sign if it's None
fn value_or_dash<T: ToString>(val: Option<T>) -> String {
    val.map_or(" -".to_string(), |v| v.to_string())
}

#[component]
pub fn NodesListView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    // this signal keeps the reactive list of log entries
    let (logs, set_logs) = signal(Vec::new());
    let (chart_data, set_chart_data) = signal((vec![], vec![]));
    let (is_render_chart, set_render_chart) = signal(false);

    // we display the instances sorted with the currently selected strategy
    let sorted_nodes = Memo::new(move |_| {
        let mut sorted = context.nodes.get().1.into_iter().collect::<Vec<_>>();
        context.nodes_sort_strategy.read().sort_items(&mut sorted);
        sorted
    });

    view! {
        <Show
            when=move || context.nodes.read().0
            fallback=move || {
                view! {
                    <div class="text-center mt-12">
                        <span class="loading loading-bars loading-lg">Loading...</span>
                    </div>
                }
            }
        >

            <div class="flex flex-wrap">
                <NodesActionsBatchesView />

                <For each=move || sorted_nodes.get() key=|(node_id, _)| node_id.clone() let:child>
                    <Show
                        when=move || !child.1.read().status.is_creating()
                        fallback=move || { view! { <CreatingNodeInstanceView /> }.into_view() }
                    >
                        <NodeInstanceView info=child.1 set_logs set_render_chart set_chart_data />
                    </Show>
                </For>
            </div>
        </Show>

        <input type="checkbox" id="logs_stream_modal" class="modal-toggle" />
        <div class="modal" role="dialog">
            <div class="modal-box border border-solid border-slate-50 max-w-full h-full overflow-hidden">
                <h3 class="text-sm font-bold">Node logs</h3>
                <div class="p-2.5 border-transparent overflow-y-auto h-full">
                    <ul>
                        <For
                            each=move || logs.get().into_iter().enumerate()
                            key=|(i, _)| *i
                            let:child
                        >
                            <li>{child.1}</li>
                        </For>
                    </ul>
                </div>

                <div class="modal-action">
                    <label
                        for="logs_stream_modal"
                        class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2"
                        on:click=move |_| context.logs_stream_on_for.set(None)
                    >
                        <IconCancel />
                    </label>
                </div>
            </div>
        </div>

        <input type="checkbox" id="node_chart_modal" class="modal-toggle" />
        <div class="modal" role="dialog">
            <div class="modal-box border border-solid border-slate-50 w-4/5 max-w-full h-3/5 max-h-full overflow-y-auto">
                <h3 class="text-sm font-bold">"Node Mem & CPU"</h3>
                <div class="border-transparent h-full">
                    <NodeChartView is_render_chart chart_data />
                </div>

                <div class="modal-action">
                    <label
                        for="node_chart_modal"
                        class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2"
                        on:click=move |_| {
                            set_render_chart.set(false);
                            context.metrics_update_on_for.set(None);
                        }
                    >
                        <IconCancel />
                    </label>
                </div>
            </div>
        </div>
    }
}

#[component]
fn CreatingNodeInstanceView() -> impl IntoView {
    view! {
        <div class="max-w-sm m-2 p-4 bg-white border border-gray-200 rounded-lg shadow dark:bg-gray-800 dark:border-gray-700">
            <div class="flex flex-col gap-4">
                <div class="skeleton h-16 w-full"></div>
                <div class="skeleton h-4 w-28"></div>
                <div class="skeleton h-4 w-56"></div>
                <div class="skeleton h-4 w-28"></div>
                <div class="skeleton h-4 w-20"></div>
                <div class="skeleton h-4 w-28"></div>
                <div class="skeleton h-4 w-40"></div>
                <div class="skeleton h-4 w-40"></div>
                <div class="skeleton h-4 w-56"></div>
            </div>
        </div>
    }
}

#[component]
fn NodesActionsBatchesView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    view! {
        <For each=move || context.scheduled_batches.get() key=|batch| batch.read().id let:child>
            <ActionBatchView batch_info=child />
        </For>
    }
}

#[component]
fn ActionBatchView(batch_info: RwSignal<NodesActionsBatch>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let batch_id = batch_info.get_untracked().id;
    let batch_type = batch_info.get_untracked().batch_type;
    let (count, batch_type_str, auto_start) = match &batch_type {
        BatchType::Create { count, node_opts } => (*count, "CREATE", node_opts.auto_start),
        BatchType::Start(l) => (l.len() as u16, "START", false),
        BatchType::Stop(l) => (l.len() as u16, "STOP", false),
        BatchType::Upgrade(l) => (l.len() as u16, "UPGRADE", false),
        BatchType::Recycle(l) => (l.len() as u16, "RECYCLE", false),
        BatchType::Remove(l) => (l.len() as u16, "REMOVE", false),
    };
    let progress = move || {
        if count > 0 {
            (batch_info.read().complete * 100) / count
        } else {
            0
        }
    };

    view! {
        <div class="max-w-sm w-80 m-2 p-4 bg-white border border-gray-200 rounded-lg shadow dark:bg-gray-800 dark:border-gray-700">
            <div class="flex justify-end">
                <div class="tooltip tooltip-bottom tooltip-info" data-tip="cancel">
                    <button
                        class="btn-node-action"
                        on:click=move |_| spawn_local({
                            context
                                .scheduled_batches
                                .update(|batches| {
                                    batches.retain(|b| { b.read_untracked().id != batch_id })
                                });
                            async move {
                                if let Err(err) = cancel_batch(batch_id).await {
                                    let msg = format!(
                                        "Failed to cancel node action batch: {err:?}",
                                    );
                                    logging::log!("{msg}");
                                    show_alert_msg(msg);
                                }
                            }
                        })
                    >
                        <IconCancel />
                    </button>
                </div>
            </div>

            <h2 class="mb-2 text-lg font-semibold text-gray-900 dark:text-white">
                "Batch " {move || batch_info.read().status.clone()} ":"
            </h2>
            <ul class="max-w-md space-y-1 text-gray-500 list-disc list-inside dark:text-gray-400">
                <li>"Total number of nodes to " {batch_type_str} ": " {count}</li>
                <li>
                    "Delay between each node action: " {batch_info.get_untracked().interval_secs}
                    " secs."
                </li>
                <Show when=move || matches!(batch_type, BatchType::Create { .. })>
                    <li>
                        "Auto-start nodes upon creation: " {if auto_start { "Yes" } else { "No" }}
                    </li>
                </Show>
            </ul>

            <div class="mt-12">
                <div class="flex justify-between mb-1">
                    <span class="text-base font-medium text-purple-700 dark:text-purple-500">
                        Nodes actions batch
                    </span>
                    <span class="text-sm font-medium text-purple-700 dark:text-purple-500">
                        {move || { format!("{}/{}", batch_info.read().complete, count) }}
                    </span>
                </div>
                <div class="w-full bg-purple-300 rounded-full h-6 dark:bg-gray-700">
                    <div
                        class="bg-purple-600 h-6 text-center text-purple-100 rounded-full dark:bg-purple-500"
                        style=move || format!("width: {}%", progress())
                    >
                        {move || progress()}
                        "%"
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn NodeInstanceView(
    info: RwSignal<NodeInstanceInfo>,
    set_logs: WriteSignal<Vec<String>>,
    set_render_chart: WriteSignal<bool>,
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selected = move || {
        info.read().status.is_locked()
            || context
                .selecting_nodes
                .read()
                .1
                .contains(&info.read_untracked().node_id)
    };
    let is_show_node_select = move || {
        let (is_selecting_nodes, _) = *context.selecting_nodes.read();
        is_selecting_nodes || info.read().status.is_locked()
    };

    let is_transitioning = move || info.read().status.is_transitioning();
    let is_locked = move || info.read().status.is_locked();

    let peer_id = move || {
        info.read()
            .short_peer_id()
            .unwrap_or_else(|| " -".to_string())
    };

    let rewards_addr = move || {
        info.read()
            .short_rewards_addr()
            .unwrap_or_else(|| " -".to_string())
    };

    let display_if_active = move |v| {
        if info.read().status.is_active() {
            value_or_dash(v)
        } else {
            " -".to_string()
        }
    };

    let node_card_clicked = move || {
        let (is_selecting, _) = *context.selecting_nodes.read_untracked();
        if is_selecting && !info.read_untracked().status.is_locked() {
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
                        if is_locked() { "node-info-item-highlight" } else { "" }
                    }>{move || info.get().status.to_string()}</span>
                    {move || {
                        if is_transitioning() {
                            " ...".to_string()
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
                        <span class="node-info-item">"Version: "</span>
                        {move || value_or_dash(info.get().bin_version)}
                    </p>
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
                            <div class="basis-2/3">
                                <span class="node-info-item">"Home-network: "</span>
                                {move || if info.read().home_network { "On" } else { "Off" }}
                            </div>
                            <div class="basis-1/3">
                                <span class="node-info-item">"UPnP: "</span>
                                {move || if info.read().upnp { "On" } else { "Off" }}
                            </div>
                        </div>
                    </p>
                    <Show when=move || !info.read().home_network fallback=|| view! { "" }>
                        <p>
                            <div class="flex flex-row">
                                <div class="basis-1/2">
                                    <span class="node-info-item">"Relay clients: "</span>
                                    {move || value_or_dash(info.get().connected_relay_clients)}
                                </div>
                                <div class="basis-1/12">
                                    <span class="node-info-item">"IPs: "</span>
                                </div>
                                <div class="basis-2/5 overflow-hidden relative">
                                    <div class="absolute whitespace-nowrap animate-slide">
                                        {move || info.get().ips.unwrap_or_default()}
                                    </div>
                                </div>
                            </div>
                        </p>
                    </Show>
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
        info.read().status.is_locked()
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
                    prop:disabled=move || info.read().status.is_locked()
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

    // action to trigger the streaming of logs from the node to the 'set_logs' signal
    let start_logs_stream = Action::new(move |id: &String| {
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
            <label
                for="logs_stream_modal"
                class=move || {
                    if !info.read_untracked().node_logs || is_selecting_nodes()
                        || info.read().status.is_transitioning() || info.read().status.is_inactive()
                    {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| {
                    set_logs.set(vec![]);
                    start_logs_stream.dispatch(info.read_untracked().node_id.clone());
                }
            >
                <IconShowLogs />
            </label>
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

    // action to trigger the update of nodes metrics charts
    let start_metrics_update = move |id: String| {
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
            <label
                for="node_chart_modal"
                class=move || {
                    if is_selecting_nodes() || info.read().status.is_transitioning()
                        || info.read().status.is_inactive()
                    {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| {
                    start_metrics_update(info.read_untracked().node_id.clone());
                }
            >
                <IconShowChart />
            </label>
        </div>
    }
}

#[component]
fn ButtonStopStart(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;
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
                class=move || match (
                    is_selecting_nodes() || info.read().status.is_locked()
                        || info.read().status.is_transitioning(),
                    info.read().status.is_active(),
                ) {
                    (true, true) => "btn-disabled-node-action btn-node-action-active",
                    (true, false) => "btn-disabled-node-action btn-node-action-inactive",
                    (false, true) => "btn-node-action btn-node-action-active",
                    (false, false) => "btn-node-action btn-node-action-inactive",
                }
                on:click=move |_| spawn_local(async move {
                    if info.read_untracked().status.is_inactive() {
                        NodeAction::Start.apply(&info).await;
                    } else {
                        NodeAction::Stop.apply(&info).await;
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
                class=move || {
                    if is_selecting_nodes() || info.read().status.is_locked()
                        || info.read().status.is_transitioning()
                    {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| spawn_local(async move {
                    NodeAction::Upgrade.apply(&info).await;
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

    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="recycle">
            <button
                class=move || {
                    if !is_selecting_nodes() && !info.read().status.is_transitioning()
                        && !info.read().status.is_locked() && info.read().peer_id.is_some()
                    {
                        "btn-node-action"
                    } else {
                        "btn-disabled-node-action"
                    }
                }
                on:click=move |_| spawn_local(async move {
                    NodeAction::Recycle.apply(&info).await;
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
                class=move || {
                    if context.selecting_nodes.read().0 {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| spawn_local(async move {
                    NodeAction::Remove.apply(&info).await;
                })
            >

                <IconRemove />
            </button>
        </div>
    }
}
