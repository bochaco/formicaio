use crate::{
    app::METRICS_MAX_SIZE_PER_NODE,
    db_client::DbClient,
    node_mgr::NodeManager,
    types::{AppSettings, NodeInstanceInfo, NodeStatus, Stats},
    views::truncated_balance_str,
};

use super::{
    BgTasksCmds, ImmutableNodeStatus, TokenContract,
    metrics_client::{NodeMetricsClient, NodesMetrics},
};

use alloy::{
    network::Network,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
};
use chrono::Utc;
use leptos::logging;
use semver::Version;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::{
    sync::{RwLock, broadcast},
    time::{Duration, sleep, timeout},
};
use url::Url;

// Timeout duration when querying for each rewards balance.
const BALANCE_QUERY_TIMEOUT: Duration = Duration::from_secs(10);
// Timeout duration when querying metrics from each node.
const NODE_METRICS_QUERY_TIMEOUT: Duration = Duration::from_secs(3);

const LCD_LABEL_NET_SIZE: &str = "Network size:";
const LCD_LABEL_ACTIVE_NODES: &str = "Active nodes:";
const LCD_LABEL_STORED_RECORDS: &str = "Stored records:";
const LCD_LABEL_BIN_VERSION: &str = "Binary version:";
const LCD_LABEL_BALANCE: &str = "Total balance:";

// Check latest version of node binary and upgrade nodes
// automatically if auto-upgrade was enabled by the user.
pub async fn check_node_bin_version(node_manager: NodeManager, db_client: DbClient) {
    if let Some(latest_version) = latest_version_available().await {
        logging::log!("Latest version of node binary available: {latest_version}");

        if let Err(err) = node_manager
            .upgrade_master_node_binary(Some(&latest_version))
            .await
        {
            logging::error!("Failed to download node binary version {latest_version}: {err:?}");
        }

        loop {
            let auto_upgrade = db_client.get_settings().await.nodes_auto_upgrade;
            logging::log!("Nodes auto-upgrading setting enabled?: {auto_upgrade}");
            if !auto_upgrade {
                break;
            }

            // we'll upgrade only one in each iteration of the loop, if the user changes the
            // settings, in next iteration we will stop the auto-upgrade and/or avoid upgrading a node
            // which may have been already upgraded by the user manually.
            match db_client
                .get_outdated_nodes_list(&latest_version)
                .await
                .map(|list| list.first().cloned())
            {
                Ok(Some((node_id, v))) => {
                    logging::log!(
                        "Auto-upgrading node binary from v{v} to v{latest_version} for node instance {node_id} ..."
                    );
                    if let Err(err) = node_manager.upgrade_node_instance(&node_id).await {
                        logging::log!(
                            "Failed to auto-upgrade node binary for node instance {node_id}: {err:?}."
                        );
                    }

                    let delay = db_client.get_settings().await.nodes_auto_upgrade_delay;
                    sleep(delay).await;
                }
                Ok(None) => break, // all nodes are up to date
                Err(err) => {
                    logging::log!("Failed to retrieve list of nodes' binary versions: {err:?}");
                    break;
                }
            }
        }
    }
}

// Query crates.io to find out latest version available of the node
async fn latest_version_available() -> Option<Version> {
    let url = "https://crates.io/api/v1/crates/ant-node".to_string();
    let client = reqwest::Client::new();
    const MY_USER_AGENT: &str = "formicaio (https://github.com/bochaco/formicaio)";

    let response = match client
        .get(url)
        .header(reqwest::header::USER_AGENT, MY_USER_AGENT)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(_) => return None,
    };

    if response.status().is_success() {
        let body = match response.text().await {
            Ok(body) => body,
            Err(_) => return None,
        };
        let json: serde_json::Value = match serde_json::from_str(&body) {
            Ok(json) => json,
            Err(_) => return None,
        };

        if let Some(version) = json["crate"]["newest_version"].as_str() {
            if let Ok(latest_version) = Version::parse(version) {
                return Some(latest_version);
            }
        }
    }

    None
}

// Update node medata into local DB cache
async fn update_node_metadata(
    node_info: &NodeInstanceInfo,
    db_client: &DbClient,
    node_status_locked: &ImmutableNodeStatus,
) {
    let update_status = !node_status_locked.is_still_locked(&node_info.node_id).await
        && !node_info.is_status_locked;
    db_client
        .update_node_metadata(node_info, update_status)
        .await;
}

// Fetch up to date information for each active node instance
// from nodes' exposed metrics server caching them in global context.
pub async fn update_nodes_info(
    node_manager: &NodeManager,
    nodes_metrics: &Arc<RwLock<NodesMetrics>>,
    db_client: &DbClient,
    node_status_locked: &ImmutableNodeStatus,
    query_bin_version: bool,
    lcd_stats: &Arc<RwLock<HashMap<String, String>>>,
    global_stats: Arc<RwLock<Stats>>,
) {
    let ts = Utc::now();
    let nodes = node_manager.get_nodes_list().await.unwrap_or_else(|err| {
        logging::log!("[{ts}] Failed to get nodes list: {err}");
        vec![]
    });

    let num_nodes = nodes.len();
    if num_nodes > 0 {
        logging::log!("[{ts}] Fetching status and metrics from {num_nodes} node/s ...");
    }

    // let's collect stats to update LCD (if enabled) and global stats records
    let mut net_size = 0;
    let mut num_active_nodes = 0;
    let mut num_inactive_nodes = 0;
    let mut records = 0;
    let mut relevant_records = 0;
    let mut connected_peers = 0;
    let mut shunned_count = 0;
    let mut bin_version = HashSet::<String>::new();

    for mut node_info in nodes.into_iter() {
        if node_info.status.is_active() {
            num_active_nodes += 1;

            if let Some(metrics_port) = node_info.metrics_port {
                // let's now collect metrics from the node
                let metrics_client = NodeMetricsClient::new(metrics_port);
                let node_short_id = node_info.short_node_id();

                match timeout(NODE_METRICS_QUERY_TIMEOUT, metrics_client.fetch_metrics()).await {
                    Ok(Ok(metrics)) => {
                        let mut node_metrics = nodes_metrics.write().await;
                        node_metrics.store(&node_info.node_id, &metrics).await;
                        node_metrics.update_node_info(&mut node_info);
                        node_info.status = NodeStatus::Active;
                    }
                    Ok(Err(err)) => {
                        node_info.set_status_to_unknown();
                        logging::log!("Failed to fetch metrics from node {node_short_id}: {err}");
                    }
                    Err(_) => {
                        node_info.set_status_to_unknown();
                        logging::log!(
                            "Timeout ({NODE_METRICS_QUERY_TIMEOUT:?}) while fetching metrics from node {node_short_id}."
                        );
                    }
                }

                net_size += node_info.net_size.unwrap_or_default();
                records += node_info.records.unwrap_or_default();
                relevant_records += node_info.relevant_records.unwrap_or_default();
                connected_peers += node_info.connected_peers.unwrap_or_default();
                shunned_count += node_info.shunned_count.unwrap_or_default();
            }
        } else if node_info.status.is_inactive() {
            num_inactive_nodes += 1;
        }

        // store up to date metadata and status onto local DB cache
        update_node_metadata(&node_info, db_client, node_status_locked).await;

        if query_bin_version {
            if let Some(ref version) = db_client.get_node_bin_version(&node_info.node_id).await {
                bin_version.insert(version.clone());
            }
        }
    }

    let mut updated_vals = vec![(
        LCD_LABEL_ACTIVE_NODES,
        format!("{num_active_nodes}/{num_nodes}"),
    )];

    let estimated_net_size = if num_active_nodes > 0 {
        let avg_net_size = net_size / num_active_nodes;
        let bin_versions = bin_version.into_iter().collect::<Vec<_>>().join(", ");

        updated_vals.extend([
            (LCD_LABEL_NET_SIZE, avg_net_size.to_string()),
            (LCD_LABEL_STORED_RECORDS, records.to_string()),
            (LCD_LABEL_BIN_VERSION, bin_versions),
        ]);
        avg_net_size
    } else {
        logging::log!("[{ts}] No active nodes to retrieve metrics from...");
        remove_lcd_stats(
            lcd_stats,
            &[
                LCD_LABEL_NET_SIZE,
                LCD_LABEL_STORED_RECORDS,
                LCD_LABEL_BIN_VERSION,
            ],
        )
        .await;
        0
    };

    update_lcd_stats(lcd_stats, &updated_vals).await;

    let mut guard = global_stats.write().await;
    guard.total_nodes = num_nodes;
    guard.active_nodes = num_active_nodes;
    guard.inactive_nodes = num_inactive_nodes;
    guard.connected_peers = connected_peers;
    guard.shunned_count = shunned_count;
    guard.estimated_net_size = estimated_net_size;
    guard.stored_records = records;
    guard.relevant_records = relevant_records;
}

// Prune metrics records from the cache DB to always keep the number of records within a limit.
pub async fn prune_metrics(node_manager: NodeManager, db_client: DbClient) {
    let nodes = match node_manager.get_nodes_list().await {
        Ok(nodes) if !nodes.is_empty() => nodes,
        Err(err) => {
            logging::log!("Failed to get nodes list: {err}");
            return;
        }
        _ => return,
    };

    for node_info in nodes.into_iter() {
        logging::log!(
            "Removing oldest metrics from DB for node {} ...",
            node_info.short_node_id()
        );
        db_client
            .remove_oldest_metrics(node_info.node_id.clone(), METRICS_MAX_SIZE_PER_NODE)
            .await;
    }
}

pub async fn balance_checker_task(
    settings: AppSettings,
    node_manager: NodeManager,
    db_client: DbClient,
    lcd_stats: Arc<RwLock<HashMap<String, String>>>,
    bg_tasks_cmds_tx: broadcast::Sender<BgTasksCmds>,
    global_stats: Arc<RwLock<Stats>>,
) {
    // cache retrieved rewards balances to not query more than once per address,
    // as well as how many nodes have each address set for rewards.
    let mut updated_balances = HashMap::<Address, (U256, u64)>::new();

    // Let's trigger a first check now
    let mut bg_tasks_cmds_rx = bg_tasks_cmds_tx.subscribe();
    if let Err(err) = bg_tasks_cmds_tx.send(BgTasksCmds::CheckAllBalances) {
        logging::warn!("Initial check for balances couldn't be triggered: {err:?}");
    }

    // helper which creates a new contract if the new configured values are valid.
    let update_token_contract = |contract_addr: &str, rpc_url: &str| {
        let addr = match contract_addr.parse::<Address>() {
            Err(err) => {
                logging::log!(
                    "Rewards balance check disabled. Invalid configured token contract address: {err}"
                );
                None
            }
            Ok(token_address) => Some(token_address),
        };
        let url = match rpc_url.parse::<Url>() {
            Err(err) => {
                logging::log!("Rewards balance check disabled. Invalid configured RPC URL: {err}");
                None
            }
            Ok(rpc_url) => Some(rpc_url),
        };

        match (addr, url) {
            (Some(token_address), Some(rpc_url)) => {
                let provider = ProviderBuilder::new().connect_http(rpc_url);
                let token_contract = TokenContract::new(token_address, provider);
                Some(token_contract)
            }
            _ => None,
        }
    };

    // Token contract used to query rewards balances.
    let mut token_contract = update_token_contract(
        &settings.token_contract_address,
        &settings.l2_network_rpc_url,
    );

    let mut prev_addr = settings.token_contract_address;
    let mut prev_url = settings.l2_network_rpc_url;

    loop {
        match bg_tasks_cmds_rx.recv().await {
            Ok(BgTasksCmds::ApplySettings(s)) => {
                if prev_addr != s.token_contract_address || prev_url != s.l2_network_rpc_url {
                    token_contract =
                        update_token_contract(&s.token_contract_address, &s.l2_network_rpc_url);
                    prev_addr = s.token_contract_address;
                    prev_url = s.l2_network_rpc_url;
                    let _ = bg_tasks_cmds_tx.send(BgTasksCmds::CheckAllBalances);
                }
            }
            Ok(BgTasksCmds::CheckBalanceFor(node_info)) => {
                if let Some(ref token_contract) = token_contract {
                    retrieve_current_balances(
                        [node_info],
                        token_contract,
                        &db_client,
                        &mut updated_balances,
                    )
                    .await;

                    let total_balance: U256 = updated_balances.values().map(|(b, _)| b).sum();
                    update_balance_lcd_stats(&lcd_stats, total_balance).await;
                    global_stats.write().await.total_balance = total_balance;
                }
            }
            Ok(BgTasksCmds::DeleteBalanceFor(node_info)) => {
                if let Some(Ok(address)) = node_info
                    .rewards_addr
                    .as_ref()
                    .map(|addr| addr.parse::<Address>())
                {
                    if let Some((_, num_nodes)) = updated_balances.get_mut(&address) {
                        *num_nodes -= 1;
                        if *num_nodes == 0 {
                            let _ = updated_balances.remove(&address);
                        }
                    }
                    let total_balance: U256 = updated_balances.values().map(|(b, _)| b).sum();
                    update_balance_lcd_stats(&lcd_stats, total_balance).await;
                    global_stats.write().await.total_balance = total_balance;
                }
            }
            Ok(BgTasksCmds::CheckAllBalances) => {
                updated_balances.clear();
                let mut total_balance = U256::from(0u64);
                if let Some(ref token_contract) = token_contract {
                    match node_manager.get_nodes_list().await {
                        Ok(nodes) if !nodes.is_empty() => {
                            retrieve_current_balances(
                                nodes,
                                token_contract,
                                &db_client,
                                &mut updated_balances,
                            )
                            .await;

                            let new_balance: U256 = updated_balances.values().map(|(b, _)| b).sum();
                            update_balance_lcd_stats(&lcd_stats, new_balance).await;
                            total_balance = new_balance
                        }
                        Err(err) => {
                            logging::log!("Failed to get nodes list: {err}");
                            remove_lcd_stats(&lcd_stats, &[LCD_LABEL_BALANCE]).await;
                        }
                        _ => {
                            remove_lcd_stats(&lcd_stats, &[LCD_LABEL_BALANCE]).await;
                        }
                    }
                }
                global_stats.write().await.total_balance = total_balance;
            }
            Err(_) => {}
        }
    }
}

async fn retrieve_current_balances<P: Provider<N>, N: Network>(
    nodes: impl IntoIterator<Item = NodeInstanceInfo>,
    token_contract: &TokenContract::TokenContractInstance<P, N>,
    db_client: &DbClient,
    updated_balances: &mut HashMap<Address, (U256, u64)>,
) {
    for node_info in nodes.into_iter() {
        let node_short_id = node_info.short_node_id();
        if let Some(Ok(address)) = node_info
            .rewards_addr
            .as_ref()
            .map(|addr| addr.parse::<Address>())
        {
            let new_balance = if let Some((balance, num_nodes)) = updated_balances.get_mut(&address)
            {
                *num_nodes += 1;
                balance.to_string()
            } else {
                // query the balance to the ERC20 contract
                logging::log!("Querying rewards balance for node {node_short_id} ...");
                match timeout(
                    BALANCE_QUERY_TIMEOUT,
                    token_contract.balanceOf(address).call(),
                )
                .await
                {
                    Ok(Ok(balance)) => {
                        updated_balances.insert(address, (balance, 1));
                        balance.to_string()
                    }
                    Ok(Err(err)) => {
                        logging::log!(
                            "Failed to query rewards balance for node {node_short_id}: {err}"
                        );
                        "".to_string()
                    }
                    Err(_) => {
                        logging::log!(
                            "Timeout ({BALANCE_QUERY_TIMEOUT:?}) while querying rewards balance for node {node_short_id}."
                        );
                        "".to_string()
                    }
                }
            };

            db_client
                .update_node_balance(&node_info.node_id, &new_balance)
                .await;
        } else {
            logging::log!("No valid rewards address set for node {node_short_id}.");
        }
    }
}

// Helper to update total balance stat to be disaplyed on external LCD device
async fn update_balance_lcd_stats(
    lcd_stats: &Arc<RwLock<HashMap<String, String>>>,
    new_balance: U256,
) {
    let balance = truncated_balance_str(new_balance);
    update_lcd_stats(lcd_stats, &[(LCD_LABEL_BALANCE, balance)]).await
}

// Helper to add/update stats to be disaplyed on external LCD device
async fn update_lcd_stats(
    lcd_stats: &Arc<RwLock<HashMap<String, String>>>,
    labels_vals: &[(&str, String)],
) {
    let mut s = lcd_stats.write().await;
    labels_vals
        .iter()
        .filter(|(l, v)| !l.is_empty() && !v.is_empty())
        .for_each(|(label, value)| {
            let _ = s.insert(label.to_string(), value.clone());
        });
}

// Helper to remove stats being displayed on external LCD device
async fn remove_lcd_stats(lcd_stats: &Arc<RwLock<HashMap<String, String>>>, labels: &[&str]) {
    let mut s = lcd_stats.write().await;
    labels.iter().for_each(|label| {
        let _ = s.remove(*label);
    });
}
