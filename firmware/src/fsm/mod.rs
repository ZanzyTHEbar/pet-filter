//! Function-pointer finite state machine engine.
//!
//! Classic embedded FSM pattern ported to Rust:
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │  StateTable                                              │
//! │  ┌────────┬───────────┬──────────┬───────────────────┐   │
//! │  │ StateId │ on_enter  │ on_exit  │ on_update         │   │
//! │  ├────────┼───────────┼──────────┼───────────────────┤   │
//! │  │ Idle    │ fn(ctx)   │ fn(ctx)  │ fn(ctx)->Option<> │   │
//! │  │ Sensing │ fn(ctx)   │ fn(ctx)  │ fn(ctx)->Option<> │   │
//! │  │ Active  │ fn(ctx)   │ fn(ctx)  │ fn(ctx)->Option<> │   │
//! │  │ Purging │ fn(ctx)   │ fn(ctx)  │ fn(ctx)->Option<> │   │
//! │  │ Error   │ fn(ctx)   │ fn(ctx)  │ fn(ctx)->Option<> │   │
//! │  └────────┴───────────┴──────────┴───────────────────┘   │
//! └──────────────────────────────────────────────────────────┘
//! ```
//!
//! Each tick the engine calls `on_update` for the **current** state.
//! If it returns `Some(next_id)`, the engine runs `on_exit` for the
//! current state, then `on_enter` for the next, and updates the
//! current pointer.  All functions receive `&mut FsmContext` which
//! holds sensor readings, actuator handles, config, and timing.

pub mod context;
pub mod states;

use context::FsmContext;
use log::info;

// ---------------------------------------------------------------------------
// State identity
// ---------------------------------------------------------------------------

/// Enumeration of all possible system states.
/// Must stay in sync with the state table built in [`states::build_state_table`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StateId {
    Idle    = 0,
    Sensing = 1,
    Active  = 2,
    Purging = 3,
    Error   = 4,
}

impl StateId {
    /// Total number of states — used to size the table array.
    pub const COUNT: usize = 5;

    /// Convert a `u8` index back to `StateId`.  Panics on out-of-range in
    /// debug builds; returns `Error` in release (safe fallback).
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Idle,
            1 => Self::Sensing,
            2 => Self::Active,
            3 => Self::Purging,
            4 => Self::Error,
            _ => {
                debug_assert!(false, "invalid state index: {idx}");
                Self::Error
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Function-pointer type aliases
// ---------------------------------------------------------------------------

/// Signature for `on_enter` and `on_exit` actions.
/// These run exactly once on each state transition.
pub type StateActionFn = fn(&mut FsmContext);

/// Signature for the per-tick update handler.
/// Returns `Some(next)` to trigger a transition, or `None` to stay.
pub type StateUpdateFn = fn(&mut FsmContext) -> Option<StateId>;

// ---------------------------------------------------------------------------
// State descriptor (one row in the table)
// ---------------------------------------------------------------------------

/// Static descriptor for a single FSM state.
/// Stored in a fixed-size array — no heap, no `dyn`.
pub struct StateDescriptor {
    pub id: StateId,
    pub name: &'static str,
    pub on_enter: Option<StateActionFn>,
    pub on_exit: Option<StateActionFn>,
    pub on_update: StateUpdateFn,
}

// ---------------------------------------------------------------------------
// FSM engine
// ---------------------------------------------------------------------------

/// The finite state machine engine.
///
/// Owns the state table (array of [`StateDescriptor`]) and a mutable
/// [`FsmContext`] that is threaded through every handler call.
pub struct Fsm {
    /// Fixed-size table indexed by `StateId as usize`.
    table: [StateDescriptor; StateId::COUNT],
    /// Index of the currently active state.
    current: usize,
    /// Monotonically increasing tick counter (wraps at u64::MAX).
    tick_count: u64,
    /// Tick at which the current state was entered.
    state_entry_tick: u64,
}

impl Fsm {
    /// Construct a new FSM with the given state table, starting in `initial`.
    pub fn new(table: [StateDescriptor; StateId::COUNT], initial: StateId) -> Self {
        Self {
            table,
            current: initial as usize,
            tick_count: 0,
            state_entry_tick: 0,
        }
    }

    /// Run the initial `on_enter` for the starting state.
    /// Call once after construction, before the first `tick()`.
    pub fn start(&mut self, ctx: &mut FsmContext) {
        info!("FSM starting in state: {}", self.table[self.current].name);
        if let Some(enter) = self.table[self.current].on_enter {
            enter(ctx);
        }
    }

    /// Advance the FSM by one tick.
    ///
    /// 1. Call `on_update` for the current state.
    /// 2. If it returns `Some(next)`, execute the transition:
    ///    `on_exit(current)` → update pointer → `on_enter(next)`.
    /// 3. Increment tick counter.
    pub fn tick(&mut self, ctx: &mut FsmContext) {
        self.tick_count += 1;
        ctx.ticks_in_state = self.tick_count - self.state_entry_tick;
        ctx.total_ticks = self.tick_count;

        let next = (self.table[self.current].on_update)(ctx);

        if let Some(next_id) = next {
            self.transition(next_id, ctx);
        }
    }

    /// Force an immediate transition (used by the safety supervisor to
    /// jump to `Error` regardless of what `on_update` returned).
    pub fn force_transition(&mut self, next: StateId, ctx: &mut FsmContext) {
        if next as usize != self.current {
            self.transition(next, ctx);
        }
    }

    /// The current state's identity.
    pub fn current_state(&self) -> StateId {
        StateId::from_index(self.current)
    }

    /// How many ticks the FSM has been in the current state.
    pub fn ticks_in_current_state(&self) -> u64 {
        self.tick_count - self.state_entry_tick
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    fn transition(&mut self, next_id: StateId, ctx: &mut FsmContext) {
        let next_idx = next_id as usize;

        info!(
            "FSM transition: {} -> {}",
            self.table[self.current].name, self.table[next_idx].name
        );

        // Exit current state
        if let Some(exit) = self.table[self.current].on_exit {
            exit(ctx);
        }

        // Update pointer and timing
        self.current = next_idx;
        self.state_entry_tick = self.tick_count;
        ctx.ticks_in_state = 0;

        // Enter new state
        if let Some(enter) = self.table[self.current].on_enter {
            enter(ctx);
        }
    }
}
