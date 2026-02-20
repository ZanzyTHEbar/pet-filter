//! Secure OTA firmware update via RPC — backed by `esp-ota` crate.
//!
//! Flow: OtaBegin → N × OtaChunk → OtaFinalize → reboot
//!
//! Session must be Authenticated before OtaBegin is accepted.
//!
//! The `esp-ota` crate provides a safe Rust wrapper around the ESP-IDF
//! OTA partition API, eliminating all unsafe FFI in this module.

use core::fmt;
use log::{info, warn};

const MAX_FIRMWARE_SIZE: u32 = 4 * 1024 * 1024; // 4 MB

// ── Error type ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtaError {
    AlreadyInProgress,
    InvalidSize,
    InvalidSha,
    NoPartition,
    BeginFailed,
    WriteFailed,
    VerifyFailed,
    BootSetFailed,
    IncompleteTransfer,
    NotReceiving,
    NonSequential,
    Overflow,
}

impl fmt::Display for OtaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyInProgress => write!(f, "OTA session already in progress"),
            Self::InvalidSize => write!(f, "firmware size out of range (max 4 MB)"),
            Self::InvalidSha => write!(f, "SHA-256 must be exactly 32 bytes"),
            Self::NoPartition => write!(f, "no inactive OTA partition available"),
            Self::BeginFailed => write!(f, "OTA begin failed"),
            Self::WriteFailed => write!(f, "OTA write failed"),
            Self::VerifyFailed => write!(f, "OTA verification failed"),
            Self::BootSetFailed => write!(f, "set boot partition failed"),
            Self::IncompleteTransfer => write!(f, "finalize called before all bytes written"),
            Self::NotReceiving => write!(f, "operation requires active Receiving state"),
            Self::NonSequential => write!(f, "chunk offset does not match expected offset"),
            Self::Overflow => write!(f, "chunk would exceed declared firmware size"),
        }
    }
}

// ── State machine ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtaState {
    Idle,
    Receiving {
        expected_size: u32,
        bytes_written: u32,
    },
    WritePending {
        expected_size: u32,
        bytes_written: u32,
        pending_offset: u32,
        pending_len: u32,
    },
    Verifying,
    ReadyToReboot,
    Failed,
}

// ── Manager (ESP-IDF: uses esp-ota crate) ─────────────────────

/// OTA firmware update manager.
///
/// On ESP-IDF targets, uses the `esp-ota` crate for safe partition
/// management. On simulation targets, uses in-memory stubs.
/// Staging buffer for deferred OTA writes (avoids blocking control loop).
const OTA_STAGING_BUF_SIZE: usize = 4096;

pub struct OtaManager {
    state: OtaState,
    expected_sha256: [u8; 32],
    staging_buf: [u8; OTA_STAGING_BUF_SIZE],
    #[cfg(target_os = "espidf")]
    ota_update: Option<esp_ota::OtaUpdate>,
}

impl OtaManager {
    pub fn new() -> Self {
        Self {
            state: OtaState::Idle,
            expected_sha256: [0u8; 32],
            staging_buf: [0u8; OTA_STAGING_BUF_SIZE],
            #[cfg(target_os = "espidf")]
            ota_update: None,
        }
    }

    pub fn state(&self) -> OtaState {
        self.state
    }

    /// Begin an OTA session. Validates size and opens the inactive partition.
    pub fn begin(&mut self, firmware_size: u32, sha256: &[u8]) -> Result<(), OtaError> {
        if self.state != OtaState::Idle {
            return Err(OtaError::AlreadyInProgress);
        }
        if firmware_size == 0 || firmware_size > MAX_FIRMWARE_SIZE {
            return Err(OtaError::InvalidSize);
        }
        if sha256.len() != 32 {
            return Err(OtaError::InvalidSha);
        }

        self.expected_sha256.copy_from_slice(sha256);

        #[cfg(target_os = "espidf")]
        {
            let update = esp_ota::OtaUpdate::begin().map_err(|e| {
                warn!("esp-ota begin failed: {:?}", e);
                OtaError::BeginFailed
            })?;
            self.ota_update = Some(update);
        }

        self.state = OtaState::Receiving {
            expected_size: firmware_size,
            bytes_written: 0,
        };
        info!("OTA: begin ({} bytes)", firmware_size);
        Ok(())
    }

    /// Write a chunk at the given byte offset. Returns total bytes written.
    pub fn write_chunk(&mut self, offset: u32, data: &[u8]) -> Result<u32, OtaError> {
        let OtaState::Receiving {
            expected_size,
            bytes_written,
        } = self.state
        else {
            return Err(OtaError::NotReceiving);
        };

        if offset != bytes_written {
            return Err(OtaError::NonSequential);
        }
        if bytes_written + data.len() as u32 > expected_size {
            return Err(OtaError::Overflow);
        }

        #[cfg(target_os = "espidf")]
        {
            if let Some(ref mut update) = self.ota_update {
                update.write(data).map_err(|e| {
                    warn!("esp-ota write failed: {:?}", e);
                    self.abort();
                    OtaError::WriteFailed
                })?;
            } else {
                return Err(OtaError::NotReceiving);
            }
        }

        let new_written = bytes_written + data.len() as u32;
        self.state = OtaState::Receiving {
            expected_size,
            bytes_written: new_written,
        };
        Ok(new_written)
    }

    /// Finalize: verify image, mark partition bootable, set ReadyToReboot.
    pub fn finalize(&mut self) -> Result<(), OtaError> {
        match self.state {
            OtaState::Receiving {
                expected_size,
                bytes_written,
            } if bytes_written == expected_size => {}
            OtaState::Receiving { .. } => return Err(OtaError::IncompleteTransfer),
            _ => return Err(OtaError::NotReceiving),
        }

        self.state = OtaState::Verifying;

        #[cfg(target_os = "espidf")]
        {
            if let Some(update) = self.ota_update.take() {
                let mut completed = update.finalize().map_err(|e| {
                    warn!("esp-ota finalize failed: {:?}", e);
                    self.state = OtaState::Failed;
                    OtaError::VerifyFailed
                })?;

                completed.set_as_boot_partition().map_err(|e| {
                    warn!("esp-ota set_as_boot_partition failed: {:?}", e);
                    self.state = OtaState::Failed;
                    OtaError::BootSetFailed
                })?;
            } else {
                self.state = OtaState::Failed;
                return Err(OtaError::NotReceiving);
            }
        }

        self.state = OtaState::ReadyToReboot;
        info!("OTA: finalized, ready to reboot");
        Ok(())
    }

    /// Queue a chunk for deferred writing. The control loop calls
    /// `flush_pending()` on the next tick to perform the actual flash write
    /// outside the RPC dispatch path.
    pub fn queue_chunk(&mut self, offset: u32, data: &[u8]) -> Result<u32, OtaError> {
        let OtaState::Receiving {
            expected_size,
            bytes_written,
        } = self.state
        else {
            return Err(OtaError::NotReceiving);
        };

        if offset != bytes_written {
            return Err(OtaError::NonSequential);
        }
        if bytes_written + data.len() as u32 > expected_size {
            return Err(OtaError::Overflow);
        }
        if data.len() > OTA_STAGING_BUF_SIZE {
            return Err(OtaError::Overflow);
        }

        self.staging_buf[..data.len()].copy_from_slice(data);
        self.state = OtaState::WritePending {
            expected_size,
            bytes_written,
            pending_offset: offset,
            pending_len: data.len() as u32,
        };
        Ok(bytes_written + data.len() as u32)
    }

    /// Perform the deferred flash write. Returns `true` if a write was
    /// flushed, `false` if nothing was pending.
    pub fn flush_pending(&mut self) -> Result<bool, OtaError> {
        let OtaState::WritePending {
            expected_size,
            bytes_written,
            pending_len,
            ..
        } = self.state
        else {
            return Ok(false);
        };

        #[cfg(target_os = "espidf")]
        {
            if let Some(ref mut update) = self.ota_update {
                update.write(data).map_err(|e| {
                    warn!("esp-ota deferred write failed: {:?}", e);
                    self.abort();
                    OtaError::WriteFailed
                })?;
            } else {
                return Err(OtaError::NotReceiving);
            }
        }

        let new_written = bytes_written + pending_len;
        self.state = OtaState::Receiving {
            expected_size,
            bytes_written: new_written,
        };
        Ok(true)
    }

    /// Returns true if there is a pending write that needs flushing.
    pub fn has_pending(&self) -> bool {
        matches!(self.state, OtaState::WritePending { .. })
    }

    /// Abort the current OTA session; resets to Idle.
    pub fn abort(&mut self) {
        #[cfg(target_os = "espidf")]
        {
            // esp-ota aborts automatically when OtaUpdate is dropped
            self.ota_update.take();
        }
        self.state = OtaState::Idle;
        warn!("OTA: aborted");
    }

    /// Soft-reset into the newly flashed firmware.
    #[cfg(target_os = "espidf")]
    pub fn reboot(&self) -> ! {
        info!("OTA: rebooting into new firmware");
        esp_ota::restart();
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn reboot(&self) -> ! {
        panic!("OTA reboot (simulation — no real hardware reset)");
    }
}

impl Default for OtaManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Boot validation ───────────────────────────────────────────

/// Check OTA image state on startup and mark this firmware as valid.
///
/// Without this, the rollback watchdog reverts to the previous firmware
/// after three consecutive failed boots.
#[cfg(target_os = "espidf")]
pub fn check_rollback() {
    match esp_ota::mark_app_valid() {
        Ok(()) => info!("OTA: firmware marked valid (rollback cancelled)"),
        Err(e) => warn!("OTA: mark_app_valid failed: {:?}", e),
    }
}

#[cfg(not(target_os = "espidf"))]
pub fn check_rollback() {
    log::info!("OTA rollback check (simulation): skipped");
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sha() -> [u8; 32] {
        [0u8; 32]
    }

    #[test]
    fn begin_requires_idle_state() {
        let mut ota = OtaManager::new();
        assert!(ota.begin(1024, &sha()).is_ok());
        assert_eq!(ota.begin(1024, &sha()), Err(OtaError::AlreadyInProgress));
    }

    #[test]
    fn begin_rejects_zero_size() {
        let mut ota = OtaManager::new();
        assert_eq!(ota.begin(0, &sha()), Err(OtaError::InvalidSize));
    }

    #[test]
    fn begin_rejects_oversized() {
        let mut ota = OtaManager::new();
        assert_eq!(
            ota.begin(5 * 1024 * 1024, &sha()),
            Err(OtaError::InvalidSize)
        );
    }

    #[test]
    fn begin_rejects_short_sha() {
        let mut ota = OtaManager::new();
        assert_eq!(ota.begin(1024, &[0u8; 16]), Err(OtaError::InvalidSha));
    }

    #[test]
    fn chunk_requires_receiving_state() {
        let mut ota = OtaManager::new();
        assert_eq!(ota.write_chunk(0, b"data"), Err(OtaError::NotReceiving));
    }

    #[test]
    fn chunk_rejects_non_sequential() {
        let mut ota = OtaManager::new();
        ota.begin(1024, &sha()).unwrap();
        assert_eq!(ota.write_chunk(4, b"data"), Err(OtaError::NonSequential));
    }

    #[test]
    fn chunk_rejects_overflow() {
        let mut ota = OtaManager::new();
        ota.begin(4, &sha()).unwrap();
        assert_eq!(ota.write_chunk(0, b"12345"), Err(OtaError::Overflow));
    }

    #[test]
    fn finalize_requires_all_bytes() {
        let mut ota = OtaManager::new();
        ota.begin(8, &sha()).unwrap();
        ota.write_chunk(0, b"half").unwrap();
        assert_eq!(ota.finalize(), Err(OtaError::IncompleteTransfer));
    }

    #[test]
    fn finalize_requires_receiving() {
        let mut ota = OtaManager::new();
        assert_eq!(ota.finalize(), Err(OtaError::NotReceiving));
    }

    #[test]
    fn abort_resets_to_idle() {
        let mut ota = OtaManager::new();
        ota.begin(1024, &sha()).unwrap();
        ota.abort();
        assert_eq!(ota.state(), OtaState::Idle);
    }

    #[test]
    fn happy_path_single_chunk() {
        let mut ota = OtaManager::new();
        ota.begin(4, &sha()).unwrap();
        assert_eq!(ota.write_chunk(0, b"1234").unwrap(), 4);
        assert!(ota.finalize().is_ok());
        assert_eq!(ota.state(), OtaState::ReadyToReboot);
    }

    #[test]
    fn happy_path_multi_chunk() {
        let mut ota = OtaManager::new();
        ota.begin(8, &sha()).unwrap();
        assert_eq!(ota.write_chunk(0, b"abcd").unwrap(), 4);
        assert_eq!(ota.write_chunk(4, b"efgh").unwrap(), 8);
        assert!(ota.finalize().is_ok());
    }

    #[test]
    fn error_display_coverage() {
        assert!(OtaError::BeginFailed.to_string().contains("begin failed"));
        assert!(OtaError::WriteFailed.to_string().contains("write failed"));
    }
}
