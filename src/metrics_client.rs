use super::{app::ServerGlobalState, docker_msgs::ContainerId, node_instance::NodeInstanceInfo};

use chrono::Utc;
use leptos::*;
use std::collections::HashMap;
use thiserror::Error;

// Default value for the nodes metrics host
const DEFAULT_NODES_METRICS_HOST: &str = "127.0.0.1";
// Maximum number of metrics data points to be kept per node
const DEFAULT_METRICS_MAX_SIZE: usize = 100;

// Predefined set of metrics to monitor and collect in cache.
const NODE_METRICS_TO_COLLECT: [&str; 2] = [
    "sn_node_total_forwarded_rewards",
    "sn_node_current_reward_wallet_balance",
];

// Structure to keep track of all nodes metrics. These metrics
// are collected periodically at the backend in a background task,
// and consumed by the frontend through a server api.
#[derive(Debug)]
pub struct NodesMetrics {
    // Cache of the metrics for each node indexed by their container id.
    data: HashMap<ContainerId, HashMap<String, Vec<NodeMetric>>>,
    // Number of data points to keep for each node.
    max_size: usize,
}

#[derive(Clone, Debug)]
pub struct NodeMetric {
    // Name/key of the metric.
    pub key: String,
    // Value measured the metric.
    pub value: String,
    // Timestamp of the metric. Note this isn't used to sorting metrics in cache.
    pub timestamp: i64,
}

impl NodesMetrics {
    // TODO: allow user to define the max number of data points to be kept
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            max_size: DEFAULT_METRICS_MAX_SIZE,
        }
    }

    // Add a data point for the specified container id,
    // removing the oldest if max size has been reached.
    pub fn push(&mut self, container_id: &ContainerId, metrics: &[NodeMetric]) {
        let nodes_metrics = self.data.entry(container_id.to_string()).or_default();
        for m in metrics {
            let metrics = nodes_metrics.entry(m.key.clone()).or_default();
            metrics.push(m.clone());
            if metrics.len() > self.max_size {
                metrics.remove(0);
            }
        }
    }

    // Return all the metrics for the specified container id
    pub fn get_all_metrics(&self, container_id: &ContainerId) -> HashMap<String, Vec<NodeMetric>> {
        self.data.get(container_id).cloned().unwrap_or_default()
    }

    // Return last data point for a specific type of metrics and specific container id
    pub fn get_last_data_point(&self, container_id: &ContainerId, key: &str) -> Option<NodeMetric> {
        self.data
            .get(container_id) // get the metrics for the given container id
            .and_then(|m| m.get(key)) // get the metrics of the given type
            .and_then(|m| m.get(m.len() - 1)) // get the last value from the data points
            .cloned()
    }
}

#[derive(Debug, Error)]
pub enum MetricsClientError {
    #[error(transparent)]
    TransportError(#[from] reqwest::Error),
}

// Client to query metrics from nodes
pub struct NodeMetricsClient {
    endpoint: String,
}

impl NodeMetricsClient {
    pub fn new(ip: &Option<String>, port: u16) -> Self {
        let host = ip.clone().unwrap_or(DEFAULT_NODES_METRICS_HOST.to_string());
        Self {
            endpoint: format!("http://{host}:{port}/metrics"),
        }
    }

    pub async fn update_node_info(&self, info: &mut NodeInstanceInfo) {
        if let Err(err) = self.fetch_rewards_metrics(info).await {
            logging::log!(
                "Failed to get metrics from running node at endpoint {}: {err:?}",
                self.endpoint
            );
        }
    }

    // Fetch metrics related to rewards and update given node instance info
    async fn fetch_rewards_metrics(
        &self,
        info: &mut NodeInstanceInfo,
    ) -> Result<(), MetricsClientError> {
        let context = expect_context::<ServerGlobalState>();
        let nodes_metrics = context.nodes_metrics.lock().await;
        if let Some(metric) =
            nodes_metrics.get_last_data_point(&info.container_id, "sn_node_total_forwarded_rewards")
        {
            info.rewards_received = metric.value.parse::<u64>().ok();
        }
        if let Some(metric) = nodes_metrics
            .get_last_data_point(&info.container_id, "sn_node_current_reward_wallet_balance")
        {
            info.balance = metric.value.parse::<u64>().ok();
        }

        Ok(())
    }

    // Fetch, filter, and return the predefined type of metrics.
    pub async fn fetch_metrics(&self) -> Result<Vec<NodeMetric>, MetricsClientError> {
        logging::log!(
            "Sending request to node metrics server: {} ...",
            self.endpoint
        );

        let response = reqwest::get(&self.endpoint).await?.text().await?;

        let mut fetched_metrics = Vec::new();
        for line in response.lines() {
            if line.starts_with('#') {
                continue; // Skip comments
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && NODE_METRICS_TO_COLLECT.contains(&parts[0]) {
                let key = parts[0].to_string();
                let value = parts[1].to_string();
                fetched_metrics.push(NodeMetric {
                    key,
                    value,
                    timestamp: Utc::now().timestamp(),
                });
            }
        }

        Ok(fetched_metrics)
    }
}
