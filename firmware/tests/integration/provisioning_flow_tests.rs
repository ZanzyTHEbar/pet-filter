//! Integration tests for the BLE → WiFi provisioning flow.
//!
//! Verifies the end-to-end sequence: BLE adapter receives credentials,
//! validates them, makes them available via take_pending_credentials(),
//! and the PSK is stored separately.

use petfilter::app::service::AppService;
use petfilter::config::SystemConfig;
use petfilter::fsm::StateId;

use crate::mock_hw::LogSink;

// ── BLE provisioning adapter tests (simulation path) ──────────

#[test]
fn ble_provisioning_full_flow() {
    use petfilter_ble_sim::*;

    let mut name = heapless::String::<24>::new();
    name.push_str("pf-test").ok();
    let mut ble = BleAdapter::new(name);

    ble.start();
    assert!(ble.is_active());

    // Simulate GATT writes in correct order: SSID → password → PSK
    ble.on_ssid_write(b"TestNetwork").unwrap();
    ble.on_password_write(b"password1").unwrap();

    let psk = [0x42u8; 32];
    ble.on_psk_write(&psk).unwrap();

    // Trigger provisioning
    ble.on_trigger_write();

    // Verify credentials are available
    let (ssid, pass) = ble.take_pending_credentials().unwrap();
    assert_eq!(ssid.as_str(), "TestNetwork");
    assert_eq!(pass.as_str(), "password1");
    assert_eq!(ble.take_pending_psk(), Some(psk));

    // After taking, none should remain
    assert!(ble.take_pending_credentials().is_none());
    assert!(ble.take_pending_psk().is_none());
}

#[test]
fn ble_stop_clears_provisioning_state() {
    use petfilter_ble_sim::*;

    let mut name = heapless::String::<24>::new();
    name.push_str("pf-test").ok();
    let mut ble = BleAdapter::new(name);

    ble.start();
    ble.on_ssid_write(b"Net").unwrap();
    ble.on_password_write(b"password1").unwrap();
    ble.on_psk_write(&[0x01; 32]).unwrap();

    ble.stop();

    assert!(ble.take_pending_credentials().is_none());
    assert!(ble.take_pending_psk().is_none());
    assert!(!ble.is_active());
}

#[test]
fn ble_rejects_invalid_credentials() {
    use petfilter_ble_sim::*;

    let mut name = heapless::String::<24>::new();
    name.push_str("pf-test").ok();
    let mut ble = BleAdapter::new(name);

    // Empty SSID
    assert!(ble.on_ssid_write(b"").is_err());

    // Valid SSID, then short password
    ble.on_ssid_write(b"GoodSSID").unwrap();
    assert!(ble.on_password_write(b"short").is_err());

    // Wrong PSK length
    assert!(ble.on_psk_write(&[0xAA; 16]).is_err());
}

#[test]
fn app_survives_scrub_after_provisioning() {
    let config = SystemConfig::default();
    let mut app = AppService::new(config);
    let mut hw = crate::mock_hw::MockHardware::new();
    let mut sink = LogSink::new();

    app.start(&mut sink);
    assert_eq!(app.state(), StateId::Idle);

    // Simulating what main.rs does after successful provisioning:
    // app handles a scrub command.
    use petfilter::app::commands::AppCommand;
    app.handle_command(AppCommand::StartScrub, &mut hw, &mut sink);

    // Should have transitioned out of Idle
    assert_ne!(app.state(), StateId::Idle);
}

// Re-export BLE adapter under a test-friendly alias to avoid
// cfg confusion — the host-target BLE adapter uses simulation stubs.
mod petfilter_ble_sim {
    pub use petfilter::adapters::ble::*;
}
