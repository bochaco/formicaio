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
            <ActionBatchViewNew batch_info=child />
        </For>
    }
}

#[component]
fn ActionBatchViewNew(batch_info: RwSignal<NodesActionsBatch>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let batch_id = batch_info.get_untracked().id;
    let (batch_type, action_duration) = {
        let batch_type = batch_info.get_untracked().batch_type;
        let action_duration = match &batch_type {
            BatchType::Start { .. } => 2,
            _ => 0,
        };
        (batch_type, action_duration)
    };
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
    let time_remaining = move || {
        if count > 0 {
            let remaining = (count - batch_info.read().complete) as u64
                * (batch_info.get_untracked().interval_secs + action_duration);
            let minutes = remaining / 60;
            let seconds = remaining % 60;
            (minutes, seconds)
        } else {
            (0, 0)
        }
    };

    view! {
        <div class="bg-slate-900/70 border border-indigo-500/30 rounded-2xl p-4 transition-all duration-300 shadow-2xl shadow-indigo-500/10 backdrop-blur-sm flex flex-col gap-4 animate-in fade-in">
            <div class="flex items-start justify-between">
                <h4 class="text-base font-bold text-white">
                    "Batch " {move || batch_info.read().status.to_string()} ":"
                </h4>
                <button
                    title="Cancel batch"
                    class="p-1 text-slate-500 hover:text-white transition-colors"
                    on:click=move |_| spawn_local({
                        context
                            .scheduled_batches
                            .update(|batches| {
                                batches.retain(|b| { b.read_untracked().id != batch_id })
                            });
                        async move {
                            if let Err(err) = cancel_batch(batch_id).await {
                                let msg = format!("Failed to cancel node action batch: {err:?}");
                                logging::log!("{msg}");
                                show_alert_msg(msg);
                            }
                        }
                    })
                >
                    <IconCancel />
                </button>
            </div>

            <ul class="text-sm text-slate-300 list-disc list-inside space-y-1.5 pl-1">
                <li>
                    "Total number of nodes to "
                    <span class="font-bold uppercase text-indigo-400">
                        {batch_type.to_string()}
                    </span> ": " {count}
                </li>
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

            <div>
                <div class="flex justify-between items-center mb-1">
                    <span class="text-xs font-semibold text-slate-400">Nodes actions batch</span>
                    <span class="text-xs font-bold text-indigo-400">
                        {move || batch_info.read().complete} " / " {count}
                    </span>
                </div>
                <div class="w-full bg-slate-800 rounded-full h-2.5">
                    <div
                        class="bg-indigo-600 h-2.5 rounded-full transition-all duration-500 ease-linear"
                        style=move || format!("width: {}%", progress())
                    />
                </div>
                <div class="flex justify-between items-center mt-1">
                    <span class="text-xs font-bold text-white">{move || progress()}%</span>
                    <span class="text-xs text-white font-medium">
                        "Time remaining: "
                        {move || {
                            if time_remaining().0 > 0 {
                                format!("{}m {}s", time_remaining().0, time_remaining().1)
                            } else {
                                format!("{}s", time_remaining().1)
                            }
                        }}
                    </span>
                </div>
            </div>
        </div>
    }
}
