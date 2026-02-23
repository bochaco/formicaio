-- Add toggle to enable/disable automatic rewards monitoring and earnings analytics.
ALTER TABLE settings ADD COLUMN rewards_monitoring_enabled INTEGER NOT NULL DEFAULT 1;
