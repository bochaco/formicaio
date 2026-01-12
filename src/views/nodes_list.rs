use super::{
    actions_batch::NodesActionsBatchesView,
    chart::ChartSeriesData,
    icons::{
        IconChevronDown, IconCollapse, IconExpand, IconRecycle, IconRemove, IconStartNode,
        IconStopNode,
    },
    node_actions::{BatchActionModal, NodeAction},
    node_instance::NodeInstanceView,
    pagination::PaginationView,
    sort_nodes::SortStrategyView,
};
use crate::{
    app::{ClientGlobalState, PAGE_SIZE},
    views::icons::IconUpgradeNode,
};

use leptos::prelude::*;

#[component]
pub fn NodesListView(
    set_logs: WriteSignal<Vec<String>>,
    set_render_chart: RwSignal<bool>,
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    // we display the instances sorted with the currently selected strategy
    let sorted_nodes = Memo::new(move |_| {
        let mut sorted = context.nodes.get().1.into_iter().collect::<Vec<_>>();
        context
            .nodes_sort_strategy
            .read()
            .sort_view_items(&mut sorted);

        let offset = PAGE_SIZE * context.current_page.get();
        sorted
            .into_iter()
            .skip(offset)
            .take(PAGE_SIZE)
            .collect::<Vec<_>>()
    });

    // signal to toggle the panel to confirm actions to nodes
    let modal_apply_action = RwSignal::new(None);

    view! {
        <div>
            // List Toolbar
            <NodeListToolbarView
                num_nodes=Memo::new(move |_| sorted_nodes.read().len())
                modal_apply_action
            />

            // Nodes Grid
            <div class="p-4 lg:p-8">
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                    <Show when=move || !context.scheduled_batches.read().is_empty()>
                        <NodesActionsBatchesView />
                    </Show>

                    <For
                        each=move || sorted_nodes.get()
                        key=|(node_id, _)| node_id.clone()
                        let:child
                    >
                        <Show
                            when=move || !child.1.read().status.is_creating()
                            fallback=move || { view! { <CreatingNodeInstanceView /> }.into_view() }
                        >
                            <NodeInstanceView
                                info=child.1
                                set_logs
                                set_render_chart
                                set_chart_data
                            />
                        </Show>
                    </For>

                    <Show when=move || {
                        modal_apply_action.read().is_some() && *context.is_online.read()
                    }>
                        <BatchActionModal action=modal_apply_action />
                    </Show>

                </div>
            </div>
        </div>
    }
}

#[component]
fn NodeListToolbarView(
    num_nodes: Memo<usize>,
    modal_apply_action: RwSignal<Option<NodeAction>>,
) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let is_selection_open = RwSignal::new(false);
    let num_selected_nodes = move || {
        context
            .selecting_nodes
            .with(|(_enabled, selected)| selected.len())
    };

    let select_all = move || {
        context.selecting_nodes.update(|(enabled, selected)| {
            context
                .nodes
                .read()
                .1
                .iter()
                .filter(|(_, n)| !n.read().is_status_locked)
                .for_each(|(id, _)| {
                    selected.insert(id.clone());
                });
            *enabled = !selected.is_empty();
        });
        is_selection_open.set(false);
    };
    let select_none = move || {
        context.selecting_nodes.update(|(enabled, selected)| {
            *enabled = false;
            selected.clear();
        });
        is_selection_open.set(false);
    };
    let select_active = move || {
        context.selecting_nodes.update(|(enabled, selected)| {
            selected.clear();
            context
                .nodes
                .read()
                .1
                .iter()
                .filter(|(_, n)| n.read().status.is_active() && !n.read().is_status_locked)
                .for_each(|(id, _)| {
                    selected.insert(id.clone());
                });
            *enabled = !selected.is_empty();
        });
        is_selection_open.set(false);
    };
    let select_inactive = move || {
        context.selecting_nodes.update(|(enabled, selected)| {
            selected.clear();
            context
                .nodes
                .read()
                .1
                .iter()
                .filter(|(_, n)| n.read().status.is_inactive() && !n.read().is_status_locked)
                .for_each(|(id, _)| {
                    selected.insert(id.clone());
                });
            *enabled = !selected.is_empty();
        });
        is_selection_open.set(false);
    };

    view! {
        <div class="sticky top-0 z-20 bg-slate-950/80 backdrop-blur-md border-b border-slate-800 px-4 lg:px-8">
            <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4 py-3">
                <div class="flex items-center gap-4">
                    <div class="relative">
                        <button
                            on:click=move |_| is_selection_open.update(|prev| *prev = !*prev)
                            class="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-slate-400 hover:text-white bg-slate-800 border border-slate-700 rounded-lg transition-all"
                        >
                            Manage Selection
                            <IconChevronDown is_down=Signal::derive(move || {
                                is_selection_open.get()
                            }) />
                        </button>
                        <Show when=move || is_selection_open.get() fallback=|| ()>
                            <div class="absolute top-full mt-2 w-48 bg-slate-800 border border-slate-700 rounded-lg shadow-xl z-10 animate-in fade-in duration-150 py-1">
                                <a
                                    on:click=move |_| select_all()
                                    class="block w-full text-left px-4 py-2 text-sm text-slate-300 hover:bg-slate-700/50 cursor-pointer"
                                >
                                    Select All
                                </a>
                                <a
                                    on:click=move |_| select_none()
                                    class="block w-full text-left px-4 py-2 text-sm text-slate-300 hover:bg-slate-700/50 cursor-pointer"
                                >
                                    Select None
                                </a>
                                <div class="h-px bg-slate-700 my-1" />
                                <a
                                    on:click=move |_| select_active()
                                    class="block w-full text-left px-4 py-2 text-sm text-slate-300 hover:bg-slate-700/50 cursor-pointer"
                                >
                                    Select Active
                                </a>
                                <a
                                    on:click=move |_| select_inactive()
                                    class="block w-full text-left px-4 py-2 text-sm text-slate-300 hover:bg-slate-700/50 cursor-pointer"
                                >
                                    Select Inactive
                                </a>
                            </div>
                        </Show>
                    </div>

                    <div class="h-4 w-px bg-slate-800" />

                    <Show
                        when=move || context.expanded_nodes.read().len() < num_nodes.get()
                        fallback=move || {
                            view! {
                                <button
                                    on:click=move |_| {
                                        context
                                            .expanded_nodes
                                            .update(|expanded| {
                                                expanded.clear();
                                            });
                                    }
                                    class="flex items-center gap-1.5 text-sm font-medium text-slate-400 hover:text-slate-200 transition-colors"
                                >
                                    <IconCollapse />
                                    <span>"Collapse All"</span>
                                </button>
                            }
                        }
                    >
                        <button
                            on:click=move |_| {
                                context
                                    .expanded_nodes
                                    .update(|expanded| {
                                        *expanded = context
                                            .nodes
                                            .read()
                                            .1
                                            .iter()
                                            .map(|(id, _)| id.clone())
                                            .collect();
                                    });
                            }
                            class="flex items-center gap-1.5 text-sm font-medium text-slate-400 hover:text-slate-200 transition-colors"
                        >
                            <IconExpand />
                            <span>"Expand All"</span>
                        </button>
                    </Show>

                    <Show when=move || 0 < num_selected_nodes()>
                        <div class="flex items-center gap-2 animate-in fade-in slide-in-from-left-2">
                            <div class="h-4 w-px bg-slate-800 mx-2" />
                            <span class="text-xs text-indigo-400 font-bold px-2 py-1 bg-indigo-500/10 rounded-lg">
                                {move || num_selected_nodes()} " selected"
                            </span>
                            <button
                                on:click=move |_| modal_apply_action.set(Some(NodeAction::Upgrade))
                                class="p-1.5 hover:bg-cyan-500/10 text-cyan-500 rounded-lg transition-colors"
                                title="Upgrade Selected"
                            >
                                <IconUpgradeNode />
                            </button>
                            <button
                                on:click=move |_| modal_apply_action.set(Some(NodeAction::Recycle))
                                class="p-1.5 hover:bg-cyan-500/10 text-cyan-500 rounded-lg transition-colors"
                                title="Recycle Selected"
                            >
                                <IconRecycle />
                            </button>
                            <button
                                on:click=move |_| modal_apply_action.set(Some(NodeAction::Start))
                                class="p-1.5 hover:bg-emerald-500/10 text-emerald-500 rounded-lg transition-colors"
                                title="Start Selected"
                            >
                                <IconStartNode />
                            </button>
                            <button
                                on:click=move |_| modal_apply_action.set(Some(NodeAction::Stop))
                                class="p-1.5 hover:bg-rose-500/10 text-rose-700 rounded-lg transition-colors"
                                title="Stop Selected"
                            >
                                <IconStopNode />
                            </button>
                            <button
                                on:click=move |_| modal_apply_action.set(Some(NodeAction::Remove))
                                class="p-1.5 hover:bg-rose-500/10 text-rose-700 rounded-lg transition-colors"
                                title="Remove Selected"
                            >
                                <IconRemove />
                            </button>
                        </div>
                    </Show>
                </div>

                <PaginationView />

                <SortStrategyView />
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
