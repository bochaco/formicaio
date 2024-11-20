CREATE TABLE IF NOT EXISTS settings (
    -- Enabled/disabled nodes auto-upgrading feature
    nodes_auto_upgrade INTEGER,
    -- Delay between each of the nodes auto-upgrading
    nodes_auto_upgrade_delay_secs INTEGER
);

INSERT INTO settings (
    nodes_auto_upgrade, 
    nodes_auto_upgrade_delay_secs
) VALUES (
    0,
    10
);
