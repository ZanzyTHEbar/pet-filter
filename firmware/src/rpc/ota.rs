//! Secure OTA firmware update via RPC.
//!
//! Flow: OtaBegin → N × OtaChunk → OtaFinalize → reboot
//!
//! Session must be Authenticated before OtaBegin is accepted.

use core::fmt;
use log::{info, warn};

#[cfg(target_os = "espidf")]
use esp_idf_svc::sys::*;

const MAX_FIRMWARE_SIZE: u32 = 4 * 1024 * 1024; // 4 MB

// ── Error type ────────────────────────────────────────────────

/// Typed errors for OTA firmware update operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtaError {
    AlreadyInProgress,
    InvalidSize,
    InvalidSha,
    NoPartition,
    BeginFailed(i32),
    WriteFailed(i32),
    VerifyFailed(i32),
    BootSetFailed(i32),
    IncompleteTransfer,
    NotReceiving,
    NonSequential,
    Overflow,
}

impl fmt::Display for OtaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyInProgress  => write!(f, "OTA session already in progress"),
            Self::InvalidSize        => write!(f, "firmware size out of range (max 4 MB)"),
            Self::InvalidSha         => write!(f, "SHA-256 must be exactly 32 bytes"),
            Self::NoPartition        => write!(f, "no inactive OTA partition available"),
            Self::BeginFailed(rc)    => write!(f, "esp_ota_begin failed (rc={})", rc),
            Self::WriteFailed(rc)    => write!(f, "esp_ota_write failed (rc={})", rc),
            Self::VerifyFailed(rc)   => write!(f, "esp_ota_end verification failed (rc={})", rc),
            Self::BootSetFailed(rc)  => write!(f, "esp_ota_set_boot_partition failed (rc={})", rc),
            Self::IncompleteTransfer => write!(f, "finalize called before all bytes written"),
            Self::NotReceiving       => write!(f, "operation requires active Receiving state"),
            Self::NonSequential      => write!(f, "chunk offset does not match expected offset"),
            Self::Overflow           => write!(f, "chunk would exceed declared firmware size"),
        }
    }
}

// ── State machine ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtaState {
    Idle,
    Receiving { expected_size: u32, bytes_written: u32 },
    Verifying,
    ReadyToReboot,
    Failed,
}

// ── Manager ───────────────────────────────────────────────────

pub struct OtaManager {
    state: OtaState,
    expected_sha256: [u8; 32],
    #[cfg(target_os = "espidf")]
    ota_handle: esp_ota_handle_t,
    #[cfg(target_os = "espidf")]
    update_partition: *const esp_partition_t,
}

impl OtaManager {
    pub fn new() -> Self {
        Self {
            state: OtaState::Idle,
            expected_sha256: [0u8; 32],
            #[cfg(target_os = "espidf")]
            ota_handle: 0,
            #[cfg(target_os = "espidf")]
            update_partition: core::ptr::null(),
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
            // SAFETY: Called from single main-task context; state == Idle
            // guarantees no concurrent OTA handle exists.
            unsafe {
                let partition = esp_ota_get_next_update_partition(core::ptr::null());
                if partition.is_null() {
                    return Err(OtaError::NoPartition);
                }
                self.update_partition = partition;

                let ret = esp_ota_begin(partition, firmware_size as usize, &mut self.ota_handle);
                if ret != ESP_OK {
                    warn!("esp_ota_begin failed: rc={}", ret);
                    return Err(OtaError::BeginFailed(ret));
                }
            }
        }

        self.state = OtaState::Receiving { expected_size: firmware_size, bytes_written: 0 };
        info!("OTA: begin ({} bytes)", firmware_size);
        Ok(())
    }

    /// Write a chunk at the given byte offset. Returns total bytes written.
    pub fn write_chunk(&mut self, offset: u32, data: &[u8]) -> Result<u32, OtaError> {
        let (expected_size, bytes_written) = match self.state {
            OtaState::Receiving { expected_size, bytes_written } => (expected_size, bytes_written),
            _ => return Err(OtaError::NotReceiving),
        };

        if offset != bytes_written {
            return Err(OtaError::NonSequential);
        }
        if bytes_written + data.len() as u32 > expected_size {
            return Err(OtaError::Overflow);
        }

        #[cfg(target_os = "espidf")]
        {
            // SAFETY: ota_handle is valid — allocated by begin() and not yet
            // freed; single-threaded access guaranteed by main-task execution.
            let ret = unsafe {
                esp_ota_write(self.ota_handle, data.as_ptr() as *const _, data.len())
            };
            if ret != ESP_OK {
                self.abort();
                return Err(OtaError::WriteFailed(ret));
            }
        }

        let new_written = bytes_written + data.len() as u32;
        self.state = OtaState::Receiving { expected_size, bytes_written: new_written };
        Ok(new_written)
    }

    /// Finalize: verify image, mark partition bootable, set ReadyToReboot.
    pub fn finalize(&mut self) -> Result<(), OtaError> {
        match self.state {
            OtaState::Receiving { expected_size, bytes_written }
                if bytes_written == expected_size => {}
            OtaState::Receiving { .. } => return Err(OtaError::IncompleteTransfer),
            _ => return Err(OtaError::NotReceiving),
        }

        self.state = OtaState::Verifying;

        #[cfg(target_os = "espidf")]
        {
            // SAFETY: ota_handle valid; called once after all chunks written.
            unsafe {
                let ret = esp_ota_end(self.ota_handle);
                if ret != ESP_OK {
                    warn!("esp_ota_end failed: rc={}", ret);
                    self.state = OtaState::Failed;
                    return Err(OtaError::VerifyFailed(ret));
                }

                let ret = esp_ota_set_boot_partition(self.update_partition);
                if ret != ESP_OK {
                    warn!("esp_ota_set_boot_partition failed: rc={}", ret);
                    self.state = OtaState::Failed;
                    return Err(OtaError::BootSetFailed(ret));
                }
            }
        }

        self.state = OtaState::ReadyToReboot;
        info!("OTA: finalized, ready to reboot");
        Ok(())
    }

    /// Abort the current OTA session; resets to Idle.
    pub fn abort(&mut self) {
        #[cfg(target_os = "espidf")]
        {
            if self.ota_handle != 0 {
                // SAFETY: ota_handle valid; esp_ota_abort invalidates it.
                unsafe { esp_ota_abort(self.ota_handle); }
                self.ota_handle = 0;
            }
        }
        self.state = OtaState::Idle;
        warn!("OTA: aborted");
    }

    /// Soft-reset into the newly flashed firmware.
    /// Only call after `finalize()` returns `Ok(())`.
    #[cfg(target_os = "espidf")]
    pub fn reboot(&self) -> ! {
        info!("OTA: rebooting into new firmware");
        // SAFETY: esp_restart() is the ESP-IDF sanctioned soft-reset.
        unsafe { esp_restart(); }
        #[allow(unreachable_code)]
        loop {}
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

/// Check OTA image state on startup and mark this firmware as valid
/// if it was just installed (`PENDING_VERIFY`).
///
/// Without this, the rollback watchdog reverts to the previous firmware
/// after three consecutive failed boots.
#[cfg(target_os = "espidf")]
pub fn check_rollback() {
    // SAFETY: Read-only partition queries; mark_app_valid is idempotent.
    unsafe {
        let mut state: esp_ota_img_states_t = 0;
        let partition = esp_ota_get_running_partition();
        if !partition.is_null() {
            esp_ota_get_state_partition(partition, &mut state);
        }

        if state == esp_ota_img_states_t_ESP_OTA_IMG_PENDING_VERIFY {
            info!("OTA: new firmware booted successfully — marking valid");
            esp_ota_mark_app_valid_cancel_rollback();
        }
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

    fn sha() -> [u8; 32] { [0u8; 32] }

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
        assert_eq!(ota.begin(5 * 1024 * 1024, &sha()), Err(OtaError::InvalidSize));
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
    fn error_display_includes_rc() {
        assert!(OtaError::BeginFailed(-1).to_string().contains("rc="));
        assert!(OtaError::WriteFailed(-5).to_string().contains("-5"));
    }
}
