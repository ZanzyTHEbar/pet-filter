//! YF-S201 hall-effect water flow sensor driver.
//!
//! The sensor outputs one pulse per ~2.22 mL of water flow.  An ISR
//! increments an atomic counter on each rising edge; the `read` method
//! samples and resets it to compute flow rate.
//!
//! Because the ISR and the main loop run on different cores (or at least
//! different priorities), the counter uses `AtomicU32` for lock-free
//! thread safety — the correct pattern for shared ISR state on ESP32.

use core::sync::atomic::{AtomicU32, Ordering};

/// Calibration: pulses per litre for YF-S201.
/// Datasheet: frequency (Hz) = 7.5 × flow_rate (L/min)
/// → 450 pulses/min at 1 L/min → 450 pulses per litre.
const PULSES_PER_LITRE: f32 = 450.0;

/// Global atomic counter incremented by the GPIO ISR.
/// `static` because ISR callbacks in ESP-IDF cannot capture closures.
static FLOW_PULSE_COUNT: AtomicU32 = AtomicU32::new(0);

/// Called from the GPIO ISR on each rising edge.
/// Must be `extern "C"` and `#[no_mangle]` compatible when registered
/// via `esp_idf_hal::gpio::PinDriver::set_interrupt`.
pub fn flow_isr_handler() {
    FLOW_PULSE_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Result of a flow measurement.
#[derive(Debug, Clone, Copy)]
pub struct FlowReading {
    /// Pulses counted in the measurement window.
    pub pulse_count: u32,
    /// Calculated flow rate (mL/min).
    pub flow_ml_per_min: f32,
    /// True if flow exceeds the minimum detection threshold.
    pub flow_detected: bool,
}

/// Flow sensor driver.
pub struct FlowSensor {
    /// Minimum flow in mL/min to be considered "detected".
    min_flow_threshold: f32,
    /// GPIO pin number (stored for diagnostics / re-init).
    _gpio: i32,
}

impl FlowSensor {
    pub fn new(gpio: i32) -> Self {
        Self {
            min_flow_threshold: 50.0,
            _gpio: gpio,
        }
    }

    /// Sample the atomic pulse counter, reset it, and compute flow rate.
    ///
    /// `elapsed_secs` is the time since the last call (typically 0.1 s at
    /// 10 Hz or 1.0 s at 1 Hz).
    pub fn read(&mut self, elapsed_secs: f32) -> FlowReading {
        // Atomically swap the counter to zero and read the old value.
        let count = FLOW_PULSE_COUNT.swap(0, Ordering::Relaxed);

        let pulses_per_min = if elapsed_secs > 0.0 {
            count as f32 / elapsed_secs * 60.0
        } else {
            0.0
        };

        // pulses_per_min / PULSES_PER_LITRE = litres/min, × 1000 = mL/min
        let flow_ml_per_min = pulses_per_min / PULSES_PER_LITRE * 1000.0;
        let flow_detected = flow_ml_per_min > self.min_flow_threshold;

        FlowReading {
            pulse_count: count,
            flow_ml_per_min,
            flow_detected,
        }
    }

    /// Override the minimum-flow detection threshold.
    pub fn set_min_flow_threshold(&mut self, ml_per_min: f32) {
        self.min_flow_threshold = ml_per_min;
    }
}
