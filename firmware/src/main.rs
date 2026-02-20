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
mod esp_link_shims;

mod adapters;
pub mod app;
mod control;
pub mod diagnostics;
mod drivers;
pub mod fsm;
pub mod rpc;
mod sensors;

// ── Imports ───────────────────────────────────────────────────
use anyhow::Result;
use log::{info, warn};

use adapters::ble::{BleAdapter, ProvisioningPort};
use adapters::device_id;
use adapters::hardware::HardwareAdapter;
use adapters::log_sink::LogEventSink;
use adapters::mdns::MdnsAdapter;
use adapters::nvs::NvsAdapter;
use adapters::rpc_adapter::RpcEventSink;
use adapters::time::Esp32TimeAdapter;
use adapters::wifi::{ConnectivityPort, WifiAdapter};
use app::commands::AppCommand;
use app::events::AppEvent;
use app::ports::{
    ActuatorPort, ConfigPort, EventSink, ScheduleFiredKind, SchedulerDelegate, SensorPort,
    StoragePort,
};
use app::service::AppService;
use config::SystemConfig;
use drivers::button::{ButtonDriver, ButtonEvent};
use drivers::led_patterns::{
    COLOUR_ACTIVE, COLOUR_IDLE, COLOUR_LOW_WATER, COLOUR_OTA, COLOUR_PROVISIONING,
    COLOUR_PURGING, COLOUR_SENSING, COLOUR_WIFI_CONNECTED, LedPatternEngine, PatternId,
};
use drivers::pump::PumpDriver;
use drivers::status_led::StatusLed;
use drivers::uvc::UvcDriver;
use events::{Event, push_event};
use fsm::StateId;
use power::{PowerManager, PowerMode, WakeReason};
use rpc::auth::MAX_CLIENTS;
use scheduler::{Schedule, ScheduleKind, Scheduler};

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
    info!(
        "║  PetFilter v{}                       ║",
        env!("CARGO_PKG_VERSION")
    );
    info!(
        "║  Built: {}                ║",
        option_env!("BUILD_TIMESTAMP").unwrap_or("dev")
    );
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
            warn!(
                "NVS init failed ({}), running with defaults and no persistence",
                e
            );
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
    // load_ulp_program() was already called in the PowerOn match arm above;
    // calling it again here would double-load the binary and reset ULP state.
    power_mgr.stop_ulp_monitor();

    // ── 5. Construct adapters ─────────────────────────────────
    let sensor_hub = sensors::SensorHub::new(
        sensors::ammonia::AmmoniaSensor::new(pins::NH3_ADC_GPIO),
        sensors::flow::FlowSensor::new(pins::FLOW_PULSE_GPIO),
        sensors::water_level::WaterLevelSensor::new(
            pins::WATER_LEVEL_A_GPIO,
            pins::WATER_LEVEL_B_GPIO,
        ),
        sensors::temperature::TemperatureSensor::new(pins::TEMP_ADC_GPIO, config.max_temperature_c),
        pins::UVC_INTERLOCK_GPIO,
    );

    let mut hw = HardwareAdapter::new(
        sensor_hub,
        PumpDriver::new(),
        UvcDriver::new(),
        StatusLed::new(),
    );

    let mut log_sink = LogEventSink::new();
    let mut rpc_sink = RpcEventSink::new();
    let mut sched = Scheduler::new();
    let mut sched_delegate = EventQueueDelegate;
    #[cfg(target_os = "espidf")]
    let mut _sntp: Option<esp_idf_svc::sntp::EspSntp<'static>> = None;

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

    // ── WiFi station adapter ──────────────────────────────────
    if let Err(e) = adapters::wifi::wifi_stack_init() {
        warn!("WiFi stack init failed: {} — WiFi unavailable", e);
    }
    let mut wifi = WifiAdapter::new();

    // ── Boot-time WiFi auto-reconnect ─────────────────────────
    let mut wifi_connected_on_boot = false;
    {
        let mut ssid_buf = [0u8; 32];
        let mut pass_buf = [0u8; 64];
        if let (Ok(ssid_len), Ok(pass_len)) = (
            nvs.read_credential("wifi_ssid", &mut ssid_buf),
            nvs.read_credential("wifi_pass", &mut pass_buf),
        ) {
            if let (Ok(ssid), Ok(pass)) = (
                core::str::from_utf8(&ssid_buf[..ssid_len]),
                core::str::from_utf8(&pass_buf[..pass_len]),
            ) {
                info!("Boot: stored WiFi credentials for '{}' — connecting", ssid);
                if wifi.set_credentials(ssid, pass).is_ok() {
                    match wifi.connect() {
                        Ok(()) => {
                            info!("Boot: WiFi connected to '{}'", ssid);
                            wifi_connected_on_boot = true;
                        }
                        Err(e) => warn!("Boot: WiFi connect failed ({}), falling back to BLE", e),
                    }
                }
            }
        }
    }

    // ── BLE provisioning adapter ──────────────────────────────
    let mut ble = BleAdapter::new(dev_hostname.clone());
    if !wifi_connected_on_boot {
        ble.start();
    } else {
        info!("Boot: skipping BLE (WiFi already connected)");
    }

    // ── mDNS service advertisement ────────────────────────────
    let mut mdns = MdnsAdapter::new(dev_hostname.clone(), dev_id.clone());
    if wifi_connected_on_boot {
        mdns.start();
    }

    // ── 6. Construct app service ──────────────────────────────
    let mut app = AppService::new(config.clone());

    if wake_reason == WakeReason::UlpWake {
        app.start_from(StateId::Sensing, &mut log_sink);
    } else {
        app.start(&mut log_sink);
    }

    // ── 6b. RPC engine + I/O task ──────────────────────────────
    let rpc_psk = b"default-psk-change-me";
    let mut rpc_engine = rpc::engine::RpcEngine::new(rpc_psk);
    rpc_engine.init_crash_log(&nvs);

    // TLS transport — multi-client server on port 4242.
    // Ownership moves to the I/O task thread; main loop communicates
    // via embassy-sync channels (CMD_CHANNEL / RESP_CHANNEL).
    let tls_transport = match adapters::tls_transport::TlsTransport::new(
        adapters::tls_transport::DEFAULT_PORT,
        rpc_psk,
    ) {
        Ok(t) => {
            info!(
                "TLS: listening on port {}",
                adapters::tls_transport::DEFAULT_PORT
            );
            Some(t)
        }
        Err(e) => {
            warn!("TLS transport init failed: {} — RPC unavailable", e);
            None
        }
    };
    let _io_handle = tls_transport.map(|t| rpc::io_task::spawn(t));

    // Register this task for FreeRTOS notification-based wake.
    // On ESP-IDF, main() runs on Core 1 (APP_CPU) per sdkconfig.
    events::register_main_task();

    info!("System ready. Entering event loop.");

    // ── 7. Event loop ─────────────────────────────────────────
    let tick_secs = config.control_loop_interval_ms as f32 / 1000.0;
    let mut telemetry_counter: u64 = 0;

    loop {
        // Block until a push_event() notification arrives or timeout expires.
        // On ESP-IDF: ulTaskNotifyTake suspends the task, allowing the
        // FreeRTOS idle task to run PM hooks (automatic light sleep).
        // Wakes instantly when any ISR/timer/software calls push_event().
        // On simulation: sleeps for the control interval then injects ControlTick.
        #[cfg(target_os = "espidf")]
        events::wait_for_event(100);

        #[cfg(not(target_os = "espidf"))]
        {
            events::wait_for_event(config.control_loop_interval_ms as u32);
            push_event(Event::ControlTick);
        }

        telemetry_counter += 1;
        if telemetry_counter >= config.telemetry_interval_secs as u64 {
            push_event(Event::TelemetryTick);
            telemetry_counter = 0;
        }

        // Tick the scheduler (delegate-driven, decoupled from events).
        #[cfg(target_os = "espidf")]
        if wifi.is_connected() && _sntp.is_none() {
            if let Ok(s) = esp_idf_svc::sntp::EspSntp::new_default() {
                _sntp = Some(s);
                info!("NTP: started");
            }
        }
        let current_hour = time_adapter.current_hour();
        sched.tick(current_hour, tick_secs, &mut sched_delegate);

        // Process all pending events.
        let mut activity = false;

        events::drain_events(|event| {
            match event {
                Event::ControlTick => {
                    app.tick(&mut hw, &mut log_sink);
                    if rpc_engine.ota_mut().has_pending() {
                        if let Err(e) = rpc_engine.ota_mut().flush_pending() {
                            warn!("OTA flush failed: {}", e);
                        }
                    }
                    if app.state() != StateId::Idle {
                        activity = true;
                    }
                }

                Event::SensorReadTick => {
                    let _ = hw.read_ammonia_fast();
                    sensors::flow::flow_clear_event_latch();
                    activity = true;
                }

                Event::TelemetryTick => {
                    let wifi_rssi = wifi.rssi();
                    let t = app.build_telemetry(wifi_rssi);
                    log_sink.emit(&AppEvent::Telemetry(t.clone()));
                    rpc_sink.emit(&AppEvent::Telemetry(t));
                    let tick_ms = config.telemetry_interval_secs as u32 * 1000;
                    for cid in 0..MAX_CLIENTS as u8 {
                        if rpc_engine.should_stream_telemetry(cid, tick_ms) {
                            if let Some(frame) = rpc_engine.build_telemetry_frame(cid, &app, wifi_rssi) {
                                rpc::io_task::send_response(frame.client_id, frame.data);
                            }
                        }
                    }
                }

                Event::SafetyFault => {
                    // Safety supervisor runs inside app.tick() — faults
                    // are evaluated on the next ControlTick automatically.
                }

                Event::InterlockChanged | Event::WaterLevelChanged => {
                    // Re-evaluate control/safety immediately after GPIO safety edges.
                    push_event(Event::ControlTick);
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
                    while let Some(cmd) = rpc::io_task::try_recv_command() {
                        if let Some(resp) = rpc_engine.dispatch(
                            cmd.client_id,
                            &cmd.frame,
                            &mut app,
                            &mut hw,
                            &mut log_sink,
                            &mut nvs,
                        ) {
                            rpc::io_task::send_response(resp.client_id, resp.data);
                        }
                    }
                    while let Some(disc) = rpc::io_task::try_recv_disconnect() {
                        info!("RPC: client {} disconnected", disc.client_id);
                        rpc_engine.reset_client(disc.client_id);
                        rpc_sink.unsubscribe(disc.client_id);
                    }
                    activity = true;
                }

                Event::ButtonShortPress => {
                    info!("Button: short press → toggle scrub");
                    app.handle_command(AppCommand::StartScrub, &mut hw, &mut log_sink);
                    activity = true;
                }

                Event::ButtonLongPress => {
                    warn!("Button: long press → FACTORY RESET");
                    let _ = nvs.erase_credentials();
                    let _ = nvs.delete("auth", "wifi_ssid");
                    let _ = nvs.delete("auth", "wifi_pass");
                    info!("Factory reset: credentials erased, restarting...");
                    #[cfg(target_os = "espidf")]
                    unsafe {
                        esp_idf_svc::sys::esp_restart();
                    }
                    activity = true;
                }

                Event::ButtonDoublePress => {
                    info!("Button: double press → manual boost (5 min)");
                    if sched.add(Schedule {
                        label: "manual-boost",
                        kind: ScheduleKind::Boost { duration_secs: 300 },
                        enabled: true,
                    }).is_none() {
                        warn!("Scheduler full, cannot add manual boost");
                    }
                    activity = true;
                }

                Event::BleConnected => {
                    ble.on_central_connected();
                    rpc::io_task::ble_set_connected(128);
                    activity = true;
                }

                Event::BleDisconnected => {
                    ble.on_central_disconnected();
                    rpc::io_task::ble_set_disconnected();
                }

                Event::BleSsidWrite => {
                    if let Some(data) = adapters::ble::take_ssid_data() {
                        if let Err(e) = ble.on_ssid_write(&data) {
                            warn!("BLE: SSID write rejected: {}", e);
                        }
                    }
                    activity = true;
                }
                Event::BlePasswordWrite => {
                    if let Some(data) = adapters::ble::take_pass_data() {
                        if let Err(e) = ble.on_password_write(&data) {
                            warn!("BLE: password write rejected: {}", e);
                        }
                    }
                    activity = true;
                }
                Event::BlePskWrite => {
                    if let Some(data) = adapters::ble::take_psk_data() {
                        if let Err(e) = ble.on_psk_write(&data) {
                            warn!("BLE: PSK write rejected: {}", e);
                        }
                    }
                    activity = true;
                }

                _ => {}
            }
        });

        // Drain BLE RPC responses from io_task and send over GATT notify.
        while let Some(resp) = rpc::io_task::try_recv_ble_response() {
            ble.send_rpc_response(&resp);
        }

        // Button gesture detection (runs outside drain_events since it uses its own atomic).
        let now_ms = (time_adapter.uptime_us() / 1000) as u32;
        if let Some(gesture) = button.tick(now_ms) {
            match gesture {
                ButtonEvent::ShortPress => {
                    push_event(Event::ButtonShortPress);
                }
                ButtonEvent::LongPress => {
                    push_event(Event::ButtonLongPress);
                }
                ButtonEvent::DoublePress => {
                    push_event(Event::ButtonDoublePress);
                }
            }
        }

        // Connectivity overlay (BLE / WiFi / OTA / low-water) — priority order.
        {
            use crate::adapters::ble::ProvisioningPort;
            use crate::error::SafetyFault;
            use crate::rpc::ota::OtaState;
            if rpc_engine.ota_mut().state() != OtaState::Idle {
                led_engine.set_connectivity_pattern(COLOUR_OTA, PatternId::FastBlink);
            } else if app.fault_flags() & SafetyFault::WaterLevelLow.mask() != 0 {
                led_engine.set_connectivity_pattern(COLOUR_LOW_WATER, PatternId::SlowPulse);
            } else if wifi.is_connected() {
                led_engine.set_connectivity_pattern(COLOUR_WIFI_CONNECTED, PatternId::Solid);
            } else if ble.is_active() {
                led_engine.set_connectivity_pattern(COLOUR_PROVISIONING, PatternId::DoubleBlink);
            } else {
                led_engine.clear_connectivity();
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
                            info!("Provisioning: WiFi connected, persisting credentials");
                            if let Err(e) = nvs.store_credential("wifi_ssid", ssid.as_bytes()) {
                                warn!("Failed to persist WiFi SSID: {:?}", e);
                            }
                            if let Err(e) = nvs.store_credential("wifi_pass", password.as_bytes()) {
                                warn!("Failed to persist WiFi password: {:?}", e);
                            }
                            info!("Provisioning: starting mDNS + TLS listener");
                            mdns.start();
                        }
                        Err(e) => {
                            warn!("Provisioning: WiFi failed ({}), restarting BLE", e);
                            ble.start();
                        }
                    }
                }
            }

            if let Some(psk) = ble.take_pending_psk() {
                info!("Provisioning: PSK received, storing in encrypted NVS");
                if let Err(e) = nvs.store_credential("psk", &psk) {
                    warn!("Failed to store PSK: {:?}", e);
                }
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
                    mdns.stop();
                    hw.all_off();
                    watchdog.feed();
                    let _wake = power_mgr.enter_light_sleep(60);
                    // Re-announce on wake
                    if wifi.is_connected() {
                        mdns.start();
                    }
                }
                PowerMode::DeepSleep if app.state() == StateId::Idle => {
                    info!("Entering deep sleep — graceful shutdown");
                    mdns.stop();
                    wifi.disconnect();
                    ble.stop();
                    app.force_save_if_dirty(&nvs);
                    hw.all_off();
                    watchdog.feed();
                    power_mgr.enter_deep_sleep(500);
                }
                _ => {}
            }
        }
    }
}
