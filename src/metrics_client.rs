use super::metrics::*;

use chrono::Utc;
use leptos::*;
use thiserror::Error;

// Default value for the nodes metrics host
const DEFAULT_NODES_METRICS_HOST: &str = "127.0.0.1";

// Predefined set of metrics to monitor and collect in cache.
const NODE_METRICS_TO_COLLECT: [&str; 8] = [
    METRIC_KEY_BALANCE,
    METRIC_KEY_STORE_COST,
    METRIC_KEY_MEM_USED_MB,
    METRIC_KEY_CPU_USEAGE,
    METRIC_KEY_RECORDS,
    METRIC_KEY_RELEVANT_RECORDS,
    METRIC_KEY_CONNECTED_PEERS,
    METRIC_KEY_PEERS_IN_RT,
];

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
