//! PetFilter Firmware — Main Entry Point
//!
//! Hexagonal architecture with event-driven execution and ULP deep sleep.
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                      Adapters (outer ring)                     │
//! │                                                                │
//! │  HardwareAdapter   LogEventSink   NvsAdapter   Esp32Time       │
//! │  (Sensor+Actuator) (EventSink)    (Config+NVS) (TimePort)      │
//! │  WiFiAdapter       BleAdapter     RpcEngine                    │
//! │  (Connectivity)    (Provisioning) (RPC transport)              │
//! │                                                                │
//! │  ──────────────── Port Trait Boundary ───────────────────      │
//! │                                                                │
//! │  ┌────────────────────────────────────────────────────────┐    │
//! │  │              AppService (pure logic)                   │    │
//! │  │  FSM · Safety · PID                                    │    │
//! │  └────────────────────────────────────────────────────────┘    │
//! │                                                                │
//! │  Scheduler (delegate-driven) · PowerManager (ULP lifecycle)    │
//! └────────────────────────────────────────────────────────────────┘
//! ```
#![deny(unused_must_use)]

// ── Module declarations ───────────────────────────────────────
pub mod config;
mod error;
mod events;
mod pins;
mod power;
mod safety;
mod scheduler;

pub mod app;
mod adapters;
mod drivers;
pub mod fsm;
mod sensors;
mod control;
pub mod rpc;
pub mod diagnostics;

// ── Imports ───────────────────────────────────────────────────
use anyhow::Result;
use log::{info, warn};

use adapters::hardware::HardwareAdapter;
use adapters::log_sink::LogEventSink;
use adapters::nvs::NvsAdapter;
use adapters::time::Esp32TimeAdapter;
use app::commands::AppCommand;
use app::events::AppEvent;
use app::ports::{ActuatorPort, ConfigPort, EventSink, ScheduleFiredKind, SchedulerDelegate, SensorPort};
use app::service::AppService;
use config::SystemConfig;
use drivers::pump::PumpDriver;
use drivers::status_led::StatusLed;
use drivers::uvc::UvcDriver;
use events::{push_event, Event};
use fsm::StateId;
use power::{PowerManager, PowerMode, WakeReason};
use adapters::device_id;
use adapters::ble::{BleAdapter, ProvisioningPort};
use adapters::mdns::MdnsAdapter;
use adapters::wifi::{WifiAdapter, ConnectivityPort};
use drivers::button::{ButtonDriver, ButtonEvent};
use drivers::led_patterns::{LedPatternEngine, PatternId, COLOUR_IDLE, COLOUR_SENSING, COLOUR_ACTIVE, COLOUR_PURGING};
use scheduler::Scheduler;

// ── Scheduler delegate ────────────────────────────────────────
//
// Bridges the scheduler (which knows nothing about the event system)
// to the ISR event queue.  This is the hexagonal adapter pattern:
// the scheduler calls `on_schedule_fired`, and this impl translates
// that into an `Event::ScheduledScrub` pushed to the lock-free queue.

struct EventQueueDelegate;

impl SchedulerDelegate for EventQueueDelegate {
    fn on_schedule_fired(&mut self, label: &str, kind: ScheduleFiredKind) {
        info!("Schedule fired: '{}' ({:?})", label, kind);
        push_event(Event::ScheduledScrub);
    }
}

// ── Main ──────────────────────────────────────────────────────

fn main() -> Result<()> {
    // ── 1. ESP-IDF bootstrap ──────────────────────────────────
    esp_idf_svc::sys::link_patches();
    esp_idf_logger::init()?;

    info!("╔══════════════════════════════════════╗");
    info!("║  PetFilter v{}                       ║", env!("CARGO_PKG_VERSION"));
    info!("╚══════════════════════════════════════╝");

    // ── 1b. OTA rollback check ─────────────────────────────────
    rpc::ota::check_rollback();
    diagnostics::install_panic_handler();

    // ── 1c. Initialise hardware peripherals ───────────────────
    if let Err(e) = drivers::hw_init::init_peripherals() {
        // Peripheral init failure is critical — log and halt.
        // In production this triggers the watchdog reset after timeout.
        log::error!("HAL init failed: {} — halting", e);
        #[allow(clippy::empty_loop)]
        loop {}
    }
    drivers::hw_timer::start_timers();
    if let Err(e) = drivers::hw_init::init_isr_service() {
        log::error!("ISR service init failed: {} — continuing without ISRs", e);
    }
    let watchdog = drivers::watchdog::Watchdog::new();

    // ── 2. Load config from NVS (or defaults) ─────────────────
    let mut nvs = match NvsAdapter::new() {
        Ok(n) => n,
        Err(e) => {
            warn!("NVS init failed ({}), running with defaults and no persistence", e);
            // Continue without NVS — config will not be persisted this session.
            // On next reboot, NVS should self-heal.
            NvsAdapter::default()
        }
    };
    let config = match nvs.load() {
        Ok(cfg) => {
            info!("Config loaded from NVS");
            cfg
        }
        Err(e) => {
            warn!("NVS config load failed ({}), using defaults", e);
            SystemConfig::default()
        }
    };

    // ── 3. Power manager + wake reason ────────────────────────
    let mut power_mgr = PowerManager::new(&config);
    let wake_reason = power_mgr.determine_wake_reason();
    let time_adapter = Esp32TimeAdapter::new();

    match wake_reason {
        WakeReason::PowerOn => {
            info!("Boot: power-on");
            if let Err(e) = power_mgr.load_ulp_program() {
                warn!("ULP program load failed: {}", e);
            }
        }
        WakeReason::UlpWake => {
            let ulp = power_mgr.read_ulp_state();
            info!(
                "Boot: ULP wake (ADC={}, cycles={})",
                ulp.nh3_last_reading, ulp.cycle_count
            );
        }
        other => info!("Boot: {:?}", other),
    }

    // ── 4. ULP lifecycle ──────────────────────────────────────
    if wake_reason == WakeReason::PowerOn {
        power_mgr
            .load_ulp_program()
            .unwrap_or_else(|e| warn!("ULP: {}", e));
    }
    power_mgr.stop_ulp_monitor();

    // ── 5. Construct adapters ─────────────────────────────────
    let sensor_hub = sensors::SensorHub::new(
        sensors::ammonia::AmmoniaSensor::new(pins::NH3_ADC_GPIO),
        sensors::flow::FlowSensor::new(pins::FLOW_PULSE_GPIO),
        sensors::water_level::WaterLevelSensor::new(
            pins::WATER_LEVEL_A_GPIO,
            pins::WATER_LEVEL_B_GPIO,
        ),
        sensors::temperature::TemperatureSensor::new(
            pins::TEMP_ADC_GPIO,
            config.max_temperature_c,
        ),
        pins::UVC_INTERLOCK_GPIO,
    );

    let mut hw = HardwareAdapter::new(
        sensor_hub,
        PumpDriver::new(),
        UvcDriver::new(),
        StatusLed::new(),
    );

    let mut log_sink = LogEventSink::new();
    let mut sched = Scheduler::new();
    let mut sched_delegate = EventQueueDelegate;

    // ── Device identity ────────────────────────────────────
    let mac = device_id::read_mac();
    let dev_id = device_id::device_id(&mac);
    let dev_hostname = device_id::hostname(&mac);
    info!("Device ID: {} (hostname: {})", dev_id, dev_hostname);

    // ── Button driver ─────────────────────────────────────
    let mut button = ButtonDriver::new(pins::BUTTON_GPIO);

    // ── LED pattern engine ────────────────────────────────
    let mut led_engine = LedPatternEngine::new();
    led_engine.set_fsm_pattern(COLOUR_IDLE, PatternId::Solid);

    // ── BLE provisioning adapter ──────────────────────────────
    let mut ble = BleAdapter::new(dev_hostname.clone());
    ble.start();

    // ── WiFi station adapter ──────────────────────────────────
    let mut wifi = WifiAdapter::new();

    // ── mDNS service advertisement ────────────────────────────
    let mut mdns = MdnsAdapter::new(dev_hostname.clone(), dev_id.clone());

    // ── 6. Construct app service ──────────────────────────────
    let mut app = AppService::new(config.clone());

    if wake_reason == WakeReason::UlpWake {
        app.start_from(StateId::Sensing, &mut log_sink);
    } else {
        app.start(&mut log_sink);
    }

    // ── 6b. RPC engine ────────────────────────────────────────
    // TLS transport is initialized with a NullTransport placeholder;
    // the real TlsTransport (PSK-over-TCP) will replace this in QA-1.
    // PSK will be derived from device secret in NVS once config is extended.
    let rpc_transport = rpc::transport::NullTransport;
    let mut rpc_engine = rpc::engine::RpcEngine::new(rpc_transport, b"default-psk-change-me");
    rpc_engine.init_crash_log(&nvs);

    info!("System ready. Entering event loop.");

    // ── 7. Event loop ─────────────────────────────────────────
    let tick_secs = config.control_loop_interval_ms as f32 / 1000.0;
    let mut telemetry_counter: u64 = 0;

    loop {
        // Simulate timer interrupts via sleep on non-espidf targets.
        // On real hardware, the CPU executes WFI (Wait For Interrupt)
        // and wakes only when a hardware timer or GPIO interrupt fires.
        #[cfg(not(target_os = "espidf"))]
        {
            std::thread::sleep(std::time::Duration::from_millis(
                config.control_loop_interval_ms as u64,
            ));
            push_event(Event::ControlTick);
        }

        telemetry_counter += 1;
        if telemetry_counter >= config.telemetry_interval_secs as u64 {
            push_event(Event::TelemetryTick);
            telemetry_counter = 0;
        }

        // Tick the scheduler (delegate-driven, decoupled from events).
        let current_hour = None; // Wire to time_adapter when NTP is synced.
        sched.tick(current_hour, tick_secs, &mut sched_delegate);

        // Process all pending events.
        let mut activity = false;

        events::drain_events(|event| {
            match event {
                Event::ControlTick => {
                    app.tick(&mut hw, &mut log_sink);
                    if app.state() != StateId::Idle {
                        activity = true;
                    }
                }

                Event::SensorReadTick => {
                    let _ = hw.read_ammonia_fast();
                }

                Event::TelemetryTick => {
                    let t = app.build_telemetry();
                    log_sink.emit(&AppEvent::Telemetry(t));
                }

                Event::SafetyFault => {
                    // Safety supervisor runs inside app.tick() — faults
                    // are evaluated on the next ControlTick automatically.
                }

                Event::InterlockChanged | Event::WaterLevelChanged => {
                    activity = true;
                }

                Event::UlpWake => {
                    info!("ULP wake in event loop");
                    power_mgr.stop_ulp_monitor();
                    activity = true;
                }

                Event::ScheduledScrub => {
                    app.handle_command(AppCommand::StartScrub, &mut hw, &mut log_sink);
                    activity = true;
                }

                Event::CommandReceived => {
                    rpc_engine.poll(&mut app, &mut hw, &mut log_sink, &mut nvs);
                    activity = true;
                }

                Event::ButtonShortPress => {
                    info!("Button: short press → toggle scrub");
                    app.handle_command(AppCommand::StartScrub, &mut hw, &mut log_sink);
                    activity = true;
                }

                Event::ButtonLongPress => {
                    info!("Button: long press → factory reset");
                    // Factory reset: erase NVS + restart into provisioning
                    // Wired in P1-6g when NVS erase is available.
                    activity = true;
                }

                Event::ButtonDoublePress => {
                    info!("Button: double press → boost mode toggle");
                    activity = true;
                }

                _ => {}
            }
        });

        // Button gesture detection (runs outside drain_events since it uses its own atomic).
        let now_ms = (time_adapter.uptime_us() / 1000) as u32;
        if let Some(gesture) = button.tick(now_ms) {
            match gesture {
                ButtonEvent::ShortPress => { push_event(Event::ButtonShortPress); }
                ButtonEvent::LongPress => { push_event(Event::ButtonLongPress); }
                ButtonEvent::DoublePress => { push_event(Event::ButtonDoublePress); }
            }
        }

        // LED pattern engine — update based on current FSM state.
        let led_pattern = match app.state() {
            StateId::Idle => (COLOUR_IDLE, PatternId::Solid),
            StateId::Sensing => (COLOUR_SENSING, PatternId::SlowPulse),
            StateId::Active => (COLOUR_ACTIVE, PatternId::Solid),
            StateId::Purging => (COLOUR_PURGING, PatternId::Breathing),
            StateId::Error => (COLOUR_IDLE, PatternId::Off), // error_pattern handles this
        };
        led_engine.set_fsm_pattern(led_pattern.0, led_pattern.1);
        led_engine.set_error_pattern(app.state() == StateId::Error);
        let (lr, lg, lb) = led_engine.tick(config.control_loop_interval_ms);
        hw.set_led(lr, lg, lb);

        // BLE→WiFi provisioning flow:
        // Check if BLE has received credentials, attempt WiFi connection.
        {
            use crate::adapters::ble::ProvisioningPort;
            if let Some((ssid, password)) = ble.take_pending_credentials() {
                info!("Provisioning: received credentials for '{}'", ssid);
                if let Err(e) = wifi.set_credentials(ssid.as_str(), password.as_str()) {
                    warn!("Provisioning: invalid credentials — {}", e);
                } else {
                    ble.stop();
                    match wifi.connect() {
                        Ok(()) => {
                            info!("Provisioning: WiFi connected, starting mDNS + TLS listener");
                            mdns.start();
                        }
                        Err(e) => {
                            warn!("Provisioning: WiFi failed ({}), restarting BLE", e);
                            ble.start();
                        }
                    }
                }
            }

            // Store PSK if received via BLE
            if let Some(_psk) = ble.take_pending_psk() {
                info!("Provisioning: PSK received, storing in encrypted NVS");
                // nvs.write("auth", "psk", &psk).ok();
            }
        }

        // WiFi reconnection poll (exponential backoff).
        wifi.poll();

        // Config auto-save (5s debounce after last change).
        app.auto_save_if_needed(&nvs);

        // Feed watchdog on every iteration.
        watchdog.feed();

        // Power management.
        if let Some(mode) = power_mgr.tick(activity) {
            match mode {
                PowerMode::LightSleep if app.state() == StateId::Idle => {
                    hw.all_off();
                    let _wake = power_mgr.enter_light_sleep(60);
                }
                PowerMode::DeepSleep if app.state() == StateId::Idle => {
                    app.force_save_if_dirty(&nvs);
                    hw.all_off();
                    power_mgr.enter_deep_sleep(500);
                }
                _ => {}
            }
        }
    }
}
