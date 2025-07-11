#[cfg(not(feature = "native"))]
mod docker;
#[cfg(feature = "native")]
mod native;

use crate::types::{
    BatchOnMatch, BatchType, NodeId, NodeInstanceInfo, NodeOpts, NodesActionsBatch, Stats,
    WidgetFourStats,
};

use alloy_primitives::Address;
use leptos::prelude::*;
use leptos::server_fn::codec::{ByteStream, Streaming};
use std::{collections::HashMap, str::FromStr};

#[cfg(feature = "ssr")]
mod ssr_imports_and_defs {
    pub use crate::{
        app::{BgTasksCmds, ServerGlobalState},
        helpers::truncated_balance_str,
        types::{NodeFilter, NodeStatus, WidgetStat},
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
pub use docker::*;
#[cfg(feature = "native")]
pub use native::*;

// Expected length of entered hex-encoded rewards address.
const REWARDS_ADDR_LENGTH: usize = 40;

/// Return a set of stats
#[server(name = FetchStats, prefix = "/api", endpoint = "/stats")]
pub async fn fetch_stats() -> Result<Stats, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let stats = context.stats.lock().await.clone();
    Ok(stats)
}

/// Return a set of stats formatted for UmbrelOS widget
#[server(name = FetchStatsWidget, prefix = "/api", endpoint = "/stats_widget")]
pub async fn fetch_stats_widget() -> Result<WidgetFourStats, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let stats = context.stats.lock().await.clone();
    let widget_stats = WidgetFourStats {
        r#type: "four-stats".to_string(),
        refresh: "5s".to_string(),
        link: "".to_string(),
        items: vec![
            WidgetStat {
                title: "Total balance".to_string(),
                text: truncated_balance_str(stats.total_balance),
                subtext: "".to_string(),
            },
            WidgetStat {
                title: "Active nodes".to_string(),
                text: format!("{}/{}", stats.active_nodes, stats.total_nodes),
                subtext: "".to_string(),
            },
            WidgetStat {
                title: "Stored records".to_string(),
                text: stats.stored_records.to_string(),
                subtext: "".to_string(),
            },
            WidgetStat {
                title: "Network size".to_string(),
                text: stats.estimated_net_size.to_string(),
                subtext: "".to_string(),
            },
        ],
    };

    Ok(widget_stats)
}

// Create and add a new node instance returning its info
#[server(name = CreateNodeInstance, prefix= "/api", endpoint = "/nodes/create")]
pub async fn create_node_instance(node_opts: NodeOpts) -> Result<NodeInstanceInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();

    // validate rewards address before proceeding
    parse_and_validate_addr(&node_opts.rewards_addr).map_err(ServerFnError::new)?;

    helper_create_node_instance(node_opts, &context).await
}

// Start a node instance with given id
#[server(name = StartNodeInstance, prefix= "/api", endpoint = "/nodes/start")]
pub async fn start_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    helper_start_node_instance(node_id, &context).await?;
    Ok(())
}

// Stop a node instance with given id
#[server(name = StopNodeInstance, prefix= "/api", endpoint = "/nodes/stop")]
pub async fn stop_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Stopping node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    helper_stop_node_instance(node_id, &context, NodeStatus::Stopping).await?;
    Ok(())
}

/// Delete a node instance with given id
#[server(name = DeleteNodeInstance, prefix= "/api", endpoint = "/nodes/delete")]
pub async fn delete_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Deleting node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    helper_delete_node_instance(node_id, &context).await?;
    Ok(())
}

// Recycle a node instance by restarting it with a new node peer-id
#[server(name = RecycleNodeInstance, prefix= "/api", endpoint = "/nodes/recycle")]
pub async fn recycle_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Recycling node instance with Id: {node_id} ...");
    helper_recycle_node_instance(node_id, &context).await?;
    Ok(())
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
#[server(name = NodeMetrics, prefix = "/api", endpoint = "/nodes/metrics")]
pub async fn node_metrics(
    node_id: NodeId,
    since: Option<i64>,
) -> Result<HashMap<String, Vec<super::types::NodeMetric>>, ServerFnError> {
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
#[server(name = GetSettings, prefix = "/api", endpoint = "/settings/get")]
pub async fn get_settings() -> Result<super::types::AppSettings, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let settings = context.db_client.get_settings().await;

    Ok(settings)
}

// Update the settings
#[server(name = UpdateSettings, prefix = "/api", endpoint = "/settings/set")]
pub async fn update_settings(settings: super::types::AppSettings) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    context.db_client.update_settings(&settings).await?;
    context
        .bg_tasks_cmds_tx
        .send(BgTasksCmds::ApplySettings(settings))?;
    Ok(())
}

/// Return list of running and scheduled nodes actions batches
#[server(name = ListNodesActionsBatches, prefix = "/api", endpoint = "/batch/list")]
pub async fn nodes_actions_batches() -> Result<Vec<NodesActionsBatch>, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();

    let batches = context.node_action_batches.lock().await.1.clone();
    Ok(batches)
}

/// Prepare a new nodes actions batch
#[server(name = CreateNodesActionsBatch, prefix = "/api", endpoint = "/batch/create")]
pub async fn nodes_actions_batch_create(
    batch_type: BatchType,
    interval_secs: u64,
) -> Result<u16, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    helper_node_action_batch(batch_type, interval_secs, &context).await
}

/// Create a nodes actions batch based on matching rules
#[server(name = CreateNodesActionsBatchOnMatch, prefix = "/api", endpoint = "/batch/create_on_match")]
pub async fn nodes_actions_batch_on_match(
    batch_on_match: BatchOnMatch,
    interval_secs: u64,
) -> Result<u16, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    #[cfg(not(feature = "native"))]
    let nodes_list = context.docker_client.get_containers_list().await?;
    #[cfg(feature = "native")]
    let nodes_list = context.db_client.get_nodes_list().await.into_values();

    let matching_nodes = move |filter: NodeFilter| {
        nodes_list
            .into_iter()
            .filter(|info| filter.matches(info))
            .map(|info| info.node_id)
            .collect::<Vec<_>>()
    };

    let batch_type = match batch_on_match {
        BatchOnMatch::StartOnMatch(filter) => BatchType::Start(matching_nodes(filter)),
        BatchOnMatch::StopOnMatch(filter) => BatchType::Stop(matching_nodes(filter)),
        BatchOnMatch::UpgradeOnMatch(filter) => BatchType::Upgrade(matching_nodes(filter)),
        BatchOnMatch::RecycleOnMatch(filter) => BatchType::Recycle(matching_nodes(filter)),
        BatchOnMatch::RemoveOnMatch(filter) => BatchType::Remove(matching_nodes(filter)),
    };
    helper_node_action_batch(batch_type, interval_secs, &context).await
}

// Helper to prepare a node actions batch
#[cfg(feature = "ssr")]
pub async fn helper_node_action_batch(
    batch_type: BatchType,
    interval_secs: u64,
    context: &ServerGlobalState,
) -> Result<u16, ServerFnError> {
    match &batch_type {
        BatchType::Create { node_opts, .. } => {
            // validate rewards address before accepting the batch
            parse_and_validate_addr(&node_opts.rewards_addr).map_err(ServerFnError::new)?;
        }
        BatchType::Start(l)
        | BatchType::Stop(l)
        | BatchType::Upgrade(l)
        | BatchType::Recycle(l)
        | BatchType::Remove(l) => {
            // TODO: filter out nodes which are already part of a batch,
            // perhaps even return an error...?...
            if l.is_empty() {
                return Err(ServerFnError::new("Empty list of node ids received."));
            }

            // let's lock all nodes which are part of the batch,
            // so the user cannot action on it till the batch is completed or cancelled.
            let duration = Duration::from_secs((interval_secs + 2) * l.len() as u64);
            for node_id in l.iter() {
                context.db_client.set_node_status_to_locked(node_id).await;

                // let's also prevent the backend from updating its status
                context
                    .node_status_locked
                    .lock(node_id.clone(), duration)
                    .await;
            }
        }
    }

    let batch_id = rand::rng().random_range(0..=u16::MAX);
    let batch_info = NodesActionsBatch::new(batch_id, batch_type, interval_secs);
    logging::log!("Creating new batch with id {batch_id}: {batch_info:?}");

    let len = {
        let batches = &mut context.node_action_batches.lock().await.1;
        batches.push(batch_info);
        batches.len()
    };
    if len == 1 {
        tokio::spawn(run_batches(context.clone()));
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
                batch.status = "In progress".to_string();
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
                        batch_id = cancel_rx.recv() => {
                            if matches!(batch_id, Ok(id) if id == batch_info.id) {
                                break;
                            }
                        },
                        _ = sleep(Duration::from_secs(batch_info.interval_secs)) => {
                            let mut node_opts_clone = node_opts.clone();
                            node_opts_clone.port += i;
                            node_opts_clone.metrics_port += i;
                            i += 1;
                            match helper_create_node_instance(node_opts_clone, &context).await {
                                Err(err) => logging::log!(
                                    "Failed to create node instance {i}/{count} as part of a batch: {err}"
                                ),
                                Ok(_) => if let Some(ref mut b) = context
                                    .node_action_batches.lock().await.1
                                    .iter_mut()
                                    .find(|batch| batch.id == batch_info.id)
                                {
                                    b.complete += 1;
                                }
                            }

                            if i == count {
                                break;
                            }
                        }
                    }
                }
            }
            BatchType::Start(ref nodes)
            | BatchType::Stop(ref nodes)
            | BatchType::Upgrade(ref nodes)
            | BatchType::Recycle(ref nodes)
            | BatchType::Remove(ref nodes) => {
                let count = nodes.len();
                logging::log!("Starting actions batch for {count} nodes ...");
                let mut i = 0;
                loop {
                    select! {
                        batch_id = cancel_rx.recv() => {
                            if matches!(batch_id, Ok(id) if id == batch_info.id) {
                                break;
                            }
                        },
                        _ = sleep(Duration::from_secs(batch_info.interval_secs)) => {
                            let node_id = nodes[i].clone();
                            context.node_status_locked.remove(&node_id).await;
                            context.db_client.unlock_node_status(&node_id).await;
                            let res = match batch_info.batch_type {
                                BatchType::Start(_) => helper_start_node_instance(node_id, &context).await,
                                BatchType::Stop(_) => helper_stop_node_instance(node_id,&context,NodeStatus::Stopping).await,
                                BatchType::Upgrade(_) => helper_upgrade_node_instance(&node_id,
                                        &context.node_status_locked,
                                        &context.db_client,
                                        #[cfg(not(feature = "native"))]
                                        &context.docker_client,
                                        #[cfg(feature = "native")]
                                        &context.node_manager
                                    ).await,
                                BatchType::Recycle(_) => helper_recycle_node_instance(node_id,&context).await,
                                BatchType::Remove(_) => helper_delete_node_instance(node_id,&context).await,
                                BatchType::Create {..} => Ok(())
                            };

                            match res {
                                Err(err) => logging::log!(
                                    "Node action failed on node instance {}/{count} as part of a batch: {err}", i+1
                                ),
                                Ok(()) => if let Some(ref mut b) = context
                                    .node_action_batches.lock().await.1
                                    .iter_mut()
                                    .find(|batch| batch.id == batch_info.id)
                                {
                                    b.complete += 1;
                                }
                            }

                            i += 1;
                            if i == count {
                                break;
                            }
                        }
                    }
                }
            }
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
#[server(name = CancelNodesActionsBatch, prefix = "/api", endpoint = "/batch/cancel")]
pub async fn cancel_batch(batch_id: u16) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Cancelling node action batch {batch_id} ...");

    let mut guard = context.node_action_batches.lock().await;
    guard.0.send(batch_id)?;

    if let Some(index) = guard.1.iter().position(|b| b.id == batch_id) {
        let batch = guard.1.remove(index);
        for node_id in batch.batch_type.ids().iter() {
            context.node_status_locked.remove(node_id).await;
            context.db_client.unlock_node_status(node_id).await;
        }
    }

    Ok(())
}

// Helper to parse and validate the rewards address
pub fn parse_and_validate_addr(input_str: &str) -> Result<Address, String> {
    let value = input_str
        .strip_prefix("0x")
        .unwrap_or(input_str)
        .to_string();

    if value.len() != REWARDS_ADDR_LENGTH {
        Err("Unexpected length of rewards address".to_string())
    } else if hex::decode(&value).is_err() {
        Err("The address entered is not hex-encoded".to_string())
    } else if value.to_lowercase() == value || value.to_uppercase() == value {
        // it's a non-checksummed address
        Address::from_str(&value).map_err(|err| err.to_string())
    } else {
        // validate checksum
        Address::parse_checksummed(format!("0x{value}"), None)
            .map_err(|_| "Checksum validation failed".to_string())
    }
}
