use super::{
    app::{AppSettings, ImmutableNodeStatus, METRICS_MAX_SIZE_PER_CONTAINER},
    db_client::DbClient,
    docker_client::DockerClient,
    lcd::display_stats_on_lcd,
    metrics_client::{NodeMetricsClient, NodesMetrics},
    node_instance::NodeInstanceInfo,
    server_api::helper_upgrade_node_instance,
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
        Self {
            formica_image_pulling: interval(FORMICA_IMAGE_PULLING_FREQ),
            node_bin_version_check: interval(settings.node_bin_version_polling_freq),
            balances_retrieval: interval(settings.rewards_balances_retrieval_freq),
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

    fn parse_token_addr_and_rpc_url(&self) -> (Option<Address>, Option<Url>) {
        let addr = match self.app_settings.token_contract_address.parse::<Address>() {
            Err(err) => {
                logging::log!("Rewards balance check disabled. Invalid configured token contract address: {err}");
                None
            }
            Ok(token_address) => Some(token_address),
        };
        let url = match self.app_settings.l2_network_rpc_url.parse::<Url>() {
            Err(err) => {
                logging::log!("Rewards balance check disabled. Invalid configured RPC URL: {err}");
                None
            }
            Ok(rpc_url) => Some(rpc_url),
        };

        (addr, url)
    }
}

// Spawn any required background tasks
#[allow(clippy::too_many_arguments)]
pub fn spawn_bg_tasks(
    docker_client: DockerClient,
    latest_bin_version: Arc<Mutex<Option<String>>>,
    nodes_metrics: Arc<Mutex<NodesMetrics>>,
    db_client: DbClient,
    server_api_hit: Arc<Mutex<bool>>,
    node_status_locked: ImmutableNodeStatus,
    mut updated_settings_rx: broadcast::Receiver<AppSettings>,
    settings: AppSettings,
) {
    logging::log!("App settings to use: {settings:#?}");
    let mut ctx = TasksContext::from(settings);

    // helper which create a new contract if the new configured values are valid.
    let update_token_contract = |ctx: &TasksContext| match ctx.parse_token_addr_and_rpc_url() {
        (Some(token_address), Some(rpc_url)) => {
            let provider = ProviderBuilder::new().on_http(rpc_url);
            let token_contract = TokenContract::new(token_address, provider);
            Some(token_contract)
        }
        _ => None,
    };

    // Token contract used to query rewards balances.
    let mut token_contract = update_token_contract(&ctx);

    let stats: HashMap<String, String> = [(
        "Formicaio".to_string(),
        format!("v{}", env!("CARGO_PKG_VERSION")),
    )]
    .into_iter()
    .collect();
    let lcd_stats = Arc::new(Mutex::new(stats));

    // Based on settings, setup LCD external device to display stats.
    if ctx.app_settings.lcd_display_enabled {
        tokio::spawn(display_stats_on_lcd(
            ctx.app_settings.clone(),
            updated_settings_rx.resubscribe(),
            lcd_stats.clone(),
        ));
    }

    tokio::spawn(async move {
        loop {
            select! {
                settings = updated_settings_rx.recv() => {
                    if let Ok(s) = settings {
                        let prev_addr = ctx.app_settings.token_contract_address.clone();
                        let prev_url = ctx.app_settings.l2_network_rpc_url.clone();

                        if s.lcd_display_enabled && (!ctx.app_settings.lcd_display_enabled
                            || ctx.app_settings.lcd_device != s.lcd_device
                            || ctx.app_settings.lcd_addr != s.lcd_addr)
                        {
                            logging::log!("Setting up LCD display with new device parameters...");
                            // TODO: when it fails, send error back to the client,
                            // perhaps we need websockets for errors like this one.
                            tokio::spawn(display_stats_on_lcd(
                                s.clone(),
                                updated_settings_rx.resubscribe(),
                                lcd_stats.clone()
                            ));
                        }

                        ctx.apply_settings(s);

                        if prev_addr != ctx.app_settings.token_contract_address
                            || prev_url != ctx.app_settings.l2_network_rpc_url
                        {
                            token_contract = update_token_contract(&ctx);
                        }
                    }
                },
                _ = ctx.formica_image_pulling.tick() => {
                    let docker_client = docker_client.clone();
                    tokio::spawn(async move {
                        logging::log!("Pulling formica node image ...");
                        if let Err(err) = docker_client.pull_formica_image().await {
                            logging::log!("Failed to pull node image from the periodic task: {err}");
                        }
                    });
                },
                _ = ctx.node_bin_version_check.tick() => {
                    tokio::spawn(check_node_bin_version(
                        docker_client.clone(),
                        latest_bin_version.clone(),
                        db_client.clone(),
                        node_status_locked.clone()
                    ));
                },
                _ = ctx.balances_retrieval.tick() => match token_contract {
                    Some(ref contract) => {
                        tokio::spawn(retrieve_current_rewards_balances(
                            contract.clone(),
                            docker_client.clone(),
                            db_client.clone(),
                            lcd_stats.clone()
                        ));
                    },
                    None => logging::log!("Skipping balances retrieval due to invalid settings")
                },
                _ = ctx.metrics_pruning.tick() => {
                    tokio::spawn(prune_metrics(
                        docker_client.clone(),
                        db_client.clone()
                    ));
                },
                _ = ctx.nodes_metrics_polling.tick() => {
                    let query_bin_version = ctx.app_settings.lcd_display_enabled;

                    // we don't spawn a task for this one just in case it's taking
                    // too long to complete and we may start overwhelming the backend
                    // with multiple overlapping tasks being launched.
                    update_nodes_info(
                        &docker_client,
                        &nodes_metrics,
                        &db_client,
                        &node_status_locked,
                        query_bin_version,
                        &lcd_stats
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
                    match docker_client.get_containers_list(true).await {
                        Ok(containers) => for container in containers.into_iter() {
                            let node_info: NodeInstanceInfo = container.into();
                            update_node_metadata(&node_info, &db_client, &node_status_locked).await;
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
    docker_client: DockerClient,
    latest_bin_version: Arc<Mutex<Option<String>>>,
    db_client: DbClient,
    node_status_locked: ImmutableNodeStatus,
) {
    if let Some(version) = latest_version_available().await {
        logging::log!("Latest version of node binary available: {version}");
        *latest_bin_version.lock().await = Some(version.clone());

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
                .get_outdated_nodes_list(&version)
                .await
                .map(|list| list.first().cloned())
            {
                Ok(Some((container_id, v))) => {
                    logging::log!("Auto-upgrading node binary from v{v} to v{version} for node instance {container_id} ...");
                    if let Err(err) = helper_upgrade_node_instance(
                        &container_id,
                        &node_status_locked,
                        &db_client,
                        &docker_client,
                    )
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
    docker_client: &DockerClient,
    nodes_metrics: &Arc<Mutex<NodesMetrics>>,
    db_client: &DbClient,
    node_status_locked: &ImmutableNodeStatus,
    query_bin_version: bool,
    lcd_stats: &Arc<Mutex<HashMap<String, String>>>,
) {
    let containers = docker_client
        .get_containers_list(true)
        .await
        .unwrap_or_else(|err| {
            logging::log!("Failed to get containers list: {err}");
            vec![]
        });

    let num_nodes = containers.len();
    if num_nodes > 0 {
        logging::log!("Fetching status and metrics from {num_nodes} node/s ...");
    }

    // let's collect stats to update LCD (if enabled)
    let mut net_size = 0;
    let mut weights = 0;
    let mut num_active_nodes = 0;
    let mut records = 0;
    let mut bin_version = HashSet::<String>::new();

    for container in containers.into_iter() {
        let mut node_info: NodeInstanceInfo = container.into();

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
                        update_node_metadata(&node_info, db_client, node_status_locked).await;
                    }
                    Ok(Err(err)) => {
                        logging::log!("Failed to fetch metrics from node {node_short_id}: {err}");
                    }
                    Err(_) => {
                        logging::log!("Timeout ({NODE_METRICS_QUERY_TIMEOUT:?}) while fetching metrics from node {node_short_id}.");
                    }
                }
            }
        }

        net_size +=
            node_info.connected_peers.unwrap_or_default() * node_info.net_size.unwrap_or_default();
        weights += node_info.connected_peers.unwrap_or_default();
        records += node_info.records.unwrap_or_default();
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
    if num_active_nodes > 0 {
        let weighted_avg = if weights > 0 { net_size / weights } else { 0 };
        let bin_versions = bin_version.into_iter().collect::<Vec<_>>().join(", ");

        updated_vals.extend([
            (LCD_LABEL_NET_SIZE, weighted_avg.to_string()),
            (LCD_LABEL_STORED_RECORDS, records.to_string()),
            (LCD_LABEL_BIN_VERSION, bin_versions),
        ]);
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
    };

    update_lcd_stats(lcd_stats, &updated_vals).await;
}

// Prune metrics records from the cache DB to always keep the number of records within a limit.
async fn prune_metrics(docker_client: DockerClient, db_client: DbClient) {
    let containers = match docker_client.get_containers_list(false).await {
        Ok(containers) if !containers.is_empty() => containers,
        Err(err) => {
            logging::log!("Failed to get containers list: {err}");
            return;
        }
        _ => return,
    };

    for container in containers.into_iter() {
        let node_info: NodeInstanceInfo = container.into();
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

async fn retrieve_current_rewards_balances<T: Transport + Clone, P: Provider<T, N>, N: Network>(
    token_contract: TokenContract::TokenContractInstance<T, P, N>,
    docker_client: DockerClient,
    db_client: DbClient,
    lcd_stats: Arc<Mutex<HashMap<String, String>>>,
) {
    // cache retrieved rewards balances to not query more than once per reward address
    let mut updated_balances = HashMap::<Address, U256>::new();

    let containers = match docker_client.get_containers_list(true).await {
        Ok(containers) if !containers.is_empty() => containers,
        Err(err) => {
            logging::log!("Failed to get containers list: {err}");
            remove_lcd_stats(&lcd_stats, &[LCD_LABEL_BALANCE]).await;
            return;
        }
        _ => {
            remove_lcd_stats(&lcd_stats, &[LCD_LABEL_BALANCE]).await;
            return;
        }
    };

    for container in containers.into_iter() {
        let node_info: NodeInstanceInfo = container.into();
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

    let balance: U256 = updated_balances.values().sum();
    update_lcd_stats(&lcd_stats, &[(LCD_LABEL_BALANCE, balance.to_string())]).await;
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
