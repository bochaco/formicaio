use alloy_primitives::U256;
use serde::{Deserialize, Serialize};

/// Node stats collected by the backend and retrievable through the public server API.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub total_balance: U256,
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub inactive_nodes: usize,
    pub connected_peers: usize,
    pub shunned_count: usize,
    pub estimated_net_size: usize,
    pub stored_records: usize,
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
