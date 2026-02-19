//! Port traits — the hexagonal boundary between domain logic and the outside world.
//!
//! ```text
//!   Adapter ──▶ Port trait ──▶ AppService (domain)
//! ```
//!
//! Driven adapters (sensors, actuators, event sinks, storage) implement these
//! traits.  The [`AppService`](super::service::AppService) consumes them via
//! generics, so the domain core never touches hardware directly.
//!
//! ## Security notes
//!
//! - **ConfigPort** implementations MUST validate before persisting.
//! - **StoragePort** implementations SHOULD encrypt sensitive keys.
//! - All port errors are typed — callers must handle every variant explicitly.

use crate::config::SystemConfig;
use crate::fsm::context::SensorSnapshot;

// ───────────────────────────────────────────────────────────────
// Sensor port (driven adapter: hardware → domain)
// ───────────────────────────────────────────────────────────────

/// Read-side port: the domain calls this to obtain sensor data.
pub trait SensorPort {
    /// Read every sensor and return a unified snapshot.
    fn read_all(&mut self, elapsed_secs: f32) -> SensorSnapshot;

    /// Fast ammonia-only read for high-frequency sampling paths.
    fn read_ammonia_fast(&mut self) -> f32;
}

// ───────────────────────────────────────────────────────────────
// Actuator port (driven adapter: domain → hardware)
// ───────────────────────────────────────────────────────────────

/// Write-side port: the domain calls this to command actuators.
pub trait ActuatorPort {
    /// Set pump duty (0–100) and direction (true = forward).
    fn set_pump(&mut self, duty: u8, forward: bool);

    /// Immediately stop the pump.
    fn stop_pump(&mut self);

    /// Enable UVC LEDs at the given duty (0–100).
    fn enable_uvc(&mut self, duty: u8);

    /// Disable UVC LEDs.
    fn disable_uvc(&mut self);

    /// Emergency UVC shutdown with reason tag.
    fn fault_shutdown_uvc(&mut self, reason: &'static str);

    /// Query whether the UVC subsystem is currently energised.
    fn is_uvc_on(&self) -> bool;

    /// Set the RGB status LED colour.
    fn set_led(&mut self, r: u8, g: u8, b: u8);

    /// Kill all actuators (pump, UVC, LED) — safe shutdown.
    fn all_off(&mut self);
}

// ───────────────────────────────────────────────────────────────
// Event sink port (driven adapter: domain → logging / telemetry)
// ───────────────────────────────────────────────────────────────

/// The domain emits structured [`AppEvent`](super::events::AppEvent)s
/// through this port.  Adapters decide where they go (serial log, MQTT,
/// BLE characteristic, etc.).
pub trait EventSink {
    fn emit(&mut self, event: &super::events::AppEvent);
}

// ───────────────────────────────────────────────────────────────
// Configuration port (driven adapter: domain ↔ persistent config)
// ───────────────────────────────────────────────────────────────

/// Loads and persists system configuration.
///
/// # Security
///
/// Implementations MUST validate config values before persisting.
/// Invalid ranges should be rejected with [`ConfigError::ValidationFailed`],
/// not silently clamped.  This prevents a compromised BLE/RPC channel
/// from injecting dangerous operating parameters (e.g., disabling thermal
/// shutdown by setting `max_temperature_c` to `f32::MAX`).
pub trait ConfigPort {
    /// Load configuration from persistent storage.
    /// Returns [`SystemConfig::default()`] if no stored config exists.
    fn load(&self) -> Result<SystemConfig, ConfigError>;

    /// Validate and persist configuration.
    /// Rejects invalid values (e.g., negative thresholds, duty > 100).
    fn save(&self, config: &SystemConfig) -> Result<(), ConfigError>;
}

// ───────────────────────────────────────────────────────────────
// Storage port (driven adapter: domain ↔ NVS / flash)
// ───────────────────────────────────────────────────────────────

/// Persistent key-value storage for NVS, credentials, crash logs, etc.
///
/// # Security
///
/// - Implementations SHOULD encrypt sensitive keys (WiFi passwords, tokens).
///   On ESP32, prefer the encrypted NVS partition for these.
/// - Keys are namespaced to prevent collisions between subsystems.
/// - Write operations MUST be atomic — no partial writes on power loss.
///   The ESP-IDF NVS API guarantees this natively; in-memory simulation
///   achieves it trivially.
/// - Callers MUST NOT store secrets in plaintext namespaces.
pub trait StoragePort {
    /// Read a value.  Returns the number of bytes written to `buf`.
    fn read(&self, namespace: &str, key: &str, buf: &mut [u8]) -> Result<usize, StorageError>;

    /// Write a value atomically.
    fn write(&mut self, namespace: &str, key: &str, data: &[u8]) -> Result<(), StorageError>;

    /// Delete a key.  Returns `Ok(())` even if the key didn't exist.
    fn delete(&mut self, namespace: &str, key: &str) -> Result<(), StorageError>;

    /// Check whether a key exists without reading it.
    fn exists(&self, namespace: &str, key: &str) -> bool;
}

// ───────────────────────────────────────────────────────────────
// Scheduler delegate (decouples scheduler from event system)
// ───────────────────────────────────────────────────────────────

/// Callback trait that the scheduler invokes when a schedule fires.
///
/// This decouples the [`Scheduler`](crate::scheduler::Scheduler) from the
/// ISR event queue.  The main loop implements this by forwarding to
/// [`push_event`](crate::events::push_event), but the scheduler itself
/// knows nothing about events, queues, or ISRs.
pub trait SchedulerDelegate {
    /// Called when a schedule fires.
    ///
    /// * `label` — the human-readable label of the schedule that fired.
    /// * `kind`  — whether it was a periodic, boost, or one-shot fire.
    fn on_schedule_fired(&mut self, label: &str, kind: ScheduleFiredKind);
}

/// Discriminant passed to [`SchedulerDelegate::on_schedule_fired`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleFiredKind {
    /// A recurring periodic schedule fired.
    Periodic,
    /// A boost-mode schedule started or is running.
    Boost,
    /// A one-shot schedule fired (auto-disables after).
    OneShot,
}

// ───────────────────────────────────────────────────────────────
// Error types
// ───────────────────────────────────────────────────────────────

/// Errors from [`ConfigPort`] operations.
#[derive(Debug)]
pub enum ConfigError {
    /// No config found in storage (first boot).
    NotFound,
    /// Stored config failed integrity / deserialization check.
    Corrupted,
    /// A config field failed range validation.
    /// The `&'static str` describes which field and why.
    ValidationFailed(&'static str),
    /// Underlying storage is full.
    StorageFull,
    /// Generic I/O error from the storage backend.
    IoError,
}

/// Errors from [`StoragePort`] operations.
#[derive(Debug)]
pub enum StorageError {
    /// Requested key does not exist.
    NotFound,
    /// Storage partition is full.
    Full,
    /// Generic I/O error.
    IoError,
    /// Encryption or decryption failed (wrong key, corrupted blob).
    EncryptionError,
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotFound => write!(f, "config not found"),
            Self::Corrupted => write!(f, "config corrupted"),
            Self::ValidationFailed(msg) => write!(f, "validation failed: {}", msg),
            Self::StorageFull => write!(f, "storage full"),
            Self::IoError => write!(f, "I/O error"),
        }
    }
}

impl core::fmt::Display for StorageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotFound => write!(f, "key not found"),
            Self::Full => write!(f, "storage full"),
            Self::IoError => write!(f, "I/O error"),
            Self::EncryptionError => write!(f, "encryption error"),
        }
    }
}
