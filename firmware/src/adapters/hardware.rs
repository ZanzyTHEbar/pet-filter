//! Hardware adapter — bridges real peripherals to domain port traits.
//!
//! Owns the [`SensorHub`] and all actuator drivers, exposing them
//! through [`SensorPort`] and [`ActuatorPort`].  This is the only
//! module in the system that touches actual hardware.  On non-espidf
//! targets, the underlying drivers use cfg-gated simulation stubs.

use crate::app::ports::{ActuatorPort, SensorPort};
use crate::drivers::pump::{Direction, PumpDriver};
use crate::drivers::status_led::StatusLed;
use crate::drivers::uvc::UvcDriver;
use crate::fsm::context::SensorSnapshot;
use crate::sensors::SensorHub;

/// Concrete adapter that combines all hardware behind port traits.
pub struct HardwareAdapter {
    sensor_hub: SensorHub,
    pump: PumpDriver,
    uvc: UvcDriver,
    led: StatusLed,
}

impl HardwareAdapter {
    pub fn new(
        sensor_hub: SensorHub,
        pump: PumpDriver,
        uvc: UvcDriver,
        led: StatusLed,
    ) -> Self {
        Self {
            sensor_hub,
            pump,
            uvc,
            led,
        }
    }
}

// ── SensorPort implementation ─────────────────────────────────

impl SensorPort for HardwareAdapter {
    fn read_all(&mut self, elapsed_secs: f32) -> SensorSnapshot {
        self.sensor_hub.read_all(elapsed_secs)
    }

    fn read_ammonia_fast(&mut self) -> f32 {
        self.sensor_hub.ammonia.read().ppm
    }
}

// ── ActuatorPort implementation ───────────────────────────────

impl ActuatorPort for HardwareAdapter {
    fn set_pump(&mut self, duty: u8, forward: bool) {
        let dir = if forward {
            Direction::Forward
        } else {
            Direction::Reverse
        };
        self.pump.set(duty, dir);
    }

    fn stop_pump(&mut self) {
        self.pump.stop();
    }

    fn enable_uvc(&mut self, duty: u8) {
        self.uvc.enable(duty);
    }

    fn disable_uvc(&mut self) {
        self.uvc.disable();
    }

    fn fault_shutdown_uvc(&mut self, reason: &'static str) {
        self.uvc.fault_shutdown(reason);
    }

    fn is_uvc_on(&self) -> bool {
        self.uvc.is_on()
    }

    fn set_led(&mut self, r: u8, g: u8, b: u8) {
        self.led.set_colour(r, g, b);
    }

    fn all_off(&mut self) {
        self.pump.stop();
        self.uvc.disable();
        self.led.off();
    }
}
