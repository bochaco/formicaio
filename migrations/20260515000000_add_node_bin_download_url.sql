-- NULL means "use the default GitHub releases URL"
ALTER TABLE settings ADD COLUMN node_bin_download_url TEXT;
