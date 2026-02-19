//! Log-based event sink adapter.
//!
//! Implements [`EventSink`] by writing structured application events to
//! the ESP-IDF logger (which goes to UART / USB-CDC in production).
//! A future MQTT or BLE adapter would implement the same trait.

use log::info;

use crate::app::events::AppEvent;
use crate::app::ports::EventSink;

/// Adapter that logs every [`AppEvent`] to the serial console.
pub struct LogEventSink;

impl LogEventSink {
    pub fn new() -> Self {
        Self
    }
}

impl EventSink for LogEventSink {
    fn emit(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Telemetry(t) => {
                info!(
                    "TELEM | state={:?} | NH3={:.1}/{:.1}ppm | flow={:.0}mL/min | \
                     T={:.1}\u{00b0}C | tank_a={} tank_b={} | pump={}% uvc={}% | \
                     faults=0b{:08b}",
                    t.state,
                    t.nh3_ppm,
                    t.nh3_avg_ppm,
                    t.flow_ml_per_min,
                    t.temperature_c,
                    if t.tank_a_ok { "OK" } else { "LOW" },
                    if t.tank_b_ok { "OK" } else { "LOW" },
                    t.pump_duty,
                    t.uvc_duty,
                    t.fault_flags,
                );
            }
            AppEvent::StateChanged { from, to } => {
                info!("STATE | {:?} -> {:?}", from, to);
            }
            AppEvent::FaultDetected(flags) => {
                info!("FAULT | detected, flags=0b{:08b}", flags);
            }
            AppEvent::FaultCleared => {
                info!("FAULT | all cleared");
            }
            AppEvent::Started(state) => {
                info!("START | initial_state={:?}", state);
            }
        }
    }
}
