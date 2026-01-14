-- The number of nodes to display per page in the list and tile views.
ALTER TABLE settings ADD COLUMN node_list_page_size INTEGER;

-- The default layout for the Nodes list page. (0 == Tile, 1 == List)
ALTER TABLE settings ADD COLUMN node_list_mode INTEGER;

-- We set a default page size of 30
UPDATE settings SET node_list_page_size=30;
-- We set a default node list layout as Tile mode
UPDATE settings SET node_list_mode=0;
