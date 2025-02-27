-- Enable/disable UPnP for nodes.
ALTER TABLE nodes ADD COLUMN upnp INTEGER;

UPDATE nodes SET upnp = 0;