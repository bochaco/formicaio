CREATE TABLE IF NOT EXISTS nodes (
    container_id TEXT PRIMARY KEY NOT NULL,
    peer_id TEXT,
    bin_version TEXT,
    rewards TEXT,
    balance TEXT,
    chunks TEXT,
    connected_peers TEXT
);