use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeMetric {
    // Name/key of the metric.
    pub key: String,
    // Value measured the metric.
    pub value: String,
    // Timestamp of the metric. Note this isn't used to sorting metrics in cache.
    pub timestamp: i64,
}

// Set of metrics collected for a node, indexed by metric name/key.
pub type Metrics = HashMap<String, Vec<NodeMetric>>;

// The number of Nanos in the node reward wallet.
pub const METRIC_KEY_BALANCE: &str = "ant_node_current_reward_wallet_balance";
// Memory used by the process in MegaBytes.
pub const METRIC_KEY_MEM_USED_MB: &str = "ant_networking_process_memory_used_mb";
// The percentage of CPU used by the process. Value is from 0-100.
pub const METRIC_KEY_CPU_USAGE: &str = "ant_networking_process_cpu_usage_percentage";
// The number of records stored locally.
pub const METRIC_KEY_RECORDS: &str = "ant_networking_records_stored";
// The number of records that we're responsible for. This is used to calculate the store cost.
pub const METRIC_KEY_RELEVANT_RECORDS: &str = "ant_networking_relevant_records";
// The number of peers that we are currently connected to.
pub const METRIC_KEY_CONNECTED_PEERS: &str = "ant_networking_connected_peers";
// The total number of peers in our routing table.
pub const METRIC_KEY_PEERS_IN_RT: &str = "ant_networking_peers_in_routing_table";
// Number of peers that have shunned our node.
pub const METRIC_KEY_SHUNNED_COUNT: &str = "ant_networking_shunned_count_total";
// The estimated number of nodes in the network calculated by the peers in our RT.
pub const METRIC_KEY_NET_SIZE: &str = "ant_networking_estimated_network_size";
// The reachability status of the node (e.g. Ongoing/NotPerformed/Reachable/NotRoutable/UPnPSupported).
pub const METRIC_KEY_REACHABILITY: &str = "ant_networking_reachability_status";
