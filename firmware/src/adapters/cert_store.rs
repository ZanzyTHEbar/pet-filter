//! Certificate store — reads X.509 certificates from flash partitions.
//!
//! On ESP-IDF targets, certificates are stored in a dedicated
//! `certs` NVS partition (or embedded via `EMBED_FILES` in the
//! partition table). On simulation targets, certs are read from
//! the filesystem.
//!
//! ## Flash partition layout
//!
//! | Key              | Content                              |
//! |------------------|--------------------------------------|
//! | `server_cert`    | PEM-encoded server certificate       |
//! | `server_key`     | PEM-encoded private key              |
//! | `ca_cert`        | PEM-encoded CA certificate chain     |

use log::{info, warn};

/// Maximum certificate size (PEM format, includes headers).
const MAX_CERT_SIZE: usize = 4096;

/// Maximum private key size.
const MAX_KEY_SIZE: usize = 2048;

/// TLS authentication mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TlsMode {
    /// Pre-shared key only (default, no certificates needed).
    #[default]
    PskOnly,
    /// X.509 certificate authentication only (no PSK fallback).
    CertOnly,
    /// Dual-mode: try certificate auth first, fall back to PSK.
    PskAndCert,
}

/// Container for loaded certificate material.
pub struct CertBundle {
    /// Server certificate (PEM-encoded, NUL-terminated for mbedTLS).
    pub server_cert: heapless::Vec<u8, MAX_CERT_SIZE>,
    /// Server private key (PEM-encoded, NUL-terminated for mbedTLS).
    pub server_key: heapless::Vec<u8, MAX_KEY_SIZE>,
    /// CA certificate chain (PEM-encoded, NUL-terminated for mbedTLS).
    pub ca_cert: heapless::Vec<u8, MAX_CERT_SIZE>,
}

impl CertBundle {
    pub fn is_complete(&self) -> bool {
        !self.server_cert.is_empty() && !self.server_key.is_empty() && !self.ca_cert.is_empty()
    }
}

/// Certificate store adapter.
pub struct CertStore {
    mode: TlsMode,
}

impl CertStore {
    pub fn new(mode: TlsMode) -> Self {
        Self { mode }
    }

    pub fn mode(&self) -> TlsMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: TlsMode) {
        self.mode = mode;
    }

    /// Load certificate bundle from the platform store.
    ///
    /// Returns `None` if certificates are not available or mode is PskOnly.
    pub fn load_bundle(&self) -> Option<CertBundle> {
        if self.mode == TlsMode::PskOnly {
            return None;
        }

        let bundle = self.platform_load()?;

        if !bundle.is_complete() {
            warn!("CertStore: incomplete certificate bundle");
            return None;
        }

        info!(
            "CertStore: loaded certificate bundle (cert={}B, key={}B, ca={}B)",
            bundle.server_cert.len(),
            bundle.server_key.len(),
            bundle.ca_cert.len(),
        );

        Some(bundle)
    }

    /// Store a certificate component. Used during RPC-based cert provisioning.
    #[cfg(target_os = "espidf")]
    pub fn store_cert(&self, key: &str, data: &[u8]) -> Result<(), CertStoreError> {
        use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};

        let nvs_partition =
            EspNvsPartition::<NvsDefault>::take().map_err(|_| CertStoreError::PartitionNotFound)?;
        let mut nvs =
            EspNvs::new(nvs_partition, "certs", true).map_err(|_| CertStoreError::NvsError)?;
        nvs.set_blob(key, data)
            .map_err(|_| CertStoreError::WriteFailed)?;
        info!("CertStore: stored '{}' ({}B)", key, data.len());
        Ok(())
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn store_cert(&self, key: &str, _data: &[u8]) -> Result<(), CertStoreError> {
        info!("CertStore(sim): store_cert '{}' (no-op in simulation)", key);
        Ok(())
    }

    // ── Platform-specific loading ────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_load(&self) -> Option<CertBundle> {
        use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};

        let nvs_partition = EspNvsPartition::<NvsDefault>::take().ok()?;
        let nvs = EspNvs::new(nvs_partition, "certs", false).ok()?;

        let mut bundle = CertBundle {
            server_cert: heapless::Vec::new(),
            server_key: heapless::Vec::new(),
            ca_cert: heapless::Vec::new(),
        };

        let mut buf = [0u8; MAX_CERT_SIZE];
        if let Ok(Some(len)) = nvs.get_blob("server_cert", &mut buf) {
            let _ = bundle.server_cert.extend_from_slice(&buf[..len]);
        }

        let mut buf = [0u8; MAX_KEY_SIZE];
        if let Ok(Some(len)) = nvs.get_blob("server_key", &mut buf) {
            let _ = bundle.server_key.extend_from_slice(&buf[..len]);
        }

        let mut buf = [0u8; MAX_CERT_SIZE];
        if let Ok(Some(len)) = nvs.get_blob("ca_cert", &mut buf) {
            let _ = bundle.ca_cert.extend_from_slice(&buf[..len]);
        }

        Some(bundle)
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_load(&self) -> Option<CertBundle> {
        info!("CertStore(sim): platform_load returning None (certs not available in simulation)");
        None
    }
}

/// Errors from the certificate store.
#[derive(Debug)]
pub enum CertStoreError {
    PartitionNotFound,
    NvsError,
    WriteFailed,
}

impl core::fmt::Display for CertStoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PartitionNotFound => write!(f, "cert partition not found"),
            Self::NvsError => write!(f, "NVS initialization error"),
            Self::WriteFailed => write!(f, "cert write failed"),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psk_only_returns_none() {
        let store = CertStore::new(TlsMode::PskOnly);
        assert!(store.load_bundle().is_none());
    }

    #[test]
    fn default_mode_is_psk() {
        assert_eq!(TlsMode::default(), TlsMode::PskOnly);
    }

    #[test]
    fn cert_bundle_completeness() {
        let mut bundle = CertBundle {
            server_cert: heapless::Vec::new(),
            server_key: heapless::Vec::new(),
            ca_cert: heapless::Vec::new(),
        };
        assert!(!bundle.is_complete());

        let _ = bundle.server_cert.extend_from_slice(b"cert");
        let _ = bundle.server_key.extend_from_slice(b"key");
        assert!(!bundle.is_complete());

        let _ = bundle.ca_cert.extend_from_slice(b"ca");
        assert!(bundle.is_complete());
    }
}
