//! Peristaltic pump motor driver (DRV8871 H-bridge).
//!
//! Variable-speed forward/reverse control via LEDC PWM (ch0) and a
//! digital direction pin.
//!
//! ## Safety contract
//!
//! The pump must never run when Tank A water level is low. Enforced by
//! the safety supervisor; this driver is a dumb actuator.
//!
//! ## Dual-target design
//!
//! On ESP-IDF: drives real PWM and GPIO via hw_init helpers.
//! On host/test: tracks state in-memory only.

use crate::drivers::hw_init;
use crate::pins;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpState {
    Stopped,
    Running { duty: u8, dir: Direction },
}

pub struct PumpDriver {
    state: PumpState,
    hw_duty: u8,
}

impl PumpDriver {
    pub fn new() -> Self {
        Self {
            state: PumpState::Stopped,
            hw_duty: 0,
        }
    }

    pub fn set(&mut self, duty: u8, direction: Direction) {
        let duty = duty.min(100);
        if duty == 0 {
            self.stop();
            return;
        }

        self.set_direction_hw(direction);
        self.set_duty_hw(duty);

        self.hw_duty = duty;
        self.state = PumpState::Running {
            duty,
            dir: direction,
        };
    }

    pub fn stop(&mut self) {
        self.set_duty_hw(0);
        self.set_direction_hw(Direction::Forward);
        self.hw_duty = 0;
        self.state = PumpState::Stopped;
    }

    fn set_direction_hw(&self, dir: Direction) {
        let high = matches!(dir, Direction::Forward);
        hw_init::gpio_write(pins::PUMP_DIR_GPIO, high);
    }

    fn set_duty_hw(&self, duty: u8) {
        let duty_8bit = ((duty as u16) * 255 / 100) as u8;
        hw_init::ledc_set(hw_init::LEDC_CH_PUMP, duty_8bit);
    }

    pub fn state(&self) -> PumpState {
        self.state
    }

    pub fn is_running(&self) -> bool {
        !matches!(self.state, PumpState::Stopped)
    }

    pub fn current_duty(&self) -> u8 {
        self.hw_duty
    }
}
