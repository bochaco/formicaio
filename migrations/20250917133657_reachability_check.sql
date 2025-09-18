-- Whether nodes shall perform reachability check upon starting up.
ALTER TABLE nodes ADD COLUMN reachability_check INTEGER;

-- We enable check on existing nodes since that's the default behaviour in node binary.
UPDATE nodes SET reachability_check=1;
