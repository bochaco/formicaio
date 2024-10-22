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
}
