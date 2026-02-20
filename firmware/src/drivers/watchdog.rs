//! Task Watchdog Timer (TWDT) driver.
//!
//! Wraps the ESP-IDF TWDT API to reset the device if the main loop
//! stalls for more than 10 seconds.
//!
//! The main loop must call `feed()` on every control tick iteration.

#[cfg(target_os = "espidf")]
use esp_idf_svc::sys::*;

#[cfg(target_os = "espidf")]
use log::info;

pub struct Watchdog {
    #[cfg(target_os = "espidf")]
    subscribed: bool,
}

impl Default for Watchdog {
    fn default() -> Self {
        Self::new()
    }
}

impl Watchdog {
    /// Initialise and subscribe the current task to the TWDT.
    pub fn new() -> Self {
        #[cfg(target_os = "espidf")]
        {
            unsafe {
                let cfg = esp_task_wdt_config_t {
                    timeout_ms: 10_000,
                    idle_core_mask: 0,
                    trigger_panic: true,
                };
                let ret = esp_task_wdt_reconfigure(&cfg);
                if ret != ESP_OK {
                    log::warn!(
                        "TWDT reconfigure returned {} (may already be configured)",
                        ret
                    );
                }

                let ret = esp_task_wdt_add(core::ptr::null_mut());
                let subscribed = ret == ESP_OK;
                if subscribed {
                    info!("Watchdog: subscribed (10s timeout, panic on trigger)");
                } else {
                    log::warn!("Watchdog: failed to subscribe ({})", ret);
                }

                Self { subscribed }
            }
        }

        #[cfg(not(target_os = "espidf"))]
        {
            log::info!("Watchdog(sim): no-op");
            Self {}
        }
    }

    /// Feed the watchdog. Must be called at least every 10 seconds.
    pub fn feed(&self) {
        #[cfg(target_os = "espidf")]
        {
            if self.subscribed {
                unsafe {
                    esp_task_wdt_reset();
                }
            }
        }
    }
}
