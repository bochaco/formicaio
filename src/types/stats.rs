use alloy_primitives::U256;
use serde::{Deserialize, Serialize};

/// Node stats collected by the backend and retrievable through the public server API.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    /// Total balance across all nodes
    pub total_balance: U256,
    /// Balances of the addresses assigned to nodes for rewards
    pub balances: Vec<(String, U256)>,
    /// Earnings analytics for rewards addresses
    pub earnings: Vec<(String, EarningsStats)>,
    /// Whether earnings history is still being fetched from the chain (not fully synced yet)
    #[serde(default)]
    pub earnings_syncing: bool,
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
    /// Total disk size/space in bytes
    pub total_disk_space: u64,
    /// Available/free disk size/space in bytes
    pub available_disk_space: u64,
    /// Total disk size/space in bytes used by all nodes
    pub used_disk_space: u64,
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

/// Detailed statistics for a single earnings period
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PeriodStats {
    /// Period label (e.g., "24 Hours", "72 Hours")
    pub label: String,
    /// Period length in hours
    pub length_hours: u32,
    /// Total earnings in this period
    pub total_earned: U256,
    /// Total earnings in previous period
    pub total_earned_prev: U256,
    /// Percentage change from previous period (None if previous was 0)
    pub change_percent: Option<f64>,
    /// Absolute change from previous period
    pub change_amount: f64,
    /// Number of payments in this period
    pub num_payments: usize,
    /// Average payment amount
    pub average_payment: U256,
    /// Median payment amount
    pub median_payment: U256,
    /// Largest payment amount
    pub largest_payment: U256,
}

/// Aggregated earnings statistics for all periods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsStats {
    pub period_1: PeriodStats,
    pub period_2: PeriodStats,
    pub period_3: PeriodStats,
    pub period_4: PeriodStats,
}

impl Default for EarningsStats {
    fn default() -> Self {
        Self {
            // Statistics for the last 48 hours
            period_1: PeriodStats {
                label: "Last 48 Hours".to_string(),
                length_hours: 48,
                ..PeriodStats::default()
            },
            // Statistics for the last week
            period_2: PeriodStats {
                label: "Last Week".to_string(),
                length_hours: 168,
                ..PeriodStats::default()
            },
            // Statistics for the last month
            period_3: PeriodStats {
                label: "Last Month".to_string(),
                length_hours: 720,
                ..PeriodStats::default()
            },
            // Statistics for the last 3 months
            period_4: PeriodStats {
                label: "Last 3 Months".to_string(),
                length_hours: 2160,
                ..PeriodStats::default()
            },
        }
    }
}
