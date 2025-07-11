use super::{
    actions_batch::NodesActionsBatchesView, chart::NodeChartView, icons::IconCancel,
    node_instance::NodeInstanceView,
};
use crate::app::{ClientGlobalState, PAGE_SIZE};

use leptos::prelude::*;

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

        <input
            type="checkbox"
            id="logs_stream_modal"
            class="modal-toggle"
            prop:checked=move || context.logs_stream_on_for.read().is_some()
        />
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
                    <button
                        class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2"
                        on:click=move |_| context.logs_stream_on_for.set(None)
                    >
                        <IconCancel />
                    </button>
                </div>
            </div>
        </div>

        <input
            type="checkbox"
            id="node_chart_modal"
            class="modal-toggle"
            prop:checked=move || context.metrics_update_on_for.read().is_some()
        />
        <div class="modal" role="dialog">
            <div class="modal-box border border-solid border-slate-50 w-4/5 max-w-full h-3/5 max-h-full overflow-y-auto">
                <h3 class="text-sm font-bold">"Node Mem & CPU"</h3>
                <div class="border-transparent h-full">
                    <NodeChartView is_render_chart chart_data />
                </div>

                <div class="modal-action">
                    <button
                        class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2"
                        on:click=move |_| {
                            set_render_chart.set(false);
                            context.metrics_update_on_for.set(None);
                        }
                    >
                        <IconCancel />
                    </button>
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
