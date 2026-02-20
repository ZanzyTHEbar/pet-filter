//! RPC engine — dispatches incoming FlatBuffer messages to the AppService.
//!
//! **Transport-decoupled**: the engine does not own a transport. Instead,
//! callers feed `(client_id, frame_bytes)` via [`dispatch`] and receive
//! serialized response frames. The I/O task (Phase 2) bridges the transport
//! layer to this engine via channels.
//!
//! All messages pass through a three-gate pipeline:
//!
//! 1. **Rate limiting** — token-bucket rejects bursts (via `burster`).
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
use crate::events::{Event, push_event};
use crate::fsm::StateId;

use super::auth::{ClientId, MAX_CLIENTS, SessionTable};
use super::codec::{FrameDecoder, encode_frame};
use super::fb;
use super::ota::OtaManager;
use crate::adapters::cert_store::{CertStore, TlsMode as CertTlsMode};

const OTA_VERSION_NAMESPACE: &str = "ota";
const OTA_VERSION_KEY: &str = "fw_version";

/// Response frame produced by the engine, tagged with destination client.
pub struct ResponseFrame {
    pub client_id: ClientId,
    pub data: heapless::Vec<u8, 512>,
}

/// Transport-decoupled RPC engine with multi-client session table.
pub struct RpcEngine {
    sessions: SessionTable,
    decoders: [FrameDecoder; MAX_CLIENTS],
    psk: [u8; 32],
    psk_len: usize,
    telemetry_subscribed: [bool; MAX_CLIENTS],
    telemetry_interval_ms: [u32; MAX_CLIENTS],
    telemetry_tick_counter: [u32; MAX_CLIENTS],
    next_msg_id: u32,
    ota: OtaManager,
    ulp_wake_count: u32,
    crash_log: CrashLog,
    cert_store: CertStore,
    ota_pending_version: Option<u32>,
}

impl RpcEngine {
    pub fn new(psk: &[u8]) -> Self {
        let mut psk_buf = [0u8; 32];
        let psk_len = psk.len().min(32);
        psk_buf[..psk_len].copy_from_slice(&psk[..psk_len]);

        Self {
            sessions: SessionTable::new(),
            decoders: core::array::from_fn(|_| FrameDecoder::new()),
            psk: psk_buf,
            psk_len,
            telemetry_subscribed: [false; MAX_CLIENTS],
            telemetry_interval_ms: [1000; MAX_CLIENTS],
            telemetry_tick_counter: [0; MAX_CLIENTS],
            next_msg_id: 1,
            ota: OtaManager::new(),
            ulp_wake_count: 0,
            crash_log: CrashLog::new(),
            cert_store: CertStore::new(CertTlsMode::PskOnly),
            ota_pending_version: None,
        }
    }

    /// Initialise the crash log from persistent NVS storage.
    pub fn init_crash_log(&mut self, nvs: &dyn StoragePort) {
        self.crash_log.init(nvs);
    }

    fn alloc_msg_id(&mut self) -> u32 {
        let id = self.next_msg_id;
        self.next_msg_id = self.next_msg_id.wrapping_add(1);
        id
    }

    /// Feed raw bytes from a client into the decoder and dispatch any
    /// complete frames. Returns a response frame if one was generated.
    pub fn feed_bytes(
        &mut self,
        client_id: ClientId,
        data: &[u8],
        app: &mut AppService,
        hw: &mut impl ActuatorPort,
        sink: &mut impl EventSink,
        nvs: &mut dyn StoragePort,
    ) -> Option<ResponseFrame> {
        let idx = client_id as usize;
        if idx >= MAX_CLIENTS {
            return None;
        }

        if let Some(frame) = self.decoders[idx].feed(data) {
            let mut frame_copy = [0u8; 4096];
            let frame_len = frame.len().min(frame_copy.len());
            frame_copy[..frame_len].copy_from_slice(&frame[..frame_len]);
            return self.dispatch_frame(client_id, &frame_copy[..frame_len], app, hw, sink, nvs);
        }
        None
    }

    /// Dispatch a complete frame from a specific client. Returns the
    /// serialized response frame (if any).
    pub fn dispatch(
        &mut self,
        client_id: ClientId,
        frame: &[u8],
        app: &mut AppService,
        hw: &mut impl ActuatorPort,
        sink: &mut impl EventSink,
        nvs: &mut dyn StoragePort,
    ) -> Option<ResponseFrame> {
        self.dispatch_frame(client_id, frame, app, hw, sink, nvs)
    }

    /// Build a telemetry frame for a specific client (if subscribed).
    pub fn build_telemetry_frame(
        &mut self,
        client_id: ClientId,
        app: &AppService,
        wifi_rssi: Option<i8>,
    ) -> Option<ResponseFrame> {
        let idx = client_id as usize;
        if idx >= MAX_CLIENTS || !self.telemetry_subscribed[idx] {
            return None;
        }

        let telem = app.build_telemetry(wifi_rssi);
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
                wifi_rssi: telem.wifi_rssi.unwrap_or(127),
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
        self.encode_response(client_id, &fbb)
    }

    /// Check if a client's telemetry timer has elapsed.
    pub fn should_stream_telemetry(&mut self, client_id: ClientId, tick_ms: u32) -> bool {
        let idx = client_id as usize;
        if idx >= MAX_CLIENTS
            || !self.telemetry_subscribed[idx]
            || self.telemetry_interval_ms[idx] == 0
        {
            return false;
        }
        self.telemetry_tick_counter[idx] += tick_ms;
        if self.telemetry_tick_counter[idx] >= self.telemetry_interval_ms[idx] {
            self.telemetry_tick_counter[idx] = 0;
            true
        } else {
            false
        }
    }

    /// Build a state-change event frame for broadcast.
    pub fn build_state_change_frame(
        &mut self,
        client_id: ClientId,
        from: StateId,
        to: StateId,
    ) -> Option<ResponseFrame> {
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
        self.encode_response(client_id, &fbb)
    }

    pub fn increment_ulp_wakes(&mut self) {
        self.ulp_wake_count = self.ulp_wake_count.saturating_add(1);
    }

    pub fn ota_mut(&mut self) -> &mut OtaManager {
        &mut self.ota
    }

    pub fn sessions(&self) -> &SessionTable {
        &self.sessions
    }

    /// Reset a client's session and telemetry state (e.g. on disconnect).
    pub fn reset_client(&mut self, client_id: ClientId) {
        let idx = client_id as usize;
        self.sessions.reset_client(client_id);
        if idx < MAX_CLIENTS {
            self.telemetry_subscribed[idx] = false;
            self.telemetry_tick_counter[idx] = 0;
            self.decoders[idx].reset();
        }
    }

    // ── Internal dispatch ─────────────────────────────────────

    fn dispatch_frame(
        &mut self,
        client_id: ClientId,
        frame: &[u8],
        app: &mut AppService,
        hw: &mut impl ActuatorPort,
        sink: &mut impl EventSink,
        nvs: &mut dyn StoragePort,
    ) -> Option<ResponseFrame> {
        let msg = match flatbuffers::root::<fb::Message>(frame) {
            Ok(m) => m,
            Err(e) => {
                warn!("RPC[{}]: invalid FlatBuffer: {:?}", client_id, e);
                return None;
            }
        };

        let reply_to = msg.id();
        let payload_type = msg.payload_type();

        let session = self.sessions.get_mut(client_id)?;

        // ── Gate 1: Rate limiting ─────────────────────────────
        if !session.check_rate_limit() {
            warn!("RPC[{}]: rate limit exceeded", client_id);
            return self.build_ack(client_id, reply_to, false, "rate limit exceeded");
        }

        // ── Gate 2: Public messages (no auth required) ────────
        match payload_type {
            fb::Payload::AuthChallengeRequest => {
                return self.handle_auth_challenge(client_id, reply_to);
            }
            fb::Payload::AuthVerifyRequest => {
                if let Some(req) = msg.payload_as_auth_verify_request() {
                    return self.handle_auth_verify(
                        client_id,
                        reply_to,
                        req.session_id(),
                        req.hmac(),
                    );
                }
                return None;
            }
            fb::Payload::GetDeviceInfoRequest => {
                info!("RPC[{}]: GetDeviceInfo", client_id);
                return self.build_device_info(client_id, reply_to);
            }
            _ => {}
        }

        // Re-borrow session after the match consumed it
        let session = self.sessions.get_mut(client_id)?;

        // ── Gate 3: Authentication required ───────────────────
        if !session.is_authenticated() {
            warn!(
                "RPC[{}]: unauthenticated request (type {:?})",
                client_id, payload_type
            );
            return self.build_ack(client_id, reply_to, false, "authentication required");
        }

        // ── Gate 4: Sequence monotonicity ─────────────────────
        if !session.check_sequence(reply_to) {
            warn!(
                "RPC[{}]: sequence check failed (msg_id={})",
                client_id, reply_to
            );
            return self.build_ack(client_id, reply_to, false, "sequence check failed");
        }

        // ── Authenticated command dispatch ────────────────────
        let idx = client_id as usize;
        match payload_type {
            fb::Payload::GetStatusRequest => {
                info!("RPC[{}]: GetStatus", client_id);
                self.build_status(client_id, app, reply_to)
            }

            fb::Payload::StartScrubRequest => {
                info!("RPC[{}]: StartScrub", client_id);
                app.handle_command(AppCommand::StartScrub, hw, sink);
                self.build_ack(client_id, reply_to, true, "scrub started")
            }

            fb::Payload::StopScrubRequest => {
                info!("RPC[{}]: StopScrub", client_id);
                app.handle_command(AppCommand::ForceState(StateId::Idle), hw, sink);
                self.build_ack(client_id, reply_to, true, "stopped")
            }

            fb::Payload::ClearFaultsRequest => {
                info!("RPC[{}]: ClearFaults", client_id);
                push_event(Event::CommandReceived);
                self.build_ack(client_id, reply_to, true, "faults clear requested")
            }

            fb::Payload::SetConfigRequest => {
                if let Some(cfg) = msg.payload_as_set_config_request() {
                    info!(
                        "RPC[{}]: SetConfig (nh3_act={:.1}, nh3_deact={:.1}, pump={}%, uvc={}%, purge={}s)",
                        client_id,
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
                    self.build_ack(client_id, reply_to, true, "config updated")
                } else {
                    None
                }
            }

            fb::Payload::SetScheduleRequest => {
                if let Some(sched) = msg.payload_as_set_schedule_request() {
                    info!(
                        "RPC[{}]: SetSchedule interval={}s duration={}s quiet={}-{}",
                        client_id,
                        sched.interval_secs(),
                        sched.duration_secs(),
                        sched.quiet_start_hour(),
                        sched.quiet_end_hour(),
                    );
                    push_event(Event::CommandReceived);
                    self.build_ack(client_id, reply_to, true, "schedule set")
                } else {
                    None
                }
            }

            fb::Payload::CancelScheduleRequest => {
                info!("RPC[{}]: CancelSchedule", client_id);
                push_event(Event::CommandReceived);
                self.build_ack(client_id, reply_to, true, "schedule cancelled")
            }

            fb::Payload::SubscribeTelemetryRequest => {
                if let Some(sub) = msg.payload_as_subscribe_telemetry_request() {
                    if idx < MAX_CLIENTS {
                        self.telemetry_subscribed[idx] = true;
                        self.telemetry_interval_ms[idx] = sub.interval_ms();
                    }
                    info!(
                        "RPC[{}]: telemetry ON (interval={}ms)",
                        client_id,
                        sub.interval_ms()
                    );
                    self.build_ack(client_id, reply_to, true, "subscribed")
                } else {
                    None
                }
            }

            fb::Payload::UnsubscribeTelemetryRequest => {
                if idx < MAX_CLIENTS {
                    self.telemetry_subscribed[idx] = false;
                }
                info!("RPC[{}]: telemetry OFF", client_id);
                self.build_ack(client_id, reply_to, true, "unsubscribed")
            }

            // ── OTA ────────────────────────────────────────────
            fb::Payload::OtaBeginRequest => {
                if let Some(req) = msg.payload_as_ota_begin_request() {
                    let sha = req.sha256().map_or(&[] as &[u8], |v| v.bytes());
                    let version = req.version();
                    info!(
                        "RPC[{}]: OTA begin requested (size={}, version={})",
                        client_id,
                        req.firmware_size(),
                        version
                    );

                    let current_version = Self::read_monotonic_fw_version(nvs);
                    if version <= current_version {
                        warn!(
                            "RPC[{}]: OTA rollback rejected (incoming={}, current={})",
                            client_id,
                            version,
                            current_version
                        );
                        return self.build_ack(
                            client_id,
                            reply_to,
                            false,
                            "rollback rejected: version must increase",
                        );
                    }

                    match self.ota.begin(req.firmware_size(), sha) {
                        Ok(()) => {
                            self.ota_pending_version = Some(version);
                            self.build_ack(client_id, reply_to, true, "OTA started")
                        }
                        Err(e) => {
                            self.ota_pending_version = None;
                            let mut buf = heapless::String::<64>::new();
                            let _ = core::fmt::Write::write_fmt(&mut buf, format_args!("{}", e));
                            self.build_ack(client_id, reply_to, false, buf.as_str())
                        }
                    }
                } else {
                    None
                }
            }

            fb::Payload::OtaChunkRequest => {
                if let Some(req) = msg.payload_as_ota_chunk_request() {
                    let data = req.data().map_or(&[] as &[u8], |v| v.bytes());
                    match self.ota.write_chunk(req.offset(), data) {
                        Ok(written) => {
                            let total = match self.ota.state() {
                                super::ota::OtaState::Receiving { expected_size, .. } => {
                                    expected_size
                                }
                                _ => 0,
                            };
                            if let Some(evt) =
                                self.build_ota_progress_event(client_id, written, total)
                            {
                                super::io_task::send_response(evt.client_id, evt.data);
                            }
                            self.build_ota_progress(client_id, reply_to, true, written)
                        }
                        Err(e) => {
                            let mut buf = heapless::String::<64>::new();
                            let _ = core::fmt::Write::write_fmt(&mut buf, format_args!("{}", e));
                            self.build_ack(client_id, reply_to, false, buf.as_str())
                        }
                    }
                } else {
                    None
                }
            }

            fb::Payload::OtaFinalizeRequest => match self.ota.finalize() {
                Ok(()) => {
                    if let Some(version) = self.ota_pending_version.take() {
                        if !Self::write_monotonic_fw_version(nvs, version) {
                            warn!(
                                "RPC[{}]: OTA finalized but failed to persist version {}",
                                client_id,
                                version
                            );
                            return self.build_ack(
                                client_id,
                                reply_to,
                                false,
                                "OTA finalize failed: version persist error",
                            );
                        }
                    }

                    #[allow(unused_variables)]
                    let resp =
                        self.build_ack(client_id, reply_to, true, "OTA finalized, rebooting");
                    self.ota.reboot();
                    #[allow(unreachable_code)]
                    resp
                }
                Err(e) => {
                    self.ota_pending_version = None;
                    let mut buf = heapless::String::<64>::new();
                    let _ = core::fmt::Write::write_fmt(&mut buf, format_args!("{}", e));
                    self.build_ack(client_id, reply_to, false, buf.as_str())
                }
            },

            // ── Diagnostics ───────────────────────────────────
            fb::Payload::GetDiagnosticsRequest => {
                info!("RPC[{}]: GetDiagnostics", client_id);
                self.build_diagnostics(client_id, app, reply_to, nvs)
            }

            fb::Payload::ClearDiagnosticsRequest => {
                info!("RPC[{}]: ClearDiagnostics", client_id);
                self.crash_log.clear(nvs);
                self.build_ack(client_id, reply_to, true, "crash log cleared")
            }

            fb::Payload::ProvisionCertRequest => {
                info!("RPC[{}]: ProvisionCert", client_id);
                if let Some(req) = msg.payload_as_provision_cert_request() {
                    self.handle_provision_cert(client_id, reply_to, req)
                } else {
                    self.build_ack(client_id, reply_to, false, "malformed ProvisionCertRequest")
                }
            }

            fb::Payload::GetCertStatusRequest => {
                info!("RPC[{}]: GetCertStatus", client_id);
                self.build_cert_status(client_id, reply_to)
            }

            other => {
                warn!("RPC[{}]: unhandled payload type {:?}", client_id, other);
                self.build_ack(client_id, reply_to, false, "unknown command")
            }
        }
    }

    fn read_monotonic_fw_version(nvs: &dyn StoragePort) -> u32 {
        let mut buf = [0u8; 8];
        match nvs.read(OTA_VERSION_NAMESPACE, OTA_VERSION_KEY, &mut buf) {
            Ok(4) => u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            Ok(_) => {
                warn!("RPC: invalid stored OTA version length");
                0
            }
            Err(_) => 0,
        }
    }

    fn write_monotonic_fw_version(nvs: &mut dyn StoragePort, version: u32) -> bool {
        let bytes = version.to_le_bytes();
        nvs.write(OTA_VERSION_NAMESPACE, OTA_VERSION_KEY, &bytes)
            .is_ok()
    }


    // ── Auth handlers ─────────────────────────────────────────

    fn handle_auth_challenge(
        &mut self,
        client_id: ClientId,
        reply_to: u32,
    ) -> Option<ResponseFrame> {
        let session = self.sessions.get_mut(client_id)?;
        let (session_id, nonce) = session.begin_challenge();
        info!(
            "RPC[{}]: AuthChallenge -> session_id={session_id}",
            client_id
        );

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
        self.encode_response(client_id, &fbb)
    }

    fn handle_auth_verify(
        &mut self,
        client_id: ClientId,
        reply_to: u32,
        session_id: u32,
        hmac: Option<flatbuffers::Vector<'_, u8>>,
    ) -> Option<ResponseFrame> {
        let hmac_bytes = match hmac {
            Some(v) => v.bytes(),
            None => {
                return self.build_auth_verify_response(client_id, reply_to, false, "missing HMAC");
            }
        };

        let psk = &self.psk[..self.psk_len];
        let session = self.sessions.get_mut(client_id)?;
        let success = session.verify_response(session_id, hmac_bytes, psk);

        if success {
            info!(
                "RPC[{}]: AuthVerify SUCCESS (session_id={session_id})",
                client_id
            );
            self.build_auth_verify_response(client_id, reply_to, true, "authenticated")
        } else {
            warn!(
                "RPC[{}]: AuthVerify FAILED (session_id={session_id})",
                client_id
            );
            self.build_auth_verify_response(client_id, reply_to, false, "verification failed")
        }
    }

    fn build_auth_verify_response(
        &mut self,
        client_id: ClientId,
        reply_to: u32,
        success: bool,
        message: &str,
    ) -> Option<ResponseFrame> {
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
        self.encode_response(client_id, &fbb)
    }

    // ── Response builders ─────────────────────────────────────

    fn build_status(
        &mut self,
        client_id: ClientId,
        app: &AppService,
        reply_to: u32,
    ) -> Option<ResponseFrame> {
        let telem = app.build_telemetry(None);
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
        self.encode_response(client_id, &fbb)
    }

    fn build_device_info(&mut self, client_id: ClientId, reply_to: u32) -> Option<ResponseFrame> {
        let mut fbb = FlatBufferBuilder::with_capacity(128);

        let ver = fbb.create_string(env!("CARGO_PKG_VERSION"));
        let hw_rev = fbb.create_string("ESP32-S3-WROOM-1");
        let mac = crate::adapters::device_id::read_mac();
        let serial_str = crate::adapters::device_id::device_id(&mac);
        let serial = fbb.create_string(serial_str.as_str());

        let has_certs = self.cert_store.mode() != CertTlsMode::PskOnly;
        let caps: u32 = (1 << 0)  // compression supported
                      | (1 << 1)  // chunked transfer supported
                      | (if has_certs { 1 << 2 } else { 0 })  // cert_auth
                      | (1 << 3); // multi_client

        let di = fb::DeviceInfoResponse::create(
            &mut fbb,
            &fb::DeviceInfoResponseArgs {
                firmware_version: Some(ver),
                hardware_revision: Some(hw_rev),
                serial_number: Some(serial),
                uptime_secs: 0,
                capabilities: caps,
                max_clients: MAX_CLIENTS as u8,
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
        self.encode_response(client_id, &fbb)
    }

    fn build_ack(
        &mut self,
        client_id: ClientId,
        reply_to: u32,
        success: bool,
        message: &str,
    ) -> Option<ResponseFrame> {
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
        self.encode_response(client_id, &fbb)
    }

    fn build_ota_progress(
        &mut self,
        client_id: ClientId,
        reply_to: u32,
        success: bool,
        bytes_written: u32,
    ) -> Option<ResponseFrame> {
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
        self.encode_response(client_id, &fbb)
    }

    fn build_diagnostics(
        &mut self,
        client_id: ClientId,
        app: &AppService,
        reply_to: u32,
        nvs: &dyn StoragePort,
    ) -> Option<ResponseFrame> {
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
        self.encode_response(client_id, &fbb)
    }

    // ── Cert provisioning handlers ────────────────────────────

    fn handle_provision_cert(
        &mut self,
        client_id: ClientId,
        reply_to: u32,
        req: fb::ProvisionCertRequest<'_>,
    ) -> Option<ResponseFrame> {
        let ca = req.ca_cert().unwrap_or_default().bytes();
        let cert = req.device_cert().unwrap_or_default().bytes();
        let key = req.device_key().unwrap_or_default().bytes();

        if ca.is_empty() || cert.is_empty() || key.is_empty() {
            return self.build_ack(client_id, reply_to, false, "incomplete certificate bundle");
        }

        if let Err(e) = self.cert_store.store_cert("ca_cert", ca) {
            warn!("RPC[{}]: cert store CA failed: {}", client_id, e);
            return self.build_ack(client_id, reply_to, false, "failed to store CA cert");
        }
        if let Err(e) = self.cert_store.store_cert("server_cert", cert) {
            warn!("RPC[{}]: cert store cert failed: {}", client_id, e);
            return self.build_ack(client_id, reply_to, false, "failed to store device cert");
        }
        if let Err(e) = self.cert_store.store_cert("server_key", key) {
            warn!("RPC[{}]: cert store key failed: {}", client_id, e);
            return self.build_ack(client_id, reply_to, false, "failed to store device key");
        }

        self.cert_store.set_mode(CertTlsMode::PskAndCert);
        info!(
            "RPC[{}]: certificates provisioned, mode=PskAndCert",
            client_id
        );
        self.build_ack(client_id, reply_to, true, "certificates provisioned")
    }

    fn build_cert_status(&mut self, client_id: ClientId, reply_to: u32) -> Option<ResponseFrame> {
        let mode = match self.cert_store.mode() {
            CertTlsMode::PskOnly => fb::TlsMode::PskOnly,
            CertTlsMode::CertOnly => fb::TlsMode::CertOnly,
            CertTlsMode::PskAndCert => fb::TlsMode::PskAndCert,
        };

        let mut fbb = FlatBufferBuilder::with_capacity(128);
        let serial = fbb.create_string("PF-000000");

        let csr = fb::CertStatusResponse::create(
            &mut fbb,
            &fb::CertStatusResponseArgs {
                mode,
                ca_fingerprint: None,
                device_serial: Some(serial),
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: reply_to,
                payload_type: fb::Payload::CertStatusResponse,
                payload: Some(csr.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.encode_response(client_id, &fbb)
    }

    // ── OTA progress event builder ────────────────────────────

    pub fn build_ota_progress_event(
        &mut self,
        client_id: ClientId,
        bytes_written: u32,
        total_bytes: u32,
    ) -> Option<ResponseFrame> {
        let percent = if total_bytes > 0 {
            ((bytes_written as u64 * 100) / total_bytes as u64) as u8
        } else {
            0
        };

        let mut fbb = FlatBufferBuilder::with_capacity(64);
        let pe = fb::OtaProgressEvent::create(
            &mut fbb,
            &fb::OtaProgressEventArgs {
                bytes_written,
                total_bytes,
                percent,
            },
        );

        let msg = fb::Message::create(
            &mut fbb,
            &fb::MessageArgs {
                id: self.alloc_msg_id(),
                payload_type: fb::Payload::OtaProgressEvent,
                payload: Some(pe.as_union_value()),
            },
        );

        fbb.finish(msg, None);
        self.encode_response(client_id, &fbb)
    }

    /// Encode a finished FlatBufferBuilder into a length-prefixed ResponseFrame.
    fn encode_response(
        &self,
        client_id: ClientId,
        fbb: &FlatBufferBuilder<'_>,
    ) -> Option<ResponseFrame> {
        let payload = fbb.finished_data();
        let mut buf = [0u8; 512];
        let len = encode_frame(payload, &mut buf)?;

        let mut data = heapless::Vec::new();
        data.extend_from_slice(&buf[..len]).ok()?;

        Some(ResponseFrame { client_id, data })
    }
}
