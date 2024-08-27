use super::{helpers::add_node_instance, icons::IconAddNode};

use leptos::*;
use std::num::ParseIntError;

// TODO: find next available port numbers by looking at already used ones
const DEFAULT_NODE_PORT: u16 = 12000;
const DEFAULT_RPC_API_PORT: u16 = 13000;

#[component]
pub fn AddNodeView() -> impl IntoView {
    let port = create_rw_signal(Ok(DEFAULT_NODE_PORT));
    let rpc_port = create_rw_signal(Ok(DEFAULT_RPC_API_PORT));
    let add_node = create_action(move |(port, rpc_port): &(u16, u16)| {
        let port = *port;
        let rpc_port = *rpc_port;
        async move {
            let _ = add_node_instance(port, rpc_port).await;
        }
    });

    view! {
        <div class="divider divider-center">
            <button class="btn" onclick="add_node_modal.showModal()">
                "Add node"
                <IconAddNode />
            </button>
            <dialog id="add_node_modal" class="modal">
                <div class="modal-box">
                    <h3 class="text-lg font-bold">Adding new node</h3>
                    <br />
                    <PortNumberInput signal=port default=DEFAULT_NODE_PORT label="Port number:" />
                    <PortNumberInput
                        signal=rpc_port
                        default=DEFAULT_RPC_API_PORT
                        label="RPC API port number:"
                    />

                    <div class="modal-action">
                        <form method="dialog">
                            <button class="btn mr-10">Cancel</button>
                            <button
                                class=move || {
                                    if port.get().is_ok() && rpc_port.get().is_ok() {
                                        "btn"
                                    } else {
                                        "btn btn-disabled"
                                    }
                                }
                                on:click=move |_| {
                                    if let (Ok(p), Ok(r)) = (port.get(), rpc_port.get()) {
                                        let _ = add_node.dispatch((p, r));
                                    }
                                }
                            >
                                Add node
                            </button>
                        </form>
                    </div>
                </div>
            </dialog>
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
        <label class="input input-bordered flex items-center gap-2 mb-6">
            {label} <input type="number" class="grow" value=default on:input=on_port_input />
            <ErrorBoundary fallback=|_errors| {
                view! {
                    // FIXME: it's not being shown
                    <p>"Not a valid port number"</p>
                }
            }>""</ErrorBoundary>
        </label>
    }
}
