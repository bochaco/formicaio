use super::{
    node_instance::{NodeId, NodeInstanceInfo},
    server_api_types::{BatchType, NodeOpts},
};

use self::server_fn::codec::{ByteStream, Streaming};
use leptos::prelude::*;
use std::collections::HashMap;

#[cfg(feature = "ssr")]
mod ssr_imports_and_defs {
    pub use crate::{
        app::{BgTasksCmds, ServerGlobalState},
        node_instance::NodeStatus,
        server_api_types::NodesActionsBatch,
    };
    pub use futures_util::StreamExt;
    pub use leptos::logging;
    pub use rand::Rng;
    pub use std::time::Duration;
    pub use tokio::{select, time::sleep};
}

#[cfg(feature = "ssr")]
use ssr_imports_and_defs::*;

#[cfg(not(feature = "native"))]
pub use super::server_api_docker::*;
#[cfg(feature = "native")]
pub use super::server_api_native::*;

// Create and add a new node instance returning its info
#[server(CreateNodeInstance, "/api", "Url", "/nodes/create")]
pub async fn create_node_instance(node_opts: NodeOpts) -> Result<NodeInstanceInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    helper_create_node_instance(node_opts, &context).await
}

// Start a node instance with given id
#[server(StartNodeInstance, "/api", "Url", "/nodes/start")]
pub async fn start_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    helper_start_node_instance(node_id, &context).await
}

// Stop a node instance with given id
#[server(StopNodeInstance, "/api", "Url", "/nodes/stop")]
pub async fn stop_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Stopping node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    helper_stop_node_instance(node_id, &context, NodeStatus::Stopping).await
}

// Start streaming logs from a node instance with given id
#[server(output = Streaming, name = StartNodeLogsStream, prefix = "/api", endpoint = "/nodes/logs_stream")]
pub async fn start_node_logs_stream(node_id: NodeId) -> Result<ByteStream, ServerFnError> {
    logging::log!("Starting logs stream from node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();

    #[cfg(not(feature = "native"))]
    let node_logs_stream = context
        .docker_client
        .get_container_logs_stream(&node_id)
        .await?;

    #[cfg(feature = "native")]
    let node_logs_stream = context.node_manager.get_node_logs_stream(&node_id).await?;

    let converted_stream = node_logs_stream.map(|item| {
        item.map_err(ServerFnError::from) // convert the error type
    });
    Ok(ByteStream::new(converted_stream))
}

// Retrieve the metrics for a node instance with given id and filters
#[server(NodeMetrics, "/api", "Url", "/nodes/metrics")]
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
#[server(GetSettings, "/api", "Url", "/settings/get")]
pub async fn get_settings() -> Result<super::server_api_types::AppSettings, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let settings = context.db_client.get_settings().await;

    Ok(settings)
}

// Update the settings
#[server(UpdateSettings, "/api", "Url", "/settings/set")]
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

// Prepare a batch of node instances creation
#[server(PrepareNodesActionsBatch, "/api", "Url", "/batch/new")]
pub async fn node_action_batch(
    batch_type: BatchType,
    interval_secs: u64,
) -> Result<u16, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let batch_id = rand::thread_rng().gen_range(0..=u16::MAX);
    let batch_info = NodesActionsBatch::new(batch_id, batch_type, interval_secs);
    logging::log!("Creating new batch with id {batch_id}: {batch_info:?}");

    let len = {
        let batches = &mut context.node_action_batches.lock().await.1;
        batches.push(batch_info);
        batches.len()
    };
    if len == 1 {
        tokio::spawn(run_batches(context));
    }

    Ok(batch_id)
}

#[cfg(feature = "ssr")]
async fn run_batches(context: ServerGlobalState) {
    let mut cancel_rx = context.node_action_batches.lock().await.0.subscribe();

    loop {
        let batch_info =
            if let Some(next_batch) = context.node_action_batches.lock().await.1.first_mut() {
                let mut batch = next_batch.clone();
                batch.status = "in progress".to_string();
                *next_batch = batch.clone();
                batch
            } else {
                return;
            };

        match batch_info.batch_type {
            BatchType::Create {
                ref node_opts,
                count,
            } => {
                logging::log!("Started node instances creation batch of {count} nodes ...");
                let mut i = 0;
                loop {
                    select! {
                        id = cancel_rx.recv() => match id {
                            Ok(id) if id == batch_info.id => break,
                            _ => {}
                        },
                        _ = sleep(Duration::from_secs(batch_info.interval_secs)) => {
                            let mut node_opts_clone = node_opts.clone();
                            node_opts_clone.port += i;
                            node_opts_clone.metrics_port += i;
                            i += 1;
                            if let Err(err) = helper_create_node_instance(node_opts_clone, &context).await {
                                logging::log!(
                                    "Failed to create node instance {i}/{count} as part of a batch: {err}"
                                );
                            } else if let Some(ref mut b) = context.node_action_batches.lock().await.1
                                        .iter_mut()
                                        .find(|batch| batch.id == batch_info.id) {
                                b.complete += 1;
                            }

                            if i == count {
                                break;
                            }
                        }
                    }
                }
            }
            other => logging::error!("Batch of type {other:?} not yet supported. Discarding it."),
        }

        context
            .node_action_batches
            .lock()
            .await
            .1
            .retain(|batch| batch.id != batch_info.id);
    }
}

// Cancel all node instances creation batches
#[server(CancelNodesActionsBatch, "/api", "Url", "/batch/cancel")]
pub async fn cancel_batch(batch_id: u16) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Cancelling node action batch {batch_id} ...");

    let mut guard = context.node_action_batches.lock().await;
    guard.0.send(batch_id)?;
    guard.1.retain(|batch| batch.id != batch_id);

    Ok(())
}
