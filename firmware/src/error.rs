#![allow(dead_code)] // Error types reserved for future SensorPort/ActuatorPort typed returns

//! Unified error types for the PetFilter firmware.
//!
//! Follows embedded best practice: a single `Error` enum that every subsystem
//! can convert into, keeping the top-level control loop's error handling uniform.
//! All variants are `Copy` so they can be cheaply passed through the safety
//! supervisor and FSM without allocation.

use core::fmt;

// ---------------------------------------------------------------------------
// Top-level firmware error
// ---------------------------------------------------------------------------

/// Every fallible operation in the firmware funnels into this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A sensor could not be read or returned out-of-range data.
    Sensor(SensorError),
    /// An actuator command failed.
    Actuator(ActuatorError),
    /// A safety interlock was violated.
    Safety(SafetyFault),
    /// A communication subsystem failed.
    Comms(CommsError),
    /// Peripheral initialisation failed.
    Init(&'static str),
    /// Configuration is invalid or could not be loaded.
    Config(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sensor(e) => write!(f, "sensor: {e}"),
            Self::Actuator(e) => write!(f, "actuator: {e}"),
            Self::Safety(e) => write!(f, "safety: {e}"),
            Self::Comms(e) => write!(f, "comms: {e}"),
            Self::Init(msg) => write!(f, "init: {msg}"),
            Self::Config(msg) => write!(f, "config: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Sensor errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorError {
    /// ADC read returned an error or timed out.
    AdcReadFailed,
    /// GPIO read returned an error.
    GpioReadFailed,
    /// Reading is outside the physically plausible range.
    OutOfRange,
    /// Sensor requires warm-up time before readings are valid.
    WarmingUp,
}

impl fmt::Display for SensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdcReadFailed => write!(f, "ADC read failed"),
            Self::GpioReadFailed => write!(f, "GPIO read failed"),
            Self::OutOfRange => write!(f, "reading out of range"),
            Self::WarmingUp => write!(f, "sensor warming up"),
        }
    }
}

impl From<SensorError> for Error {
    fn from(e: SensorError) -> Self {
        Self::Sensor(e)
    }
}

// ---------------------------------------------------------------------------
// Actuator errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActuatorError {
    /// PWM duty-cycle write failed.
    PwmWriteFailed,
    /// GPIO set failed.
    GpioWriteFailed,
    /// Interlock prevents the actuator from operating.
    InterlockOpen,
    /// Actuator thermal limit reached.
    OverTemperature,
}

impl fmt::Display for ActuatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PwmWriteFailed => write!(f, "PWM write failed"),
            Self::GpioWriteFailed => write!(f, "GPIO write failed"),
            Self::InterlockOpen => write!(f, "interlock open"),
            Self::OverTemperature => write!(f, "over temperature"),
        }
    }
}

impl From<ActuatorError> for Error {
    fn from(e: ActuatorError) -> Self {
        Self::Actuator(e)
    }
}

// ---------------------------------------------------------------------------
// Safety faults
// ---------------------------------------------------------------------------

/// Safety faults are a special category: they trigger an immediate transition
/// to the ERROR state and disable all actuators.  They are accumulated in a
/// bitfield by the safety supervisor so that multiple simultaneous faults can
/// be tracked and individually cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SafetyFault {
    /// Tank A (supply) water level below minimum.
    WaterLevelLow = 0b0000_0001,
    /// Pump commanded on but flow sensor reads zero.
    NoFlowDetected = 0b0000_0010,
    /// UVC heatsink temperature exceeds limit.
    OverTemperature = 0b0000_0100,
    /// UVC chamber lid/cover is open.
    UvcInterlockOpen = 0b0000_1000,
}

impl SafetyFault {
    /// Return the bitmask for this fault.
    pub const fn mask(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for SafetyFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WaterLevelLow => write!(f, "water level low"),
            Self::NoFlowDetected => write!(f, "no flow detected"),
            Self::OverTemperature => write!(f, "over temperature"),
            Self::UvcInterlockOpen => write!(f, "UVC interlock open"),
        }
    }
}

impl From<SafetyFault> for Error {
    fn from(e: SafetyFault) -> Self {
        Self::Safety(e)
    }
}

// ---------------------------------------------------------------------------
// Communications errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommsError {
    WifiConnectFailed,
    WifiDisconnected,
    MqttPublishFailed,
    BleInitFailed,
    OtaFailed,
}

impl fmt::Display for CommsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WifiConnectFailed => write!(f, "WiFi connect failed"),
            Self::WifiDisconnected => write!(f, "WiFi disconnected"),
            Self::MqttPublishFailed => write!(f, "MQTT publish failed"),
            Self::BleInitFailed => write!(f, "BLE init failed"),
            Self::OtaFailed => write!(f, "OTA update failed"),
        }
    }
}

impl From<CommsError> for Error {
    fn from(e: CommsError) -> Self {
        Self::Comms(e)
    }
}

// ---------------------------------------------------------------------------
// Convenience Result alias
// ---------------------------------------------------------------------------

/// Firmware-wide `Result` alias.
pub type Result<T> = core::result::Result<T, Error>;
