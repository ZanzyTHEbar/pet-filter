//! RPC event sink adapter.
//!
//! Implements [`EventSink`] by serialising [`AppEvent`]s into FlatBuffer
//! `Message` frames and writing them to a response channel. This is the
//! "push" direction — the device sends events to connected clients.

use flatbuffers::FlatBufferBuilder;
use log::info;

use crate::app::events::AppEvent;
use crate::app::ports::EventSink;
use crate::rpc::auth::{ClientId, MAX_CLIENTS};
use crate::rpc::codec::encode_frame;
use crate::rpc::fb;

/// Adapter that bridges AppEvents to the RPC channel layer for
/// push-direction events (state changes, faults).
///
pub struct RpcEventSink {
    subscribed: [bool; MAX_CLIENTS],
    msg_id: u32,
}

impl Default for RpcEventSink {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcEventSink {
    pub fn new() -> Self {
        Self {
            subscribed: [false; MAX_CLIENTS],
            msg_id: 0x8000_0000,
        }
    }

    fn alloc_msg_id(&mut self) -> u32 {
        let id = self.msg_id;
        self.msg_id = self.msg_id.wrapping_add(1);
        id
    }

    /// Subscribe a client to push events.
    pub fn subscribe(&mut self, client_id: ClientId) {
        if let Some(s) = self.subscribed.get_mut(client_id as usize) {
            *s = true;
            info!("RPC sink: client {} subscribed to events", client_id);
        }
    }

    /// Unsubscribe a client from push events.
    pub fn unsubscribe(&mut self, client_id: ClientId) {
        if let Some(s) = self.subscribed.get_mut(client_id as usize) {
            *s = false;
        }
    }

    /// Build and enqueue an event frame to all subscribed clients.
    fn broadcast_frame(&mut self, fbb: &FlatBufferBuilder<'_>) {
        let payload = fbb.finished_data();
        let mut buf = [0u8; 256];
        let Some(len) = encode_frame(payload, &mut buf) else {
            return;
        };

        for (i, sub) in self.subscribed.iter().enumerate() {
            if !sub {
                continue;
            }
            let mut data = heapless::Vec::new();
            if data.extend_from_slice(&buf[..len]).is_ok() {
                crate::rpc::io_task::send_response(i as ClientId, data);
            }
        }
    }
}

impl EventSink for RpcEventSink {
    fn emit(&mut self, event: &AppEvent) {
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
                self.broadcast_frame(&fbb);
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
                self.broadcast_frame(&fbb);
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
                self.broadcast_frame(&fbb);
            }

            _ => {}
        }
    }
}
