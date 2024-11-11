use super::{
    app::get_addr_from_metamask,
    helpers::add_node_instance,
    icons::{IconAddNode, IconCloseModal, IconPasteAddr},
};

use alloy::primitives::Address;
use leptos::*;
use std::num::ParseIntError;

// TODO: find next available port numbers by looking at already used ones
const DEFAULT_NODE_PORT: u16 = 12000;
const DEFAULT_RPC_API_PORT: u16 = 13000;
const DEFAULT_METRICS_PORT: u16 = 14000;

// Expected length of entered hex-encoded rewards address.
const REWARDS_ADDR_LENGTH: usize = 40;

#[component]
pub fn AddNodeView() -> impl IntoView {
    let port = create_rw_signal(Ok(DEFAULT_NODE_PORT));
    let rpc_port = create_rw_signal(Ok(DEFAULT_RPC_API_PORT));
    let metrics_port = create_rw_signal(Ok(DEFAULT_METRICS_PORT));
    let rewards_addr = create_rw_signal(Err((
        "Enter a rewards address".to_string(),
        "0x".to_string(),
    )));
    let add_node = create_action(
        move |(port, rpc_port, metrics_port, rewards_addr): &(u16, u16, u16, String)| {
            let port = *port;
            let rpc_port = *rpc_port;
            let metrics_port = *metrics_port;
            let rewards_addr = rewards_addr
                .strip_prefix("0x")
                .unwrap_or(rewards_addr)
                .to_string();
            async move {
                let _ = add_node_instance(port, rpc_port, metrics_port, rewards_addr).await;
            }
        },
    );
    let modal_visibility = create_rw_signal(false);

    view! {
        <div class="divider divider-center">
            <button type="button" class="btn-add-node" on:click=move |_| modal_visibility.set(true)>
                <IconAddNode />
                Add node
            </button>

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
                                Adding a node
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
                            <form class="space-y-4">
                                <PortNumberInput
                                    signal=port
                                    default=DEFAULT_NODE_PORT
                                    label="Port number:"
                                />
                                <PortNumberInput
                                    signal=rpc_port
                                    default=DEFAULT_RPC_API_PORT
                                    label="RPC API port number:"
                                />
                                <PortNumberInput
                                    signal=metrics_port
                                    default=DEFAULT_METRICS_PORT
                                    label="Node metrics port number:"
                                />
                                <RewardsAddrInput signal=rewards_addr label="Rewards address:" />

                                <button
                                    type="button"
                                    disabled=move || {
                                        port.get().is_err() || rpc_port.get().is_err()
                                            || metrics_port.get().is_err()
                                            || rewards_addr.get().is_err()
                                    }
                                    on:click=move |_| {
                                        modal_visibility.set(false);
                                        if let (Ok(p), Ok(r), Ok(m), Ok(addr)) = (
                                            port.get(),
                                            rpc_port.get(),
                                            metrics_port.get(),
                                            rewards_addr.get(),
                                        ) {
                                            add_node.dispatch((p, r, m, addr));
                                        }
                                    }
                                    class="text-white bg-gray-800 hover:bg-gray-900 focus:outline-none focus:ring-4 focus:ring-gray-300 font-medium rounded-lg text-sm px-5 py-2.5 me-2 mb-2 dark:bg-gray-800 dark:hover:bg-gray-700 dark:focus:ring-gray-700 dark:border-gray-700"
                                >
                                    Add node
                                </button>
                            </form>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn PortNumberInput(
    signal: RwSignal<Result<u16, ParseIntError>>,
    default: u16,
    label: &'static str,
) -> impl IntoView {
    let on_port_input = move |ev| signal.set(event_target_value(&ev).parse::<u16>());

    view! {
        <div>
            <label for="port" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">
                {label}
            </label>

            <input
                type="number"
                name="port"
                id="port"
                on:input=on_port_input
                class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
                value=default
                required
            />
            <Show when=move || signal.get().is_err() fallback=move || view! { "" }.into_view()>
                <p class="mt-2 text-sm text-red-600 dark:text-red-500">Not a valid port number</p>
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
            match Address::parse_checksummed(&input_str, None) {
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
