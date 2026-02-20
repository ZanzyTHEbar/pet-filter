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
    Idle = 0,
    Sensing = 1,
    Active = 2,
    Purging = 3,
    Error = 4,
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

#[cfg(test)]
mod tests {
    use super::context::FsmContext;
    use super::*;
    use crate::config::SystemConfig;

    fn make_ctx() -> FsmContext {
        FsmContext::new(SystemConfig::default())
    }

    fn make_fsm() -> Fsm {
        Fsm::new(states::build_state_table(), StateId::Idle)
    }

    #[test]
    fn starts_in_idle() {
        let fsm = make_fsm();
        assert_eq!(fsm.current_state(), StateId::Idle);
    }

    #[test]
    fn start_runs_on_enter() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);
        assert_eq!(ctx.commands.led_rgb, (0, 180, 148));
    }

    #[test]
    fn tick_increments_counter() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);
        fsm.tick(&mut ctx);
        assert_eq!(fsm.ticks_in_current_state(), 1);
        fsm.tick(&mut ctx);
        assert_eq!(fsm.ticks_in_current_state(), 2);
    }

    #[test]
    fn idle_to_sensing_on_nh3_spike() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);

        ctx.sensors.nh3_ppm = ctx.config.nh3_activate_threshold_ppm + 1.0;
        fsm.tick(&mut ctx);
        assert_eq!(fsm.current_state(), StateId::Sensing);
    }

    #[test]
    fn idle_stays_when_nh3_low() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);

        ctx.sensors.nh3_ppm = ctx.config.nh3_activate_threshold_ppm - 1.0;
        fsm.tick(&mut ctx);
        assert_eq!(fsm.current_state(), StateId::Idle);
    }

    #[test]
    fn sensing_to_idle_on_false_alarm() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);

        ctx.sensors.nh3_ppm = ctx.config.nh3_activate_threshold_ppm + 5.0;
        fsm.tick(&mut ctx);
        assert_eq!(fsm.current_state(), StateId::Sensing);

        ctx.sensors.nh3_ppm = ctx.config.nh3_activate_threshold_ppm - 1.0;
        ctx.sensors.nh3_avg_ppm = ctx.config.nh3_activate_threshold_ppm - 1.0;
        fsm.tick(&mut ctx);
        assert_eq!(fsm.current_state(), StateId::Idle);
    }

    #[test]
    fn sensing_to_active_after_confirmation() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);

        let threshold = ctx.config.nh3_activate_threshold_ppm;
        ctx.sensors.nh3_ppm = threshold + 5.0;
        ctx.sensors.nh3_avg_ppm = threshold + 5.0;
        fsm.tick(&mut ctx);
        assert_eq!(fsm.current_state(), StateId::Sensing);

        let ticks_needed =
            (ctx.config.nh3_confirm_duration_secs as f32 / ctx.tick_period_secs) as u64 + 1;
        for _ in 0..ticks_needed {
            fsm.tick(&mut ctx);
        }
        assert_eq!(fsm.current_state(), StateId::Active);
    }

    #[test]
    fn active_enables_actuators() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);

        let threshold = ctx.config.nh3_activate_threshold_ppm;
        ctx.sensors.nh3_ppm = threshold + 5.0;
        ctx.sensors.nh3_avg_ppm = threshold + 5.0;
        fsm.tick(&mut ctx);
        let ticks_needed =
            (ctx.config.nh3_confirm_duration_secs as f32 / ctx.tick_period_secs) as u64 + 1;
        for _ in 0..ticks_needed {
            fsm.tick(&mut ctx);
        }
        assert_eq!(fsm.current_state(), StateId::Active);
        assert!(ctx.commands.pump_duty > 0);
        assert!(ctx.commands.uvc_duty > 0);
    }

    #[test]
    fn active_to_purging_on_nh3_drop() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);
        fsm.force_transition(StateId::Active, &mut ctx);
        assert_eq!(fsm.current_state(), StateId::Active);

        ctx.sensors.nh3_avg_ppm = ctx.config.nh3_deactivate_threshold_ppm - 1.0;
        fsm.tick(&mut ctx);
        assert_eq!(fsm.current_state(), StateId::Purging);
    }

    #[test]
    fn purging_to_idle_after_duration() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);
        fsm.force_transition(StateId::Purging, &mut ctx);

        ctx.sensors.nh3_avg_ppm = 0.0;
        let ticks_needed =
            (ctx.config.purge_duration_secs as f32 / ctx.tick_period_secs) as u64 + 1;
        for _ in 0..ticks_needed {
            fsm.tick(&mut ctx);
        }
        assert_eq!(fsm.current_state(), StateId::Idle);
    }

    #[test]
    fn purging_returns_to_active_on_re_elevation() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);
        fsm.force_transition(StateId::Purging, &mut ctx);

        ctx.sensors.nh3_avg_ppm = ctx.config.nh3_activate_threshold_ppm + 5.0;
        fsm.tick(&mut ctx);
        assert_eq!(fsm.current_state(), StateId::Active);
    }

    #[test]
    fn fault_sends_any_state_to_error() {
        for start_state in [
            StateId::Idle,
            StateId::Sensing,
            StateId::Active,
            StateId::Purging,
        ] {
            let mut fsm = make_fsm();
            let mut ctx = make_ctx();
            fsm.start(&mut ctx);
            if start_state != StateId::Idle {
                fsm.force_transition(start_state, &mut ctx);
            }

            ctx.fault_flags = crate::error::SafetyFault::OverTemperature.mask();
            fsm.tick(&mut ctx);
            assert_eq!(
                fsm.current_state(),
                StateId::Error,
                "Expected Error from {:?}",
                start_state
            );
        }
    }

    #[test]
    fn error_kills_all_actuators() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);
        ctx.commands.pump_duty = 100;
        ctx.commands.uvc_duty = 100;
        fsm.force_transition(StateId::Error, &mut ctx);
        assert_eq!(ctx.commands.pump_duty, 0);
        assert_eq!(ctx.commands.uvc_duty, 0);
    }

    #[test]
    fn error_to_idle_when_faults_clear() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);
        fsm.force_transition(StateId::Error, &mut ctx);
        assert_eq!(fsm.current_state(), StateId::Error);

        ctx.fault_flags = 0;
        fsm.tick(&mut ctx);
        assert_eq!(fsm.current_state(), StateId::Idle);
    }

    #[test]
    fn error_stays_while_faults_active() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);
        fsm.force_transition(StateId::Error, &mut ctx);

        ctx.fault_flags = crate::error::SafetyFault::WaterLevelLow.mask();
        for _ in 0..10 {
            fsm.tick(&mut ctx);
        }
        assert_eq!(fsm.current_state(), StateId::Error);
    }

    #[test]
    fn force_transition_calls_enter_and_exit() {
        let mut fsm = make_fsm();
        let mut ctx = make_ctx();
        fsm.start(&mut ctx);
        fsm.force_transition(StateId::Active, &mut ctx);
        assert!(ctx.commands.pump_duty > 0);
        assert_eq!(ctx.commands.led_rgb, (0, 50, 255));
    }

    #[test]
    fn state_id_from_index_roundtrip() {
        for i in 0..StateId::COUNT {
            let id = StateId::from_index(i);
            assert_eq!(id as usize, i);
        }
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn state_id_from_invalid_index_returns_error() {
        let id = StateId::from_index(99);
        assert_eq!(id, StateId::Error);
    }
}

#[cfg(test)]
mod proptests {
    use super::context::FsmContext;
    use super::*;
    use crate::config::SystemConfig;
    use proptest::prelude::*;

    fn arb_event() -> impl Strategy<Value = (f32, f32, u8)> {
        (
            0.0f32..50.0, // nh3_ppm
            0.0f32..50.0, // nh3_avg_ppm
            0u8..16,      // fault_flags
        )
    }

    proptest! {
        #[test]
        fn no_invalid_state_reachable(events in proptest::collection::vec(arb_event(), 1..100)) {
            let mut fsm = Fsm::new(states::build_state_table(), StateId::Idle);
            let mut ctx = FsmContext::new(SystemConfig::default());
            fsm.start(&mut ctx);

            let valid_states = [StateId::Idle, StateId::Sensing, StateId::Active, StateId::Purging, StateId::Error];

            for (nh3, avg, faults) in events {
                ctx.sensors.nh3_ppm = nh3;
                ctx.sensors.nh3_avg_ppm = avg;
                ctx.fault_flags = faults;
                fsm.tick(&mut ctx);

                let current = fsm.current_state();
                prop_assert!(valid_states.contains(&current),
                    "FSM reached invalid state: {:?}", current);
            }
        }

        #[test]
        fn faults_always_reach_error(fault_flags in 1u8..=255) {
            let mut fsm = Fsm::new(states::build_state_table(), StateId::Idle);
            let mut ctx = FsmContext::new(SystemConfig::default());
            fsm.start(&mut ctx);

            ctx.fault_flags = fault_flags;

            // Tick enough times for any state to notice the fault
            for _ in 0..5 {
                fsm.tick(&mut ctx);
            }
            prop_assert_eq!(fsm.current_state(), StateId::Error);
        }
    }
}
