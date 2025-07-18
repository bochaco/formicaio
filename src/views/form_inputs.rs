use crate::{app::get_addr_from_metamask, server_api::parse_and_validate_addr};

use super::icons::*;

use leptos::{prelude::*, task::spawn_local};
use std::{net::IpAddr, num::ParseIntError, path::PathBuf};

#[component]
pub fn PortNumberInput(
    id: &'static str,
    signal: RwSignal<Result<u16, ParseIntError>>,
    default: u16,
    label: &'static str,
    help_msg: &'static str,
) -> impl IntoView {
    let on_port_input = move |ev| signal.set(event_target_value(&ev).parse::<u16>());

    view! {
        <div class="relative w-full">
            <div class="flex flex-row">
                <div class="basis-7/12">
                    <label for=id class="form-label">
                        {label}
                    </label>
                </div>
                <div class="basis-4/12">
                    <input
                        type="number"
                        name=id
                        id=id
                        on:input=on_port_input
                        class="form-input-box"
                        value=default
                        required
                    />
                </div>
                <div class="basis-1/12">
                    <button
                        data-popover-target=format!("popover-{id}")
                        data-popover-placement="bottom-end"
                        type="button"
                        class="btn-node-action"
                    >
                        <IconHelpMsg />
                        <span class="sr-only">Show information</span>
                    </button>

                    <div
                        data-popover
                        id=format!("popover-{id}")
                        role="tooltip"
                        class="absolute z-10 invisible inline-block text-sm text-white transition-opacity duration-300 bg-gray-800 border border-gray-200 rounded-lg shadow-xs opacity-0 w-72 dark:bg-gray-800 dark:border-gray-600 dark:text-gray-400"
                    >
                        <div class="p-3 space-y-2">{help_msg}</div>
                        <div data-popper-arrow></div>
                    </div>
                </div>
            </div>
            <div>
                <Show when=move || signal.read().is_err() fallback=move || view! { "" }.into_view()>
                    <p class="form-invalid-input-msg">Not a valid port number</p>
                </Show>
            </div>
        </div>
    }
}

#[component]
pub fn IpAddrInput(
    signal: RwSignal<Result<IpAddr, (String, String)>>,
    label: &'static str,
    help_msg: &'static str,
) -> impl IntoView {
    let validate_and_set = move |input_str: String| {
        let res = match input_str.parse() {
            Ok(addr) => Ok(addr),
            Err::<IpAddr, std::net::AddrParseError>(err) => Err((err.to_string(), input_str)),
        };

        signal.set(res);
    };

    view! {
        <div>
            <label for="node_ip" class="form-label">
                {label}
            </label>

            <div class="flex items-center">
                <div class="relative w-full">
                    <input
                        type="text"
                        name="node_ip"
                        id="node_ip"
                        on:input=move |ev| validate_and_set(event_target_value(&ev))
                        required
                        class="form-input-box"
                        prop:value=move || match signal.get() {
                            Ok(s) => s.to_string(),
                            Err((_, s)) => s,
                        }
                    />
                </div>

                <button
                    data-popover-target="popover-node_ip"
                    data-popover-placement="bottom-end"
                    type="button"
                    class="btn-node-action"
                >
                    <IconHelpMsg />
                    <span class="sr-only">Show information</span>
                </button>

                <div
                    data-popover
                    id="popover-node_ip"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block text-sm text-white transition-opacity duration-300 bg-gray-800 border border-gray-200 rounded-lg shadow-xs opacity-0 w-72 dark:bg-gray-800 dark:border-gray-600 dark:text-gray-400"
                >
                    <div class="p-3 space-y-2">{help_msg}</div>
                    <div data-popper-arrow></div>
                </div>
            </div>

            <Show when=move || signal.read().is_err() fallback=move || view! { "" }.into_view()>
                <p class="form-invalid-input-msg">{signal.get().err().map(|(e, _)| e)}</p>
            </Show>
        </div>
    }
}

#[component]
pub fn NumberInput(
    id: &'static str,
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
                <label for=id class="form-label">
                    {label}
                </label>
            </div>
            <div class="basis-1/3">
                <input
                    type="number"
                    id=id
                    on:input=on_input
                    class="form-input-box"
                    value=signal.get_untracked().unwrap_or_default()
                    required
                />
            </div>
        </div>
        <div>
            <Show when=move || signal.read().is_err() fallback=move || view! { "" }.into_view()>
                <p class="ml-2 text-sm text-red-600 dark:text-red-500">
                    "Invalid value: " {signal.get().err()}
                </p>
            </Show>
        </div>
    }
}

#[component]
pub fn TextInput(
    signal: RwSignal<PathBuf>,
    label: &'static str,
    help_msg: &'static str,
) -> impl IntoView {
    view! {
        <div>
            <label for="textinput" class="form-label">
                {label}
            </label>

            <div class="flex items-center">
                <div class="relative w-full">
                    <input
                        type="text"
                        id="textinput"
                        on:input=move |ev| {
                            signal.set(PathBuf::from(event_target_value(&ev)));
                        }
                        class="form-input-box"
                        value=signal.get_untracked().display().to_string()
                    />
                </div>
                <button
                    data-popover-target="popover-text"
                    data-popover-placement="top-end"
                    type="button"
                    class="btn-node-action"
                >
                    <IconHelpMsg />
                    <span class="sr-only">Show information</span>
                </button>

                <div
                    data-popover
                    id="popover-text"
                    role="tooltip"
                    class="absolute z-10 invisible inline-block text-sm text-white transition-opacity duration-300 bg-gray-800 border border-gray-200 rounded-lg shadow-xs opacity-0 w-72 dark:bg-gray-800 dark:border-gray-600 dark:text-gray-400"
                >
                    <div class="p-3 space-y-2">{help_msg}</div>
                    <div data-popper-arrow></div>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn CheckboxInput(
    signal: RwSignal<bool>,
    id: &'static str,
    label: &'static str,
    help_msg: &'static str,
) -> impl IntoView {
    let checked = signal.get();
    view! {
        <div class="flex items-center">
            <input
                checked=checked
                prop:checked=move || signal.get()
                id=id
                type="checkbox"
                on:change=move |ev| { signal.set(event_target_checked(&ev)) }
                class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-400"
            />
            <label for=id class="ms-2 text-sm font-medium text-gray-900 dark:text-gray-300">
                {label}
            </label>

            <button
                data-popover-target=format!("popover-{id}")
                data-popover-placement="top-end"
                type="button"
                class="btn-node-action"
            >
                <IconHelpMsg />
                <span class="sr-only">Show information</span>
            </button>

            <div
                data-popover
                id=format!("popover-{id}")
                role="tooltip"
                class="absolute z-10 invisible inline-block text-sm text-white transition-opacity duration-300 bg-gray-800 border border-gray-200 rounded-lg shadow-xs opacity-0 w-72 dark:bg-gray-800 dark:border-gray-600 dark:text-gray-400"
            >
                <div class="p-3 space-y-2">{help_msg}</div>
                <div data-popper-arrow></div>
            </div>
        </div>
    }
}

#[component]
pub fn RewardsAddrInput(
    signal: RwSignal<Result<String, (String, String)>>,
    label: &'static str,
) -> impl IntoView {
    let validate_and_set = move |input_str: String| {
        let res = match parse_and_validate_addr(&input_str) {
            Ok(_) => Ok(input_str),
            Err(err) => Err((err, input_str)),
        };

        signal.set(res);
    };

    view! {
        <div>
            <label for="rewards_addr" class="form-label">
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
                        class="form-input-box"
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
                    class="absolute z-10 invisible inline-block px-3 py-2 text-sm font-medium text-white transition-opacity duration-300 bg-gray-800 rounded-lg shadow-sm opacity-0 tooltip dark:bg-gray-700"
                >
                    <span>Retrieve address from Metamask</span>
                    <div class="tooltip-arrow" data-popper-arrow></div>
                </div>
            </div>

            <Show when=move || signal.read().is_err() fallback=move || view! { "" }.into_view()>
                <p class="form-invalid-input-msg">{signal.get().err().map(|(e, _)| e)}</p>
            </Show>
        </div>
    }
}
