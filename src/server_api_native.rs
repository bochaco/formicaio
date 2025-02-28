use super::{node_instance::NodeId, server_api_types::NodesInstancesInfo};

use leptos::prelude::*;

#[cfg(feature = "ssr")]
mod ssr_imports_and_defs {
    pub use crate::{
        app::{BgTasksCmds, ImmutableNodeStatus, ServerGlobalState},
        db_client::DbClient,
        node_instance::{NodeInstanceInfo, NodeStatus},
        node_manager::NodeManager,
        server_api_types::NodeOpts,
    };
    pub use alloy_primitives::Address;
    pub use chrono::{DateTime, Utc};
    pub use futures_util::StreamExt;
    pub use leptos::logging;
    pub use rand::distributions::{Alphanumeric, DistString};
    pub use std::time::Duration;

    // Length of generated node ids
    pub const NODE_ID_LENGTH: usize = 12;

    // Number of seconds before timing out an attempt to upgrade the node binary.
    pub const UPGRADE_NODE_BIN_TIMEOUT_SECS: u64 = 8 * 60; // 8 mins
}

#[cfg(feature = "ssr")]
use ssr_imports_and_defs::*;

/// Obtain the list of existing nodes instances with their info
#[server(ListNodeInstances, "/api", "Url", "/nodes/list")]
pub async fn nodes_instances() -> Result<NodesInstancesInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    *context.server_api_hit.lock().await = true;

    let latest_bin_version = context.latest_bin_version.lock().await.clone();
    let stats = context.stats.lock().await.clone();

    let mut nodes = context.db_client.get_nodes_list().await;
    for (_, node_info) in nodes.iter_mut() {
        helper_gen_status_info(node_info);
        if node_info.status.is_active() {
            // let's get up to date metrics info
            // which was retrieved through the metrics server
            context
                .nodes_metrics
                .lock()
                .await
                .update_node_info(node_info);
        }
    }

    let scheduled_batches = context.node_action_batches.lock().await.1.clone();

    Ok(NodesInstancesInfo {
        latest_bin_version: latest_bin_version.map(|v| v.to_string()),
        nodes,
        stats,
        scheduled_batches,
    })
}

// Helper to generate a string with additional info about current node's status
#[cfg(feature = "ssr")]
fn helper_gen_status_info(node_info: &mut NodeInstanceInfo) {
    let status = &node_info.status;
    let status_info = if status.is_transitioning() {
        "".to_string()
    } else {
        match node_info.status_changed {
            None => "Created".to_string(),
            Some(v) => {
                let changed = DateTime::<Utc>::from_timestamp(v as i64, 0).unwrap_or_default();
                let elapsed = Utc::now() - changed;
                let elapsed_str = if elapsed.num_weeks() > 1 {
                    format!("{} weeks", elapsed.num_weeks())
                } else if elapsed.num_days() > 1 {
                    format!("{} days", elapsed.num_days())
                } else if elapsed.num_hours() > 1 {
                    format!("{} hours", elapsed.num_hours())
                } else if elapsed.num_minutes() > 1 {
                    format!("{} minutes", elapsed.num_minutes())
                } else if elapsed.num_seconds() > 1 {
                    format!("{} seconds", elapsed.num_seconds())
                } else {
                    "about a second".to_string()
                };
                if status.is_active() {
                    format!("Up {elapsed_str}")
                } else {
                    format!("Since {elapsed_str} ago")
                }
            }
        }
    };

    node_info.status_info = status_info;
}

// Helper to create a node instance
#[cfg(feature = "ssr")]
pub(crate) async fn helper_create_node_instance(
    node_opts: NodeOpts,
    context: &ServerGlobalState,
) -> Result<NodeInstanceInfo, ServerFnError> {
    // Generate a random string as node id
    let random_str = Alphanumeric.sample_string(&mut rand::thread_rng(), NODE_ID_LENGTH / 2);
    let node_id = hex::encode(random_str);
    logging::log!(
        "Creating new node with port {} and Id {node_id} ...",
        node_opts.port
    );
    let _ = node_opts.rewards_addr.parse::<Address>()?;

    let node_info = NodeInstanceInfo {
        node_id: node_id.clone(),
        created: Utc::now().timestamp() as u64,
        status: NodeStatus::Inactive,
        port: Some(node_opts.port),
        metrics_port: Some(node_opts.metrics_port),
        rewards_addr: Some(node_opts.rewards_addr),
        home_network: node_opts.home_network,
        upnp: node_opts.upnp,
        node_logs: node_opts.node_logs,
        ..Default::default()
    };

    if let Err(err) = context.node_manager.new_node(&node_info).await {
        logging::error!("Failed to create new node's directory: {err:?}");
        return Err(err.into());
    }

    context.db_client.insert_node_metadata(&node_info).await;
    logging::log!("New node created with id: {node_id}");

    if node_opts.auto_start {
        helper_start_node_instance(node_id.clone(), context).await?;
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
    let mut node_info = NodeInstanceInfo::new(node_id);
    context.db_client.get_node_metadata(&mut node_info).await;
    if node_info.status.is_active() {
        // kill node's process
        context.node_manager.kill_node(&node_info.node_id).await;
    }

    // remove node's metadata and directory
    context
        .db_client
        .delete_node_metadata(&node_info.node_id)
        .await;
    context
        .node_manager
        .remove_node_dir(&node_info.node_id)
        .await;

    context
        .nodes_metrics
        .lock()
        .await
        .remove_node_metrics(&node_info.node_id)
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
    logging::log!("Starting node with Id: {node_id} ...");
    context
        .node_status_locked
        .insert(node_id.clone(), Duration::from_secs(20))
        .await;

    let mut node_info = NodeInstanceInfo::new(node_id.clone());
    context.db_client.get_node_metadata(&mut node_info).await;
    if node_info.status.is_active() {
        return Ok(());
    }

    context
        .db_client
        .update_node_status(&node_id, NodeStatus::Restarting)
        .await;
    context.node_manager.spawn_new_node(&mut node_info).await?;

    node_info.status = NodeStatus::Active;
    node_info.status_changed = Some(Utc::now().timestamp() as u64);
    context
        .db_client
        .update_node_metadata(&node_info, true)
        .await;
    context.node_status_locked.remove(&node_id).await;

    Ok(())
}

// Helper to stop a node instance with given id
#[cfg(feature = "ssr")]
pub(crate) async fn helper_stop_node_instance(
    node_id: NodeId,
    context: &ServerGlobalState,
    status: NodeStatus,
) -> Result<(), ServerFnError> {
    context
        .node_status_locked
        .insert(node_id.clone(), Duration::from_secs(20))
        .await;
    context.db_client.update_node_status(&node_id, status).await;

    context.node_manager.kill_node(&node_id).await;

    // set connected/kbucket peers back to 0 and update cache
    let node_info = NodeInstanceInfo {
        node_id: node_id.clone(),
        status: NodeStatus::Inactive,
        status_changed: Some(Utc::now().timestamp() as u64),
        pid: Some(0),
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

    context.node_status_locked.remove(&node_id).await;

    Ok(())
}

/// Upgrade a node instance with given id
#[server(UpgradeNodeInstance, "/api", "Url", "/nodes/upgrade")]
pub async fn upgrade_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    // TODO: check if node is not locked

    logging::log!("Upgrading node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();

    helper_upgrade_node_instance(
        &node_id,
        &context.node_status_locked,
        &context.db_client,
        &context.node_manager,
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
    node_manager: &NodeManager,
) -> Result<(), ServerFnError> {
    node_status_locked
        .insert(
            node_id.clone(),
            Duration::from_secs(UPGRADE_NODE_BIN_TIMEOUT_SECS),
        )
        .await;
    db_client
        .update_node_status(node_id, NodeStatus::Upgrading)
        .await;

    let mut node_info = NodeInstanceInfo::new(node_id.clone());
    db_client.get_node_metadata(&mut node_info).await;

    let res = node_manager.upgrade_node(&mut node_info).await;

    if res.is_ok() {
        logging::log!(
            "Node binary upgraded to v{} in node {node_id}.",
            node_info.bin_version.as_deref().unwrap_or("[unknown]")
        );

        node_info.status = NodeStatus::Transitioned("Upgraded".to_string());
        node_info.status_changed = Some(Utc::now().timestamp() as u64);

        db_client.update_node_metadata(&node_info, true).await;
    }

    node_status_locked.remove(node_id).await;

    Ok(res?)
}

// Helper to recycle a node instance by restarting it with a new node peer-id
#[cfg(feature = "ssr")]
pub(crate) async fn helper_recycle_node_instance(
    node_id: NodeId,
    context: &ServerGlobalState,
) -> Result<(), ServerFnError> {
    context
        .node_status_locked
        .insert(node_id.clone(), Duration::from_secs(20))
        .await;
    context
        .db_client
        .update_node_status(&node_id, NodeStatus::Recycling)
        .await;

    let mut node_info = NodeInstanceInfo::new(node_id.clone());
    context.db_client.get_node_metadata(&mut node_info).await;

    context
        .node_manager
        .regenerate_peer_id(&mut node_info)
        .await?;
    node_info.status = NodeStatus::Active;
    node_info.status_changed = Some(Utc::now().timestamp() as u64);
    context
        .db_client
        .update_node_metadata(&node_info, true)
        .await;

    context.node_status_locked.remove(&node_id).await;

    Ok(())
}
