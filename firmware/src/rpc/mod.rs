//! Transport-agnostic RPC subsystem.
//!
//! Binary-efficient, streaming-capable RPC layer built on FlatBuffers.
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │                      RPC Stack                             │
//! │                                                            │
//! │  ┌──────────┐   ┌──────────┐   ┌───────────────────────┐ │
//! │  │ Transport │──▶│  Codec   │──▶│  Engine (dispatcher)  │ │
//! │  │ (trait)   │   │ (framing)│   │  → AppService         │ │
//! │  └──────────┘   └──────────┘   └───────────────────────┘ │
//! │       ▲                                    │              │
//! │       │              ┌─────────────────────┘              │
//! │       │              ▼                                    │
//! │  ┌──────────┐   ┌──────────┐                             │
//! │  │ Transport │◀──│  Stream  │   (continuous telemetry)   │
//! │  │ (write)   │   │ (push)   │                            │
//! │  └──────────┘   └──────────┘                             │
//! └────────────────────────────────────────────────────────────┘
//! ```

pub mod auth;
pub mod codec;
pub mod engine;
pub mod fb;
pub mod stream;
pub mod transport;
pub mod ota;
