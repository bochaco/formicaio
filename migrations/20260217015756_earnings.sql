-- History of reward payments, i.e. earnings, for all addresses
CREATE TABLE IF NOT EXISTS earnings (
    address TEXT NOT NULL,
    amount TEXT,
    block_number INTEGER,
    timestamp INTEGER
);
