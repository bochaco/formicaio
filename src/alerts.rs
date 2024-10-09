use super::{app::ClientGlobalState, icons::IconAlertMsgError};

use leptos::*;

#[component]
pub fn AlertMsg() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    view! {
        <For each=move || context.alerts.get() key=|(id, _)| *id let:child>
            <div
                id="marketing-banner"
                tabindex="-1"
                class="fixed z-50 flex flex-col md:flex-row justify-between w-[calc(100%-2rem)] p-4 -translate-x-1/2 bg-red-50 rounded-lg shadow-sm lg:max-w-7xl left-1/2 top-6 dark:bg-gray-800"
            >
                <p class="flex items-center text-sm font-normal text-red-800 bg-red-50 dark:text-red-400 dark:bg-gray-800">
                    <IconAlertMsgError />
                    {child.1}
                </p>
            </div>
        </For>
    }
}
