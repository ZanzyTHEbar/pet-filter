//! RPC inter-task communication channels.
//!
//! Uses `embassy-sync` bounded MPMC channels to bridge the async I/O
//! task with the synchronous control loop. Both tasks share these
//! static channels without heap allocation.
//!
//! ```text
//! ┌──────────────┐  CommandMsg  ┌──────────────┐
//! │   I/O Task   │────────────▶│  Control Loop │
//! │  (async)     │◀────────────│  (sync)       │
//! └──────────────┘  ResponseMsg └──────────────┘
//! ```

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use heapless::Vec;

use super::auth::ClientId;

/// Inbound command from a client, delivered to the control loop.
pub struct CommandMsg {
    /// Which client sent this frame.
    pub client_id: ClientId,
    /// Raw frame bytes (already decoded from length-prefix framing).
    pub frame: Vec<u8, 4096>,
}

/// Outbound response from the control loop, delivered to the I/O task.
pub struct ResponseMsg {
    /// Which client this response is destined for.
    pub client_id: ClientId,
    /// Length-prefix-encoded response bytes (ready to write to transport).
    pub data: Vec<u8, 512>,
}

/// Channel depth for command (inbound) messages.
const CMD_DEPTH: usize = 8;

/// Channel depth for response (outbound) messages.
const RESP_DEPTH: usize = 16;

/// Inbound command channel: I/O task → control loop.
pub static CMD_CHANNEL: Channel<CriticalSectionRawMutex, CommandMsg, CMD_DEPTH> = Channel::new();

/// Outbound response channel: control loop → I/O task.
pub static RESP_CHANNEL: Channel<CriticalSectionRawMutex, ResponseMsg, RESP_DEPTH> = Channel::new();

/// Notification channel for client disconnect events.
pub struct DisconnectMsg {
    pub client_id: ClientId,
}

pub static DISCONNECT_CHANNEL: Channel<CriticalSectionRawMutex, DisconnectMsg, 4> = Channel::new();
