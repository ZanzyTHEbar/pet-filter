//! Core-pinned thread spawning for ESP32-S3 dual-core.
//!
//! Wraps `esp_pthread_set_cfg()` so that `std::thread::spawn` creates a
//! FreeRTOS task pinned to a specific CPU core with explicit priority
//! and stack size. On non-ESP targets, falls back to plain thread spawn.
//!
//! # ESP-IDF Threading Model
//!
//! ESP-IDF implements `std::thread` via pthreads, which are thin wrappers
//! around FreeRTOS tasks. `esp_pthread_set_cfg()` sets thread-local
//! configuration that applies to the *next* `pthread_create()` call from
//! the calling thread. This means the config→spawn pair must not be
//! interleaved with other thread creation on the same thread.

/// CPU core identifiers for the ESP32-S3 Xtensa LX7 dual-core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Core {
    /// Core 0 (PRO_CPU) — protocol stacks (WiFi, BLE, lwIP).
    Pro = 0,
    /// Core 1 (APP_CPU) — application / safety-critical logic.
    App = 1,
}

/// Spawn a thread pinned to a specific core with explicit priority and stack.
///
/// On ESP-IDF, uses `esp_pthread_set_cfg()` to configure core affinity,
/// priority, and stack size before `std::thread::spawn`. The `name` parameter
/// must be a null-terminated string (e.g. `"rpc-io\0"`).
///
/// On non-ESP targets, ignores `core` and `priority`, using only `stack_kb`.
#[cfg(target_os = "espidf")]
pub fn spawn_on_core(
    core: Core,
    priority: u8,
    stack_kb: usize,
    name: &'static str,
    f: impl FnOnce() + Send + 'static,
) -> std::thread::JoinHandle<()> {
    unsafe {
        let mut cfg = esp_idf_sys::esp_create_default_pthread_config();
        cfg.pin_to_core = core as i32;
        cfg.prio = priority as i32;
        cfg.stack_size = (stack_kb * 1024) as i32;
        cfg.thread_name = name.as_ptr() as *const _;
        let ret = esp_idf_sys::esp_pthread_set_cfg(&cfg);
        assert!(
            ret == esp_idf_sys::ESP_OK as i32,
            "esp_pthread_set_cfg failed: {ret}"
        );
    }

    let display_name = name.trim_end_matches('\0');
    log::info!(
        "Spawning '{}' on {:?} (pri={}, stack={}KB)",
        display_name,
        core,
        priority,
        stack_kb
    );

    std::thread::Builder::new()
        .name(display_name.into())
        .spawn(f)
        .expect("spawn_on_core: thread creation failed")
}

/// Simulation fallback — ignores core affinity and priority.
#[cfg(not(target_os = "espidf"))]
pub fn spawn_on_core(
    _core: Core,
    _priority: u8,
    stack_kb: usize,
    name: &'static str,
    f: impl FnOnce() + Send + 'static,
) -> std::thread::JoinHandle<()> {
    let display_name = name.trim_end_matches('\0');
    log::info!(
        "Spawning '{}' (sim, no core pinning, stack={}KB)",
        display_name,
        stack_kb
    );

    std::thread::Builder::new()
        .name(display_name.into())
        .stack_size(stack_kb * 1024)
        .spawn(f)
        .expect("spawn_on_core(sim): thread creation failed")
}
