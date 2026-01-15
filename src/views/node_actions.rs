use crate::{
    app::ClientGlobalState,
    server_api::{
        nodes_actions_batch_create, recycle_node_instance, start_node_instance, stop_node_instance,
        upgrade_node_instance,
    },
    types::{BatchType, NodeInstanceInfo, NodeStatus, NodesActionsBatch, Stats},
};

use super::{
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
pub fn BatchActionModal(action: RwSignal<Option<NodeAction>>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let interval = RwSignal::new(60);

    struct ActionDetails {
        title: &'static str,
        verb: &'static str,
        icon: AnyView,
        colors: ActionColors,
    }
    struct ActionColors {
        header_bg: &'static str,
        icon_text: &'static str,
        button: &'static str,
    }

    let details = move || match action.get_untracked() {
        None => ActionDetails {
            title: "",
            verb: "",
            icon: view! { "" }.into_any(),
            colors: ActionColors {
                header_bg: "bg-emerald-500/10",
                icon_text: "text-emerald-400",
                button: "bg-emerald-600 hover:bg-emerald-500 shadow-emerald-500/20",
            },
        },
        Some(NodeAction::Start) => ActionDetails {
            title: "Start Nodes",
            verb: "start",
            icon: view! { <IconStartNode /> }.into_any(),
            colors: ActionColors {
                header_bg: "bg-emerald-500/10",
                icon_text: "text-emerald-400",
                button: "bg-emerald-600 hover:bg-emerald-500 shadow-emerald-500/20",
            },
        },
        Some(NodeAction::Stop) => ActionDetails {
            title: "Stop Nodes",
            verb: "stop",
            icon: view! { <IconStopNode /> }.into_any(),
            colors: ActionColors {
                header_bg: "bg-rose-500/10",
                icon_text: "text-rose-400",
                button: "bg-rose-600 hover:bg-rose-500 shadow-rose-500/20",
            },
        },
        Some(NodeAction::Remove) => ActionDetails {
            title: "Remove Nodes",
            verb: "remove",
            icon: view! { <IconRemove /> }.into_any(),
            colors: ActionColors {
                header_bg: "bg-rose-500/10",
                icon_text: "text-rose-400",
                button: "bg-rose-600 hover:bg-rose-500 shadow-rose-500/20",
            },
        },
        Some(NodeAction::Upgrade) => ActionDetails {
            title: "Upgrade Nodes",
            verb: "upgrade",
            icon: view! { <IconUpgradeNode /> }.into_any(),
            colors: ActionColors {
                header_bg: "bg-cyan-500/10",
                icon_text: "text-cyan-400",
                button: "bg-cyan-600 hover:bg-cyan-500 shadow-cyan-500/20",
            },
        },
        Some(NodeAction::Recycle) => ActionDetails {
            title: "Recycle Nodes",
            verb: "recycle",
            icon: view! { <IconRecycle /> }.into_any(),
            colors: ActionColors {
                header_bg: "bg-cyan-500/10",
                icon_text: "text-cyan-400",
                button: "bg-cyan-600 hover:bg-cyan-500 shadow-cyan-500/20",
            },
        },
    };

    view! {
        <div class="fixed inset-0 z-[100] flex items-center justify-center p-4 backdrop-blur-sm animate-in fade-in duration-300">
            <div class="bg-slate-900 border border-slate-800 w-full max-w-lg rounded-3xl overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
                <div class=format!(
                    "p-6 border-b border-slate-800 flex items-center justify-between {}",
                    details().colors.header_bg,
                )>
                    <div class="flex items-center gap-4">
                        <div class=details().colors.icon_text>{details().icon}</div>
                        <h3 class="text-xl font-bold text-white">Confirm Batch Action</h3>
                    </div>
                    <button
                        on:click=move |_| action.set(None)
                        class="p-2 text-slate-500 hover:text-white transition-colors"
                    >
                        <IconCancel />
                    </button>
                </div>

                <div class="p-8 space-y-6">
                    <p class="text-slate-300 text-center text-lg">
                        "You are about to "
                        <span class=format!(
                            "font-bold uppercase {}",
                            details().colors.icon_text,
                        )>
                            {details().verb}" "{context.selecting_nodes.read_untracked().1.len()}
                        </span> " selected node(s)."
                    </p>

                    <div class="space-y-3 pt-4">
                        <label
                            for="interval"
                            class="flex items-center gap-2 text-sm font-bold text-slate-400 uppercase tracking-widest"
                        >
                            "Delay between each node action in the batch:"
                        </label>
                        <div class="relative">
                            <input
                                id="interval"
                                type="number"
                                value=move || interval.get()
                                min=0
                                on:change=move |e| {
                                    interval.set(event_target_value(&e).parse::<u64>().unwrap_or(0))
                                }
                                class="w-full bg-slate-950 border border-slate-700 rounded-xl px-4 py-3 text-lg focus:ring-2 focus:ring-indigo-500 focus:outline-none transition-all pr-20"
                            />
                            <span class="absolute right-4 top-1/2 -translate-y-1/2 text-slate-500 text-sm">
                                seconds
                            </span>
                        </div>
                    </div>

                    <Show when=move || details().verb == "remove">
                        <div class="bg-rose-500/10 border border-rose-500/20 text-rose-300 text-sm rounded-xl p-4 flex items-start gap-3">
                            <p>
                                This action is irreversible. Please ensure you have selected the correct nodes before proceeding.
                            </p>
                        </div>
                    </Show>
                </div>

                <div class="p-6 bg-slate-950 border-t border-slate-800 flex items-center justify-end gap-4">
                    <button
                        class="px-6 py-2.5 rounded-xl font-bold text-slate-400 hover:bg-slate-800 transition-colors"
                        on:click=move |_| action.set(None)
                    >
                        Cancel
                    </button>
                    <button
                        class=format!(
                            "px-8 py-2.5 rounded-xl font-bold text-white transition-all shadow-lg {}",
                            details().colors.button,
                        )
                        on:click=move |_| {
                            if let Some(a) = action.get() {
                                apply_on_selected(a, interval.get(), context);
                                action.set(None);
                            }
                        }
                    >
                        "Confirm & "
                        {details().title}
                    </button>
                </div>
            </div>
        </div>
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
