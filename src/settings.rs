use super::{
    app::AppSettings,
    helpers::show_alert_msg,
    icons::IconCloseModal,
    server_api::{get_settings, update_settings},
};

use alloy::primitives::Address;
use leptos::{logging, prelude::*};
use std::time::Duration;
use url::Url;

#[component]
pub fn SettingsView(settings_panel: RwSignal<bool>) -> impl IntoView {
    let current_settings = Resource::new(
        move || settings_panel.get() == true,
        |_| async move { get_settings().await.unwrap_or_default() },
    );
    let active_tab = RwSignal::new(0);

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
                    <div class="flex items-center justify-between p-4 md:p-5 rounded-t dark:border-gray-600">
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

                    <div class="border-b text-sm font-medium text-center text-gray-500 border-b border-gray-200 dark:text-gray-400 dark:border-gray-700">
                        <ul class="flex flex-wrap -mb-px">
                            <li class="me-2">
                                <a
                                    href="#"
                                    on:click=move |_| active_tab.set(0)
                                    class=move || {
                                        if active_tab.get() == 0 {
                                            "settings-active-tab"
                                        } else {
                                            "settings-tab"
                                        }
                                    }
                                >
                                    General
                                </a>
                            </li>
                            <li class="me-2">
                                <a
                                    href="#"
                                    on:click=move |_| active_tab.set(1)
                                    class=move || {
                                        if active_tab.get() == 1 {
                                            "settings-active-tab"
                                        } else {
                                            "settings-tab"
                                        }
                                    }
                                >
                                    LCD device setup
                                </a>
                            </li>
                        </ul>
                    </div>

                    <div class="p-4 md:p-5">
                        <Suspense fallback=move || {
                            view! { <p>"Loading..."</p> }
                        }>
                            {move || Suspend::new(async move {
                                view! {
                                    <SettingsForm
                                        curr=current_settings.await
                                        settings_panel
                                        active_tab
                                    />
                                }
                            })}
                        </Suspense>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn SettingsForm(
    curr: AppSettings,
    settings_panel: RwSignal<bool>,
    active_tab: RwSignal<u8>,
) -> impl IntoView {
    let auto_upgrade = RwSignal::new(curr.nodes_auto_upgrade);
    let auto_upgrade_delay = RwSignal::new(Ok(curr.nodes_auto_upgrade_delay.as_secs()));
    let bin_version_polling_freq = RwSignal::new(Ok(curr.node_bin_version_polling_freq.as_secs()));
    let balances_retrieval_freq = RwSignal::new(Ok(curr.rewards_balances_retrieval_freq.as_secs()));
    let metrics_polling_freq = RwSignal::new(Ok(curr.nodes_metrics_polling_freq.as_secs()));
    let l2_network_rpc_url = RwSignal::new(Ok(curr.l2_network_rpc_url.clone()));
    let token_contract_address = RwSignal::new(Ok(curr.token_contract_address.clone()));
    let lcd_enabled = RwSignal::new(curr.lcd_display_enabled);
    let lcd_device = RwSignal::new(Ok(curr.lcd_device.clone()));
    let lcd_addr = RwSignal::new(Ok(curr.lcd_addr.clone()));

    let update_settings_action = Action::new(move |settings: &AppSettings| {
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
        <span hidden=move || active_tab.get() != 0>
            <form class="space-y-4">
                <div class="flex items-center">
                    <input
                        checked=move || { curr.nodes_auto_upgrade }
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
                    signal=auto_upgrade_delay
                    min=0
                    label="Delay (in seconds) between nodes upgrading when auto-upgrading is enabled"
                />
                <NumberInput
                    signal=bin_version_polling_freq
                    min=3600
                    label="How often (in seconds) to check which is the latest version of the node binary"
                />
                <NumberInput
                    signal=balances_retrieval_freq
                    min=600
                    label="How often (in seconds) to query balances from the ledger using the configured L2 network RPC URL and token contract address"
                />
                <NumberInput
                    signal=metrics_polling_freq
                    min=5
                    label="How often (in seconds) to fetch metrics and node info from active/running nodes"
                />
                <TextInput
                    signal=l2_network_rpc_url
                    label="RPC URL to send queries to get rewards addresses balances from L2 network:"
                    validator=|v| { v.parse::<Url>().map_err(|err| err.to_string()).map(|_| v) }
                />
                <TextInput
                    signal=token_contract_address
                    label="ERC20 token contract address:"
                    validator=|v| { v.parse::<Address>().map_err(|err| err.to_string()).map(|_| v) }
                />
            </form>
        </span>

        <span hidden=move || active_tab.get() != 1>
            <form class="space-y-4">
                <div class="flex items-center">
                    <input
                        checked=move || {
                            let current = curr.lcd_display_enabled;
                            lcd_enabled.set(current);
                            current
                        }
                        id="lcd_enabled"
                        type="checkbox"
                        on:change=move |ev| { lcd_enabled.set(event_target_checked(&ev)) }
                        class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
                    />
                    <label
                        for="lcd_enabled"
                        class="ms-2 text-sm font-medium text-gray-900 dark:text-gray-300"
                    >
                        "Display nodes stats in external LCD display device (connected with an I2C backpack/interface)"
                    </label>
                </div>
                <TextInput
                    signal=lcd_device
                    label="I2C bus number (e.g. if the device path is configured at /dev/i2c-1, the bus number is 1):"
                    validator=|v| { v.parse::<u8>().map_err(|err| err.to_string()).map(|_| v) }
                />
                <TextInput
                    signal=lcd_addr
                    label="I2C backpack address (usually 0x27 or 0x3F):"
                    validator=|v| {
                        u16::from_str_radix(&v.strip_prefix("0x").unwrap_or(&v), 16)
                            .map_err(|err| err.to_string())
                            .map(|_| v)
                    }
                />
            </form>
        </span>
        <button
            type="button"
            disabled=move || {
                auto_upgrade_delay.get().is_err() || bin_version_polling_freq.get().is_err()
                    || balances_retrieval_freq.get().is_err() || metrics_polling_freq.get().is_err()
                    || l2_network_rpc_url.get().is_err() || token_contract_address.get().is_err()
            }

            on:click=move |_| {
                let values = (
                    auto_upgrade_delay.get_untracked(),
                    bin_version_polling_freq.get_untracked(),
                    balances_retrieval_freq.get_untracked(),
                    metrics_polling_freq.get_untracked(),
                    l2_network_rpc_url.get_untracked(),
                    token_contract_address.get_untracked(),
                    lcd_device.get_untracked(),
                    lcd_addr.get_untracked(),
                );
                if let (Ok(v1), Ok(v2), Ok(v3), Ok(v4), Ok(v5), Ok(v6), Ok(v7), Ok(v8)) = values {
                    update_settings_action
                        .dispatch(AppSettings {
                            nodes_auto_upgrade: auto_upgrade.get_untracked(),
                            nodes_auto_upgrade_delay: Duration::from_secs(v1),
                            node_bin_version_polling_freq: Duration::from_secs(v2),
                            rewards_balances_retrieval_freq: Duration::from_secs(v3),
                            nodes_metrics_polling_freq: Duration::from_secs(v4),
                            l2_network_rpc_url: v5,
                            token_contract_address: v6,
                            lcd_display_enabled: lcd_enabled.get_untracked(),
                            lcd_device: v7,
                            lcd_addr: v8,
                        });
                }
            }
            class="text-white bg-gray-800 hover:bg-gray-900 focus:outline-none focus:ring-4 focus:ring-gray-300 font-medium rounded-lg text-sm px-5 py-2.5 me-2 mb-2 dark:bg-gray-800 dark:hover:bg-gray-700 dark:focus:ring-gray-700 dark:border-gray-700"
        >
            Save
        </button>
    }
}

#[component]
pub fn NumberInput(
    signal: RwSignal<Result<u64, String>>,
    min: u64,
    label: &'static str,
) -> impl IntoView {
    let on_input = move |ev| {
        let val = match event_target_value(&ev).parse::<u64>() {
            Ok(v) if v < min => Err(format!("value cannot be smaller than {min}.")),
            Ok(v) => Ok(v),
            Err(err) => Err(err.to_string()),
        };
        signal.set(val);
    };

    view! {
        <div class="flex flex-row">
            <div class="basis-3/4">
                <span class="block mr-2 text-sm font-medium text-gray-900 dark:text-white">
                    {label} " (min: " {min} ")"
                </span>
            </div>
            <div class="basis-1/4">
                <input
                    type="number"
                    on:input=on_input
                    class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
                    value=signal.get_untracked().unwrap_or_default()
                    required
                />
            </div>
        </div>
        <div>
            <Show when=move || signal.get().is_err() fallback=move || view! { "" }.into_view()>
                <p class="ml-2 text-sm text-red-600 dark:text-red-500">
                    "Invalid value: " {signal.get().err()}
                </p>
            </Show>
        </div>
    }
}

#[component]
pub fn TextInput(
    signal: RwSignal<Result<String, String>>,
    label: &'static str,
    validator: fn(String) -> Result<String, String>,
) -> impl IntoView {
    let on_input = move |ev| {
        let val = match validator(event_target_value(&ev)) {
            Ok(v) => Ok(v),
            Err(err) => Err(err.to_string()),
        };
        signal.set(val);
    };

    view! {
        <div>
            <span class="block mt-5 mr-2 text-sm font-medium text-gray-900 dark:text-white">
                {label}
            </span>
        </div>
        <div>
            <input
                type="text"
                on:input=on_input
                class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
                value=signal.get_untracked().unwrap_or_default()
                required
            />
        </div>
        <div>
            <Show when=move || signal.get().is_err() fallback=move || view! { "" }.into_view()>
                <p class="ml-2 text-sm text-red-600 dark:text-red-500">
                    "Invalid value: " {signal.get().err()}
                </p>
            </Show>
        </div>
    }
}
