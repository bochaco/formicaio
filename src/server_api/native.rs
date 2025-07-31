use crate::types::{NodeFilter, NodeId, NodesInstancesInfo};

use leptos::prelude::*;

#[cfg(feature = "ssr")]
mod ssr_imports_and_defs {
    pub use crate::{
        app::ServerGlobalState,
        bg_tasks::{BgTasksCmds, ImmutableNodeStatus},
        db_client::DbClient,
        node_manager::NodeManager,
        types::{InactiveReason, NodeInstanceInfo, NodeOpts, NodeStatus},
    };
    pub use alloy_primitives::Address;
    pub use chrono::{DateTime, Utc};
    pub use futures_util::StreamExt;
    pub use leptos::logging;
    pub use std::time::Duration;

    // Number of seconds before timing out an attempt to upgrade the node binary.
    pub const UPGRADE_NODE_BIN_TIMEOUT_SECS: u64 = 8 * 60; // 8 mins
}

#[cfg(feature = "ssr")]
use ssr_imports_and_defs::*;

/// Obtain the list of existing nodes instances with their info.
#[server(name = ListNodeInstances, prefix = "/api", endpoint = "/nodes/list")]
pub async fn nodes_instances(
    filter: Option<NodeFilter>,
) -> Result<NodesInstancesInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();

    let latest_bin_version = context.latest_bin_version.read().await.clone();
    let stats = context.stats.read().await.clone();

    let mut nodes = context.db_client.get_nodes_list().await;
    // TODO: pass the filter/s to the db-client
    if let Some(filter) = filter {
        nodes.retain(|_, info| filter.passes(info));
    }

    for (_, node_info) in nodes.iter_mut() {
        helper_gen_status_info(node_info);
        if node_info.status.is_active() {
            // let's get up to date metrics info
            // which was retrieved through the metrics server
            context
                .nodes_metrics
                .write()
                .await
                .update_node_info(node_info);
        }
    }

    let scheduled_batches = context.node_action_batches.read().await.1.clone();

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
        let changed =
            DateTime::<Utc>::from_timestamp(node_info.status_changed as i64, 0).unwrap_or_default();
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
        } else if status.is_inactive() {
            format!("{elapsed_str} ago")
        } else {
            format!("Since {elapsed_str} ago")
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
    let node_id = NodeId::random();
    logging::log!(
        "Creating new node with IP '{}', port {}, and ID {node_id} ...",
        node_opts.node_ip,
        node_opts.port
    );
    let _ = node_opts.rewards_addr.parse::<Address>()?;

    let node_info = NodeInstanceInfo {
        node_id: node_id.clone(),
        created: Utc::now().timestamp() as u64,
        status: NodeStatus::Inactive(InactiveReason::Created),
        status_changed: Utc::now().timestamp() as u64,
        node_ip: Some(node_opts.node_ip),
        port: Some(node_opts.port),
        metrics_port: Some(node_opts.metrics_port),
        rewards_addr: Some(node_opts.rewards_addr),
        upnp: node_opts.upnp,
        node_logs: node_opts.node_logs,
        data_dir_path: Some(node_opts.data_dir_path.clone()),
        ..Default::default()
    };

    if let Err(err) = context.node_manager.new_node(&node_info).await {
        logging::error!("[ERROR] Failed to create new node: {err:?}");
        return Err(err.into());
    }

    context.db_client.insert_node_metadata(&node_info).await;
    logging::log!("New node created successfully with ID: {node_id}");

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
    context
        .db_client
        .get_node_metadata(&mut node_info, true)
        .await;
    if node_info.status.is_active() {
        // kill node's process
        context.node_manager.kill_node(&node_info.node_id).await;
    }

    // remove node's metadata and directory
    context
        .db_client
        .delete_node_metadata(&node_info.node_id)
        .await;
    context.node_manager.remove_node_dir(&node_info).await;

    context
        .nodes_metrics
        .write()
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
    let mut node_info = context
        .db_client
        .check_node_is_not_batched(&node_id)
        .await?;
    if node_info.status.is_active() {
        return Ok(());
    }

    logging::log!("Starting node with ID: {node_id} ...");
    context
        .node_status_locked
        .lock(node_id.clone(), Duration::from_secs(20))
        .await;

    node_info.status = NodeStatus::Restarting;
    context
        .db_client
        .update_node_status(&node_id, &node_info.status)
        .await;
    let res = context.node_manager.spawn_new_node(&mut node_info).await;

    node_info.status = match &res {
        Ok(pid) => {
            context.db_client.update_node_pid(&node_id, *pid).await;
            NodeStatus::Active
        }
        Err(err) => NodeStatus::Inactive(InactiveReason::StartFailed(err.to_string())),
    };

    node_info.set_status_changed_now();
    context
        .db_client
        .update_node_metadata(&node_info, true)
        .await;
    context.node_status_locked.remove(&node_id).await;

    res?;
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

    context.node_manager.kill_node(&node_id).await;

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

    context.node_status_locked.remove(&node_id).await;

    Ok(())
}

/// Upgrade a node instance with given id
#[server(name = UpgradeNodeInstance, prefix = "/api", endpoint = "/nodes/upgrade")]
pub async fn upgrade_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Upgrading node with ID: {node_id} ...");
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
    let mut node_info = db_client.check_node_is_not_batched(node_id).await?;

    node_status_locked
        .lock(
            node_id.clone(),
            Duration::from_secs(UPGRADE_NODE_BIN_TIMEOUT_SECS),
        )
        .await;

    node_info.status = NodeStatus::Upgrading;
    db_client
        .update_node_status(node_id, &node_info.status)
        .await;

    let res = node_manager.upgrade_node(&mut node_info).await;

    node_info.status = match &res {
        Ok(pid) => {
            logging::log!(
                "Node binary upgraded to v{} in node {node_id}, new PID: {pid}.",
                node_info.bin_version.as_deref().unwrap_or("[unknown]")
            );
            db_client.update_node_pid(node_id, *pid).await;
            NodeStatus::Active
        }
        Err(err) => NodeStatus::Inactive(InactiveReason::StartFailed(err.to_string())),
    };

    node_info.set_status_changed_now();
    db_client.update_node_metadata(&node_info, true).await;
    node_status_locked.remove(node_id).await;

    res?;
    Ok(())
}

// Helper to recycle a node instance by restarting it with a new node peer-id
#[cfg(feature = "ssr")]
pub(crate) async fn helper_recycle_node_instance(
    node_id: NodeId,
    context: &ServerGlobalState,
) -> Result<(), ServerFnError> {
    let mut node_info = context
        .db_client
        .check_node_is_not_batched(&node_id)
        .await?;

    context
        .node_status_locked
        .lock(node_id.clone(), Duration::from_secs(20))
        .await;

    node_info.status = NodeStatus::Recycling;
    context
        .db_client
        .update_node_status(&node_id, &node_info.status)
        .await;

    let res = context
        .node_manager
        .regenerate_peer_id(&mut node_info)
        .await;

    node_info.status = match &res {
        Ok(pid) => {
            context.db_client.update_node_pid(&node_id, *pid).await;
            NodeStatus::Active
        }
        Err(err) => NodeStatus::Inactive(InactiveReason::StartFailed(err.to_string())),
    };

    node_info.set_status_changed_now();
    context
        .db_client
        .update_node_metadata(&node_info, true)
        .await;
    context.node_status_locked.remove(&node_id).await;

    res?;
    Ok(())
}
