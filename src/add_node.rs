use super::{
    app::get_addr_from_metamask,
    helpers::add_node_instances,
    icons::{IconAddNode, IconCloseModal, IconPasteAddr},
};

use alloy::primitives::Address;
use leptos::*;
use std::num::ParseIntError;

// TODO: find next available port numbers by looking at already used ones
const DEFAULT_NODE_PORT: u16 = 12000;
const DEFAULT_METRICS_PORT: u16 = 14000;

// Expected length of entered hex-encoded rewards address.
const REWARDS_ADDR_LENGTH: usize = 40;

#[component]
pub fn AddNodeView() -> impl IntoView {
    let modal_visibility = create_rw_signal(false);

    view! {
        <div class="divider divider-center">
            <button type="button" class="btn-add-node" on:click=move |_| modal_visibility.set(true)>
                <IconAddNode />
                Add nodes
            </button>
        </div>
        <div
            id="add_node_modal"
            tabindex="-1"
            aria-hidden="true"
            class=move || {
                if modal_visibility.get() {
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
                            Add nodes
                        </h3>
                        <button
                            type="button"
                            class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white"
                            on:click=move |_| modal_visibility.set(false)
                        >
                            <IconCloseModal />
                            <span class="sr-only">Cancel</span>
                        </button>
                    </div>

                    <div class="p-4 md:p-5">
                        <AddNodesForm modal_visibility />
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn AddNodesForm(modal_visibility: RwSignal<bool>) -> impl IntoView {
    let port = create_rw_signal(Ok(DEFAULT_NODE_PORT));
    let metrics_port = create_rw_signal(Ok(DEFAULT_METRICS_PORT));
    let count = create_rw_signal(Ok(1));
    let rewards_addr = create_rw_signal(Err((
        "Enter a rewards address".to_string(),
        "0x".to_string(),
    )));
    let auto_start = create_rw_signal(false);
    let interval = create_rw_signal(Ok(60));

    let add_node = create_action(
        move |(port, metrics_port, count, rewards_addr, auto_start, interval): &(
            u16,
            u16,
            u16,
            String,
            bool,
            u64,
        )| {
            let port = *port;
            let metrics_port = *metrics_port;
            let count = *count;
            let rewards_addr = rewards_addr
                .strip_prefix("0x")
                .unwrap_or(rewards_addr)
                .to_string();
            let auto_start = *auto_start;
            let interval = *interval;
            async move {
                let _ = add_node_instances(
                    port,
                    metrics_port,
                    count,
                    rewards_addr,
                    auto_start,
                    interval,
                )
                .await;
            }
        },
    );

    view! {
        <form class="space-y-4">
            <PortNumberInput
                signal=port
                default=DEFAULT_NODE_PORT
                label="Port number (range start):"
            />
            <PortNumberInput
                signal=metrics_port
                default=DEFAULT_METRICS_PORT
                label="Node metrics port number (range start):"
            />
            <RewardsAddrInput signal=rewards_addr label="Rewards address:" />
            <NumberInput
                signal=count
                min=1
                label="Number of nodes (a batch will be created if the number is greater than one):"
            />
            <NumberInput
                signal=interval
                min=0
                label="Delay (in seconds) between the creation of each node in the batch:"
            />
            <div class="flex items-center">
                <input
                    checked=false
                    id="auto-start"
                    type="checkbox"
                    on:change=move |ev| { auto_start.set(event_target_checked(&ev)) }
                    class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
                />
                <label
                    for="auto-start"
                    class="ms-2 text-sm font-medium text-gray-900 dark:text-gray-300"
                >
                    "Automatically starts nodes upon creation"
                </label>
            </div>

            <button
                type="button"
                disabled=move || {
                    port.get().is_err() || metrics_port.get().is_err() || count.get().is_err()
                        || rewards_addr.get().is_err() || interval.get().is_err()
                }
                on:click=move |_| {
                    if let (Ok(p), Ok(m), Ok(c), Ok(addr), Ok(i)) = (
                        port.get(),
                        metrics_port.get(),
                        count.get(),
                        rewards_addr.get(),
                        interval.get(),
                    ) {
                        modal_visibility.set(false);
                        add_node.dispatch((p, m, c, addr, auto_start.get(), i as u64));
                    }
                }
                class="text-white bg-gray-800 hover:bg-gray-900 focus:outline-none focus:ring-4 focus:ring-gray-300 font-medium rounded-lg text-sm px-5 py-2.5 me-2 mb-2 dark:bg-gray-800 dark:hover:bg-gray-700 dark:focus:ring-gray-700 dark:border-gray-700"
            >
                {move || {
                    let count = count.get().unwrap_or_default();
                    if count > 1 {
                        format!("Add {count} nodes")
                    } else {
                        "Add single node".to_string()
                    }
                }}
            </button>
        </form>
    }
}

#[component]
fn PortNumberInput(
    signal: RwSignal<Result<u16, ParseIntError>>,
    default: u16,
    label: &'static str,
) -> impl IntoView {
    let on_port_input = move |ev| signal.set(event_target_value(&ev).parse::<u16>());

    view! {
        <div>
            <span class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">
                {label}
            </span>
            <input
                type="number"
                name="port"
                id="port"
                on:input=on_port_input
                class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
                value=default
                required
            />
        </div>
        <div>
            <Show when=move || signal.get().is_err() fallback=move || view! { "" }.into_view()>
                <p class="mt-2 text-sm text-red-600 dark:text-red-500">Not a valid port number</p>
            </Show>
        </div>
    }
}

#[component]
pub fn NumberInput(
    signal: RwSignal<Result<u16, String>>,
    min: u16,
    label: &'static str,
) -> impl IntoView {
    let on_input = move |ev| {
        let val = match event_target_value(&ev).parse::<u16>() {
            Ok(v) if v < min => Err(format!("value cannot be smaller than {min}.")),
            Ok(v) => Ok(v),
            Err(err) => Err(err.to_string()),
        };
        signal.set(val);
    };

    view! {
        <div class="flex flex-row">
            <div class="basis-2/3">
                <span class="block mr-2 text-sm font-medium text-gray-900 dark:text-white">
                    {label}
                </span>
            </div>
            <div class="basis-1/3">
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
pub fn RewardsAddrInput(
    signal: RwSignal<Result<String, (String, String)>>,
    label: &'static str,
) -> impl IntoView {
    let validate_and_set = move |input_str: String| {
        let value = input_str
            .strip_prefix("0x")
            .unwrap_or(&input_str)
            .to_string();

        let res = if value.len() != REWARDS_ADDR_LENGTH {
            Err((
                "Unexpected length of rewards address".to_string(),
                input_str,
            ))
        } else if hex::decode(&value).is_err() {
            Err((
                "The address entered is not hex-encoded".to_string(),
                input_str,
            ))
        } else if value.to_lowercase() == value || value.to_uppercase() == value {
            // it's a non-checksummed address
            Ok(input_str)
        } else {
            // validate checksum
            match Address::parse_checksummed(&format!("0x{value}"), None) {
                Ok(_) => Ok(input_str),
                Err(_) => Err(("Checksum validation failed".to_string(), input_str)),
            }
        };

        signal.set(res);
    };

    view! {
        <div>
            <label
                for="rewards_addr"
                class="block mb-2 text-sm font-medium text-gray-900 dark:text-white"
            >
                {label}
            </label>

            <div class="flex items-center">
                <div class="relative w-full">
                    <input
                        type="text"
                        name="rewards_addr"
                        id="rewards_addr"
                        on:input=move |ev| validate_and_set(event_target_value(&ev))
                        required
                        class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
                        prop:value=move || match signal.get() {
                            Ok(s) => s,
                            Err((_, s)) => s,
                        }
                    />
                </div>

                <button
                    data-tooltip-target="tooltip-rewards_addr"
                    class="btn-node-action"
                    type="button"
                    on:click=move |_| {
                        spawn_local(async move {
                            if let Some(addr) = get_addr_from_metamask().await.as_string() {
                                validate_and_set(addr);
                            } else {
                                let prev = match signal.get_untracked() {
                                    Ok(s) => s,
                                    Err((_, s)) => s,
                                };
                                signal
                                    .set(
                                        Err((
                                            "Failed to retrieve address from Metamask".to_string(),
                                            prev,
                                        )),
                                    )
                            }
                        });
                    }
                >
                    <IconPasteAddr />
                </button>
                <div
                    id="tooltip-rewards_addr"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-900 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    <span>Retrieve address from Metamask</span>
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>
            </div>

            <Show when=move || signal.get().is_err() fallback=move || view! { "" }.into_view()>
                <p class="mt-2 text-sm text-red-600 dark:text-red-500">
                    {signal.get().err().map(|(e, _)| e)}
                </p>
            </Show>
        </div>
    }
}
