//! Crash logging and runtime diagnostics.
//!
//! Stores up to 4 crash entries in an NVS ring buffer under the "crash"
//! namespace. Each entry captures timestamp, reason, program counter, and
//! a truncated backtrace. A custom panic handler writes the entry before
//! the TWDT or panic handler triggers a reset.
//!
//! Runtime metrics (heap, RSSI, NVS usage, ULP wakes) are collected
//! on-demand for the diagnostics RPC response.

use serde::{Deserialize, Serialize};

const CRASH_RING_SLOTS: usize = 4;
const CRASH_KEY_PREFIX: &str = "crash";
const CRASH_INDEX_KEY: &str = "crash_idx";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashEntry {
    pub uptime_secs: u64,
    pub reason: heapless::String<64>,
    pub pc: u32,
    pub backtrace: heapless::Vec<u32, 8>,
}

impl CrashEntry {
    pub fn new(uptime_secs: u64, reason: &str, pc: u32) -> Self {
        let mut r = heapless::String::new();
        let _ = r.push_str(&reason[..reason.len().min(63)]);
        Self {
            uptime_secs,
            reason: r,
            pc,
            backtrace: heapless::Vec::new(),
        }
    }
}

/// NVS-backed ring buffer for crash entries.
#[derive(Default)]
pub struct CrashLog {
    write_index: usize,
}

impl CrashLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the write index from NVS, or default to 0.
    pub fn init(&mut self, nvs: &dyn crate::app::ports::StoragePort) {
        let mut buf = [0u8; 4];
        if let Ok(4) = nvs.read(CRASH_KEY_PREFIX, CRASH_INDEX_KEY, &mut buf) {
            self.write_index = u32::from_le_bytes(buf) as usize % CRASH_RING_SLOTS;
        }
    }

    /// Write a crash entry to the next ring slot and advance the index.
    pub fn write_entry(
        &mut self,
        nvs: &mut dyn crate::app::ports::StoragePort,
        entry: &CrashEntry,
    ) {
        let slot_key = Self::slot_key(self.write_index);
        if let Ok(bytes) = postcard::to_allocvec(entry) {
            let _ = nvs.write(CRASH_KEY_PREFIX, &slot_key, &bytes);
        }

        self.write_index = (self.write_index + 1) % CRASH_RING_SLOTS;
        let idx_bytes = (self.write_index as u32).to_le_bytes();
        let _ = nvs.write(CRASH_KEY_PREFIX, CRASH_INDEX_KEY, &idx_bytes);
    }

    /// Read all stored crash entries (up to 4).
    pub fn read_all(
        &self,
        nvs: &dyn crate::app::ports::StoragePort,
    ) -> heapless::Vec<CrashEntry, 4> {
        let mut entries = heapless::Vec::new();
        for i in 0..CRASH_RING_SLOTS {
            let slot_key = Self::slot_key(i);
            let mut buf = [0u8; 256];
            if let Ok(len) = nvs.read(CRASH_KEY_PREFIX, &slot_key, &mut buf) {
                if let Ok(entry) = postcard::from_bytes::<CrashEntry>(&buf[..len]) {
                    let _ = entries.push(entry);
                }
            }
        }
        entries
    }

    /// Erase all crash entries and reset the index.
    pub fn clear(&mut self, nvs: &mut dyn crate::app::ports::StoragePort) {
        for i in 0..CRASH_RING_SLOTS {
            let slot_key = Self::slot_key(i);
            let _ = nvs.delete(CRASH_KEY_PREFIX, &slot_key);
        }
        let _ = nvs.delete(CRASH_KEY_PREFIX, CRASH_INDEX_KEY);
        self.write_index = 0;
    }

    pub fn count(&self, nvs: &dyn crate::app::ports::StoragePort) -> usize {
        (0..CRASH_RING_SLOTS)
            .filter(|i| nvs.exists(CRASH_KEY_PREFIX, &Self::slot_key(*i)))
            .count()
    }

    fn slot_key(index: usize) -> heapless::String<16> {
        let mut s = heapless::String::new();
        let _ = core::fmt::Write::write_fmt(&mut s, format_args!("e{}", index));
        s
    }
}

/// Runtime diagnostics snapshot collected on-demand.
#[derive(Debug, Clone)]
pub struct RuntimeMetrics {
    pub uptime_secs: u64,
    pub control_cycles: u64,
    pub fault_count: u32,
    pub crash_count: u32,
    pub heap_free: u32,
    pub heap_min_free: u32,
    pub wifi_rssi: i8,
    pub nvs_free_entries: u32,
    pub ulp_wake_count: u32,
}

impl RuntimeMetrics {
    #[cfg(target_os = "espidf")]
    pub fn collect(
        uptime_secs: u64,
        control_cycles: u64,
        fault_count: u32,
        crash_count: u32,
        ulp_wakes: u32,
    ) -> Self {
        use esp_idf_svc::sys::*;
        let heap_free = unsafe { esp_get_free_heap_size() };
        let heap_min = unsafe { esp_get_minimum_free_heap_size() };

        let wifi_rssi = Self::read_wifi_rssi();
        let nvs_free_entries = Self::read_nvs_free_entries();

        Self {
            uptime_secs,
            control_cycles,
            fault_count,
            crash_count,
            heap_free,
            heap_min_free: heap_min,
            wifi_rssi,
            nvs_free_entries,
            ulp_wake_count: ulp_wakes,
        }
    }

    #[cfg(target_os = "espidf")]
    fn read_wifi_rssi() -> i8 {
        use esp_idf_svc::sys::*;
        let mut ap_info: wifi_ap_record_t = unsafe { core::mem::zeroed() };
        let ret = unsafe { esp_wifi_sta_get_ap_info(&mut ap_info) };
        if ret == ESP_OK { ap_info.rssi } else { 0 }
    }

    #[cfg(target_os = "espidf")]
    fn read_nvs_free_entries() -> u32 {
        use esp_idf_svc::sys::*;
        let mut stats: nvs_stats_t = unsafe { core::mem::zeroed() };
        let part_name = b"nvs\0";
        let ret = unsafe { nvs_get_stats(part_name.as_ptr() as *const _, &mut stats) };
        if ret == ESP_OK {
            stats.free_entries as u32
        } else {
            0
        }
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn collect(
        uptime_secs: u64,
        control_cycles: u64,
        fault_count: u32,
        crash_count: u32,
        ulp_wakes: u32,
    ) -> Self {
        // Return realistic synthetic values so simulation paths exercise
        // the same code branches as real hardware.
        // Heap "decays" slightly over time to model fragmentation.
        let base_free: u32 = 307_200; // 300 KB
        let decay = (uptime_secs / 60) as u32 * 512; // lose ~512B/min
        let heap_free = base_free.saturating_sub(decay);
        let heap_min_free = (heap_free as f32 * 0.85) as u32;

        Self {
            uptime_secs,
            control_cycles,
            fault_count,
            crash_count,
            heap_free,
            heap_min_free,
            wifi_rssi: -60,
            nvs_free_entries: 120,
            ulp_wake_count: ulp_wakes,
        }
    }
}

// ───────────────────────────────────────────────────────────────
// Custom panic handler — writes a CrashEntry to NVS before reset
// ───────────────────────────────────────────────────────────────

/// Install a panic hook that persists crash info to NVS.
///
/// Must be called once during init, after NVS is ready.
/// On panic, captures the reason string and writes a CrashEntry
/// to the NVS ring buffer before the default panic handler aborts.
pub fn install_panic_handler() {
    std::panic::set_hook(Box::new(|info| {
        let reason = if let Some(msg) = info.payload().downcast_ref::<&str>() {
            *msg
        } else if let Some(msg) = info.payload().downcast_ref::<String>() {
            msg.as_str()
        } else {
            "unknown panic"
        };

        log::error!("PANIC: {}", reason);

        #[cfg(target_os = "espidf")]
        {
            // SAFETY: esp_timer_get_time is safe to call from panic context
            // (it is a simple RTC counter read with no dynamic allocation).
            let uptime = (unsafe { esp_idf_svc::sys::esp_timer_get_time() }) as u64 / 1_000_000;
            let entry = CrashEntry::new(uptime, reason, 0);

            // Attempt to write the crash entry; if NVS is unavailable at this
            // point (e.g. the panic occurred before init), we log and skip.
            // We do NOT call nvs_flash_init() here — that is not safe in
            // panic context. Instead we rely on the existing NVS session
            // established in main(). If it was never initialised, the write
            // will silently fail, which is acceptable.
            match crate::adapters::nvs::NvsAdapter::new() {
                Ok(mut nvs) => {
                    let mut crash_log = CrashLog::new();
                    crash_log.init(&nvs);
                    crash_log.write_entry(&mut nvs, &entry);
                }
                Err(_) => {
                    log::error!("Panic handler: NVS unavailable — crash entry not persisted");
                }
            }
        }

        #[cfg(not(target_os = "espidf"))]
        {
            log::error!("Crash entry (simulation): {}", reason);
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ports::{StorageError, StoragePort};
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct MockStorage {
        data: RefCell<HashMap<String, Vec<u8>>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                data: RefCell::new(HashMap::new()),
            }
        }
    }

    impl StoragePort for MockStorage {
        fn read(&self, ns: &str, key: &str, buf: &mut [u8]) -> Result<usize, StorageError> {
            let k = format!("{ns}::{key}");
            match self.data.borrow().get(&k) {
                Some(v) => {
                    let len = v.len().min(buf.len());
                    buf[..len].copy_from_slice(&v[..len]);
                    Ok(len)
                }
                None => Err(StorageError::NotFound),
            }
        }

        fn write(&mut self, ns: &str, key: &str, data: &[u8]) -> Result<(), StorageError> {
            let k = format!("{ns}::{key}");
            self.data.borrow_mut().insert(k, data.to_vec());
            Ok(())
        }

        fn delete(&mut self, ns: &str, key: &str) -> Result<(), StorageError> {
            let k = format!("{ns}::{key}");
            self.data.borrow_mut().remove(&k);
            Ok(())
        }

        fn exists(&self, ns: &str, key: &str) -> bool {
            let k = format!("{ns}::{key}");
            self.data.borrow().contains_key(&k)
        }
    }

    #[test]
    fn crash_log_starts_at_zero() {
        let log = CrashLog::new();
        assert_eq!(log.write_index, 0);
    }

    #[test]
    fn write_and_read_single_entry() {
        let mut nvs = MockStorage::new();
        let mut log = CrashLog::new();
        let entry = CrashEntry::new(42, "test panic", 0xDEAD);

        log.write_entry(&mut nvs, &entry);
        let entries = log.read_all(&nvs);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].uptime_secs, 42);
        assert_eq!(entries[0].pc, 0xDEAD);
    }

    #[test]
    fn ring_buffer_wraps() {
        let mut nvs = MockStorage::new();
        let mut log = CrashLog::new();

        for i in 0..6 {
            let entry = CrashEntry::new(i as u64, &format!("crash_{i}"), i as u32);
            log.write_entry(&mut nvs, &entry);
        }
        let entries = log.read_all(&nvs);
        assert_eq!(entries.len(), CRASH_RING_SLOTS);
    }

    #[test]
    fn clear_erases_all() {
        let mut nvs = MockStorage::new();
        let mut log = CrashLog::new();

        log.write_entry(&mut nvs, &CrashEntry::new(1, "x", 0));
        log.write_entry(&mut nvs, &CrashEntry::new(2, "y", 0));
        log.clear(&mut nvs);

        let entries = log.read_all(&nvs);
        assert_eq!(entries.len(), 0);
        assert_eq!(log.write_index, 0);
    }

    #[test]
    fn crash_entry_truncates_long_reason() {
        let long = "a".repeat(200);
        let entry = CrashEntry::new(0, &long, 0);
        assert!(entry.reason.len() <= 63);
    }

    #[test]
    fn count_matches_entries() {
        let mut nvs = MockStorage::new();
        let mut log = CrashLog::new();

        assert_eq!(log.count(&nvs), 0);
        log.write_entry(&mut nvs, &CrashEntry::new(1, "a", 0));
        assert_eq!(log.count(&nvs), 1);
        log.write_entry(&mut nvs, &CrashEntry::new(2, "b", 0));
        assert_eq!(log.count(&nvs), 2);
    }
}
