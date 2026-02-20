//! RGB status LED driver.
//!
//! Three LEDC PWM channels (CH2-4) drive discrete R/G/B LEDs (or a
//! common-cathode RGB LED).
//!
//! ## Dual-target design
//!
//! On ESP-IDF: drives three LEDC PWM channels via hw_init.
//! On host/test: tracks state in-memory only.

use crate::drivers::hw_init;

pub struct StatusLed {
    current: (u8, u8, u8),
}

impl StatusLed {
    pub fn new() -> Self {
        Self { current: (0, 0, 0) }
    }

    pub fn set_colour(&mut self, r: u8, g: u8, b: u8) {
        hw_init::ledc_set(hw_init::LEDC_CH_LED_R, r);
        hw_init::ledc_set(hw_init::LEDC_CH_LED_G, g);
        hw_init::ledc_set(hw_init::LEDC_CH_LED_B, b);
        self.current = (r, g, b);
    }

    pub fn off(&mut self) {
        self.set_colour(0, 0, 0);
    }

    pub fn current_colour(&self) -> (u8, u8, u8) {
        self.current
    }
}
