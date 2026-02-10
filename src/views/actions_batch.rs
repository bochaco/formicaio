use super::{helpers::show_error_alert_msg, icons::IconCancel};
use crate::{
    app::ClientGlobalState,
    server_api::cancel_batch,
    types::{BatchStatus, BatchType, NodesActionsBatch},
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
    let finished = move || match &batch_info.read().status {
        BatchStatus::Scheduled => 0,
        BatchStatus::InProgress => batch_info.read().complete,
        BatchStatus::InProgressWithFailures(c, _) => *c + batch_info.read().complete,
        BatchStatus::Failed(_) => count,
    };
    let progress = move || {
        if count > 0 {
            (finished() * 100) / count
        } else {
            0
        }
    };
    let time_remaining = move || {
        if count > 0 {
            let remaining = (count - finished()) as u64
                * (batch_info.get_untracked().interval_secs + action_duration);
            let minutes = remaining / 60;
            let seconds = remaining % 60;
            (minutes, seconds)
        } else {
            (0, 0)
        }
    };
    let is_failed = Memo::new(move |_| match &batch_info.read().status {
        BatchStatus::Failed(reason) => Some((
            reason.clone(),
            count - batch_info.read().complete,
            "Batch failed".to_string(),
        )),
        BatchStatus::InProgressWithFailures(c, reason) => Some((
            reason.clone(),
            *c,
            "Batch in progress with failures".to_string(),
        )),
        _ => None,
    });

    view! {
        <div class=move || {
            format!(
                "border-2 rounded-2xl p-5 transition-all duration-500 shadow-2xl backdrop-blur-md flex flex-col gap-4 animate-in fade-in slide-in-from-top-4 {}",
                if is_failed.read().is_some() {
                    "bg-rose-950/40 border-rose-500/50 shadow-rose-500/10"
                } else {
                    "bg-slate-900/70 border-indigo-500/30 shadow-indigo-500/10"
                },
            )
        }>
            <div class="flex items-start justify-between">
                <h4 class="text-base font-bold text-white">
                    {move || {
                        if let Some((_, _, failure_status)) = is_failed.get() {
                            failure_status
                        } else {
                            batch_info.read().status.to_string()
                        }
                    }} ":"
                </h4>
                <button
                    title=move || {
                        if is_failed.read().is_none() { "Cancel batch" } else { "Dismiss" }
                    }
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
                                show_error_alert_msg(msg);
                            }
                        }
                    })
                >
                    <IconCancel />
                </button>
            </div>

            <Show when=move || is_failed.read().is_some()>
                <div class="bg-rose-500/10 border border-rose-500/20 rounded-xl p-4 animate-in zoom-in-95">
                    <span class="text-[10px] font-bold text-rose-400 uppercase tracking-widest block mb-2">
                        Last Error
                    </span>
                    <p class="text-sm text-rose-100 font-mono leading-relaxed">
                        {move || {
                            is_failed.get().map(|(reason, _, _)| reason.clone()).unwrap_or_default()
                        }}
                    </p>
                    <div class="mt-3 text-xs text-rose-300 flex items-center gap-2">
                        <span class="w-1.5 h-1.5 rounded-full bg-rose-500" />
                        {move || is_failed.get().map(|(_, count, _)| count).unwrap_or_default()}
                        " failures."
                    </div>
                </div>
            </Show>

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
                        {move || finished()} " / " {count}
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
                    <Show when=move || !batch_info.read().status.is_finished()>
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
                    </Show>
                </div>
            </div>
        </div>
    }
}
