use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    pub node_list_page_size: u64,
    pub node_list_mode: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // Node auto-upgrading is disabled by default.
            nodes_auto_upgrade: false,
            // Delay 10 secs. between each node being auto-upgraded.
            nodes_auto_upgrade_delay: Duration::from_secs(10),
            // Check latest version of node binary every six hours.
            node_bin_version_polling_freq: Duration::from_secs(60 * 60 * 6),
            // How often to fetch metrics and node info from active/running nodes
            nodes_metrics_polling_freq: Duration::from_secs(5),
            // Retrieve balances every 15 mins.
            rewards_balances_retrieval_freq: Duration::from_secs(60 * 15),
            // Arbitrum One network.
            l2_network_rpc_url: "https://arb1.arbitrum.io/rpc".to_string(),
            // ANT token contract on Arbitrum One network.
            token_contract_address: "0xa78d8321B20c4Ef90eCd72f2588AA985A4BDb684".to_string(),
            // External LCD device disabled.
            lcd_display_enabled: false,
            // I2C bus number 1, i.e. device at /dev/i2c-1.
            lcd_device: "1".to_string(),
            // I2C backpack address 0x27, another common addr is: 0x3f. Check it out with 'sudo ic2detect -y <bus-number>'.
            lcd_addr: "0x27".to_string(),
            // The number of nodes to display per page in the list and tile views.
            node_list_page_size: 30u64,
            // The default layout for the Nodes list page. (0 == Tile, 1 == List)
            node_list_mode: 0u64,
        }
    }
}
