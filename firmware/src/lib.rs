//! PetFilter firmware library.
//!
//! Exposes the pure-logic modules for integration testing and external
//! inspection. All ESP-IDF-specific code is guarded by
//! `#[cfg(target_os = "espidf")]` within each module.

#![deny(unused_must_use)]

pub mod app;
pub mod config;
pub mod diagnostics;
pub mod events;
pub mod fsm;
pub mod power;
pub mod rpc;
pub mod safety;
pub mod scheduler;

mod error;
mod pins;

// Re-export the ESPidf-only modules so the crate compiles; the actual
// implementations are guarded by cfg attributes inside.
pub mod adapters;
pub mod control;
pub mod drivers;
pub mod sensors;
