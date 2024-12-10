use super::{
    app::{get_addr_from_metamask, ClientGlobalState, NODES_LIST_POLLING_FREQ_MILLIS},
    helpers::{add_node_instances, remove_node_instance, show_alert_msg},
    icons::{
        IconAddNode, IconCancel, IconManageNodes, IconOpenActionsMenu, IconPasteAddr, IconRecycle,
        IconRemove, IconStartNode, IconStopNode, IconUpgradeNode,
    },
    node_instance::{NodeInstanceInfo, NodeStatus},
    server_api::{
        recycle_node_instance, start_node_instance, stop_node_instance, upgrade_node_instance,
    },
};

use alloy::primitives::Address;
use gloo_timers::future::sleep;
use leptos::{logging, prelude::*, task::spawn_local};
use std::{num::ParseIntError, time::Duration};

// TODO: find next available port numbers by looking at already used ones
const DEFAULT_NODE_PORT: u16 = 12000;
const DEFAULT_METRICS_PORT: u16 = 14000;

// Expected length of entered hex-encoded rewards address.
const REWARDS_ADDR_LENGTH: usize = 40;

// Action to apply on a node instance
#[derive(Clone, Debug, PartialEq)]
pub enum NodeAction {
    Start,
    Stop,
    Upgrade,
    Recycle,
    Remove,
}

impl NodeAction {
    // Apply the action to the given node instance
    pub async fn apply(&self, info: &RwSignal<NodeInstanceInfo>) {
        let container_id = info.read_untracked().container_id.clone();
        let previous_status = info.read_untracked().status.clone();
        let res = match self {
            Self::Start => {
                if !previous_status.is_inactive() {
                    return;
                }
                info.update(|node| node.status = NodeStatus::Restarting);
                start_node_instance(container_id.clone()).await
            }
            Self::Stop => {
                if !previous_status.is_active() {
                    return;
                }
                info.update(|node| node.status = NodeStatus::Stopping);
                let res = stop_node_instance(container_id.clone()).await;

                if matches!(res, Ok(())) {
                    info.update(|node| {
                        node.connected_peers = Some(0);
                        node.kbuckets_peers = Some(0);
                    })
                }

                res
            }
            Self::Upgrade => {
                if !info.read_untracked().upgradeable() {
                    return;
                }
                info.update(|node| node.status = NodeStatus::Upgrading);
                let res = upgrade_node_instance(container_id.clone()).await;

                if matches!(res, Ok(())) {
                    info.update(|node| {
                        node.bin_version = None;
                    })
                }

                res
            }
            Self::Recycle => {
                if !previous_status.is_active() {
                    return;
                }
                info.update(|node| node.status = NodeStatus::Recycling);
                recycle_node_instance(container_id.clone()).await
            }
            Self::Remove => {
                info.update(|node| node.status = NodeStatus::Removing);
                remove_node_instance(container_id.clone()).await
            }
        };

        if let Err(err) = res {
            let msg = format!(
                "Failed to {self:?} node {}: {err:?}",
                info.read_untracked().short_container_id()
            );
            logging::log!("{msg}");
            show_alert_msg(msg);
            info.update(|node| node.status = previous_status);
        }
    }
}

#[component]
pub fn NodesActionsView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;
    let is_selection_executing = move || context.selecting_nodes.read().1;
    let show_actions_menu = RwSignal::new(false);
    let actions_class = move || {
        if !is_selecting_nodes() {
            "hidden"
        } else if is_selection_executing() || context.selecting_nodes.read().2.is_empty() {
            "btn-manage-nodes-action btn-disabled"
        } else {
            "btn-manage-nodes-action"
        }
    };

    let apply_on_selected = move |action: NodeAction| {
        show_actions_menu.set(false);
        let action = action.clone();
        context
            .selecting_nodes
            .update(|(_, executing, _)| *executing = true);
        let selected = context.selecting_nodes.get_untracked().2;
        let nodes = context
            .nodes
            .read_untracked()
            .values()
            .filter(|n| selected.contains(&n.read_untracked().container_id))
            .cloned()
            .collect::<Vec<_>>();

        spawn_local(async move {
            let was_cancelled = move || !context.selecting_nodes.read_untracked().0;
            for info in nodes {
                if was_cancelled() {
                    break;
                }

                action.apply(&info).await;
                context.selecting_nodes.update(|(_, _, s)| {
                    s.remove(&info.read_untracked().container_id);
                });

                // let's await for it to finish transitioning unless it was a removal action
                while action != NodeAction::Remove
                    && !was_cancelled()
                    && info.read_untracked().status.is_transitioning()
                {
                    sleep(Duration::from_millis(NODES_LIST_POLLING_FREQ_MILLIS)).await;
                }
            }

            context.selecting_nodes.update(|(f, executing, s)| {
                *f = false;
                *executing = false;
                s.clear();
            });
        });
    };

    // signal to switch the panel to add nodes
    let modal_visibility = RwSignal::new(false);

    view! {
        <div class="fixed end-6 bottom-6 group z-10">
            <div class=move || {
                if *show_actions_menu.read() {
                    "flex flex-col items-center mb-4 space-y-2"
                } else {
                    "hidden"
                }
            }>

                <button
                    type="button"
                    on:click=move |_| {
                        context
                            .selecting_nodes
                            .update(|(f, _, s)| {
                                s.clear();
                                *f = false;
                            })
                    }
                    data-tooltip-target="tooltip-cancel"
                    data-tooltip-placement="left"
                    class=move || {
                        if is_selecting_nodes() {
                            "btn-manage-nodes-action ring-4 ring-gray-300 outline-none dark:ring-gray-400"
                        } else {
                            "hidden"
                        }
                    }
                >
                    <IconCancel />
                    <span class="sr-only">Cancel</span>
                </button>
                <div
                    id="tooltip-cancel"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Cancel
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        context.selecting_nodes.update(|(f, _, _)| *f = true);
                    }
                    data-tooltip-target="tooltip-manage"
                    data-tooltip-placement="left"
                    class=move || {
                        if is_selecting_nodes() || is_selection_executing() {
                            "hidden"
                        } else {
                            "btn-manage-nodes-action"
                        }
                    }
                >
                    <IconManageNodes />
                    <span class="sr-only">Manage</span>
                </button>
                <div
                    id="tooltip-manage"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Manage
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        apply_on_selected(NodeAction::Start);
                    }
                    data-tooltip-target="tooltip-start"
                    data-tooltip-placement="left"
                    class=move || actions_class()
                >
                    <IconStartNode />
                    <span class="sr-only">Start selected</span>
                </button>
                <div
                    id="tooltip-start"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Start selected
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        apply_on_selected(NodeAction::Stop);
                    }
                    data-tooltip-target="tooltip-stop"
                    data-tooltip-placement="left"
                    class=move || actions_class()
                >
                    <IconStopNode />
                    <span class="sr-only">Stop selected</span>
                </button>
                <div
                    id="tooltip-stop"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Stop selected
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        apply_on_selected(NodeAction::Upgrade);
                    }
                    data-tooltip-target="tooltip-upgrade"
                    data-tooltip-placement="left"
                    class=move || actions_class()
                >
                    <IconUpgradeNode color="currentColor".to_string() />
                    <span class="sr-only">Upgrade selected</span>
                </button>
                <div
                    id="tooltip-upgrade"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Upgrade selected
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        apply_on_selected(NodeAction::Recycle);
                    }
                    data-tooltip-target="tooltip-recycle"
                    data-tooltip-placement="left"
                    class=move || actions_class()
                >
                    <IconRecycle />
                    <span class="sr-only">Recycle selected</span>
                </button>
                <div
                    id="tooltip-recycle"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Recycle selected
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        apply_on_selected(NodeAction::Remove);
                    }
                    data-tooltip-target="tooltip-remove"
                    data-tooltip-placement="left"
                    class=move || actions_class()
                >
                    <IconRemove />
                    <span class="sr-only">Remove selected</span>
                </button>
                <div
                    id="tooltip-remove"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Remove selected
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        show_actions_menu.set(false);
                        modal_visibility.set(true);
                    }
                    data-tooltip-target="tooltip-add-nodes"
                    data-tooltip-placement="left"
                    class=move || {
                        if is_selecting_nodes() { "hidden" } else { "btn-manage-nodes-action" }
                    }
                >
                    <IconAddNode />
                    <span class="sr-only">Add nodes</span>
                </button>
                <div
                    id="tooltip-add-nodes"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Add nodes
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>
            </div>

            <button
                type="button"
                on:click=move |_| {
                    let showing = *show_actions_menu.read_untracked();
                    show_actions_menu.set(!showing);
                }
                class="flex items-center justify-center text-white bg-blue-700 rounded-full w-14 h-14 hover:bg-blue-800 dark:bg-blue-600 dark:hover:bg-blue-700 focus:ring-4 focus:ring-blue-300 focus:outline-none dark:focus:ring-blue-800"
            >
                <IconOpenActionsMenu />
                <span class="sr-only">Open actions menu</span>
            </button>
        </div>

        <div
            id="add_node_modal"
            tabindex="-1"
            aria-hidden="true"
            class=move || {
                if modal_visibility.get() {
                    "overflow-y-auto overflow-x-hidden fixed inset-0 flex z-50 justify-center items-center w-full md:inset-0 h-[calc(100%-1rem)] max-h-full"
                } else {
                    "hidden"
                }
            }
        >
            <div class="relative p-4 w-full max-w-lg max-h-full">
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
                            Add nodes
                        </h3>
                        <button
                            type="button"
                            class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white"
                            on:click=move |_| modal_visibility.set(false)
                        >
                            <IconCancel />
                            <span class="sr-only">Cancel</span>
                        </button>
                    </div>

                    <div class="p-4 md:p-5">
                        <AddNodesForm modal_visibility />
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn AddNodesForm(modal_visibility: RwSignal<bool>) -> impl IntoView {
    let port = RwSignal::new(Ok(DEFAULT_NODE_PORT));
    let metrics_port = RwSignal::new(Ok(DEFAULT_METRICS_PORT));
    let count = RwSignal::new(Ok(1));
    let rewards_addr = RwSignal::new(Err((
        "Enter a rewards address".to_string(),
        "0x".to_string(),
    )));
    let auto_start = RwSignal::new(false);
    let interval = RwSignal::new(Ok(60));

    let add_node = Action::new(
        move |(port, metrics_port, count, rewards_addr, auto_start, interval): &(
            u16,
            u16,
            u16,
            String,
            bool,
            u64,
        )| {
            let port = *port;
            let metrics_port = *metrics_port;
            let count = *count;
            let rewards_addr = rewards_addr
                .strip_prefix("0x")
                .unwrap_or(rewards_addr)
                .to_string();
            let auto_start = *auto_start;
            let interval = *interval;
            async move {
                let _ = add_node_instances(
                    port,
                    metrics_port,
                    count,
                    rewards_addr,
                    auto_start,
                    interval,
                )
                .await;
            }
        },
    );

    view! {
        <form class="space-y-4">
            <PortNumberInput
                signal=port
                default=DEFAULT_NODE_PORT
                label="Port number (range start):"
            />
            <PortNumberInput
                signal=metrics_port
                default=DEFAULT_METRICS_PORT
                label="Node metrics port number (range start):"
            />
            <RewardsAddrInput signal=rewards_addr label="Rewards address:" />
            <NumberInput
                signal=count
                min=1
                label="Number of nodes (a batch will be created if the number is greater than one):"
            />
            <NumberInput
                signal=interval
                min=0
                label="Delay (in seconds) between the creation of each node in the batch:"
            />
            <div class="flex items-center">
                <input
                    checked=false
                    id="auto-start"
                    type="checkbox"
                    on:change=move |ev| { auto_start.set(event_target_checked(&ev)) }
                    class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
                />
                <label
                    for="auto-start"
                    class="ms-2 text-sm font-medium text-gray-900 dark:text-gray-300"
                >
                    "Automatically starts nodes upon creation"
                </label>
            </div>

            <button
                type="button"
                disabled=move || {
                    port.read().is_err() || metrics_port.read().is_err() || count.read().is_err()
                        || rewards_addr.read().is_err() || interval.read().is_err()
                }
                on:click=move |_| {
                    if let (Ok(p), Ok(m), Ok(c), Ok(addr), Ok(i)) = (
                        port.get(),
                        metrics_port.get(),
                        count.get(),
                        rewards_addr.get(),
                        interval.get(),
                    ) {
                        modal_visibility.set(false);
                        add_node.dispatch((p, m, c, addr, auto_start.get(), i as u64));
                    }
                }
                class="text-white bg-gray-800 hover:bg-gray-900 focus:outline-none focus:ring-4 focus:ring-gray-300 font-medium rounded-lg text-sm px-5 py-2.5 me-2 mb-2 dark:bg-gray-800 dark:hover:bg-gray-700 dark:focus:ring-gray-700 dark:border-gray-700"
            >
                {move || {
                    let count = count.get().unwrap_or_default();
                    if count > 1 {
                        format!("Add {count} nodes")
                    } else {
                        "Add single node".to_string()
                    }
                }}
            </button>
        </form>
    }
}

#[component]
fn PortNumberInput(
    signal: RwSignal<Result<u16, ParseIntError>>,
    default: u16,
    label: &'static str,
) -> impl IntoView {
    let on_port_input = move |ev| signal.set(event_target_value(&ev).parse::<u16>());

    view! {
        <div>
            <span class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">
                {label}
            </span>
            <input
                type="number"
                name="port"
                id="port"
                on:input=on_port_input
                class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
                value=default
                required
            />
        </div>
        <div>
            <Show when=move || signal.read().is_err() fallback=move || view! { "" }.into_view()>
                <p class="mt-2 text-sm text-red-600 dark:text-red-500">Not a valid port number</p>
            </Show>
        </div>
    }
}

#[component]
pub fn NumberInput(
    signal: RwSignal<Result<u16, String>>,
    min: u16,
    label: &'static str,
) -> impl IntoView {
    let on_input = move |ev| {
        let val = match event_target_value(&ev).parse::<u16>() {
            Ok(v) if v < min => Err(format!("value cannot be smaller than {min}.")),
            Ok(v) => Ok(v),
            Err(err) => Err(err.to_string()),
        };
        signal.set(val);
    };

    view! {
        <div class="flex flex-row">
            <div class="basis-2/3">
                <span class="block mr-2 text-sm font-medium text-gray-900 dark:text-white">
                    {label}
                </span>
            </div>
            <div class="basis-1/3">
                <input
                    type="number"
                    on:input=on_input
                    class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
                    value=signal.get_untracked().unwrap_or_default()
                    required
                />
            </div>
        </div>
        <div>
            <Show when=move || signal.read().is_err() fallback=move || view! { "" }.into_view()>
                <p class="ml-2 text-sm text-red-600 dark:text-red-500">
                    "Invalid value: " {signal.get().err()}
                </p>
            </Show>
        </div>
    }
}

#[component]
pub fn RewardsAddrInput(
    signal: RwSignal<Result<String, (String, String)>>,
    label: &'static str,
) -> impl IntoView {
    let validate_and_set = move |input_str: String| {
        let value = input_str
            .strip_prefix("0x")
            .unwrap_or(&input_str)
            .to_string();

        let res = if value.len() != REWARDS_ADDR_LENGTH {
            Err((
                "Unexpected length of rewards address".to_string(),
                input_str,
            ))
        } else if hex::decode(&value).is_err() {
            Err((
                "The address entered is not hex-encoded".to_string(),
                input_str,
            ))
        } else if value.to_lowercase() == value || value.to_uppercase() == value {
            // it's a non-checksummed address
            Ok(input_str)
        } else {
            // validate checksum
            match Address::parse_checksummed(&format!("0x{value}"), None) {
                Ok(_) => Ok(input_str),
                Err(_) => Err(("Checksum validation failed".to_string(), input_str)),
            }
        };

        signal.set(res);
    };

    view! {
        <div>
            <label
                for="rewards_addr"
                class="block mb-2 text-sm font-medium text-gray-900 dark:text-white"
            >
                {label}
            </label>

            <div class="flex items-center">
                <div class="relative w-full">
                    <input
                        type="text"
                        name="rewards_addr"
                        id="rewards_addr"
                        on:input=move |ev| validate_and_set(event_target_value(&ev))
                        required
                        class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
                        prop:value=move || match signal.get() {
                            Ok(s) => s,
                            Err((_, s)) => s,
                        }
                    />
                </div>

                <button
                    data-tooltip-target="tooltip-rewards_addr"
                    class="btn-node-action"
                    type="button"
                    on:click=move |_| {
                        spawn_local(async move {
                            if let Some(addr) = get_addr_from_metamask().await.as_string() {
                                validate_and_set(addr);
                            } else {
                                let prev = match signal.get_untracked() {
                                    Ok(s) => s,
                                    Err((_, s)) => s,
                                };
                                signal
                                    .set(
                                        Err((
                                            "Failed to retrieve address from Metamask".to_string(),
                                            prev,
                                        )),
                                    )
                            }
                        });
                    }
                >
                    <IconPasteAddr />
                </button>
                <div
                    id="tooltip-rewards_addr"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    <span>Retrieve address from Metamask</span>
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>
            </div>

            <Show when=move || signal.read().is_err() fallback=move || view! { "" }.into_view()>
                <p class="mt-2 text-sm text-red-600 dark:text-red-500">
                    {signal.get().err().map(|(e, _)| e)}
                </p>
            </Show>
        </div>
    }
}
