-- How often (in secs) to check nodes disks usage
ALTER TABLE settings ADD COLUMN disks_usage_check_freq INTEGER;

-- Node disk usage in bytes
ALTER TABLE nodes ADD COLUMN disk_usage INTEGER;

-- How often (in secs) to check nodes disks usage
UPDATE settings SET disks_usage_check_freq=60;