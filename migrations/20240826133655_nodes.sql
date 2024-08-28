CREATE TABLE IF NOT EXISTS nodes (
    container_id TEXT PRIMARY KEY NOT NULL,
    peer_id TEXT,
    bin_version TEXT,
    port INTEGER,
    rpc_api_port INTEGER,
    rewards TEXT,
    balance TEXT,
    records TEXT,
    connected_peers TEXT
);