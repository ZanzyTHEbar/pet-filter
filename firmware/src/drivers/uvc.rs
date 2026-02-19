//! UVC LED driver (PT4115 constant-current buck).
//!
//! Dual-layer safety:
//! 1. Hardware interlock — relay in series, controlled by magnetic reed switch.
//! 2. Software enable — GPIO output to relay coil.
//!
//! A single-point software bug cannot energise UVC with the chamber open.
//!
//! ## Dual-target design
//!
//! On ESP-IDF: drives real GPIO enable pin and LEDC PWM via hw_init.
//! On host/test: tracks state in-memory only.

use log::warn;

use crate::drivers::hw_init;
use crate::pins;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UvcState {
    Off,
    On { duty: u8 },
    Faulted(&'static str),
}

pub struct UvcDriver {
    state: UvcState,
    hw_duty: u8,
}

impl UvcDriver {
    pub fn new() -> Self {
        Self {
            state: UvcState::Off,
            hw_duty: 0,
        }
    }

    pub fn enable(&mut self, duty: u8) {
        let duty = duty.min(100);
        if duty == 0 {
            self.disable();
            return;
        }

        self.set_enable_hw(true);
        self.set_duty_hw(duty);

        self.hw_duty = duty;
        self.state = UvcState::On { duty };
    }

    pub fn disable(&mut self) {
        self.set_enable_hw(false);
        self.set_duty_hw(0);

        self.hw_duty = 0;
        self.state = UvcState::Off;
    }

    pub fn fault_shutdown(&mut self, reason: &'static str) {
        self.disable();
        self.state = UvcState::Faulted(reason);
        warn!("UVC fault shutdown: {}", reason);
    }

    fn set_enable_hw(&self, on: bool) {
        hw_init::gpio_write(pins::UVC_ENABLE_GPIO, on);
    }

    fn set_duty_hw(&self, duty: u8) {
        let duty_8bit = ((duty as u16) * 255 / 100) as u8;
        hw_init::ledc_set(hw_init::LEDC_CH_UVC, duty_8bit);
    }

    pub fn state(&self) -> UvcState {
        self.state
    }

    pub fn is_on(&self) -> bool {
        matches!(self.state, UvcState::On { .. })
    }

    pub fn current_duty(&self) -> u8 {
        self.hw_duty
    }
}
