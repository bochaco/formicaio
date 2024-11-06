use super::{
    app::ClientGlobalState,
    chart_view::{ChartSeriesData, NodeChartView},
    helpers::{node_logs_stream, remove_node_instance, show_alert_msg},
    icons::{
        IconCloseModal, IconRemoveNode, IconShowChart, IconShowLogs, IconStartNode, IconStopNode,
        IconUpgradeNode,
    },
    node_instance::{NodeInstanceInfo, NodeStatus},
    server_api::{start_node_instance, stop_node_instance, upgrade_node_instance},
};

use chrono::{DateTime, Utc};
use leptos::*;

#[component]
pub fn NodesListView() -> impl IntoView {
    // we use the context to switch on/off the streaming of logs
    let context = expect_context::<ClientGlobalState>();
    // this signal keeps the reactive list of log entries
    let (logs, set_logs) = create_signal(Vec::new());
    let (chart_data, set_chart_data) = create_signal((vec![], vec![]));

    // we display the instances sorted by creation time, newest to oldest
    let sorted_nodes = create_memo(move |_| {
        let mut sorted = context.nodes.get().into_iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| b.1.get().created.cmp(&a.1.get().created));
        sorted
    });

    view! {
        <div class="flex flex-wrap">
            <For
                each=move || sorted_nodes.get()
                key=|(container_id, _)| container_id.clone()
                let:child
            >
                <Show
                    when=move || !child.1.get().status.is_creating()
                    fallback=move || { view! { <CreatingNodeInstanceView /> }.into_view() }
                >
                    <NodeInstanceView info=child.1 set_logs set_chart_data />
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
                    <NodeChartView chart_data />
                </div>

                <div class="modal-action">
                    <label
                        for="node_chart_modal"
                        class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2"
                        on:click=move |_| context.metrics_update_on_for.set(None)
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
fn NodeInstanceView(
    info: RwSignal<NodeInstanceInfo>,
    set_logs: WriteSignal<Vec<String>>,
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> impl IntoView {
    let container_id = info.get_untracked().short_container_id();

    let spinner_msg = move || {
        let status = info.get().status;
        if status.is_transitioning() {
            format!("{status}")
        } else {
            "".to_string()
        }
    };

    let peer_id = move || {
        info.get()
            .short_peer_id()
            .unwrap_or_else(|| "unknown".to_string())
    };

    let rewards_addr = move || {
        info.get()
            .short_rewards_addr()
            .unwrap_or_else(|| "unknown".to_string())
    };

    view! {
        <div class="max-w-sm m-2 p-4 bg-gray-50 border border-gray-200 rounded-lg shadow dark:bg-gray-800 dark:border-gray-700">
            <div class="flex justify-end">
                <Show
                    when=move || info.get().status.is_transitioning()
                    fallback=move || view! { "" }.into_view()
                >
                    <div>
                        <span class="loading loading-spinner mr-2"></span>
                    </div>
                    <div class="mr-6">{spinner_msg}</div>
                </Show>

                <Show
                    when=move || info.get().upgradeable()
                    fallback=move || view! { "" }.into_view()
                >
                    <ButtonUpgrade info />
                </Show>

                <NodeLogs info set_logs />
                <NodeChartShow info set_chart_data />
                <ButtonStopStart info />
                <ButtonRemove info />
            </div>
            <div class="mt-2">
                <p>
                    <span class="node-info-item">"Node Id: "</span>
                    {container_id.clone()}
                </p>
                <p>
                    <span class="node-info-item">"Peer Id: "</span>
                    {move || peer_id}
                </p>
                <p>
                    <span class="node-info-item">"Status: "</span>
                    {move || format!("{}, {}", info.get().status, info.get().status_info)}
                </p>
                <p>
                    <span class="node-info-item">"Version: "</span>
                    {move || info.get().bin_version.unwrap_or_else(|| "unknown".to_string())}
                </p>
                <p>
                    <span class="node-info-item">"Balance: "</span>
                    {move || info.get().balance.map_or("unknown".to_string(), |v| v.to_string())}
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
                                info.get().port.map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                        <div class="basis-2/3">
                            <span class="node-info-item">"RPC API Port: "</span>
                            {move || {
                                info.get()
                                    .rpc_api_port
                                    .map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                    </div>
                </p>
                <p>
                    <span class="node-info-item">"Node metrics Port: "</span>
                    {move || {
                        info.get().metrics_port.map_or("unknown".to_string(), |v| v.to_string())
                    }}
                </p>
                <p>
                    <span class="node-info-item">"Store cost: "</span>
                    {move || {
                        info.get().store_cost.map_or("unknown".to_string(), |v| v.to_string())
                    }}
                </p>
                <p>
                    <div class="flex flex-row">
                        <div class="basis-1/2">
                            <span class="node-info-item">"Records: "</span>
                            {move || {
                                info.get().records.map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                        <div class="basis-1/2">
                            <span class="node-info-item">"Relevant: "</span>
                            {move || {
                                info.get()
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
                                info.get()
                                    .connected_peers
                                    .map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                        <div class="basis-1/2">
                            <span class="node-info-item">"Shunned by: "</span>
                            {move || {
                                info.get()
                                    .shunned_count
                                    .map_or("unknown".to_string(), |v| v.to_string())
                            }}
                        </div>
                    </div>
                </p>
                <p>
                    <span class="node-info-item">"kBuckets peers: "</span>
                    {move || {
                        info.get().kbuckets_peers.map_or("unknown".to_string(), |v| v.to_string())
                    }}
                </p>
                <p>
                    <div class="flex flex-row">
                        <div class="basis-2/3">
                            <span class="node-info-item">"Memory used: "</span>
                            {move || {
                                info.get().mem_used.map_or("".to_string(), |v| format!("{v} MB"))
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
                        DateTime::<Utc>::from_timestamp(info.get().created as i64, 0)
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
    let start_logs_stream = create_action(move |id: &String| {
        context.logs_stream_on_for.set(Some(id.clone()));
        let id = id.clone();
        async move {
            if let Err(err) = node_logs_stream(id, set_logs).await {
                logging::log!("Failed to start logs stream: {err:?}");
                show_alert_msg(err.to_string());
            }
        }
    });

    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="view logs">
            <label
                for="logs_stream_modal"
                class=move || {
                    if info.get().status.is_transitioning() || info.get().status.is_inactive() {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| {
                    set_logs.set(vec![]);
                    start_logs_stream.dispatch(info.get_untracked().container_id.clone());
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
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> impl IntoView {
    // we use the context to switch on/off the update of metrics charts
    let context = expect_context::<ClientGlobalState>();

    // action to trigger the update of nodes metrics charts
    let start_metrics_update = create_action(move |id: &String| {
        context.metrics_update_on_for.set(Some(id.clone()));
        let id = id.clone();
        async move {
            if let Err(err) = super::chart_view::node_metrics_update(id, set_chart_data).await {
                logging::log!("Failed to start updating metrics charts: {err:?}");
                show_alert_msg(err.to_string());
            }
        }
    });

    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="mem & cpu">
            <label
                for="node_chart_modal"
                class=move || {
                    if info.get().status.is_transitioning() || info.get().status.is_inactive() {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| {
                    start_metrics_update.dispatch(info.get_untracked().container_id.clone());
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
        if info.get().status.is_inactive() {
            "start"
        } else {
            "stop"
        }
    };

    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip=tip>
            <button
                class=move || {
                    if info.get().status.is_transitioning() {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| {
                    let container_id = info.get().container_id.clone();
                    let previous_status = info.get().status;
                    if previous_status.is_inactive() {
                        info.update(|node| node.status = NodeStatus::Restarting);
                        spawn_local(async move {
                            match start_node_instance(container_id).await {
                                Ok(()) => {
                                    info.update(|node| {
                                        node.status = NodeStatus::Transitioned(
                                            "Restarted".to_string(),
                                        );
                                    })
                                }
                                Err(err) => {
                                    logging::log!("Failed to start node: {err:?}");
                                    show_alert_msg(err.to_string());
                                    info.update(|node| node.status = previous_status);
                                }
                            }
                        });
                    } else {
                        info.update(|node| node.status = NodeStatus::Stopping);
                        spawn_local(async move {
                            match stop_node_instance(container_id).await {
                                Ok(()) => {
                                    info.update(|node| {
                                        node.connected_peers = Some(0);
                                        node.kbuckets_peers = Some(0);
                                        node.status = NodeStatus::Transitioned(
                                            "Stopped".to_string(),
                                        );
                                    })
                                }
                                Err(err) => {
                                    logging::log!("Failed to stop node: {err:?}");
                                    show_alert_msg(err.to_string());
                                    info.update(|node| node.status = previous_status);
                                }
                            }
                        });
                    }
                }
            >
                <Show
                    when=move || info.get().status.is_inactive()
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
                    if info.get().status.is_transitioning() {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| spawn_local({
                    let previous_status = info.get().status;
                    info.update(|info| info.status = NodeStatus::Upgrading);
                    let container_id = info.get().container_id.clone();
                    async move {
                        match upgrade_node_instance(container_id).await {
                            Ok(()) => {
                                info.update(|node| {
                                    node.status = NodeStatus::Transitioned("Upgraded".to_string());
                                })
                            }
                            Err(err) => {
                                logging::log!("Failed to upgrade node: {err:?}");
                                show_alert_msg(err.to_string());
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
fn ButtonRemove(info: RwSignal<NodeInstanceInfo>) -> impl IntoView {
    view! {
        <div class="tooltip tooltip-bottom tooltip-info" data-tip="remove">

            <button
                class=move || {
                    if info.get().status.is_transitioning() {
                        "btn-disabled-node-action"
                    } else {
                        "btn-node-action"
                    }
                }
                on:click=move |_| spawn_local({
                    info.update(|info| info.status = NodeStatus::Removing);
                    let container_id = info.get().container_id.clone();
                    async move {
                        if let Err(err) = remove_node_instance(container_id).await {
                            logging::log!("Failed to remove node: {err:?}");
                            show_alert_msg(err.to_string());
                        }
                    }
                })
            >
                <IconRemoveNode />
            </button>
        </div>
    }
}
