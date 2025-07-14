use async_graphql::{Context, Object, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::schema::{MetricsFilter, NodeFilter, NodeInstanceInfoType, NodeMetricType, StatsType};
use crate::app::ServerGlobalState;
use crate::types::{
    NodeId, NodeInstanceInfo, NodeMetric, Stats, filters::NodeFilter as AppNodeFilter,
};

pub struct Query {
    state: Arc<ServerGlobalState>,
}

impl Query {
    pub fn new(state: Arc<ServerGlobalState>) -> Self {
        Self { state }
    }
}

#[Object]
impl Query {
    /// Get all nodes with optional filtering
    async fn nodes(&self, filter: Option<NodeFilter>) -> Result<Vec<NodeInstanceInfoType>> {
        let context = &self.state;

        #[cfg(not(feature = "native"))]
        let nodes_list = context
            .docker_client
            .get_containers_list()
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to get containers: {}", e)))?;

        #[cfg(feature = "native")]
        let nodes_list = context.db_client.get_nodes_list().await.into_values();

        let mut nodes = Vec::new();
        for mut node_info in nodes_list.into_iter() {
            // Read node metadata from database
            context
                .db_client
                .get_node_metadata(&mut node_info, false)
                .await;

            // Apply filter if provided
            if let Some(ref filter) = filter {
                let app_filter = AppNodeFilter {
                    node_ids: None, // GraphQL filter doesn't support node_ids yet
                    status: filter.status.as_ref().map(|s| {
                        // Convert string status to NodeStatusFilter
                        // This is a simplified conversion - you might want to expand this
                        vec![]
                    }),
                };
                if !app_filter.passes(&node_info) {
                    continue;
                }
            }

            // Update with metrics if node is active
            if node_info.status.is_active() {
                context
                    .nodes_metrics
                    .write()
                    .await
                    .update_node_info(&mut node_info);
            }

            nodes.push(node_info.into());
        }

        Ok(nodes)
    }

    /// Get a specific node by ID
    async fn node(&self, node_id: String) -> Result<Option<NodeInstanceInfoType>> {
        let context = &self.state;

        #[cfg(not(feature = "native"))]
        let nodes_list = context
            .docker_client
            .get_containers_list()
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to get containers: {}", e)))?;

        #[cfg(feature = "native")]
        let nodes_list = context.db_client.get_nodes_list().await.into_values();

        for mut node_info in nodes_list.into_iter() {
            if node_info.node_id == node_id {
                // Read node metadata from database
                context
                    .db_client
                    .get_node_metadata(&mut node_info, false)
                    .await;

                // Update with metrics if node is active
                if node_info.status.is_active() {
                    context
                        .nodes_metrics
                        .write()
                        .await
                        .update_node_info(&mut node_info);
                }

                return Ok(Some(node_info.into()));
            }
        }

        Ok(None)
    }

    /// Get metrics for a specific node
    async fn node_metrics(
        &self,
        node_id: String,
        filter: Option<MetricsFilter>,
    ) -> Result<HashMap<String, Vec<NodeMetricType>>> {
        let context = &self.state;
        let since = filter.and_then(|f| f.since);

        let metrics = context
            .nodes_metrics
            .read()
            .await
            .get_node_metrics(node_id, since)
            .await;

        let result: HashMap<String, Vec<NodeMetricType>> = metrics
            .into_iter()
            .map(|(key, metrics)| {
                let metric_types: Vec<NodeMetricType> =
                    metrics.into_iter().map(|m| m.into()).collect();
                (key, metric_types)
            })
            .collect();

        Ok(result)
    }

    /// Get all stats
    async fn stats(&self) -> Result<StatsType> {
        let context = &self.state;
        let stats = context.stats.read().await.clone();
        Ok(stats.into())
    }

    /// Get active nodes count
    async fn active_nodes_count(&self) -> Result<usize> {
        let context = &self.state;
        let stats = context.stats.read().await.clone();
        Ok(stats.active_nodes)
    }

    /// Get total nodes count
    async fn total_nodes_count(&self) -> Result<usize> {
        let context = &self.state;
        let stats = context.stats.read().await.clone();
        Ok(stats.total_nodes)
    }

    /// Get total balance
    async fn total_balance(&self) -> Result<String> {
        let context = &self.state;
        let stats = context.stats.read().await.clone();
        Ok(stats.total_balance.to_string())
    }

    /// Get network size estimate
    async fn network_size(&self) -> Result<usize> {
        let context = &self.state;
        let stats = context.stats.read().await.clone();
        Ok(stats.estimated_net_size)
    }

    /// Get stored records count
    async fn stored_records(&self) -> Result<usize> {
        let context = &self.state;
        let stats = context.stats.read().await.clone();
        Ok(stats.stored_records)
    }
}
