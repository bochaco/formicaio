use super::{
    node_instance::{ContainerId, NodeInstanceInfo},
    server_api_types::NodesInstancesInfo,
};

use self::server_fn::codec::{ByteStream, Streaming};
use leptos::prelude::*;
use std::collections::HashMap;

#[cfg(feature = "ssr")]
use super::{
    app::{BgTasksCmds, ImmutableNodeStatus, ServerGlobalState},
    db_client::DbClient,
    docker_client::{DockerClient, DockerClientError, UPGRADE_NODE_BIN_TIMEOUT_SECS},
    node_instance::{NodeInstancesBatch, NodeStatus},
    server_api_types::BatchInProgress,
};
#[cfg(feature = "ssr")]
use futures_util::StreamExt;
#[cfg(feature = "ssr")]
use leptos::logging;
#[cfg(feature = "ssr")]
use std::time::Duration;
#[cfg(feature = "ssr")]
use tokio::{select, time::sleep};

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

    let batches = &context.node_instaces_batches.lock().await.1;
    let batch_in_progress = if let Some(b) = batches.first() {
        let init = BatchInProgress {
            auto_start: b.auto_start,
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
        latest_bin_version,
        nodes,
        stats,
        batch_in_progress,
    })
}

// Create and add a new node instance returning its info
#[server(CreateNodeInstance, "/api", "Url", "/create_node")]
pub async fn create_node_instance(
    port: u16,
    metrics_port: u16,
    rewards_addr: String,
    home_network: bool,
    auto_start: bool,
) -> Result<NodeInstanceInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    helper_create_node_instance(
        port,
        metrics_port,
        rewards_addr,
        home_network,
        auto_start,
        &context,
    )
    .await
}

/// Helper to create a node instance
#[cfg(feature = "ssr")]
async fn helper_create_node_instance(
    port: u16,
    metrics_port: u16,
    rewards_addr: String,
    home_network: bool,
    auto_start: bool,
    context: &ServerGlobalState,
) -> Result<NodeInstanceInfo, ServerFnError> {
    logging::log!("Creating new node container with port {port} ...");
    let container_id = context
        .docker_client
        .create_new_container(port, metrics_port, rewards_addr, home_network)
        .await?;
    logging::log!("New node container Id: {container_id} ...");

    let mut node_info = context
        .docker_client
        .get_container_info(&container_id)
        .await?;
    logging::log!("New node container created: {node_info:?}");

    context.db_client.insert_node_metadata(&node_info).await;

    if auto_start {
        helper_start_node_instance(container_id.clone(), context).await?;
        node_info = context
            .docker_client
            .get_container_info(&container_id)
            .await?;
    }

    context
        .bg_tasks_cmds_tx
        .send(BgTasksCmds::CheckBalanceFor(node_info.clone()))?;

    Ok(node_info)
}

// Delete a node instance with given id
#[server(DeleteNodeInstance, "/api", "Url", "/delete_node")]
pub async fn delete_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    logging::log!("Deleting node container with Id: {container_id} ...");
    let context = expect_context::<ServerGlobalState>();
    let node_info = context
        .docker_client
        .get_container_info(&container_id)
        .await?;
    context
        .docker_client
        .delete_container(&container_id)
        .await?;
    context.db_client.delete_node_metadata(&container_id).await;
    context
        .nodes_metrics
        .lock()
        .await
        .remove_container_metrics(&container_id)
        .await;

    context
        .bg_tasks_cmds_tx
        .send(BgTasksCmds::DeleteBalanceFor(node_info))?;

    Ok(())
}

// Start a node instance with given id
#[server(StartNodeInstance, "/api", "Url", "/start_node")]
pub async fn start_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    helper_start_node_instance(container_id, &context).await
}

// Helper to start a node instance with given id
#[cfg(feature = "ssr")]
async fn helper_start_node_instance(
    container_id: ContainerId,
    context: &ServerGlobalState,
) -> Result<(), ServerFnError> {
    logging::log!("Starting node container with Id: {container_id} ...");

    context
        .db_client
        .update_node_status(&container_id, NodeStatus::Restarting)
        .await;

    let (version, peer_id, ips) = context
        .docker_client
        .start_container(&container_id, true)
        .await?;
    context
        .db_client
        .update_node_metadata_fields(
            &container_id,
            &[
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
pub async fn stop_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    logging::log!("Stopping node container with Id: {container_id} ...");
    let context = expect_context::<ServerGlobalState>();
    helper_stop_node_instance(container_id, &context, NodeStatus::Stopping).await
}

// Helper to stop a node instance with given id
#[cfg(feature = "ssr")]
async fn helper_stop_node_instance(
    container_id: ContainerId,
    context: &ServerGlobalState,
    status: NodeStatus,
) -> Result<(), ServerFnError> {
    context
        .node_status_locked
        .insert(container_id.clone(), Duration::from_secs(20))
        .await;
    context
        .db_client
        .update_node_status(&container_id, status)
        .await;

    let res = context.docker_client.stop_container(&container_id).await;

    if matches!(res, Ok(())) {
        // set connected/kbucket peers back to 0 and update cache
        context
            .db_client
            .update_node_metadata_fields(
                &container_id,
                &[
                    ("connected_peers", "0"),
                    ("kbuckets_peers", "0"),
                    ("records", ""),
                    ("ips", ""),
                ],
            )
            .await;
        context
            .db_client
            .update_node_status(&container_id, NodeStatus::Inactive)
            .await;
    }

    context.node_status_locked.remove(&container_id).await;

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
    node_status_locked: &ImmutableNodeStatus,
    db_client: &DbClient,
    docker_client: &DockerClient,
) -> Result<(Option<String>, Option<String>), DockerClientError> {
    // TODO: use docker 'extract' api to simply copy the new node binary into the container.
    node_status_locked
        .insert(
            container_id.clone(),
            Duration::from_secs(UPGRADE_NODE_BIN_TIMEOUT_SECS),
        )
        .await;
    db_client
        .update_node_status(container_id, NodeStatus::Upgrading)
        .await;

    let res = docker_client
        .upgrade_node_in_container(container_id, true)
        .await;

    if let Ok((ref new_version, ref ips)) = res {
        logging::log!(
            "Node binary upgraded to v{} in container {container_id}.",
            new_version.as_deref().unwrap_or("[unknown]")
        );

        // set bin_version to new version obtained
        db_client
            .update_node_metadata_fields(
                container_id,
                &[
                    ("bin_version", new_version.as_deref().unwrap_or_default()),
                    ("ips", ips.as_deref().unwrap_or_default()),
                ],
            )
            .await;
        db_client
            .update_node_status(
                container_id,
                NodeStatus::Transitioned("Upgraded".to_string()),
            )
            .await;
    }

    node_status_locked.remove(container_id).await;

    res
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
pub async fn recycle_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Recycling node instance with Id: {container_id} ...");
    context
        .node_status_locked
        .insert(container_id.clone(), Duration::from_secs(20))
        .await;
    context
        .db_client
        .update_node_status(&container_id, NodeStatus::Recycling)
        .await;

    let (version, peer_id, ips) = context
        .docker_client
        .regenerate_peer_id_in_container(&container_id, true)
        .await?;

    context
        .db_client
        .update_node_metadata_fields(
            &container_id,
            &[
                ("bin_version", &version.unwrap_or_default()),
                ("peer_id", &peer_id.unwrap_or_default()),
                ("ips", &ips.unwrap_or_default()),
            ],
        )
        .await;

    context.node_status_locked.remove(&container_id).await;

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
    port_start: u16,
    metrics_port_start: u16,
    count: u16,
    rewards_addr: String,
    home_network: bool,
    auto_start: bool,
    interval_secs: u64,
) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!(
        "Creating new batch of {count} nodes with port range starting at {port_start} ..."
    );

    let batch_info = NodeInstancesBatch {
        port_start,
        metrics_port_start,
        created: 0,
        total: count,
        rewards_addr,
        home_network,
        auto_start,
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
                        if let Err(err) = helper_create_node_instance(
                            batch_info.port_start + i,
                            batch_info.metrics_port_start + i,
                            batch_info.rewards_addr.clone(),
                            batch_info.home_network,
                            batch_info.auto_start,
                            &context
                        )
                        .await
                        {
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
