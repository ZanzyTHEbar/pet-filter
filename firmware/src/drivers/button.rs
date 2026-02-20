//! ISR-debounced button driver with short, long, and double press detection.
//!
//! ## Hardware
//!
//! Active-low momentary switch with external pull-up. GPIO fires on
//! falling edge; ISR records the raw timestamp into an atomic, and the
//! `tick()` method (called from the main loop at control-tick rate)
//! runs the debounce + gesture state machine.
//!
//! ## Gesture detection
//!
//! | Gesture      | Condition                              | Event                  |
//! |-------------|----------------------------------------|------------------------|
//! | Short press | Release < 2s, no second press < 300ms  | `ButtonShortPress`     |
//! | Long press  | Hold >= 5s                             | `ButtonLongPress`      |
//! | Double press| Two presses within 300ms gap           | `ButtonDoublePress`    |

use core::sync::atomic::{AtomicU32, Ordering};

const DEBOUNCE_MS: u32 = 50;
const LONG_PRESS_MS: u32 = 5000;
const DOUBLE_PRESS_WINDOW_MS: u32 = 300;

/// Raw ISR timestamp (milliseconds since boot, truncated to u32).
/// Written by the ISR, read by the main loop.
static BUTTON_ISR_TIMESTAMP: AtomicU32 = AtomicU32::new(0);

/// Button events emitted after gesture classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEvent {
    ShortPress,
    LongPress,
    DoublePress,
}

/// Internal state machine for gesture detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GestureState {
    Idle,
    DebounceWait { since_ms: u32 },
    Pressed { since_ms: u32 },
    WaitSecondPress { first_release_ms: u32 },
}

pub struct ButtonDriver {
    gpio: i32,
    state: GestureState,
    last_isr_ms: u32,
}

impl ButtonDriver {
    pub fn new(gpio: i32) -> Self {
        Self {
            gpio,
            state: GestureState::Idle,
            last_isr_ms: 0,
        }
    }

    /// GPIO pin this button is attached to.
    pub fn gpio(&self) -> i32 {
        self.gpio
    }

    /// Call from the main loop at each control tick.
    /// `now_ms` is the current monotonic time in milliseconds.
    /// Returns a classified gesture event, if any.
    pub fn tick(&mut self, now_ms: u32) -> Option<ButtonEvent> {
        let isr_ms = BUTTON_ISR_TIMESTAMP.load(Ordering::Acquire);
        let new_press = isr_ms != self.last_isr_ms && isr_ms != 0;

        match self.state {
            GestureState::Idle => {
                if new_press {
                    self.last_isr_ms = isr_ms;
                    self.state = GestureState::DebounceWait { since_ms: now_ms };
                }
                None
            }

            GestureState::DebounceWait { since_ms } => {
                if now_ms.wrapping_sub(since_ms) >= DEBOUNCE_MS {
                    self.state = GestureState::Pressed { since_ms: now_ms };
                }
                None
            }

            GestureState::Pressed { since_ms } => {
                let held_ms = now_ms.wrapping_sub(since_ms);

                if held_ms >= LONG_PRESS_MS {
                    self.state = GestureState::Idle;
                    return Some(ButtonEvent::LongPress);
                }

                if new_press && isr_ms != self.last_isr_ms {
                    self.last_isr_ms = isr_ms;
                    self.state = GestureState::WaitSecondPress {
                        first_release_ms: now_ms,
                    };
                    return None;
                }

                if !Self::is_pressed_hw() && held_ms >= DEBOUNCE_MS {
                    self.state = GestureState::WaitSecondPress {
                        first_release_ms: now_ms,
                    };
                }

                None
            }

            GestureState::WaitSecondPress { first_release_ms } => {
                let gap = now_ms.wrapping_sub(first_release_ms);

                if new_press && isr_ms != self.last_isr_ms {
                    self.last_isr_ms = isr_ms;
                    if gap <= DOUBLE_PRESS_WINDOW_MS {
                        self.state = GestureState::Idle;
                        return Some(ButtonEvent::DoublePress);
                    }
                }

                if gap > DOUBLE_PRESS_WINDOW_MS {
                    self.state = GestureState::Idle;
                    return Some(ButtonEvent::ShortPress);
                }

                None
            }
        }
    }

    #[cfg(target_os = "espidf")]
    fn is_pressed_hw() -> bool {
        !crate::drivers::hw_init::gpio_read(crate::pins::BUTTON_GPIO)
    }

    #[cfg(not(target_os = "espidf"))]
    fn is_pressed_hw() -> bool {
        false
    }
}

/// ISR handler — register this on the button GPIO falling edge.
/// Safe to call from interrupt context (lock-free atomic store).
#[allow(unused)]
pub fn button_isr_handler(now_ms: u32) {
    BUTTON_ISR_TIMESTAMP.store(now_ms, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_isr() {
        BUTTON_ISR_TIMESTAMP.store(0, Ordering::SeqCst);
    }

    #[test]
    fn no_events_without_press() {
        reset_isr();
        let mut btn = ButtonDriver::new(16);
        assert_eq!(btn.tick(100), None);
        assert_eq!(btn.tick(200), None);
    }

    #[test]
    fn debounce_filters_rapid_noise() {
        reset_isr();
        let mut btn = ButtonDriver::new(16);
        button_isr_handler(100);
        assert_eq!(btn.tick(100), None); // debounce wait
        assert_eq!(btn.tick(130), None); // still within 50ms debounce
    }

    #[test]
    fn long_press_detected() {
        reset_isr();
        let mut btn = ButtonDriver::new(16);
        button_isr_handler(1000);
        btn.tick(1000); // ISR detected
        btn.tick(1060); // debounce clears -> Pressed
        // Simulate holding for 5 seconds
        assert_eq!(btn.tick(6100), Some(ButtonEvent::LongPress));
    }
}
