use super::node_instance::NodeInstanceInfo;

use alloy_primitives::U256;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};

/// List of nodes, stats and currently running batch.
#[derive(Clone, Serialize, Deserialize)]
pub struct NodesInstancesInfo {
    pub latest_bin_version: Option<String>,
    pub nodes: HashMap<String, NodeInstanceInfo>,
    pub stats: Stats,
    pub batch_in_progress: Option<BatchInProgress>,
}

/// Application settings values.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppSettings {
    pub nodes_auto_upgrade: bool,
    pub nodes_auto_upgrade_delay: Duration,
    pub node_bin_version_polling_freq: Duration,
    pub nodes_metrics_polling_freq: Duration,
    pub rewards_balances_retrieval_freq: Duration,
    pub l2_network_rpc_url: String,
    pub token_contract_address: String,
    pub lcd_display_enabled: bool,
    pub lcd_device: String,
    pub lcd_addr: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // Node auto-upgrading is disabled by default.
            nodes_auto_upgrade: false,
            // Delay 10 secs. between each node being auto-upgraded.
            nodes_auto_upgrade_delay: Duration::from_secs(10),
            // Check latest version of node binary every couple of hours.
            node_bin_version_polling_freq: Duration::from_secs(60 * 60 * 2),
            // How often to fetch metrics and node info from active/running nodes
            nodes_metrics_polling_freq: Duration::from_secs(5),
            // Retrieve balances every 15 mins.
            rewards_balances_retrieval_freq: Duration::from_secs(60 * 15),
            // Arbitrum Sepolia testnet.
            l2_network_rpc_url: "https://sepolia-rollup.arbitrum.io/rpc".to_string(),
            // ANT token contract on Arbitrum Sepolia testnet.
            token_contract_address: "0xBE1802c27C324a28aeBcd7eeC7D734246C807194".to_string(),
            // External LCD device disabled.
            lcd_display_enabled: false,
            // I2C bus number 1, i.e. device at /dev/i2c-1.
            lcd_device: "1".to_string(),
            // I2C backpack address 0x27, another common addr is: 0x3f. Check it out with 'sudo ic2detect -y <bus-number>'.
            lcd_addr: "0x27".to_string(),
        }
    }
}

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

/// Information about any actively running nodes creation batch.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct BatchInProgress {
    pub created: u16,
    pub total: u16,
    pub auto_start: bool,
    pub interval_secs: u64,
}

/// Options when creating a new node instance.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodeOpts {
    pub port: u16,
    pub metrics_port: u16,
    pub rewards_addr: String,
    pub home_network: bool,
    pub node_logs: bool,
    pub auto_start: bool,
}
