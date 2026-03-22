-- Remove columns no longer supported by saorsa-node
ALTER TABLE nodes DROP COLUMN upnp;
ALTER TABLE nodes DROP COLUMN reachability_check;
-- Rename ip_addr to ip_version (stores "ipv4"/"ipv6"/"dual" instead of an IP address)
ALTER TABLE nodes RENAME COLUMN ip_addr TO ip_version;
-- Convert any stored IP addresses to "dual" as default fallback
UPDATE nodes SET ip_version = 'dual'
    WHERE ip_version != 'ipv4' AND ip_version != 'ipv6' AND ip_version != 'dual';
