use alloy_primitives::U256;
use serde::{Deserialize, Serialize};

/// Node stats collected by the backend and retrievable through the public server API.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    /// Total balance across all nodes
    pub total_balance: U256,
    /// Total number of node instances
    pub total_nodes: usize,
    /// Number of currently active nodes
    pub active_nodes: usize,
    /// Number of currently inactive nodes
    pub inactive_nodes: usize,
    /// Total number of peers connected across all nodes
    pub connected_peers: usize,
    /// Total number of peers which have shunned nodes
    pub shunned_count: usize,
    /// Estimated total network size calculated as the average of all nodes' network size observations
    pub estimated_net_size: usize,
    /// Total number of records stored across all nodes
    pub stored_records: usize,
    /// Total number of relevant records stored across all nodes
    pub relevant_records: usize,
}

/// Node stats formatted for UmbrelOS widgets.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WidgetFourStats {
    pub r#type: String,
    pub refresh: String,
    pub link: String,
    pub items: Vec<WidgetStat>,
}

/// Node stats collected by the backend to be retrieved for UmbrelOS widgets.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WidgetStat {
    pub title: String,
    pub text: String,
    pub subtext: String,
}
