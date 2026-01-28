use super::icons::IconCancel;
use crate::app::ClientGlobalState;

use leptos::{html, prelude::*};

#[component]
pub fn LogViewerModal(logs: ReadSignal<Vec<String>>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let list_element: NodeRef<html::Main> = NodeRef::new();
    let auto_scroll = RwSignal::new(true);

    Effect::new(move |_| {
        if !auto_scroll.get() || logs.read().is_empty() {
            return;
        }
        if let Some(node) = list_element.get() {
            node.set_scroll_top(node.scroll_height());
        }
    });

    let is_active = move || {
        context
            .logs_stream_on_for
            .read()
            .map(|info| info.read().status.is_active())
            .unwrap_or(false)
    };
    let status_summary = move || {
        context
            .logs_stream_on_for
            .read()
            .map(|info| info.read().status_summary())
            .unwrap_or_default()
    };

    view! {
        <div class="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm animate-in fade-in duration-300">
            <div class="bg-slate-900 border border-slate-800 w-full max-w-8xl h-[90vh] rounded-2xl overflow-hidden shadow-2xl flex flex-col animate-in zoom-in-95 duration-300">
                <header class="p-4 border-b border-slate-800 flex items-center justify-between bg-slate-800/30 shrink-0">
                    <div class="flex items-center gap-3">
                        <h3 class="text-sm font-bold">
                            "Live logs of node: "
                            <span class="text-indigo-400 font-mono">
                                {move || context
                                    .logs_stream_on_for
                                    .read()
                                    .as_ref()
                                    .map(|nid| nid.read().short_node_id())
                                    .unwrap_or("".to_string())}
                            </span>
                        </h3>
                    </div>
                    <button
                        on:click=move |_| context.logs_stream_on_for.set(None)
                        class="p-2 text-slate-500 hover:text-white transition-colors rounded-lg"
                    >
                        <IconCancel />
                    </button>
                </header>

                <main
                    node_ref=list_element
                    class="flex-1 p-6 font-mono text-sm overflow-y-auto bg-slate-950/80 no-scrollbar space-y-1.5"
                >
                    <For
                        each=move || logs.get().into_iter().enumerate()
                        key=|(i, _)| *i
                        children=move |(index, log_entry)| {
                            let color = if log_entry.contains("ERROR") {
                                "text-rose-400"
                            } else if log_entry.contains("WARN") {
                                "text-amber-400"
                            } else {
                                "text-slate-400"
                            };
                            view! {
                                <div
                                    prop:key=index
                                    class=format!("whitespace-pre-wrap break-words {color}")
                                >
                                    {log_entry}
                                </div>
                            }
                        }
                    />
                </main>
                <footer class="p-3 border-t border-slate-800 bg-slate-800/30 text-xs text-slate-500 flex items-center gap-2">
                    <div class=move || {
                        format!(
                            "w-2 h-2 rounded-full {}",
                            if is_active() { "bg-emerald-500 animate-pulse" } else { "bg-rose-500" },
                        )
                    } />
                    <span>
                        Node Status:
                        <span class="font-bold capitalize">{move || status_summary()}</span>
                    </span>
                    <div class="flex-1" />
                    <label
                        for="autoScrollToggle"
                        class="flex items-center gap-2 cursor-pointer group"
                    >
                        <span class="font-semibold group-hover:text-slate-300 transition-colors">
                            Auto-scrolling to bottom
                        </span>
                        <div class="relative">
                            <input
                                id="autoScrollToggle"
                                type="checkbox"
                                class="sr-only peer"
                                checked=auto_scroll
                                on:change=move |_| auto_scroll.update(|prev| *prev = !*prev)
                            />
                            <div class="w-10 h-5 bg-slate-700 rounded-full peer peer-checked:after:translate-x-full after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-indigo-600"></div>
                        </div>
                    </label>
                </footer>
            </div>
        </div>
    }
}
