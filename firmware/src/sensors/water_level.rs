//! Capacitive non-contact water level sensors for Tank A and Tank B.
//!
//! XKC-Y25-T12V sensors output a digital HIGH when water is present.
//! Each sensor is wired to a GPIO configured as a pull-up input.
//!
//! ## Dual-target design
//!
//! On ESP-IDF: reads real GPIO levels via hw_init helpers.
//! On host/test: defaults to water-present (safe default).

use core::sync::atomic::{AtomicBool, Ordering};

use crate::drivers::hw_init;
use crate::pins;

static SIM_WATER_A: AtomicBool = AtomicBool::new(true);
static SIM_WATER_B: AtomicBool = AtomicBool::new(true);

#[cfg(not(target_os = "espidf"))]
pub fn sim_set_water_a(present: bool) {
    SIM_WATER_A.store(present, Ordering::Relaxed);
}

#[cfg(not(target_os = "espidf"))]
pub fn sim_set_water_b(present: bool) {
    SIM_WATER_B.store(present, Ordering::Relaxed);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tank {
    A,
    B,
}

#[derive(Debug, Clone, Copy)]
pub struct WaterLevelReading {
    pub tank: Tank,
    pub water_present: bool,
}

pub struct WaterLevelSensor {
    _gpio_a: i32,
    _gpio_b: i32,
    last_a: bool,
    last_b: bool,
}

impl WaterLevelSensor {
    pub fn new(gpio_a: i32, gpio_b: i32) -> Self {
        Self {
            _gpio_a: gpio_a,
            _gpio_b: gpio_b,
            last_a: true,
            last_b: true,
        }
    }

    pub fn read(&mut self) -> (WaterLevelReading, WaterLevelReading) {
        self.last_a = self.read_gpio_a();
        self.last_b = self.read_gpio_b();

        (
            WaterLevelReading { tank: Tank::A, water_present: self.last_a },
            WaterLevelReading { tank: Tank::B, water_present: self.last_b },
        )
    }

    #[cfg(target_os = "espidf")]
    fn read_gpio_a(&self) -> bool {
        hw_init::gpio_read(pins::WATER_LEVEL_A_GPIO)
    }

    #[cfg(not(target_os = "espidf"))]
    fn read_gpio_a(&self) -> bool {
        SIM_WATER_A.load(Ordering::Relaxed)
    }

    #[cfg(target_os = "espidf")]
    fn read_gpio_b(&self) -> bool {
        hw_init::gpio_read(pins::WATER_LEVEL_B_GPIO)
    }

    #[cfg(not(target_os = "espidf"))]
    fn read_gpio_b(&self) -> bool {
        SIM_WATER_B.load(Ordering::Relaxed)
    }

    pub fn supply_adequate(&self) -> bool {
        self.last_a
    }

    pub fn needs_refill(&self) -> bool {
        !self.last_a
    }
}
