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
    sync::Mutex,
    time::{sleep, Duration},
};

// TODO: move all the following consts to become part of AppSettings, an keep
// a copy of current settings in memory/ServerGlobalState

// URL to send queries using RPC to get rewards addresses balances from L2.
const L2_RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";

// ERC20 token contract address.
const ANT_TOKEN_CONTRACT_ADDR: &str = "0xBE1802c27C324a28aeBcd7eeC7D734246C807194";

// Check latest version of node binary every couple of hours
const BIN_VERSION_POLLING_FREQ: Duration = Duration::from_secs(60 * 60 * 2);

// How many cycles of metrics polling before performing a metrics pruning in the DB.
const METRICS_PRUNING: u32 = 3_600_000 / METRICS_POLLING_FREQ_MILLIS; // every ~1hr.

// How many cycles of metrics polling before querying balances from the ledger.
const REWARDS_BALANCES_RETRIEVAL: u32 = 900_000 / METRICS_POLLING_FREQ_MILLIS; // every ~15mins.

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
    // Let's pull the node image already to reduce the time it'll take
    // to create the very first node instance.
    // Also, attempt to pull a new version of the formica image every six hours
    let docker_client_clone = docker_client.clone();
    tokio::spawn(async move {
        loop {
            logging::log!("Pulling formica node image ...");
            if let Err(err) = docker_client_clone.pull_formica_image().await {
                logging::log!("Failed to pull node image when starting up: {err}");
            }
            sleep(FORMICA_IMAGE_PULLING_FREQ).await;
        }
    });

    // Spawn a task which checks nodes binary versions against latest available
    tokio::spawn(check_node_bin_version(
        docker_client.clone(),
        latest_bin_version,
        db_client.clone(),
        node_status_locked.clone(),
    ));

    // Spawn a task to update nodes info and metrics
    tokio::spawn(update_nodes_info(
        docker_client,
        nodes_metrics,
        db_client,
        server_api_hit,
        node_status_locked,
    ));
}

// Check latest version of node binary and upgrade nodes
// automatically if auto-upgrade was enabled by the user.
async fn check_node_bin_version(
    docker_client: DockerClient,
    latest_bin_version: Arc<Mutex<Option<String>>>,
    db_client: DbClient,
    node_status_locked: Arc<Mutex<HashSet<ContainerId>>>,
) {
    loop {
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

        sleep(BIN_VERSION_POLLING_FREQ).await;
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

// Periodically fetch up to date information for each active node instance
// from different sources:
//    - Nodes' RPC API to get binary version and peer id.
//    - Nodes' exposed metrics server to obtain stats.
//    - L2 ledger to retrieve rewards addresses current balances.
//
// It also prunes historic nodes metrics data from the cache DB.
async fn update_nodes_info(
    docker_client: DockerClient,
    nodes_metrics: Arc<Mutex<NodesMetrics>>,
    db_client: DbClient,
    server_api_hit: Arc<Mutex<bool>>,
    node_status_locked: Arc<Mutex<HashSet<ContainerId>>>,
) -> Result<(), eyre::Error> {
    // we start a counter to stop polling RPC API when there is no active client
    let mut poll_rpc_counter = SelfResetCounter::new(5);

    // we do a clean up of the cache DB to always keep the number of records within a limit.
    // we will do a first clean up when starting up.
    let mut metrics_pruning_counter = SelfResetCounter::start_in_max(METRICS_PRUNING);

    // we retrieve balances of rewards addresses from the ledger directly.
    // we will do a first retrieval when starting up.
    let mut balances_retrieval_counter = SelfResetCounter::start_in_max(REWARDS_BALANCES_RETRIEVAL);

    let token_address: Address = ANT_TOKEN_CONTRACT_ADDR.parse()?;
    let provider = ProviderBuilder::new().on_http(L2_RPC_URL.parse()?);
    let token_contract = TokenContract::new(token_address, provider);

    // Collect metrics from nodes and cache them in global context
    const NODES_METRICS_POLLING_FREQ: Duration =
        Duration::from_millis(METRICS_POLLING_FREQ_MILLIS as u64);

    loop {
        sleep(NODES_METRICS_POLLING_FREQ).await;

        let retrieve_balances = balances_retrieval_counter.is_max();

        let containers = match docker_client.get_containers_list(retrieve_balances).await {
            Ok(containers) if !containers.is_empty() => containers,
            Err(err) => {
                logging::log!("Failed to get containers list: {err}");
                continue;
            }
            _ => continue,
        };

        if *server_api_hit.lock().await {
            // reset the countdown to five more cycles
            poll_rpc_counter.reset();
            *server_api_hit.lock().await = false;
        } else if !poll_rpc_counter.is_max() {
            poll_rpc_counter.increment();
        }

        // cache retrieved rewards balances to not query more than once per reward address
        let mut updated_balances = HashMap::new();

        logging::log!("Fetching metrics for {} node/s ...", containers.len());
        for container in containers.into_iter() {
            let mut node_info: NodeInstanceInfo = container.into();

            // we collect up to date metrics only from active nodes
            if node_info.status.is_active() {
                fetch_new_metrics(
                    &mut node_info,
                    &nodes_metrics,
                    &db_client,
                    &poll_rpc_counter,
                    &metrics_pruning_counter,
                )
                .await;
            }

            if retrieve_balances {
                retrieve_current_rewards_balances(
                    &mut node_info,
                    &mut updated_balances,
                    &token_contract,
                )
                .await;
            }

            let update_status = !node_status_locked
                .lock()
                .await
                .contains(&node_info.container_id);
            db_client
                .update_node_metadata(&node_info, update_status)
                .await;
        }

        balances_retrieval_counter.increment();
        metrics_pruning_counter.increment();
    }
}

async fn fetch_new_metrics(
    mut node_info: &mut NodeInstanceInfo,
    nodes_metrics: &Arc<Mutex<NodesMetrics>>,
    db_client: &DbClient,
    poll_rpc_counter: &SelfResetCounter,
    metrics_pruning_counter: &SelfResetCounter,
) {
    if !poll_rpc_counter.is_max() {
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

    if metrics_pruning_counter.is_max() {
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
    node_info: &mut NodeInstanceInfo,
    updated_balances: &mut HashMap<Address, U256>,
    token_contract: &TokenContract::TokenContractInstance<T, P, N>,
) {
    let node_short_id = node_info.short_container_id();
    if let Some(Ok(address)) = node_info
        .rewards_addr
        .as_ref()
        .map(|addr| addr.parse::<Address>())
    {
        if let Some(balance) = updated_balances.get(&address) {
            node_info.balance = Some(*balance);
        } else {
            // query the balance to the ERC20 contract
            logging::log!("Querying rewards balance for node {node_short_id} ...");
            match token_contract.balanceOf(address).call().await {
                Ok(balance) => {
                    let balance = balance._0;
                    updated_balances.insert(address, balance);
                    node_info.balance = Some(balance);
                }
                Err(err) => {
                    logging::log!("Failed to query rewards balance for node {node_short_id}: {err}")
                }
            }
        }
    } else {
        logging::log!("No valid rewards address set for node {node_short_id}.");
    }
}

// Helper to maintain a few counters in the background task
struct SelfResetCounter {
    max: u32,
    current: u32,
}

impl SelfResetCounter {
    fn new(max: u32) -> Self {
        Self { max, current: 0 }
    }

    fn start_in_max(max: u32) -> Self {
        Self { max, current: max }
    }

    fn is_max(&self) -> bool {
        self.max == self.current
    }

    fn reset(&mut self) {
        self.current = 0;
    }

    fn increment(&mut self) {
        if self.is_max() {
            self.current = 0;
        } else {
            self.current += 1;
        }
    }
}
