use super::{
    app::AppSettings,
    helpers::show_alert_msg,
    icons::IconCloseModal,
    server_api::{get_settings, update_settings},
};

use leptos::*;
use std::num::ParseIntError;

#[component]
pub fn SettingsView(settings_panel: RwSignal<bool>) -> impl IntoView {
    let current_values = create_resource(
        move || settings_panel.get() == true,
        |_| async move { get_settings().await.unwrap_or_default() },
    );

    view! {
        <div
            id="settings_modal"
            tabindex="-1"
            aria-hidden="true"
            class=move || {
                if settings_panel.get() {
                    "overflow-y-auto overflow-x-hidden fixed inset-0 flex z-50 justify-center items-center w-full md:inset-0 h-[calc(100%-1rem)] max-h-full"
                } else {
                    "hidden"
                }
            }
        >
            <div class="relative p-4 w-full max-w-lg max-h-full">
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
                            Settings
                        </h3>
                        <button
                            type="button"
                            class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white"
                            on:click=move |_| settings_panel.set(false)
                        >
                            <IconCloseModal />
                            <span class="sr-only">Cancel</span>
                        </button>
                    </div>

                    <div class="p-4 md:p-5">
                        <SettingsForm current_values settings_panel />
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn SettingsForm(
    current_values: Resource<bool, AppSettings>,
    settings_panel: RwSignal<bool>,
) -> impl IntoView {
    let auto_upgrade = create_rw_signal(false);
    let auto_upgrade_delay_secs = create_rw_signal(Ok(0));

    let update_settings_action = create_action(move |settings: &AppSettings| {
        let settings = settings.clone();
        async move {
            if let Err(err) = update_settings(settings).await {
                logging::log!("Failed to update settings: {err:?}");
                show_alert_msg(err.to_string());
            } else {
                settings_panel.set(false);
            }
        }
    });

    view! {
        <Suspense fallback=move || view! { <p>"Loading..."</p> }>
            <form class="space-y-4">
                <div class="flex items-center">
                    <input
                        checked=move || {
                            let current = current_values
                                .get()
                                .map(|s| s.nodes_auto_upgrade)
                                .unwrap_or_default();
                            auto_upgrade.set(current);
                            current
                        }
                        id="auto_upgrade"
                        type="checkbox"
                        on:change=move |ev| { auto_upgrade.set(event_target_checked(&ev)) }
                        class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
                    />
                    <label
                        for="auto_upgrade"
                        class="ms-2 text-sm font-medium text-gray-900 dark:text-gray-300"
                    >
                        "Nodes auto-upgrading"
                    </label>
                </div>
                <NumberInput
                    signal=auto_upgrade_delay_secs
                    default=current_values
                        .get()
                        .map(|s| s.nodes_auto_upgrade_delay_secs)
                        .unwrap_or_default()
                    label="Delay (in seconds) between nodes upgrading when auto-upgrading is enabled"
                />

                <button
                    type="button"
                    disabled=move || auto_upgrade_delay_secs.get().is_err()
                    on:click=move |_| {
                        if let Ok(secs) = auto_upgrade_delay_secs.get_untracked() {
                            update_settings_action
                                .dispatch(AppSettings {
                                    nodes_auto_upgrade: auto_upgrade.get_untracked(),
                                    nodes_auto_upgrade_delay_secs: secs,
                                });
                        }
                    }
                    class="text-white bg-gray-800 hover:bg-gray-900 focus:outline-none focus:ring-4 focus:ring-gray-300 font-medium rounded-lg text-sm px-5 py-2.5 me-2 mb-2 dark:bg-gray-800 dark:hover:bg-gray-700 dark:focus:ring-gray-700 dark:border-gray-700"
                >
                    Save
                </button>

            </form>
        </Suspense>
    }
}

#[component]
pub fn NumberInput(
    signal: RwSignal<Result<u64, ParseIntError>>,
    default: u64,
    label: &'static str,
) -> impl IntoView {
    signal.set(Ok(default));
    let on_input = move |ev| signal.set(event_target_value(&ev).parse::<u64>());

    view! {
        <div class="flex flex-row">
            <div class="basis-3/4">
                <span class="block mr-2 text-sm font-medium text-gray-900 dark:text-white">
                    {label}
                </span>
            </div>
            <div class="basis-1/4">
                <input
                    type="number"
                    on:input=on_input
                    class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
                    value=default
                    required
                />
                <Show when=move || signal.get().is_err() fallback=move || view! { "" }.into_view()>
                    <p class="ml-2 text-sm text-red-600 dark:text-red-500">Invalid value</p>
                </Show>
            </div>
        </div>
    }
}
