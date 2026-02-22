-- AI Agent: autonomous events and agent settings

-- Log of autonomous agent actions and observations
CREATE TABLE IF NOT EXISTS agent_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,   -- 'action_taken', 'anomaly_detected', 'info', 'error'
    description TEXT NOT NULL,
    timestamp INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_events_ts
    ON agent_events(timestamp DESC);

-- Agent configuration columns added to the existing settings table
ALTER TABLE settings ADD COLUMN llm_base_url TEXT NOT NULL DEFAULT 'http://localhost:11434';
ALTER TABLE settings ADD COLUMN llm_model TEXT NOT NULL DEFAULT 'llama3.2:3b';
ALTER TABLE settings ADD COLUMN llm_api_key TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN system_prompt TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN max_context_messages INTEGER NOT NULL DEFAULT 20;
ALTER TABLE settings ADD COLUMN autonomous_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN autonomous_check_interval_secs INTEGER NOT NULL DEFAULT 60;
ALTER TABLE settings ADD COLUMN autonomous_max_actions_per_cycle INTEGER NOT NULL DEFAULT 3;
