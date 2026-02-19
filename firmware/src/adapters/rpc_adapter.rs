//! RPC event sink adapter.
//!
//! Implements [`EventSink`] by serialising [`AppEvent`]s into FlatBuffer
//! `Message` frames and writing them to the transport.  This is the
//! "push" direction — the device sends events to the connected client
//! without being asked.

use flatbuffers::FlatBufferBuilder;
use log::info;

use crate::app::events::AppEvent;
use crate::app::ports::EventSink;
use crate::rpc::codec::encode_frame;
use crate::rpc::fb;
use crate::rpc::transport::Transport;

/// Adapter that bridges AppEvents to the RPC transport.
pub struct RpcEventSink<T: Transport> {
    transport: Option<T>,
    write_buf: [u8; 256],
    enabled: bool,
    msg_id: u32,
}

impl<T: Transport> RpcEventSink<T> {
    pub fn new() -> Self {
        Self {
            transport: None,
            write_buf: [0; 256],
            enabled: false,
            msg_id: 0x8000_0000,
        }
    }

    fn alloc_msg_id(&mut self) -> u32 {
        let id = self.msg_id;
        self.msg_id = self.msg_id.wrapping_add(1);
        id
    }

    /// Attach a transport (called when an RPC client connects).
    pub fn attach(&mut self, transport: T) {
        self.transport = Some(transport);
        self.enabled = true;
        info!("RPC sink: transport attached");
    }

    /// Detach the transport (called on disconnect).
    pub fn detach(&mut self) -> Option<T> {
        self.enabled = false;
        self.transport.take()
    }

    pub fn is_attached(&self) -> bool {
        self.enabled && self.transport.is_some()
    }

    fn send_finished(&mut self, fbb: &FlatBufferBuilder<'_>) {
        let transport = match &mut self.transport {
            Some(t) => t,
            None => return,
        };
        let data = fbb.finished_data();
        if let Some(len) = encode_frame(data, &mut self.write_buf) {
            let _ = transport.write(&self.write_buf[..len]);
            let _ = transport.flush();
        }
    }
}

impl<T: Transport> EventSink for RpcEventSink<T> {
    fn emit(&mut self, event: &AppEvent) {
        if !self.enabled || self.transport.is_none() {
            return;
        }

        match event {
            AppEvent::StateChanged { from, to } => {
                let mut fbb = FlatBufferBuilder::with_capacity(64);
                let sc = fb::StateChangeEvent::create(
                    &mut fbb,
                    &fb::StateChangeEventArgs {
                        from_state: fb::state_to_fb(*from),
                        to_state: fb::state_to_fb(*to),
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

            AppEvent::FaultDetected(flags) => {
                let mut fbb = FlatBufferBuilder::with_capacity(64);
                let fe = fb::FaultEvent::create(
                    &mut fbb,
                    &fb::FaultEventArgs {
                        fault_flags: *flags,
                        is_clear: false,
                        timestamp_ms: 0,
                    },
                );
                let msg = fb::Message::create(
                    &mut fbb,
                    &fb::MessageArgs {
                        id: self.alloc_msg_id(),
                        payload_type: fb::Payload::FaultEvent,
                        payload: Some(fe.as_union_value()),
                    },
                );
                fbb.finish(msg, None);
                self.send_finished(&fbb);
            }

            AppEvent::FaultCleared => {
                let mut fbb = FlatBufferBuilder::with_capacity(64);
                let fe = fb::FaultEvent::create(
                    &mut fbb,
                    &fb::FaultEventArgs {
                        fault_flags: 0,
                        is_clear: true,
                        timestamp_ms: 0,
                    },
                );
                let msg = fb::Message::create(
                    &mut fbb,
                    &fb::MessageArgs {
                        id: self.alloc_msg_id(),
                        payload_type: fb::Payload::FaultEvent,
                        payload: Some(fe.as_union_value()),
                    },
                );
                fbb.finish(msg, None);
                self.send_finished(&fbb);
            }

            _ => {}
        }
    }
}
