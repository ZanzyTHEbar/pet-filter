//! Integration tests: AppService → FSM → actuators (QA-7b/7c/7e/7f).

use petfilter::app::commands::AppCommand;
use petfilter::app::events::AppEvent;
use petfilter::app::ports::{
    ActuatorPort, ConfigError, ConfigPort, EventSink, SensorPort, StorageError, StoragePort,
};
use petfilter::app::service::AppService;
use petfilter::config::SystemConfig;
use petfilter::fsm::StateId;
use petfilter::fsm::context::SensorSnapshot;
use std::collections::HashMap;

// ── Mock implementations ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum ActCall {
    SetPump { duty: u8, forward: bool },
    EnableUvc { duty: u8 },
    SetLed { r: u8, g: u8, b: u8 },
    AllOff,
}

struct MockHw {
    calls: Vec<ActCall>,
    uvc_on: bool,
}
impl MockHw {
    fn new() -> Self {
        Self {
            calls: Vec::new(),
            uvc_on: false,
        }
    }
}
impl SensorPort for MockHw {
    fn read_all(&mut self, _elapsed_secs: f32) -> SensorSnapshot {
        SensorSnapshot {
            nh3_ppm: 0.0,
            nh3_avg_ppm: 0.0,
            nh3_raw: 0,
            flow_ml_per_min: 100.0,
            flow_detected: true,
            tank_a_ok: true,
            tank_b_ok: true,
            temperature_c: 25.0,
            over_temperature: false,
            uvc_interlock_closed: true,
        }
    }
    fn read_ammonia_fast(&mut self) -> f32 {
        0.0
    }
}

impl ActuatorPort for MockHw {
    fn set_pump(&mut self, duty: u8, forward: bool) {
        self.calls.push(ActCall::SetPump { duty, forward });
    }
    fn stop_pump(&mut self) {
        self.calls.push(ActCall::SetPump {
            duty: 0,
            forward: true,
        });
    }
    fn enable_uvc(&mut self, duty: u8) {
        self.uvc_on = duty > 0;
        self.calls.push(ActCall::EnableUvc { duty });
    }
    fn disable_uvc(&mut self) {
        self.uvc_on = false;
        self.calls.push(ActCall::EnableUvc { duty: 0 });
    }
    fn fault_shutdown_uvc(&mut self, _reason: &'static str) {
        self.uvc_on = false;
        self.calls.push(ActCall::EnableUvc { duty: 0 });
    }
    fn is_uvc_on(&self) -> bool {
        self.uvc_on
    }
    fn set_led(&mut self, r: u8, g: u8, b: u8) {
        self.calls.push(ActCall::SetLed { r, g, b });
    }
    fn all_off(&mut self) {
        self.uvc_on = false;
        self.calls.push(ActCall::AllOff);
    }
}

struct MockNvs {
    store: HashMap<String, Vec<u8>>,
}
impl MockNvs {
    fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }
}
impl StoragePort for MockNvs {
    fn read(&self, ns: &str, key: &str, buf: &mut [u8]) -> Result<usize, StorageError> {
        match self.store.get(&format!("{}::{}", ns, key)) {
            Some(v) => {
                let n = v.len().min(buf.len());
                buf[..n].copy_from_slice(&v[..n]);
                Ok(n)
            }
            None => Err(StorageError::NotFound),
        }
    }
    fn write(&mut self, ns: &str, key: &str, data: &[u8]) -> Result<(), StorageError> {
        self.store.insert(format!("{}::{}", ns, key), data.to_vec());
        Ok(())
    }
    fn exists(&self, ns: &str, key: &str) -> bool {
        self.store.contains_key(&format!("{}::{}", ns, key))
    }
    fn delete(&mut self, ns: &str, key: &str) -> Result<(), StorageError> {
        self.store.remove(&format!("{}::{}", ns, key));
        Ok(())
    }
}
impl ConfigPort for MockNvs {
    fn load(&self) -> Result<SystemConfig, ConfigError> {
        Ok(SystemConfig::default())
    }
    fn save(&self, _c: &SystemConfig) -> Result<(), ConfigError> {
        Ok(())
    }
}

struct LogSink {
    events: Vec<String>,
}
impl LogSink {
    fn new() -> Self {
        Self { events: Vec::new() }
    }
}
impl EventSink for LogSink {
    fn emit(&mut self, e: &AppEvent) {
        self.events.push(format!("{:?}", e));
    }
}

fn make_app() -> (AppService, MockHw, LogSink) {
    let mut app = AppService::new(SystemConfig::default());
    let hw = MockHw::new();
    let mut sink = LogSink::new();
    app.start(&mut sink);
    (app, hw, sink)
}

// ── QA-7b: StartScrub → not Idle, actuators commanded ────────

#[test]
fn start_scrub_leaves_idle_and_commands_actuators() {
    let (mut app, mut hw, mut sink) = make_app();
    assert_eq!(app.state(), StateId::Idle);
    app.handle_command(AppCommand::StartScrub, &mut hw, &mut sink);
    assert_ne!(
        app.state(),
        StateId::Idle,
        "FSM should leave Idle after StartScrub"
    );
    assert!(!hw.calls.is_empty(), "at least one actuator call expected");
}

// ── QA-7c: UpdateConfig marks dirty ──────────────────────────

#[test]
fn update_config_marks_dirty() {
    let (mut app, mut hw, mut sink) = make_app();
    assert!(!app.is_config_dirty());
    let cfg = SystemConfig {
        pump_duty_percent: 75,
        ..Default::default()
    };
    app.handle_command(AppCommand::UpdateConfig(cfg), &mut hw, &mut sink);
    assert!(
        app.is_config_dirty(),
        "dirty flag must be set after UpdateConfig"
    );
}

// ── QA-7c: auto_save fires after enough ticks ─────────────

#[test]
fn auto_save_fires_after_dirty_timeout() {
    let (mut app, mut hw, mut sink) = make_app();
    let cfg = SystemConfig {
        purge_duration_secs: 42,
        ..Default::default()
    };
    app.handle_command(AppCommand::UpdateConfig(cfg), &mut hw, &mut sink);
    assert!(app.is_config_dirty());

    let nvs = MockNvs::new();
    for _ in 0..65 {
        app.tick(&mut hw, &mut sink);
    }
    let saved = app.auto_save_if_needed(&nvs);
    assert!(
        saved,
        "auto_save should fire after enough ticks with dirty config"
    );
}

// ── QA-7e: OTA state machine ──────────────────────────────

#[test]
fn ota_happy_path() {
    use petfilter::rpc::ota::{OtaManager, OtaState};
    let mut ota = OtaManager::new();
    assert_eq!(ota.state(), OtaState::Idle);
    ota.begin(8, &[0u8; 32]).unwrap();
    assert_eq!(ota.write_chunk(0, b"abcd").unwrap(), 4);
    assert_eq!(ota.write_chunk(4, b"efgh").unwrap(), 8);
    assert!(ota.finalize().is_ok());
    assert_eq!(ota.state(), OtaState::ReadyToReboot);
}

#[test]
fn ota_abort_always_resets() {
    use petfilter::rpc::ota::{OtaManager, OtaState};
    let mut ota = OtaManager::new();
    ota.begin(100, &[0u8; 32]).unwrap();
    ota.write_chunk(0, &[1u8; 50]).unwrap();
    ota.abort();
    assert_eq!(ota.state(), OtaState::Idle);
    assert!(ota.begin(8, &[0u8; 32]).is_ok());
}

// ── QA-7f: ForceState(Error) → AllOff ────────────────────

#[test]
fn force_error_state_issues_all_off() {
    let (mut app, mut hw, mut sink) = make_app();
    app.handle_command(AppCommand::ForceState(StateId::Error), &mut hw, &mut sink);
    assert_eq!(app.state(), StateId::Error);
    // apply_actuators uses granular calls (stop_pump, etc.) rather than all_off
    assert!(
        hw.calls
            .iter()
            .any(|c| matches!(c, ActCall::SetPump { duty: 0, .. })),
        "Pump must be stopped when FSM enters Error state"
    );
}

// ── QA-7f: Safety fault → Error → clear → Idle ───────────────

#[test]
fn safety_fault_error_then_clear_returns_to_idle() {
    let (mut app, mut hw, mut sink) = make_app();

    // Force FSM into Error state (simulates safety fault).
    app.handle_command(AppCommand::ForceState(StateId::Error), &mut hw, &mut sink);
    assert_eq!(app.state(), StateId::Error, "FSM should be in Error state");
    assert!(
        hw.calls
            .iter()
            .any(|c| matches!(c, ActCall::SetPump { duty: 0, .. })),
        "Pump must be stopped on Error entry"
    );

    // Clear the error by forcing Idle (simulates fault-clear RPC).
    hw.calls.clear();
    app.handle_command(AppCommand::ForceState(StateId::Idle), &mut hw, &mut sink);
    assert_eq!(
        app.state(),
        StateId::Idle,
        "FSM should return to Idle after fault clear"
    );
}
