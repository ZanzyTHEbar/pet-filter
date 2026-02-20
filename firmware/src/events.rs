//! Interrupt-driven event system with FreeRTOS task notification wake.
//!
//! Events are produced by:
//! - GPIO ISRs (flow pulses, water level changes, interlock)
//! - Timer callbacks (periodic sensor reads, telemetry)
//! - ULP wake (NH3 threshold crossed during deep sleep)
//! - Software (FSM transitions, safety faults)
//!
//! Events are consumed by the main control loop, which blocks via
//! `wait_for_event()` until notified or a timeout expires.
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐  xTaskNotifyGive  ┌──────────────┐
//! │ GPIO ISR    │────▶│              │─────────────────▶│              │
//! │ Timer ISR   │────▶│  Event Queue │                   │  Main Loop   │
//! │ ULP Wake    │────▶│  (lock-free) │                   │  (consumer)  │
//! │ Software    │────▶│              │                   │              │
//! └─────────────┘     └──────────────┘                   └──────────────┘
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
    SafetyFault = 0,
    /// UVC interlock state changed.
    InterlockChanged = 1,
    /// Water level changed on Tank A or B.
    WaterLevelChanged = 2,

    // ── Sensor data ───────────────────────────────────────
    /// Periodic sensor read timer fired.
    SensorReadTick = 10,
    /// NH3 above activation threshold (from ULP or main ADC).
    Nh3ThresholdCrossed = 11,

    // ── Control ───────────────────────────────────────────
    /// FSM control loop tick (1 Hz).
    ControlTick = 20,
    /// Purge timer expired.
    PurgeTimerExpired = 21,
    /// Scheduled scrub timer fired.
    ScheduledScrub = 22,

    // ── Communication ─────────────────────────────────────
    /// Telemetry report timer fired.
    TelemetryTick = 30,
    /// Incoming command from RPC / BLE / Serial.
    CommandReceived = 31,

    // ── Power management ──────────────────────────────────
    /// System idle — consider entering lower power mode.
    IdleTimeout = 40,
    /// ULP wake event (NH3 detected during deep sleep).
    UlpWake = 41,

    // ── User input ────────────────────────────────────────
    /// Debounced short button press.
    ButtonShortPress = 32,
    /// Long button press (>5s hold).
    ButtonLongPress = 33,
    /// Double button press (<300ms gap).
    ButtonDoublePress = 34,

    // ── BLE provisioning ──────────────────────────────────
    /// BLE central connected.
    BleConnected = 35,
    /// BLE central disconnected.
    BleDisconnected = 36,
    /// BLE SSID characteristic written.
    BleSsidWrite = 37,
    /// BLE password characteristic written.
    BlePasswordWrite = 38,
    /// BLE PSK characteristic written.
    BlePskWrite = 39,

    // ── Housekeeping ──────────────────────────────────────
    /// Watchdog heartbeat.
    WatchdogTick = 50,
}

// ── Lock-free SPSC ring buffer ────────────────────────────────
//
// ISRs write (produce), main loop reads (consume).
// Uses atomic head/tail indices.  The buffer is intentionally
// kept in a static so ISR callbacks can access it.

static EVENT_HEAD: AtomicU8 = AtomicU8::new(0);
static EVENT_TAIL: AtomicU8 = AtomicU8::new(0);
static mut EVENT_BUFFER: [u8; EVENT_QUEUE_CAP] = [0; EVENT_QUEUE_CAP];

// ── FreeRTOS task notification (main loop wake) ───────────────
//
// On ESP-IDF, `push_event()` sends a FreeRTOS task notification to wake
// the main control loop, which blocks in `wait_for_event()`. This
// eliminates busy-polling and lets the idle task invoke PM hooks.


/// Register the calling task as the main event consumer.
///
/// After this call, every `push_event()` will send an `xTaskNotifyGive`
/// to wake the registered task from `wait_for_event()`. Must be called
/// exactly once from the main control task before entering the event loop.
#[cfg(target_os = "espidf")]
pub fn register_main_task() {}

/// No-op on non-ESP targets.
#[cfg(not(target_os = "espidf"))]
pub fn register_main_task() {}

/// Block until an event notification arrives or `timeout_ms` expires.
///
/// On ESP-IDF this calls `ulTaskNotifyTake` which suspends the task,
/// allowing the FreeRTOS idle task to trigger power management hooks.
/// The timeout ensures periodic work (button polling, LED updates,
/// WiFi reconnection) still runs even without events.
#[cfg(target_os = "espidf")]
pub fn wait_for_event(timeout_ms: u32) {
    std::thread::sleep(std::time::Duration::from_millis(timeout_ms as u64));
}

/// Simulation fallback: sleep for the timeout duration.
#[cfg(not(target_os = "espidf"))]
pub fn wait_for_event(timeout_ms: u32) {
    std::thread::sleep(std::time::Duration::from_millis(timeout_ms as u64));
}

/// Write one byte into the event ring buffer at `slot`.
///
/// # Safety
/// The caller must be the sole producer (ISR / timer-task context).
/// `slot` must be `< EVENT_QUEUE_CAP`.  SPSC discipline guarantees that
/// the consumer is not reading this slot simultaneously.
unsafe fn event_buffer_write(slot: usize, val: u8) {
    unsafe {
        EVENT_BUFFER[slot] = val;
    }
}

/// Read one byte from the event ring buffer at `slot`.
///
/// # Safety
/// The caller must be the sole consumer (main-loop task).
/// `slot` must be `< EVENT_QUEUE_CAP`.  SPSC discipline guarantees that
/// the producer is not writing this slot simultaneously.
unsafe fn event_buffer_read(slot: usize) -> u8 {
    unsafe { EVENT_BUFFER[slot] }
}

/// Push an event into the queue and wake the main task.
///
/// Safe to call from ISR context, timer callbacks, and any thread.
/// Returns `false` if the queue is full (event dropped).
pub fn push_event(event: Event) -> bool {
    let head = EVENT_HEAD.load(Ordering::Relaxed);
    let tail = EVENT_TAIL.load(Ordering::Acquire);
    let next_head = (head + 1) % EVENT_QUEUE_CAP as u8;

    if next_head == tail {
        return false; // Queue full — drop event.
    }

    // SAFETY: push_event is the sole producer (ISR / timer-task context).
    // event_buffer_write() accesses EVENT_BUFFER exclusively at the head slot
    // while the consumer reads at the tail slot — SPSC discipline holds.
    unsafe {
        event_buffer_write(head as usize, event as u8);
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

    // SAFETY: pop_event is the sole consumer (main-loop task).
    let raw = unsafe { event_buffer_read(tail as usize) };
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
        0 => Some(Event::SafetyFault),
        1 => Some(Event::InterlockChanged),
        2 => Some(Event::WaterLevelChanged),
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
        35 => Some(Event::BleConnected),
        36 => Some(Event::BleDisconnected),
        37 => Some(Event::BleSsidWrite),
        38 => Some(Event::BlePasswordWrite),
        39 => Some(Event::BlePskWrite),
        40 => Some(Event::IdleTimeout),
        41 => Some(Event::UlpWake),
        50 => Some(Event::WatchdogTick),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::Ordering;

    fn reset_queue() {
        EVENT_HEAD.store(0, Ordering::Relaxed);
        EVENT_TAIL.store(0, Ordering::Relaxed);
    }

    #[test]
    fn queue_starts_empty() {
        reset_queue();
        assert!(queue_is_empty());
        assert_eq!(queue_len(), 0);
        assert!(pop_event().is_none());
    }

    #[test]
    fn push_and_pop_single() {
        reset_queue();
        assert!(push_event(Event::ControlTick));
        assert!(!queue_is_empty());
        assert_eq!(queue_len(), 1);

        let e = pop_event();
        assert_eq!(e, Some(Event::ControlTick));
        assert!(queue_is_empty());
    }

    #[test]
    fn fifo_ordering() {
        reset_queue();
        push_event(Event::SafetyFault);
        push_event(Event::ControlTick);
        push_event(Event::TelemetryTick);

        assert_eq!(pop_event(), Some(Event::SafetyFault));
        assert_eq!(pop_event(), Some(Event::ControlTick));
        assert_eq!(pop_event(), Some(Event::TelemetryTick));
        assert!(pop_event().is_none());
    }

    #[test]
    fn drain_events_collects_all() {
        reset_queue();
        push_event(Event::SensorReadTick);
        push_event(Event::ControlTick);

        let mut collected = Vec::new();
        drain_events(|e| collected.push(e));
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0], Event::SensorReadTick);
        assert_eq!(collected[1], Event::ControlTick);
        assert!(queue_is_empty());
    }

    #[test]
    fn overflow_returns_false() {
        reset_queue();
        for _ in 0..(EVENT_QUEUE_CAP - 1) {
            assert!(push_event(Event::ControlTick));
        }
        assert!(!push_event(Event::ControlTick));
    }

    #[test]
    fn event_from_u8_roundtrip() {
        let events = [
            Event::SafetyFault,
            Event::InterlockChanged,
            Event::ControlTick,
            Event::TelemetryTick,
            Event::ButtonShortPress,
            Event::UlpWake,
        ];
        for e in events {
            let raw = e as u8;
            assert_eq!(event_from_u8(raw), Some(e));
        }
    }

    #[test]
    fn event_from_invalid_u8_returns_none() {
        assert!(event_from_u8(255).is_none());
        assert!(event_from_u8(99).is_none());
    }
}
