//! FlatBuffers generated types for the PetFilter RPC protocol.
//!
//! Wraps the `flatc --rust` output from `schemas/petfilter.fbs`.
//! Regeneration happens in `build.rs` when `flatc` is on PATH;
//! otherwise the checked-in `generated/petfilter_generated.rs` is
//! used directly.

// Suppress lints in generated code — it uses `unsafe fn` calling `unsafe fn`
// without explicit inner unsafe blocks (pre-2024-edition pattern).
#[allow(warnings, unsafe_op_in_unsafe_fn)]
#[path = "generated/petfilter_generated.rs"]
mod petfilter_generated;

pub use petfilter_generated::petfilter::rpc::*;

use crate::fsm::StateId;

pub fn state_to_fb(state: StateId) -> DeviceState {
    match state {
        StateId::Idle => DeviceState::Idle,
        StateId::Sensing => DeviceState::Sensing,
        StateId::Active => DeviceState::Active,
        StateId::Purging => DeviceState::Purging,
        StateId::Error => DeviceState::Error,
    }
}

pub fn fb_to_state(state: DeviceState) -> StateId {
    match state.0 {
        0 => StateId::Idle,
        1 => StateId::Sensing,
        2 => StateId::Active,
        3 => StateId::Purging,
        _ => StateId::Error,
    }
}
