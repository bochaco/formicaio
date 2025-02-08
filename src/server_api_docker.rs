use super::{
    node_instance::{NodeId, NodeInstanceInfo},
    server_api_types::{NodeOpts, NodesInstancesInfo},
};

use leptos::prelude::*;
#[cfg(feature = "ssr")]
use std::collections::HashMap;

#[cfg(feature = "ssr")]
mod ssr_imports_and_defs {
    pub use crate::{
        app::{BgTasksCmds, ImmutableNodeStatus, ServerGlobalState},
        db_client::DbClient,
        docker_client::{DockerClient, DockerClientError, UPGRADE_NODE_BIN_TIMEOUT_SECS},
        node_instance::NodeStatus,
        server_api_types::BatchInProgress,
    };
    pub use alloy_primitives::Address;
    pub use chrono::Utc;
    pub use futures_util::StreamExt;
    pub use leptos::logging;
    pub use std::time::Duration;
}

#[cfg(feature = "ssr")]
use ssr_imports_and_defs::*;

// Obtain the list of existing nodes instances with their info
#[server(ListNodeInstances, "/api", "Url", "/list_nodes")]
pub async fn nodes_instances() -> Result<NodesInstancesInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let latest_bin_version = context.latest_bin_version.lock().await.clone();
    let nodes_list = context.docker_client.get_containers_list(true).await?;
    let stats = context.stats.lock().await.clone();
    *context.server_api_hit.lock().await = true;

    let mut nodes = HashMap::new();
    for mut node_info in nodes_list.into_iter() {
        // we first read node metadata cached in the database
        // TODO: fetch metadata of all nodes from DB with a single DB call
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

        nodes.insert(node_info.node_id.clone(), node_info);
    }

    let batches = &context.node_instaces_batches.lock().await.1;
    let batch_in_progress = if let Some(b) = batches.first() {
        let init = BatchInProgress {
            auto_start: b.node_opts.auto_start,
            interval_secs: b.interval_secs,
            ..Default::default()
        };
        Some(batches.iter().fold(init, |mut acc, b| {
            acc.created += b.created;
            acc.total += b.total;
            acc
        }))
    } else {
        None
    };

    Ok(NodesInstancesInfo {
        latest_bin_version: latest_bin_version.map(|v| v.to_string()),
        nodes,
        stats,
        batch_in_progress,
    })
}

// Create and add a new node instance returning its info
#[server(CreateNodeInstance, "/api", "Url", "/create_node")]
pub async fn create_node_instance(node_opts: NodeOpts) -> Result<NodeInstanceInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    helper_create_node_instance(node_opts, &context).await
}

/// Helper to create a node instance
#[cfg(feature = "ssr")]
pub async fn helper_create_node_instance(
    node_opts: NodeOpts,
    context: &ServerGlobalState,
) -> Result<NodeInstanceInfo, ServerFnError> {
    logging::log!("Creating new node with port {} ...", node_opts.port);
    let auto_start = node_opts.auto_start;
    let _ = node_opts.rewards_addr.parse::<Address>()?;

    let node_id = context
        .docker_client
        .create_new_container(node_opts)
        .await?;
    logging::log!("New node Id: {node_id} ...");

    let mut node_info = context.docker_client.get_container_info(&node_id).await?;
    logging::log!("New node created: {node_info:?}");

    context.db_client.insert_node_metadata(&node_info).await;

    if auto_start {
        helper_start_node_instance(node_id.clone(), context).await?;
        node_info = context.docker_client.get_container_info(&node_id).await?;
    }

    context
        .bg_tasks_cmds_tx
        .send(BgTasksCmds::CheckBalanceFor(node_info.clone()))?;

    Ok(node_info)
}

// Delete a node instance with given id
#[server(DeleteNodeInstance, "/api", "Url", "/delete_node")]
pub async fn delete_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Deleting node node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    let node_info = context.docker_client.get_container_info(&node_id).await?;
    context.docker_client.delete_container(&node_id).await?;
    context.db_client.delete_node_metadata(&node_id).await;
    context
        .nodes_metrics
        .lock()
        .await
        .remove_node_metrics(&node_id)
        .await;

    context
        .bg_tasks_cmds_tx
        .send(BgTasksCmds::DeleteBalanceFor(node_info))?;

    Ok(())
}

// Start a node instance with given id
#[server(StartNodeInstance, "/api", "Url", "/start_node")]
pub async fn start_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    helper_start_node_instance(node_id, &context).await
}

// Helper to start a node instance with given id
#[cfg(feature = "ssr")]
async fn helper_start_node_instance(
    node_id: NodeId,
    context: &ServerGlobalState,
) -> Result<(), ServerFnError> {
    logging::log!("Starting node with Id: {node_id} ...");

    context
        .db_client
        .update_node_status(&node_id, NodeStatus::Restarting)
        .await;

    let (version, peer_id, ips) = context
        .docker_client
        .start_container(&node_id, true)
        .await?;
    context
        .db_client
        .update_node_metadata_fields(
            &node_id,
            &[
                ("status_changed", &Utc::now().timestamp().to_string()),
                ("bin_version", &version.unwrap_or_default()),
                ("peer_id", &peer_id.unwrap_or_default()),
                ("ips", &ips.unwrap_or_default()),
            ],
        )
        .await;

    Ok(())
}

// Stop a node instance with given id
#[server(StopNodeInstance, "/api", "Url", "/stop_node")]
pub async fn stop_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Stopping node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    helper_stop_node_instance(node_id, &context, NodeStatus::Stopping).await
}

// Helper to stop a node instance with given id
#[cfg(feature = "ssr")]
async fn helper_stop_node_instance(
    node_id: NodeId,
    context: &ServerGlobalState,
    status: NodeStatus,
) -> Result<(), ServerFnError> {
    context
        .node_status_locked
        .insert(node_id.clone(), Duration::from_secs(20))
        .await;
    context.db_client.update_node_status(&node_id, status).await;

    let res = context.docker_client.stop_container(&node_id).await;

    if matches!(res, Ok(())) {
        // set connected/kbucket peers back to 0 and update cache
        context
            .db_client
            .update_node_metadata_fields(
                &node_id,
                &[
                    ("status_changed", &Utc::now().timestamp().to_string()),
                    ("connected_peers", "0"),
                    ("kbuckets_peers", "0"),
                    ("records", ""),
                    ("ips", ""),
                ],
            )
            .await;
        context
            .db_client
            .update_node_status(&node_id, NodeStatus::Inactive)
            .await;
    }

    context.node_status_locked.remove(&node_id).await;

    Ok(res?)
}

// Upgrade a node instance with given id
#[server(UpgradeNodeInstance, "/api", "Url", "/upgrade_node")]
pub async fn upgrade_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Upgrading node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();

    helper_upgrade_node_instance(
        &node_id,
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
    node_id: &NodeId,
    node_status_locked: &ImmutableNodeStatus,
    db_client: &DbClient,
    docker_client: &DockerClient,
) -> Result<(), DockerClientError> {
    // TODO: use docker 'extract' api to simply copy the new node binary into the container.
    node_status_locked
        .insert(
            node_id.clone(),
            Duration::from_secs(UPGRADE_NODE_BIN_TIMEOUT_SECS),
        )
        .await;
    db_client
        .update_node_status(node_id, NodeStatus::Upgrading)
        .await;

    let res = docker_client.upgrade_node_in_container(node_id, true).await;

    if let Ok((ref new_version, ref ips)) = res {
        logging::log!(
            "Node binary upgraded to v{} in node {node_id}.",
            new_version.as_deref().unwrap_or("[unknown]")
        );

        // set bin_version to new version obtained
        db_client
            .update_node_metadata_fields(
                node_id,
                &[
                    ("status_changed", &Utc::now().timestamp().to_string()),
                    ("bin_version", new_version.as_deref().unwrap_or_default()),
                    ("ips", ips.as_deref().unwrap_or_default()),
                ],
            )
            .await;
        db_client
            .update_node_status(node_id, NodeStatus::Transitioned("Upgraded".to_string()))
            .await;
    }

    node_status_locked.remove(node_id).await;

    let _ = res?;

    Ok(())
}

// Recycle a node instance by restarting it with a new node peer-id
#[server(RecycleNodeInstance, "/api", "Url", "/recycle_node_instance")]
pub async fn recycle_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Recycling node instance with Id: {node_id} ...");
    context
        .node_status_locked
        .insert(node_id.clone(), Duration::from_secs(20))
        .await;
    context
        .db_client
        .update_node_status(&node_id, NodeStatus::Recycling)
        .await;

    let (version, peer_id, ips) = context
        .docker_client
        .regenerate_peer_id_in_container(&node_id, true)
        .await?;

    context
        .db_client
        .update_node_metadata_fields(
            &node_id,
            &[
                ("status_changed", &Utc::now().timestamp().to_string()),
                ("bin_version", &version.unwrap_or_default()),
                ("peer_id", &peer_id.unwrap_or_default()),
                ("ips", &ips.unwrap_or_default()),
            ],
        )
        .await;

    context.node_status_locked.remove(&node_id).await;

    Ok(())
}
