use crate::types::{NodeFilter, NodeId, NodesInstancesInfo};

use leptos::prelude::*;
#[cfg(feature = "ssr")]
use std::collections::HashMap;

#[cfg(feature = "ssr")]
mod ssr_imports_and_defs {
    pub use crate::{
        app::{BgTasksCmds, ImmutableNodeStatus, ServerGlobalState},
        db_client::DbClient,
        docker_client::{DockerClient, UPGRADE_NODE_BIN_TIMEOUT_SECS},
        types::{InactiveReason, NodeInstanceInfo, NodeOpts, NodeStatus},
    };
    pub use chrono::Utc;
    pub use futures_util::StreamExt;
    pub use leptos::logging;
    pub use std::time::Duration;
}

#[cfg(feature = "ssr")]
use ssr_imports_and_defs::*;

/// Obtain the list of existing nodes instances with their info
#[server(name = ListNodeInstances, prefix = "/api", endpoint = "/nodes/list")]
pub async fn nodes_instances(
    filter: Option<NodeFilter>,
) -> Result<NodesInstancesInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let latest_bin_version = context.latest_bin_version.lock().await.clone();
    let nodes_list = context.docker_client.get_containers_list().await?;
    let stats = context.stats.lock().await.clone();

    let mut nodes = HashMap::new();
    for mut node_info in nodes_list.into_iter() {
        // we first read node metadata cached in the database
        // TODO: fetch metadata of all nodes from DB with a single DB call
        context
            .db_client
            .get_node_metadata(&mut node_info, false)
            .await;

        // TODO: pass the filter/s to docker-client
        if let Some(ref filter) = filter {
            if !filter.passes(&node_info) {
                continue;
            }
        }

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

    let scheduled_batches = context.node_action_batches.lock().await.1.clone();

    Ok(NodesInstancesInfo {
        latest_bin_version: latest_bin_version.map(|v| v.to_string()),
        nodes,
        stats,
        scheduled_batches,
    })
}

// Helper to create a node instance
#[cfg(feature = "ssr")]
pub(crate) async fn helper_create_node_instance(
    node_opts: NodeOpts,
    context: &ServerGlobalState,
) -> Result<NodeInstanceInfo, ServerFnError> {
    logging::log!("Creating new node with port {} ...", node_opts.port);
    let auto_start = node_opts.auto_start;
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

// Helper to delete a node instance with given id
#[cfg(feature = "ssr")]
pub(crate) async fn helper_delete_node_instance(
    node_id: NodeId,
    context: &ServerGlobalState,
) -> Result<(), ServerFnError> {
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

// Helper to start a node instance with given id
#[cfg(feature = "ssr")]
pub(crate) async fn helper_start_node_instance(
    node_id: NodeId,
    context: &ServerGlobalState,
) -> Result<(), ServerFnError> {
    let _ = context
        .db_client
        .check_node_is_not_batched(&node_id)
        .await?;

    logging::log!("Starting node with Id: {node_id} ...");

    context
        .db_client
        .update_node_status(&node_id, &NodeStatus::Restarting)
        .await;

    let (bin_version, peer_id, ips) = context
        .docker_client
        .start_container(&node_id, true)
        .await?;

    let node_info = NodeInstanceInfo {
        node_id,
        status_changed: Utc::now().timestamp() as u64,
        bin_version: Some(bin_version.clone().unwrap_or_default()),
        peer_id: Some(peer_id.unwrap_or_default()),
        ips: Some(ips.unwrap_or_default()),
        ..Default::default()
    };

    context
        .db_client
        .update_node_metadata(&node_info, false)
        .await;

    Ok(())
}

// Helper to stop a node instance with given id
#[cfg(feature = "ssr")]
pub(crate) async fn helper_stop_node_instance(
    node_id: NodeId,
    context: &ServerGlobalState,
    transient_status: NodeStatus,
) -> Result<(), ServerFnError> {
    let _ = context
        .db_client
        .check_node_is_not_batched(&node_id)
        .await?;

    context
        .node_status_locked
        .lock(node_id.clone(), Duration::from_secs(20))
        .await;
    context
        .db_client
        .update_node_status(&node_id, &transient_status)
        .await;

    let res = context.docker_client.stop_container(&node_id).await;

    if matches!(res, Ok(())) {
        // set connected/kbucket peers back to 0 and update cache
        let node_info = NodeInstanceInfo {
            node_id: node_id.clone(),
            status: NodeStatus::Inactive(InactiveReason::Stopped),
            status_changed: Utc::now().timestamp() as u64,
            connected_peers: Some(0),
            kbuckets_peers: Some(0),
            records: Some(0),
            ips: Some("".to_string()),
            ..Default::default()
        };

        context
            .db_client
            .update_node_metadata(&node_info, true)
            .await;
    }

    context.node_status_locked.remove(&node_id).await;

    Ok(res?)
}

/// Upgrade a node instance with given id
#[server(name = UpgradeNodeInstance, prefix = "/api", endpoint = "/nodes/upgrade")]
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

// Helper to upgrade a node instance with given id
#[cfg(feature = "ssr")]
pub(crate) async fn helper_upgrade_node_instance(
    node_id: &NodeId,
    node_status_locked: &ImmutableNodeStatus,
    db_client: &DbClient,
    docker_client: &DockerClient,
) -> Result<(), ServerFnError> {
    let _ = db_client.check_node_is_not_batched(node_id).await?;

    // TODO: use docker 'extract' api to simply copy the new node binary into the container.
    node_status_locked
        .lock(
            node_id.clone(),
            Duration::from_secs(UPGRADE_NODE_BIN_TIMEOUT_SECS),
        )
        .await;
    db_client
        .update_node_status(node_id, &NodeStatus::Upgrading)
        .await;

    let res = docker_client.upgrade_node_in_container(node_id, true).await;

    if let Ok((ref new_version, ref ips)) = res {
        logging::log!(
            "Node binary upgraded to v{} in node {node_id}.",
            new_version.as_deref().unwrap_or("[unknown]")
        );

        // set bin_version to new version obtained
        let node_info = NodeInstanceInfo {
            node_id: node_id.clone(),
            status: NodeStatus::Upgrading,
            status_changed: Utc::now().timestamp() as u64,
            bin_version: Some(new_version.clone().unwrap_or_default()),
            ips: Some(ips.clone().unwrap_or_default()),
            ..Default::default()
        };

        db_client.update_node_metadata(&node_info, true).await;
    }

    node_status_locked.remove(node_id).await;

    let _ = res?;

    Ok(())
}

// Helper to recycle a node instance by restarting it with a new node peer-id
#[cfg(feature = "ssr")]
pub(crate) async fn helper_recycle_node_instance(
    node_id: NodeId,
    context: &ServerGlobalState,
) -> Result<(), ServerFnError> {
    let _ = context
        .db_client
        .check_node_is_not_batched(&node_id)
        .await?;

    context
        .node_status_locked
        .lock(node_id.clone(), Duration::from_secs(20))
        .await;
    context
        .db_client
        .update_node_status(&node_id, &NodeStatus::Recycling)
        .await;

    let (bin_version, peer_id, ips) = context
        .docker_client
        .regenerate_peer_id_in_container(&node_id, true)
        .await?;

    let node_info = NodeInstanceInfo {
        node_id: node_id.clone(),
        status_changed: Utc::now().timestamp() as u64,
        bin_version: Some(bin_version.clone().unwrap_or_default()),
        peer_id: Some(peer_id.unwrap_or_default()),
        ips: Some(ips.unwrap_or_default()),
        ..Default::default()
    };

    context
        .db_client
        .update_node_metadata(&node_info, false)
        .await;

    context.node_status_locked.remove(&node_id).await;

    Ok(())
}
