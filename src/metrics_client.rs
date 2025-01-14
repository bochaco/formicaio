use super::{
    db_client::DbClient,
    metrics::*,
    node_instance::{ContainerId, NodeInstanceInfo},
};

use alloy::primitives::U256;
use chrono::Utc;
use leptos::{logging, prelude::*};
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;

// Default value for the nodes metrics host
const DEFAULT_NODES_METRICS_HOST: &str = "127.0.0.1";

// Predefined set of metrics to monitor and collect.
const NODE_METRICS_TO_COLLECT: [&str; 9] = [
    METRIC_KEY_BALANCE,
    METRIC_KEY_MEM_USED_MB,
    METRIC_KEY_CPU_USEAGE,
    METRIC_KEY_RECORDS,
    METRIC_KEY_RELEVANT_RECORDS,
    METRIC_KEY_CONNECTED_PEERS,
    METRIC_KEY_PEERS_IN_RT,
    METRIC_KEY_SHUNNED_COUNT,
    METRIC_KEY_NET_SIZE,
];

// Predefined set of historic metrics to store in DB.
const NODE_METRICS_TO_STORE_IN_DB: [&str; 2] = [METRIC_KEY_MEM_USED_MB, METRIC_KEY_CPU_USEAGE];
// Env var to enable the use of a metrics proxy service by providing its IP and port number.
const METRICS_PROXY_ADDR: &str = "METRICS_PROXY_ADDR";

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
        let endpoint = match std::env::var(METRICS_PROXY_ADDR) {
            Ok(addr) => format!("http://{addr}/{port}"),
            Err(_) => {
                let host = ip.clone().unwrap_or(DEFAULT_NODES_METRICS_HOST.to_string());
                format!("http://{host}:{port}/metrics")
            }
        };

        Self { endpoint }
    }

    // Fetch, filter, and return the predefined type of metrics.
    pub async fn fetch_metrics(&self) -> Result<Vec<NodeMetric>, MetricsClientError> {
        logging::log!(
            "Sending request to node metrics server: {} ...",
            self.endpoint
        );

        let response = reqwest::get(&self.endpoint).await?.text().await?;

        let mut fetched_metrics = Vec::new();
        let timestamp = Utc::now().timestamp_millis();
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
                    timestamp,
                });
            }
        }

        Ok(fetched_metrics)
    }
}

// Structure to keep track of all nodes metrics. These metrics
// are collected periodically at the backend in a background task,
// and consumed by the frontend through a server api.
#[derive(Debug)]
pub struct NodesMetrics {
    // Cache of the last metrics for each node indexed by their container id.
    data: HashMap<ContainerId, HashMap<String, NodeMetric>>,
    // DB client to store all metrics collected overtime
    db_client: DbClient,
}

impl NodesMetrics {
    pub fn new(db_client: DbClient) -> Self {
        Self {
            data: HashMap::new(),
            db_client,
        }
    }

    // Store a data point for the specified container id.
    pub async fn store(&mut self, container_id: &ContainerId, metrics: &[NodeMetric]) {
        // store into our DB cache those we keep as logs/historic values
        self.db_client
            .store_node_metrics(
                container_id.to_string(),
                metrics
                    .iter()
                    .filter(|m| NODE_METRICS_TO_STORE_IN_DB.contains(&m.key.as_str())),
            )
            .await;

        // let's now update our in-memory cache with new metrics values
        let metrics: HashMap<String, NodeMetric> =
            metrics.iter().map(|m| (m.key.clone(), m.clone())).collect();
        let _ = self.data.insert(container_id.to_string(), metrics.clone());
    }

    // Remove all the metrics for the specified container id
    pub async fn remove_container_metrics(&mut self, container_id: &ContainerId) {
        self.db_client.delete_node_metrics(container_id).await;
        let _ = self.data.remove(container_id);
    }

    // Return all the metrics for the specified container id with given filters
    pub async fn get_container_metrics(
        &self,
        container_id: ContainerId,
        since: Option<i64>,
    ) -> Metrics {
        self.db_client.get_node_metrics(container_id, since).await
    }

    // Update given node instance info with in-memory cached metrics
    pub fn update_node_info(&self, info: &mut NodeInstanceInfo) {
        if let Some(metrics) = self.data.get(&info.container_id) {
            if let Some(metric) = metrics.get(METRIC_KEY_BALANCE) {
                info.rewards = U256::from_str(&metric.value).ok();
            }

            if let Some(metric) = metrics.get(METRIC_KEY_MEM_USED_MB) {
                info.mem_used = metric.value.parse::<f64>().ok();
            }

            if let Some(metric) = metrics.get(METRIC_KEY_CPU_USEAGE) {
                info.cpu_usage = metric.value.parse::<f64>().ok();
            }

            if let Some(metric) = metrics.get(METRIC_KEY_RECORDS) {
                info.records = metric.value.parse::<usize>().ok();
            }

            if let Some(metric) = metrics.get(METRIC_KEY_RELEVANT_RECORDS) {
                info.relevant_records = metric.value.parse::<usize>().ok();
            }

            if let Some(metric) = metrics.get(METRIC_KEY_CONNECTED_PEERS) {
                info.connected_peers = metric.value.parse::<usize>().ok();
            }

            if let Some(metric) = metrics.get(METRIC_KEY_PEERS_IN_RT) {
                info.kbuckets_peers = metric.value.parse::<usize>().ok();
            }

            if let Some(metric) = metrics.get(METRIC_KEY_SHUNNED_COUNT) {
                info.shunned_count = metric.value.parse::<usize>().ok();
            }

            if let Some(metric) = metrics.get(METRIC_KEY_NET_SIZE) {
                info.net_size = metric.value.parse::<usize>().ok();
            }
        }
    }
}
