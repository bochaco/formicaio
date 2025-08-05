use crate::types::{
    BatchOnMatch, BatchType, NodeFilter, NodeId, NodeInstanceInfo, NodeOpts, NodesActionsBatch,
    NodesInstancesInfo, Stats, WidgetFourStats,
};

use alloy_primitives::Address;
use leptos::prelude::*;
use leptos::server_fn::codec::{ByteStream, Streaming};
use std::{collections::HashMap, str::FromStr};

#[cfg(feature = "ssr")]
mod ssr_imports_and_defs {
    pub use crate::{
        app::ServerGlobalState,
        bg_tasks::{BgTasksCmds, prepare_node_action_batch},
        types::WidgetStat,
        views::truncated_balance_str,
    };
    pub use futures_util::StreamExt;
    pub use leptos::logging;
}

#[cfg(feature = "ssr")]
use ssr_imports_and_defs::*;

// Expected length of entered hex-encoded rewards address.
const REWARDS_ADDR_LENGTH: usize = 40;

/// Return a set of stats
#[server(name = FetchStats, prefix = "/api", endpoint = "/stats")]
pub async fn fetch_stats() -> Result<Stats, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let stats = context.stats.read().await.clone();
    Ok(stats)
}

/// Return a set of stats formatted for UmbrelOS widget
#[server(name = FetchStatsWidget, prefix = "/api", endpoint = "/stats_widget")]
pub async fn fetch_stats_widget() -> Result<WidgetFourStats, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let stats = context.stats.read().await.clone();
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

/// Obtain the list of existing nodes instances with their info.
#[server(name = ListNodeInstances, prefix = "/api", endpoint = "/nodes/list")]
pub async fn nodes_instances(
    filter: Option<NodeFilter>,
) -> Result<NodesInstancesInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();

    let latest_bin_version = context
        .app_ctx
        .latest_bin_version
        .read()
        .await
        .clone()
        .map(|v| v.to_string());
    let stats = context.stats.read().await.clone();

    let nodes = context
        .node_manager
        .filtered_nodes_list(filter, context.app_ctx.nodes_metrics)
        .await?;

    let scheduled_batches = context.app_ctx.node_action_batches.read().await.1.clone();

    Ok(NodesInstancesInfo {
        latest_bin_version,
        nodes,
        stats,
        scheduled_batches,
    })
}

/// Create and add a new node instance returning its info
#[server(name = CreateNodeInstance, prefix= "/api", endpoint = "/nodes/create")]
pub async fn create_node_instance(node_opts: NodeOpts) -> Result<NodeInstanceInfo, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();

    // validate rewards address before proceeding
    parse_and_validate_addr(&node_opts.rewards_addr).map_err(ServerFnError::new)?;

    let info = context.node_manager.create_node_instance(node_opts).await?;
    Ok(info)
}

/// Start a node instance with given id
#[server(name = StartNodeInstance, prefix= "/api", endpoint = "/nodes/start")]
pub async fn start_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    context.node_manager.start_node_instance(node_id).await?;
    Ok(())
}

/// Stop a node instance with given id
#[server(name = StopNodeInstance, prefix= "/api", endpoint = "/nodes/stop")]
pub async fn stop_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Stopping node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    context.node_manager.stop_node_instance(node_id).await?;
    Ok(())
}

/// Delete a node instance with given id
#[server(name = DeleteNodeInstance, prefix= "/api", endpoint = "/nodes/delete")]
pub async fn delete_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Deleting node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    context.node_manager.delete_node_instance(node_id).await?;
    Ok(())
}

/// Upgrade a node instance with given id
#[server(name = UpgradeNodeInstance, prefix = "/api", endpoint = "/nodes/upgrade")]
pub async fn upgrade_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    logging::log!("Upgrading node with ID: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    context.node_manager.upgrade_node_instance(&node_id).await?;
    Ok(())
}

/// Recycle a node instance by restarting it with a new node peer-id
#[server(name = RecycleNodeInstance, prefix= "/api", endpoint = "/nodes/recycle")]
pub async fn recycle_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Recycling node instance with Id: {node_id} ...");
    context.node_manager.recycle_node_instance(node_id).await?;
    Ok(())
}

/// Start streaming logs from a node instance with given id
#[server(output = Streaming, name = StartNodeLogsStream, prefix = "/api", endpoint = "/nodes/logs_stream")]
pub async fn start_node_logs_stream(node_id: NodeId) -> Result<ByteStream, ServerFnError> {
    logging::log!("Starting logs stream from node with Id: {node_id} ...");
    let context = expect_context::<ServerGlobalState>();
    let node_logs_stream = context.node_manager.get_node_logs_stream(&node_id).await?;

    let converted_stream = node_logs_stream.map(|item| {
        item.map_err(ServerFnError::from) // convert the error type
    });
    Ok(ByteStream::new(converted_stream))
}

/// Retrieve the metrics for a node instance with given id and filters
#[server(name = NodeMetrics, prefix = "/api", endpoint = "/nodes/metrics")]
pub async fn node_metrics(
    node_id: NodeId,
    since: Option<i64>,
) -> Result<HashMap<String, Vec<super::types::NodeMetric>>, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let metrics = context
        .app_ctx
        .nodes_metrics
        .read()
        .await
        .get_node_metrics(node_id, since)
        .await;
    Ok(metrics)
}

/// Retrieve the settings
#[server(name = GetSettings, prefix = "/api", endpoint = "/settings/get")]
pub async fn get_settings() -> Result<super::types::AppSettings, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let settings = context.db_client.get_settings().await;
    Ok(settings)
}

/// Update the settings
#[server(name = UpdateSettings, prefix = "/api", endpoint = "/settings/set")]
pub async fn update_settings(settings: super::types::AppSettings) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    context.db_client.update_settings(&settings).await?;
    context
        .app_ctx
        .bg_tasks_cmds_tx
        .send(BgTasksCmds::ApplySettings(settings))?;
    Ok(())
}

/// Return list of running and scheduled nodes actions batches
#[server(name = ListNodesActionsBatches, prefix = "/api", endpoint = "/batch/list")]
pub async fn nodes_actions_batches() -> Result<Vec<NodesActionsBatch>, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let batches = context.app_ctx.node_action_batches.read().await.1.clone();
    Ok(batches)
}

/// Prepare a new nodes actions batch
#[server(name = CreateNodesActionsBatch, prefix = "/api", endpoint = "/batch/create")]
pub async fn nodes_actions_batch_create(
    batch_type: BatchType,
    interval_secs: u64,
) -> Result<u16, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let batch_id = prepare_node_action_batch(
        batch_type,
        interval_secs,
        &context.app_ctx,
        &context.node_manager,
        &context.db_client,
    )
    .await?;
    Ok(batch_id)
}

/// Create a nodes actions batch based on matching rules
#[server(name = CreateNodesActionsBatchOnMatch, prefix = "/api", endpoint = "/batch/create_on_match")]
pub async fn nodes_actions_batch_on_match(
    batch_on_match: BatchOnMatch,
    interval_secs: u64,
) -> Result<u16, ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    let nodes_list = context.node_manager.get_nodes_list().await?;

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

    let batch_id = prepare_node_action_batch(
        batch_type,
        interval_secs,
        &context.app_ctx,
        &context.node_manager,
        &context.db_client,
    )
    .await?;
    Ok(batch_id)
}

/// Cancel all node instances creation batches
#[server(name = CancelNodesActionsBatch, prefix = "/api", endpoint = "/batch/cancel")]
pub async fn cancel_batch(batch_id: u16) -> Result<(), ServerFnError> {
    let context = expect_context::<ServerGlobalState>();
    logging::log!("Cancelling node action batch {batch_id} ...");

    let mut guard = context.app_ctx.node_action_batches.write().await;
    guard.0.send(batch_id)?;

    if let Some(index) = guard.1.iter().position(|b| b.id == batch_id) {
        let batch = guard.1.remove(index);
        for node_id in batch.batch_type.ids().iter() {
            context.app_ctx.node_status_locked.remove(node_id).await;
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
