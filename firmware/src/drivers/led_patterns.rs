//! LED pattern engine with priority-based pattern selection.
//!
//! Generates time-varying RGB values for the status LED. The main loop
//! calls `tick()` each control cycle, and the engine outputs the RGB
//! tuple to feed into `StatusLed::set_colour()`.
//!
//! ## Priority hierarchy (highest first)
//!
//! 1. **Error** — rapid red flash (8 Hz)
//! 2. **FSM state** — solid brand colours (idle=teal, sensing=blue, etc.)
//! 3. **Connectivity** — patterns overlay when no higher priority is active
//!
//! ## Pattern types
//!
//! | Pattern      | Description                      | Rate   |
//! |-------------|----------------------------------|--------|
//! | Solid        | Constant colour                  | —      |
//! | SlowPulse    | Sinusoidal brightness fade       | 1 Hz   |
//! | FastBlink    | On/off square wave               | 4 Hz   |
//! | Breathing    | Smooth ramp up/down              | 0.5 Hz |
//! | DoubleBlink  | Two quick flashes, then pause    | 1 Hz   |
//! | RapidFlash   | Very fast on/off                 | 8 Hz   |

/// Colour as (R, G, B) tuple, each 0–255.
pub type Rgb = (u8, u8, u8);

/// Pattern identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternId {
    Solid,
    SlowPulse,
    FastBlink,
    Breathing,
    DoubleBlink,
    RapidFlash,
    Off,
}

/// A pattern request with colour and type.
#[derive(Debug, Clone, Copy)]
pub struct PatternRequest {
    pub colour: Rgb,
    pub pattern: PatternId,
    pub priority: u8,
}

/// LED pattern engine. Stack-allocated, no heap.
pub struct LedPatternEngine {
    phase_ms: u32,
    active: Option<PatternRequest>,
    fsm_request: Option<PatternRequest>,
    connectivity_request: Option<PatternRequest>,
    error_request: Option<PatternRequest>,
}

impl LedPatternEngine {
    pub fn new() -> Self {
        Self {
            phase_ms: 0,
            active: None,
            fsm_request: None,
            connectivity_request: None,
            error_request: None,
        }
    }

    /// Set the FSM-layer pattern (priority 2).
    pub fn set_fsm_pattern(&mut self, colour: Rgb, pattern: PatternId) {
        self.fsm_request = Some(PatternRequest {
            colour,
            pattern,
            priority: 2,
        });
    }

    /// Set the connectivity-layer pattern (priority 3 — lowest).
    pub fn set_connectivity_pattern(&mut self, colour: Rgb, pattern: PatternId) {
        self.connectivity_request = Some(PatternRequest {
            colour,
            pattern,
            priority: 3,
        });
    }

    /// Set or clear the error pattern (priority 1 — highest).
    pub fn set_error_pattern(&mut self, active: bool) {
        if active {
            self.error_request = Some(PatternRequest {
                colour: (255, 0, 0),
                pattern: PatternId::RapidFlash,
                priority: 1,
            });
        } else {
            self.error_request = None;
        }
    }

    /// Clear all patterns — LED will be off.
    pub fn clear_all(&mut self) {
        self.fsm_request = None;
        self.connectivity_request = None;
        self.error_request = None;
        self.active = None;
        self.phase_ms = 0;
    }

    /// Advance the pattern phase and return the current RGB output.
    /// `delta_ms` is the time since the last call (typically 1000ms for 1Hz control loop).
    pub fn tick(&mut self, delta_ms: u32) -> Rgb {
        self.phase_ms = self.phase_ms.wrapping_add(delta_ms);

        let selected = self.select_active();
        let reset_phase = match (&self.active, &selected) {
            (Some(prev), Some(next)) => {
                prev.priority != next.priority || prev.pattern != next.pattern
            }
            (None, Some(_)) => true,
            _ => false,
        };
        if reset_phase {
            self.phase_ms = 0;
        }
        self.active = selected;

        match &self.active {
            Some(req) => self.generate(req.colour, req.pattern),
            None => (0, 0, 0),
        }
    }

    fn select_active(&self) -> Option<PatternRequest> {
        // Priority: error (1) > fsm (2) > connectivity (3)
        self.error_request
            .or(self.fsm_request)
            .or(self.connectivity_request)
    }

    fn generate(&self, colour: Rgb, pattern: PatternId) -> Rgb {
        let (r, g, b) = colour;
        match pattern {
            PatternId::Solid => colour,
            PatternId::Off => (0, 0, 0),
            PatternId::SlowPulse => {
                let brightness = Self::sine_brightness(self.phase_ms, 1000);
                Self::scale(r, g, b, brightness)
            }
            PatternId::FastBlink => {
                let on = (self.phase_ms % 250) < 125;
                if on { colour } else { (0, 0, 0) }
            }
            PatternId::Breathing => {
                let brightness = Self::sine_brightness(self.phase_ms, 2000);
                Self::scale(r, g, b, brightness)
            }
            PatternId::DoubleBlink => {
                let cycle = self.phase_ms % 1000;
                let on = cycle < 100 || (200..300).contains(&cycle);
                if on { colour } else { (0, 0, 0) }
            }
            PatternId::RapidFlash => {
                let on = (self.phase_ms % 125) < 63;
                if on { colour } else { (0, 0, 0) }
            }
        }
    }

    /// Attempt a sine-ish brightness curve without libm.
    /// Uses a triangular approximation: ramps 0→255→0 over `period_ms`.
    fn sine_brightness(phase_ms: u32, period_ms: u32) -> u8 {
        let pos = (phase_ms % period_ms) as u64;
        let half = period_ms as u64 / 2;
        if pos < half {
            ((pos * 255) / half) as u8
        } else {
            (((period_ms as u64 - pos) * 255) / half) as u8
        }
    }

    fn scale(r: u8, g: u8, b: u8, brightness: u8) -> Rgb {
        let br = brightness as u16;
        (
            ((r as u16 * br) / 255) as u8,
            ((g as u16 * br) / 255) as u8,
            ((b as u16 * br) / 255) as u8,
        )
    }
}

// ── Well-known colour constants (PetFilter brand) ─────────────

pub const COLOUR_IDLE: Rgb = (0, 180, 148); // Teal
pub const COLOUR_SENSING: Rgb = (0, 100, 255); // Blue
pub const COLOUR_ACTIVE: Rgb = (0, 50, 255); // Deep blue
pub const COLOUR_PURGING: Rgb = (0, 200, 200); // Cyan
pub const COLOUR_ERROR: Rgb = (255, 0, 0); // Red
pub const COLOUR_PROVISIONING: Rgb = (128, 0, 255); // Purple
pub const COLOUR_WIFI_CONNECTING: Rgb = (0, 100, 255); // Blue
pub const COLOUR_WIFI_CONNECTED: Rgb = (0, 255, 50); // Green
pub const COLOUR_LOW_WATER: Rgb = (255, 200, 0); // Yellow

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_pattern_returns_exact_colour() {
        let mut engine = LedPatternEngine::new();
        engine.set_fsm_pattern(COLOUR_IDLE, PatternId::Solid);
        assert_eq!(engine.tick(1000), COLOUR_IDLE);
    }

    #[test]
    fn off_pattern_returns_black() {
        let mut engine = LedPatternEngine::new();
        engine.set_fsm_pattern(COLOUR_IDLE, PatternId::Off);
        assert_eq!(engine.tick(1000), (0, 0, 0));
    }

    #[test]
    fn error_overrides_fsm() {
        let mut engine = LedPatternEngine::new();
        engine.set_fsm_pattern(COLOUR_IDLE, PatternId::Solid);
        engine.set_error_pattern(true);
        let rgb = engine.tick(0);
        // Error uses RapidFlash with red — at phase 0, should be on
        assert_eq!(rgb, COLOUR_ERROR);
    }

    #[test]
    fn fsm_overrides_connectivity() {
        let mut engine = LedPatternEngine::new();
        engine.set_connectivity_pattern(COLOUR_WIFI_CONNECTING, PatternId::FastBlink);
        engine.set_fsm_pattern(COLOUR_ACTIVE, PatternId::Solid);
        assert_eq!(engine.tick(1000), COLOUR_ACTIVE);
    }

    #[test]
    fn clearing_error_falls_back_to_fsm() {
        let mut engine = LedPatternEngine::new();
        engine.set_fsm_pattern(COLOUR_IDLE, PatternId::Solid);
        engine.set_error_pattern(true);
        let _ = engine.tick(0);
        engine.set_error_pattern(false);
        assert_eq!(engine.tick(1000), COLOUR_IDLE);
    }

    #[test]
    fn no_pattern_returns_black() {
        let mut engine = LedPatternEngine::new();
        assert_eq!(engine.tick(1000), (0, 0, 0));
    }

    #[test]
    fn fast_blink_alternates() {
        let mut engine = LedPatternEngine::new();
        engine.set_fsm_pattern((255, 255, 255), PatternId::FastBlink);
        engine.tick(0); // reset phase
        let on = engine.tick(0);
        let off = engine.tick(130); // 130ms into 250ms cycle → past half
        assert_eq!(on, (255, 255, 255));
        assert_eq!(off, (0, 0, 0));
    }

    #[test]
    fn brightness_ramp() {
        assert_eq!(LedPatternEngine::sine_brightness(0, 1000), 0);
        assert_eq!(LedPatternEngine::sine_brightness(500, 1000), 255);
        assert_eq!(LedPatternEngine::sine_brightness(1000, 1000), 0);
    }
}
