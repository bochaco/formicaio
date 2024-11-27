use i2cdev::core::I2CDevice;
use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};
use lcd::{Delay, Hardware};
use leptos::*;
use std::{collections::HashMap, sync::Arc, thread, thread::sleep, time::Duration};
use tokio::sync::Mutex;

// Delay between each stat shown on the LCD device.
const LCD_DELAY_SECS: u64 = 4;

// Attempt to setup I2C communication to external LCD device.
pub fn setup_lcd() -> Result<Arc<Mutex<HashMap<String, String>>>, eyre::Error> {
    let stats = Arc::new(Mutex::new(HashMap::<String, String>::new()));

    let i2c_bus_str = "1";
    let i2c_bus_number = u8::from_str_radix(i2c_bus_str, 10).map_err(|err| {
        eyre::eyre!("Cannot display stats in LCD. Invalid I2C bus number {i2c_bus_str}: {err}")
    })?;

    let i2c_addr_hex = "27"; // another common addr is: 0x3f ,find out with '$ sudo ic2detect'
    let i2c_addr = u16::from_str_radix(i2c_addr_hex, 16).map_err(|err| {
        eyre::eyre!("Cannot display stats in LCD. Invalid I2C address 0x{i2c_addr_hex}: {err}")
    })?;

    let pcf_8574 = Pcf8574::new(i2c_bus_number, i2c_addr)
    .map_err(|err| eyre::eyre!("Cannot display stats in LCD with I2C device /dev/i2c-{i2c_bus_str} and address 0x{i2c_addr}: {err}"))?;

    tokio::spawn(display_stats_on_lcd(pcf_8574, stats.clone()));

    Ok(stats)
}

// Watch the stats and display them on the external LCD device.
async fn display_stats_on_lcd(pcf_8574: Pcf8574, stats: Arc<Mutex<HashMap<String, String>>>) {
    let mut display = lcd::Display::new(pcf_8574);
    display.init(lcd::FunctionLine::Line2, lcd::FunctionDots::Dots5x8);

    loop {
        let stats_clone = { stats.lock().await.clone() };
        logging::log!("Updating stats on LCD display...");
        if stats_clone.is_empty() {
            display.display(
                lcd::DisplayMode::DisplayOff,
                lcd::DisplayCursor::CursorOff,
                lcd::DisplayBlink::BlinkOff,
            );
        } else {
            display.display(
                lcd::DisplayMode::DisplayOn,
                lcd::DisplayCursor::CursorOff,
                lcd::DisplayBlink::BlinkOff,
            );
        }

        for (k, v) in stats_clone.iter() {
            display.clear();
            display.home();
            display.print(&k);
            display.position(15 - v.len() as u8, 1);
            display.print(&v);
            sleep(std::time::Duration::from_secs(LCD_DELAY_SECS));
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
    ///
    // After opening the device, defaults to ignoring all I/O errors; see [`Self::on_error`] and
    // [`ErrorHandling`] for how to change this behavior.
    fn new(bus: u8, address: u16) -> Result<Self, LinuxI2CError> {
        Ok(Self {
            dev: LinuxI2CDevice::new(format!("/dev/i2c-{bus}"), address)?,
            data: 0b0000_1000, // backlight on by default
        })
    }

    // Set the display's backlight on or off.
    #[allow(dead_code)]
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
            logging::log!("smbus_write_byte failed: {err}");
        }
    }
}

impl Delay for Pcf8574 {
    fn delay_us(&mut self, delay_usec: u32) {
        thread::sleep(Duration::from_micros(u64::from(delay_usec)));
    }
}
