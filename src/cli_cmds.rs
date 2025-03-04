use crate::{helpers::truncated_balance_str, server_api::*, server_api_types::*};

use alloy_primitives::{utils::format_units, Address};
use chrono::{DateTime, Local, Utc};
use leptos::prelude::ServerFnError;
use prettytable::{format, row, Table};
use std::{
    io::{Error, Write},
    net::SocketAddr,
};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "formicaio", about = "CLI interface for Formicaio application.")]
pub struct CliCmds {
    /// Backend IP address and port.
    #[structopt(long, global = true)]
    pub addr: Option<SocketAddr>,
    #[structopt(subcommand)]
    pub sub_cmds: CliSubCmds,
}

#[derive(Debug, PartialEq, StructOpt)]
pub enum CliSubCmds {
    /// Start Formicaio backend application
    Start,
    #[structopt(flatten)]
    CliCommands(CliCommands),
}

#[derive(Debug, PartialEq, StructOpt)]
pub enum CliCommands {
    /// Nodes commands
    Nodes(NodesSubcommands),
    /// Stats commands
    Stats,
    /// Batches commands
    Batches(BatchesSubcommands),
    /// Settings commands
    Settings(SettingsSubcommands),
}

#[derive(Debug, PartialEq, StructOpt)]
pub enum NodesSubcommands {
    /// List existing nodes
    Ls {
        /// List nodes which match any of the provided id/s.
        /// Multiple ids can be provided, e.g. '--id 726d63514a6d --id 59566d447968'.
        #[structopt(long)]
        id: Option<Vec<NodeId>>,
        /// List only nodes wich match any of the provided status.
        /// Multiple status can be provided, e.g. '--status active --status restarting'.
        #[structopt(long, parse(try_from_str = parse_node_status))]
        status: Option<Vec<NodeStatus>>,
        /// Display all details of each listed node
        #[structopt(short, long, global = true)]
        extended: bool,
    },
    /// Create nodes instances
    Create(NodeOptsCmd),
    /// Remove existing nodes
    Remove {
        /// Remove nodes which match any of the provided id/s.
        /// Multiple ids can be provided, e.g. '--id 726d63514a6d --id 59566d447968'.
        #[structopt(long)]
        id: Vec<NodeId>,
        /// Interval between each action.
        #[structopt(long, default_value = "0")]
        interval: u64,
    },
    /// Start nodes
    Start {
        /// Start nodes which match any of the provided id/s.
        /// Multiple ids can be provided, e.g. '--id 726d63514a6d --id 59566d447968'.
        #[structopt(long)]
        id: Vec<NodeId>,
        /// Interval between each action.
        #[structopt(long, default_value = "0")]
        interval: u64,
    },
    /// Stop nodes
    Stop {
        /// Stop nodes which match any of the provided id/s.
        /// Multiple ids can be provided, e.g. '--id 726d63514a6d --id 59566d447968'.
        #[structopt(long)]
        id: Vec<NodeId>,
        /// Interval between each action.
        #[structopt(long, default_value = "0")]
        interval: u64,
    },
    /// Recycle nodes
    Recycle {
        /// Recycle nodes which match any of the provided id/s.
        /// Multiple ids can be provided, e.g. '--id 726d63514a6d --id 59566d447968'.
        #[structopt(long)]
        id: Vec<NodeId>,
        /// Interval between each action.
        #[structopt(long, default_value = "0")]
        interval: u64,
    },
    /// Upgrade nodes
    Upgrade {
        /// Upgrade nodes which match any of the provided id/s.
        /// Multiple ids can be provided, e.g. '--id 726d63514a6d --id 59566d447968'.
        #[structopt(long)]
        id: Vec<NodeId>,
        /// Interval between each action.
        #[structopt(long, default_value = "0")]
        interval: u64,
    },
}

#[derive(Debug, PartialEq, StructOpt)]
pub struct NodeOptsCmd {
    /// Node port number (range start when creating multiple nodes)
    #[structopt(long)]
    port: u16,
    /// Node metrics port number (range start when creating multiple nodes)
    #[structopt(long)]
    metrics_port: u16,
    /// Rewards address
    #[structopt(long, parse(try_from_str = parse_and_validate_addr))]
    rewards_addr: Address,
    /// Home network: the node is operating from a home network
    /// and situated behind a NAT without port forwarding capabilities.
    /// If this is not enabled and you're behind a NAT, the node is terminated.
    #[structopt(long)]
    home_network: bool,
    /// Try to use UPnP to open a port in the home router and allow incoming connections.
    /// If your router does not support UPnP, your node/s may struggle to connect to any peers. In this situation, create new node/s with UPnP disabled.
    #[structopt(long)]
    upnp: bool,
    /// Automatically starts nodes upon creation.
    #[structopt(long)]
    auto_start: bool,
    /// Number of nodes to create (a batch will be created if the number is greater than one).
    #[structopt(long, default_value = "1")]
    count: u16,
    /// Delay (in seconds) between the creation of each node in the batch.
    #[structopt(long, default_value = "0")]
    interval: u64,
}

// Parser for the node status CLI args
fn parse_node_status(src: &str) -> Result<NodeStatus, serde_json::Error> {
    if let Some(first_char) = src.chars().next() {
        let s = format!("\"{}{}\"", first_char.to_uppercase(), &src[1..]);
        serde_json::from_str(&s)
    } else {
        serde_json::from_str(src)
    }
}

#[derive(Debug, PartialEq, StructOpt)]
pub enum BatchesSubcommands {
    /// List running and scheduled nodes actions batches
    Ls,
    /// Cancel batch
    Cancel {
        /// Batch Id to cancel
        batch_id: u16,
    },
}

#[derive(Debug, PartialEq, StructOpt)]
pub enum SettingsSubcommands {
    /// List current settings values
    Ls,
}

#[derive(Debug)]
pub enum CliCmdResponse {
    Nodes(NodeList, bool),
    NodeCreated(NodeInstanceInfo),
    Stats(Stats),
    Batches(Vec<NodesActionsBatch>),
    Settings(AppSettings),
    BatchCreated(u16),
    Success,
}

impl CliCommands {
    /// Process command, sending corresponding request to Formicaio backend server
    pub async fn process_command(&self) -> Result<CliCmdResponse, ServerFnError> {
        let res = match &self {
            CliCommands::Nodes(NodesSubcommands::Ls {
                id,
                status,
                extended,
            }) => CliCmdResponse::Nodes(
                nodes_instances(Some(QueryFilter {
                    node_ids: id.clone(),
                    status: status.clone(),
                }))
                .await?
                .nodes,
                *extended,
            ),
            CliCommands::Nodes(NodesSubcommands::Create(node_opts_cmd)) => {
                let node_opts = NodeOpts {
                    port: node_opts_cmd.port,
                    metrics_port: node_opts_cmd.metrics_port,
                    rewards_addr: node_opts_cmd.rewards_addr.to_string(),
                    home_network: node_opts_cmd.home_network,
                    upnp: node_opts_cmd.upnp,
                    node_logs: true,
                    auto_start: node_opts_cmd.auto_start,
                };
                if node_opts_cmd.count > 1 {
                    let batch_id = node_action_batch(
                        BatchType::Create {
                            node_opts,
                            count: node_opts_cmd.count,
                        },
                        node_opts_cmd.interval,
                    )
                    .await?;
                    CliCmdResponse::BatchCreated(batch_id)
                } else {
                    let new_node = create_node_instance(node_opts).await?;
                    CliCmdResponse::NodeCreated(new_node)
                }
            }
            CliCommands::Nodes(NodesSubcommands::Remove { id, interval }) => {
                if id.len() > 1 {
                    let batch_id =
                        node_action_batch(BatchType::Remove(id.clone()), *interval).await?;
                    return Ok(CliCmdResponse::BatchCreated(batch_id));
                }
                if let Some(node_id) = id.first() {
                    delete_node_instance(node_id.clone()).await?;
                }
                CliCmdResponse::Success
            }
            CliCommands::Nodes(NodesSubcommands::Start { id, interval }) => {
                if id.len() > 1 {
                    let batch_id =
                        node_action_batch(BatchType::Start(id.clone()), *interval).await?;
                    return Ok(CliCmdResponse::BatchCreated(batch_id));
                }
                if let Some(node_id) = id.first() {
                    start_node_instance(node_id.clone()).await?;
                }
                CliCmdResponse::Success
            }
            CliCommands::Nodes(NodesSubcommands::Stop { id, interval }) => {
                if id.len() > 1 {
                    let batch_id =
                        node_action_batch(BatchType::Stop(id.clone()), *interval).await?;
                    return Ok(CliCmdResponse::BatchCreated(batch_id));
                }
                if let Some(node_id) = id.first() {
                    stop_node_instance(node_id.clone()).await?;
                }
                CliCmdResponse::Success
            }
            CliCommands::Nodes(NodesSubcommands::Recycle { id, interval }) => {
                if id.len() > 1 {
                    let batch_id =
                        node_action_batch(BatchType::Recycle(id.clone()), *interval).await?;
                    return Ok(CliCmdResponse::BatchCreated(batch_id));
                }
                if let Some(node_id) = id.first() {
                    recycle_node_instance(node_id.clone()).await?;
                }
                CliCmdResponse::Success
            }
            CliCommands::Nodes(NodesSubcommands::Upgrade { id, interval }) => {
                if id.len() > 1 {
                    let batch_id =
                        node_action_batch(BatchType::Upgrade(id.clone()), *interval).await?;
                    return Ok(CliCmdResponse::BatchCreated(batch_id));
                }
                if let Some(node_id) = id.first() {
                    upgrade_node_instance(node_id.clone()).await?;
                }
                CliCmdResponse::Success
            }
            CliCommands::Stats => CliCmdResponse::Stats(nodes_instances(None).await?.stats),
            CliCommands::Batches(BatchesSubcommands::Ls) => {
                CliCmdResponse::Batches(nodes_instances(None).await?.scheduled_batches)
            }
            CliCommands::Batches(BatchesSubcommands::Cancel { batch_id }) => {
                cancel_batch(*batch_id).await?;
                CliCmdResponse::Success
            }
            CliCommands::Settings(SettingsSubcommands::Ls) => {
                CliCmdResponse::Settings(get_settings().await?)
            }
        };

        Ok(res)
    }

    #[cfg(feature = "ssr")]
    pub async fn send_request(&self, addr: SocketAddr) -> Result<CliCmdResponse, String> {
        let api_url = format!("http://{addr}/api");

        match &self {
            CliCommands::Nodes(NodesSubcommands::Ls {
                id,
                status,
                extended,
            }) => {
                let filter = QueryFilter {
                    node_ids: id.clone(),
                    status: status.clone(),
                };
                // TODO: use some crate which performs this serialisation
                let mut body = "".to_string();
                if let Some(node_ids) = filter.node_ids {
                    for (i, id) in node_ids.iter().enumerate() {
                        if i > 0 {
                            body = format!("{body}&");
                        }
                        body = format!("{body}filter[node_ids][{i}]={id}");
                    }
                }
                if let Some(status) = filter.status {
                    for (i, s) in status.iter().enumerate() {
                        if i > 0 || !body.is_empty() {
                            body = format!("{body}&");
                        }
                        body = format!("{body}filter[status][{i}]={s}");
                    }
                }

                send_req(&format!("{api_url}/nodes/list"), Some(body))
                    .await
                    .map(|res: NodesInstancesInfo| CliCmdResponse::Nodes(res.nodes, *extended))
            }
            CliCommands::Nodes(NodesSubcommands::Create(opts)) => {
                if opts.count > 1 {
                    // TODO: use some crate which performs this serialisation
                    let body = format!(
                        "batch_type[Create][node_opts][port]={}&batch_type[Create][node_opts][metrics_port]={}&batch_type[Create][node_opts][rewards_addr]={}&batch_type[Create][node_opts][home_network]={}&batch_type[Create][node_opts][upnp]={}&batch_type[Create][node_opts][node_logs]={}&batch_type[Create][node_opts][auto_start]={}&batch_type[Create][count]={}&interval_secs={}",
                        opts.port,
                        opts.metrics_port,
                        opts.rewards_addr,
                        opts.home_network,
                        opts.upnp,
                        true,
                        opts.auto_start,
                        opts.count,
                        opts.interval
                    );

                    let batch_id =
                        send_req::<u16>(&format!("{api_url}/batch/new"), Some(body)).await?;
                    Ok(CliCmdResponse::BatchCreated(batch_id))
                } else {
                    // TODO: use some crate which performs this serialisation
                    let body = format!(
                        "node_opts[port]={}&node_opts[metrics_port]={}&node_opts[rewards_addr]={}&node_opts[home_network]={}&node_opts[upnp]={}&node_opts[node_logs]={}&node_opts[auto_start]={}",
                        opts.port,
                        opts.metrics_port,
                        opts.rewards_addr,
                        opts.home_network,
                        opts.upnp,
                        true,
                        opts.auto_start
                    );

                    match send_req::<NodeInstanceInfo>(
                        &format!("{api_url}/nodes/create"),
                        Some(body),
                    )
                    .await
                    {
                        Ok(new_node) => Ok(CliCmdResponse::NodeCreated(new_node)),
                        Err(err) => Err(err),
                    }
                }
            }
            CliCommands::Nodes(NodesSubcommands::Remove { id, interval }) => {
                send_node_action_req(
                    &format!("{api_url}/nodes/delete"),
                    &format!("{api_url}/batch/new"),
                    id,
                    *interval,
                    "Remove",
                )
                .await
            }
            CliCommands::Nodes(NodesSubcommands::Start { id, interval }) => {
                send_node_action_req(
                    &format!("{api_url}/nodes/start"),
                    &format!("{api_url}/batch/new"),
                    id,
                    *interval,
                    "Start",
                )
                .await
            }
            CliCommands::Nodes(NodesSubcommands::Stop { id, interval }) => {
                send_node_action_req(
                    &format!("{api_url}/nodes/stop"),
                    &format!("{api_url}/batch/new"),
                    id,
                    *interval,
                    "Stop",
                )
                .await
            }
            CliCommands::Nodes(NodesSubcommands::Recycle { id, interval }) => {
                send_node_action_req(
                    &format!("{api_url}/nodes/recycle"),
                    &format!("{api_url}/batch/new"),
                    id,
                    *interval,
                    "Recycle",
                )
                .await
            }
            CliCommands::Nodes(NodesSubcommands::Upgrade { id, interval }) => {
                send_node_action_req(
                    &format!("{api_url}/nodes/upgrade"),
                    &format!("{api_url}/batch/new"),
                    id,
                    *interval,
                    "Upgrade",
                )
                .await
            }
            CliCommands::Stats => send_req(&format!("{api_url}/nodes/list"), None)
                .await
                .map(|res: NodesInstancesInfo| CliCmdResponse::Stats(res.stats)),
            CliCommands::Batches(BatchesSubcommands::Ls) => {
                send_req(&format!("{api_url}/nodes/list"), None)
                    .await
                    .map(|res: NodesInstancesInfo| CliCmdResponse::Batches(res.scheduled_batches))
            }
            CliCommands::Batches(BatchesSubcommands::Cancel { batch_id }) => {
                let body = format!("batch_id={batch_id}");
                send_req::<()>(&format!("{api_url}/batch/cancel"), Some(body)).await?;
                Ok(CliCmdResponse::Success)
            }
            CliCommands::Settings(SettingsSubcommands::Ls) => {
                send_req(&format!("{api_url}/settings/get"), None)
                    .await
                    .map(|settings: AppSettings| CliCmdResponse::Settings(settings))
            }
        }
    }
}

// Helper which converts a value to string or a dash sign if it's None
fn value_or_dash<T: ToString>(val: Option<T>) -> String {
    val.map_or("-".to_string(), |v| v.to_string())
}

impl CliCmdResponse {
    pub fn print<T: Write + ?Sized>(&self, out: &mut T) -> Result<(), Error> {
        let tables = self.gen_print_table();
        for t in tables {
            t.print(out)?;
            writeln!(out)?;
        }
        Ok(())
    }

    pub fn printstd(&self) {
        let tables = self.gen_print_table();
        tables.iter().for_each(|t| {
            t.printstd();
            println!()
        });
    }

    fn gen_print_table(&self) -> Vec<Table> {
        let mut tables = vec![];
        match self {
            CliCmdResponse::Nodes(nodes, extended) => {
                if *extended {
                    for info in nodes.values() {
                        let mut table = Table::new();
                        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
                        table.set_titles(row!["Node Id", info.short_node_id()]);
                        table.add_row(row!["Status", format_node_status(info)]);
                        table.add_row(row![
                            "Created",
                            DateTime::<Utc>::from_timestamp(info.created as i64, 0)
                                .unwrap_or_default()
                                .with_timezone(&Local)
                                .to_string()
                        ]);
                        table.add_row(row!["PID", value_or_dash(info.pid)]);
                        table.add_row(row!["Version", value_or_dash(info.bin_version.clone())]);
                        table.add_row(row![
                            "Memory Used",
                            value_or_dash(info.mem_used.map(|v| format!("{v:.2} MB")))
                        ]);
                        table.add_row(row![
                            "CPU",
                            value_or_dash(info.cpu_usage.map(|v| format!("{v:.2}%")))
                        ]);
                        table.add_row(row!["Stored Records", value_or_dash(info.records)]);
                        table.add_row(row![
                            "Relevant Records",
                            value_or_dash(info.relevant_records)
                        ]);
                        table.add_row(row!["Connected peers", value_or_dash(info.connected_peers)]);
                        table.add_row(row!["Shunned by", value_or_dash(info.shunned_count)]);
                        table.add_row(row!["kBuckets peers", value_or_dash(info.kbuckets_peers)]);
                        table.add_row(row!["Estimated network size", value_or_dash(info.net_size)]);

                        table.add_row(row![
                            "Balance",
                            value_or_dash(
                                info.balance
                                    .map(|b| format_units(b, "ether").unwrap_or_default(),)
                            )
                        ]);
                        table.add_row(row![
                            "Rewards",
                            value_or_dash(
                                info.rewards
                                    .map(|b| format_units(b, "ether").unwrap_or_default(),)
                            )
                        ]);
                        table.add_row(row!["Port", value_or_dash(info.port)]);
                        table.add_row(row!["Metrics port", value_or_dash(info.metrics_port)]);
                        table.add_row(row![
                            "Home-network",
                            if info.home_network { "On" } else { "Off" }
                        ]);
                        table.add_row(row!["UPnP", if info.upnp { "On" } else { "Off" }]);
                        if !info.home_network {
                            table.add_row(row![
                                "Relay clients",
                                value_or_dash(info.connected_relay_clients)
                            ]);
                            table.add_row(row!["IPs", value_or_dash(info.ips.clone())]);
                        }
                        table.add_row(row![
                            "Rewards address",
                            value_or_dash(info.rewards_addr.clone())
                        ]);
                        table.add_row(row!["Peer Id", value_or_dash(info.peer_id.clone())]);
                        tables.push(table);
                    }
                } else {
                    let mut table = Table::new();
                    table.set_titles(row![
                        "Node Id",
                        "Memory used",
                        "CPU",
                        "Records",
                        "Conn. peers",
                        "Status"
                    ]);
                    for info in nodes.values() {
                        table.add_row(row![
                            info.short_node_id(),
                            value_or_dash(info.mem_used.map(|v| format!("{v:.2} MB"))),
                            value_or_dash(info.cpu_usage.map(|v| format!("{v:.2}%"))),
                            value_or_dash(info.records),
                            value_or_dash(info.connected_peers),
                            format_node_status(info)
                        ]);
                    }
                    tables.push(table);
                }
            }
            CliCmdResponse::NodeCreated(info) => {
                let mut table = Table::new();
                table.set_titles(row![
                    "Node Id",
                    "Memory used",
                    "CPU",
                    "Records",
                    "Conn. peers",
                    "Status"
                ]);
                table.add_row(row![
                    info.node_id,
                    value_or_dash(info.mem_used.map(|v| format!("{v:.2} MB"))),
                    value_or_dash(info.cpu_usage.map(|v| format!("{v:.2}%"))),
                    value_or_dash(info.records),
                    value_or_dash(info.connected_peers),
                    format_node_status(info)
                ]);

                tables.push(table);
            }
            CliCmdResponse::Stats(stats) => {
                let mut table = Table::new();
                table.set_titles(row![
                    "Total balance",
                    "Connected peers",
                    "Active nodes",
                    "Stored records",
                    "Estimated network size"
                ]);
                table.add_row(row![
                    truncated_balance_str(stats.total_balance),
                    stats.connected_peers,
                    format!("{}/{}", stats.active_nodes, stats.total_nodes),
                    stats.stored_records,
                    stats.estimated_net_size
                ]);
                tables.push(table);
            }
            CliCmdResponse::Batches(batches) => {
                let mut table = Table::new();
                table.set_titles(row!["Batch Id", "Action", "Status", "Interval", "Progress"]);
                for batch in batches {
                    let (count, extra_detail) = match &batch.batch_type {
                        BatchType::Create { node_opts, count } => (
                            *count,
                            format!(
                                " (auto-start: {})",
                                if node_opts.auto_start { "yes" } else { "no" }
                            ),
                        ),
                        other => (other.ids().len() as u16, "".to_string()),
                    };
                    let progress = if count > 0 {
                        (batch.complete * 100) / count
                    } else {
                        0
                    };

                    table.add_row(row![
                        batch.id,
                        format!("{}{}", batch.batch_type, extra_detail),
                        batch.status,
                        format!("{}s", batch.interval_secs),
                        format!("{}/{} ({}%)", batch.complete, count, progress)
                    ]);
                }
                tables.push(table);
            }
            CliCmdResponse::Settings(settings) => {
                let mut table = Table::new();
                table.set_titles(row!["Settings"]);
                table.add_row(row!["Nodes auto-upgrade", settings.nodes_auto_upgrade]);
                table.add_row(row![
                    "Nodes auto-upgrade delay",
                    format!("{:?}", settings.nodes_auto_upgrade_delay)
                ]);
                table.add_row(row![
                    "Node latest version check freq.",
                    format!("{:?}", settings.node_bin_version_polling_freq)
                ]);
                table.add_row(row![
                    "Nodes metrics polling freq.",
                    format!("{:?}", settings.nodes_metrics_polling_freq)
                ]);
                table.add_row(row![
                    "Rewards balances retrieval freq.",
                    format!("{:?}", settings.rewards_balances_retrieval_freq)
                ]);
                table.add_row(row!["L2 network RPC URL", settings.l2_network_rpc_url]);
                table.add_row(row![
                    "Token contract address",
                    settings.token_contract_address
                ]);
                table.add_row(row!["LCD display enabled", settings.lcd_display_enabled]);
                table.add_row(row!["LCD device", settings.lcd_device]);
                table.add_row(row!["LCD address", settings.lcd_addr]);
                tables.push(table);
            }
            CliCmdResponse::BatchCreated(batch_id) => {
                let mut table = Table::new();
                table.set_format(*prettytable::format::consts::FORMAT_NO_BORDER);
                table.add_row(row![format!(
                    "Batch created successfully. Batch Id: {batch_id}"
                )]);
                tables.push(table);
            }
            CliCmdResponse::Success => {
                let mut table = Table::new();
                table.set_format(*prettytable::format::consts::FORMAT_NO_BORDER);
                table.add_row(row!["Success!"]);
                tables.push(table);
            }
        }

        tables
    }
}

fn format_node_status(info: &NodeInstanceInfo) -> String {
    if info.status.is_transitioning() {
        format!("{} ...", info.status)
    } else if info.status_info.is_empty() {
        format!("{}", info.status)
    } else {
        format!("{}, {}", info.status, info.status_info)
    }
}

// Helper to send request and parse response
#[cfg(feature = "ssr")]
async fn send_req<T: serde::de::DeserializeOwned>(
    url: &str,
    body: Option<String>,
) -> Result<T, String> {
    let client = reqwest::Client::new();
    let mut req_builder = client.post(url);

    if let Some(body) = body {
        req_builder = req_builder.body(body);
    }

    let res = req_builder
        .send()
        .await
        .map_err(|err| format!("Failed to send request: {err:?}"))?;

    if res.status().is_success() {
        res.json::<T>().await.map_err(|err| format!("{err:?}"))
    } else {
        let err_msg = format!("Faile decode response: {res:?}");
        Err(res.text().await.unwrap_or(err_msg).to_string())
    }
}

// Helper to send node action request
#[cfg(feature = "ssr")]
async fn send_node_action_req(
    url: &str,
    batch_url: &str,
    node_ids: &[NodeId],
    interval: u64,
    action_type: &str,
) -> Result<CliCmdResponse, String> {
    if node_ids.len() > 1 {
        // create batch for multiple ids
        // TODO: use some crate which performs this serialisation
        let mut body = "".to_string();
        for (i, node_id) in node_ids.iter().enumerate() {
            body = format!("{body}batch_type[{action_type}][{i}]={node_id}&");
        }
        let body = format!("{body}interval_secs={interval}");
        let batch_id = send_req::<u16>(batch_url, Some(body)).await?;
        Ok(CliCmdResponse::BatchCreated(batch_id))
    } else if let Some(node_id) = node_ids.first() {
        let body = format!("node_id={node_id}");
        send_req::<()>(url, Some(body)).await?;
        Ok(CliCmdResponse::Success)
    } else {
        send_req::<()>(url, None).await?;
        Ok(CliCmdResponse::Success)
    }
}
