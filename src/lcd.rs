use super::{app::BgTasksCmds, server_api_types::AppSettings};

use eyre::eyre;
use i2cdev::{
    core::I2CDevice,
    linux::{LinuxI2CDevice, LinuxI2CError},
};
use lcd::{Delay, Display, Hardware};
use leptos::logging;
use std::{collections::HashMap, sync::Arc, thread, time::Duration};
use tokio::{
    select,
    sync::{broadcast, Mutex},
    time::{interval, sleep},
};

// Delay between each stat shown on the LCD device.
const LCD_DELAY_SECS: u64 = 4;

// Helper to setup I2C interface with given I2C bus number and address.
fn setup_i2c(settings: &AppSettings) -> Result<Display<Pcf8574>, eyre::Error> {
    let i2c_bus_str = &settings.lcd_device;
    let i2c_bus_number = i2c_bus_str.parse::<u8>().map_err(|err| {
        eyre!("Failed to setup LCD display. Invalid I2C bus number {i2c_bus_str}: {err}")
    })?;

    let i2c_addr_str = settings
        .lcd_addr
        .strip_prefix("0x")
        .unwrap_or(&settings.lcd_addr)
        .to_string();
    let i2c_addr = u16::from_str_radix(&i2c_addr_str, 16).map_err(|err| {
        eyre!("Failed to setup LCD display. Invalid I2C address 0x{i2c_addr_str}: {err}")
    })?;

    let pcf_8574 = Pcf8574::new(i2c_bus_number, i2c_addr) .map_err(|err| {
        eyre!("Failed to setup LCD display with I2C device /dev/i2c-{i2c_bus_number} and address 0x{i2c_addr_str}: {err}")
    })?;

    let mut display = lcd::Display::new(pcf_8574);
    display.init(lcd::FunctionLine::Line2, lcd::FunctionDots::Dots5x8);
    display.display(
        lcd::DisplayMode::DisplayOn,
        lcd::DisplayCursor::CursorOff,
        lcd::DisplayBlink::BlinkOff,
    );

    Ok(display)
}

// Watch the stats and display them on the external LCD device.
pub async fn display_stats_on_lcd(
    settings: AppSettings,
    mut bg_tasks_cmds_rx: broadcast::Receiver<BgTasksCmds>,
    stats: Arc<Mutex<HashMap<String, String>>>,
) {
    let cur_lcd_device = settings.lcd_device.clone();
    let cur_lcd_addr = settings.lcd_addr.clone();
    let mut display = match setup_i2c(&settings) {
        Ok(d) => {
            logging::log!(
                "LCD device configured with I2C device: /dev/i2c-{cur_lcd_device}, address: 0x{cur_lcd_addr}."
            );
            d
        }
        Err(err) => {
            logging::log!("[ERROR] {err}");
            return;
        }
    };

    let delay = Duration::from_secs(LCD_DELAY_SECS);
    let mut update_lcd = interval(delay);

    loop {
        select! {
            settings = bg_tasks_cmds_rx.recv() => {
                if let Ok(BgTasksCmds::ApplySettings(s)) = settings {
                    if !s.lcd_display_enabled
                        || cur_lcd_device != s.lcd_device
                        || cur_lcd_addr != s.lcd_addr
                    {
                        logging::log!("Disabling LCD device on /dev/i2c-{cur_lcd_device}, address: 0x{cur_lcd_addr}...");
                        let mut pcf_8574: Pcf8574 = display.unwrap(); // this unwrap doesn't panic
                        pcf_8574.backlight(false);
                        return;
                    }
                }
            },
            _ = update_lcd.tick() => {
                let stats_clone = { stats.lock().await.clone() };
                logging::log!("Updating stats on LCD device...");
                for (k, v) in stats_clone.iter() {
                    display.clear();
                    display.home();
                    display.print(k);
                    display.position(15 - v.len() as u8, 1);
                    display.print(v);
                    sleep(delay).await;
                }

                update_lcd.reset_after(update_lcd.period());
            }
        }
    }
}

// Represents an LCD display attached via PCF8574 I2C expander. Use the traits in the [`lcd`]
// crate to interact with it.
struct Pcf8574 {
    dev: LinuxI2CDevice,
    data: u8,
}

impl Pcf8574 {
    // Create a new instance, using the Linux I2C interface for communication. `bus` is the number
    // of `/dev/i2c-<bus>` to use, and `address` is the I2C address of the device.
    //
    // After opening the device, defaults to ignoring all I/O errors; see [`Self::on_error`] and
    // [`ErrorHandling`] for how to change this behavior.
    fn new(bus: u8, address: u16) -> Result<Self, LinuxI2CError> {
        Ok(Self {
            dev: LinuxI2CDevice::new(format!("/dev/i2c-{bus}"), address)?,
            data: 0b0000_1000, // backlight on by default
        })
    }

    // Set the display's backlight on or off.
    fn backlight(&mut self, on: bool) {
        self.set_bit(3, on);
        self.apply();
    }

    fn set_bit(&mut self, offset: u8, bit: bool) {
        if bit {
            self.data |= 1 << offset;
        } else {
            self.data &= !(1 << offset);
        }
    }
}

impl Hardware for Pcf8574 {
    fn rs(&mut self, bit: bool) {
        self.set_bit(0, bit);
    }

    fn enable(&mut self, bit: bool) {
        self.set_bit(2, bit);
    }

    fn data(&mut self, bits: u8) {
        assert!(bits & 0xF0 == 0, "4-bit mode is required");
        self.data = (self.data & 0x0F) | (bits << 4);
    }

    fn apply(&mut self) {
        if let Err(err) = self.dev.smbus_write_byte(self.data) {
            logging::log!("[ERROR] LCD smbus_write_byte failed: {err}");
        }
    }
}

impl Delay for Pcf8574 {
    fn delay_us(&mut self, delay_usec: u32) {
        thread::sleep(Duration::from_micros(u64::from(delay_usec)));
    }
}
