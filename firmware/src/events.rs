//! Interrupt-driven event system.
//!
//! Events are produced by:
//! - GPIO ISRs (flow pulses, water level changes, interlock)
//! - Timer callbacks (periodic sensor reads, telemetry)
//! - ULP wake (NH3 threshold crossed during deep sleep)
//! - Software (FSM transitions, safety faults)
//!
//! Events are consumed by the main control loop, which processes them
//! one at a time in priority order.
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌──────────────┐
//! │ GPIO ISR    │────▶│              │     │              │
//! │ Timer ISR   │────▶│  Event Queue │────▶│  Main Loop   │
//! │ ULP Wake    │────▶│  (lock-free) │     │  (consumer)  │
//! │ Software    │────▶│              │     │              │
//! └─────────────┘     └──────────────┘     └──────────────┘
//! ```

use core::sync::atomic::{AtomicU8, Ordering};

/// Maximum number of pending events.
/// Power of 2 for efficient ring buffer modulo.
const EVENT_QUEUE_CAP: usize = 32;

/// System event types, ordered by rough priority.
/// Lower discriminant = higher priority when multiple events
/// are pending simultaneously.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Event {
    // ── Safety-critical (highest priority) ────────────────
    /// Safety fault detected or cleared.
    SafetyFault            = 0,
    /// UVC interlock state changed.
    InterlockChanged       = 1,
    /// Water level changed on Tank A or B.
    WaterLevelChanged      = 2,

    // ── Sensor data ───────────────────────────────────────
    /// Periodic sensor read timer fired.
    SensorReadTick         = 10,
    /// NH3 above activation threshold (from ULP or main ADC).
    Nh3ThresholdCrossed    = 11,

    // ── Control ───────────────────────────────────────────
    /// FSM control loop tick (1 Hz).
    ControlTick            = 20,
    /// Purge timer expired.
    PurgeTimerExpired      = 21,
    /// Scheduled scrub timer fired.
    ScheduledScrub         = 22,

    // ── Communication ─────────────────────────────────────
    /// Telemetry report timer fired.
    TelemetryTick          = 30,
    /// Incoming command from RPC / BLE / Serial.
    CommandReceived        = 31,

    // ── Power management ──────────────────────────────────
    /// System idle — consider entering lower power mode.
    IdleTimeout            = 40,
    /// ULP wake event (NH3 detected during deep sleep).
    UlpWake                = 41,

    // ── User input ────────────────────────────────────────
    /// Debounced short button press.
    ButtonShortPress       = 32,
    /// Long button press (>5s hold).
    ButtonLongPress        = 33,
    /// Double button press (<300ms gap).
    ButtonDoublePress      = 34,

    // ── Housekeeping ──────────────────────────────────────
    /// Watchdog heartbeat.
    WatchdogTick           = 50,
}

// ── Lock-free SPSC ring buffer ────────────────────────────────
//
// ISRs write (produce), main loop reads (consume).
// Uses atomic head/tail indices.  The buffer is intentionally
// kept in a static so ISR callbacks can access it.

static EVENT_HEAD: AtomicU8 = AtomicU8::new(0);
static EVENT_TAIL: AtomicU8 = AtomicU8::new(0);
// SAFETY: EVENT_BUFFER is accessed exclusively through the SPSC queue
// (heapless::spsc::Queue split into producer/consumer halves).
// Producer (push_event): ISR / timer-task context — one writer.
// Consumer (drain_events): main-loop task — one reader.
// No concurrent mutable access is possible; the atomics inside Queue
// enforce the SPSC discipline.
static mut EVENT_BUFFER: [u8; EVENT_QUEUE_CAP] = [0; EVENT_QUEUE_CAP];

/// Push an event into the queue.
/// Safe to call from ISR context (lock-free).
/// Returns `false` if the queue is full (event dropped).
pub fn push_event(event: Event) -> bool {
    let head = EVENT_HEAD.load(Ordering::Relaxed);
    let tail = EVENT_TAIL.load(Ordering::Acquire);
    let next_head = (head + 1) % EVENT_QUEUE_CAP as u8;

    if next_head == tail {
        return false; // Queue full — drop event.
    }

    // SAFETY: Only one producer (ISR context is single-threaded on each
    // core, and we use Relaxed ordering which is sufficient for SPSC).
    unsafe {
        EVENT_BUFFER[head as usize] = event as u8;
    }

    EVENT_HEAD.store(next_head, Ordering::Release);
    true
}

/// Pop the next event from the queue.
/// Called from the main loop (single consumer).
/// Returns `None` if the queue is empty.
pub fn pop_event() -> Option<Event> {
    let tail = EVENT_TAIL.load(Ordering::Relaxed);
    let head = EVENT_HEAD.load(Ordering::Acquire);

    if tail == head {
        return None; // Empty.
    }

    let raw = unsafe { EVENT_BUFFER[tail as usize] };
    EVENT_TAIL.store((tail + 1) % EVENT_QUEUE_CAP as u8, Ordering::Release);

    event_from_u8(raw)
}

/// Drain all pending events into a callback.
/// Processes events in FIFO order.
pub fn drain_events(mut handler: impl FnMut(Event)) {
    while let Some(event) = pop_event() {
        handler(event);
    }
}

/// Check if the event queue is empty.
pub fn queue_is_empty() -> bool {
    let tail = EVENT_TAIL.load(Ordering::Relaxed);
    let head = EVENT_HEAD.load(Ordering::Acquire);
    tail == head
}

/// Number of pending events.
pub fn queue_len() -> usize {
    let head = EVENT_HEAD.load(Ordering::Relaxed) as usize;
    let tail = EVENT_TAIL.load(Ordering::Relaxed) as usize;
    (head + EVENT_QUEUE_CAP - tail) % EVENT_QUEUE_CAP
}

// ── Internal ──────────────────────────────────────────────────

fn event_from_u8(raw: u8) -> Option<Event> {
    match raw {
        0  => Some(Event::SafetyFault),
        1  => Some(Event::InterlockChanged),
        2  => Some(Event::WaterLevelChanged),
        10 => Some(Event::SensorReadTick),
        11 => Some(Event::Nh3ThresholdCrossed),
        20 => Some(Event::ControlTick),
        21 => Some(Event::PurgeTimerExpired),
        22 => Some(Event::ScheduledScrub),
        30 => Some(Event::TelemetryTick),
        31 => Some(Event::CommandReceived),
        32 => Some(Event::ButtonShortPress),
        33 => Some(Event::ButtonLongPress),
        34 => Some(Event::ButtonDoublePress),
        40 => Some(Event::IdleTimeout),
        41 => Some(Event::UlpWake),
        50 => Some(Event::WatchdogTick),
        _  => None,
    }
}
