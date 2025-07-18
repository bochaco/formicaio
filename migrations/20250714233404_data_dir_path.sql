-- Path where the node stores its data and tracking information.
-- If empty, then default is used instead.
ALTER TABLE nodes ADD COLUMN data_dir_path TEXT;
