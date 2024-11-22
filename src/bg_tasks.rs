use super::{
    app::{METRICS_MAX_SIZE_PER_CONTAINER, METRICS_POLLING_FREQ_MILLIS},
    db_client::DbClient,
    docker_client::DockerClient,
    metrics_client::{NodeMetricsClient, NodesMetrics},
    node_instance::{ContainerId, NodeInstanceInfo},
    node_rpc_client::NodeRpcClient,
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
    sync::Mutex,
    time::{interval, sleep, Duration},
};

// TODO: move all the following consts to become part of AppSettings, an keep
// a copy of current settings in memory/ServerGlobalState

// URL to send queries using RPC to get rewards addresses balances from L2.
const L2_RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";

// ERC20 token contract address.
const ANT_TOKEN_CONTRACT_ADDR: &str = "0xBE1802c27C324a28aeBcd7eeC7D734246C807194";

// How often to fetch metrics and node info from nodes
const NODES_METRICS_POLLING_FREQ: Duration =
    Duration::from_millis(METRICS_POLLING_FREQ_MILLIS as u64);

// Check latest version of node binary every couple of hours
const BIN_VERSION_POLLING_FREQ: Duration = Duration::from_secs(60 * 60 * 2);

// How often to perform a metrics pruning in the DB.
const METRICS_PRUNING_FREQ: Duration = Duration::from_secs(60 * 60); // every hour.

// How often to query balances from the ledger.
const REWARDS_BALANCES_RETRIEVAL: Duration = Duration::from_secs(60 * 15); // every 15mins.

// Frequency to pull a new version of the formica image
const FORMICA_IMAGE_PULLING_FREQ: Duration = Duration::from_secs(60 * 60 * 6); // every 6 hours.

// Create ERC20 contract instance
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    TokenContract,
    "artifacts/token_contract_abi.json"
);

// Spawn any required background tasks
pub fn spawn_bg_tasks(
    docker_client: DockerClient,
    latest_bin_version: Arc<Mutex<Option<String>>>,
    nodes_metrics: Arc<Mutex<NodesMetrics>>,
    db_client: DbClient,
    server_api_hit: Arc<Mutex<bool>>,
    node_status_locked: Arc<Mutex<HashSet<ContainerId>>>,
) {
    let mut formica_image_pulling = interval(FORMICA_IMAGE_PULLING_FREQ);
    let mut node_bin_version_check = interval(BIN_VERSION_POLLING_FREQ);
    let mut balances_retrieval = interval(REWARDS_BALANCES_RETRIEVAL);
    let mut metrics_pruning = interval(METRICS_PRUNING_FREQ);
    let mut nodes_metrics_polling = interval(NODES_METRICS_POLLING_FREQ);

    // we start a counter to stop polling RPC API when there is no active client
    let mut poll_rpc_counter = 5;

    // FIXME: remove unwrap calls
    let token_address: Address = ANT_TOKEN_CONTRACT_ADDR.parse().unwrap();
    let provider = ProviderBuilder::new().on_http(L2_RPC_URL.parse().unwrap());
    let token_contract = TokenContract::new(token_address, provider);

    tokio::spawn(async move {
        loop {
            select! {
                _ = formica_image_pulling.tick() => {
                    let docker_client_clone = docker_client.clone();
                    tokio::spawn(async move {
                        logging::log!("Pulling formica node image ...");
                        if let Err(err) = docker_client_clone.pull_formica_image().await {
                            logging::log!("Failed to pull node image from the periodic task: {err}");
                        }
                    });
                },
                _ = node_bin_version_check.tick() => {
                    tokio::spawn(check_node_bin_version(
                        docker_client.clone(),
                        latest_bin_version.clone(),
                        db_client.clone(),
                        node_status_locked.clone(),
                    ));
                },
                _ = balances_retrieval.tick() => {
                    tokio::spawn(retrieve_current_rewards_balances(
                        token_contract.clone(),
                        docker_client.clone(),
                        db_client.clone()
                    ));
                },
                _ = metrics_pruning.tick() => {
                    tokio::spawn(prune_metrics(
                        docker_client.clone(),
                        db_client.clone()
                    ));
                },
                _ = nodes_metrics_polling.tick() => {
                    if *server_api_hit.lock().await {
                        // reset the countdown to five more cycles
                        poll_rpc_counter = 5;
                        *server_api_hit.lock().await = false;
                    } else if poll_rpc_counter > 0 {
                        poll_rpc_counter -= 1;
                    }
                    let poll_rpc_api = poll_rpc_counter > 0;

                    // we don't spawn a task for this one just in case it's taking
                    // too long to complete and we may start overwhelming the backend
                    // with multiple overlapping tasks being launched.
                    update_nodes_info(
                        docker_client.clone(),
                        nodes_metrics.clone(),
                        db_client.clone(),
                        node_status_locked.clone(),
                        poll_rpc_api
                    ).await;
                    // reset timer to start next period from this instant,
                    // regardless how long the above polling task lasted.
                    nodes_metrics_polling.reset_after(NODES_METRICS_POLLING_FREQ);
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
    node_status_locked: Arc<Mutex<HashSet<ContainerId>>>,
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

                    let delay = Duration::from_secs(
                        db_client.get_settings().await.nodes_auto_upgrade_delay_secs,
                    );
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
    let url = format!("https://crates.io/api/v1/crates/{}", "sn_node");
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

// Fetch up to date information for each active node instance
// from different sources caching them in global context:
//    - Nodes' RPC API to get binary version and peer id.
//    - Nodes' exposed metrics server to obtain stats.
async fn update_nodes_info(
    docker_client: DockerClient,
    nodes_metrics: Arc<Mutex<NodesMetrics>>,
    db_client: DbClient,
    node_status_locked: Arc<Mutex<HashSet<ContainerId>>>,
    poll_rpc_api: bool,
) {
    let containers = match docker_client.get_containers_list(false).await {
        Ok(containers) if !containers.is_empty() => containers,
        Err(err) => {
            logging::log!("Failed to get containers list: {err}");
            return;
        }
        _ => {
            logging::log!("No active nodes to retrieve metrics from...");
            return;
        }
    };

    logging::log!("Fetching metrics from {} node/s ...", containers.len());
    for container in containers.into_iter() {
        let mut node_info: NodeInstanceInfo = container.into();

        if poll_rpc_api {
            // let's fetch up to date info using its RPC API
            if let Some(port) = node_info.rpc_api_port {
                match NodeRpcClient::new(&node_info.node_ip, port) {
                    Ok(node_rpc_client) => {
                        node_rpc_client.update_node_info(&mut node_info).await;
                    }
                    Err(err) => {
                        logging::log!("Failed to connect to RPC API endpoint: {err}")
                    }
                }
            }
        }

        if let Some(metrics_port) = node_info.metrics_port {
            // let's now collect metrics from the node
            let metrics_client = NodeMetricsClient::new(&node_info.node_ip, metrics_port);
            match metrics_client.fetch_metrics().await {
                Ok(metrics) => {
                    let mut node_metrics = nodes_metrics.lock().await;
                    node_metrics.store(&node_info.container_id, &metrics).await;
                    node_metrics.update_node_info(&mut node_info);
                }
                Err(err) => logging::log!("Failed to fetch node metrics: {err}"),
            }
        }

        let update_status = !node_status_locked
            .lock()
            .await
            .contains(&node_info.container_id);
        db_client
            .update_node_metadata(&node_info, update_status)
            .await;
    }
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
) {
    // cache retrieved rewards balances to not query more than once per reward address
    let mut updated_balances = HashMap::<Address, U256>::new();

    let containers = match docker_client.get_containers_list(true).await {
        Ok(containers) if !containers.is_empty() => containers,
        Err(err) => {
            logging::log!("Failed to get containers list: {err}");
            return;
        }
        _ => return,
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
                *balance
            } else {
                // query the balance to the ERC20 contract
                logging::log!("Querying rewards balance for node {node_short_id} ...");
                match token_contract.balanceOf(address).call().await {
                    Ok(balance) => {
                        let balance = balance._0;
                        updated_balances.insert(address, balance);
                        balance
                    }
                    Err(err) => {
                        logging::log!(
                            "Failed to query rewards balance for node {node_short_id}: {err}"
                        );
                        continue;
                    }
                }
            };

            db_client
                .update_node_metadata_fields(
                    &node_info.container_id,
                    &[("balance", &new_balance.to_string())],
                )
                .await;
        } else {
            logging::log!("No valid rewards address set for node {node_short_id}.");
        }
    }
}
