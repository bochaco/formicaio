use crate::types::NodeOpts;

use super::{
    form_inputs::{
        CheckboxInput, IpAddrInput, NumberInput, PortNumberInput, RewardsAddrInput, TextInput,
    },
    helpers::{add_node_instances, show_error_alert_msg},
    icons::IconCancel,
};

use leptos::{logging, prelude::*};
use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

const DEFAULT_NODE_IP: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
const DEFAULT_NODE_PORT: u16 = 12000;
const DEFAULT_METRICS_PORT: u16 = 14000;

#[component]
pub fn AddNodesForm(is_open: RwSignal<bool>) -> impl IntoView {
    let active_tab = RwSignal::new(0);

    view! {
        <div class="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/90 backdrop-blur-sm animate-in fade-in duration-300">
            <div class="bg-slate-900 border border-slate-800 w-full max-w-lg rounded-2xl overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
                <div class="p-6 border-b border-slate-800 flex items-center justify-between">
                    <h3 class="text-xl font-bold">Add nodes</h3>
                    <button
                        on:click=move |_| is_open.set(false)
                        class="p-2 text-slate-500 hover:text-white transition-colors"
                    >
                        <IconCancel />
                    </button>
                </div>

                <div class="px-8 pt-6">
                    <div class="flex items-center border-b border-slate-800">
                        <TabButton label="Basic" active_tab tab_index=0 />
                        <TabButton label="Advanced" active_tab tab_index=1 />
                    </div>
                </div>

                <AddNodeTabs is_open active_tab />
            </div>
        </div>
    }
}

#[component]
fn AddNodeTabs(is_open: RwSignal<bool>, active_tab: RwSignal<u8>) -> impl IntoView {
    let node_ip = RwSignal::new(Ok(DEFAULT_NODE_IP));
    let port = RwSignal::new(Ok(DEFAULT_NODE_PORT));
    let metrics_port = RwSignal::new(Ok(DEFAULT_METRICS_PORT));
    let count = RwSignal::new(Ok(1));
    let rewards_addr = RwSignal::new(Err((
        "0x".to_string(),
        "Enter a rewards address".to_string(),
    )));
    let upnp = RwSignal::new(true);
    let reachability_check = RwSignal::new(true);
    let auto_start = RwSignal::new(false);
    let interval = RwSignal::new(Ok(60));
    let data_dir_path = RwSignal::new(PathBuf::default());

    let add_node = Action::new(move |(node_opts, count, interval): &(NodeOpts, u16, u64)| {
        let node_opts = node_opts.clone();
        let count = *count;
        let interval = *interval;
        async move {
            if let Err(err) = add_node_instances(node_opts, count, interval).await {
                let msg = format!("Failed to create node/s: {err}");
                logging::error!("[ERROR] {msg}");
                show_error_alert_msg(msg);
            }
        }
    });

    view! {
        <div class="p-8 space-y-6">
            <span hidden=move || active_tab.read() != 0>
                <div class="space-y-4 animate-in fade-in duration-300">
                    <PortNumberInput
                        id="port"
                        signal=port
                        default=DEFAULT_NODE_PORT
                        label="Port number (range start):"
                        help_msg="Node port number (range start when creating multiple nodes)."
                    />
                    <PortNumberInput
                        id="metrics_port"
                        signal=metrics_port
                        default=DEFAULT_METRICS_PORT
                        label="Node metrics port number (range start):"
                        help_msg="Node metrics port number (range start when creating multiple nodes)."
                    />
                    <RewardsAddrInput signal=rewards_addr label="Rewards address:" />
                    <div class="grid grid-cols-2 gap-4">
                        <NumberInput
                            id="nodes_count"
                            signal=count
                            min=1
                            label="Number of nodes:"
                            help_msg="A batch will be created if the number is greater than one."
                            help_align="left-0"
                        />
                        <NumberInput
                            id="create_interval"
                            signal=interval
                            min=0
                            label="Delay (in seconds):"
                            help_msg="The time to wait between creating each node in a batch."
                        />
                    </div>
                    <div class="flex items-center justify-between pt-2">
                        <CheckboxInput
                            signal=auto_start
                            id="auto_start"
                            label="Auto start"
                            help_msg="Automatically starts nodes upon creation."
                            help_align="left-0"
                        />
                        <CheckboxInput
                            signal=upnp
                            id="upnp"
                            label="Try UPnP"
                            help_msg="Try to use UPnP to open a port in the home router and allow incoming connections. If your router does not support UPnP, your node/s may struggle to connect to any peers. In this situation, create new node/s with UPnP disabled."
                        />
                    </div>
                </div>
            </span>

            <span hidden=move || active_tab.read() != 1>
                <div class="space-y-4 animate-in fade-in duration-300">
                    <IpAddrInput
                        signal=node_ip
                        label="Node IP listening address:"
                        help_msg="Specify the IP to listen on. The special value `0.0.0.0` binds to all IPv4 network interfaces available, while `::` binds to all IP v4 and v6 network interfaces available."
                    />
                    <TextInput
                        signal=data_dir_path
                        label="Data directory path (optional):"
                        help_msg="Custom path for storing node data files. If the path is not absolute, it will be created as a subdirectory inside the default data directory. Leave empty to use the default data directory."
                    />
                </div>
            </span>
        </div>

        <div class="p-6 bg-slate-950 border-t border-slate-800 flex justify-end">
            <button
                prop:disabled=move || {
                    port.read().is_err() || metrics_port.read().is_err() || count.read().is_err()
                        || rewards_addr.read().is_err() || interval.read().is_err()
                }
                on:click=move |_| {
                    if let (Ok(ip), Ok(p), Ok(m), Ok(c), Ok(addr), Ok(i)) = (
                        node_ip.get(),
                        port.get(),
                        metrics_port.get(),
                        count.get(),
                        rewards_addr.get(),
                        interval.get(),
                    ) {
                        is_open.set(false);
                        let node_opts = NodeOpts {
                            node_ip: ip,
                            port: p,
                            metrics_port: m,
                            rewards_addr: addr.strip_prefix("0x").unwrap_or(&addr).to_string(),
                            upnp: upnp.get(),
                            reachability_check: reachability_check.get(),
                            node_logs: true,
                            auto_start: auto_start.get(),
                            data_dir_path: data_dir_path.get(),
                        };
                        add_node.dispatch((node_opts, c, i as u64));
                    }
                }
                class="bg-indigo-600 hover:bg-indigo-500 text-white px-6 py-2.5 rounded-lg font-bold transition-all shadow-lg shadow-indigo-500/20 flex items-center gap-2 disabled:opacity-75 disabled:bg-slate-600 disabled:text-slate-400 disabled:shadow-none disabled:cursor-not-allowed"
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
        </div>
    }
}

#[component]
fn TabButton(label: &'static str, active_tab: RwSignal<u8>, tab_index: u8) -> impl IntoView {
    let is_active = move || active_tab.get() == tab_index;

    view! {
        <button
            type="button"
            on:click=move |_| active_tab.set(tab_index)
            class=move || {
                format!(
                    "px-4 py-2 text-sm font-medium transition-colors relative {}",
                    if is_active() { "text-white" } else { "text-slate-400 hover:text-white" },
                )
            }
        >
            {label}
            <Show when=move || is_active()>
                <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-indigo-500 rounded-full" />
            </Show>
        </button>
    }
}
