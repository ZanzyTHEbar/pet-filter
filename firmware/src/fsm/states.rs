//! Concrete state handler functions and table builder.
//!
//! Each state is defined by three plain `fn` pointers — no closures, no
//! dynamic dispatch, no heap.  This is the classic embedded C FSM pattern
//! expressed in safe Rust.
//!
//! ```text
//!  IDLE ──[NH3 > activate]──▶ SENSING
//!    ▲                            │
//!    │                   [confirmed]  [cleared]
//!    │                            ▼       │
//!    │                         ACTIVE ◀───┘
//!    │                            │
//!    │                   [NH3 < deactivate]
//!    │                            ▼
//!    └──────[purge done]────── PURGING
//!
//!  Any state ──[safety fault]──▶ ERROR ──[faults cleared]──▶ IDLE
//! ```

use super::context::FsmContext;
use super::{StateDescriptor, StateId};
use log::{info, warn};

// ═══════════════════════════════════════════════════════════════════════════
//  Table builder
// ═══════════════════════════════════════════════════════════════════════════

/// Build the static state table.  Called once at startup.
pub fn build_state_table() -> [StateDescriptor; StateId::COUNT] {
    [
        // Index 0 — Idle
        StateDescriptor {
            id: StateId::Idle,
            name: "Idle",
            on_enter: Some(idle_enter),
            on_exit: None,
            on_update: idle_update,
        },
        // Index 1 — Sensing
        StateDescriptor {
            id: StateId::Sensing,
            name: "Sensing",
            on_enter: Some(sensing_enter),
            on_exit: None,
            on_update: sensing_update,
        },
        // Index 2 — Active
        StateDescriptor {
            id: StateId::Active,
            name: "Active",
            on_enter: Some(active_enter),
            on_exit: Some(active_exit),
            on_update: active_update,
        },
        // Index 3 — Purging
        StateDescriptor {
            id: StateId::Purging,
            name: "Purging",
            on_enter: Some(purging_enter),
            on_exit: Some(purging_exit),
            on_update: purging_update,
        },
        // Index 4 — Error
        StateDescriptor {
            id: StateId::Error,
            name: "Error",
            on_enter: Some(error_enter),
            on_exit: Some(error_exit),
            on_update: error_update,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
//  IDLE state
// ═══════════════════════════════════════════════════════════════════════════

fn idle_enter(ctx: &mut FsmContext) {
    // All actuators off, status LED teal (brand colour — healthy idle).
    ctx.commands.pump_duty = 0;
    ctx.commands.uvc_duty = 0;
    ctx.commands.led_rgb = (0, 180, 148); // teal #00B894
    info!("IDLE: system quiescent, monitoring NH3");
}

fn idle_update(ctx: &mut FsmContext) -> Option<StateId> {
    // Guard: any safety fault → Error
    if ctx.has_faults() {
        return Some(StateId::Error);
    }

    // Trigger: instantaneous NH3 above activation threshold
    if ctx.sensors.nh3_ppm > ctx.config.nh3_activate_threshold_ppm {
        return Some(StateId::Sensing);
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════
//  SENSING state — confirming the NH3 reading before full activation
// ═══════════════════════════════════════════════════════════════════════════

fn sensing_enter(ctx: &mut FsmContext) {
    ctx.commands.led_rgb = (0, 100, 255); // blue — "thinking"
    info!(
        "SENSING: NH3 at {:.1} ppm, confirming for {}s",
        ctx.sensors.nh3_ppm, ctx.config.nh3_confirm_duration_secs
    );
}

fn sensing_update(ctx: &mut FsmContext) -> Option<StateId> {
    if ctx.has_faults() {
        return Some(StateId::Error);
    }

    // If NH3 drops back below activation threshold → false alarm, return to Idle
    if ctx.sensors.nh3_avg_ppm < ctx.config.nh3_activate_threshold_ppm {
        info!(
            "SENSING: NH3 avg dropped to {:.1} ppm, returning to Idle",
            ctx.sensors.nh3_avg_ppm
        );
        return Some(StateId::Idle);
    }

    // If we've been in Sensing long enough and the average is still high → confirm
    let confirm_secs = ctx.config.nh3_confirm_duration_secs as f32;
    if ctx.secs_in_state() >= confirm_secs
        && ctx.sensors.nh3_avg_ppm >= ctx.config.nh3_activate_threshold_ppm
    {
        info!(
            "SENSING: confirmed NH3 avg {:.1} ppm after {:.0}s → activating",
            ctx.sensors.nh3_avg_ppm,
            ctx.secs_in_state()
        );
        return Some(StateId::Active);
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════
//  ACTIVE state — pump running, UVC on, actively scrubbing
// ═══════════════════════════════════════════════════════════════════════════

fn active_enter(ctx: &mut FsmContext) {
    ctx.commands.pump_duty = ctx.config.pump_duty_percent;
    ctx.commands.pump_forward = true;
    ctx.commands.uvc_duty = ctx.config.uvc_duty_percent;
    ctx.commands.led_rgb = (0, 50, 255); // deep blue — "working"
    info!(
        "ACTIVE: pump at {}%, UVC at {}%",
        ctx.config.pump_duty_percent, ctx.config.uvc_duty_percent
    );
}

fn active_exit(ctx: &mut FsmContext) {
    // UVC off immediately on exit (pump continues in purge)
    ctx.commands.uvc_duty = 0;
    info!("ACTIVE: UVC disabled on state exit");
}

fn active_update(ctx: &mut FsmContext) -> Option<StateId> {
    if ctx.has_faults() {
        return Some(StateId::Error);
    }

    // NH3 dropped below deactivation threshold → begin purge
    if ctx.sensors.nh3_avg_ppm < ctx.config.nh3_deactivate_threshold_ppm {
        info!(
            "ACTIVE: NH3 avg {:.1} ppm < {:.1} threshold → purging",
            ctx.sensors.nh3_avg_ppm, ctx.config.nh3_deactivate_threshold_ppm
        );
        return Some(StateId::Purging);
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════
//  PURGING state — flushing the system before returning to idle
// ═══════════════════════════════════════════════════════════════════════════

fn purging_enter(ctx: &mut FsmContext) {
    // Keep pump running at reduced duty to flush residual ammonia from water.
    // UVC already disabled by active_exit.
    ctx.commands.pump_duty = (ctx.config.pump_duty_percent / 2).max(20);
    ctx.commands.pump_forward = true;
    ctx.commands.uvc_duty = 0;
    ctx.commands.led_rgb = (0, 200, 200); // cyan — "winding down"
    info!(
        "PURGING: pump at {}% for {}s",
        ctx.commands.pump_duty, ctx.config.purge_duration_secs
    );
}

fn purging_exit(ctx: &mut FsmContext) {
    ctx.commands.pump_duty = 0;
    info!("PURGING: pump stopped, system returning to idle");
}

fn purging_update(ctx: &mut FsmContext) -> Option<StateId> {
    if ctx.has_faults() {
        return Some(StateId::Error);
    }

    // If NH3 spikes back up during purge → skip back to Active
    if ctx.sensors.nh3_avg_ppm >= ctx.config.nh3_activate_threshold_ppm {
        info!(
            "PURGING: NH3 re-elevated to {:.1} ppm, returning to Active",
            ctx.sensors.nh3_avg_ppm
        );
        return Some(StateId::Active);
    }

    // Purge complete after configured duration
    let purge_secs = ctx.config.purge_duration_secs as f32;
    if ctx.secs_in_state() >= purge_secs {
        return Some(StateId::Idle);
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════
//  ERROR state — safety fault, all actuators disabled
// ═══════════════════════════════════════════════════════════════════════════

fn error_enter(ctx: &mut FsmContext) {
    // Kill everything immediately.
    ctx.commands = super::context::ActuatorCommands::all_off();
    ctx.commands.led_rgb = (255, 0, 0); // red — fault
    warn!(
        "ERROR: all actuators disabled, fault_flags=0b{:08b}",
        ctx.fault_flags
    );
}

fn error_exit(ctx: &mut FsmContext) {
    info!("ERROR: faults cleared, resuming normal operation");
    ctx.commands.led_rgb = (0, 180, 148); // back to teal
}

fn error_update(ctx: &mut FsmContext) -> Option<StateId> {
    // Blink the red LED (on for 500ms, off for 500ms at 1Hz tick rate)
    if ctx.ticks_in_state % 2 == 0 {
        ctx.commands.led_rgb = (255, 0, 0);
    } else {
        ctx.commands.led_rgb = (40, 0, 0);
    }

    // Stay in Error until ALL faults are cleared
    if !ctx.has_faults() {
        return Some(StateId::Idle);
    }

    None
}
