//! RPC engine — dispatches incoming FlatBuffer messages to the AppService.
//!
//! All messages pass through a three-gate pipeline:
//!
//! 1. **Rate limiting** — token-bucket rejects bursts above 60 req/min.
//! 2. **Authentication gate** — only `GetDeviceInfo` and auth handshake
//!    messages are allowed before a session is authenticated.
//! 3. **Sequence check** — monotonically increasing `msg.id()` prevents
//!    replay attacks within an authenticated session.

use flatbuffers::FlatBufferBuilder;
use heapless;
use log::{info, warn};

use crate::app::commands::AppCommand;
use crate::app::ports::{ActuatorPort, EventSink, StoragePort};
use crate::app::service::AppService;
use crate::diagnostics::CrashLog;
use crate::events::{push_event, Event};
use crate::fsm::StateId;

use super::auth::Session;
use super::codec::{encode_frame, FrameDecoder};
use super::fb;
use super::ota::OtaManager;
use super::transport::Transport;

/// RPC engine — generic over the byte-oriented transport.
pub struct RpcEngine<T: Transport> {
    transport: T,
    decoder: FrameDecoder,
    write_buf: [u8; 512],
    session: Session,
    psk: [u8; 32],
    psk_len: usize,
    telemetry_subscribed: bool,
    telemetry_interval_ms: u32,
    telemetry_tick_counter: u32,
    next_msg_id: u32,
    ota: OtaManager,
    ulp_wake_count: u32,
    crash_log: CrashLog,
}

impl<T: Transport> RpcEngine<T> {
    pub fn new(transport: T, psk: &[u8]) -> Self {
        let mut psk_buf = [0u8; 32];
        let psk_len = psk.len().min(32);
        psk_buf[..psk_len].copy_from_slice(&psk[..psk_len]);

        Self {
            transport,
            decoder: FrameDecoder::new(),
            write_buf: [0; 512],
            session: Session::new(),
            psk: psk_buf,
            psk_len,
            telemetry_subscribed: false,
            telemetry_interval_ms: 1000,
            telemetry_tick_counter: 0,
            next_msg_id: 1,
            ota: OtaManager::new(),
            ulp_wake_count: 0,
            crash_log: CrashLog::new(),
        }
    }

    /// Initialise the crash log from persistent NVS storage.
    /// Call once after NVS is ready in main().
    pub fn init_crash_log(&mut self, nvs: &dyn StoragePort) {
        self.crash_log.init(nvs);
    }

    fn alloc_msg_id(&mut self) -> u32 {
        let id = self.next_msg_id;
        self.next_msg_id = self.next_msg_id.wrapping_add(1);
        id
    }

    /// Poll the transport for incoming data and dispatch complete messages.
    pub fn poll(
        &mut self,
        app: &mut AppService,
        hw: &mut impl ActuatorPort,
        sink: &mut impl EventSink,
        nvs: &mut dyn StoragePort,
    ) {
        let mut read_buf = [0u8; 512];
        let n = match self.transport.read(&mut read_buf) {
            Ok(n) if n > 0 => n,
            _ => return,
        };

        if let Some(frame) = self.decoder.feed(&read_buf[..n]) {
            let mut frame_copy = [0u8; 512];
            let frame_len = frame.len().min(frame_copy.len());
            frame_copy[..frame_len].copy_from_slice(&frame[..frame_len]);
            self.dispatch_frame(&frame_copy[..frame_len], app, hw, sink, nvs);
        }
    }

    /// Tick periodic maintenance: telemetry streaming + rate-limit token refill.
    pub fn tick(&mut self, tick_ms: u32) {
        let elapsed_secs = tick_ms as f32 / 1000.0;
        self.session.refill_rate_limit(elapsed_secs);
    }

    /// Returns `true` if a telemetry frame should be streamed this tick.
    pub fn should_stream_telemetry(&mut self, tick_ms: u32) -> bool {
        if !self.telemetry_subscribed || self.telemetry_interval_ms == 0 {
            return false;
        }
        self.telemetry_tick_counter += tick_ms;
        if self.telemetry_tick_counter >= self.telemetry_interval_ms {
            self.telemetry_tick_counter = 0;
            true
        } else {
            false
        }
    }

    /// Send a telemetry frame to the connected client.
    pub fn send_telemetry(&mut self, app: &AppService) {
        let telem = app.build_telemetry();
        let mut fbb = FlatBufferBuilder::with_capacity(256);

        let tf = fb::TelemetryFrame::create(
            &mut fbb,
            &fb::TelemetryFrameArgs {
                timestamp_ms: 0,
                state: fb::state_to_fb(telem.state),
                nh3_ppm: telem.nh3_ppm,
                nh3_avg_ppm: telem.nh3_avg_ppm,
                flow_ml_per_min: telem.flow_ml_per_min,
                temperature_c: telem.temperature_c,
                pump_duty: telem.pump_duty,
                uvc_duty: telem.uvc_duty,
                fault_flags: telem.fault_flags,
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: self.alloc_msg_id(),
                payload_type: fb::Payload::TelemetryFrame,
                payload: Some(tf.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.send_finished(&fbb);
    }

    /// Send a state-change event frame.
    pub fn send_state_change(&mut self, from: StateId, to: StateId) {
        let mut fbb = FlatBufferBuilder::with_capacity(64);

        let sc = fb::StateChangeEvent::create(
            &mut fbb,
            &fb::StateChangeEventArgs {
                from_state: fb::state_to_fb(from),
                to_state: fb::state_to_fb(to),
                timestamp_ms: 0,
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: self.alloc_msg_id(),
                payload_type: fb::Payload::StateChangeEvent,
                payload: Some(sc.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.send_finished(&fbb);
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn increment_ulp_wakes(&mut self) {
        self.ulp_wake_count = self.ulp_wake_count.saturating_add(1);
    }

    pub fn ota_mut(&mut self) -> &mut OtaManager {
        &mut self.ota
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Reset the session (e.g. on transport disconnect).
    pub fn reset_session(&mut self) {
        self.session.reset();
        self.telemetry_subscribed = false;
    }

    // ── Internal dispatch ─────────────────────────────────────

    fn dispatch_frame(
        &mut self,
        frame: &[u8],
        app: &mut AppService,
        hw: &mut impl ActuatorPort,
        sink: &mut impl EventSink,
        nvs: &mut dyn StoragePort,
    ) {
        let msg = match flatbuffers::root::<fb::Message>(frame) {
            Ok(m) => m,
            Err(e) => {
                warn!("RPC: invalid FlatBuffer: {:?}", e);
                return;
            }
        };

        let reply_to = msg.id();
        let payload_type = msg.payload_type();

        // ── Gate 1: Rate limiting ─────────────────────────────
        if !self.session.check_rate_limit() {
            warn!("RPC: rate limit exceeded");
            self.send_ack(reply_to, false, "rate limit exceeded");
            return;
        }

        // ── Gate 2: Public messages (no auth required) ────────
        match payload_type {
            fb::Payload::AuthChallengeRequest => {
                return self.handle_auth_challenge(reply_to);
            }
            fb::Payload::AuthVerifyRequest => {
                if let Some(req) = msg.payload_as_auth_verify_request() {
                    return self.handle_auth_verify(reply_to, req.session_id(), req.hmac());
                }
                return;
            }
            fb::Payload::GetDeviceInfoRequest => {
                info!("RPC: GetDeviceInfo");
                return self.send_device_info(reply_to);
            }
            _ => {}
        }

        // ── Gate 3: Authentication required ───────────────────
        if !self.session.is_authenticated() {
            warn!("RPC: unauthenticated request (type {:?})", payload_type);
            self.send_ack(reply_to, false, "authentication required");
            return;
        }

        // ── Gate 4: Sequence monotonicity ─────────────────────
        if !self.session.check_sequence(reply_to) {
            warn!("RPC: sequence check failed (msg_id={})", reply_to);
            self.send_ack(reply_to, false, "sequence check failed");
            return;
        }

        // ── Authenticated command dispatch ────────────────────
        match payload_type {
            fb::Payload::GetStatusRequest => {
                info!("RPC: GetStatus");
                self.send_status(app, reply_to);
            }

            fb::Payload::StartScrubRequest => {
                info!("RPC: StartScrub");
                app.handle_command(AppCommand::StartScrub, hw, sink);
                self.send_ack(reply_to, true, "scrub started");
            }

            fb::Payload::StopScrubRequest => {
                info!("RPC: StopScrub");
                app.handle_command(AppCommand::ForceState(StateId::Idle), hw, sink);
                self.send_ack(reply_to, true, "stopped");
            }

            fb::Payload::ClearFaultsRequest => {
                info!("RPC: ClearFaults");
                push_event(Event::CommandReceived);
                self.send_ack(reply_to, true, "faults clear requested");
            }

            fb::Payload::SetConfigRequest => {
                if let Some(cfg) = msg.payload_as_set_config_request() {
                    info!(
                        "RPC: SetConfig (nh3_act={:.1}, nh3_deact={:.1}, pump={}%, uvc={}%, purge={}s)",
                        cfg.nh3_activate_ppm(),
                        cfg.nh3_deactivate_ppm(),
                        cfg.pump_duty_percent(),
                        cfg.uvc_duty_percent(),
                        cfg.purge_duration_secs(),
                    );
                    let mut new_config = app.current_config();
                    new_config.nh3_activate_threshold_ppm = cfg.nh3_activate_ppm();
                    new_config.nh3_deactivate_threshold_ppm = cfg.nh3_deactivate_ppm();
                    new_config.pump_duty_percent = cfg.pump_duty_percent();
                    new_config.uvc_duty_percent = cfg.uvc_duty_percent();
                    new_config.purge_duration_secs = cfg.purge_duration_secs();
                    app.handle_command(AppCommand::UpdateConfig(new_config), hw, sink);
                    self.send_ack(reply_to, true, "config updated");
                }
            }

            fb::Payload::SetScheduleRequest => {
                if let Some(sched) = msg.payload_as_set_schedule_request() {
                    info!(
                        "RPC: SetSchedule interval={}s duration={}s quiet={}-{}",
                        sched.interval_secs(),
                        sched.duration_secs(),
                        sched.quiet_start_hour(),
                        sched.quiet_end_hour(),
                    );
                    push_event(Event::CommandReceived);
                    self.send_ack(reply_to, true, "schedule set");
                }
            }

            fb::Payload::CancelScheduleRequest => {
                info!("RPC: CancelSchedule");
                push_event(Event::CommandReceived);
                self.send_ack(reply_to, true, "schedule cancelled");
            }

            fb::Payload::SubscribeTelemetryRequest => {
                if let Some(sub) = msg.payload_as_subscribe_telemetry_request() {
                    self.telemetry_subscribed = true;
                    self.telemetry_interval_ms = sub.interval_ms();
                    info!(
                        "RPC: telemetry ON (interval={}ms)",
                        self.telemetry_interval_ms
                    );
                    self.send_ack(reply_to, true, "subscribed");
                }
            }

            fb::Payload::UnsubscribeTelemetryRequest => {
                self.telemetry_subscribed = false;
                info!("RPC: telemetry OFF");
                self.send_ack(reply_to, true, "unsubscribed");
            }

            // ── OTA ────────────────────────────────────────────
            fb::Payload::OtaBeginRequest => {
                if let Some(req) = msg.payload_as_ota_begin_request() {
                    let sha = req.sha256().map(|v| v.bytes()).unwrap_or(&[]);
                    match self.ota.begin(req.firmware_size(), sha) {
                        Ok(()) => self.send_ack(reply_to, true, "OTA started"),
                        Err(e) => {
                            let mut buf = heapless::String::<64>::new();
                            let _ = core::fmt::Write::write_fmt(&mut buf, format_args!("{}", e));
                            self.send_ack(reply_to, false, buf.as_str());
                        }
                    }
                }
            }

            fb::Payload::OtaChunkRequest => {
                if let Some(req) = msg.payload_as_ota_chunk_request() {
                    let data = req.data().map(|v| v.bytes()).unwrap_or(&[]);
                    match self.ota.write_chunk(req.offset(), data) {
                        Ok(written) => {
                            self.send_ota_progress(reply_to, true, written);
                        }
                        Err(e) => {
                            let mut buf = heapless::String::<64>::new();
                            let _ = core::fmt::Write::write_fmt(&mut buf, format_args!("{}", e));
                            self.send_ack(reply_to, false, buf.as_str());
                        }
                    }
                }
            }

            fb::Payload::OtaFinalizeRequest => {
                match self.ota.finalize() {
                    Ok(()) => {
                        self.send_ack(reply_to, true, "OTA finalized, rebooting");
                        self.ota.reboot();
                    }
                    Err(e) => {
                        let mut buf = heapless::String::<64>::new();
                        let _ = core::fmt::Write::write_fmt(&mut buf, format_args!("{}", e));
                        self.send_ack(reply_to, false, buf.as_str());
                    }
                }
            }

            // ── Diagnostics ───────────────────────────────────
            fb::Payload::GetDiagnosticsRequest => {
                info!("RPC: GetDiagnostics");
                self.send_diagnostics(app, reply_to, nvs);
            }

            fb::Payload::ClearDiagnosticsRequest => {
                info!("RPC: ClearDiagnostics");
                self.crash_log.clear(nvs);
                self.send_ack(reply_to, true, "crash log cleared");
            }

            other => {
                warn!("RPC: unhandled payload type {:?}", other);
                self.send_ack(reply_to, false, "unknown command");
            }
        }
    }

    // ── Auth handlers ─────────────────────────────────────────

    fn handle_auth_challenge(&mut self, reply_to: u32) {
        let (session_id, nonce) = self.session.begin_challenge();
        info!("RPC: AuthChallenge → session_id={session_id}");

        let mut fbb = FlatBufferBuilder::with_capacity(128);
        let nonce_vec = fbb.create_vector(&nonce);

        let resp = fb::AuthChallengeResponse::create(
            &mut fbb,
            &fb::AuthChallengeResponseArgs {
                nonce: Some(nonce_vec),
                session_id,
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: reply_to,
                payload_type: fb::Payload::AuthChallengeResponse,
                payload: Some(resp.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.send_finished(&fbb);
    }

    fn handle_auth_verify(
        &mut self,
        reply_to: u32,
        session_id: u32,
        hmac: Option<flatbuffers::Vector<'_, u8>>,
    ) {
        let hmac_bytes = match hmac {
            Some(v) => v.bytes(),
            None => {
                self.send_auth_verify_response(reply_to, false, "missing HMAC");
                return;
            }
        };

        let psk = &self.psk[..self.psk_len];
        let success = self.session.verify_response(session_id, hmac_bytes, psk);

        if success {
            info!("RPC: AuthVerify SUCCESS (session_id={session_id})");
            self.send_auth_verify_response(reply_to, true, "authenticated");
        } else {
            warn!("RPC: AuthVerify FAILED (session_id={session_id})");
            self.send_auth_verify_response(reply_to, false, "verification failed");
        }
    }

    fn send_auth_verify_response(&mut self, reply_to: u32, success: bool, message: &str) {
        let mut fbb = FlatBufferBuilder::with_capacity(128);
        let msg_str = fbb.create_string(message);

        let avr = fb::AuthVerifyResponse::create(
            &mut fbb,
            &fb::AuthVerifyResponseArgs {
                success,
                message: Some(msg_str),
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: reply_to,
                payload_type: fb::Payload::AuthVerifyResponse,
                payload: Some(avr.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.send_finished(&fbb);
    }

    // ── Response builders ─────────────────────────────────────

    fn send_status(&mut self, app: &AppService, reply_to: u32) {
        let telem = app.build_telemetry();
        let mut fbb = FlatBufferBuilder::with_capacity(128);

        let sr = fb::StatusResponse::create(
            &mut fbb,
            &fb::StatusResponseArgs {
                state: fb::state_to_fb(telem.state),
                nh3_ppm: telem.nh3_ppm,
                nh3_avg_ppm: telem.nh3_avg_ppm,
                flow_ml_per_min: telem.flow_ml_per_min,
                temperature_c: telem.temperature_c,
                tank_a_ok: telem.tank_a_ok,
                tank_b_ok: telem.tank_b_ok,
                pump_duty: telem.pump_duty,
                uvc_duty: telem.uvc_duty,
                fault_flags: telem.fault_flags,
                uptime_secs: 0,
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: reply_to,
                payload_type: fb::Payload::StatusResponse,
                payload: Some(sr.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.send_finished(&fbb);
    }

    fn send_device_info(&mut self, reply_to: u32) {
        let mut fbb = FlatBufferBuilder::with_capacity(128);

        let ver = fbb.create_string(env!("CARGO_PKG_VERSION"));
        let hw_rev = fbb.create_string("ESP32-S3-WROOM-1");
        let serial = fbb.create_string("PF-000000");

        let di = fb::DeviceInfoResponse::create(
            &mut fbb,
            &fb::DeviceInfoResponseArgs {
                firmware_version: Some(ver),
                hardware_revision: Some(hw_rev),
                serial_number: Some(serial),
                uptime_secs: 0,
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: reply_to,
                payload_type: fb::Payload::DeviceInfoResponse,
                payload: Some(di.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.send_finished(&fbb);
    }

    fn send_ack(&mut self, reply_to: u32, success: bool, message: &str) {
        let mut fbb = FlatBufferBuilder::with_capacity(128);

        let msg_str = fbb.create_string(message);
        let ack = fb::AckResponse::create(
            &mut fbb,
            &fb::AckResponseArgs {
                success,
                message: Some(msg_str),
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: reply_to,
                payload_type: fb::Payload::AckResponse,
                payload: Some(ack.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.send_finished(&fbb);
    }

    fn send_finished(&mut self, fbb: &FlatBufferBuilder<'_>) {
        let data = fbb.finished_data();
        if let Some(len) = encode_frame(data, &mut self.write_buf) {
            let _ = self.transport.write(&self.write_buf[..len]);
            let _ = self.transport.flush();
        }
    }

    fn send_ota_progress(&mut self, reply_to: u32, success: bool, bytes_written: u32) {
        let mut fbb = FlatBufferBuilder::with_capacity(64);
        let msg_str = fbb.create_string("chunk written");

        let resp = fb::OtaResponse::create(
            &mut fbb,
            &fb::OtaResponseArgs {
                success,
                message: Some(msg_str),
                bytes_written,
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: reply_to,
                payload_type: fb::Payload::OtaResponse,
                payload: Some(resp.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.send_finished(&fbb);
    }

    fn send_diagnostics(&mut self, app: &AppService, reply_to: u32, nvs: &dyn StoragePort) {
        let time_adapter = crate::adapters::time::Esp32TimeAdapter::new();
        let uptime_secs = time_adapter.uptime_us() / 1_000_000;

        let crash_count = self.crash_log.count(nvs) as u32;
        let crash_entries_raw = self.crash_log.read_all(nvs);

        let metrics = crate::diagnostics::RuntimeMetrics::collect(
            uptime_secs,
            app.tick_count(),
            app.fault_flags() as u32,
            crash_count,
            self.ulp_wake_count,
        );

        let mut fbb = FlatBufferBuilder::with_capacity(512);

        let mut crash_entries_vec: heapless::Vec<flatbuffers::WIPOffset<fb::CrashEntryFbs>, 4> =
            heapless::Vec::new();
        for entry in &crash_entries_raw {
            let reason_str = fbb.create_string(entry.reason.as_str());
            let fbs_entry = fb::CrashEntryFbs::create(
                &mut fbb,
                &fb::CrashEntryFbsArgs {
                    uptime_secs: entry.uptime_secs,
                    pc: entry.pc,
                    reason: Some(reason_str),
                    backtrace: None,
                },
            );
            let _ = crash_entries_vec.push(fbs_entry);
        }
        let crash_vector = fbb.create_vector(crash_entries_vec.as_slice());

        let dr = fb::DiagnosticsResponse::create(
            &mut fbb,
            &fb::DiagnosticsResponseArgs {
                uptime_secs: metrics.uptime_secs,
                control_cycles: metrics.control_cycles,
                fault_count: metrics.fault_count,
                crash_count: metrics.crash_count,
                heap_free: metrics.heap_free,
                heap_min_free: metrics.heap_min_free,
                wifi_rssi: metrics.wifi_rssi,
                nvs_free_entries: metrics.nvs_free_entries,
                ulp_wake_count: metrics.ulp_wake_count,
                crash_entries: Some(crash_vector),
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: reply_to,
                payload_type: fb::Payload::DiagnosticsResponse,
                payload: Some(dr.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.send_finished(&fbb);
    }
}
