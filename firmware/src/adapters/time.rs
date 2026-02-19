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
}
