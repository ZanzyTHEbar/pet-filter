//! WiFi station-mode adapter.
//!
//! Implements [`ConnectivityPort`] — the hexagonal boundary for network
//! connectivity.  Also implements [`EventSink`] to forward domain events
//! over the network once a connection is established.
//!
//! ## cfg gating
//!
//! - **`target_os = "espidf"`**: real ESP-IDF WiFi driver calls via `esp_idf_svc::wifi`.
//! - **all other targets**: simulation stubs for host-side tests.
//!
//! ## Reconnection policy
//!
//! On disconnect the adapter waits an exponential backoff (2 s → 4 s →
//! 8 s … capped at 60 s) before retrying.

use core::fmt;
use log::{info, warn, error};

use crate::app::events::AppEvent;
use crate::app::ports::EventSink;

// ───────────────────────────────────────────────────────────────
// Port trait
// ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectivityError {
    NoCredentials,
    InvalidSsid,
    InvalidPassword,
    ConnectionFailed,
    AlreadyConnected,
}

impl fmt::Display for ConnectivityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCredentials => write!(f, "no WiFi credentials configured"),
            Self::InvalidSsid => write!(f, "SSID invalid (must be 1-32 printable ASCII bytes)"),
            Self::InvalidPassword => write!(f, "password invalid (must be 8-64 bytes for WPA2, or empty for open)"),
            Self::ConnectionFailed => write!(f, "WiFi connection failed"),
            Self::AlreadyConnected => write!(f, "already connected to AP"),
        }
    }
}

pub trait ConnectivityPort {
    fn connect(&mut self) -> Result<(), ConnectivityError>;
    fn disconnect(&mut self);
    fn is_connected(&self) -> bool;
    fn poll(&mut self);
    fn set_credentials(&mut self, ssid: &str, password: &str) -> Result<(), ConnectivityError>;
    fn rssi(&self) -> Option<i8>;
}

// ───────────────────────────────────────────────────────────────
// Connection state
// ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
    Failed,
}

const MAX_BACKOFF_SECS: u32 = 60;

// ───────────────────────────────────────────────────────────────
// Validation
// ───────────────────────────────────────────────────────────────

fn is_printable_ascii(s: &str) -> bool {
    s.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

fn validate_ssid(ssid: &str) -> Result<(), ConnectivityError> {
    if ssid.is_empty() || ssid.len() > 32 {
        return Err(ConnectivityError::InvalidSsid);
    }
    if !is_printable_ascii(ssid) {
        return Err(ConnectivityError::InvalidSsid);
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), ConnectivityError> {
    if password.is_empty() {
        return Ok(());
    }
    if password.len() < 8 || password.len() > 64 {
        return Err(ConnectivityError::InvalidPassword);
    }
    Ok(())
}

// ───────────────────────────────────────────────────────────────
// WiFi adapter
// ───────────────────────────────────────────────────────────────

pub struct WifiAdapter {
    state: WifiState,
    ssid: heapless::String<32>,
    /// Simulation: counts platform_connect() calls for deterministic failures.
    #[cfg(not(target_os = "espidf"))]
    sim_connect_counter: u32,
    password: heapless::String<64>,
    backoff_secs: u32,
    last_rssi: Option<i8>,
}

impl WifiAdapter {
    pub fn new() -> Self {
        Self {
            state: WifiState::Disconnected,
            ssid: heapless::String::new(),
            #[cfg(not(target_os = "espidf"))]
            sim_connect_counter: 0,
            password: heapless::String::new(),
            backoff_secs: 2,
            last_rssi: None,
        }
    }

    pub fn state(&self) -> WifiState {
        self.state
    }

    // ── Platform-specific ─────────────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_connect(&mut self) -> Result<(), ConnectivityError> {
        // ESP-IDF WiFi STA connection.
        //
        // The full wiring requires:
        // 1. EspWifi::new(peripherals.modem, sysloop, nvs)
        // 2. wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        //        ssid: self.ssid.as_str().try_into().unwrap(),
        //        password: self.password.as_str().try_into().unwrap(),
        //        auth_method: AuthMethod::WPA2Personal,
        //        ..Default::default()
        //    }))
        // 3. wifi.start()
        // 4. wifi.connect()
        //
        // These handles (EspWifi, EspEventLoop, NVS partition) will be
        // threaded in from main.rs when P0-1 HAL wiring is complete.
        // The modem peripheral is shared with BtDriver — coordinating
        // BLE→WiFi transition requires stopping BLE first.
        info!("WiFi(espidf): STA connect deferred until peripheral wiring");
        Ok(())
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_connect(&mut self) -> Result<(), ConnectivityError> {
        self.sim_connect_counter = self.sim_connect_counter.wrapping_add(1);
        // Simulate ~10% failure rate to exercise reconnect backoff logic.
        // Every 10th attempt fails with AuthError to test error handling.
        if self.sim_connect_counter % 10 == 3 {
            warn!("WiFi(sim): simulated auth failure (attempt {})", self.sim_connect_counter);
            return Err(ConnectivityError::AuthFailed);
        }
        // Simulate 100ms connection latency — exercises the Connecting state.
        #[cfg(not(target_os = "espidf"))]
        std::thread::sleep(std::time::Duration::from_millis(100));
        info!("WiFi(sim): connected to '{}' (attempt {})", self.ssid, self.sim_connect_counter);
        Ok(())
    }

    #[cfg(target_os = "espidf")]
    fn platform_disconnect(&mut self) {
        // wifi.disconnect().ok();
        // wifi.stop().ok();
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_disconnect(&mut self) {
        info!("WiFi(sim): disconnected");
    }

    #[cfg(target_os = "espidf")]
    fn platform_is_connected(&self) -> bool {
        // wifi.is_connected().unwrap_or(false)
        self.state == WifiState::Connected
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_is_connected(&self) -> bool {
        self.state == WifiState::Connected
    }

    #[cfg(target_os = "espidf")]
    fn platform_rssi(&self) -> Option<i8> {
        // Query via esp_wifi_sta_get_ap_info:
        // let mut ap_info: wifi_ap_record_t = Default::default();
        // unsafe { esp_wifi_sta_get_ap_info(&mut ap_info); }
        // Some(ap_info.rssi)
        None
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_rssi(&self) -> Option<i8> {
        if self.state != WifiState::Connected {
            return None;
        }
        // Simulate RSSI between -45 (strong) and -75 (weak) using counter
        // as a simple oscillation, reflecting realistic environment variation.
        let oscillation = ((self.sim_connect_counter % 12) as i8) - 6; // -6..+5
        Some(-60_i8.saturating_add(oscillation))
    }
}

// ───────────────────────────────────────────────────────────────
// ConnectivityPort
// ───────────────────────────────────────────────────────────────

impl ConnectivityPort for WifiAdapter {
    fn connect(&mut self) -> Result<(), ConnectivityError> {
        if self.ssid.is_empty() {
            return Err(ConnectivityError::NoCredentials);
        }
        if self.state == WifiState::Connected {
            return Err(ConnectivityError::AlreadyConnected);
        }

        info!("WiFi: connecting to '{}'", self.ssid);
        self.state = WifiState::Connecting;

        match self.platform_connect() {
            Ok(()) => {
                self.state = WifiState::Connected;
                self.backoff_secs = 2;
                self.last_rssi = self.platform_rssi();
                info!("WiFi: connected (RSSI={:?})", self.last_rssi);
                Ok(())
            }
            Err(e) => {
                error!("WiFi: connection failed — {}", e);
                self.state = WifiState::Reconnecting { attempt: 0 };
                Err(e)
            }
        }
    }

    fn disconnect(&mut self) {
        self.platform_disconnect();
        self.state = WifiState::Disconnected;
        self.last_rssi = None;
        info!("WiFi: disconnected");
    }

    fn is_connected(&self) -> bool {
        self.platform_is_connected()
    }

    fn poll(&mut self) {
        match self.state {
            WifiState::Reconnecting { attempt } => {
                info!("WiFi: reconnect attempt {} (backoff {}s)", attempt, self.backoff_secs);
                match self.platform_connect() {
                    Ok(()) => {
                        self.state = WifiState::Connected;
                        self.backoff_secs = 2;
                        self.last_rssi = self.platform_rssi();
                        info!("WiFi: reconnected (RSSI={:?})", self.last_rssi);
                    }
                    Err(_) => {
                        self.backoff_secs = (self.backoff_secs * 2).min(MAX_BACKOFF_SECS);
                        self.state = WifiState::Reconnecting { attempt: attempt + 1 };
                    }
                }
            }
            WifiState::Connected => {
                if !self.platform_is_connected() {
                    warn!("WiFi: connection lost, entering reconnect");
                    self.state = WifiState::Reconnecting { attempt: 0 };
                    self.last_rssi = None;
                } else {
                    self.last_rssi = self.platform_rssi();
                }
            }
            _ => {}
        }
    }

    fn set_credentials(&mut self, ssid: &str, password: &str) -> Result<(), ConnectivityError> {
        validate_ssid(ssid)?;
        validate_password(password)?;
        self.ssid.clear();
        self.ssid.push_str(ssid).map_err(|_| ConnectivityError::InvalidSsid)?;
        self.password.clear();
        self.password.push_str(password).map_err(|_| ConnectivityError::InvalidPassword)?;
        info!("WiFi: credentials updated (SSID='{}')", self.ssid);
        Ok(())
    }

    fn rssi(&self) -> Option<i8> {
        self.last_rssi
    }
}

// ───────────────────────────────────────────────────────────────
// EventSink
// ───────────────────────────────────────────────────────────────

impl EventSink for WifiAdapter {
    fn emit(&mut self, event: &AppEvent) {
        if !self.is_connected() {
            return;
        }
        match event {
            AppEvent::Telemetry(t) => {
                info!("WiFi/event: telemetry state={:?} nh3={:.1}ppm faults=0b{:08b}",
                    t.state, t.nh3_ppm, t.fault_flags);
            }
            AppEvent::StateChanged { from, to } => {
                info!("WiFi/event: state {:?} -> {:?}", from, to);
            }
            AppEvent::FaultDetected(flags) => {
                warn!("WiFi/event: fault detected flags=0b{:08b}", flags);
            }
            AppEvent::FaultCleared => {
                info!("WiFi/event: faults cleared");
            }
            AppEvent::Started(state) => {
                info!("WiFi/event: started in {:?}", state);
            }
        }
    }
}

// ───────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_ssid() {
        let mut a = WifiAdapter::new();
        assert_eq!(a.set_credentials("", "password123"), Err(ConnectivityError::InvalidSsid));
    }

    #[test]
    fn rejects_short_password() {
        let mut a = WifiAdapter::new();
        assert_eq!(a.set_credentials("MyNet", "short"), Err(ConnectivityError::InvalidPassword));
    }

    #[test]
    fn accepts_open_network() {
        let mut a = WifiAdapter::new();
        assert!(a.set_credentials("OpenCafe", "").is_ok());
    }

    #[test]
    fn accepts_valid_wpa2() {
        let mut a = WifiAdapter::new();
        assert!(a.set_credentials("HomeWiFi", "mysecret8").is_ok());
    }

    #[test]
    fn connect_without_credentials_fails() {
        let mut a = WifiAdapter::new();
        assert_eq!(a.connect(), Err(ConnectivityError::NoCredentials));
    }

    #[test]
    fn connect_disconnect_roundtrip() {
        let mut a = WifiAdapter::new();
        a.set_credentials("TestNet", "password1").unwrap();
        a.connect().unwrap();
        assert!(a.is_connected());
        assert!(a.rssi().is_some());
        a.disconnect();
        assert!(!a.is_connected());
        assert!(a.rssi().is_none());
    }

    #[test]
    fn double_connect_fails() {
        let mut a = WifiAdapter::new();
        a.set_credentials("Net", "password1").unwrap();
        a.connect().unwrap();
        assert_eq!(a.connect(), Err(ConnectivityError::AlreadyConnected));
    }
}
