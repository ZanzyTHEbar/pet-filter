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
        self.eval_fault(SafetyFault::WaterLevelLow, !snap.tank_a_ok);

        // ── Flow check (only after grace period while pump on) ───
        if self.pump_commanded {
            self.pump_on_ticks = self.pump_on_ticks.saturating_add(1);
            if self.pump_on_ticks > self.flow_grace_ticks {
                self.eval_fault(SafetyFault::NoFlowDetected, !snap.flow_detected);
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
        self.eval_fault(SafetyFault::UvcInterlockOpen, !snap.uvc_interlock_closed);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SystemConfig;
    use crate::fsm::context::SensorSnapshot;

    fn make_supervisor() -> SafetySupervisor {
        SafetySupervisor::new(&SystemConfig::default())
    }

    fn nominal_snapshot() -> SensorSnapshot {
        SensorSnapshot {
            tank_a_ok: true,
            tank_b_ok: true,
            flow_detected: true,
            temperature_c: 25.0,
            over_temperature: false,
            uvc_interlock_closed: true,
            ..Default::default()
        }
    }

    #[test]
    fn no_faults_initially() {
        let s = make_supervisor();
        assert!(!s.has_faults());
        assert_eq!(s.faults(), 0);
    }

    #[test]
    fn over_temperature_sets_fault() {
        let mut s = make_supervisor();
        let mut snap = nominal_snapshot();
        snap.temperature_c = 85.0;
        s.evaluate(&snap);
        assert!(s.has_fault(SafetyFault::OverTemperature));
    }

    #[test]
    fn over_temperature_clears_when_cooled() {
        let mut s = make_supervisor();
        let mut snap = nominal_snapshot();
        snap.temperature_c = 85.0;
        s.evaluate(&snap);
        assert!(s.has_fault(SafetyFault::OverTemperature));

        snap.temperature_c = 50.0;
        s.evaluate(&snap);
        assert!(!s.has_fault(SafetyFault::OverTemperature));
    }

    #[test]
    fn water_level_low_fault() {
        let mut s = make_supervisor();
        let mut snap = nominal_snapshot();
        snap.tank_a_ok = false;
        s.evaluate(&snap);
        assert!(s.has_fault(SafetyFault::WaterLevelLow));
    }

    #[test]
    fn interlock_open_fault() {
        let mut s = make_supervisor();
        let mut snap = nominal_snapshot();
        snap.uvc_interlock_closed = false;
        s.evaluate(&snap);
        assert!(s.has_fault(SafetyFault::UvcInterlockOpen));
    }

    #[test]
    fn no_flow_fault_after_grace_period() {
        let mut s = make_supervisor();
        s.set_pump_commanded(true);
        let mut snap = nominal_snapshot();
        snap.flow_detected = false;

        // During grace period — no fault
        for _ in 0..3 {
            s.evaluate(&snap);
        }
        assert!(!s.has_fault(SafetyFault::NoFlowDetected));

        // After grace period — fault should fire
        for _ in 0..5 {
            s.evaluate(&snap);
        }
        assert!(s.has_fault(SafetyFault::NoFlowDetected));
    }

    #[test]
    fn no_flow_fault_clears_when_pump_off() {
        let mut s = make_supervisor();
        s.set_pump_commanded(true);
        let mut snap = nominal_snapshot();
        snap.flow_detected = false;

        for _ in 0..10 {
            s.evaluate(&snap);
        }
        assert!(s.has_fault(SafetyFault::NoFlowDetected));

        s.set_pump_commanded(false);
        s.evaluate(&snap);
        assert!(!s.has_fault(SafetyFault::NoFlowDetected));
    }

    #[test]
    fn multiple_simultaneous_faults() {
        let mut s = make_supervisor();
        let snap = SensorSnapshot {
            tank_a_ok: false,
            temperature_c: 100.0,
            uvc_interlock_closed: false,
            ..Default::default()
        };
        s.evaluate(&snap);
        assert!(s.has_fault(SafetyFault::WaterLevelLow));
        assert!(s.has_fault(SafetyFault::OverTemperature));
        assert!(s.has_fault(SafetyFault::UvcInterlockOpen));
    }

    #[test]
    fn individual_fault_clear_does_not_affect_others() {
        let mut s = make_supervisor();
        let snap = SensorSnapshot {
            tank_a_ok: false,
            temperature_c: 100.0,
            uvc_interlock_closed: true,
            ..Default::default()
        };
        s.evaluate(&snap);
        assert!(s.has_fault(SafetyFault::WaterLevelLow));
        assert!(s.has_fault(SafetyFault::OverTemperature));

        let snap2 = SensorSnapshot {
            tank_a_ok: true,
            temperature_c: 100.0,
            uvc_interlock_closed: true,
            ..Default::default()
        };
        s.evaluate(&snap2);
        assert!(!s.has_fault(SafetyFault::WaterLevelLow));
        assert!(s.has_fault(SafetyFault::OverTemperature));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::config::SystemConfig;
    use crate::fsm::context::SensorSnapshot;
    use proptest::prelude::*;

    fn arb_snapshot() -> impl Strategy<Value = SensorSnapshot> {
        (
            prop::bool::ANY, // tank_a_ok
            prop::bool::ANY, // flow_detected
            -40.0f32..150.0, // temperature_c
            prop::bool::ANY, // uvc_interlock_closed
        )
            .prop_map(|(tank_a, flow, temp, interlock)| SensorSnapshot {
                tank_a_ok: tank_a,
                tank_b_ok: true,
                flow_detected: flow,
                temperature_c: temp,
                over_temperature: temp > 80.0,
                uvc_interlock_closed: interlock,
                ..Default::default()
            })
    }

    proptest! {
        #[test]
        fn safety_invariants_hold(snapshots in proptest::collection::vec(arb_snapshot(), 1..50)) {
            let config = SystemConfig::default();
            let mut sup = SafetySupervisor::new(&config);
            sup.set_pump_commanded(true);

            for snap in &snapshots {
                let faults = sup.evaluate(snap);

                // Over-temp fault must be set iff temp exceeds max
                if snap.temperature_c > config.max_temperature_c {
                    prop_assert!(faults & SafetyFault::OverTemperature.mask() != 0);
                }

                // Water level fault must be set iff tank_a is not ok
                if !snap.tank_a_ok {
                    prop_assert!(faults & SafetyFault::WaterLevelLow.mask() != 0);
                }

                // Interlock fault must be set iff interlock is open
                if !snap.uvc_interlock_closed {
                    prop_assert!(faults & SafetyFault::UvcInterlockOpen.mask() != 0);
                }
            }
        }

        #[test]
        fn faults_always_clear_when_conditions_resolve(
            bad_temp in 81.0f32..150.0,
            good_temp in -40.0f32..79.0,
        ) {
            let config = SystemConfig::default();
            let mut sup = SafetySupervisor::new(&config);

            let bad_snap = SensorSnapshot {
                temperature_c: bad_temp,
                tank_a_ok: true,
                uvc_interlock_closed: true,
                ..Default::default()
            };
            sup.evaluate(&bad_snap);
            prop_assert!(sup.has_fault(SafetyFault::OverTemperature));

            let good_snap = SensorSnapshot {
                temperature_c: good_temp,
                tank_a_ok: true,
                uvc_interlock_closed: true,
                ..Default::default()
            };
            sup.evaluate(&good_snap);
            prop_assert!(!sup.has_fault(SafetyFault::OverTemperature));
        }
    }
}
