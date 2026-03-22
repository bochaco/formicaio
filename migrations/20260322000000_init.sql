-- Consolidated initial schema (squashed from 14 migrations on 2026-03-22)

CREATE TABLE IF NOT EXISTS nodes (
    node_id TEXT PRIMARY KEY NOT NULL,
    pid INTEGER,
    created INTEGER,
    status_changed INTEGER,
    peer_id TEXT,
    status TEXT,
    bin_version TEXT,
    port INTEGER,
    metrics_port INTEGER,
    rewards TEXT,
    balance TEXT,
    rewards_addr TEXT,
    node_logs INTEGER,
    records TEXT,
    connected_peers TEXT,
    kbuckets_peers TEXT,
    ips TEXT,
    ip_version TEXT,
    is_status_locked INTEGER,
    is_status_unknown INTEGER,
    data_dir_path TEXT,
    disk_usage INTEGER
);

CREATE TABLE IF NOT EXISTS nodes_metrics (
    node_id TEXT NOT NULL,
    timestamp INTEGER,
    key TEXT,
    value TEXT
);

CREATE TABLE IF NOT EXISTS settings (
    -- Enabled/disabled nodes auto-upgrading feature
    nodes_auto_upgrade INTEGER,
    -- Delay between each of the nodes auto-upgrading
    nodes_auto_upgrade_delay_secs INTEGER,
    -- How often to check which is the latest version of the node binary.
    node_bin_version_polling_freq_secs INTEGER,
    -- How often to query balances from the ledger.
    rewards_balances_retrieval_freq_secs INTEGER,
    -- How often to fetch metrics and node info from active/running nodes
    nodes_metrics_polling_freq_secs INTEGER,
    -- URL to send queries using RPC to get rewards addresses balances from L2 network.
    l2_network_rpc_url TEXT,
    -- ERC20 token contract address.
    token_contract_address TEXT,
    -- Enable/disable external LCD device where nodes stats can be shown.
    lcd_display_enabled INTEGER,
    -- I2C bus number which is used to access the device '/dev/i2c-<bus-number>'.
    lcd_device TEXT,
    -- I2C backpack address (usually 0x27 or 0x3F).
    lcd_addr TEXT,
    -- The number of nodes to display per page in the list and tile views.
    node_list_page_size INTEGER,
    -- The default layout for the Nodes list page. (0 == Tile, 1 == List)
    node_list_mode INTEGER,
    -- How often (in secs) to check nodes disks usage
    disks_usage_check_freq INTEGER,
    -- LLM base URL for the AI agent (OpenAI-compatible endpoint)
    llm_base_url TEXT NOT NULL DEFAULT 'http://localhost:11434',
    -- LLM model name
    llm_model TEXT NOT NULL DEFAULT 'llama3.2:3b',
    -- LLM API key (optional)
    llm_api_key TEXT NOT NULL DEFAULT '',
    -- Custom system prompt appended to the agent's instructions
    system_prompt TEXT NOT NULL DEFAULT '',
    -- Maximum number of messages to keep in the agent's context window
    max_context_messages INTEGER NOT NULL DEFAULT 20,
    -- Whether autonomous monitoring mode is enabled
    autonomous_enabled INTEGER NOT NULL DEFAULT 0,
    -- How often (in secs) the autonomous agent checks node health
    autonomous_check_interval_secs INTEGER NOT NULL DEFAULT 60,
    -- Maximum actions the autonomous agent may take per check cycle
    autonomous_max_actions_per_cycle INTEGER NOT NULL DEFAULT 3,
    -- Enable/disable automatic rewards monitoring and earnings analytics
    rewards_monitoring_enabled INTEGER NOT NULL DEFAULT 1
);

-- Insert default settings row only on fresh databases
INSERT INTO settings (
    nodes_auto_upgrade,
    nodes_auto_upgrade_delay_secs,
    node_bin_version_polling_freq_secs,
    rewards_balances_retrieval_freq_secs,
    nodes_metrics_polling_freq_secs,
    l2_network_rpc_url,
    token_contract_address,
    lcd_display_enabled,
    lcd_device,
    lcd_addr,
    node_list_page_size,
    node_list_mode,
    disks_usage_check_freq
)
SELECT
    -- Nodes auto-upgrade disabled.
    0,
    -- 10 secs delay between each node auto-upgrading.
    10,
    -- Check latest version of node binary every 6 hours.
    60 * 60 * 6,
    -- Check balances every 15 mins.
    60 * 15,
    -- Poll nodes metrics every 5 secs.
    5,
    -- Arbitrum One network.
    'https://arb1.arbitrum.io/rpc',
    -- ANT token contract on Arbitrum One network.
    '0xa78d8321B20c4Ef90eCd72f2588AA985A4BDb684',
    -- LCD device disabled.
    0,
    -- I2C bus number 1, i.e. /dev/i2c-1.
    '1',
    -- I2C address 0x27.
    '0x27',
    -- Default page size of 30 nodes.
    30,
    -- Default layout: Tile mode (0).
    0,
    -- Check disk usage every 60 secs.
    60
WHERE NOT EXISTS (SELECT 1 FROM settings LIMIT 1);

-- History of reward payments, i.e. earnings, for all addresses
CREATE TABLE IF NOT EXISTS earnings (
    address TEXT NOT NULL,
    amount TEXT,
    block_number INTEGER,
    timestamp INTEGER
);

-- Log of autonomous agent actions and observations
CREATE TABLE IF NOT EXISTS agent_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    description TEXT NOT NULL,
    timestamp INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_events_ts
    ON agent_events(timestamp DESC);
