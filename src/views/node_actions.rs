use crate::{
    app::ClientGlobalState,
    server_api::{
        nodes_actions_batch_create, recycle_node_instance, start_node_instance, stop_node_instance,
        upgrade_node_instance,
    },
    types::{BatchType, NodeInstanceInfo, NodeStatus, NodesActionsBatch, Stats},
};

use super::{
    add_nodes::AddNodesForm,
    form_inputs::NumberInput,
    helpers::{remove_node_instance, show_alert_msg},
    icons::*,
};

use leptos::{logging, prelude::*, task::spawn_local};

// Action to apply on a node instance
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeAction {
    Start,
    Stop,
    Upgrade,
    Recycle,
    Remove,
}

impl NodeAction {
    // Apply the action to the given node instance
    pub async fn apply(&self, info: &RwSignal<NodeInstanceInfo>, stats: &RwSignal<Stats>) {
        let node_id = info.read_untracked().node_id.clone();
        let previous_status = info.read_untracked().status.clone();
        let res = match self {
            Self::Start => {
                if !previous_status.is_inactive() {
                    return;
                }
                info.update(|node| node.status = NodeStatus::Restarting);
                stats.update(|stats| {
                    no_zero_overflow_subs(&mut stats.active_nodes);
                    stats.inactive_nodes += 1;
                });
                start_node_instance(node_id.clone()).await
            }
            Self::Stop => {
                if !previous_status.is_active() {
                    return;
                }
                info.update(|node| node.status = NodeStatus::Stopping);
                stats.update(|stats| {
                    no_zero_overflow_subs(&mut stats.active_nodes);
                    stats.inactive_nodes += 1;
                });
                let res = stop_node_instance(node_id.clone()).await;

                if matches!(res, Ok(())) {
                    info.update(|node| {
                        node.connected_peers = Some(0);
                        node.kbuckets_peers = Some(0);
                    });
                }

                res
            }
            Self::Upgrade => {
                if !info.read_untracked().upgradeable() {
                    return;
                }
                info.update(|node| node.status = NodeStatus::Upgrading);
                stats.update(|stats| {
                    no_zero_overflow_subs(&mut stats.active_nodes);
                    stats.inactive_nodes += 1;
                });
                let res = upgrade_node_instance(node_id.clone()).await;

                if matches!(res, Ok(())) {
                    info.update(|node| {
                        node.bin_version = None;
                    })
                }

                res
            }
            Self::Recycle => {
                if previous_status.is_transitioning() || info.read_untracked().peer_id.is_none() {
                    return;
                }
                info.update(|node| node.status = NodeStatus::Recycling);
                stats.update(|stats| {
                    no_zero_overflow_subs(&mut stats.active_nodes);
                    stats.inactive_nodes += 1;
                });
                recycle_node_instance(node_id.clone()).await
            }
            Self::Remove => {
                info.update(|node| node.status = NodeStatus::Removing);
                stats.update(|stats| {
                    no_zero_overflow_subs(&mut stats.total_nodes);
                    if previous_status.is_active() {
                        no_zero_overflow_subs(&mut stats.active_nodes);
                    } else {
                        no_zero_overflow_subs(&mut stats.inactive_nodes);
                    }
                });
                remove_node_instance(node_id.clone()).await
            }
        };

        if let Err(err) = res {
            let msg = format!(
                "Failed to {self:?} node {}: {err:?}",
                info.read_untracked().short_node_id()
            );
            logging::log!("{msg}");
            show_alert_msg(msg);
            info.update(|node| node.status = previous_status);
        }
    }
}

// Helper to safely decrement a counter without underflowing
fn no_zero_overflow_subs(v: &mut usize) {
    if *v > 0 {
        *v -= 1;
    }
}

#[component]
pub fn NodesActionsView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;
    let btn_nodes_action_class = move || {
        if is_selecting_nodes() {
            "hidden"
        } else if context.nodes.read().1.is_empty() {
            "btn-disabled btn-manage-nodes-action"
        } else {
            "btn-manage-nodes-action"
        }
    };

    let show_actions_menu = RwSignal::new(false);
    // signal to toggle the panel to add nodes
    let modal_visibility = RwSignal::new(false);
    // signal to toggle the panel to confirm actions to nodes
    let modal_apply_action = RwSignal::new(None);

    view! {
        <div class="fixed end-6 bottom-6 group z-10">
            <div class=move || {
                if *show_actions_menu.read() {
                    "flex flex-col items-center mb-4 space-y-2"
                } else {
                    "hidden"
                }
            }>

                <ActionsOnSelected show_actions_menu modal_apply_action />

                <button
                    type="button"
                    on:click=move |_| {
                        context
                            .selecting_nodes
                            .update(|(enabled, selected)| {
                                *enabled = true;
                                context
                                    .nodes
                                    .read()
                                    .1
                                    .iter()
                                    .filter(|(_, n)| !n.read().is_status_locked)
                                    .for_each(|(id, _)| {
                                        selected.insert(id.clone());
                                    });
                            });
                    }
                    data-tooltip-target="tooltip-select-all"
                    data-tooltip-placement="left"
                    class=btn_nodes_action_class
                >
                    <IconSelectAll />
                    <span class="sr-only">Select all</span>
                </button>
                <div
                    id="tooltip-select-all"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Select all
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        context
                            .selecting_nodes
                            .update(|(enabled, selected)| {
                                *enabled = true;
                                context
                                    .nodes
                                    .read()
                                    .1
                                    .iter()
                                    .filter(|(_, n)| {
                                        n.read().status.is_active() && !n.read().is_status_locked
                                    })
                                    .for_each(|(id, _)| {
                                        selected.insert(id.clone());
                                    });
                            });
                    }
                    data-tooltip-target="tooltip-select-actives"
                    data-tooltip-placement="left"
                    class=btn_nodes_action_class
                >
                    <IconSelectActives />
                    <span class="sr-only">Select actives</span>
                </button>
                <div
                    id="tooltip-select-actives"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Select actives
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        context
                            .selecting_nodes
                            .update(|(enabled, selected)| {
                                *enabled = true;
                                context
                                    .nodes
                                    .read()
                                    .1
                                    .iter()
                                    .filter(|(_, n)| {
                                        n.read().status.is_inactive() && !n.read().is_status_locked
                                    })
                                    .for_each(|(id, _)| {
                                        selected.insert(id.clone());
                                    });
                            });
                    }
                    data-tooltip-target="tooltip-select-inactives"
                    data-tooltip-placement="left"
                    class=btn_nodes_action_class
                >
                    <IconSelectInactives />
                    <span class="sr-only">Select inactives</span>
                </button>
                <div
                    id="tooltip-select-inactives"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    Select inactives
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>

                <button
                    type="button"
                    on:click=move |_| {
                        context.selecting_nodes.update(|(enabled, _)| *enabled = true);
                    }
                    data-tooltip-target="tooltip-manage"
                    data-tooltip-placement="left"
                    class=btn_nodes_action_class
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
                if modal_visibility.get() && *context.is_online.read() {
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

                    <AddNodesForm modal_visibility />
                </div>
            </div>
        </div>

        <div
            id="apply_node_action_modal"
            tabindex="-1"
            aria-hidden="true"
            class=move || {
                if modal_apply_action.read().is_some() && *context.is_online.read() {
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
                            Apply action to selected nodes
                        </h3>
                        <button
                            type="button"
                            class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white"
                            on:click=move |_| {
                                show_actions_menu.set(true);
                                modal_apply_action.set(None);
                            }
                        >
                            <IconCancel />
                            <span class="sr-only">Cancel</span>
                        </button>
                    </div>

                    <div class="p-4 md:p-5">
                        <MultipleNodesActionConfirm modal_apply_action />
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn MultipleNodesActionConfirm(modal_apply_action: RwSignal<Option<NodeAction>>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let interval = RwSignal::new(Ok(60));

    view! {
        <form class="space-y-3">
            <NumberInput
                id="actions_interval"
                signal=interval
                min=0
                label="Delay (in seconds) between each node action in the batch:"
            />

            <button
                type="button"
                disabled=move || (interval.read().is_err() || modal_apply_action.read().is_none())
                on:click=move |_| {
                    if let (Ok(i), Some(action)) = (interval.get(), modal_apply_action.get()) {
                        modal_apply_action.set(None);
                        apply_on_selected(action, i.into(), context);
                    }
                }
                class="btn-modal"
            >
                {move || {
                    let count = context.selecting_nodes.read_untracked().1.len();
                    if let Some(action) = modal_apply_action.get() {
                        format!(
                            "{action:?} selected {}",
                            if count > 1 { format!("{count} nodes") } else { "node".to_string() },
                        )
                    } else {
                        "Apply action on selected node/s".to_string()
                    }
                }}
            </button>
        </form>
    }
}

#[component]
fn ActionsOnSelected(
    show_actions_menu: RwSignal<bool>,
    modal_apply_action: RwSignal<Option<NodeAction>>,
) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;

    view! {
        <button
            type="button"
            on:click=move |_| {
                show_actions_menu.set(false);
                context
                    .selecting_nodes
                    .update(|(enabled, selected)| {
                        selected.clear();
                        *enabled = false;
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

        <NodeActionButton
            label="Start selected"
            show_actions_menu
            modal_apply_action
            action=NodeAction::Start
            icon=IconStartNode.into_any()
        />

        <NodeActionButton
            label="Stop selected"
            show_actions_menu
            modal_apply_action
            action=NodeAction::Stop
            icon=IconStopNode.into_any()
        />

        <NodeActionButton
            label="Upgrade selected"
            show_actions_menu
            modal_apply_action
            action=NodeAction::Upgrade
            icon=view! { <IconUpgradeNode /> }.into_any()
        />

        <NodeActionButton
            label="Recycle selected"
            show_actions_menu
            modal_apply_action
            action=NodeAction::Recycle
            icon=IconRecycle.into_any()
        />

        <NodeActionButton
            label="Remove selected"
            show_actions_menu
            modal_apply_action
            action=NodeAction::Remove
            icon=IconRemove.into_any()
        />
    }
}

// Helper to apply an action on the set of nodes selected by the user,
// with the user selected delay between each action in a running batch.
fn apply_on_selected(action: NodeAction, interval: u64, context: ClientGlobalState) {
    let selected: Vec<_> = context
        .selecting_nodes
        .get_untracked()
        .1
        .into_iter()
        .collect();

    let batch_type = match action {
        NodeAction::Start => BatchType::Start(selected),
        NodeAction::Stop => BatchType::Stop(selected),
        NodeAction::Upgrade => BatchType::Upgrade(selected),
        NodeAction::Recycle => BatchType::Recycle(selected),
        NodeAction::Remove => BatchType::Remove(selected),
    };

    spawn_local(async move {
        match nodes_actions_batch_create(batch_type.clone(), interval).await {
            Ok(batch_id) => {
                let batch_info = NodesActionsBatch::new(batch_id, batch_type, interval);
                // update context for a better UX, this will get updated in next poll anyways
                context
                    .scheduled_batches
                    .update(|batches| batches.push(RwSignal::new(batch_info)));
                context.selecting_nodes.update(|(enabled, selected)| {
                    *enabled = false;
                    context.nodes.update(|(_, nodes)| {
                        selected.drain().for_each(|node_id| {
                            if let Some(node_info) = nodes.get(&node_id) {
                                node_info.update(|n| n.lock_status())
                            }
                        });
                    });
                });
            }
            Err(err) => {
                let msg = format!("Failed to schedule batch of {action:?}: {err:?}");
                logging::error!("{msg}");
                show_alert_msg(msg);
            }
        }
    });
}

#[component]
fn NodeActionButton(
    label: &'static str,
    show_actions_menu: RwSignal<bool>,
    modal_apply_action: RwSignal<Option<NodeAction>>,
    action: NodeAction,
    icon: AnyView,
) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selecting_nodes = move || context.selecting_nodes.read().0;
    let id = label.replace(" ", "-");
    let actions_class = move || {
        if !is_selecting_nodes() {
            "hidden"
        } else if context.selecting_nodes.read().1.is_empty() {
            "btn-manage-nodes-action btn-disabled"
        } else {
            "btn-manage-nodes-action"
        }
    };

    view! {
        <button
            type="button"
            on:click=move |_| {
                show_actions_menu.set(false);
                modal_apply_action.set(Some(action));
            }
            data-tooltip-target=id.clone()
            data-tooltip-placement="left"
            class=actions_class
        >
            {icon}
            <span class="sr-only">{label}</span>
        </button>
        <div
            id=id
            role="tooltip"
            class="absolute z-10 invisible inline-block w-auto px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
        >
            {label}
            <div class="tooltip-arrow" data-popper-arrow></div>
        </div>
    }
}
