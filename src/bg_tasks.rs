#[cfg(not(feature = "native"))]
use super::{
    docker_client::{DockerClient, DockerClientError},
    server_api::helper_upgrade_node_instance,
};
#[cfg(feature = "native")]
use super::{
    node_instance::NodeStatus,
    node_manager::{NodeManager, NodeManagerError},
    server_api_native::helper_upgrade_node_instance,
};

use super::{
    app::{BgTasksCmds, ImmutableNodeStatus, METRICS_MAX_SIZE_PER_CONTAINER},
    db_client::DbClient,
    lcd::display_stats_on_lcd,
    metrics_client::{NodeMetricsClient, NodesMetrics},
    node_instance::{ContainerId, NodeInstanceInfo},
    server_api_types::{AppSettings, Stats},
};
use alloy::{
    primitives::{Address, U256},
    providers::{Network, Provider, ProviderBuilder},
    transports::Transport,
};
use alloy_sol_types::sol;
use leptos::logging;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::{
    select,
    sync::{broadcast, Mutex},
    time::{interval, sleep, timeout, Duration, Interval},
};
use url::Url;

// How often to perform a metrics pruning in the DB.
const METRICS_PRUNING_FREQ: Duration = Duration::from_secs(60 * 60); // every hour.

// Frequency to pull a new version of the formica image.
const FORMICA_IMAGE_PULLING_FREQ: Duration = Duration::from_secs(60 * 60 * 6); // every 6 hours.

// Timeout duration when querying for each rewards balance.
const BALANCE_QUERY_TIMEOUT: Duration = Duration::from_secs(10);
// Timeout duration when querying metrics from each node.
const NODE_METRICS_QUERY_TIMEOUT: Duration = Duration::from_secs(3);

// Frequency to poll node status from Docker engine
const NODE_STATUS_POLLING_FREQ: Duration = Duration::from_secs(5);

const LCD_LABEL_NET_SIZE: &str = "Network size:";
const LCD_LABEL_ACTIVE_NODES: &str = "Active nodes:";
const LCD_LABEL_STORED_RECORDS: &str = "Stored records:";
const LCD_LABEL_BIN_VERSION: &str = "Binary version:";
const LCD_LABEL_BALANCE: &str = "Total balance:";

// ERC20 token contract ABI
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    TokenContract,
    "artifacts/token_contract_abi.json"
);

// App settings and set of intervals used to schedule each of the tasks.
struct TasksContext {
    formica_image_pulling: Interval,
    node_bin_version_check: Interval,
    balances_retrieval: Interval,
    metrics_pruning: Interval,
    nodes_metrics_polling: Interval,
    nodes_status_polling: Interval,
    app_settings: AppSettings,
}

impl TasksContext {
    fn from(settings: AppSettings) -> Self {
        let mut balances_retrieval = interval(settings.rewards_balances_retrieval_freq);
        balances_retrieval.reset(); // the task will trigger the first check by itself

        Self {
            formica_image_pulling: interval(FORMICA_IMAGE_PULLING_FREQ),
            node_bin_version_check: interval(settings.node_bin_version_polling_freq),
            balances_retrieval,
            metrics_pruning: interval(METRICS_PRUNING_FREQ),
            nodes_metrics_polling: interval(settings.nodes_metrics_polling_freq),
            nodes_status_polling: interval(NODE_STATUS_POLLING_FREQ),
            app_settings: settings,
        }
    }

    fn apply_settings(&mut self, settings: AppSettings) {
        logging::log!("Applying new settings values immediataly to bg tasks: {settings:#?}");

        // helper to create a new interval only if new period differs from current
        let update_interval = |target: &mut Interval, new_period: Duration| {
            let curr_period = target.period();
            if new_period != curr_period {
                *target = interval(new_period);
                // reset interval to start next period from this instant
                target.reset();
            }
        };

        update_interval(
            &mut self.node_bin_version_check,
            settings.node_bin_version_polling_freq,
        );
        update_interval(
            &mut self.balances_retrieval,
            settings.rewards_balances_retrieval_freq,
        );
        update_interval(
            &mut self.nodes_metrics_polling,
            settings.nodes_metrics_polling_freq,
        );
        self.app_settings = settings;
    }
}

#[derive(Clone)]
struct NodeManagerProxy {
    db_client: DbClient,
    #[cfg(not(feature = "native"))]
    docker_client: DockerClient,
    #[cfg(feature = "native")]
    node_manager: NodeManager,
}

impl NodeManagerProxy {
    #[cfg(not(feature = "native"))]
    async fn get_nodes_list(&self, all: bool) -> Result<Vec<NodeInstanceInfo>, DockerClientError> {
        self.docker_client.get_containers_list(all).await
    }

    #[cfg(feature = "native")]
    async fn get_nodes_list(&self, all: bool) -> Result<Vec<NodeInstanceInfo>, NodeManagerError> {
        let active_nodes = self.node_manager.get_active_nodes_list().await?;
        let nodes_in_db = self.db_client.get_nodes_list().await;

        let nodes = nodes_in_db
            .into_iter()
            .filter_map(|(_, mut node_info)| {
                node_info.status = NodeStatus::Inactive;
                if node_info.pid.map(|pid| active_nodes.contains(&pid)) == Some(true) {
                    node_info.status = NodeStatus::Active;
                }

                if all || node_info.status.is_active() {
                    Some(node_info)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // TODO: what if there are active PIDs not found in DB...
        // ...populate them in DB so the user can see/delete them...?

        Ok(nodes)
    }

    #[cfg(not(feature = "native"))]
    async fn upgrade_node_instance(
        &self,
        container_id: &ContainerId,
        node_status_locked: &ImmutableNodeStatus,
    ) -> Result<(), DockerClientError> {
        helper_upgrade_node_instance(
            container_id,
            node_status_locked,
            &self.db_client,
            &self.docker_client,
        )
        .await
    }

    #[cfg(feature = "native")]
    async fn upgrade_node_instance(
        &self,
        container_id: &ContainerId,
        node_status_locked: &ImmutableNodeStatus,
    ) -> Result<(), NodeManagerError> {
        helper_upgrade_node_instance(
            container_id,
            node_status_locked,
            &self.db_client,
            &self.node_manager,
        )
        .await
    }

    #[cfg(not(feature = "native"))]
    async fn pull_formica_image(&self) -> Result<(), DockerClientError> {
        logging::log!("Pulling formica node image ...");
        self.docker_client.pull_formica_image().await
    }

    #[cfg(feature = "native")]
    async fn pull_formica_image(&self) -> Result<(), NodeManagerError> {
        Ok(())
    }

    #[cfg(not(feature = "native"))]
    async fn upgrade_node_binary(
        &self,
        version: &str,
        latest_bin_version: Arc<Mutex<Option<String>>>,
    ) {
        *latest_bin_version.lock().await = Some(version.to_string());
    }

    #[cfg(feature = "native")]
    async fn upgrade_node_binary(
        &self,
        version: &str,
        latest_bin_version: Arc<Mutex<Option<String>>>,
    ) {
        logging::log!("Downloading latest node binary ...");
        match self.node_manager.upgrade_node_binary(Some(version)).await {
            Ok(_) => {
                logging::log!("Node binary {version} downloaded successfully!");
                *latest_bin_version.lock().await = Some(version.to_string());
            }
            Err(err) => logging::error!("Failed to download new version of node binary: {err:?}"),
        }
    }
}

// Spawn any required background tasks
#[allow(clippy::too_many_arguments)]
pub fn spawn_bg_tasks(
    #[cfg(not(feature = "native"))] docker_client: DockerClient,
    #[cfg(feature = "native")] node_manager: NodeManager,
    latest_bin_version: Arc<Mutex<Option<String>>>,
    nodes_metrics: Arc<Mutex<NodesMetrics>>,
    db_client: DbClient,
    server_api_hit: Arc<Mutex<bool>>,
    node_status_locked: ImmutableNodeStatus,
    bg_tasks_cmds_tx: broadcast::Sender<BgTasksCmds>,
    global_stats: Arc<Mutex<Stats>>,
    settings: AppSettings,
) {
    logging::log!("App settings to use: {settings:#?}");
    let mut ctx = TasksContext::from(settings);

    let lcd_stats = Arc::new(Mutex::new(
        [(
            "Formicaio".to_string(),
            format!("v{}", env!("CARGO_PKG_VERSION")),
        )]
        .into_iter()
        .collect::<HashMap<String, String>>(),
    ));

    // Based on settings, setup LCD external device to display stats.
    if ctx.app_settings.lcd_display_enabled {
        tokio::spawn(display_stats_on_lcd(
            ctx.app_settings.clone(),
            bg_tasks_cmds_tx.subscribe(),
            lcd_stats.clone(),
        ));
    }

    #[cfg(not(feature = "native"))]
    let node_mgr_proxy = NodeManagerProxy {
        db_client: db_client.clone(),
        docker_client,
    };
    #[cfg(feature = "native")]
    let node_mgr_proxy = NodeManagerProxy {
        db_client: db_client.clone(),
        node_manager,
    };

    // Spawn task which checks address balances as requested on the provided channel
    tokio::spawn(balance_checker_task(
        ctx.app_settings.clone(),
        node_mgr_proxy.clone(),
        db_client.clone(),
        lcd_stats.clone(),
        bg_tasks_cmds_tx.clone(),
        global_stats.clone(),
    ));

    tokio::spawn(async move {
        let mut bg_tasks_cmds_rx = bg_tasks_cmds_tx.subscribe();
        loop {
            select! {
                settings = bg_tasks_cmds_rx.recv() => {
                    if let Ok(BgTasksCmds::ApplySettings(s)) = settings {
                        if s.lcd_display_enabled && (!ctx.app_settings.lcd_display_enabled
                            || ctx.app_settings.lcd_device != s.lcd_device
                            || ctx.app_settings.lcd_addr != s.lcd_addr)
                        {
                            logging::log!("Setting up LCD display with new device parameters...");
                            // TODO: when it fails, send error back to the client,
                            // perhaps we need websockets for errors like this one.
                            tokio::spawn(display_stats_on_lcd(
                                s.clone(),
                                bg_tasks_cmds_tx.subscribe(),
                                lcd_stats.clone()
                            ));
                        }

                        ctx.apply_settings(s);
                    }
                },
                _ = ctx.formica_image_pulling.tick() => {
                    let node_mgr_proxy = node_mgr_proxy.clone();
                    tokio::spawn(async move {
                        if let Err(err) = node_mgr_proxy.pull_formica_image().await {
                            logging::log!("Failed to pull node image from the periodic task: {err}");
                        }
                    });
                },
                _ = ctx.node_bin_version_check.tick() => {
                    tokio::spawn(check_node_bin_version(
                        node_mgr_proxy.clone(),
                        latest_bin_version.clone(),
                        db_client.clone(),
                        node_status_locked.clone()
                    ));
                },
                _ = ctx.balances_retrieval.tick() => {
                    let _ = bg_tasks_cmds_tx.send(BgTasksCmds::CheckAllBalances);
                },
                _ = ctx.metrics_pruning.tick() => {
                    tokio::spawn(prune_metrics(
                        node_mgr_proxy.clone(),
                        db_client.clone()
                    ));
                },
                _ = ctx.nodes_metrics_polling.tick() => {
                    let query_bin_version = ctx.app_settings.lcd_display_enabled;

                    // we don't spawn a task for this one just in case it's taking
                    // too long to complete and we may start overwhelming the backend
                    // with multiple overlapping tasks being launched.
                    update_nodes_info(
                        &node_mgr_proxy,
                        &nodes_metrics,
                        &db_client,
                        &node_status_locked,
                        query_bin_version,
                        &lcd_stats,
                        global_stats.clone()

                    ).await;
                    // reset interval to start next period from this instant,
                    // regardless how long the above polling task lasted.
                    ctx.nodes_metrics_polling.reset_after(ctx.nodes_metrics_polling.period());
                },
                _ = ctx.nodes_status_polling.tick() => {
                    // we poll node status only if a client is currently querying the API,
                    // and only if the metrics polling is not frequent enough
                    let api_hit = *server_api_hit.lock().await;
                    if !api_hit || 2 * ctx.nodes_status_polling.period() > ctx.nodes_metrics_polling.period() {
                        continue;
                    }

                    *server_api_hit.lock().await = false;
                    match node_mgr_proxy.get_nodes_list(true).await {
                        Ok(nodes) => {
                            let total_nodes = nodes.len();
                            let mut num_active_nodes = 0;
                            let mut num_inactive_nodes = 0;

                            for node_info in nodes.into_iter() {
                                update_node_metadata(&node_info, &db_client, &node_status_locked).await;
                                if node_info.status.is_active() {
                                    num_active_nodes += 1;
                                } else if node_info.status.is_inactive() {
                                    num_inactive_nodes += 1;
                                }
                            }

                            let mut guard = global_stats.lock().await;
                            guard.total_nodes = total_nodes;
                            guard.active_nodes = num_active_nodes;
                            guard.inactive_nodes = num_inactive_nodes;
                        },
                        Err(err) => logging::log!("Failed to get containers list: {err}")
                    }
                }
            }
        }
    });
}

// Check latest version of node binary and upgrade nodes
// automatically if auto-upgrade was enabled by the user.
async fn check_node_bin_version(
    node_mgr_proxy: NodeManagerProxy,
    latest_bin_version: Arc<Mutex<Option<String>>>,
    db_client: DbClient,
    node_status_locked: ImmutableNodeStatus,
) {
    if let Some(latest_version) = latest_version_available().await {
        logging::log!("Latest version of node binary available: {latest_version}");

        let latest_known_version = latest_bin_version.lock().await.clone();
        match latest_known_version {
            // TODO: use semantic version to make the comparison.
            Some(known) if known != latest_version => {
                node_mgr_proxy
                    .upgrade_node_binary(&latest_version, latest_bin_version.clone())
                    .await
            }
            None => {
                node_mgr_proxy
                    .upgrade_node_binary(&latest_version, latest_bin_version.clone())
                    .await
            }
            _ => {}
        }

        loop {
            let auto_upgrade = db_client.get_settings().await.nodes_auto_upgrade;
            logging::log!("Nodes auto-upgrading setting enabled?: {auto_upgrade}",);
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
                Ok(Some((container_id, v))) => {
                    logging::log!("Auto-upgrading node binary from v{v} to v{latest_version} for node instance {container_id} ...");
                    if let Err(err) = node_mgr_proxy
                        .upgrade_node_instance(&container_id, &node_status_locked)
                        .await
                    {
                        logging::log!("Failed to auto-upgrade node binary for node instance {container_id}: {err:?}.");
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
async fn latest_version_available() -> Option<String> {
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
            if let Ok(latest_version) = semver::Version::parse(version) {
                return Some(latest_version.to_string());
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
    let update_status = !node_status_locked
        .is_still_locked(&node_info.container_id)
        .await;
    db_client
        .update_node_metadata(node_info, update_status)
        .await;
}

// Fetch up to date information for each active node instance
// from nodes' exposed metrics server caching them in global context.
async fn update_nodes_info(
    node_mgr_proxy: &NodeManagerProxy,
    nodes_metrics: &Arc<Mutex<NodesMetrics>>,
    db_client: &DbClient,
    node_status_locked: &ImmutableNodeStatus,
    query_bin_version: bool,
    lcd_stats: &Arc<Mutex<HashMap<String, String>>>,
    global_stats: Arc<Mutex<Stats>>,
) {
    let nodes = node_mgr_proxy
        .get_nodes_list(true)
        .await
        .unwrap_or_else(|err| {
            logging::log!("Failed to get nodes list: {err}");
            vec![]
        });

    let num_nodes = nodes.len();
    if num_nodes > 0 {
        logging::log!("Fetching status and metrics from {num_nodes} node/s ...");
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
                let metrics_client = NodeMetricsClient::new(&node_info.node_ip, metrics_port);
                let node_short_id = node_info.short_container_id();

                match timeout(NODE_METRICS_QUERY_TIMEOUT, metrics_client.fetch_metrics()).await {
                    Ok(Ok(metrics)) => {
                        let mut node_metrics = nodes_metrics.lock().await;
                        node_metrics.store(&node_info.container_id, &metrics).await;
                        node_metrics.update_node_info(&mut node_info);
                    }
                    Ok(Err(err)) => {
                        logging::log!("Failed to fetch metrics from node {node_short_id}: {err}");
                    }
                    Err(_) => {
                        logging::log!("Timeout ({NODE_METRICS_QUERY_TIMEOUT:?}) while fetching metrics from node {node_short_id}.");
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
            if let Some(ref version) = db_client
                .get_node_bin_version(&node_info.container_id)
                .await
            {
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
        logging::log!("No active nodes to retrieve metrics from...");
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

    let mut guard = global_stats.lock().await;
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
async fn prune_metrics(node_mgr_proxy: NodeManagerProxy, db_client: DbClient) {
    let nodes = match node_mgr_proxy.get_nodes_list(false).await {
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
            node_info.short_container_id()
        );
        db_client
            .remove_oldest_metrics(
                node_info.container_id.clone(),
                METRICS_MAX_SIZE_PER_CONTAINER,
            )
            .await;
    }
}

async fn balance_checker_task(
    settings: AppSettings,
    node_mgr_proxy: NodeManagerProxy,
    db_client: DbClient,
    lcd_stats: Arc<Mutex<HashMap<String, String>>>,
    bg_tasks_cmds_tx: broadcast::Sender<BgTasksCmds>,
    global_stats: Arc<Mutex<Stats>>,
) {
    // cache retrieved rewards balances to not query more than once per address
    let mut updated_balances = HashMap::<Address, U256>::new();

    // Let's trigger a first check now
    let mut bg_tasks_cmds_rx = bg_tasks_cmds_tx.subscribe();
    if let Err(err) = bg_tasks_cmds_tx.send(BgTasksCmds::CheckAllBalances) {
        logging::warn!("Initial check for balances couldn't be triggered: {err:?}");
    }

    // helper which creates a new contract if the new configured values are valid.
    let update_token_contract = |contract_addr: &str, rpc_url: &str| {
        let addr = match contract_addr.parse::<Address>() {
            Err(err) => {
                logging::log!("Rewards balance check disabled. Invalid configured token contract address: {err}");
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
                let provider = ProviderBuilder::new().on_http(rpc_url);
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

                    let total_balance: U256 = updated_balances.values().sum();
                    update_lcd_stats(
                        &lcd_stats,
                        &[(LCD_LABEL_BALANCE, total_balance.to_string())],
                    )
                    .await;
                    global_stats.lock().await.total_balance = total_balance;
                }
            }
            Ok(BgTasksCmds::DeleteBalanceFor(node_info)) => {
                if let Some(Ok(address)) = node_info
                    .rewards_addr
                    .as_ref()
                    .map(|addr| addr.parse::<Address>())
                {
                    updated_balances.remove(&address);
                    let total_balance: U256 = updated_balances.values().sum();
                    update_lcd_stats(
                        &lcd_stats,
                        &[(LCD_LABEL_BALANCE, total_balance.to_string())],
                    )
                    .await;
                    global_stats.lock().await.total_balance = total_balance;
                }
            }
            Ok(BgTasksCmds::CheckAllBalances) => {
                updated_balances.clear();
                let mut total_balance = U256::from(0u64);
                if let Some(ref token_contract) = token_contract {
                    match node_mgr_proxy.get_nodes_list(true).await {
                        Ok(nodes) if !nodes.is_empty() => {
                            retrieve_current_balances(
                                nodes,
                                token_contract,
                                &db_client,
                                &mut updated_balances,
                            )
                            .await;

                            let new_balance: U256 = updated_balances.values().sum();
                            update_lcd_stats(
                                &lcd_stats,
                                &[(LCD_LABEL_BALANCE, new_balance.to_string())],
                            )
                            .await;
                            total_balance = new_balance
                        }
                        Err(err) => {
                            logging::log!("Failed to get containers list: {err}");
                            remove_lcd_stats(&lcd_stats, &[LCD_LABEL_BALANCE]).await;
                        }
                        _ => {
                            remove_lcd_stats(&lcd_stats, &[LCD_LABEL_BALANCE]).await;
                        }
                    }
                }
                global_stats.lock().await.total_balance = total_balance;
            }
            Err(_) => {}
        }
    }
}

async fn retrieve_current_balances<T: Transport + Clone, P: Provider<T, N>, N: Network>(
    nodes: impl IntoIterator<Item = NodeInstanceInfo>,
    token_contract: &TokenContract::TokenContractInstance<T, P, N>,
    db_client: &DbClient,
    updated_balances: &mut HashMap<Address, U256>,
) {
    for node_info in nodes.into_iter() {
        let node_short_id = node_info.short_container_id();
        if let Some(Ok(address)) = node_info
            .rewards_addr
            .as_ref()
            .map(|addr| addr.parse::<Address>())
        {
            let new_balance = if let Some(balance) = updated_balances.get(&address) {
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
                        let balance = balance._0;
                        updated_balances.insert(address, balance);
                        balance.to_string()
                    }
                    Ok(Err(err)) => {
                        logging::log!(
                            "Failed to query rewards balance for node {node_short_id}: {err}"
                        );
                        "".to_string()
                    }
                    Err(_) => {
                        logging::log!("Timeout ({BALANCE_QUERY_TIMEOUT:?}) while querying rewards balance for node {node_short_id}.");
                        "".to_string()
                    }
                }
            };

            db_client
                .update_node_metadata_fields(&node_info.container_id, &[("balance", &new_balance)])
                .await;
        } else {
            logging::log!("No valid rewards address set for node {node_short_id}.");
        }
    }
}

// Helper to add/update stats to be disaplyed on external LCD device
async fn update_lcd_stats(
    lcd_stats: &Arc<Mutex<HashMap<String, String>>>,
    labels_vals: &[(&str, String)],
) {
    let mut s = lcd_stats.lock().await;
    labels_vals
        .iter()
        .filter(|(l, v)| !l.is_empty() && !v.is_empty())
        .for_each(|(label, value)| {
            let _ = s.insert(label.to_string(), value.clone());
        });
}

// Helper to remove stats being displayed on external LCD device
async fn remove_lcd_stats(lcd_stats: &Arc<Mutex<HashMap<String, String>>>, labels: &[&str]) {
    let mut s = lcd_stats.lock().await;
    labels.iter().for_each(|label| {
        let _ = s.remove(*label);
    });
}
