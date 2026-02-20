//! Streaming support for continuous telemetry push.
//!
//! The `StreamManager` tracks active subscriptions and handles
//! backpressure (dropping frames if the transport can't keep up).

/// Stream subscription state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Not subscribed — no frames sent.
    Inactive,
    /// Actively streaming at the configured interval.
    Active { interval_ms: u32 },
    /// Paused (e.g. during deep sleep transition).
    Paused,
}

/// Manages streaming telemetry to a connected client.
/// Manages streaming telemetry subscriptions.
pub struct StreamManager {
    state: StreamState,
    elapsed_ms: u32,
    /// Number of frames dropped due to backpressure.
    dropped_count: u32,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            state: StreamState::Inactive,
            elapsed_ms: 0,
            dropped_count: 0,
        }
    }

    /// Subscribe to telemetry streaming.
    pub fn subscribe(&mut self, interval_ms: u32) {
        self.state = StreamState::Active {
            interval_ms: interval_ms.max(100), // Floor at 100ms (10 Hz)
        };
        self.elapsed_ms = 0;
    }

    /// Unsubscribe from telemetry streaming.
    pub fn unsubscribe(&mut self) {
        self.state = StreamState::Inactive;
    }

    /// Pause streaming (e.g. before sleep).
    pub fn pause(&mut self) {
        if let StreamState::Active { .. } = self.state {
            self.state = StreamState::Paused;
        }
    }

    /// Resume streaming after pause.
    pub fn resume(&mut self, interval_ms: u32) {
        if self.state == StreamState::Paused {
            self.state = StreamState::Active { interval_ms };
            self.elapsed_ms = 0;
        }
    }

    /// Tick the stream manager.  Returns `true` if a frame should be sent.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        match self.state {
            StreamState::Active { interval_ms } => {
                self.elapsed_ms += delta_ms;
                if self.elapsed_ms >= interval_ms {
                    self.elapsed_ms -= interval_ms;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Record a dropped frame (transport couldn't keep up).
    pub fn record_drop(&mut self) {
        self.dropped_count += 1;
    }

    pub fn state(&self) -> StreamState {
        self.state
    }

    pub fn dropped_count(&self) -> u32 {
        self.dropped_count
    }
}
