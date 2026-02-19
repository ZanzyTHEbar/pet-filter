//! Hardware timer module using ESP-IDF's esp_timer API.
//!
//! Creates periodic timers that push events into the lock-free SPSC queue.
//! On simulation targets, uses thread::sleep to approximate timing.
//!
//! Timer callbacks execute in the ESP timer task context (not ISR), so
//! they can safely call push_event() which uses AtomicU8.

use crate::events::{push_event, Event};

#[cfg(target_os = "espidf")]
use esp_idf_svc::sys::*;

#[cfg(target_os = "espidf")]
use log::info;

#[cfg(target_os = "espidf")]
static mut SENSOR_TIMER: esp_timer_handle_t = core::ptr::null_mut();
#[cfg(target_os = "espidf")]
static mut CONTROL_TIMER: esp_timer_handle_t = core::ptr::null_mut();

/// SAFETY: SENSOR_TIMER is written once in `start_timers()` before any
/// timer callbacks fire.  Only called from the single main task.
#[cfg(target_os = "espidf")]
unsafe fn sensor_timer() -> esp_timer_handle_t { unsafe { SENSOR_TIMER } }

/// SAFETY: Same invariants as `sensor_timer()`.
#[cfg(target_os = "espidf")]
unsafe fn control_timer() -> esp_timer_handle_t { unsafe { CONTROL_TIMER } }

#[cfg(target_os = "espidf")]
unsafe extern "C" fn sensor_tick_cb(_arg: *mut core::ffi::c_void) {
    push_event(Event::SensorReadTick);
}

#[cfg(target_os = "espidf")]
unsafe extern "C" fn control_tick_cb(_arg: *mut core::ffi::c_void) {
    push_event(Event::ControlTick);
}

/// Start the hardware tick timers.
///
/// - 10 Hz sensor read timer (100ms period)
/// - 1 Hz control tick timer (1000ms period)
#[cfg(target_os = "espidf")]
pub fn start_timers() {
    // SAFETY: SENSOR_TIMER and CONTROL_TIMER are written here once at boot
    // from the single main-task context before any timer callbacks fire.
    // The callbacks themselves only call push_event(), which is ISR-safe.
    unsafe {
        // Sensor timer: 10 Hz
        let sensor_args = esp_timer_create_args_t {
            callback: Some(sensor_tick_cb),
            arg: core::ptr::null_mut(),
            dispatch_method: esp_timer_dispatch_t_ESP_TIMER_TASK,
            name: b"sensor\0".as_ptr() as *const _,
            skip_unhandled_events: false,
        };
        let ret = esp_timer_create(&sensor_args, &raw mut SENSOR_TIMER);
        if ret != ESP_OK {
            log::error!("hw_timer: sensor timer create failed (rc={}) — continuing without sensor ticks", ret);
            return;
        }
        let ret = esp_timer_start_periodic(SENSOR_TIMER, 100_000); // 100ms
        if ret != ESP_OK {
            log::error!("hw_timer: sensor timer start failed (rc={})", ret);
            return;
        }

        // Control timer: 1 Hz
        let control_args = esp_timer_create_args_t {
            callback: Some(control_tick_cb),
            arg: core::ptr::null_mut(),
            dispatch_method: esp_timer_dispatch_t_ESP_TIMER_TASK,
            name: b"control\0".as_ptr() as *const _,
            skip_unhandled_events: false,
        };
        let ret = esp_timer_create(&control_args, &raw mut CONTROL_TIMER);
        if ret != ESP_OK {
            log::error!("hw_timer: control timer create failed (rc={}) — continuing without control ticks", ret);
            return;
        }
        let ret = esp_timer_start_periodic(CONTROL_TIMER, 1_000_000); // 1s
        if ret != ESP_OK {
            log::error!("hw_timer: control timer start failed (rc={})", ret);
            return;
        }

        info!("hw_timer: sensor@10Hz + control@1Hz started");
    }
}

#[cfg(not(target_os = "espidf"))]
pub fn start_timers() {
    log::info!("hw_timer(sim): timers not started (events driven by sleep loop)");
}

/// Stop all hardware tick timers.
#[cfg(target_os = "espidf")]
pub fn stop_timers() {
    // SAFETY: SENSOR_TIMER/CONTROL_TIMER are valid handles if start_timers()
    // succeeded; null-check prevents double-free.
    unsafe {
        // SAFETY: sensor_timer()/control_timer() contract — main task only.
        let st = sensor_timer();
        if !st.is_null() { esp_timer_stop(st); }
        let ct = control_timer();
        if !ct.is_null() { esp_timer_stop(ct); }
    }
}

#[cfg(not(target_os = "espidf"))]
pub fn stop_timers() {}
