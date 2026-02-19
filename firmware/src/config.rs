//! System configuration parameters
//!
//! All tunable parameters for the PetFilter system.
//! Values can be overridden via NVS (non-volatile storage) or BLE provisioning.

use serde::{Deserialize, Serialize};

/// Core system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    // --- Pump ---
    /// Target pump flow rate in mL/min
    pub pump_flow_ml_per_min: u16,
    /// Pump PWM duty cycle (0-100%)
    pub pump_duty_percent: u8,

    // --- NH3 Thresholds ---
    /// NH3 concentration (ppm) to trigger activation
    pub nh3_activate_threshold_ppm: f32,
    /// NH3 concentration (ppm) to trigger deactivation
    pub nh3_deactivate_threshold_ppm: f32,
    /// Sensing confirmation period (seconds)
    pub nh3_confirm_duration_secs: u16,

    // --- UVC ---
    /// UVC LED PWM duty cycle (0-100%)
    pub uvc_duty_percent: u8,

    // --- Safety ---
    /// Maximum allowed temperature (Celsius) before thermal shutdown
    pub max_temperature_c: f32,
    /// Purge duration after scrubbing (seconds)
    pub purge_duration_secs: u16,
    /// Minimum water level for operation (0-100%)
    pub min_water_level_percent: u8,

    // --- Timing ---
    /// Sensor read interval (milliseconds)
    pub sensor_read_interval_ms: u32,
    /// Control loop interval (milliseconds)
    pub control_loop_interval_ms: u32,
    /// Telemetry report interval (seconds)
    pub telemetry_interval_secs: u32,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            // Pump
            pump_flow_ml_per_min: 1000,  // 1 L/min
            pump_duty_percent: 70,

            // NH3 thresholds
            nh3_activate_threshold_ppm: 10.0,
            nh3_deactivate_threshold_ppm: 5.0,
            nh3_confirm_duration_secs: 30,

            // UVC
            uvc_duty_percent: 100,

            // Safety
            max_temperature_c: 80.0,
            purge_duration_secs: 120,
            min_water_level_percent: 20,

            // Timing
            sensor_read_interval_ms: 100,    // 10 Hz
            control_loop_interval_ms: 1000,  // 1 Hz
            telemetry_interval_secs: 60,     // 1/min
        }
    }
}
