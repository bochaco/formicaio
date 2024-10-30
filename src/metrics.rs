use super::node_instance::NodeInstanceInfo;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Hex-encoded container id
pub type ContainerId = String;

// Set of metrics collected for a node, indexed by metric name/key.
pub type Metrics = HashMap<String, Vec<NodeMetric>>;

// Structure to keep track of all nodes metrics. These metrics
// are collected periodically at the backend in a background task,
// and consumed by the frontend through a server api.
#[derive(Debug)]
pub struct NodesMetrics {
    // Cache of the metrics for each node indexed by their container id.
    data: HashMap<ContainerId, Metrics>,
    // Number of data points to keep for each node.
    max_size: usize,
}

// The number of Nanos in the node reward wallet.
pub const METRIC_KEY_BALANCE: &str = "sn_node_current_reward_wallet_balance";
// The store cost of the node.
pub const METRIC_KEY_STORE_COST: &str = "sn_networking_store_cost";
// Memory used by the process in MegaBytes.
pub const METRIC_KEY_MEM_USED_MB: &str = "sn_networking_process_memory_used_mb";
// The percentage of CPU used by the process. Value is from 0-100.
pub const METRIC_KEY_CPU_USEAGE: &str = "sn_networking_process_cpu_usage_percentage";
// The number of records stored locally.
pub const METRIC_KEY_RECORDS: &str = "sn_networking_records_stored";
// The number of records that we're responsible for. This is used to calculate the store cost.
pub const METRIC_KEY_RELEVANT_RECORDS: &str = "sn_networking_relevant_records";
// The number of peers that we are currently connected to.
pub const METRIC_KEY_CONNECTED_PEERS: &str = "sn_networking_connected_peers";
// The total number of peers in our routing table.
pub const METRIC_KEY_PEERS_IN_RT: &str = "sn_networking_peers_in_routing_table";

// Maximum number of metrics data points to be kept per node
const DEFAULT_METRICS_MAX_SIZE: usize = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    pub fn get_container_metrics(&self, container_id: &ContainerId) -> Option<&Metrics> {
        self.data.get(container_id)
    }

    // Return all the metrics for the specified container id with given filters
    pub fn get_metrics(
        &self,
        container_id: &ContainerId,
        since: Option<i64>,
        keys: &[String],
    ) -> Metrics {
        if let Some(metrics) = self.data.get(container_id) {
            metrics
                .iter()
                .filter(|(k, _)| keys.is_empty() || keys.contains(k))
                .map(|(k, values)| {
                    let filtered_values = if let Some(t) = since {
                        values.iter().cloned().filter(|v| v.timestamp > t).collect()
                    } else {
                        values.clone()
                    };
                    (k.clone(), filtered_values)
                })
                .collect()
        } else {
            Metrics::default()
        }
    }

    // Update given node instance info with in-memory cached metrics
    pub fn update_node_info(&self, info: &mut NodeInstanceInfo) {
        if let Some(metrics) = self.get_container_metrics(&info.container_id) {
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
    }
}

// Return last data point for a specific metric
fn get_last_data_point<'a>(metrics: &'a Metrics, key: &'a str) -> Option<&'a NodeMetric> {
    metrics
        .get(key) // get the metrics of the given type
        .and_then(|m| m.get(m.len() - 1)) // get the last value from the data points
}
