//! Inbound commands to the application service.
//!
//! These represent actions requested by the outside world (BLE, serial,
//! MQTT, scheduler) that the [`AppService`](super::service::AppService)
//! interprets and acts upon.

use crate::config::SystemConfig;
use crate::fsm::StateId;

/// Commands that external adapters can send into the application core.
#[derive(Debug, Clone)]
pub enum AppCommand {
    /// Trigger an immediate scrub cycle (e.g. scheduled or manual).
    StartScrub,

    /// Force the FSM into a specific state (debug / testing only).
    ForceState(StateId),

    /// Hot-reload configuration (e.g. from BLE provisioning or NVS).
    UpdateConfig(SystemConfig),

    /// Explicitly persist the current config to NVS immediately.
    SaveConfig,

    /// Return crash log entries and runtime metrics via RPC.
    GetDiagnostics,

    /// Erase all crash log entries from NVS.
    ClearDiagnostics,
}
