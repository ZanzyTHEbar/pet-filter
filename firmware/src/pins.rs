//! GPIO / peripheral pin assignments for the PetFilter main board.
//!
//! Single source of truth — every driver references this module rather than
//! hard-coding pin numbers.  Change a pin here and it propagates everywhere.
//!
//! Pin assignments match the schematic in `docs/engineering/system-architecture.md`.

// ---------------------------------------------------------------------------
// Pump motor driver (DRV8871 H-bridge)
// ---------------------------------------------------------------------------

/// LEDC PWM channel for pump speed control.
pub const PUMP_PWM_GPIO: i32 = 1;
/// Digital output: HIGH = forward, LOW = reverse.
pub const PUMP_DIR_GPIO: i32 = 2;

// ---------------------------------------------------------------------------
// UVC LED driver (PT4115 constant-current buck)
// ---------------------------------------------------------------------------

/// Digital output: enables the UVC LED driver (active HIGH).
/// Routed through a relay in series for hardware-level interlock.
pub const UVC_ENABLE_GPIO: i32 = 3;
/// LEDC PWM channel for UVC dimming (optional — can be tied HIGH).
pub const UVC_PWM_GPIO: i32 = 4;
/// Digital input: reed-switch interlock on UVC chamber lid.
/// LOW = lid closed (magnet present), HIGH = lid open.
pub const UVC_INTERLOCK_GPIO: i32 = 10;

// ---------------------------------------------------------------------------
// Sensors — Analog (ADC1)
// ---------------------------------------------------------------------------

/// MQ-137 NH3 gas sensor — analog voltage via resistive divider.
/// ADC1 channel 4 (GPIO 5 on ESP32-S3).
pub const NH3_ADC_GPIO: i32 = 5;
/// ADC attenuation for NH3 sensor (11 dB → 0 – 3.1 V range).
pub const NH3_ADC_ATTEN: u32 = 3; // esp_idf_hal::adc::attenuation::DB_11

/// NTC thermistor — 10 kΩ @ 25 °C, voltage-divider to ADC.
/// ADC1 channel 8 (GPIO 9 on ESP32-S3).
pub const TEMP_ADC_GPIO: i32 = 9;

// ---------------------------------------------------------------------------
// Sensors — Digital / Pulse
// ---------------------------------------------------------------------------

/// YF-S201 hall-effect flow sensor — pulse output, interrupt-driven.
pub const FLOW_PULSE_GPIO: i32 = 6;

/// Capacitive water-level sensor — Tank A (supply). HIGH = water present.
pub const WATER_LEVEL_A_GPIO: i32 = 7;
/// Capacitive water-level sensor — Tank B (collection). HIGH = water present.
pub const WATER_LEVEL_B_GPIO: i32 = 8;

// ---------------------------------------------------------------------------
// Status LED (WS2812B or discrete RGB)
// ---------------------------------------------------------------------------

pub const LED_R_GPIO: i32 = 11;
pub const LED_G_GPIO: i32 = 12;
pub const LED_B_GPIO: i32 = 13;

// ---------------------------------------------------------------------------
// I²C bus (future expansion: SGP30, OLED, etc.)
// ---------------------------------------------------------------------------

pub const I2C_SDA_GPIO: i32 = 14;
pub const I2C_SCL_GPIO: i32 = 15;

// ---------------------------------------------------------------------------
// UART debug
// ---------------------------------------------------------------------------

pub const UART_TX_GPIO: i32 = 17;
pub const UART_RX_GPIO: i32 = 18;

// ---------------------------------------------------------------------------
// PWM configuration
// ---------------------------------------------------------------------------

/// LEDC timer resolution (bits).  8-bit gives 0 – 255 duty levels.
pub const PWM_RESOLUTION_BITS: u32 = 8;
/// LEDC base frequency for pump motor (25 kHz — inaudible).
pub const PUMP_PWM_FREQ_HZ: u32 = 25_000;
/// LEDC base frequency for UVC LED driver (1 kHz — driver-compatible).
pub const UVC_PWM_FREQ_HZ: u32 = 1_000;
/// LEDC frequency for RGB status LED (1 kHz).
pub const LED_PWM_FREQ_HZ: u32 = 1_000;

// ---------------------------------------------------------------------------
// User button (active-low with external pull-up)
// ---------------------------------------------------------------------------

/// Momentary push-button for manual control.
pub const BUTTON_GPIO: i32 = 16;
