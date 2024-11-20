use super::node_instance::{ContainerId, NodeInstanceInfo};

#[cfg(feature = "ssr")]
use super::{
    app::ServerGlobalState,
    db_client::DbClient,
    docker_client::{DockerClient, DockerClientError},
    node_instance::NodeStatus,
};

#[cfg(feature = "ssr")]
use futures_util::StreamExt;
use leptos::*;
use serde::{Deserialize, Serialize};
use server_fn::codec::{ByteStream, Streaming};
use std::collections::HashMap;

#[cfg(feature = "ssr")]
use std::{collections::HashSet, sync::Arc};
#[cfg(feature = "ssr")]
use tokio::sync::Mutex;

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
        let mut node_info = container.into();

        // we first read node metadata cached in the database
        // TODO: fetch metadata of all containers from DB with a single DB call
        context.db_client.get_node_metadata(&mut node_info).await;

        // if the node is Active, let's also get up to date metrics
        // info that was retrieved through the metrics server
        if node_info.status.is_active() {
            context
                .nodes_metrics
                .lock()
                .await
                .update_node_info(&mut node_info);
        }

        nodes.insert(node_info.container_id.clone(), node_info);
    }

    Ok(NodesInstancesInfo {
        latest_bin_version,
        nodes,
    })
}

// Create and add a new node instance returning its info
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

    let node_info = container.into();
    context.db_client.insert_node_metadata(&node_info).await;

    Ok(node_info)
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
    context.db_client.delete_node_metadata(&container_id).await;
    context
        .nodes_metrics
        .lock()
        .await
        .remove_container_metrics(&container_id)
        .await;

    Ok(())
}

// Start a node instance with given id
#[server(StartNodeInstance, "/api", "Url", "/start_node")]
pub async fn start_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    logging::log!("Starting node container with Id: {container_id} ...");
    let context = expect_context::<ServerGlobalState>();
    context
        .db_client
        .update_node_status(&container_id, NodeStatus::Restarting)
        .await;
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
        .node_status_locked
        .lock()
        .await
        .insert(container_id.clone());
    context
        .db_client
        .update_node_status(&container_id, NodeStatus::Stopping)
        .await;

    let res = context
        .docker_client
        .stop_container_with(&container_id)
        .await;

    if matches!(res, Ok(())) {
        // set connected/kbucket peers back to 0 and update cache
        context
            .db_client
            .update_node_metadata_fields(
                &container_id,
                &[("connected_peers", "0"), ("kbuckets_peers", "0")],
            )
            .await;
        context
            .db_client
            .update_node_status(&container_id, NodeStatus::Inactive)
            .await;
    }

    context
        .node_status_locked
        .lock()
        .await
        .remove(&container_id);

    Ok(res?)
}

// Upgrade a node instance with given id
#[server(UpgradeNodeInstance, "/api", "Url", "/upgrade_node")]
pub async fn upgrade_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    logging::log!("Upgrading node container with Id: {container_id} ...");
    let context = expect_context::<ServerGlobalState>();

    helper_upgrade_node_instance(
        &container_id,
        &context.node_status_locked,
        &context.db_client,
        &context.docker_client,
    )
    .await?;

    Ok(())
}

/// Helper to upgrade a node instance with given id
#[cfg(feature = "ssr")]
pub(crate) async fn helper_upgrade_node_instance(
    container_id: &ContainerId,
    node_status_locked: &Arc<Mutex<HashSet<ContainerId>>>,
    db_client: &DbClient,
    docker_client: &DockerClient,
) -> Result<(), DockerClientError> {
    // TODO: use docker 'extract' api to simply copy the new node binary into the container.
    node_status_locked.lock().await.insert(container_id.clone());
    db_client
        .update_node_status(container_id, NodeStatus::Upgrading)
        .await;

    let res = docker_client
        .upgrade_node_in_container_with(container_id)
        .await;

    if matches!(res, Ok(())) {
        // set bin_version to 'unknown', otherwise it can be confusing while the
        // node is restarting what version it really is running.
        db_client
            .update_node_metadata_fields(container_id, &[("bin_version", "")])
            .await;
        db_client
            .update_node_status(
                &container_id,
                NodeStatus::Transitioned("Upgraded".to_string()),
            )
            .await;
    }

    node_status_locked.lock().await.remove(container_id);

    Ok(res?)
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
) -> Result<HashMap<String, Vec<super::app::NodeMetric>>, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let metrics = context
        .nodes_metrics
        .lock()
        .await
        .get_container_metrics(container_id, since)
        .await;

    Ok(metrics)
}

// Retrieve the settings
#[server(GetSettings, "/api", "Url", "/get_settings")]
pub async fn get_settings() -> Result<super::app::AppSettings, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let settings = context.db_client.get_settings().await;

    Ok(settings)
}

// Update the settings
#[server(UpdateSettings, "/api", "Url", "/update_settings")]
pub async fn update_settings(settings: super::app::AppSettings) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    context.db_client.update_settings(settings).await?;
    Ok(())
}
