-- Add metrics_mode setting column (0 = HTTP endpoint, 1 = OS/Docker system stats, 2 = disabled)
ALTER TABLE settings ADD COLUMN metrics_mode INTEGER NOT NULL DEFAULT 0;
