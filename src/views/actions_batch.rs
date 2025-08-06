use super::{helpers::show_alert_msg, icons::IconCancel};
use crate::{
    app::ClientGlobalState,
    server_api::cancel_batch,
    types::{BatchType, NodesActionsBatch},
};

use leptos::{logging, prelude::*, task::spawn_local};

#[component]
pub(super) fn NodesActionsBatchesView() -> impl IntoView {
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
    let (count, auto_start) = if let BatchType::Create { count, node_opts } = &batch_type {
        (*count, node_opts.auto_start)
    } else {
        (batch_type.ids().len() as u16, false)
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
                "Batch " {move || batch_info.read().status.to_string()} ":"
            </h2>
            <ul class="max-w-md space-y-1 text-gray-500 list-disc list-inside dark:text-gray-400">
                <li>"Total number of nodes to " {batch_type.to_string()} ": " {count}</li>
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
