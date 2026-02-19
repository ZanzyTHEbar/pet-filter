//! Outbound application events.
//!
//! The [`AppService`](super::service::AppService) emits these through the
//! [`EventSink`](super::ports::EventSink) port.  Adapters on the other
//! side decide what to do with them — log to serial, publish over MQTT,
//! update a BLE characteristic, etc.

use crate::fsm::StateId;

/// Structured events emitted by the application core.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Periodic telemetry snapshot.
    Telemetry(TelemetryData),

    /// The FSM transitioned between states.
    StateChanged { from: StateId, to: StateId },

    /// One or more safety faults were raised.
    FaultDetected(u8),

    /// All safety faults have been cleared.
    FaultCleared,

    /// The application service has started (carries initial state).
    Started(StateId),
}

/// A point-in-time telemetry snapshot suitable for logging or transmission.
#[derive(Debug, Clone)]
pub struct TelemetryData {
    pub state: StateId,
    pub nh3_ppm: f32,
    pub nh3_avg_ppm: f32,
    pub flow_ml_per_min: f32,
    pub temperature_c: f32,
    pub tank_a_ok: bool,
    pub tank_b_ok: bool,
    pub pump_duty: u8,
    pub uvc_duty: u8,
    pub fault_flags: u8,
}
