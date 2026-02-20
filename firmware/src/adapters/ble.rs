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

use super::utils::is_printable_ascii;
use core::fmt;
use log::{info, warn};

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

// is_printable_ascii is provided by super::utils

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

// ── ESP-IDF BLE static state (ISR-safe atomics) ───────────────
//
// Bluedroid callbacks are C function pointers that cannot capture Rust
// closures. These atomics bridge the callback context to the adapter.

#[cfg(target_os = "espidf")]
use core::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

#[cfg(target_os = "espidf")]
static BLE_GATTS_IF: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "espidf")]
static BLE_CONN_ID: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "espidf")]
static BLE_STATUS_CHAR_HANDLE: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "espidf")]
static BLE_SSID_CHAR_HANDLE: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "espidf")]
static BLE_PASS_CHAR_HANDLE: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "espidf")]
static BLE_PSK_CHAR_HANDLE: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "espidf")]
static BLE_SVC_HANDLE: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "espidf")]
static BLE_CHAR_STEP: AtomicU32 = AtomicU32::new(0);

// Data buffers bridging GATTS write callback → BleAdapter.
// GATTS callbacks run in the Bluedroid task (not ISR), so std Mutex is safe.
#[cfg(target_os = "espidf")]
static BLE_SSID_BUF: std::sync::Mutex<heapless::Vec<u8, 32>> =
    std::sync::Mutex::new(heapless::Vec::new());
#[cfg(target_os = "espidf")]
static BLE_PASS_BUF: std::sync::Mutex<heapless::Vec<u8, 64>> =
    std::sync::Mutex::new(heapless::Vec::new());
#[cfg(target_os = "espidf")]
static BLE_PSK_BUF: std::sync::Mutex<heapless::Vec<u8, 32>> =
    std::sync::Mutex::new(heapless::Vec::new());

#[cfg(target_os = "espidf")]
fn uuid128_to_esp(uuid: u128) -> esp_idf_svc::sys::esp_bt_uuid_t {
    let mut t: esp_idf_svc::sys::esp_bt_uuid_t = unsafe { core::mem::zeroed() };
    t.len = 16;
    unsafe {
        t.uuid.uuid128 = uuid.to_le_bytes();
    }
    t
}

#[cfg(target_os = "espidf")]
unsafe fn add_gatt_char(svc_handle: u16, uuid: u128, perm: u32, prop: u32) {
    use esp_idf_svc::sys::*;
    let mut char_uuid = uuid128_to_esp(uuid);
    esp_ble_gatts_add_char(
        svc_handle,
        &mut char_uuid,
        perm as esp_gatt_perm_t,
        prop as esp_gatt_char_prop_t,
        core::ptr::null_mut(),
        core::ptr::null_mut(),
    );
}

/// Consume SSID bytes written by a BLE client via GATT.
#[cfg(target_os = "espidf")]
pub fn take_ssid_data() -> Option<heapless::Vec<u8, 32>> {
    BLE_SSID_BUF.lock().ok().and_then(|mut buf| {
        if buf.is_empty() {
            return None;
        }
        let data = buf.clone();
        buf.clear();
        Some(data)
    })
}

/// Consume password bytes written by a BLE client via GATT.
#[cfg(target_os = "espidf")]
pub fn take_pass_data() -> Option<heapless::Vec<u8, 64>> {
    BLE_PASS_BUF.lock().ok().and_then(|mut buf| {
        if buf.is_empty() {
            return None;
        }
        let data = buf.clone();
        buf.clear();
        Some(data)
    })
}

/// Consume PSK bytes written by a BLE client via GATT.
#[cfg(target_os = "espidf")]
pub fn take_psk_data() -> Option<heapless::Vec<u8, 32>> {
    BLE_PSK_BUF.lock().ok().and_then(|mut buf| {
        if buf.is_empty() {
            return None;
        }
        let data = buf.clone();
        buf.clear();
        Some(data)
    })
}

#[cfg(not(target_os = "espidf"))]
pub fn take_ssid_data() -> Option<heapless::Vec<u8, 32>> {
    None
}
#[cfg(not(target_os = "espidf"))]
pub fn take_pass_data() -> Option<heapless::Vec<u8, 64>> {
    None
}
#[cfg(not(target_os = "espidf"))]
pub fn take_psk_data() -> Option<heapless::Vec<u8, 32>> {
    None
}

#[cfg(target_os = "espidf")]
unsafe extern "C" fn ble_gap_event_handler(
    event: esp_idf_svc::sys::esp_gap_ble_cb_event_t,
    param: *mut esp_idf_svc::sys::esp_ble_gap_cb_param_t,
) {
    use esp_idf_svc::sys::*;
    match event {
        esp_gap_ble_cb_event_t_ESP_GAP_BLE_ADV_START_COMPLETE_EVT => {
            log::info!("BLE GAP: advertising started");
        }
        esp_gap_ble_cb_event_t_ESP_GAP_BLE_ADV_STOP_COMPLETE_EVT => {
            log::info!("BLE GAP: advertising stopped");
        }
        esp_gap_ble_cb_event_t_ESP_GAP_BLE_SEC_REQ_EVT => {
            esp_ble_gap_security_rsp((*param).ble_security.ble_req.bd_addr.as_mut_ptr(), true);
        }
        esp_gap_ble_cb_event_t_ESP_GAP_BLE_AUTH_CMPL_EVT => {
            let p = &(*param).ble_security.auth_cmpl;
            if p.success {
                log::info!("BLE GAP: authentication complete (bonded)");
            } else {
                log::warn!("BLE GAP: authentication failed (reason={})", p.fail_reason);
            }
        }
        _ => {}
    }
}

#[cfg(target_os = "espidf")]
unsafe extern "C" fn ble_gatts_event_handler(
    event: esp_idf_svc::sys::esp_gatts_cb_event_t,
    gatts_if: esp_idf_svc::sys::esp_gatt_if_t,
    param: *mut esp_idf_svc::sys::esp_ble_gatts_cb_param_t,
) {
    use esp_idf_svc::sys::*;

    BLE_GATTS_IF.store(gatts_if as u32, AtomicOrdering::Relaxed);

    match event {
        esp_gatts_cb_event_t_ESP_GATTS_REG_EVT => {
            log::info!("BLE GATTS: app registered (if={})", gatts_if);
            let svc_uuid = uuid128_to_esp(SERVICE_UUID);
            let mut svc_id = esp_gatt_srvc_id_t {
                id: esp_gatt_id_t {
                    uuid: svc_uuid,
                    inst_id: 0,
                },
                is_primary: true,
            };
            esp_ble_gatts_create_service(gatts_if, &mut svc_id, 12);
        }
        esp_gatts_cb_event_t_ESP_GATTS_CREATE_EVT => {
            let p = &(*param).create;
            let svc_handle = p.service_handle;
            BLE_SVC_HANDLE.store(svc_handle as u32, AtomicOrdering::Relaxed);
            log::info!("BLE GATTS: service created (handle={})", svc_handle);
            esp_ble_gatts_start_service(svc_handle);
            BLE_CHAR_STEP.store(1, AtomicOrdering::Relaxed);
            add_gatt_char(
                svc_handle,
                CHAR_WIFI_SSID,
                ESP_GATT_PERM_WRITE,
                ESP_GATT_CHAR_PROP_BIT_WRITE,
            );
        }
        esp_gatts_cb_event_t_ESP_GATTS_ADD_CHAR_EVT => {
            let p = &(*param).add_char;
            let handle = p.attr_handle;
            let step = BLE_CHAR_STEP.load(AtomicOrdering::Relaxed);
            let svc_handle = BLE_SVC_HANDLE.load(AtomicOrdering::Relaxed) as u16;
            match step {
                1 => {
                    BLE_SSID_CHAR_HANDLE.store(handle as u32, AtomicOrdering::Relaxed);
                    log::info!("BLE GATTS: SSID char (handle={})", handle);
                    BLE_CHAR_STEP.store(2, AtomicOrdering::Relaxed);
                    add_gatt_char(
                        svc_handle,
                        CHAR_WIFI_PASS,
                        ESP_GATT_PERM_WRITE,
                        ESP_GATT_CHAR_PROP_BIT_WRITE,
                    );
                }
                2 => {
                    BLE_PASS_CHAR_HANDLE.store(handle as u32, AtomicOrdering::Relaxed);
                    log::info!("BLE GATTS: password char (handle={})", handle);
                    BLE_CHAR_STEP.store(3, AtomicOrdering::Relaxed);
                    add_gatt_char(
                        svc_handle,
                        CHAR_PSK_PAIRING,
                        ESP_GATT_PERM_WRITE,
                        ESP_GATT_CHAR_PROP_BIT_WRITE,
                    );
                }
                3 => {
                    BLE_PSK_CHAR_HANDLE.store(handle as u32, AtomicOrdering::Relaxed);
                    log::info!("BLE GATTS: PSK char (handle={})", handle);
                    BLE_CHAR_STEP.store(4, AtomicOrdering::Relaxed);
                    add_gatt_char(
                        svc_handle,
                        CHAR_STATUS,
                        ESP_GATT_PERM_READ,
                        ESP_GATT_CHAR_PROP_BIT_READ | ESP_GATT_CHAR_PROP_BIT_NOTIFY,
                    );
                }
                4 => {
                    BLE_STATUS_CHAR_HANDLE.store(handle as u32, AtomicOrdering::Relaxed);
                    BLE_CHAR_STEP.store(5, AtomicOrdering::Relaxed);
                    log::info!(
                        "BLE GATTS: status char (handle={}) — all registered",
                        handle
                    );
                }
                _ => {}
            }
        }
        esp_gatts_cb_event_t_ESP_GATTS_CONNECT_EVT => {
            let p = unsafe { &(*param).connect };
            BLE_CONN_ID.store(p.conn_id as u32, AtomicOrdering::Relaxed);
            log::info!("BLE GATTS: client connected (conn_id={})", p.conn_id);
            crate::events::push_event(crate::events::Event::BleConnected);
        }
        esp_gatts_cb_event_t_ESP_GATTS_DISCONNECT_EVT => {
            BLE_CONN_ID.store(0, AtomicOrdering::Relaxed);
            log::info!("BLE GATTS: client disconnected");
            crate::events::push_event(crate::events::Event::BleDisconnected);
            // Restart advertising after disconnect.
            let mut adv_params = esp_ble_adv_params_t {
                adv_int_min: 0x20,
                adv_int_max: 0x40,
                adv_type: esp_ble_adv_type_t_ADV_TYPE_IND,
                own_addr_type: esp_ble_addr_type_t_BLE_ADDR_TYPE_PUBLIC,
                channel_map: esp_ble_adv_channel_t_ADV_CHNL_ALL,
                adv_filter_policy: esp_ble_adv_filter_t_ADV_FILTER_ALLOW_SCAN_ANY_CON_ANY,
                ..core::mem::zeroed()
            };
            esp_ble_gap_start_advertising(&mut adv_params);
        }
        esp_gatts_cb_event_t_ESP_GATTS_WRITE_EVT => {
            let p = unsafe { &(*param).write };
            let handle = p.handle as u32;
            let data = unsafe { core::slice::from_raw_parts(p.value, p.len as usize) };

            if handle == BLE_SSID_CHAR_HANDLE.load(AtomicOrdering::Relaxed) {
                if let Ok(mut buf) = BLE_SSID_BUF.lock() {
                    buf.clear();
                    let _ = buf.extend_from_slice(data);
                }
                crate::events::push_event(crate::events::Event::BleSsidWrite);
            } else if handle == BLE_PASS_CHAR_HANDLE.load(AtomicOrdering::Relaxed) {
                if let Ok(mut buf) = BLE_PASS_BUF.lock() {
                    buf.clear();
                    let _ = buf.extend_from_slice(data);
                }
                crate::events::push_event(crate::events::Event::BlePasswordWrite);
            } else if handle == BLE_PSK_CHAR_HANDLE.load(AtomicOrdering::Relaxed) {
                if let Ok(mut buf) = BLE_PSK_BUF.lock() {
                    buf.clear();
                    let _ = buf.extend_from_slice(data);
                }
                crate::events::push_event(crate::events::Event::BlePskWrite);
            }
        }
        _ => {}
    }
}

pub struct BleAdapter {
    state: BleState,
    pending_ssid: Option<heapless::String<32>>,
    pending_password: Option<heapless::String<64>>,
    pending_psk: Option<[u8; PSK_LEN]>,
    status_buf: heapless::String<64>,
    device_name: heapless::String<24>,
    /// Simulation: tracks provisioning write sequence (0=idle, 1=ssid, 2=pass, 3=psk).
    /// Used to enforce correct SSID→Password→PSK order and detect timeouts.
    #[cfg(not(target_os = "espidf"))]
    sim_provision_step: u8,
    /// Simulation: ticks since last write; pairing timeout if > 60.
    #[cfg(not(target_os = "espidf"))]
    sim_pairing_ticks: u32,
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
            #[cfg(not(target_os = "espidf"))]
            sim_pairing_ticks: 0,
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
        ssid.push_str(s)
            .map_err(|_| ProvisioningError::InvalidSsid)?;
        self.pending_ssid = Some(ssid);
        #[cfg(not(target_os = "espidf"))]
        {
            self.sim_provision_step = 1;
            self.sim_pairing_ticks = 0;
        }
        info!("BLE: SSID written (len={})", s.len());
        Ok(())
    }

    pub fn on_password_write(&mut self, raw: &[u8]) -> Result<(), ProvisioningError> {
        #[cfg(not(target_os = "espidf"))]
        if self.sim_provision_step != 1 {
            warn!(
                "BLE(sim): out-of-order write — password before SSID (step={})",
                self.sim_provision_step
            );
            return Err(ProvisioningError::InvalidPassword);
        }
        let s = sanitize_ble_string(raw, MAX_PASSWORD_LEN)?;
        validate_password(s)?;
        let mut pw = heapless::String::<64>::new();
        pw.push_str(s)
            .map_err(|_| ProvisioningError::InvalidPassword)?;
        self.pending_password = Some(pw);
        #[cfg(not(target_os = "espidf"))]
        {
            self.sim_provision_step = 2;
        }
        info!("BLE: password written (len={})", s.len());
        Ok(())
    }

    pub fn on_psk_write(&mut self, raw: &[u8]) -> Result<(), ProvisioningError> {
        #[cfg(not(target_os = "espidf"))]
        if self.sim_provision_step != 2 {
            warn!(
                "BLE(sim): out-of-order write — PSK before password (step={})",
                self.sim_provision_step
            );
            return Err(ProvisioningError::InvalidPsk);
        }
        let psk = validate_psk(raw)?;
        self.pending_psk = Some(psk);
        #[cfg(not(target_os = "espidf"))]
        {
            self.sim_provision_step = 3;
        }
        info!("BLE: PSK pairing key written");
        Ok(())
    }

    pub fn on_trigger_write(&mut self) {
        info!("BLE: WiFi provisioning triggered");
    }

    pub fn on_central_connected(&mut self) {
        info!("BLE: central connected");
        self.state = BleState::Connected;
        #[cfg(not(target_os = "espidf"))]
        {
            self.sim_provision_step = 0;
            self.sim_pairing_ticks = 0;
        }
    }

    /// Advance the pairing timeout counter (call from main loop tick).
    /// Returns `true` if the pairing session timed out (>60 ticks without writing SSID).
    #[cfg(not(target_os = "espidf"))]
    pub fn sim_tick_pairing_timeout(&mut self) -> bool {
        if self.state != BleState::Connected || self.sim_provision_step == 0 {
            return false;
        }
        self.sim_pairing_ticks = self.sim_pairing_ticks.wrapping_add(1);
        if self.sim_pairing_ticks > 60 {
            warn!("BLE(sim): pairing timeout — no credentials after 60 ticks");
            self.sim_provision_step = 0;
            self.sim_pairing_ticks = 0;
            self.pending_ssid = None;
            self.pending_password = None;
            return true;
        }
        false
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
        use esp_idf_svc::sys::*;
        unsafe {
            // Release classic BT memory (BLE-only mode saves ~30 KB).
            esp_bt_controller_mem_release(esp_bt_mode_t_ESP_BT_MODE_CLASSIC_BT);

            let bt_cfg = esp_bt_controller_config_t::default();
            let mut bt_cfg = bt_cfg;
            let ret = esp_bt_controller_init(&mut bt_cfg);
            if ret != ESP_OK as i32 {
                error!("BLE: bt_controller_init failed ({})", ret);
                self.state = BleState::Failed;
                return;
            }

            let ret = esp_bt_controller_enable(esp_bt_mode_t_ESP_BT_MODE_BLE);
            if ret != ESP_OK as i32 {
                error!("BLE: bt_controller_enable failed ({})", ret);
                self.state = BleState::Failed;
                return;
            }

            let ret = esp_bluedroid_init();
            if ret != ESP_OK as i32 {
                error!("BLE: bluedroid_init failed ({})", ret);
                self.state = BleState::Failed;
                return;
            }

            let ret = esp_bluedroid_enable();
            if ret != ESP_OK as i32 {
                error!("BLE: bluedroid_enable failed ({})", ret);
                self.state = BleState::Failed;
                return;
            }

            // Register GAP and GATTS callbacks.
            // The actual event dispatching uses static callback functions
            // that post events to the main event queue for processing.
            esp_ble_gap_register_callback(Some(ble_gap_event_handler));
            esp_ble_gatts_register_callback(Some(ble_gatts_event_handler));
            esp_ble_gatts_app_register(0);

            // Configure BLE security: just-works pairing with bonding.
            let auth_req = esp_ble_auth_req_t_ESP_LE_AUTH_REQ_SC_BOND;
            let iocap = esp_ble_io_cap_t_ESP_IO_CAP_NONE;
            let key_size: u8 = 16;
            let init_key: u8 = (ESP_BLE_ENC_KEY_MASK | ESP_BLE_ID_KEY_MASK) as u8;
            let rsp_key: u8 = (ESP_BLE_ENC_KEY_MASK | ESP_BLE_ID_KEY_MASK) as u8;
            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_AUTHEN_REQ_MODE,
                &auth_req as *const _ as *mut _,
                core::mem::size_of_val(&auth_req) as u32,
            );
            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_IOCAP_MODE,
                &iocap as *const _ as *mut _,
                core::mem::size_of_val(&iocap) as u32,
            );
            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_MAX_KEY_SIZE,
                &key_size as *const _ as *mut _,
                1,
            );
            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_SET_INIT_KEY,
                &init_key as *const _ as *mut _,
                1,
            );
            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_SET_RSP_KEY,
                &rsp_key as *const _ as *mut _,
                1,
            );

            // Set device name for advertising.
            let name = self.device_name.as_bytes();
            esp_ble_gap_set_device_name(name.as_ptr() as *const _);

            // Configure advertising parameters.
            let mut adv_params = esp_ble_adv_params_t {
                adv_int_min: 0x20,
                adv_int_max: 0x40,
                adv_type: esp_ble_adv_type_t_ADV_TYPE_IND,
                own_addr_type: esp_ble_addr_type_t_BLE_ADDR_TYPE_PUBLIC,
                channel_map: esp_ble_adv_channel_t_ADV_CHNL_ALL,
                adv_filter_policy: esp_ble_adv_filter_t_ADV_FILTER_ALLOW_SCAN_ANY_CON_ANY,
                ..core::mem::zeroed()
            };
            esp_ble_gap_start_advertising(&mut adv_params);

            info!(
                "BLE(espidf): Bluedroid stack initialized, advertising as '{}'",
                self.device_name
            );
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_start(&mut self) {
        info!(
            "BLE(sim): advertising '{}' (service {:032x})",
            self.device_name, SERVICE_UUID
        );
    }

    #[cfg(target_os = "espidf")]
    fn platform_stop(&mut self) {
        use esp_idf_svc::sys::*;
        unsafe {
            esp_ble_gap_stop_advertising();
            esp_bluedroid_disable();
            esp_bluedroid_deinit();
            esp_bt_controller_disable();
            esp_bt_controller_deinit();
        }
        info!("BLE(espidf): stack shut down");
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_stop(&mut self) {
        info!("BLE(sim): stopped");
    }

    #[cfg(target_os = "espidf")]
    fn platform_update_status(&mut self, payload: &str) {
        use esp_idf_svc::sys::*;
        unsafe {
            let handle = BLE_STATUS_CHAR_HANDLE.load(core::sync::atomic::Ordering::Relaxed);
            let conn = BLE_CONN_ID.load(core::sync::atomic::Ordering::Relaxed);
            if handle != 0 && conn != 0 {
                esp_ble_gatts_send_indicate(
                    BLE_GATTS_IF.load(core::sync::atomic::Ordering::Relaxed) as u8,
                    conn as u16,
                    handle as u16,
                    payload.len() as u16,
                    payload.as_ptr() as *mut u8,
                    false,
                );
            }
        }
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
        #[cfg(not(target_os = "espidf"))]
        {
            self.sim_provision_step = 0;
            self.sim_pairing_ticks = 0;
        }
        info!("BLE: stopped");
    }

    fn is_active(&self) -> bool {
        matches!(self.state, BleState::Advertising | BleState::Connected)
    }

    fn take_pending_credentials(&mut self) -> Option<(heapless::String<32>, heapless::String<64>)> {
        let ssid = self.pending_ssid.take()?;
        let password = self.pending_password.take().unwrap_or_default();
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
            warn!(
                "BLE: status payload truncated ({} > {})",
                payload_str.len(),
                MAX_STATUS_BYTES
            );
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
        assert_eq!(
            adapter.on_ssid_write(b""),
            Err(ProvisioningError::InvalidSsid)
        );
    }

    #[test]
    fn rejects_ssid_too_long() {
        let mut adapter = make_adapter();
        assert_eq!(
            adapter.on_ssid_write(&[b'A'; 33]),
            Err(ProvisioningError::DataTooLong)
        );
    }

    #[test]
    fn valid_password_write() {
        let mut adapter = make_adapter();
        adapter.on_ssid_write(b"TestNetwork").unwrap();
        assert!(adapter.on_password_write(b"mysecret8").is_ok());
    }

    #[test]
    fn accepts_empty_password_for_open() {
        let mut adapter = make_adapter();
        adapter.on_ssid_write(b"TestNetwork").unwrap();
        assert!(adapter.on_password_write(b"").is_ok());
    }

    #[test]
    fn rejects_short_password() {
        let mut adapter = make_adapter();
        adapter.on_ssid_write(b"TestNetwork").unwrap();
        assert_eq!(
            adapter.on_password_write(b"short"),
            Err(ProvisioningError::InvalidPassword)
        );
    }

    #[test]
    fn valid_psk_write() {
        let mut adapter = make_adapter();
        adapter.on_ssid_write(b"TestNetwork").unwrap();
        adapter.on_password_write(b"password1").unwrap();
        let psk = [0xAA; 32];
        assert!(adapter.on_psk_write(&psk).is_ok());
    }

    #[test]
    fn rejects_wrong_length_psk() {
        let mut adapter = make_adapter();
        adapter.on_ssid_write(b"TestNetwork").unwrap();
        adapter.on_password_write(b"password1").unwrap();
        assert_eq!(
            adapter.on_psk_write(&[0xBB; 16]),
            Err(ProvisioningError::InvalidPsk)
        );
        assert_eq!(
            adapter.on_psk_write(&[0xCC; 33]),
            Err(ProvisioningError::InvalidPsk)
        );
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
        adapter.on_ssid_write(b"TestNetwork").unwrap();
        adapter.on_password_write(b"password1").unwrap();
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
        adapter.on_password_write(b"password1").unwrap();
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
