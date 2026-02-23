use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Application settings values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AppSettings {
    pub nodes_auto_upgrade: bool,
    pub nodes_auto_upgrade_delay: Duration,
    pub node_bin_version_polling_freq: Duration,
    pub nodes_metrics_polling_freq: Duration,
    pub disks_usage_check_freq: Duration,
    pub rewards_balances_retrieval_freq: Duration,
    pub rewards_monitoring_enabled: bool,
    pub l2_network_rpc_url: String,
    pub token_contract_address: String,
    pub lcd_display_enabled: bool,
    pub lcd_device: String,
    pub lcd_addr: String,
    pub node_list_page_size: u64,
    pub node_list_mode: u64,
    // ── AI Agent ──────────────────────────────────────────────────────────────
    /// Base URL of the OpenAI-compatible LLM API (e.g. "http://localhost:11434" for Ollama).
    pub llm_base_url: String,
    /// Model name to use (e.g. "llama3.2:3b").
    pub llm_model: String,
    /// Optional API key (empty string means no authentication).
    pub llm_api_key: String,
    /// Custom system prompt appended to the default Formicaio system prompt.
    pub system_prompt: String,
    /// Maximum number of prior messages to include in each LLM request.
    pub max_context_messages: u64,
    /// Whether the autonomous monitoring mode is currently enabled.
    pub autonomous_enabled: bool,
    /// How often (in seconds) the autonomous agent checks node health.
    pub autonomous_check_interval_secs: u64,
    /// Maximum number of tool-based actions the agent may take per monitoring cycle.
    pub autonomous_max_actions_per_cycle: u64,
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
            // How often to check nodes disks usage
            disks_usage_check_freq: Duration::from_secs(60),
            // Retrieve balances every 15 mins.
            rewards_balances_retrieval_freq: Duration::from_secs(60 * 15),
            // Rewards monitoring is enabled by default.
            rewards_monitoring_enabled: true,
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
            // Default to a local Ollama instance.
            llm_base_url: "http://localhost:11434".to_string(),
            // A small but capable model available in Ollama out of the box.
            llm_model: "llama3.2:3b".to_string(),
            // No API key required for local backends.
            llm_api_key: String::new(),
            // No extra instructions appended by default.
            system_prompt: String::new(),
            // Keep the last 20 messages for context; balances recall vs. token cost.
            max_context_messages: 20,
            // Autonomous monitoring is opt-in.
            autonomous_enabled: false,
            // Check node health every 60 seconds when autonomous mode is active.
            autonomous_check_interval_secs: 60,
            // Allow at most 3 corrective actions per monitoring cycle to avoid runaway behaviour.
            autonomous_max_actions_per_cycle: 3,
        }
    }
}
