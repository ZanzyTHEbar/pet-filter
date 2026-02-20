//! Mock hardware adapter for integration tests.
//!
//! Records every actuator call so tests can assert on the full command
//! history without touching real GPIO/PWM registers.

use petfilter::app::ports::{ActuatorPort, ConfigError, ConfigPort, EventSink, StoragePort};
use petfilter::config::SystemConfig;
use std::collections::HashMap;

// ── Actuator call record ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ActuatorCall {
    SetPump { duty: u8, forward: bool },
    EnableUvc { duty: u8 },
    SetLed { r: u8, g: u8, b: u8 },
    AllOff,
}

// ── MockHardware ──────────────────────────────────────────────

pub struct MockHardware {
    pub calls: Vec<ActuatorCall>,
}

#[allow(dead_code)]
impl MockHardware {
    pub fn new() -> Self {
        Self { calls: Vec::new() }
    }

    pub fn last_call(&self) -> Option<&ActuatorCall> {
        self.calls.last()
    }

    pub fn pump_on(&self) -> bool {
        self.calls
            .iter()
            .rev()
            .find_map(|c| match c {
                ActuatorCall::SetPump { duty, .. } => Some(*duty > 0),
                ActuatorCall::AllOff => Some(false),
                _ => None,
            })
            .unwrap_or(false)
    }

    pub fn uvc_on(&self) -> bool {
        self.calls
            .iter()
            .rev()
            .find_map(|c| match c {
                ActuatorCall::EnableUvc { duty } => Some(*duty > 0),
                ActuatorCall::AllOff => Some(false),
                _ => None,
            })
            .unwrap_or(false)
    }
}

impl Default for MockHardware {
    fn default() -> Self {
        Self::new()
    }
}

impl ActuatorPort for MockHardware {
    fn set_pump(&mut self, duty: u8, forward: bool) {
        self.calls.push(ActuatorCall::SetPump { duty, forward });
    }

    fn stop_pump(&mut self) {
        self.calls.push(ActuatorCall::SetPump {
            duty: 0,
            forward: true,
        });
    }

    fn enable_uvc(&mut self, duty: u8) {
        self.calls.push(ActuatorCall::EnableUvc { duty });
    }

    fn disable_uvc(&mut self) {
        self.calls.push(ActuatorCall::EnableUvc { duty: 0 });
    }

    fn fault_shutdown_uvc(&mut self, _reason: &'static str) {
        self.calls.push(ActuatorCall::EnableUvc { duty: 0 });
    }

    fn is_uvc_on(&self) -> bool {
        self.uvc_on()
    }

    fn set_led(&mut self, r: u8, g: u8, b: u8) {
        self.calls.push(ActuatorCall::SetLed { r, g, b });
    }

    fn all_off(&mut self) {
        self.calls.push(ActuatorCall::AllOff);
    }
}

// ── SensorPort for MockHardware ──────────────────────────────

impl petfilter::app::ports::SensorPort for MockHardware {
    fn read_all(&mut self, _elapsed_secs: f32) -> petfilter::fsm::context::SensorSnapshot {
        petfilter::fsm::context::SensorSnapshot {
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

// ── MockNvs ───────────────────────────────────────────────────

pub struct MockNvs {
    store: HashMap<String, Vec<u8>>,
}

impl MockNvs {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }
}

impl Default for MockNvs {
    fn default() -> Self {
        Self::new()
    }
}

impl StoragePort for MockNvs {
    fn read(
        &self,
        namespace: &str,
        key: &str,
        buf: &mut [u8],
    ) -> Result<usize, petfilter::app::ports::StorageError> {
        let k = format!("{}::{}", namespace, key);
        match self.store.get(&k) {
            Some(v) => {
                let n = v.len().min(buf.len());
                buf[..n].copy_from_slice(&v[..n]);
                Ok(n)
            }
            None => Err(petfilter::app::ports::StorageError::NotFound),
        }
    }

    fn write(
        &mut self,
        namespace: &str,
        key: &str,
        data: &[u8],
    ) -> Result<(), petfilter::app::ports::StorageError> {
        let k = format!("{}::{}", namespace, key);
        self.store.insert(k, data.to_vec());
        Ok(())
    }

    fn exists(&self, namespace: &str, key: &str) -> bool {
        self.store.contains_key(&format!("{}::{}", namespace, key))
    }

    fn delete(
        &mut self,
        namespace: &str,
        key: &str,
    ) -> Result<(), petfilter::app::ports::StorageError> {
        self.store.remove(&format!("{}::{}", namespace, key));
        Ok(())
    }
}

impl ConfigPort for MockNvs {
    fn load(&self) -> Result<SystemConfig, ConfigError> {
        Ok(SystemConfig::default())
    }

    fn save(&self, _config: &SystemConfig) -> Result<(), ConfigError> {
        Ok(())
    }
}

// ── LogEventSink ─────────────────────────────────────────────

pub struct LogSink {
    pub events: Vec<String>,
}

impl LogSink {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }
}

impl Default for LogSink {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSink for LogSink {
    fn emit(&mut self, event: &petfilter::app::events::AppEvent) {
        self.events.push(format!("{:?}", event));
    }
}
