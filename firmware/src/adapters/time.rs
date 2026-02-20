//! ESP32 time adapter.
//!
//! Provides monotonic time queries for the PetFilter system.
//!
//! - **`target_os = "espidf"`** — wraps `esp_timer_get_time()` from the
//!   ESP-IDF high-resolution timer (microsecond precision, monotonic).
//! - **`not(target_os = "espidf")`** — uses `std::time::Instant` for
//!   host-side testing and simulation.

/// Time adapter for the ESP32-S3 platform.
pub struct Esp32TimeAdapter {
    #[cfg(not(target_os = "espidf"))]
    start: std::time::Instant,
}

impl Default for Esp32TimeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Esp32TimeAdapter {
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_os = "espidf"))]
            start: std::time::Instant::now(),
        }
    }

    /// Seconds since boot (monotonic).
    #[cfg(target_os = "espidf")]
    pub fn uptime_secs(&self) -> u64 {
        (unsafe { esp_idf_svc::sys::esp_timer_get_time() }) as u64 / 1_000_000
    }

    /// Seconds since boot (monotonic).
    #[cfg(not(target_os = "espidf"))]
    pub fn uptime_secs(&self) -> u64 {
        self.start.elapsed().as_secs()
    }

    /// Microseconds since boot (monotonic, wraps at `u64::MAX`).
    #[cfg(target_os = "espidf")]
    pub fn uptime_us(&self) -> u64 {
        (unsafe { esp_idf_svc::sys::esp_timer_get_time() }) as u64
    }

    /// Microseconds since boot (monotonic, wraps at `u64::MAX`).
    #[cfg(not(target_os = "espidf"))]
    pub fn uptime_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }

    /// Current hour-of-day (0–23) from system clock. `None` if wall clock not synced (e.g. pre-NTP).
    #[cfg(target_os = "espidf")]
    pub fn current_hour(&self) -> Option<u8> {
        use core::ptr;
        let mut tv = esp_idf_svc::sys::timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        if unsafe { esp_idf_svc::sys::gettimeofday(&mut tv, ptr::null_mut()) } != 0 {
            return None;
        }
        // Reject obviously unsynced time (e.g. before 2020-01-01)
        const EPOCH_2020: i64 = 1_577_836_800;
        if tv.tv_sec < EPOCH_2020 {
            return None;
        }
        let secs = tv.tv_sec as esp_idf_svc::sys::time_t;
        let mut tm: esp_idf_svc::sys::tm = unsafe { core::mem::zeroed() };
        if unsafe { esp_idf_svc::sys::localtime_r(&secs, &mut tm) }.is_null() {
            return None;
        }
        let hour = tm.tm_hour;
        if hour < 0 || hour > 23 {
            return None;
        }
        Some(hour as u8)
    }

    /// Current hour-of-day. On non-ESP targets (simulation) always `None`.
    #[cfg(not(target_os = "espidf"))]
    pub fn current_hour(&self) -> Option<u8> {
        None
    }
}
