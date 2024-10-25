use super::{app::ContainerId, node_instance::NodeInstanceInfo};

#[cfg(feature = "ssr")]
use super::{
    app::ServerGlobalState, docker_client::LABEL_KEY_REWARDS_ADDR,
    metrics_client::NodeMetricsClient, node_instance::NodeStatus, node_rpc_client::NodeRpcClient,
};

#[cfg(feature = "ssr")]
use futures_util::StreamExt;
use leptos::*;
use serde::{Deserialize, Serialize};
use server_fn::codec::{ByteStream, Streaming};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
pub struct NodesInstancesInfo {
    pub latest_bin_version: Option<String>,
    pub nodes: HashMap<String, NodeInstanceInfo>,
}

// Obtain the list of existing nodes instances with their info
#[server(ListNodeInstances, "/api", "Url", "/list_nodes")]
pub async fn nodes_instances() -> Result<NodesInstancesInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let latest_bin_version = context.latest_bin_version.lock().await.clone();
    let containers = context.docker_client.get_containers_list().await?;

    let mut nodes = HashMap::new();
    for container in containers {
        let mut node_instance_info = NodeInstanceInfo {
            container_id: container.Id.clone(),
            created: container.Created,
            peer_id: None,
            status: NodeStatus::from(&container.State),
            status_info: container.Status.clone(),
            bin_version: None,
            port: container.port(),
            rpc_api_port: container.rpc_api_port(),
            metrics_port: container.metrics_port(),
            node_ip: container.node_ip(),
            balance: None,
            rewards_addr: container.Labels.get(LABEL_KEY_REWARDS_ADDR).cloned(),
            records: None,
            relevant_records: None,
            store_cost: None,
            mem_used: None,
            cpu_usage: None,

            connected_peers: None,
            kbuckets_peers: None,
        };

        // we first read node metadata cached in the database
        // TODO: fetch metadata of all containers from DB with a single DB call
        context
            .db_client
            .get_node_metadata(&mut node_instance_info)
            .await?;

        // if the node is Active, we can also fetch up to date info using its RPC API
        retrive_and_cache_updated_metadata(&mut node_instance_info).await?;

        nodes.insert(container.Id, node_instance_info);
    }

    Ok(NodesInstancesInfo {
        latest_bin_version,
        nodes,
    })
}

// Create and add a new node instance returning its info
// TODO: read node instances metadata form a database
#[server(CreateNodeInstance, "/api", "Url", "/create_node")]
pub async fn create_node_instance(
    port: u16,
    rpc_api_port: u16,
    metrics_port: u16,
    rewards_addr: String,
) -> Result<NodeInstanceInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Creating new node container with port {port}, RPC API port {rpc_api_port} ...");
    let container_id = context
        .docker_client
        .create_new_container(port, rpc_api_port, metrics_port, rewards_addr.clone())
        .await?;
    logging::log!("New node container Id: {container_id} ...");

    let container = context
        .docker_client
        .get_container_info(&container_id)
        .await?;
    logging::log!("New node container created: {container:?}");

    let node_instance_info = NodeInstanceInfo {
        container_id: container.Id,
        created: container.Created,
        peer_id: None,
        status: NodeStatus::from(&container.State),
        status_info: container.Status,
        bin_version: None,
        port: Some(port),
        rpc_api_port: Some(rpc_api_port),
        metrics_port: Some(metrics_port),
        node_ip: container
            .NetworkSettings
            .Networks
            .get("bridge")
            .and_then(|n| {
                if n.IPAddress.is_empty() {
                    None
                } else {
                    Some(n.IPAddress.clone())
                }
            }),
        balance: None,
        rewards_addr: if rewards_addr.is_empty() {
            None
        } else {
            Some(rewards_addr)
        },
        records: None,
        relevant_records: None,
        store_cost: None,
        mem_used: None,
        cpu_usage: None,
        connected_peers: None,
        kbuckets_peers: None,
    };

    context
        .db_client
        .store_node_metadata(&node_instance_info)
        .await?;

    Ok(node_instance_info)
}

// Delete a node instance with given id
#[server(DeleteNodeInstance, "/api", "Url", "/delete_node")]
pub async fn delete_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    logging::log!("Deleting node container with Id: {container_id} ...");
    let context = expect_context::<ServerGlobalState>();
    context
        .docker_client
        .delete_container_with(&container_id)
        .await?;
    context
        .db_client
        .delete_node_metadata(&container_id)
        .await?;
    Ok(())
}

// Start a node instance with given id
#[server(StartNodeInstance, "/api", "Url", "/start_node")]
pub async fn start_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    logging::log!("Starting node container with Id: {container_id} ...");
    let context = expect_context::<ServerGlobalState>();
    context
        .docker_client
        .start_container_with(&container_id)
        .await?;
    Ok(())
}

// Stop a node instance with given id
#[server(StopNodeInstance, "/api", "Url", "/stop_node")]
pub async fn stop_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    logging::log!("Stopping node container with Id: {container_id} ...");
    let context = expect_context::<ServerGlobalState>();
    context
        .docker_client
        .stop_container_with(&container_id)
        .await?;
    // set connect_peers back to 0 and update cache
    context
        .db_client
        .update_node_metadata_field(&container_id, "connected_peers", "0")
        .await?;

    Ok(())
}

// Upgrade a node instance with given id
#[server(UpgradeNodeInstance, "/api", "Url", "/upgrade_node")]
pub async fn upgrade_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    logging::log!("Upgrading node container with Id: {container_id} ...");
    let context = expect_context::<ServerGlobalState>();
    context
        .docker_client
        .upgrade_node_in_container_with(&container_id)
        .await?;
    Ok(())
}

// Start streaming logs from a node instance with given id
#[server(output = Streaming)]
pub async fn start_node_logs_stream(
    container_id: ContainerId,
) -> Result<ByteStream, ServerFnError> {
    logging::log!("Starting logs stream from container with Id: {container_id} ...");
    let context = expect_context::<ServerGlobalState>();
    let container_logs_stream = context
        .docker_client
        .get_container_logs_stream(&container_id)
        .await?;
    let converted_stream = container_logs_stream.map(|item| {
        item.map_err(ServerFnError::from) // convert the error type
    });
    Ok(ByteStream::new(converted_stream))
}

// Retrieve the metrics for a node instance with given id and filters
#[server(NodeMetrics, "/api", "Url", "/node_metrics")]
pub async fn node_metrics(
    container_id: ContainerId,
    since: Option<i64>,
    keys: Vec<String>,
) -> Result<HashMap<String, Vec<super::app::NodeMetric>>, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let metrics = context
        .nodes_metrics
        .lock()
        .await
        .get_metrics(&container_id, since, &keys);
    Ok(metrics)
}

// If the node is active, retrieve up to date node's metadata through
// its RPC API and update its cache in local database.
#[cfg(feature = "ssr")]
async fn retrive_and_cache_updated_metadata(
    node_instance_info: &mut NodeInstanceInfo,
) -> Result<(), ServerFnError> {
    if node_instance_info.status.is_active() {
        let context = expect_context::<ServerGlobalState>();
        if let Some(port) = node_instance_info.rpc_api_port {
            // TODO: send info back to the user if we receive an error from using RPC client.
            match NodeRpcClient::new(&node_instance_info.node_ip, port) {
                Ok(mut node_rpc_client) => {
                    node_rpc_client.update_node_info(node_instance_info).await
                }
                Err(err) => logging::log!("Failed to connect to RPC API endpoint: {err}"),
            }
        }

        // update with info retrieved through the metrics server
        NodeMetricsClient::update_node_info(node_instance_info).await;

        // update DB with this new info we just obtained
        context
            .db_client
            .store_node_metadata(&node_instance_info)
            .await?;
    }

    Ok(())
}
