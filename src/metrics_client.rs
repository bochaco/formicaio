use super::{
    app::ServerGlobalState,
    metrics::{Metrics, NodeMetric},
    node_instance::NodeInstanceInfo,
};

use chrono::Utc;
use leptos::*;
use thiserror::Error;

// Default value for the nodes metrics host
const DEFAULT_NODES_METRICS_HOST: &str = "127.0.0.1";

// Keys returned by the node for each of the metrics we will be monitoring
// The cumulative number of Nanos forwarded by the node.
const METRIC_KEY_REWARDS: &str = "sn_node_total_forwarded_rewards";
// The number of Nanos in the node reward wallet.
const METRIC_KEY_BALANCE: &str = "sn_node_current_reward_wallet_balance";
// The store cost of the node.
const METRIC_KEY_STORE_COST: &str = "sn_networking_store_cost";
// Memory used by the process in MegaBytes.
const METRIC_KEY_MEM_USED_MB: &str = "sn_networking_process_memory_used_mb";
// The percentage of CPU used by the process. Value is from 0-100.
const METRIC_KEY_CPU_USEAGE: &str = "sn_networking_process_cpu_usage_percentage";
// The number of records stored locally.
const METRIC_KEY_RECORDS: &str = "sn_networking_records_stored";
// The number of records that we're responsible for. This is used to calculate the store cost.
const METRIC_KEY_RELEVANT_RECORDS: &str = "sn_networking_relevant_records";
// The number of peers that we are currently connected to.
const METRIC_KEY_CONNECTED_PEERS: &str = "sn_networking_connected_peers";
// The total number of peers in our routing table.
const METRIC_KEY_PEERS_IN_RT: &str = "sn_networking_peers_in_routing_table";

// Predefined set of metrics to monitor and collect in cache.
const NODE_METRICS_TO_COLLECT: [&str; 9] = [
    METRIC_KEY_REWARDS,
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

    // Fetch metrics and update given node instance info
    pub async fn update_node_info(info: &mut NodeInstanceInfo) {
        let context = expect_context::<ServerGlobalState>();
        let nodes_metrics = context.nodes_metrics.lock().await;

        let metrics = nodes_metrics.get_container_metrics(&info.container_id);

        get_last_data_point(metrics, METRIC_KEY_REWARDS)
            .map(|metric| info.rewards = metric.value.parse::<u64>().ok());

        get_last_data_point(metrics, METRIC_KEY_BALANCE)
            .map(|metric| info.balance = metric.value.parse::<u64>().ok());

        get_last_data_point(metrics, METRIC_KEY_STORE_COST)
            .map(|metric| info.store_cost = metric.value.parse::<u64>().ok());

        get_last_data_point(metrics, METRIC_KEY_MEM_USED_MB)
            .map(|metric| info.mem_used = metric.value.parse::<u64>().ok());

        get_last_data_point(metrics, METRIC_KEY_CPU_USEAGE)
            .map(|metric| info.cpu_usage = Some(metric.value.clone()));

        get_last_data_point(metrics, METRIC_KEY_RECORDS)
            .map(|metric| info.records = metric.value.parse::<usize>().ok());

        get_last_data_point(metrics, METRIC_KEY_RELEVANT_RECORDS)
            .map(|metric| info.relevant_records = metric.value.parse::<usize>().ok());

        get_last_data_point(metrics, METRIC_KEY_CONNECTED_PEERS)
            .map(|metric| info.connected_peers = metric.value.parse::<usize>().ok());

        get_last_data_point(metrics, METRIC_KEY_PEERS_IN_RT)
            .map(|metric| info.kbuckets_peers = metric.value.parse::<usize>().ok());
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

// Return last data point for a specific metric
pub fn get_last_data_point<'a>(
    metrics: Option<&'a Metrics>,
    key: &'a str,
) -> Option<&'a NodeMetric> {
    metrics
        .and_then(|m| m.get(key)) // get the metrics of the given type
        .and_then(|m| m.get(m.len() - 1)) // get the last value from the data points
}
