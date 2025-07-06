-- IP address where nodes listen on.
ALTER TABLE nodes ADD COLUMN ip_addr TEXT;

UPDATE nodes SET ip_addr = "0.0.0.0";