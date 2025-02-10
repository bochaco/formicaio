use super::{
    node_instance::{NodeId, NodeInstanceInfo},
    server_api_types::NodeOpts,
};

use self::server_fn::codec::{ByteStream, Streaming};
use leptos::prelude::*;
use std::collections::HashMap;

#[cfg(feature = "ssr")]
mod ssr_imports_and_defs {
    pub use crate::{
        app::{BgTasksCmds, ServerGlobalState},
        node_instance::{NodeInstancesBatch, NodeStatus},
    };
    pub use futures_util::StreamExt;
    pub use leptos::logging;
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
#[server(PrepareNodeInstancesBatch, "/api", "Url", "/batch/prepare")]
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
#[server(CancelNodeInstancesBatch, "/api", "Url", "/batch/cancel")]
pub async fn cancel_node_instances_batch() -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Cancelling all node instances creation batches ...");

    let mut guard = context.node_instaces_batches.lock().await;
    guard.0.send(())?;
    guard.1.clear();

    Ok(())
}
