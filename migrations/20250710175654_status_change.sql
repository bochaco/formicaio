ALTER TABLE nodes ADD COLUMN is_status_locked INTEGER;
ALTER TABLE nodes ADD COLUMN is_status_unknown INTEGER;

UPDATE nodes SET is_status_locked=0,is_status_unknown=0;

UPDATE nodes SET status='"Active"', is_status_locked=1 WHERE status = '{"Locked":"Active"}';
UPDATE nodes SET status='"Active"', is_status_unknown=1 WHERE status = '{"Unknown":"Active"}';
UPDATE nodes SET status='{"Inactive":"Stopped"}' WHERE status != '"Active"';
