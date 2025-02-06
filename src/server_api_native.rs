use super::{
    node_instance::{NodeId, NodeInstanceInfo},
    server_api_types::{NodeOpts, NodesInstancesInfo},
};

use self::server_fn::codec::{ByteStream, Streaming};
use leptos::prelude::*;
use std::collections::HashMap;

#[cfg(feature = "ssr")]
mod ssr_imports_and_defs {
    pub use crate::{
        app::{BgTasksCmds, ImmutableNodeStatus, ServerGlobalState},
        db_client::DbClient,
        node_instance::{NodeInstancesBatch, NodeStatus},
        node_manager::{NodeManager, NodeManagerError},
        server_api_types::BatchInProgress,
    };
    pub use alloy_primitives::Address;
    pub use chrono::{DateTime, Utc};
    pub use futures_util::StreamExt;
    pub use leptos::logging;
    pub use rand::distributions::{Alphanumeric, DistString};
    pub use std::time::Duration;
    pub use tokio::{select, time::sleep};

    // Length of generated node ids
    pub const NODE_ID_LENGTH: usize = 12;

    // Number of seconds before timing out an attempt to upgrade the node binary.
    pub const UPGRADE_NODE_BIN_TIMEOUT_SECS: u64 = 8 * 60; // 8 mins
}

#[cfg(feature = "ssr")]
use ssr_imports_and_defs::*;

// Obtain the list of existing nodes instances with their info
#[server(ListNodeInstances, "/api", "Url", "/list_nodes")]
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

// Create and add a new node instance returning its info
#[server(CreateNodeInstance, "/api", "Url", "/create_node")]
pub async fn create_node_instance(node_opts: NodeOpts) -> Result<NodeInstanceInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    helper_create_node_instance(node_opts, &context).await
}

/// Helper to create a node instance
#[cfg(feature = "ssr")]
async fn helper_create_node_instance(
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

// Delete a node instance with given id
#[server(DeleteNodeInstance, "/api", "Url", "/delete_node")]
pub async fn delete_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Deleting node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    let mut node_info = NodeInstanceInfo::new(node_id);
    context.db_client.get_node_metadata(&mut node_info).await;
    context
        .db_client
        .delete_node_metadata(&node_info.node_id)
        .await;

    if node_info.status.is_active() {
        // kill node's process
        context.node_manager.kill_node(&node_info.node_id).await?;
    }
    // remove node's directory
    context
        .node_manager
        .remove_node_dir(&node_info.node_id)
        .await?;

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
        .node_status_locked
        .insert(node_id.clone(), Duration::from_secs(20))
        .await;
    context
        .db_client
        .update_node_status(&node_id, NodeStatus::Restarting)
        .await;

    let mut node_info = NodeInstanceInfo::new(node_id.clone());
    context.db_client.get_node_metadata(&mut node_info).await;
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

    let res = context.node_manager.kill_node(&node_id).await;

    if matches!(res, Ok(())) {
        // set connected/kbucket peers back to 0 and update cache
        context
            .db_client
            .update_node_metadata_fields(
                &node_id,
                &[
                    ("status_changed", &Utc::now().timestamp().to_string()),
                    ("pid", "0"),
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
        &context.node_manager,
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
    node_manager: &NodeManager,
) -> Result<(), NodeManagerError> {
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

    res
}

// Start streaming logs from a node instance with given id
#[server(output = Streaming, name = StartNodeLogsStream, prefix = "/api", endpoint = "/node_logs_stream")]
pub async fn start_node_logs_stream(node_id: NodeId) -> Result<ByteStream, ServerFnError> {
    logging::log!("Starting logs stream from node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();

    let node_logs_stream = context.node_manager.get_node_logs_stream(&node_id).await?;
    let converted_stream = node_logs_stream.map(|item| {
        item.map_err(ServerFnError::from) // convert the error type
    });
    Ok(ByteStream::new(converted_stream))
}

// Retrieve the metrics for a node instance with given id and filters
#[server(NodeMetrics, "/api", "Url", "/node_metrics")]
pub async fn node_metrics(
    node_id: NodeId,
    since: Option<i64>,
) -> Result<HashMap<String, Vec<super::app::NodeMetric>>, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let metrics = context
        .nodes_metrics
        .lock()
        .await
        .get_node_metrics(node_id, since)
        .await;

    Ok(metrics)
}

// Retrieve the settings
#[server(GetSettings, "/api", "Url", "/get_settings")]
pub async fn get_settings() -> Result<super::server_api_types::AppSettings, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let settings = context.db_client.get_settings().await;

    Ok(settings)
}

// Update the settings
#[server(UpdateSettings, "/api", "Url", "/update_settings")]
pub async fn update_settings(
    settings: super::server_api_types::AppSettings,
) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    context.db_client.update_settings(&settings).await?;
    context
        .bg_tasks_cmds_tx
        .send(BgTasksCmds::ApplySettings(settings))?;
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

// Prepare a batch of node instances creation
#[server(
    PrepareNodeInstancesBatch,
    "/api",
    "Url",
    "/prepare_node_instances_batch"
)]
pub async fn prepare_node_instances_batch(
    node_opts: NodeOpts,
    count: u16,
    interval_secs: u64,
) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!(
        "Creating new batch of {count} nodes with port range starting at {} ...",
        node_opts.port
    );

    let batch_info = NodeInstancesBatch {
        node_opts,
        created: 0,
        total: count,
        interval_secs,
    };
    logging::log!("New batch created: {batch_info:?}");
    let len = {
        let batches = &mut context.node_instaces_batches.lock().await.1;
        batches.push(batch_info);
        batches.len()
    };
    if len == 1 {
        tokio::spawn(run_batches(context));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
async fn run_batches(context: ServerGlobalState) {
    let mut cancel_rx = context.node_instaces_batches.lock().await.0.subscribe();

    loop {
        let next_batch = context
            .node_instaces_batches
            .lock()
            .await
            .1
            .first()
            .cloned();

        if let Some(batch_info) = next_batch {
            let total = batch_info.total;
            logging::log!("Started node instances creation batch of {total} nodes ...");
            for i in 0..total {
                select! {
                    _ = cancel_rx.recv() => return,
                    _ = sleep(Duration::from_secs(batch_info.interval_secs)) => {
                        let mut node_opts = batch_info.node_opts.clone();
                        node_opts.port += i;
                        node_opts.metrics_port += i;
                        if let Err(err) = helper_create_node_instance(node_opts, &context).await {
                            logging::log!(
                                "Failed to create node instance {i}/{total} as part of a batch: {err}"
                            );
                        }

                        if let Some(b) = context.node_instaces_batches.lock().await.1.get_mut(0) {
                            b.created += 1;
                        }
                    }
                }
            }

            let _ = context.node_instaces_batches.lock().await.1.remove(0);
        } else {
            return;
        }
    }
}

// Cancel all node instances creation batches
#[server(
    CancelNodeInstancesBatch,
    "/api",
    "Url",
    "/cancel_node_instances_batch"
)]
pub async fn cancel_node_instances_batch() -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Cancelling all node instances creation batches ...");

    let mut guard = context.node_instaces_batches.lock().await;
    guard.0.send(())?;
    guard.1.clear();

    Ok(())
}
