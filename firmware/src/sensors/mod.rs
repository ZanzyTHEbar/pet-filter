//! Sensor subsystem — individual drivers and the aggregating [`SensorHub`].
//!
//! The hub owns every sensor driver and produces a [`SensorSnapshot`] each
//! tick that gets written into `FsmContext.sensors`.

pub mod ammonia;
pub mod flow;
pub mod temperature;
pub mod water_level;

use core::sync::atomic::{AtomicBool, Ordering};

use crate::fsm::context::SensorSnapshot;
use ammonia::AmmoniaSensor;
use flow::FlowSensor;
use temperature::TemperatureSensor;
use water_level::WaterLevelSensor;

/// Atomic cache of the UVC interlock state, written from the GPIO ISR or
/// from `set_interlock_from_isr()` at boot.  `true` = interlock closed
/// (lid magnet present = safe); `false` = open (fault condition).
static INTERLOCK_CLOSED_ATOMIC: AtomicBool = AtomicBool::new(false);

/// Update the interlock state from an ISR or boot-time GPIO read.
/// Lock-free — safe to call from interrupt context.
pub fn set_interlock_from_isr(closed: bool) {
    INTERLOCK_CLOSED_ATOMIC.store(closed, Ordering::Release);
}

/// Aggregates all sensor drivers and produces a unified snapshot.
#[allow(dead_code)]
pub struct SensorHub {
    pub ammonia: AmmoniaSensor,
    pub flow: FlowSensor,
    pub water_level: WaterLevelSensor,
    pub temperature: TemperatureSensor,
    /// Cached UVC interlock state (read from GPIO).
    interlock_closed: bool,
    interlock_gpio: i32,
}

impl SensorHub {
    /// Construct a new hub.  Pass in pre-built drivers (built in main
    /// where peripheral ownership is established).
    pub fn new(
        ammonia: AmmoniaSensor,
        flow: FlowSensor,
        water_level: WaterLevelSensor,
        temperature: TemperatureSensor,
        interlock_gpio: i32,
    ) -> Self {
        Self {
            ammonia,
            flow,
            water_level,
            temperature,
            interlock_closed: false,
            interlock_gpio,
        }
    }

    /// Read every sensor and return a unified snapshot.
    ///
    /// Individual read failures are logged and the previous good value is
    /// retained — a single flaky sensor must not crash the control loop.
    pub fn read_all(&mut self, elapsed_secs: f32) -> SensorSnapshot {
        let nh3 = self.ammonia.read();
        let flow = self.flow.read(elapsed_secs);
        let (level_a, level_b) = self.water_level.read();
        let temp = self.temperature.read();

        // Read the interlock state from the ISR-maintained atomic.
        // set_interlock_from_isr() is called on every GPIO edge and once
        // at boot in init_isr_service() to seed the initial level.
        self.interlock_closed = INTERLOCK_CLOSED_ATOMIC.load(Ordering::Acquire);

        SensorSnapshot {
            nh3_ppm: nh3.ppm,
            nh3_avg_ppm: nh3.avg_ppm,
            nh3_raw: nh3.raw,
            flow_ml_per_min: flow.flow_ml_per_min,
            flow_detected: flow.flow_detected,
            tank_a_ok: level_a.water_present,
            tank_b_ok: level_b.water_present,
            temperature_c: temp.celsius,
            over_temperature: temp.over_temp,
            uvc_interlock_closed: self.interlock_closed,
        }
    }

    /// Externally set the interlock state (called from main loop GPIO read).
    pub fn set_interlock(&mut self, closed: bool) {
        self.interlock_closed = closed;
    }
}
