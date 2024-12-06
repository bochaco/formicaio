use super::{
    app::{BatchInProgress, ClientGlobalState},
    chart_view::{node_metrics_update, ChartSeriesData, NodeChartView},
    helpers::{node_logs_stream, remove_node_instance, show_alert_msg},
    icons::{
        IconCloseModal, IconRecycle, IconRemove, IconShowChart, IconShowLogs, IconStartNode,
        IconStopNode, IconUpgradeNode,
    },
    node_instance::{NodeInstanceInfo, NodeStatus},
    server_api::{
        cancel_node_instances_batch, recycle_node_instance, start_node_instance,
        stop_node_instance, upgrade_node_instance,
    },
};

use chrono::{DateTime, Utc};
use leptos::{logging, prelude::*, task::spawn_local};

#[component]
pub fn NodesListView() -> impl IntoView {
    // we use the context to switch on/off the streaming of logs
    let context = expect_context::<ClientGlobalState>();
    // this signal keeps the reactive list of log entries
    let (logs, set_logs) = signal(Vec::new());
    let (chart_data, set_chart_data) = signal((vec![], vec![]));
    let (is_render_chart, set_render_chart) = signal(false);

    // we display the instances sorted by creation time, newest to oldest
    let sorted_nodes = Memo::new(move |_| {
        let mut sorted = context.nodes.get().into_iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| b.1.read().created.cmp(&a.1.read().created));
        sorted
    });

    view! {
        <div class="flex flex-wrap">
            <BatchInProgressView batch_info=context.batch_in_progress />

            <For
                each=move || sorted_nodes.get()
                key=|(container_id, _)| container_id.clone()
                let:child
            >
                <Show
                    when=move || !child.1.read().status.is_creating()
                    fallback=move || { view! { <CreatingNodeInstanceView /> }.into_view() }
                >
                    <NodeInstanceView info=child.1 set_logs set_render_chart set_chart_data />
                </Show>
            </For>
        </div>

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
                        <IconCloseModal />
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
                        <IconCloseModal />
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
fn BatchInProgressView(batch_info: RwSignal<Option<BatchInProgress>>) -> impl IntoView {
    let progress = move || {
        (batch_info.get().unwrap_or_default().created * 100)
            / batch_info.get().unwrap_or_default().total
    };

    view! {
        <Show when=move || batch_info.read().is_some() fallback=move || { view! {}.into_view() }>
            <div class="max-w-sm w-80 m-2 p-4 bg-white border border-gray-200 rounded-lg shadow dark:bg-gray-800 dark:border-gray-700">
                <div class="flex justify-end">
                    <div class="tooltip tooltip-bottom tooltip-info" data-tip="cancel">
                        <button
                            class="btn-node-action"
                            on:click=move |_| spawn_local({
                                batch_info.update(|info| *info = None);
                                async move {
                                    if let Err(err) = cancel_node_instances_batch().await {
                                        let msg = format!(
                                            "Failed to cancel nodes creation batch: {err:?}",
                                        );
                                        logging::log!("{msg}");
                                        show_alert_msg(msg);
                                    }
                                }
                            })
                        >
                            <IconRemove />
                        </button>
                    </div>
                </div>

                <h2 class="mb-2 text-lg font-semibold text-gray-900 dark:text-white">
                    Batch in progress:
                </h2>
                <ul class="max-w-md space-y-1 text-gray-500 list-disc list-inside dark:text-gray-400">
                    <li>
                        "Total number of nodes to create: "
                        {batch_info.get().unwrap_or_default().total}
                    </li>
                    <li>
                        "Delay between the creation of each node: "
                        {batch_info.get().unwrap_or_default().interval_secs} " secs."
                    </li>
                    <li>
                        "Auto-start nodes upon creation: "
                        {if batch_info.get().unwrap_or_default().auto_start { "Yes" } else { "No" }}
                    </li>
                </ul>

                <div class="mt-12">
                    <div class="flex justify-between mb-1">
                        <span class="text-base font-medium text-purple-700 dark:text-purple-500">
                            Nodes creation batch
                        </span>
                        <span class="text-sm font-medium text-purple-700 dark:text-purple-500">
                            {move || {
                                format!(
                                    "{}/{}",
                                    batch_info.get().unwrap_or_default().created,
                                    batch_info.get().unwrap_or_default().total,
                                )
                            }}

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
        </Show>
    }
}

#[component]
fn NodeInstanceView(
    info: RwSignal<NodeInstanceInfo>,
    set_logs: WriteSignal<Vec<String>>,
    set_render_chart: WriteSignal<bool>,
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> impl IntoView {
    let container_id = info.read_untracked().short_container_id();

    let spinner_msg = move || {
        let status = info.get().status;
        if status.is_transitioning() {
            format!("{status}")
        } else {
            "".to_string()
        }
    };

    let peer_id = move || {
        info.read()
            .short_peer_id()
            .unwrap_or_else(|| "unknown".to_string())
    };

    let rewards_addr = move || {
        info.read()
            .short_rewards_addr()
            .unwrap_or_else(|| "unknown".to_string())
    };

    view! {
        <div class="max-w-sm m-2 p-4 bg-gray-50 border border-gray-200 rounded-lg shadow dark:bg-gray-800 dark:border-gray-700">
            <div class="flex justify-end">
                <Show
                    when=move || info.read().status.is_transitioning()
                    fallback=move || view! { "" }.into_view()
                >
                    <div>
                        <span class="loading loading-spinner mr-2"></span>
                    </div>
                    <div class="mr-6">{spinner_msg}</div>
                </Show>

                <Show
                    when=move || info.read().upgradeable()
                    fallback=move || view! { "" }.into_view()
                >
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
                    <span class="node-info-item">"Node Id: "</span>
                    {container_id.clone()}
                </p>
                <p>
                    <span class="node-info-item">"Peer Id: "</span>
                    {move || {
                        if info.read().status.is_recycling() {
                            view! {
                                <span class="bg-indigo-100 text-indigo-800 text-xs font-medium me-2 px-2.5 py-0.5 rounded dark:bg-indigo-900 dark:text-indigo-300">
                                    "... generating new node peer-id ..."
                                </span>
                            }
                                .into_any()
                        } else {
                            peer_id().into_any()
                        }
                    }}
                </p>
                <p>
                    <span class="node-info-item">"Status: "</span>
                    {move || format!("{}, {}", info.read().status, info.read().status_info)}
                </p>
                <p>
                    <span class="node-info-item">"Version: "</span>
                    {move || info.get().bin_version.unwrap_or_else(|| "unknown".to_string())}
                </p>
                <p>
                    <div class="flex flex-row">
                        <div class="basis-1/2">
                            <span class="node-info-item">"Balance: "</span>
                            {move || {
                                info.read().balance.map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                        <div class="basis-1/2">
                            <span class="node-info-item">"Rewards: "</span>
                            {move || {
                                info.read().rewards.map_or("unknown".to_string(), |v| v.to_string())
                            }}
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
                            {move || {
                                info.read().port.map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                        <div class="basis-2/3">
                            <span class="node-info-item">"Node metrics Port: "</span>
                            {move || {
                                info.read()
                                    .metrics_port
                                    .map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                    </div>
                </p>
                <p>
                    <span class="node-info-item">"Store cost: "</span>
                    {move || {
                        info.read().store_cost.map_or("unknown".to_string(), |v| v.to_string())
                    }}
                </p>
                <p>
                    <div class="flex flex-row">
                        <div class="basis-1/2">
                            <span class="node-info-item">"Records: "</span>
                            {move || {
                                info.read().records.map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                        <div class="basis-1/2">
                            <span class="node-info-item">"Relevant: "</span>
                            {move || {
                                info.read()
                                    .relevant_records
                                    .map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                    </div>
                </p>
                <p>
                    <div class="flex flex-row">
                        <div class="basis-1/2">
                            <span class="node-info-item">"Conn. peers: "</span>
                            {move || {
                                info.read()
                                    .connected_peers
                                    .map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                        <div class="basis-1/2">
                            <span class="node-info-item">"Shunned by: "</span>
                            {move || {
                                info.read()
                                    .shunned_count
                                    .map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                    </div>
                </p>
                <p>
                    <span class="node-info-item">"kBuckets peers: "</span>
                    {move || {
                        info.read().kbuckets_peers.map_or("unknown".to_string(), |v| v.to_string())
                    }}
                </p>
                <p>
                    <div class="flex flex-row">
                        <div class="basis-2/3">
                            <span class="node-info-item">"Memory used: "</span>
                            {move || {
                                info.read().mem_used.map_or("".to_string(), |v| format!("{v} MB"))
                            }}
                        </div>
                        <div class="basis-1/3">
                            <span class="node-info-item">"CPU: "</span>
                            {move || {
                                info.get().cpu_usage.map_or("".to_string(), |v| format!("{v}%"))
                            }}
                        </div>
                    </div>
                </p>
                <p>
                    <span class="node-info-item">"Created: "</span>
                    {move || {
                        DateTime::<Utc>::from_timestamp(info.read().created as i64, 0)
                            .unwrap()
                            .to_string()
                    }}
                </p>
            </div>
        </div>
    }
}

#[component]
fn NodeLogs(info: RwSignal<NodeInstanceInfo>, set_logs: WriteSignal<Vec<String>>) -> impl IntoView {
    // we use the context to switch on/off the streaming of logs
    let context = expect_context::<ClientGlobalState>();

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
                    if info.read().status.is_transitioning() || info.read().status.is_inactive() {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| {
                    set_logs.set(vec![]);
                    start_logs_stream.dispatch(info.read_untracked().container_id.clone());
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
                    if info.read().status.is_transitioning() || info.read().status.is_inactive() {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| {
                    start_metrics_update(info.read_untracked().container_id.clone());
                }
            >
                <IconShowChart />
            </label>
        </div>
    }
}

#[component]
fn ButtonStopStart(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
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
                class=move || {
                    if info.read().status.is_transitioning() {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| {
                    let container_id = info.read_untracked().container_id.clone();
                    let previous_status = info.read_untracked().status.clone();
                    if previous_status.is_inactive() {
                        info.update(|node| node.status = NodeStatus::Restarting);
                        spawn_local(async move {
                            if let Err(err) = start_node_instance(container_id.clone()).await {
                                let msg = format!("Failed to start node {container_id}: {err:?}");
                                logging::log!("{msg}");
                                show_alert_msg(msg);
                                info.update(|node| node.status = previous_status);
                            }
                        });
                    } else {
                        info.update(|node| node.status = NodeStatus::Stopping);
                        spawn_local(async move {
                            match stop_node_instance(container_id.clone()).await {
                                Ok(()) => {
                                    info.update(|node| {
                                        node.connected_peers = Some(0);
                                        node.kbuckets_peers = Some(0);
                                    })
                                }
                                Err(err) => {
                                    let msg = format!(
                                        "Failed to stop node {container_id}: {err:?}",
                                    );
                                    logging::log!("{msg}");
                                    show_alert_msg(msg);
                                    info.update(|node| node.status = previous_status);
                                }
                            }
                        });
                    }
                }
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
                    if info.read().status.is_transitioning() {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| spawn_local({
                    let previous_status = info.read_untracked().status.clone();
                    info.update(|info| info.status = NodeStatus::Upgrading);
                    let container_id = info.read_untracked().container_id.clone();
                    async move {
                        match upgrade_node_instance(container_id.clone()).await {
                            Ok(()) => {
                                info.update(|node| {
                                    node.bin_version = None;
                                })
                            }
                            Err(err) => {
                                let msg = format!("Failed to upgrade node {container_id}: {err:?}");
                                logging::log!("{msg}");
                                show_alert_msg(msg);
                                info.update(|node| node.status = previous_status);
                            }
                        }
                    }
                })
            >
                <IconUpgradeNode />
            </button>
        </div>
    }
}

#[component]
fn ButtonRecycle(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="recycle">
            <button
                class=move || {
                    if info.read().status.is_active() {
                        "btn-node-action"
                    } else {
                        "btn-disabled-node-action"
                    }
                }
                on:click=move |_| spawn_local({
                    info.update(|info| info.status = NodeStatus::Recycling);
                    let container_id = info.read_untracked().container_id.clone();
                    async move {
                        if let Err(err) = recycle_node_instance(container_id.clone()).await {
                            let msg = format!("Failed to recycle node {container_id}: {err:?}");
                            logging::log!("{msg}");
                            show_alert_msg(msg);
                        }
                    }
                })
            >
                <IconRecycle />
            </button>
        </div>
    }
}

#[component]
fn ButtonRemove(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="remove">
            <button
                class="btn-node-action"
                on:click=move |_| spawn_local({
                    info.update(|info| info.status = NodeStatus::Removing);
                    let container_id = info.read_untracked().container_id.clone();
                    async move {
                        if let Err(err) = remove_node_instance(container_id.clone()).await {
                            let msg = format!("Failed to remove node {container_id}: {err:?}");
                            logging::log!("{msg}");
                            show_alert_msg(msg);
                        }
                    }
                })
            >
                <IconRemove />
            </button>
        </div>
    }
}
