-- How often to check which is the latest version of the node binary.
ALTER TABLE settings ADD COLUMN node_bin_version_polling_freq_secs INTEGER;

-- How often to query balances from the ledger.
ALTER TABLE settings ADD COLUMN rewards_balances_retrieval_freq_secs INTEGER;

-- How often to fetch metrics and node info from active/running nodes
ALTER TABLE settings ADD COLUMN nodes_metrics_polling_freq_secs INTEGER;

-- URL to send queries using RPC to get rewards addresses balances from L2 network.
ALTER TABLE settings ADD COLUMN l2_network_rpc_url TEXT;

-- ERC20 token contract address.
ALTER TABLE settings ADD COLUMN token_contract_address TEXT;


UPDATE settings SET
    -- Check latest version of node binary every couple of hours.
    node_bin_version_polling_freq_secs = 60 * 60 * 2,
    -- Check balances every 15mins.
    rewards_balances_retrieval_freq_secs = 60 * 15,
    -- Poll nodes metrics every 5 secs.
    nodes_metrics_polling_freq_secs = 5,
    -- Arbitrum Sepolia testnet.
    l2_network_rpc_url = "https://sepolia-rollup.arbitrum.io/rpc",
    -- ANT token contract on Arbitrum Sepolia testnet.
    token_contract_address = "0xBE1802c27C324a28aeBcd7eeC7D734246C807194";
