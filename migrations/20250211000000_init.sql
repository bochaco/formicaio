-- Initialise Formicaio database tables

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
    home_network INTEGER,
    node_logs INTEGER,
    records TEXT,
    connected_peers TEXT,
    kbuckets_peers TEXT,
    ips TEXT
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
    lcd_addr TEXT
);

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
    lcd_addr
) VALUES (
    -- Nodes auto-upgrade disabled.
    0,
    -- 10secs delay between each node auto-upgrading.
    10,
    -- Check latest version of node binary every couple of hours.
    60 * 60 * 2,
    -- Check balances every 15mins.
    60 * 15,
    -- Poll nodes metrics every 5 secs.
    5,
    -- Arbitrum One network.
    "https://arb1.arbitrum.io/rpc",
    -- ANT token contract on Arbitrum One network.
    "0xa78d8321B20c4Ef90eCd72f2588AA985A4BDb684",
    -- LCD device disabled.
    0,
    -- I2C bus number 1, i.e. /dev/i2c-1.
    "1",
    -- I2C address 0x27.
    "0x27"
);
