//! Application service — the hexagonal core.
//!
//! [`AppService`] owns the FSM, safety supervisor, and shared context.
//! It exposes a clean, hardware-agnostic API.  All I/O flows through
//! port traits injected at call sites, making the entire service
//! testable with mock adapters.
//!
//! ```text
//!  SensorPort ──▶ ┌────────────────────────┐ ──▶ EventSink
//!                 │      AppService         │
//! ActuatorPort ◀──│  FSM · Safety · PID     │
//!                 └────────────────────────┘
//! ```

use log::{info, warn};

use crate::config::SystemConfig;
use crate::control::pid::PidController;
use crate::fsm::context::FsmContext;
use crate::fsm::states::build_state_table;
use crate::fsm::{Fsm, StateId};
use crate::safety::SafetySupervisor;

use super::commands::AppCommand;
use super::events::{AppEvent, TelemetryData};
use super::ports::{ActuatorPort, EventSink, SensorPort};

// ───────────────────────────────────────────────────────────────
// AppService
// ───────────────────────────────────────────────────────────────

/// The application service orchestrates all domain logic.
pub struct AppService {
    fsm: Fsm,
    ctx: FsmContext,
    safety: SafetySupervisor,
    /// PID controller for closed-loop pump flow regulation.
    pid: PidController,
    /// Seconds per control tick (derived from config).
    tick_secs: f32,
    tick_count: u64,
    config_dirty: bool,
    dirty_since_tick: u64,
}

impl AppService {
    /// Construct the service from configuration.
    ///
    /// Does **not** start the FSM — call [`start`] or [`start_from`] next.
    pub fn new(config: SystemConfig) -> Self {
        let tick_secs = config.control_loop_interval_ms as f32 / 1000.0;
        let pump_flow = config.pump_flow_ml_per_min as f32;
        let safety = SafetySupervisor::new(&config);
        let ctx = FsmContext::new(config);
        let state_table = build_state_table();
        let fsm = Fsm::new(state_table, StateId::Idle);

        let mut pid = PidController::new(
            2.0, // Kp — proportional gain
            0.5, // Ki — integral gain (slow wind-up for steady-state)
            0.1, // Kd — derivative gain (dampen oscillation)
            pump_flow,
        );
        pid.set_limits(0.0, 100.0);

        Self {
            fsm,
            ctx,
            safety,
            pid,
            tick_secs,
            tick_count: 0,
            config_dirty: false,
            dirty_since_tick: 0,
        }
    }

    // ── Lifecycle ─────────────────────────────────────────────

    /// Start the FSM in its default initial state (Idle).
    pub fn start(&mut self, sink: &mut impl EventSink) {
        self.fsm.start(&mut self.ctx);
        sink.emit(&AppEvent::Started(self.fsm.current_state()));
        info!("AppService started in {:?}", self.fsm.current_state());
    }

    /// Start the FSM and immediately jump to `state` (e.g. after ULP wake).
    pub fn start_from(&mut self, state: StateId, sink: &mut impl EventSink) {
        self.fsm.start(&mut self.ctx);
        self.fsm.force_transition(state, &mut self.ctx);
        sink.emit(&AppEvent::Started(state));
        info!("AppService started from {:?}", state);
    }

    // ── Per-tick orchestration ────────────────────────────────

    /// Run one full control cycle: read sensors → safety → FSM → actuators.
    ///
    /// The `hw` parameter satisfies **both** [`SensorPort`] and
    /// [`ActuatorPort`] — this avoids a double mutable borrow while
    /// keeping the port boundary explicit.
    pub fn tick(&mut self, hw: &mut (impl SensorPort + ActuatorPort), sink: &mut impl EventSink) {
        self.tick_count += 1;
        let prev_state = self.fsm.current_state();

        // 1. Read sensors via SensorPort
        let snapshot = hw.read_all(self.tick_secs);
        self.ctx.sensors = snapshot;

        // 2. Safety evaluation
        self.safety
            .set_pump_commanded(self.ctx.commands.pump_duty > 0);
        let faults = self.safety.evaluate(&snapshot);
        self.ctx.fault_flags = faults;

        if faults != 0 && self.fsm.current_state() != StateId::Error {
            warn!("Safety fault! flags=0b{:08b}", faults);
            self.fsm.force_transition(StateId::Error, &mut self.ctx);
            sink.emit(&AppEvent::FaultDetected(faults));
        }

        // 3. FSM tick (pure state logic)
        self.fsm.tick(&mut self.ctx);

        // 4. PID flow regulation — modulate pump duty based on actual vs target flow
        if self.ctx.commands.pump_duty > 0 && snapshot.flow_detected {
            let pid_duty = self.pid.compute(snapshot.flow_ml_per_min, self.tick_secs);
            self.ctx.commands.pump_duty = (pid_duty as u8).clamp(10, 100);
        } else if self.ctx.commands.pump_duty > 0 {
            self.pid.reset();
        }

        // 5. Apply actuator commands via ActuatorPort
        self.apply_actuators(hw);

        // 6. Emit state change if the FSM moved
        let new_state = self.fsm.current_state();
        if new_state != prev_state {
            sink.emit(&AppEvent::StateChanged {
                from: prev_state,
                to: new_state,
            });
        }
    }

    // ── Command handling ──────────────────────────────────────

    /// Process an external command (from scheduler, BLE, serial, etc.).
    pub fn handle_command(
        &mut self,
        cmd: AppCommand,
        hw: &mut impl ActuatorPort,
        sink: &mut impl EventSink,
    ) {
        match cmd {
            AppCommand::StartScrub => {
                if self.fsm.current_state() == StateId::Idle {
                    let prev = self.fsm.current_state();
                    self.fsm.force_transition(StateId::Active, &mut self.ctx);
                    self.apply_actuators(hw);
                    sink.emit(&AppEvent::StateChanged {
                        from: prev,
                        to: StateId::Active,
                    });
                }
            }
            AppCommand::ForceState(target) => {
                let prev = self.fsm.current_state();
                self.fsm.force_transition(target, &mut self.ctx);
                self.apply_actuators(hw);
                sink.emit(&AppEvent::StateChanged {
                    from: prev,
                    to: target,
                });
            }
            AppCommand::UpdateConfig(new_config) => {
                self.mark_config_dirty();
                self.ctx.config = new_config;
                info!("Configuration updated at runtime");
            }
            AppCommand::SaveConfig => {
                self.dirty_since_tick = 0;
                self.mark_config_dirty();
                info!("Explicit config save requested (will flush on next auto-save check)");
            }
            AppCommand::GetDiagnostics | AppCommand::ClearDiagnostics => {
                // Handled by the RPC dispatch layer, not the FSM/service.
            }
        }
    }

    // ── Queries ───────────────────────────────────────────────

    /// Build a telemetry snapshot from the current context.
    /// `wifi_rssi`: WiFi signal strength in dBm when connected; None when not available (e.g. BLE-only).
    pub fn build_telemetry(&self, wifi_rssi: Option<i8>) -> TelemetryData {
        TelemetryData {
            state: self.fsm.current_state(),
            nh3_ppm: self.ctx.sensors.nh3_ppm,
            nh3_avg_ppm: self.ctx.sensors.nh3_avg_ppm,
            flow_ml_per_min: self.ctx.sensors.flow_ml_per_min,
            temperature_c: self.ctx.sensors.temperature_c,
            tank_a_ok: self.ctx.sensors.tank_a_ok,
            tank_b_ok: self.ctx.sensors.tank_b_ok,
            pump_duty: self.ctx.commands.pump_duty,
            uvc_duty: self.ctx.commands.uvc_duty,
            fault_flags: self.ctx.fault_flags,
            wifi_rssi,
        }
    }

    /// Current FSM state.
    pub fn state(&self) -> StateId {
        self.fsm.current_state()
    }

    /// Total control ticks executed since startup.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Current active fault bitmask (0 = no faults).
    pub fn fault_flags(&self) -> u8 {
        self.ctx.fault_flags
    }

    /// Clone of the live configuration (for RPC read-back or delta updates).
    pub fn current_config(&self) -> SystemConfig {
        self.ctx.config.clone()
    }

    // ── Internal ──────────────────────────────────────────────

    /// Translate FSM actuator commands into port calls.
    fn apply_actuators(&self, hw: &mut impl ActuatorPort) {
        let cmds = &self.ctx.commands;
        let snap = &self.ctx.sensors;

        // ── Pump ──────────────────────────────────────────────
        if cmds.pump_duty > 0 && !self.safety.has_faults() {
            hw.set_pump(cmds.pump_duty, cmds.pump_forward);
        } else {
            hw.stop_pump();
        }

        // ── UVC (double-gated: interlock + safety) ───────────
        if cmds.uvc_duty > 0 && snap.uvc_interlock_closed && !self.safety.has_faults() {
            hw.enable_uvc(cmds.uvc_duty);
        } else if hw.is_uvc_on() {
            if self.safety.has_faults() {
                hw.fault_shutdown_uvc("safety fault");
            } else if !snap.uvc_interlock_closed {
                hw.fault_shutdown_uvc("interlock open");
            } else {
                hw.disable_uvc();
            }
        }

        // ── Status LED ───────────────────────────────────────
        let (r, g, b) = cmds.led_rgb;
        hw.set_led(r, g, b);
    }

    // ── Config dirty-flag management ──────────────────────────

    /// Mark the config as modified. Called by `handle_command(UpdateConfig)`.
    pub fn mark_config_dirty(&mut self) {
        if !self.config_dirty {
            self.config_dirty = true;
            self.dirty_since_tick = self.tick_count;
        }
    }

    /// Check if auto-save should trigger (5 seconds after last change).
    /// Returns `true` if the config was saved.
    pub fn auto_save_if_needed(&mut self, storage: &impl super::ports::ConfigPort) -> bool {
        if !self.config_dirty {
            return false;
        }
        let ticks_since_dirty = self.tick_count.saturating_sub(self.dirty_since_tick);
        let secs_since_dirty = ticks_since_dirty as f32 * self.tick_secs;
        if secs_since_dirty < 5.0 {
            return false;
        }
        match storage.save(&self.ctx.config) {
            Ok(()) => {
                self.config_dirty = false;
                log::info!("Config auto-saved to NVS");
                true
            }
            Err(e) => {
                log::warn!("Config auto-save failed: {}", e);
                false
            }
        }
    }

    /// Force-save if dirty (call before deep sleep or error state).
    pub fn force_save_if_dirty(&mut self, storage: &impl super::ports::ConfigPort) {
        if !self.config_dirty {
            return;
        }
        match storage.save(&self.ctx.config) {
            Ok(()) => {
                self.config_dirty = false;
                log::info!("Config force-saved before shutdown");
            }
            Err(e) => {
                log::warn!("Config force-save failed: {}", e);
            }
        }
    }

    /// Whether the config has unsaved changes.
    pub fn is_config_dirty(&self) -> bool {
        self.config_dirty
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SystemConfig;

    #[test]
    fn build_telemetry_wifi_rssi_propagates() {
        let config = SystemConfig::default();
        let app = AppService::new(config);
        let t_none = app.build_telemetry(None);
        assert!(t_none.wifi_rssi.is_none());
        let t_some = app.build_telemetry(Some(-42));
        assert_eq!(t_some.wifi_rssi, Some(-42));
    }
}

