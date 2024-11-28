-- Enable/disable external LCD device where nodes stats can be shown.
ALTER TABLE settings ADD COLUMN lcd_display_enabled INTEGER;

-- I2C bus number which is used to access the device '/dev/i2c-<bus-number>'.
ALTER TABLE settings ADD COLUMN lcd_device TEXT;

-- I2C backpack address (usually 0x27 or 0x3F).
ALTER TABLE settings ADD COLUMN lcd_addr TEXT;

UPDATE settings SET
    -- LCD device disabled.
    lcd_display_enabled = 0,
    -- I2C bus number 1, i.e. /dev/i2c-1.
    lcd_device = "1",
    -- I2C address 0x27.
    lcd_addr = "0x27";
