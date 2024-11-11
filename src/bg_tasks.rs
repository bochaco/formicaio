use super::{
    app::{METRICS_MAX_SIZE_PER_CONTAINER, METRICS_POLLING_FREQ_MILLIS},
    db_client::DbClient,
    docker_client::{DockerClient, LABEL_KEY_REWARDS_ADDR},
    metrics_client::{NodeMetricsClient, NodesMetrics},
    node_instance::NodeInstanceInfo,
    node_rpc_client::NodeRpcClient,
};
use leptos::logging;
use std::sync::Arc;
use tokio::{
    sync::Mutex,
    time::{sleep, Duration},
};

// Spawn any required background tasks
pub fn spawn_bg_tasks(
    docker_client: DockerClient,
    latest_bin_version: Arc<Mutex<Option<String>>>,
    nodes_metrics: Arc<Mutex<NodesMetrics>>,
    db_client: DbClient,
    server_api_hit: Arc<Mutex<bool>>,
) {
    // Check latest version of node binary every couple of hours
    const BIN_VERSION_POLLING_FREQ: Duration = Duration::from_secs(60 * 60 * 2);

    tokio::spawn(async move {
        loop {
            if let Some(version) = latest_version_available().await {
                logging::log!("Latest version of node binary available: {version}");
                *latest_bin_version.lock().await = Some(version);
            }
            sleep(BIN_VERSION_POLLING_FREQ).await;
        }
    });

    // Let's pull the node image already to reduce the time it'll take
    // to create the very first node instance.
    // Also, attempt to pull a new version of the formica image every six hours
    const FORMICA_IMAGE_PULLING_FREQ: Duration = Duration::from_secs(60 * 60 * 6);

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

    tokio::spawn(update_nodes_info(
        docker_client,
        nodes_metrics,
        db_client,
        server_api_hit,
    ));
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
//
// It also prunes historic nodes metrics data from the cache DB.
async fn update_nodes_info(
    docker_client: DockerClient,
    nodes_metrics: Arc<Mutex<NodesMetrics>>,
    db_client: DbClient,
    server_api_hit: Arc<Mutex<bool>>,
) {
    // Collect metrics from nodes and cache them in global context
    const NODES_METRICS_POLLING_FREQ: Duration =
        Duration::from_millis(METRICS_POLLING_FREQ_MILLIS as u64);

    // How many cycles of metrics polling before performing a clean up in the DB.
    const METRICS_CLEAN_UP: u32 = 3_600_000 / METRICS_POLLING_FREQ_MILLIS; // every ~1hr.

    // we start a countdown to stop polling RPC API when there is no active client
    let mut poll_rpc_countdown = 1;

    // we do a clean up of the cache DB to always keep the number of records within a limit.
    let mut metrics_pruning_countdown = 0; // we will do a first clean up when starting up.

    loop {
        sleep(NODES_METRICS_POLLING_FREQ).await;

        let containers = match docker_client.get_containers_list(false).await {
            Ok(containers) if !containers.is_empty() => containers,
            Err(err) => {
                logging::log!("Failed to get containers list: {err}");
                continue;
            }
            _ => continue,
        };

        if *server_api_hit.lock().await {
            // reset the countdown to five more cycles
            poll_rpc_countdown = 5;
            *server_api_hit.lock().await = false;
        } else if poll_rpc_countdown > 0 {
            poll_rpc_countdown -= 1;
        }

        logging::log!("Polling {} node/s metrics service...", containers.len());
        for container in containers {
            let node_ip = container.node_ip();
            let mut node_info = NodeInstanceInfo {
                container_id: container.Id.clone(),
                port: container.port(),
                rpc_api_port: container.rpc_api_port(),
                rewards_addr: container.Labels.get(LABEL_KEY_REWARDS_ADDR).cloned(),
                ..Default::default()
            };

            if poll_rpc_countdown > 0 {
                // let's fetch up to date info using its RPC API
                if let Some(port) = node_info.rpc_api_port {
                    match NodeRpcClient::new(&node_ip, port) {
                        Ok(node_rpc_client) => {
                            node_rpc_client.update_node_info(&mut node_info).await;
                        }
                        Err(err) => {
                            logging::log!("Failed to connect to RPC API endpoint: {err}")
                        }
                    }
                }
            }

            if let Some(metrics_port) = container.metrics_port() {
                // let's now collect metrics from the node
                let metrics_client = NodeMetricsClient::new(&node_ip, metrics_port);
                match metrics_client.fetch_metrics().await {
                    Ok(metrics) => {
                        let mut node_metrics = nodes_metrics.lock().await;
                        node_metrics.store(&container.Id, &metrics).await;
                        node_metrics.update_node_info(&mut node_info);
                    }
                    Err(err) => logging::log!("Failed to fetch node metrics: {err}"),
                }
            }

            if let Err(err) = db_client.update_node_metadata(&node_info).await {
                logging::log!(
                    "Failed to update DB cache for node {}: {err}",
                    node_info.short_container_id()
                );
            }

            if metrics_pruning_countdown == 0 {
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

        if metrics_pruning_countdown == 0 {
            metrics_pruning_countdown = METRICS_CLEAN_UP;
        }
        metrics_pruning_countdown -= 1;
    }
}
