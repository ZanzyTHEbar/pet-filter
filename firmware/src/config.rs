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
            pump_flow_ml_per_min: 1000, // 1 L/min
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
            sensor_read_interval_ms: 100,   // 10 Hz
            control_loop_interval_ms: 1000, // 1 Hz
            telemetry_interval_secs: 60,    // 1/min
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_sane() {
        let c = SystemConfig::default();
        assert!(c.nh3_activate_threshold_ppm > c.nh3_deactivate_threshold_ppm);
        assert!(c.pump_duty_percent > 0 && c.pump_duty_percent <= 100);
        assert!(c.uvc_duty_percent > 0 && c.uvc_duty_percent <= 100);
        assert!(c.max_temperature_c > 0.0);
        assert!(c.purge_duration_secs > 0);
        assert!(c.control_loop_interval_ms > 0);
        assert!(c.sensor_read_interval_ms > 0);
    }

    #[test]
    fn serde_roundtrip() {
        let c = SystemConfig::default();
        let json = serde_json::to_string(&c).unwrap();
        let c2: SystemConfig = serde_json::from_str(&json).unwrap();
        assert!((c.nh3_activate_threshold_ppm - c2.nh3_activate_threshold_ppm).abs() < 0.001);
        assert_eq!(c.pump_duty_percent, c2.pump_duty_percent);
        assert_eq!(c.purge_duration_secs, c2.purge_duration_secs);
    }

    #[test]
    fn activate_above_deactivate_invariant() {
        let c = SystemConfig::default();
        assert!(
            c.nh3_activate_threshold_ppm > c.nh3_deactivate_threshold_ppm,
            "activate threshold must be above deactivate to prevent oscillation"
        );
    }

    #[test]
    fn timing_ratios_make_sense() {
        let c = SystemConfig::default();
        assert!(
            c.sensor_read_interval_ms < c.control_loop_interval_ms,
            "sensor reads should be faster than control loop"
        );
        assert!(
            c.control_loop_interval_ms < c.telemetry_interval_secs * 1000,
            "control loop should be faster than telemetry"
        );
    }

    #[test]
    fn postcard_roundtrip() {
        let c = SystemConfig::default();
        let bytes = postcard::to_allocvec(&c).unwrap();
        let c2: SystemConfig = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(c.pump_duty_percent, c2.pump_duty_percent);
        assert!((c.max_temperature_c - c2.max_temperature_c).abs() < 0.001);
    }
}
