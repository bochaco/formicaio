use crate::{helpers::add_node_instances, types::NodeOpts};

use super::form_inputs::{
    CheckboxInput, IpAddrInput, NumberInput, PortNumberInput, RewardsAddrInput, TextInput,
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
pub fn AddNodesForm(modal_visibility: RwSignal<bool>) -> impl IntoView {
    let node_ip = RwSignal::new(Ok(DEFAULT_NODE_IP));
    let port = RwSignal::new(Ok(DEFAULT_NODE_PORT));
    let metrics_port = RwSignal::new(Ok(DEFAULT_METRICS_PORT));
    let count = RwSignal::new(Ok(1));
    let rewards_addr = RwSignal::new(Err((
        "Enter a rewards address".to_string(),
        "0x".to_string(),
    )));
    let home_network = RwSignal::new(true);
    let upnp = RwSignal::new(true);
    let auto_start = RwSignal::new(false);
    let interval = RwSignal::new(Ok(60));
    let data_dir_path = RwSignal::new(PathBuf::default());

    let add_node = Action::new(move |(node_opts, count, interval): &(NodeOpts, u16, u64)| {
        let node_opts = node_opts.clone();
        let count = *count;
        let interval = *interval;
        async move {
            if let Err(err) = add_node_instances(node_opts, count, interval).await {
                logging::error!("Failed to create node/s: {err}");
            }
        }
    });

    view! {
        <form class="space-y-4">
            <IpAddrInput
                signal=node_ip
                label="Node IP listening address:"
                help_msg="Specify the IP to listen on. The special value `0.0.0.0` binds to all IPv4 network interfaces available, while `::` binds to all IP v4 and v6 network interfaces available."
            />
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
            <NumberInput
                id="nodes_count"
                signal=count
                min=1
                label="Number of nodes (a batch will be created if the number is greater than one):"
            />
            <NumberInput
                id="create_interval"
                signal=interval
                min=0
                label="Delay (in seconds) between the creation of each node in the batch:"
            />
            <TextInput
                signal=data_dir_path
                label="Data directory path (optional):"
                help_msg="Custom path for storing node data files. If the path is not absolute, it will be created as a subdirectory inside the default data directory. Leave empty to use the default data directory."
            />
            <div class="flex flex-row">
                <div class="basis-4/12">
                    <CheckboxInput
                        signal=auto_start
                        id="auto_start"
                        label="Auto start"
                        help_msg="Automatically starts nodes upon creation."
                    />
                </div>
                <div class="basis-5/12">
                    <CheckboxInput
                        signal=home_network
                        id="home-network"
                        label="Home network"
                        help_msg="Enables the mode to run as a relay client if it is behind a NAT and it is not externally reachable."
                    />
                </div>
                <div class="basis-4/12">
                    <CheckboxInput
                        signal=upnp
                        id="upnp"
                        label="Try UPnP"
                        help_msg="Try to use UPnP to open a port in the home router and allow incoming connections. If your router does not support UPnP, your node/s may struggle to connect to any peers. In this situation, create new node/s with UPnP disabled."
                    />
                </div>
            </div>

            <button
                type="button"
                disabled=move || {
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
                        modal_visibility.set(false);
                        let node_opts = NodeOpts {
                            node_ip: ip,
                            port: p,
                            metrics_port: m,
                            rewards_addr: addr.strip_prefix("0x").unwrap_or(&addr).to_string(),
                            home_network: home_network.get(),
                            upnp: upnp.get(),
                            node_logs: true,
                            auto_start: auto_start.get(),
                            data_dir_path: data_dir_path.get(),
                        };
                        add_node.dispatch((node_opts, c, i as u64));
                    }
                }
                class="btn-modal"
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
