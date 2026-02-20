//! Transport-agnostic, multi-client RPC subsystem.
//!
//! Binary-efficient, streaming-capable RPC layer built on FlatBuffers.
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │                      RPC Stack (v2)                        │
//! │                                                            │
//! │  ┌──────────┐   ┌──────────┐   ┌───────────────────────┐ │
//! │  │ I/O Task │──▶│  Codec   │──▶│  Engine (dispatcher)  │ │
//! │  │ (async)  │   │ (framing)│   │  → AppService         │ │
//! │  └──────────┘   └──────────┘   └───────────────────────┘ │
//! │       ▲    ▲                              │              │
//! │       │    │  embassy-sync  ┌─────────────┘              │
//! │       │    │  channels      ▼                            │
//! │  ┌──────────┐           ┌──────────┐                     │
//! │  │Transport │           │ Channels │ (CMD/RESP/DISC)     │
//! │  │(TLS/BLE) │◀──────── │ (static) │                     │
//! │  └──────────┘           └──────────┘                     │
//! └────────────────────────────────────────────────────────────┘
//! ```

pub mod auth;
pub mod channels;
pub mod chunked;
pub mod codec;
pub mod compress;
pub mod engine;
pub mod fb;
pub mod io_task;
pub mod ota;
pub mod stream;
pub mod transport;
