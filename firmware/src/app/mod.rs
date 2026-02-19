//! Application core — pure domain logic, zero I/O.
//!
//! This module contains the business rules for the PetFilter system:
//! FSM orchestration, safety evaluation, PID control, and scheduling.
//! All interaction with hardware happens through **port traits** defined
//! in [`ports`], keeping this layer fully testable without real peripherals.

pub mod commands;
pub mod events;
pub mod ports;
pub mod service;
