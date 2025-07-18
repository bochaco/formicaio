use super::icons::IconAlertMsgError;
use crate::app::ClientGlobalState;

use leptos::prelude::*;

#[component]
pub fn AlertMsg() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    view! {
        <For each=move || context.alerts.get() key=|(id, _)| *id let:child>
            <div
                id="alerts"
                tabindex="-1"
                class="fixed z-50 flex flex-col md:flex-row justify-between w-[calc(100%-2rem)] p-4 -translate-x-1/2 bg-gray-50 rounded-lg shadow-sm lg:max-w-7xl left-1/2 top-6 dark:bg-gray-800"
            >
                <p class="flex items-center text-sm font-normal text-red-400 bg-gray-50 dark:text-red-400 dark:bg-gray-800">
                    <IconAlertMsgError />
                    {child.1}
                </p>
            </div>
        </For>
    }
}

#[component]
pub fn OfflineMsg() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    view! {
        <div
            id="offline-modal"
            tabindex="-1"
            class=move || {
                if *context.is_online.read() {
                    "hidden overflow-y-auto overflow-x-hidden fixed inset-0 flex z-50 justify-center items-center w-full md:inset-0 h-[calc(100%-1rem)] max-h-full"
                } else {
                    "overflow-y-auto overflow-x-hidden fixed inset-0 flex z-50 justify-center items-center w-full md:inset-0 h-[calc(100%-1rem)] max-h-full"
                }
            }
        >
            <div class="relative p-4 w-full max-w-md max-h-full">
                <div class="relative bg-white rounded-lg shadow-sm dark:bg-gray-700">
                    <div class="p-4 md:p-5 text-center">
                        <svg
                            class="mx-auto mb-4 text-gray-400 w-12 h-12 dark:text-gray-200"
                            aria-hidden="true"
                            xmlns="http://www.w3.org/2000/svg"
                            fill="none"
                            viewBox="0 0 20 20"
                        >
                            <path
                                stroke="currentColor"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                stroke-width="2"
                                d="M10 11V6m0 8h.01M19 10a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"
                            />
                        </svg>
                        <h3 class="mb-5 text-lg font-normal text-gray-500 dark:text-gray-400">
                            <p>"The connection to the backend has been lost."</p>
                            <br />
                            <p>
                                "Please try refreshing this page by pressing "
                                <kbd class="px-2 py-1.5 text-xs font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-lg dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500">
                                    "Ctrl"
                                </kbd>" + "
                                <kbd class="px-2 py-1.5 text-xs font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-lg dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500">
                                    "F5"
                                </kbd>
                                ". If the issue persists, check to ensure that the backend server is operational."
                            </p>
                            <br />
                            <p>
                                "Once the backend is confirmed to be running, you can attempt to refresh this page again using Ctrl+F5. This message will disappear automatically once the connection is reestablished."
                            </p>
                        </h3>
                    </div>
                </div>
            </div>
        </div>
    }
}
