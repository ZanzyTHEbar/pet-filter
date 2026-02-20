//! ESP-IDF runtime symbol providers for third-party crates.

#[cfg(target_os = "espidf")]
use core::cell::{Cell, RefCell};
#[cfg(target_os = "espidf")]
use core::time::Duration;
#[cfg(target_os = "espidf")]
use std::sync::{Mutex, MutexGuard};

#[cfg(target_os = "espidf")]
static CRITICAL_SECTION_MUTEX: Mutex<()> = Mutex::new(());

#[cfg(target_os = "espidf")]
thread_local! {
    static CRITICAL_SECTION_DEPTH: Cell<u8> = const { Cell::new(0) };
    static CRITICAL_SECTION_GUARD: RefCell<Option<MutexGuard<'static, ()>>> = const { RefCell::new(None) };
}

/// Runtime-backed critical-section acquire used by `critical-section` 1.x.
#[cfg(target_os = "espidf")]
#[unsafe(no_mangle)]
pub extern "C" fn _critical_section_1_0_acquire() -> u8 {
    CRITICAL_SECTION_DEPTH.with(|depth| {
        CRITICAL_SECTION_GUARD.with(|guard| {
            let d = depth.get();
            if d == 0 {
                let lock = CRITICAL_SECTION_MUTEX
                    .lock()
                    .expect("critical-section mutex poisoned");
                *guard.borrow_mut() = Some(lock);
            }
            let new_depth = d.saturating_add(1);
            depth.set(new_depth);
            new_depth
        })
    })
}

/// Runtime-backed critical-section release used by `critical-section` 1.x.
#[cfg(target_os = "espidf")]
#[unsafe(no_mangle)]
pub extern "C" fn _critical_section_1_0_release(_token: u8) {
    CRITICAL_SECTION_DEPTH.with(|depth| {
        CRITICAL_SECTION_GUARD.with(|guard| {
            let d = depth.get();
            if d == 0 {
                return;
            }
            let new_depth = d - 1;
            depth.set(new_depth);
            if new_depth == 0 {
                *guard.borrow_mut() = None;
            }
        })
    })
}

#[cfg(target_os = "espidf")]
#[unsafe(no_mangle)]
pub extern "C" fn _embassy_time_now() -> u64 {
    unsafe { esp_idf_svc::sys::esp_timer_get_time() as u64 }
}

/// Runtime-backed wake scheduler for async timers.
#[cfg(target_os = "espidf")]
#[unsafe(no_mangle)]
pub extern "C" fn _embassy_time_schedule_wake(at: u64, waker: *mut core::ffi::c_void) {
    if waker.is_null() {
        return;
    }

    // SAFETY: embassy-time passes a valid pointer to a `Waker` for the duration
    // of schedule registration. We clone it immediately and move the clone.
    let waker = unsafe { (&*(waker as *const core::task::Waker)).clone() };
    std::thread::spawn(move || {
        let now = _embassy_time_now();
        if at > now {
            std::thread::sleep(Duration::from_micros(at - now));
        }
        waker.wake();
    });
}
