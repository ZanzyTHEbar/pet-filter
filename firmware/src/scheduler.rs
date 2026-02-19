//! Timer/scheduler engine.
//!
//! Coexists with the ammonia-triggered FSM cycle.  The scheduler
//! notifies a [`SchedulerDelegate`] when schedules fire; the main
//! loop implements the delegate to push events into the ISR queue.
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │                     Trigger Sources                          │
//! │                                                              │
//! │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌──────────┐   │
//! │  │ NH3 Sensor│  │ Periodic  │  │ Boost     │  │ One-Shot │   │
//! │  │ (ISR/ULP) │  │ Schedule  │  │ Mode      │  │ Timer    │   │
//! │  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────┬────┘   │
//! │        │              │              │              │        │
//! │        ▼              ▼              ▼              ▼        │
//! │  ┌────────────────────────────────────────────────────────┐  │
//! │  │              SchedulerDelegate                         │  │
//! │  │       (main loop pushes into Event Queue)              │  │
//! │  └───────────────────────┬────────────────────────────────┘  │
//! │                          │                                   │
//! │                          ▼                                   │
//! │                    AppService.tick()                         │
//! │                    AppService.handle_command()               │
//! └──────────────────────────────────────────────────────────────┘
//! ```

use crate::app::ports::{ScheduleFiredKind, SchedulerDelegate};
use log::info;

// ═══════════════════════════════════════════════════════════════
//  Schedule types
// ═══════════════════════════════════════════════════════════════

/// A single schedule entry.
#[derive(Debug, Clone)]
pub struct Schedule {
    /// Human-readable label (e.g., "Morning refresh").
    pub label: &'static str,
    /// Type of schedule.
    pub kind: ScheduleKind,
    /// Whether this schedule is currently enabled.
    pub enabled: bool,
}

/// The type of schedule determines how and when it fires.
#[derive(Debug, Clone)]
pub enum ScheduleKind {
    /// Fire every `interval_secs` seconds, run scrub for `duration_secs`.
    Periodic {
        interval_secs: u32,
        duration_secs: u16,
    },
    /// Continuous scrub for `duration_secs`, then stop.
    /// Intended for "I'm having guests, blast the scrubber for an hour".
    Boost {
        duration_secs: u16,
    },
    /// Fire once after `delay_secs`, then auto-disable.
    OneShot {
        delay_secs: u32,
    },
}

/// Time-of-day restriction (quiet hours).
/// During quiet hours, periodic schedules are suppressed.
#[derive(Debug, Clone, Copy)]
pub struct QuietHours {
    /// Start hour (0-23 inclusive).  E.g. 23 = 11 PM.
    pub start_hour: u8,
    /// End hour (0-23 inclusive).  E.g. 7 = 7 AM.
    pub end_hour: u8,
}

impl QuietHours {
    /// Check if the given hour is within quiet hours.
    pub fn is_quiet(&self, hour: u8) -> bool {
        if self.start_hour <= self.end_hour {
            // e.g., 9..17 (daytime quiet)
            hour >= self.start_hour && hour < self.end_hour
        } else {
            // e.g., 23..7 (overnight quiet — wraps around midnight)
            hour >= self.start_hour || hour < self.end_hour
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Scheduler engine
// ═══════════════════════════════════════════════════════════════

/// Maximum number of concurrent schedules (stack-allocated).
const MAX_SCHEDULES: usize = 4;

/// The scheduler engine.
///
/// This struct is intentionally decoupled from the event system.
/// When a schedule fires, it invokes the [`SchedulerDelegate`] callback
/// rather than directly pushing events.  This makes the scheduler
/// independently testable and reusable across different execution contexts.
pub struct Scheduler {
    /// Active schedules.
    schedules: [Option<ScheduleEntry>; MAX_SCHEDULES],
    /// Quiet hours restriction (applies to Periodic schedules).
    quiet_hours: Option<QuietHours>,
    /// Global enable flag.
    enabled: bool,
}

/// Internal bookkeeping for a live schedule.
#[derive(Debug, Clone)]
struct ScheduleEntry {
    schedule: Schedule,
    /// Ticks elapsed since last fire (for Periodic).
    elapsed_ticks: u64,
    /// Ticks remaining for Boost mode.
    remaining_ticks: Option<u64>,
    /// Whether the schedule has fired (for OneShot).
    fired: bool,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            schedules: [None, None, None, None],
            quiet_hours: None,
            enabled: true,
        }
    }

    /// Add a schedule.  Returns the slot index, or `None` if full.
    pub fn add(&mut self, schedule: Schedule) -> Option<usize> {
        for (i, slot) in self.schedules.iter_mut().enumerate() {
            if slot.is_none() {
                info!("Scheduler: added '{}' at slot {}", schedule.label, i);
                *slot = Some(ScheduleEntry {
                    schedule,
                    elapsed_ticks: 0,
                    remaining_ticks: None,
                    fired: false,
                });
                return Some(i);
            }
        }
        None // All slots full.
    }

    /// Remove a schedule by slot index.
    pub fn remove(&mut self, slot: usize) {
        if slot < MAX_SCHEDULES {
            if let Some(entry) = &self.schedules[slot] {
                info!("Scheduler: removed '{}' from slot {}", entry.schedule.label, slot);
            }
            self.schedules[slot] = None;
        }
    }

    /// Set quiet hours.
    pub fn set_quiet_hours(&mut self, quiet: QuietHours) {
        self.quiet_hours = Some(quiet);
        info!(
            "Scheduler: quiet hours {}:00–{}:00",
            quiet.start_hour, quiet.end_hour
        );
    }

    /// Clear quiet hours.
    pub fn clear_quiet_hours(&mut self) {
        self.quiet_hours = None;
    }

    /// Enable or disable the entire scheduler.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Tick the scheduler.  Call once per control loop tick.
    ///
    /// When a schedule fires, `delegate.on_schedule_fired()` is called
    /// with the schedule label and fire kind.  The caller decides what
    /// to do with the notification (e.g., push an event, log, etc.).
    ///
    /// # Parameters
    ///
    /// * `current_hour` — current hour-of-day (0-23), or `None` if
    ///   wall-clock time is unavailable (pre-NTP sync).
    /// * `tick_secs` — duration of one tick in seconds.
    /// * `delegate` — receives fire notifications.
    pub fn tick(
        &mut self,
        current_hour: Option<u8>,
        tick_secs: f32,
        delegate: &mut dyn SchedulerDelegate,
    ) {
        if !self.enabled {
            return;
        }

        let is_quiet = current_hour
            .and_then(|h| self.quiet_hours.map(|q| q.is_quiet(h)))
            .unwrap_or(false);

        for slot in self.schedules.iter_mut() {
            let entry = match slot {
                Some(e) if e.schedule.enabled => e,
                _ => continue,
            };

            entry.elapsed_ticks += 1;
            let elapsed_secs = entry.elapsed_ticks as f32 * tick_secs;

            match &entry.schedule.kind {
                ScheduleKind::Periodic { interval_secs, .. } => {
                    if is_quiet {
                        continue; // Suppress during quiet hours.
                    }
                    if elapsed_secs >= *interval_secs as f32 {
                        info!(
                            "Scheduler: '{}' periodic fire (every {}s)",
                            entry.schedule.label, interval_secs
                        );
                        delegate.on_schedule_fired(
                            entry.schedule.label,
                            ScheduleFiredKind::Periodic,
                        );
                        entry.elapsed_ticks = 0;
                    }
                }

                ScheduleKind::Boost { duration_secs } => {
                    if entry.remaining_ticks.is_none() {
                        // First tick — start boost.
                        let ticks = (*duration_secs as f32 / tick_secs) as u64;
                        entry.remaining_ticks = Some(ticks);
                        info!(
                            "Scheduler: '{}' boost started ({}s)",
                            entry.schedule.label, duration_secs
                        );
                        delegate.on_schedule_fired(
                            entry.schedule.label,
                            ScheduleFiredKind::Boost,
                        );
                    }

                    if let Some(ref mut remaining) = entry.remaining_ticks {
                        if *remaining == 0 {
                            info!("Scheduler: '{}' boost complete", entry.schedule.label);
                            entry.schedule.enabled = false; // Auto-disable.
                        } else {
                            *remaining = remaining.saturating_sub(1);
                        }
                    }
                }

                ScheduleKind::OneShot { delay_secs } => {
                    if !entry.fired && elapsed_secs >= *delay_secs as f32 {
                        info!(
                            "Scheduler: '{}' one-shot fired (after {}s)",
                            entry.schedule.label, delay_secs
                        );
                        delegate.on_schedule_fired(
                            entry.schedule.label,
                            ScheduleFiredKind::OneShot,
                        );
                        entry.fired = true;
                        entry.schedule.enabled = false; // Auto-disable.
                    }
                }
            }
        }
    }

    /// Number of active (enabled) schedules.
    pub fn active_count(&self) -> usize {
        self.schedules
            .iter()
            .filter(|s| s.as_ref().map_or(false, |e| e.schedule.enabled))
            .count()
    }
}

// ═══════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Test delegate that records fire events.
    struct RecordingDelegate {
        fires: Vec<(String, ScheduleFiredKind)>,
    }

    impl RecordingDelegate {
        fn new() -> Self {
            Self { fires: Vec::new() }
        }
    }

    impl SchedulerDelegate for RecordingDelegate {
        fn on_schedule_fired(&mut self, label: &str, kind: ScheduleFiredKind) {
            self.fires.push((label.to_string(), kind));
        }
    }

    #[test]
    fn periodic_fires_at_interval() {
        let mut sched = Scheduler::new();
        let mut delegate = RecordingDelegate::new();

        sched.add(Schedule {
            label: "test-periodic",
            kind: ScheduleKind::Periodic {
                interval_secs: 10,
                duration_secs: 5,
            },
            enabled: true,
        });

        // Tick 9 times at 1s each — should NOT fire.
        for _ in 0..9 {
            sched.tick(None, 1.0, &mut delegate);
        }
        assert!(delegate.fires.is_empty());

        // 10th tick — should fire.
        sched.tick(None, 1.0, &mut delegate);
        assert_eq!(delegate.fires.len(), 1);
        assert_eq!(delegate.fires[0].0, "test-periodic");
        assert_eq!(delegate.fires[0].1, ScheduleFiredKind::Periodic);
    }

    #[test]
    fn oneshot_fires_once() {
        let mut sched = Scheduler::new();
        let mut delegate = RecordingDelegate::new();

        sched.add(Schedule {
            label: "test-oneshot",
            kind: ScheduleKind::OneShot { delay_secs: 5 },
            enabled: true,
        });

        for _ in 0..4 {
            sched.tick(None, 1.0, &mut delegate);
        }
        assert!(delegate.fires.is_empty());

        // 5th tick — fires.
        sched.tick(None, 1.0, &mut delegate);
        assert_eq!(delegate.fires.len(), 1);
        assert_eq!(delegate.fires[0].1, ScheduleFiredKind::OneShot);

        // Subsequent ticks — no more fires.
        for _ in 0..10 {
            sched.tick(None, 1.0, &mut delegate);
        }
        assert_eq!(delegate.fires.len(), 1);
    }

    #[test]
    fn quiet_hours_suppress_periodic() {
        let mut sched = Scheduler::new();
        let mut delegate = RecordingDelegate::new();

        sched.add(Schedule {
            label: "test-quiet",
            kind: ScheduleKind::Periodic {
                interval_secs: 1,
                duration_secs: 1,
            },
            enabled: true,
        });
        sched.set_quiet_hours(QuietHours {
            start_hour: 23,
            end_hour: 7,
        });

        // Hour 2 (quiet) — should NOT fire.
        for _ in 0..5 {
            sched.tick(Some(2), 1.0, &mut delegate);
        }
        assert!(delegate.fires.is_empty());

        // Hour 12 (not quiet) — should fire.
        sched.tick(Some(12), 1.0, &mut delegate);
        assert_eq!(delegate.fires.len(), 1);
    }

    #[test]
    fn boost_starts_and_auto_disables() {
        let mut sched = Scheduler::new();
        let mut delegate = RecordingDelegate::new();

        sched.add(Schedule {
            label: "test-boost",
            kind: ScheduleKind::Boost { duration_secs: 3 },
            enabled: true,
        });

        // First tick starts boost.
        sched.tick(None, 1.0, &mut delegate);
        assert_eq!(delegate.fires.len(), 1);
        assert_eq!(delegate.fires[0].1, ScheduleFiredKind::Boost);
        assert_eq!(sched.active_count(), 1);

        // Run through remaining ticks.
        for _ in 0..3 {
            sched.tick(None, 1.0, &mut delegate);
        }
        // Should auto-disable after duration.
        assert_eq!(sched.active_count(), 0);
    }

    #[test]
    fn disabled_scheduler_does_nothing() {
        let mut sched = Scheduler::new();
        let mut delegate = RecordingDelegate::new();

        sched.add(Schedule {
            label: "test-disabled",
            kind: ScheduleKind::Periodic {
                interval_secs: 1,
                duration_secs: 1,
            },
            enabled: true,
        });
        sched.set_enabled(false);

        for _ in 0..10 {
            sched.tick(None, 1.0, &mut delegate);
        }
        assert!(delegate.fires.is_empty());
    }
}
