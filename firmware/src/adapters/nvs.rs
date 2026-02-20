//! NVS (Non-Volatile Storage) adapter.
//!
//! Implements both [`ConfigPort`] and [`StoragePort`] for the PetFilter system.
//!
//! # Security
//!
//! - Config validation: all fields are range-checked before persistence.
//! - Encrypted NVS: on ESP32, the "auth" namespace lives on the encrypted
//!   NVS partition. The simulation backend uses plaintext (dev/test only).
//! - Namespace isolation: each subsystem uses its own namespace prefix.
//! - Atomic writes: ESP-IDF NVS commits are atomic per nvs_commit().

use crate::app::ports::{ConfigError, ConfigPort, StorageError, StoragePort};
use crate::config::SystemConfig;
use log::info;

#[cfg(not(target_os = "espidf"))]
use std::collections::HashMap;

#[cfg(target_os = "espidf")]
use esp_idf_svc::sys::*;

#[cfg(target_os = "espidf")]
use core::ffi::CStr;

const CONFIG_NAMESPACE: &str = "petfilter";
const CONFIG_KEY: &str = "syscfg";

#[allow(dead_code)]
const MAX_BLOB_SIZE: usize = 4000;
const CRED_NAMESPACE: &str = "auth";

pub struct NvsAdapter {
    #[cfg(not(target_os = "espidf"))]
    store: std::cell::RefCell<HashMap<String, Vec<u8>>>,
}

impl NvsAdapter {
    /// Create a new NvsAdapter and initialise NVS flash.
    ///
    /// Returns `Err(ConfigError::IoError)` if flash initialisation fails
    /// unrecoverably. On first boot or after a version mismatch the NVS
    /// partition is erased and re-initialised automatically.
    pub fn new() -> Result<Self, crate::app::ports::ConfigError> {
        #[cfg(target_os = "espidf")]
        {
            // SAFETY: nvs_flash_init / nvs_flash_erase are called from the
            // single main-task context before any concurrent NVS access.
            let ret = unsafe { nvs_flash_init() };
            if ret == ESP_ERR_NVS_NO_FREE_PAGES || ret == ESP_ERR_NVS_NEW_VERSION_FOUND {
                warn!("NVS: erasing and re-initialising flash partition");
                let ret2 = unsafe { nvs_flash_erase() };
                if ret2 != ESP_OK {
                    return Err(crate::app::ports::ConfigError::IoError);
                }
                let ret3 = unsafe { nvs_flash_init() };
                if ret3 != ESP_OK {
                    return Err(crate::app::ports::ConfigError::IoError);
                }
            } else if ret != ESP_OK {
                return Err(crate::app::ports::ConfigError::IoError);
            }
            info!("NvsAdapter: ESP-IDF NVS initialised");
        }

        #[cfg(not(target_os = "espidf"))]
        info!("NvsAdapter: simulation backend");

        Ok(Self {
            #[cfg(not(target_os = "espidf"))]
            store: std::cell::RefCell::new(HashMap::new()),
        })
    }

    fn composite_key(namespace: &str, key: &str) -> String {
        format!("{}::{}", namespace, key)
    }

    /// Open an NVS namespace, execute a closure with the handle, then close.
    #[cfg(target_os = "espidf")]
    fn with_nvs_handle<F, T>(namespace: &str, write: bool, f: F) -> Result<T, i32>
    where
        F: FnOnce(nvs_handle_t) -> Result<T, i32>,
    {
        let mut ns_buf = [0u8; 16];
        let ns_bytes = namespace.as_bytes();
        let len = ns_bytes.len().min(15);
        ns_buf[..len].copy_from_slice(&ns_bytes[..len]);

        let mut handle: nvs_handle_t = 0;
        let mode = if write {
            nvs_open_mode_t_NVS_READWRITE
        } else {
            nvs_open_mode_t_NVS_READONLY
        };

        let ret = unsafe { nvs_open(ns_buf.as_ptr() as *const _, mode, &mut handle) };
        if ret != ESP_OK {
            return Err(ret);
        }

        let result = f(handle);
        unsafe {
            nvs_close(handle);
        }
        result
    }
}

fn validate_config(cfg: &SystemConfig) -> Result<(), ConfigError> {
    if !(1.0..=200.0).contains(&cfg.nh3_activate_threshold_ppm) {
        return Err(ConfigError::ValidationFailed(
            "nh3_activate_threshold_ppm must be 1.0–200.0",
        ));
    }
    if !(0.5..=199.0).contains(&cfg.nh3_deactivate_threshold_ppm) {
        return Err(ConfigError::ValidationFailed(
            "nh3_deactivate_threshold_ppm must be 0.5–199.0",
        ));
    }
    if cfg.nh3_deactivate_threshold_ppm >= cfg.nh3_activate_threshold_ppm {
        return Err(ConfigError::ValidationFailed(
            "nh3_deactivate_threshold_ppm must be < nh3_activate_threshold_ppm",
        ));
    }
    if cfg.pump_duty_percent > 100 {
        return Err(ConfigError::ValidationFailed(
            "pump_duty_percent must be 0–100",
        ));
    }
    if cfg.uvc_duty_percent > 100 {
        return Err(ConfigError::ValidationFailed(
            "uvc_duty_percent must be 0–100",
        ));
    }
    if !(40.0..=120.0).contains(&cfg.max_temperature_c) {
        return Err(ConfigError::ValidationFailed(
            "max_temperature_c must be 40.0–120.0",
        ));
    }
    if !(10..=600).contains(&cfg.purge_duration_secs) {
        return Err(ConfigError::ValidationFailed(
            "purge_duration_secs must be 10–600",
        ));
    }
    if !(100..=5000).contains(&cfg.control_loop_interval_ms) {
        return Err(ConfigError::ValidationFailed(
            "control_loop_interval_ms must be 100–5000",
        ));
    }
    if !(5..=3600).contains(&cfg.telemetry_interval_secs) {
        return Err(ConfigError::ValidationFailed(
            "telemetry_interval_secs must be 5–3600",
        ));
    }
    Ok(())
}

impl ConfigPort for NvsAdapter {
    fn load(&self) -> Result<SystemConfig, ConfigError> {
        #[cfg(not(target_os = "espidf"))]
        {
            let key = Self::composite_key(CONFIG_NAMESPACE, CONFIG_KEY);
            if let Some(bytes) = self.store.borrow().get(&key) {
                let cfg: SystemConfig =
                    postcard::from_bytes(bytes).map_err(|_| ConfigError::Corrupted)?;
                info!("NvsAdapter: loaded config from store");
                Ok(cfg)
            } else {
                info!("NvsAdapter: no stored config, using defaults");
                Ok(SystemConfig::default())
            }
        }

        #[cfg(target_os = "espidf")]
        {
            let result = Self::with_nvs_handle(CONFIG_NAMESPACE, false, |handle| {
                let key_cstr = b"syscfg\0";
                let mut size: usize = 0;

                // First call: get size
                let ret = unsafe {
                    nvs_get_blob(
                        handle,
                        key_cstr.as_ptr() as *const _,
                        core::ptr::null_mut(),
                        &mut size,
                    )
                };
                if ret == ESP_ERR_NVS_NOT_FOUND {
                    return Err(ESP_ERR_NVS_NOT_FOUND);
                }
                if ret != ESP_OK || size == 0 || size > MAX_BLOB_SIZE {
                    return Err(ret);
                }

                let mut buf = vec![0u8; size];
                let ret = unsafe {
                    nvs_get_blob(
                        handle,
                        key_cstr.as_ptr() as *const _,
                        buf.as_mut_ptr() as *mut _,
                        &mut size,
                    )
                };
                if ret != ESP_OK {
                    return Err(ret);
                }

                Ok(buf)
            });

            match result {
                Ok(bytes) => {
                    let cfg: SystemConfig =
                        postcard::from_bytes(&bytes).map_err(|_| ConfigError::Corrupted)?;
                    info!("NvsAdapter: loaded config from NVS ({} bytes)", bytes.len());
                    Ok(cfg)
                }
                Err(e) if e == ESP_ERR_NVS_NOT_FOUND => {
                    info!("NvsAdapter: no stored config, using defaults");
                    Ok(SystemConfig::default())
                }
                Err(e) => {
                    warn!("NvsAdapter: NVS read error {}, using defaults", e);
                    Ok(SystemConfig::default())
                }
            }
        }
    }

    fn save(&self, config: &SystemConfig) -> Result<(), ConfigError> {
        validate_config(config)?;

        #[cfg(not(target_os = "espidf"))]
        {
            let key = Self::composite_key(CONFIG_NAMESPACE, CONFIG_KEY);
            let bytes = postcard::to_allocvec(config).map_err(|_| ConfigError::IoError)?;
            self.store.borrow_mut().insert(key, bytes);
            info!("NvsAdapter: config saved (simulation)");
            Ok(())
        }

        #[cfg(target_os = "espidf")]
        {
            let bytes = postcard::to_allocvec(config).map_err(|_| ConfigError::IoError)?;
            let result = Self::with_nvs_handle(CONFIG_NAMESPACE, true, |handle| {
                let key_cstr = b"syscfg\0";
                let ret = unsafe {
                    nvs_set_blob(
                        handle,
                        key_cstr.as_ptr() as *const _,
                        bytes.as_ptr() as *const _,
                        bytes.len(),
                    )
                };
                if ret != ESP_OK {
                    return Err(ret);
                }
                let ret = unsafe { nvs_commit(handle) };
                if ret != ESP_OK {
                    return Err(ret);
                }
                Ok(())
            });
            match result {
                Ok(()) => {
                    info!("NvsAdapter: config saved to NVS ({} bytes)", bytes.len());
                    Ok(())
                }
                Err(e) => {
                    warn!("NvsAdapter: NVS write error {}", e);
                    Err(ConfigError::IoError)
                }
            }
        }
    }
}

impl StoragePort for NvsAdapter {
    fn read(&self, namespace: &str, key: &str, buf: &mut [u8]) -> Result<usize, StorageError> {
        #[cfg(not(target_os = "espidf"))]
        {
            let composite = Self::composite_key(namespace, key);
            match self.store.borrow().get(&composite) {
                Some(data) => {
                    let len = data.len().min(buf.len());
                    buf[..len].copy_from_slice(&data[..len]);
                    Ok(len)
                }
                None => Err(StorageError::NotFound),
            }
        }

        #[cfg(target_os = "espidf")]
        {
            let result = Self::with_nvs_handle(namespace, false, |handle| {
                let mut key_buf = [0u8; 16];
                let kb = key.as_bytes();
                let kl = kb.len().min(15);
                key_buf[..kl].copy_from_slice(&kb[..kl]);

                let mut size = buf.len();
                let ret = unsafe {
                    nvs_get_blob(
                        handle,
                        key_buf.as_ptr() as *const _,
                        buf.as_mut_ptr() as *mut _,
                        &mut size,
                    )
                };
                if ret == ESP_ERR_NVS_NOT_FOUND {
                    return Err(ESP_ERR_NVS_NOT_FOUND);
                }
                if ret != ESP_OK {
                    return Err(ret);
                }
                Ok(size)
            });
            match result {
                Ok(size) => Ok(size),
                Err(e) if e == ESP_ERR_NVS_NOT_FOUND => Err(StorageError::NotFound),
                Err(_) => Err(StorageError::IoError),
            }
        }
    }

    fn write(&mut self, namespace: &str, key: &str, data: &[u8]) -> Result<(), StorageError> {
        #[cfg(not(target_os = "espidf"))]
        {
            let composite = Self::composite_key(namespace, key);
            self.store.borrow_mut().insert(composite, data.to_vec());
            Ok(())
        }

        #[cfg(target_os = "espidf")]
        {
            let result = Self::with_nvs_handle(namespace, true, |handle| {
                let mut key_buf = [0u8; 16];
                let kb = key.as_bytes();
                let kl = kb.len().min(15);
                key_buf[..kl].copy_from_slice(&kb[..kl]);

                let ret = unsafe {
                    nvs_set_blob(
                        handle,
                        key_buf.as_ptr() as *const _,
                        data.as_ptr() as *const _,
                        data.len(),
                    )
                };
                if ret != ESP_OK {
                    return Err(ret);
                }
                let ret = unsafe { nvs_commit(handle) };
                if ret != ESP_OK {
                    return Err(ret);
                }
                Ok(())
            });
            result.map_err(|_| StorageError::IoError)
        }
    }

    fn delete(&mut self, namespace: &str, key: &str) -> Result<(), StorageError> {
        #[cfg(not(target_os = "espidf"))]
        {
            let composite = Self::composite_key(namespace, key);
            self.store.borrow_mut().remove(&composite);
            Ok(())
        }

        #[cfg(target_os = "espidf")]
        {
            let result = Self::with_nvs_handle(namespace, true, |handle| {
                let mut key_buf = [0u8; 16];
                let kb = key.as_bytes();
                let kl = kb.len().min(15);
                key_buf[..kl].copy_from_slice(&kb[..kl]);

                let ret = unsafe { nvs_erase_key(handle, key_buf.as_ptr() as *const _) };
                if ret != ESP_OK && ret != ESP_ERR_NVS_NOT_FOUND {
                    return Err(ret);
                }
                let ret = unsafe { nvs_commit(handle) };
                if ret != ESP_OK {
                    return Err(ret);
                }
                Ok(())
            });
            result.map_err(|_| StorageError::IoError)
        }
    }

    fn exists(&self, namespace: &str, key: &str) -> bool {
        #[cfg(not(target_os = "espidf"))]
        {
            let composite = Self::composite_key(namespace, key);
            self.store.borrow().contains_key(&composite)
        }

        #[cfg(target_os = "espidf")]
        {
            let result = Self::with_nvs_handle(namespace, false, |handle| {
                let mut key_buf = [0u8; 16];
                let kb = key.as_bytes();
                let kl = kb.len().min(15);
                key_buf[..kl].copy_from_slice(&kb[..kl]);

                let ret = unsafe {
                    nvs_find_key(handle, key_buf.as_ptr() as *const _, core::ptr::null_mut())
                };
                Ok(ret == ESP_OK)
            });
            result.unwrap_or(false)
        }
    }
}

// ── Secure credential storage ──────────────────────────────────────
//
// Credentials (PSK, WiFi passwords) are stored in the "auth" NVS
// namespace.  On ESP32, NVS encryption is enabled partition-wide via
// the nvs_key partition (see partitions.csv).  When CONFIG_NVS_ENCRYPTION
// is set, all nvs_open / nvs_get / nvs_set calls transparently use
// AES-XTS encryption.  No code change is required beyond ensuring the
// nvs_key partition exists and flash encryption is enabled in production.
//
// For development: encryption is transparent — the same read/write/delete
// API works whether encryption is on or off.
impl NvsAdapter {
    /// Store a credential blob in the encrypted "auth" namespace.
    pub fn store_credential(
        &mut self,
        key: &str,
        data: &[u8],
    ) -> Result<(), crate::app::ports::StorageError> {
        self.write(CRED_NAMESPACE, key, data)
    }

    /// Read a credential blob from the encrypted "auth" namespace.
    pub fn read_credential(
        &self,
        key: &str,
        buf: &mut [u8],
    ) -> Result<usize, crate::app::ports::StorageError> {
        self.read(CRED_NAMESPACE, key, buf)
    }

    /// Delete a credential from the encrypted "auth" namespace.
    pub fn delete_credential(&mut self, key: &str) -> Result<(), crate::app::ports::StorageError> {
        self.delete(CRED_NAMESPACE, key)
    }

    /// Check if a credential exists in the encrypted "auth" namespace.
    pub fn credential_exists(&self, key: &str) -> bool {
        self.exists(CRED_NAMESPACE, key)
    }

    /// Erase the entire "auth" namespace (factory reset).
    #[cfg(target_os = "espidf")]
    pub fn erase_credentials(&mut self) -> Result<(), crate::app::ports::StorageError> {
        let result = Self::with_nvs_handle(CRED_NAMESPACE, true, |handle| {
            let ret = unsafe { nvs_erase_all(handle) };
            if ret != ESP_OK {
                return Err(ret);
            }
            let ret = unsafe { nvs_commit(handle) };
            if ret != ESP_OK {
                return Err(ret);
            }
            Ok(())
        });
        result.map_err(|_| crate::app::ports::StorageError::IoError)
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn erase_credentials(&mut self) -> Result<(), crate::app::ports::StorageError> {
        let prefix = format!("{}::", CRED_NAMESPACE);
        self.store
            .borrow_mut()
            .retain(|k: &String, _: &mut Vec<u8>| !k.starts_with(&prefix));
        Ok(())
    }
}

impl Default for NvsAdapter {
    fn default() -> Self {
        // Panics if NVS is unavailable — only use as a last-resort fallback.
        Self::new().unwrap_or_else(|_| Self {
            #[cfg(not(target_os = "espidf"))]
            store: std::cell::RefCell::new(HashMap::new()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_passes_validation() {
        let cfg = SystemConfig::default();
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn rejects_activate_below_range() {
        let cfg = SystemConfig {
            nh3_activate_threshold_ppm: 0.5,
            ..Default::default()
        };
        assert!(matches!(
            validate_config(&cfg),
            Err(ConfigError::ValidationFailed(_))
        ));
    }

    #[test]
    fn rejects_deactivate_above_activate() {
        let cfg = SystemConfig {
            nh3_activate_threshold_ppm: 10.0,
            nh3_deactivate_threshold_ppm: 15.0,
            ..Default::default()
        };
        assert!(matches!(
            validate_config(&cfg),
            Err(ConfigError::ValidationFailed(_))
        ));
    }

    #[test]
    fn rejects_duty_over_100() {
        let cfg = SystemConfig {
            pump_duty_percent: 101,
            ..Default::default()
        };
        assert!(matches!(
            validate_config(&cfg),
            Err(ConfigError::ValidationFailed(_))
        ));
    }

    #[test]
    fn storage_round_trip() {
        let mut nvs = NvsAdapter::new().unwrap();
        let data = b"hello NVS";
        nvs.write("test_ns", "greeting", data).unwrap();
        assert!(nvs.exists("test_ns", "greeting"));

        let mut buf = [0u8; 64];
        let len = nvs.read("test_ns", "greeting", &mut buf).unwrap();
        assert_eq!(&buf[..len], data);

        nvs.delete("test_ns", "greeting").unwrap();
        assert!(!nvs.exists("test_ns", "greeting"));
    }

    #[test]
    fn storage_read_missing_key() {
        let nvs = NvsAdapter::new().unwrap();
        let mut buf = [0u8; 64];
        assert!(matches!(
            nvs.read("ns", "nope", &mut buf),
            Err(StorageError::NotFound)
        ));
    }

    #[test]
    fn credential_store_roundtrip() {
        let mut nvs = NvsAdapter::new().unwrap();
        let psk = [0xAA_u8; 32];
        nvs.store_credential("psk", &psk).unwrap();
        assert!(nvs.credential_exists("psk"));

        let mut buf = [0u8; 64];
        let len = nvs.read_credential("psk", &mut buf).unwrap();
        assert_eq!(&buf[..len], &psk);

        nvs.delete_credential("psk").unwrap();
        assert!(!nvs.credential_exists("psk"));
    }

    #[test]
    fn erase_credentials_clears_all() {
        let mut nvs = NvsAdapter::new().unwrap();
        nvs.store_credential("psk", &[0x01; 32]).unwrap();
        nvs.store_credential("wifi_pw", b"secret123").unwrap();
        nvs.erase_credentials().unwrap();
        assert!(!nvs.credential_exists("psk"));
        assert!(!nvs.credential_exists("wifi_pw"));
    }

    #[test]
    fn namespace_isolation() {
        let mut nvs = NvsAdapter::new().unwrap();
        nvs.write("ns_a", "key", b"alpha").unwrap();
        nvs.write("ns_b", "key", b"bravo").unwrap();

        let mut buf = [0u8; 64];
        let len = nvs.read("ns_a", "key", &mut buf).unwrap();
        assert_eq!(&buf[..len], b"alpha");

        let len = nvs.read("ns_b", "key", &mut buf).unwrap();
        assert_eq!(&buf[..len], b"bravo");
    }
}
