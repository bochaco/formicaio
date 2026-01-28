use crate::{app::get_addr_from_metamask, server_api::parse_and_validate_addr};

use super::icons::*;

use leptos::{prelude::*, task::spawn_local};
use std::{net::IpAddr, num::ParseIntError, path::PathBuf};

#[component]
fn FormField(
    label: &'static str,
    help_msg: &'static str,
    #[prop(default = "right-0")] help_align: &'static str,
    #[prop(default = Signal::derive(|| None))] error: Signal<Option<String>>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="space-y-2">
            <label class="text-sm font-medium text-slate-300 flex items-center justify-between">
                <span>{label}</span>
                <div class="relative group flex items-center">
                    <IconHelpMsg />
                    <div class=format!(
                        "{help_align} absolute bottom-full mb-2 w-64 bg-slate-950 text-white text-xs font-normal text-left px-3 py-2 rounded-lg border border-slate-700 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 shadow-lg",
                    )>{help_msg}</div>
                </div>
            </label>
            {children()}
            <Show when=move || error.read().is_some()>
                <p class="text-rose-500 text-sm mt-1">{error.get()}</p>
            </Show>
        </div>
    }
}

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
        <FormField
            label
            help_msg
            error=Signal::derive(move || {
                signal.read().clone().map_err(|_| "Not a valid port number".to_string()).err()
            })
        >
            <input
                type="number"
                name=id
                id=id
                on:input=on_port_input
                class="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-2.5 text-sm focus:ring-1 focus:ring-indigo-500 focus:outline-none"
                value=default
                required
            />
        </FormField>
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
            Err::<IpAddr, std::net::AddrParseError>(err) => Err((input_str, err.to_string())),
        };
        signal.set(res);
    };

    view! {
        <FormField
            label
            help_msg
            error=Signal::derive(move || signal.read().clone().map_err(|(_, err)| err).err())
        >
            <input
                type="text"
                name="node_ip"
                id="node_ip"
                on:input=move |ev| validate_and_set(event_target_value(&ev))
                required
                prop:value=move || match signal.get() {
                    Ok(s) => s.to_string(),
                    Err((s, _)) => s,
                }
                class="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-2.5 text-sm focus:ring-1 focus:ring-indigo-500 focus:outline-none font-mono"
            />
        </FormField>
    }
}

#[component]
pub fn NumberInput(
    id: &'static str,
    signal: RwSignal<Result<u16, String>>,
    min: u16,
    label: &'static str,
    help_msg: &'static str,
    #[prop(default = "right-0")] help_align: &'static str,
) -> impl IntoView {
    let on_input = move |ev| {
        let val = match event_target_value(&ev).parse::<u16>() {
            Ok(v) if v < min => Err(format!("Value cannot be smaller than {min}.")),
            Ok(v) => Ok(v),
            Err(err) => Err(format!("Invalid value: {err}")),
        };
        signal.set(val);
    };

    view! {
        <FormField
            label
            help_msg
            help_align
            error=Signal::derive(move || signal.read().clone().err())
        >
            <input
                type="number"
                id=id
                on:input=on_input
                class="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-2.5 text-sm focus:ring-1 focus:ring-indigo-500 focus:outline-none"
                value=signal.get_untracked().unwrap_or_default()
                required
            />
        </FormField>
    }
}

#[component]
pub fn TextInput(
    signal: RwSignal<PathBuf>,
    label: &'static str,
    help_msg: &'static str,
) -> impl IntoView {
    view! {
        <FormField label help_msg>
            <input
                type="text"
                id="textinput"
                on:input=move |ev| {
                    signal.set(PathBuf::from(event_target_value(&ev)));
                }
                value=signal.get_untracked().display().to_string()
                class="w-full bg-slate-800 border rounded-lg px-4 py-2.5 text-sm focus:ring-1 focus:outline-none border-slate-700 focus:ring-indigo-500"
            />
        </FormField>
    }
}

#[component]
pub fn CheckboxInput(
    signal: RwSignal<bool>,
    id: &'static str,
    label: &'static str,
    help_msg: &'static str,
    #[prop(default = "right-0")] help_align: &'static str,
) -> impl IntoView {
    view! {
        <label class="flex items-center gap-1 cursor-pointer">
            <input
                type="checkbox"
                checked=move || signal.get()
                prop:checked=move || signal.get()
                id=id
                on:change=move |ev| { signal.set(event_target_checked(&ev)) }
                class="w-4 h-4 rounded bg-slate-800 border-slate-700 text-indigo-600 focus:ring-indigo-500"
            />
            <span class="text-sm text-slate-300">{label}</span>
            <div class="relative group flex items-center">
                <IconHelpMsg />
                <div class=format!(
                    "{help_align} absolute bottom-full mb-2 w-64 bg-slate-950 text-white text-xs font-normal text-left px-3 py-2 rounded-lg border border-slate-700 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 shadow-lg",
                )>{help_msg}</div>
            </div>
        </label>
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
            Err(err) => Err((input_str, err)),
        };
        signal.set(res);
    };

    view! {
        <FormField
            label
            help_msg="The Arbitrum One network address where network rewards will be sent."
            error=Signal::derive(move || signal.read().clone().map_err(|(_, err)| err).err())
        >
            <div class="relative group flex items-center">
                <input
                    type="text"
                    name="rewardsAddress"
                    placeholder="0x"
                    prop:value=move || match signal.get() {
                        Ok(s) => s,
                        Err((s, _)) => s,
                    }
                    class="w-full bg-slate-800 border rounded-lg px-4 py-2.5 text-sm focus:ring-1 focus:outline-none border-slate-700 focus:ring-indigo-500"
                    on:input=move |ev| validate_and_set(event_target_value(&ev))
                />
                <button
                    type="button"
                    class="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white"
                    on:click=move |_| {
                        spawn_local(async move {
                            if let Some(addr) = get_addr_from_metamask().await.as_string() {
                                validate_and_set(addr);
                            } else {
                                let prev = match signal.get_untracked() {
                                    Ok(s) => s,
                                    Err((s, _)) => s,
                                };
                                signal
                                    .set(
                                        Err((
                                            prev,
                                            "Failed to retrieve address from Metamask".to_string(),
                                        )),
                                    )
                            }
                        });
                    }
                >
                    <IconPasteAddr />
                </button>
                <div class="absolute bottom-full mb-2 right-0 bg-slate-950 text-white text-xs font-normal text-left px-3 py-2 rounded-lg border border-slate-700 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 shadow-lg">
                    "Retrieve address from Metamask"
                </div>
            </div>
        </FormField>
    }
}
