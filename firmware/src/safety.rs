//! Safety supervisor.
//!
//! The supervisor runs **every tick before the FSM** and accumulates a
//! fault bitmask in `FsmContext.fault_flags`.  The FSM state handlers
//! check this mask to decide whether to transition to `Error`.
//!
//! ## Fault lifecycle
//!
//! 1. A condition triggers a fault (e.g. water level low).
//! 2. The supervisor sets the corresponding bit in `fault_flags`.
//! 3. The FSM transitions to `Error`; `error_enter` kills all actuators.
//! 4. Each tick in `Error`, the supervisor re-evaluates.  If the
//!    condition clears, it unsets the bit.
//! 5. When `fault_flags == 0`, the FSM `error_update` handler returns
//!    `Some(StateId::Idle)`.
//!
//! This approach supports **multiple simultaneous faults**: the system
//! does not leave `Error` until *every* fault is resolved.

use crate::config::SystemConfig;
use crate::error::SafetyFault;
use crate::fsm::context::SensorSnapshot;
use log::{error, info};

/// Safety supervisor.
pub struct SafetySupervisor {
    max_temp_c: f32,
    /// Latched fault bitmask.
    faults: u8,
    /// Whether the pump is currently commanded on (set by main loop).
    pump_commanded: bool,
    /// Grace period (ticks) after pump start before flow is checked.
    flow_grace_ticks: u32,
    /// Ticks since pump was last commanded on.
    pump_on_ticks: u32,
}

impl SafetySupervisor {
    pub fn new(config: &SystemConfig) -> Self {
        Self {
            max_temp_c: config.max_temperature_c,
            faults: 0,
            pump_commanded: false,
            // Allow ~3 seconds for the peristaltic pump to prime.
            flow_grace_ticks: 3000 / config.control_loop_interval_ms,
            pump_on_ticks: 0,
        }
    }

    /// Inform the supervisor whether the pump is currently commanded on.
    pub fn set_pump_commanded(&mut self, on: bool) {
        if on && !self.pump_commanded {
            self.pump_on_ticks = 0; // Reset grace counter on fresh start
        }
        self.pump_commanded = on;
    }

    /// Evaluate all safety conditions against the latest sensor snapshot.
    /// Returns the updated fault bitmask.
    pub fn evaluate(&mut self, snap: &SensorSnapshot) -> u8 {
        // ── Water level ───────────────────────────────────────────
        self.eval_fault(
            SafetyFault::WaterLevelLow,
            !snap.tank_a_ok,
        );

        // ── Flow check (only after grace period while pump on) ───
        if self.pump_commanded {
            self.pump_on_ticks = self.pump_on_ticks.saturating_add(1);
            if self.pump_on_ticks > self.flow_grace_ticks {
                self.eval_fault(
                    SafetyFault::NoFlowDetected,
                    !snap.flow_detected,
                );
            }
        } else {
            // Pump is off — clear flow fault (not relevant)
            self.clear_fault(SafetyFault::NoFlowDetected);
            self.pump_on_ticks = 0;
        }

        // ── Temperature ───────────────────────────────────────────
        self.eval_fault(
            SafetyFault::OverTemperature,
            snap.temperature_c > self.max_temp_c,
        );

        // ── UVC interlock ─────────────────────────────────────────
        self.eval_fault(
            SafetyFault::UvcInterlockOpen,
            !snap.uvc_interlock_closed,
        );

        self.faults
    }

    /// Current fault bitmask.
    pub fn faults(&self) -> u8 {
        self.faults
    }

    /// True if **any** fault is active.
    pub fn has_faults(&self) -> bool {
        self.faults != 0
    }

    /// Check if a specific fault is active.
    pub fn has_fault(&self, fault: SafetyFault) -> bool {
        self.faults & fault.mask() != 0
    }

    // ── Internal ──────────────────────────────────────────────────

    /// Set or clear a fault bit based on a boolean condition.
    fn eval_fault(&mut self, fault: SafetyFault, condition: bool) {
        if condition {
            if self.faults & fault.mask() == 0 {
                error!("SAFETY FAULT SET: {fault}");
            }
            self.faults |= fault.mask();
        } else {
            if self.faults & fault.mask() != 0 {
                info!("SAFETY FAULT CLEARED: {fault}");
            }
            self.faults &= !fault.mask();
        }
    }

    /// Unconditionally clear a fault bit.
    fn clear_fault(&mut self, fault: SafetyFault) {
        self.faults &= !fault.mask();
    }
}
