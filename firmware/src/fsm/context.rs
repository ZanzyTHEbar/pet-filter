//! Shared mutable context threaded through every FSM handler.
//!
//! `FsmContext` is the single struct that state handlers read from and
//! write to.  It contains the latest sensor snapshot, actuator command
//! outputs, timing information, configuration, and accumulated safety
//! faults.  Think of it as the "blackboard" in a blackboard architecture.

use crate::config::SystemConfig;

// ---------------------------------------------------------------------------
// Sensor snapshot (read-only to state handlers; written by sensor hub)
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of every sensor in the system.
#[derive(Debug, Clone, Copy, Default)]
pub struct SensorSnapshot {
    /// Instantaneous NH3 concentration (ppm).
    pub nh3_ppm: f32,
    /// Running-average NH3 concentration (ppm) over the confirmation window.
    pub nh3_avg_ppm: f32,
    /// Raw ADC value from the NH3 sensor (0 – 4095).
    pub nh3_raw: u16,

    /// Water flow rate through the venturi (mL/min).
    pub flow_ml_per_min: f32,
    /// True if flow rate is above the minimum detection threshold.
    pub flow_detected: bool,

    /// Tank A (supply) has adequate water.
    pub tank_a_ok: bool,
    /// Tank B (collection) has adequate water.
    pub tank_b_ok: bool,

    /// UVC heatsink / system temperature (°C).
    pub temperature_c: f32,
    /// True if temperature exceeds the configured maximum.
    pub over_temperature: bool,

    /// UVC chamber interlock: true = lid closed (safe to operate).
    pub uvc_interlock_closed: bool,
}

// ---------------------------------------------------------------------------
// Actuator commands (written by state handlers; consumed by main loop)
// ---------------------------------------------------------------------------

/// Commands that state handlers write to request actuator actions.
/// The main loop applies these to the actual drivers each tick.
#[derive(Debug, Clone, Copy)]
pub struct ActuatorCommands {
    /// Desired pump duty cycle (0 = off, 1–100 = on).
    pub pump_duty: u8,
    /// Desired pump direction: `true` = forward, `false` = reverse.
    pub pump_forward: bool,
    /// Desired UVC LED duty cycle (0 = off, 1–100 = on).
    pub uvc_duty: u8,
    /// Status LED colour (R, G, B) — each 0–255.
    pub led_rgb: (u8, u8, u8),
}

impl Default for ActuatorCommands {
    fn default() -> Self {
        Self {
            pump_duty: 0,
            pump_forward: true,
            uvc_duty: 0,
            led_rgb: (0, 0, 0), // off
        }
    }
}

impl ActuatorCommands {
    /// All actuators off — safe default.
    pub fn all_off() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// FsmContext
// ---------------------------------------------------------------------------

/// The shared context passed to every state handler function.
pub struct FsmContext {
    // -- Timing --
    /// Ticks elapsed since the current state was entered.
    pub ticks_in_state: u64,
    /// Monotonic total tick count.
    pub total_ticks: u64,
    /// Duration of one tick in seconds (inverse of control loop frequency).
    pub tick_period_secs: f32,

    // -- Sensor data --
    /// Latest sensor readings.  Updated before each FSM tick.
    pub sensors: SensorSnapshot,

    // -- Actuator outputs --
    /// Commands to be applied to actuators after the FSM tick.
    pub commands: ActuatorCommands,

    // -- Configuration --
    /// System configuration (tunable parameters).
    pub config: SystemConfig,

    // -- Safety --
    /// Accumulated safety fault bitmask (see `SafetyFault::mask()`).
    /// Set by the safety supervisor, read by state handlers.
    pub fault_flags: u8,
}

impl FsmContext {
    /// Create a new context with the given configuration.
    pub fn new(config: SystemConfig) -> Self {
        Self {
            ticks_in_state: 0,
            total_ticks: 0,
            tick_period_secs: config.control_loop_interval_ms as f32 / 1000.0,
            sensors: SensorSnapshot::default(),
            commands: ActuatorCommands::all_off(),
            config,
            fault_flags: 0,
        }
    }

    /// Seconds elapsed since the current state was entered.
    pub fn secs_in_state(&self) -> f32 {
        self.ticks_in_state as f32 * self.tick_period_secs
    }

    /// Returns `true` if **any** safety fault is active.
    pub fn has_faults(&self) -> bool {
        self.fault_flags != 0
    }

    /// Check whether a specific fault flag is set.
    pub fn has_fault(&self, fault: crate::error::SafetyFault) -> bool {
        self.fault_flags & fault.mask() != 0
    }
}
