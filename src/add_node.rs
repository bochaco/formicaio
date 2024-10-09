use super::{
    helpers::add_node_instance,
    icons::{IconAddNode, IconCloseModal},
};

use leptos::*;
use std::num::ParseIntError;

// TODO: find next available port numbers by looking at already used ones
const DEFAULT_NODE_PORT: u16 = 12000;
const DEFAULT_RPC_API_PORT: u16 = 13000;

#[component]
pub fn AddNodeView() -> impl IntoView {
    let port = create_rw_signal(Ok(DEFAULT_NODE_PORT));
    let rpc_port = create_rw_signal(Ok(DEFAULT_RPC_API_PORT));
    let beta_tester_id = create_rw_signal("".to_string());
    let add_node = create_action(
        move |(port, rpc_port, beta_tester_id): &(u16, u16, String)| {
            let port = *port;
            let rpc_port = *rpc_port;
            let beta_tester_id = beta_tester_id.to_string();
            async move {
                let _ = add_node_instance(port, rpc_port, beta_tester_id).await;
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
                <div class="relative p-4 w-full max-w-md max-h-full">
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
                                <BetaTesterInput signal=beta_tester_id label="Beta tester id:" />

                                <button
                                    type="button"
                                    disabled=move || port.get().is_err() || rpc_port.get().is_err()
                                    on:click=move |_| {
                                        modal_visibility.set(false);
                                        if let (Ok(p), Ok(r), t) = (
                                            port.get(),
                                            rpc_port.get(),
                                            beta_tester_id.get(),
                                        ) {
                                            add_node.dispatch((p, r, t));
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
pub fn BetaTesterInput(signal: RwSignal<String>, label: &'static str) -> impl IntoView {
    let on_input = move |ev| signal.set(event_target_value(&ev));

    view! {
        <div>
            <label
                for="beta_tester_id"
                class="block mb-2 text-sm font-medium text-gray-900 dark:text-white"
            >
                {label}
            </label>
            <input
                type="text"
                name="beta_tester_id"
                id="beta_tester_id"
                on:input=on_input
                class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white"
            />
        </div>
    }
}
