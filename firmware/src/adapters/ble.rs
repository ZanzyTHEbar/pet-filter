//! BLE provisioning adapter.
//!
//! Implements [`ProvisioningPort`] — the hexagonal boundary for Bluetooth
//! Low Energy device provisioning and status reporting.
//!
//! ## cfg gating
//!
//! - **`target_os = "espidf"`**: Bluedroid BLE GATT server via `esp_idf_svc::bt::ble`.
//! - **all other targets**: simulation stubs for host-side tests.
//!
//! ## GATT Service Layout
//!
//! | Characteristic       | UUID                                    | Perms       |
//! |----------------------|-----------------------------------------|-------------|
//! | WiFi SSID            | `4a650002-…-5f6c9a1d7e3a`               | Write       |
//! | WiFi Password        | `4a650003-…-5f6c9a1d7e3a`               | Write       |
//! | WiFi Trigger         | `4a650004-…-5f6c9a1d7e3a`               | Write+Notify|
//! | Device Status        | `4a650010-…-5f6c9a1d7e3a`               | Read+Notify |
//! | Configuration        | `4a650020-…-5f6c9a1d7e3a`               | Read+Write  |
//! | PSK Pairing          | `4a650030-…-5f6c9a1d7e3a`               | Write       |

use core::fmt;
use log::{info, warn, error};

// ───────────────────────────────────────────────────────────────
// Constants
// ───────────────────────────────────────────────────────────────

pub const SERVICE_UUID: u128 = 0x4a650001_b7e4_4b91_a032_5f6c9a1d7e3a;
pub const CHAR_WIFI_SSID: u128 = 0x4a650002_b7e4_4b91_a032_5f6c9a1d7e3a;
pub const CHAR_WIFI_PASS: u128 = 0x4a650003_b7e4_4b91_a032_5f6c9a1d7e3a;
pub const CHAR_WIFI_TRIGGER: u128 = 0x4a650004_b7e4_4b91_a032_5f6c9a1d7e3a;
pub const CHAR_STATUS: u128 = 0x4a650010_b7e4_4b91_a032_5f6c9a1d7e3a;
pub const CHAR_CONFIG: u128 = 0x4a650020_b7e4_4b91_a032_5f6c9a1d7e3a;
pub const CHAR_PSK_PAIRING: u128 = 0x4a650030_b7e4_4b91_a032_5f6c9a1d7e3a;

const MAX_STATUS_BYTES: usize = 64;
const MAX_SSID_LEN: usize = 32;
const MAX_PASSWORD_LEN: usize = 64;
const MIN_WPA2_PASSWORD_LEN: usize = 8;
const PSK_LEN: usize = 32;

// ───────────────────────────────────────────────────────────────
// Error types
// ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisioningError {
    StackInitFailed,
    DataTooLong,
    InvalidUtf8,
    InvalidSsid,
    InvalidPassword,
    InvalidPsk,
}

impl fmt::Display for ProvisioningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StackInitFailed => write!(f, "BLE stack initialisation failed"),
            Self::DataTooLong => write!(f, "BLE write exceeds max characteristic length"),
            Self::InvalidUtf8 => write!(f, "BLE write contains invalid UTF-8"),
            Self::InvalidSsid => write!(f, "SSID invalid (1-32 printable ASCII bytes)"),
            Self::InvalidPassword => write!(f, "password invalid (8-64 bytes for WPA2 or empty)"),
            Self::InvalidPsk => write!(f, "PSK must be exactly 32 bytes"),
        }
    }
}

// ───────────────────────────────────────────────────────────────
// Port trait
// ───────────────────────────────────────────────────────────────

pub trait ProvisioningPort {
    fn start(&mut self);
    fn stop(&mut self);
    fn is_active(&self) -> bool;
    fn take_pending_credentials(&mut self) -> Option<(heapless::String<32>, heapless::String<64>)>;
    fn take_pending_psk(&mut self) -> Option<[u8; PSK_LEN]>;
    fn update_status(&mut self, state_name: &str, nh3_ppm: f32, faults: u8);
}

// ───────────────────────────────────────────────────────────────
// BLE state
// ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BleState {
    Idle,
    Advertising,
    Connected,
    Failed,
}

// ───────────────────────────────────────────────────────────────
// Validation helpers
// ───────────────────────────────────────────────────────────────

fn is_printable_ascii(s: &str) -> bool {
    s.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

fn sanitize_ble_string(raw: &[u8], max_len: usize) -> Result<&str, ProvisioningError> {
    if raw.len() > max_len {
        return Err(ProvisioningError::DataTooLong);
    }
    core::str::from_utf8(raw).map_err(|_| ProvisioningError::InvalidUtf8)
}

fn validate_ssid(ssid: &str) -> Result<(), ProvisioningError> {
    if ssid.is_empty() || ssid.len() > MAX_SSID_LEN {
        return Err(ProvisioningError::InvalidSsid);
    }
    if !is_printable_ascii(ssid) {
        return Err(ProvisioningError::InvalidSsid);
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), ProvisioningError> {
    if password.is_empty() {
        return Ok(());
    }
    if password.len() < MIN_WPA2_PASSWORD_LEN || password.len() > MAX_PASSWORD_LEN {
        return Err(ProvisioningError::InvalidPassword);
    }
    Ok(())
}

fn validate_psk(raw: &[u8]) -> Result<[u8; PSK_LEN], ProvisioningError> {
    if raw.len() != PSK_LEN {
        return Err(ProvisioningError::InvalidPsk);
    }
    let mut psk = [0u8; PSK_LEN];
    psk.copy_from_slice(raw);
    Ok(psk)
}

// ───────────────────────────────────────────────────────────────
// BLE adapter
// ───────────────────────────────────────────────────────────────

pub struct BleAdapter {
    state: BleState,
    pending_ssid: Option<heapless::String<32>>,
    pending_password: Option<heapless::String<64>>,
    pending_psk: Option<[u8; PSK_LEN]>,
    status_buf: heapless::String<64>,
    device_name: heapless::String<24>,
}

impl BleAdapter {
    pub fn new(device_name: heapless::String<24>) -> Self {
        Self {
            state: BleState::Idle,
            pending_ssid: None,
            pending_password: None,
            pending_psk: None,
            status_buf: heapless::String::new(),
            #[cfg(not(target_os = "espidf"))]
            sim_provision_step: 0,
            device_name,
        }
    }

    pub fn state(&self) -> BleState {
        self.state
    }

    pub fn on_ssid_write(&mut self, raw: &[u8]) -> Result<(), ProvisioningError> {
        let s = sanitize_ble_string(raw, MAX_SSID_LEN)?;
        validate_ssid(s)?;
        let mut ssid = heapless::String::<32>::new();
        ssid.push_str(s).map_err(|_| ProvisioningError::InvalidSsid)?;
        self.pending_ssid = Some(ssid);
        info!("BLE: SSID written (len={})", s.len());
        Ok(())
    }

    pub fn on_password_write(&mut self, raw: &[u8]) -> Result<(), ProvisioningError> {
        let s = sanitize_ble_string(raw, MAX_PASSWORD_LEN)?;
        validate_password(s)?;
        let mut pw = heapless::String::<64>::new();
        pw.push_str(s).map_err(|_| ProvisioningError::InvalidPassword)?;
        self.pending_password = Some(pw);
        info!("BLE: password written (len={})", s.len());
        Ok(())
    }

    pub fn on_psk_write(&mut self, raw: &[u8]) -> Result<(), ProvisioningError> {
        let psk = validate_psk(raw)?;
        self.pending_psk = Some(psk);
        info!("BLE: PSK pairing key written");
        Ok(())
    }

    pub fn on_trigger_write(&mut self) {
        info!("BLE: WiFi provisioning triggered");
    }

    pub fn on_central_connected(&mut self) {
        info!("BLE: central connected");
        self.state = BleState::Connected;
    }

    pub fn on_central_disconnected(&mut self) {
        info!("BLE: central disconnected");
        if self.state != BleState::Idle {
            self.state = BleState::Advertising;
        }
    }

    // ── Platform-specific ─────────────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_start(&mut self) {
        // ESP-IDF: BtDriver + EspBleGap + EspGatts initialization.
        //
        // This requires 'static lifetimes and Arc/Mutex for the callback
        // closures. The full wiring follows the bt_gatt_server example
        // from esp-idf-svc:
        //
        // 1. BtDriver::new(peripherals.modem, nvs)
        // 2. EspBleGap::new(bt.clone()) → subscribe gap events
        // 3. EspGatts::new(bt.clone()) → subscribe gatts events
        // 4. Register app (APP_ID = 0)
        // 5. On GattsEvent::ServiceRegistered → create service
        // 6. On GattsEvent::ServiceCreated → add characteristics
        // 7. On GattsEvent::CharacteristicAdded → store handle
        // 8. On write events → dispatch to on_ssid_write/on_password_write/etc.
        //
        // The Bluedroid stack requires ~30 KB RAM and these sdkconfig settings:
        //   CONFIG_BT_ENABLED=y
        //   CONFIG_BT_BLUEDROID_ENABLED=y
        //   CONFIG_BT_CLASSIC_ENABLED=n
        //   CONFIG_BTDM_CTRL_MODE_BLE_ONLY=y
        //
        // Actual peripheral and NVS handles will be threaded through
        // from main.rs when P0-1 HAL integration is complete. For now,
        // the state machine and data flow are fully defined; only the
        // BtDriver/EspBleGap/EspGatts construction is deferred.
        info!("BLE(espidf): GATT server init deferred until peripheral wiring");
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_start(&mut self) {
        info!("BLE(sim): advertising '{}' (service {:032x})", self.device_name, SERVICE_UUID);
    }

    #[cfg(target_os = "espidf")]
    fn platform_stop(&mut self) {
        // Stop advertising, deinit BtDriver
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_stop(&mut self) {
        info!("BLE(sim): stopped");
    }

    #[cfg(target_os = "espidf")]
    fn platform_update_status(&mut self, _payload: &str) {
        // Write to CHAR_STATUS characteristic value and send notification
        // to all subscribed centrals.
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_update_status(&mut self, payload: &str) {
        info!("BLE(sim): status updated — {}", payload);
    }
}

// ───────────────────────────────────────────────────────────────
// ProvisioningPort implementation
// ───────────────────────────────────────────────────────────────

impl ProvisioningPort for BleAdapter {
    fn start(&mut self) {
        info!("BLE: starting advertising as '{}'", self.device_name);
        self.platform_start();
        self.state = BleState::Advertising;
    }

    fn stop(&mut self) {
        self.platform_stop();
        self.state = BleState::Idle;
        self.pending_ssid = None;
        self.pending_password = None;
        self.pending_psk = None;
        info!("BLE: stopped");
    }

    fn is_active(&self) -> bool {
        matches!(self.state, BleState::Advertising | BleState::Connected)
    }

    fn take_pending_credentials(&mut self) -> Option<(heapless::String<32>, heapless::String<64>)> {
        let ssid = self.pending_ssid.take()?;
        let password = self.pending_password.take().unwrap_or_else(heapless::String::new);
        Some((ssid, password))
    }

    fn take_pending_psk(&mut self) -> Option<[u8; PSK_LEN]> {
        self.pending_psk.take()
    }

    fn update_status(&mut self, state_name: &str, nh3_ppm: f32, faults: u8) {
        self.status_buf.clear();
        use core::fmt::Write;
        let _ = write!(
            self.status_buf,
            "{{\"s\":\"{}\",\"nh3\":{:.1},\"f\":{}}}",
            state_name, nh3_ppm, faults,
        );

        let payload_str = self.status_buf.clone();
        if payload_str.len() > MAX_STATUS_BYTES {
            warn!("BLE: status payload truncated ({} > {})", payload_str.len(), MAX_STATUS_BYTES);
        }

        self.platform_update_status(&payload_str);
    }
}

// ───────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_adapter() -> BleAdapter {
        let mut name = heapless::String::<24>::new();
        name.push_str("petfilter-test").ok();
        BleAdapter::new(name)
    }

    #[test]
    fn start_stop_lifecycle() {
        let mut adapter = make_adapter();
        assert_eq!(adapter.state(), BleState::Idle);
        assert!(!adapter.is_active());
        adapter.start();
        assert_eq!(adapter.state(), BleState::Advertising);
        assert!(adapter.is_active());
        adapter.stop();
        assert_eq!(adapter.state(), BleState::Idle);
    }

    #[test]
    fn connection_state_callbacks() {
        let mut adapter = make_adapter();
        adapter.start();
        adapter.on_central_connected();
        assert_eq!(adapter.state(), BleState::Connected);
        adapter.on_central_disconnected();
        assert_eq!(adapter.state(), BleState::Advertising);
    }

    #[test]
    fn valid_ssid_write() {
        let mut adapter = make_adapter();
        assert!(adapter.on_ssid_write(b"HomeWiFi").is_ok());
    }

    #[test]
    fn rejects_empty_ssid() {
        let mut adapter = make_adapter();
        assert_eq!(adapter.on_ssid_write(b""), Err(ProvisioningError::InvalidSsid));
    }

    #[test]
    fn rejects_ssid_too_long() {
        let mut adapter = make_adapter();
        assert_eq!(adapter.on_ssid_write(&[b'A'; 33]), Err(ProvisioningError::DataTooLong));
    }

    #[test]
    fn valid_password_write() {
        let mut adapter = make_adapter();
        assert!(adapter.on_password_write(b"mysecret8").is_ok());
    }

    #[test]
    fn accepts_empty_password_for_open() {
        let mut adapter = make_adapter();
        assert!(adapter.on_password_write(b"").is_ok());
    }

    #[test]
    fn rejects_short_password() {
        let mut adapter = make_adapter();
        assert_eq!(adapter.on_password_write(b"short"), Err(ProvisioningError::InvalidPassword));
    }

    #[test]
    fn valid_psk_write() {
        let mut adapter = make_adapter();
        let psk = [0xAA; 32];
        assert!(adapter.on_psk_write(&psk).is_ok());
    }

    #[test]
    fn rejects_wrong_length_psk() {
        let mut adapter = make_adapter();
        assert_eq!(adapter.on_psk_write(&[0xBB; 16]), Err(ProvisioningError::InvalidPsk));
        assert_eq!(adapter.on_psk_write(&[0xCC; 33]), Err(ProvisioningError::InvalidPsk));
    }

    #[test]
    fn take_credentials_roundtrip() {
        let mut adapter = make_adapter();
        assert!(adapter.take_pending_credentials().is_none());
        adapter.on_ssid_write(b"TestNet").unwrap();
        adapter.on_password_write(b"password1").unwrap();
        adapter.on_trigger_write();
        let (ssid, pw) = adapter.take_pending_credentials().unwrap();
        assert_eq!(ssid.as_str(), "TestNet");
        assert_eq!(pw.as_str(), "password1");
        assert!(adapter.take_pending_credentials().is_none());
    }

    #[test]
    fn take_psk_roundtrip() {
        let mut adapter = make_adapter();
        assert!(adapter.take_pending_psk().is_none());
        let key = [0x42; 32];
        adapter.on_psk_write(&key).unwrap();
        assert_eq!(adapter.take_pending_psk(), Some(key));
        assert!(adapter.take_pending_psk().is_none());
    }

    #[test]
    fn stop_clears_all_pending() {
        let mut adapter = make_adapter();
        adapter.start();
        adapter.on_ssid_write(b"Net").unwrap();
        adapter.on_psk_write(&[0x01; 32]).unwrap();
        adapter.stop();
        assert!(adapter.take_pending_credentials().is_none());
        assert!(adapter.take_pending_psk().is_none());
    }

    #[test]
    fn update_status_payload() {
        let mut adapter = make_adapter();
        adapter.start();
        adapter.update_status("Scrubbing", 12.5, 0);
    }
}
