CREATE TABLE IF NOT EXISTS nodes_metrics (
    container_id TEXT NOT NULL,
    timestamp INTEGER,
    key TEXT,
    value TEXT
);