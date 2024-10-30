use super::{app::ContainerId, node_instance::NodeInstanceInfo};

#[cfg(feature = "ssr")]
use super::{
    app::ServerGlobalState, docker_client::LABEL_KEY_REWARDS_ADDR, node_instance::NodeStatus,
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
    *context.server_api_hit.lock().await = true;
    let latest_bin_version = context.latest_bin_version.lock().await.clone();
    let containers = context.docker_client.get_containers_list(true).await?;

    let mut nodes = HashMap::new();
    for container in containers {
        let mut node_instance_info = NodeInstanceInfo {
            container_id: container.Id.clone(),
            created: container.Created,
            status: NodeStatus::from(&container.State),
            status_info: container.Status.clone(),
            port: container.port(),
            rpc_api_port: container.rpc_api_port(),
            metrics_port: container.metrics_port(),
            node_ip: container.node_ip(),
            rewards_addr: container.Labels.get(LABEL_KEY_REWARDS_ADDR).cloned(),
            ..Default::default()
        };

        // we first read node metadata cached in the database
        // TODO: fetch metadata of all containers from DB with a single DB call
        context
            .db_client
            .get_node_metadata(&mut node_instance_info)
            .await?;

        // if the node is Active, let's also get up to date metrics
        // info retrieved through the metrics server
        if node_instance_info.status.is_active() {
            // TOOD: have all/some of this data to be also cached in DB
            context
                .nodes_metrics
                .lock()
                .await
                .update_node_info(&mut node_instance_info);
        }

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
        status: NodeStatus::from(&container.State),
        status_info: container.Status,
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
        rewards_addr: if rewards_addr.is_empty() {
            None
        } else {
            Some(rewards_addr)
        },
        ..Default::default()
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
    // set connected/kbucket peers back to 0 and update cache
    context
        .db_client
        .update_node_metadata_field(&container_id, "connected_peers", "0")
        .await?;
    context
        .db_client
        .update_node_metadata_field(&container_id, "kbuckets_peers", "0")
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
