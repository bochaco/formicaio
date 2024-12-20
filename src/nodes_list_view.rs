use super::{
    app::{BatchInProgress, ClientGlobalState},
    chart_view::{node_metrics_update, ChartSeriesData, NodeChartView},
    helpers::{node_logs_stream, show_alert_msg},
    icons::{
        IconCancel, IconRecycle, IconRemove, IconShowChart, IconShowLogs, IconStartNode,
        IconStopNode, IconUpgradeNode,
    },
    node_actions::NodeAction,
    node_instance::NodeInstanceInfo,
    server_api::cancel_node_instances_batch,
};

use chrono::{DateTime, Utc};
use leptos::{logging, prelude::*, task::spawn_local};

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
fn BatchInProgressView(batch_info: RwSignal<Option<BatchInProgress>>) -> impl IntoView {
    let progress = move || {
        (batch_info.get().unwrap_or_default().created * 100)
            / batch_info.get().unwrap_or_default().total
    };

    view! {
        <Show when=move || batch_info.read().is_some()>
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
                            <IconCancel />
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
    let context = expect_context::<ClientGlobalState>();
    let is_selected = move || {
        context
            .selecting_nodes
            .read()
            .2
            .contains(&info.read_untracked().container_id)
    };
    let is_selection_on = move || {
        let (is_selecting_nodes, is_selection_executing, _) = *context.selecting_nodes.read();
        is_selecting_nodes && (!is_selection_executing || is_selected())
    };

    let is_transitioning = move || info.read().status.is_transitioning();

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

    let node_card_clicked = move || {
        let (is_selecting, is_executing, _) = *context.selecting_nodes.read();
        if is_selecting && !is_executing {
            if is_selected() {
                context.selecting_nodes.update(|(_, _, selected)| {
                    selected.remove(&info.read_untracked().container_id);
                })
            } else {
                context.selecting_nodes.update(|(_, _, selected)| {
                    selected.insert(info.read_untracked().container_id.clone());
                })
            }
        }
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
                <Show when=move || is_selection_on()>
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
                    {move || {
                        if is_transitioning() {
                            format!("{} ...", info.read().status)
                        } else {
                            format!("{}, {}", info.read().status, info.read().status_info)
                        }
                    }}
                </p>
                <span class=move || { if is_transitioning() { "opacity-60" } else { "" } }>
                    <p>
                        <span class="node-info-item">"Node Id: "</span>
                        {info.read_untracked().short_container_id()}
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
                        {move || info.get().bin_version.unwrap_or_else(|| " -".to_string())}
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-1/2">
                                <span class="node-info-item">"Balance: "</span>
                                {move || {
                                    info.read().balance.map_or(" -".to_string(), |v| v.to_string())
                                }}
                            </div>
                            <div class="basis-1/2">
                                <span class="node-info-item">"Rewards: "</span>
                                {move || {
                                    info.read().rewards.map_or(" -".to_string(), |v| v.to_string())
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
                                    info.read().port.map_or(" -".to_string(), |v| v.to_string())
                                }}
                            </div>
                            <div class="basis-2/3">
                                <span class="node-info-item">"Node metrics Port: "</span>
                                {move || {
                                    info.read()
                                        .metrics_port
                                        .map_or(" -".to_string(), |v| v.to_string())
                                }}
                            </div>
                        </div>
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-1/2">
                                <span class="node-info-item">"Records: "</span>
                                {move || {
                                    info.read().records.map_or(" -".to_string(), |v| v.to_string())
                                }}
                            </div>
                            <div class="basis-1/2">
                                <span class="node-info-item">"Relevant: "</span>
                                {move || {
                                    info.read()
                                        .relevant_records
                                        .map_or(" -".to_string(), |v| v.to_string())
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
                                        .map_or(" -".to_string(), |v| v.to_string())
                                }}
                            </div>
                            <div class="basis-1/2">
                                <span class="node-info-item">"Shunned by: "</span>
                                {move || {
                                    info.read()
                                        .shunned_count
                                        .map_or(" -".to_string(), |v| v.to_string())
                                }}
                            </div>
                        </div>
                    </p>
                    <p>
                        <span class="node-info-item">"kBuckets peers: "</span>
                        {move || {
                            info.read().kbuckets_peers.map_or(" -".to_string(), |v| v.to_string())
                        }}
                    </p>
                    <p>
                        <div class="flex flex-row">
                            <div class="basis-2/3">
                                <span class="node-info-item">"Memory used: "</span>
                                {move || {
                                    info.read()
                                        .mem_used
                                        .map_or("".to_string(), |v| format!("{v} MB"))
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
                </span>
            </div>
        </div>
    }
}

#[component]
fn NodeSelection(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selection_executing = move || context.selecting_nodes.read().1;
    let is_selected = move || {
        context
            .selecting_nodes
            .read()
            .2
            .contains(&info.read_untracked().container_id)
    };

    view! {
        <div>
            <span class="absolute left-4">
                <input
                    type="checkbox"
                    prop:checked=move || is_selected()
                    prop:disabled=move || is_selection_executing()
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
                    if is_selecting_nodes() || info.read().status.is_transitioning()
                        || info.read().status.is_inactive()
                    {
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
                    is_selecting_nodes() || info.read().status.is_transitioning(),
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
                    if is_selecting_nodes() || info.read().status.is_transitioning() {
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
                        && info.read().peer_id.is_some()
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
