use super::icons::{IconCancel, IconOffline};
use crate::app::ClientGlobalState;

use leptos::prelude::*;

#[component]
pub fn AlertMsg() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let alerts = Memo::new(move |_| {
        context
            .alerts
            .get()
            .into_iter()
            .filter(|notif| !notif.shown)
            .collect::<Vec<_>>()
    });

    view! {
        <div class="fixed top-20 right-4 z-[200] space-y-3 w-full max-w-sm">
            <For each=move || alerts.get() key=|notif| notif.id let:child>
                <div
                    prop:key=child.id
                    class=format!(
                        "w-full max-w-sm rounded-2xl p-4 border flex items-start gap-4 shadow-2xl backdrop-blur-md animate-in fade-in slide-in-from-top-4 duration-300 {}",
                        child.color(),
                    )
                >
                    <div class=format!("mt-0.5 {}", child.icon_color())>{child.icon()}</div>
                    <div class="flex-1 text-sm text-slate-200 font-medium">
                        {child.message.clone()}
                    </div>
                    <button
                        on:click=move |_| {
                            context.alerts.update(|msgs| msgs.retain(|n| n.id != child.id))
                        }
                        class="p-1 -m-1 text-slate-500 hover:text-white transition-colors rounded-full"
                    >
                        <IconCancel />
                    </button>
                </div>
            </For>
        </div>
    }
}

#[component]
pub fn OfflineMsg() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    view! {
        <Show when=move || !context.is_online.get()>
            <div class="fixed inset-0 z-[200] flex items-center justify-center p-4 bg-black/90 backdrop-blur-sm animate-in fade-in duration-500">
                <div class="bg-slate-900 border border-rose-500/30 w-full max-w-md rounded-2xl overflow-hidden shadow-2xl shadow-rose-500/10 flex flex-col items-center text-center p-8 gap-4">
                    <div class="w-16 h-16 rounded-full bg-rose-500/10 border-4 border-rose-500/20 flex items-center justify-center text-rose-500">
                        <IconOffline />
                    </div>
                    <h3 class="text-xl font-bold text-white mt-2">
                        The connection to the backend has been lost
                    </h3>
                    <p class="text-slate-400">
                        "Try refreshing this page by pressing "
                        <kbd class="px-2 py-1.5 text-xs font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-lg dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500">
                            "Ctrl"
                        </kbd>" + "
                        <kbd class="px-2 py-1.5 text-xs font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-lg dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500">
                            "F5"
                        </kbd>
                        ". If the issue persists, check to ensure that the backend server is operational."
                    </p>
                    <p class="text-slate-400">
                        "Once the backend is confirmed to be running, you can attempt to refresh this page again using Ctrl+F5. This message will disappear automatically once the connection is reestablished."
                    </p>

                    <div class="mt-4 flex items-center gap-2 text-slate-500">
                        <div class="w-3 h-3 border-2 border-slate-500 rounded-full animate-spin border-t-transparent" />
                        <span>Attempting to reconnect...</span>
                    </div>
                </div>
            </div>
        </Show>
    }
}
