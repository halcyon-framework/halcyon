// ============================================================================
// SPEC-HAL-046: Unified High-Frequency Scheduler (UHFS)
// ============================================================================
//
// Fixed-step accumulator scheduler for high-frequency jobs (animations, charts).
// Runs inside the UI thread, independent of GPU frame rate.
//
// Key principles:
// - Fixed time step (default 120Hz) for deterministic simulation
// - Virtual time advances by exactly fixed_dt per step
// - Accumulator-based catchup with max_catchup_steps cap
// - No blocking, no allocation in hot path

use crate::core::app::{FrameReason, FrameTime};
use web_time::Instant;

// ============================================================================
// Types
// ============================================================================

/// Monotonic virtual time in seconds.
/// Advanced only by the scheduler with fixed steps.
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtualTime(pub f64);

/// Unique identifier for a scheduler job.
pub type SchedulerJobId = u64;

/// Signals and counters from the scheduler.
/// Updated each frame, read-only for jobs.
#[derive(Debug, Clone, Copy)]
pub struct SchedulerSignals {
    /// Number of steps dropped due to overload (accumulator exceeded max_catchup_steps)
    pub dropped_steps: u64,
    /// Total steps executed since scheduler creation
    pub total_steps: u64,
    /// Configured scheduler frequency in Hz
    pub sched_hz: f32,
    /// Fixed time step in seconds
    pub fixed_dt: f32,
}

/// Minimal event bus for scheduler events.
/// Placeholder for future chart/animation subsystems.
#[derive(Debug, Default)]
pub struct EventBus {
    events: Vec<SchedulerEvent>,
}

impl EventBus {
    /// Create a new empty event bus.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Push an event onto the bus.
    pub fn push(&mut self, event: SchedulerEvent) {
        self.events.push(event);
    }

    /// Drain all events from the bus.
    pub fn drain(&mut self) -> impl Iterator<Item = SchedulerEvent> + '_ {
        self.events.drain(..)
    }

    /// Check if the bus has any events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Scheduler event type.
/// Placeholder enum - will be extended by chart/animation subsystems.
#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    /// Custom event with a static string identifier (for testing/debugging)
    Custom(&'static str),
}

/// Context passed to high-frequency jobs each fixed step.
pub struct HFContext<'a> {
    /// Fixed time step in seconds (always the same value)
    pub dt: f32,
    /// Current virtual time in seconds
    pub virtual_time: f64,
    /// Event bus for emitting events
    pub events: &'a mut EventBus,
    /// Scheduler signals (counters, config)
    pub signals: &'a SchedulerSignals,
    /// Actual frame dt from ticker (informational only)
    pub frame_tick_dt: f32,
    /// Why this frame was triggered
    pub frame_reason: FrameReason,
    /// UI frame counter
    pub frame_index: u64,
    /// ID of the job being run
    pub job_id: SchedulerJobId,
}

/// Configuration for the high-frequency scheduler.
#[derive(Debug, Clone, Copy)]
pub struct HighFreqSchedulerConfig {
    /// Target frequency in Hz (default 120.0)
    pub sched_hz: f32,
    /// Maximum steps to run per frame before dropping (default 4)
    pub max_catchup_steps: u32,
}

impl HighFreqSchedulerConfig {
    /// Default configuration: 120Hz with max 4 catchup steps.
    pub fn default_120hz() -> Self {
        Self {
            sched_hz: 120.0,
            max_catchup_steps: 4,
        }
    }
}

impl Default for HighFreqSchedulerConfig {
    fn default() -> Self {
        Self::default_120hz()
    }
}

// ============================================================================
// Internal Types
// ============================================================================

/// Internal representation of a high-frequency job.
struct HighFreqJob {
    id: SchedulerJobId,
    active: bool,
    callback: Box<dyn FnMut(&mut HFContext<'_>)>,
}

// ============================================================================
// HighFreqScheduler
// ============================================================================

/// SPEC-HAL-046: Unified High-Frequency Scheduler.
///
/// Fixed-step accumulator scheduler running at a configurable frequency.
/// Jobs receive a context with fixed dt and virtual time.
pub struct HighFreqScheduler {
    config: HighFreqSchedulerConfig,
    fixed_dt: f32,
    virtual_time: VirtualTime,
    last_tick: Option<Instant>,
    accumulator: f32,
    next_job_id: SchedulerJobId,
    jobs: Vec<HighFreqJob>,
    signals: SchedulerSignals,
    event_bus: EventBus,
}

impl HighFreqScheduler {
    /// Create a new scheduler with the given configuration.
    pub fn new(config: HighFreqSchedulerConfig) -> Self {
        let fixed_dt = 1.0 / config.sched_hz;
        Self {
            config,
            fixed_dt,
            virtual_time: VirtualTime(0.0),
            last_tick: None,
            accumulator: 0.0,
            next_job_id: 1,
            jobs: Vec::new(),
            signals: SchedulerSignals {
                dropped_steps: 0,
                total_steps: 0,
                sched_hz: config.sched_hz,
                fixed_dt,
            },
            event_bus: EventBus::new(),
        }
    }

    /// Create a scheduler with default 120Hz configuration.
    pub fn default_120hz() -> Self {
        Self::new(HighFreqSchedulerConfig::default_120hz())
    }

    /// Register a high-frequency job.
    ///
    /// The job will be called once per fixed step with an HFContext.
    /// Returns a job ID that can be used to remove the job later.
    pub fn register_hfj<F>(&mut self, f: F) -> SchedulerJobId
    where
        F: FnMut(&mut HFContext<'_>) + 'static,
    {
        let id = self.next_job_id;
        self.next_job_id += 1;
        self.jobs.push(HighFreqJob {
            id,
            active: true,
            callback: Box::new(f),
        });
        id
    }

    /// Remove a job by ID.
    pub fn remove(&mut self, id: SchedulerJobId) {
        self.jobs.retain(|j| j.id != id);
        if self.jobs.is_empty() {
            self.last_tick = None;
            self.accumulator = 0.0;
        }
    }

    /// Returns true when at least one high-frequency job is registered.
    pub fn has_jobs(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Get the current virtual time.
    pub fn virtual_time(&self) -> VirtualTime {
        self.virtual_time
    }

    /// Get the current scheduler signals.
    pub fn signals(&self) -> &SchedulerSignals {
        &self.signals
    }

    /// Get access to the event bus (for draining events).
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Get mutable access to the event bus.
    pub fn event_bus_mut(&mut self) -> &mut EventBus {
        &mut self.event_bus
    }

    /// SPEC-HAL-046: Run the scheduler for this frame.
    ///
    /// Computes how many fixed steps to run based on elapsed time,
    /// caps to max_catchup_steps, and calls all active jobs for each step.
    ///
    /// # Arguments
    /// - `frame_time`: The current frame timing from EngineTicker
    pub fn run(&mut self, frame_time: &FrameTime) {
        if self.jobs.is_empty() {
            self.last_tick = None;
            self.accumulator = 0.0;
            return;
        }

        // First frame: initialize last_tick and return
        let Some(last_tick) = self.last_tick else {
            self.last_tick = Some(frame_time.now);
            return;
        };

        // Compute real frame dt
        let frame_dt_sec = frame_time
            .now
            .saturating_duration_since(last_tick)
            .as_secs_f32();

        // Clamp to avoid insane accumulators after long stalls (e.g., debugger pause)
        let frame_dt_clamped = frame_dt_sec.min(0.25);

        // Update last_tick
        self.last_tick = Some(frame_time.now);

        // Accumulate time
        self.accumulator += frame_dt_clamped;

        // Compute steps to run
        let mut steps = (self.accumulator / self.fixed_dt).floor() as u32;
        if steps == 0 {
            return;
        }

        // Cap steps and track dropped
        if steps > self.config.max_catchup_steps {
            self.signals.dropped_steps += (steps - self.config.max_catchup_steps) as u64;
            steps = self.config.max_catchup_steps;
        }

        // Run fixed steps
        for _ in 0..steps {
            self.accumulator -= self.fixed_dt;
            self.virtual_time.0 += self.fixed_dt as f64;
            self.signals.total_steps += 1;

            // Run all active jobs
            for job in &mut self.jobs {
                if job.active {
                    let mut cx = HFContext {
                        dt: self.fixed_dt,
                        virtual_time: self.virtual_time.0,
                        events: &mut self.event_bus,
                        signals: &self.signals,
                        frame_tick_dt: frame_time.dt.as_secs_f32(),
                        frame_reason: frame_time.reason,
                        frame_index: frame_time.frame_index,
                        job_id: job.id,
                    };
                    (job.callback)(&mut cx);
                }
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use web_time::Duration;

    /// Create a synthetic FrameTime for testing.
    fn make_frame_time(now: Instant, dt_ms: u64, frame_index: u64) -> FrameTime {
        FrameTime {
            now,
            dt: Duration::from_millis(dt_ms),
            frame_index,
            reason: FrameReason::Tick,
        }
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = HighFreqScheduler::default_120hz();
        assert!((scheduler.fixed_dt - 1.0 / 120.0).abs() < 0.0001);
        assert_eq!(scheduler.virtual_time().0, 0.0);
        assert_eq!(scheduler.signals().total_steps, 0);
        assert_eq!(scheduler.signals().dropped_steps, 0);
    }

    #[test]
    fn test_scheduler_fixed_step() {
        let mut scheduler = HighFreqScheduler::default_120hz();

        // Track virtual times
        let times = Rc::new(RefCell::new(Vec::new()));
        let times_clone = times.clone();

        scheduler.register_hfj(move |cx| {
            times_clone.borrow_mut().push(cx.virtual_time);
        });

        let start = Instant::now();

        // First frame: initializes, no steps
        scheduler.run(&make_frame_time(start, 0, 0));
        assert!(times.borrow().is_empty());

        // Second frame: 16ms later (~2 steps at 120Hz)
        let frame2 = start + Duration::from_millis(16);
        scheduler.run(&make_frame_time(frame2, 16, 1));

        // At 120Hz, fixed_dt = 8.33ms, so 16ms = 1 step (floor)
        // Actually 16 / 8.33 = 1.92, floor = 1
        let recorded = times.borrow();
        assert_eq!(recorded.len(), 1);

        // Third frame: another 16ms
        drop(recorded);
        let frame3 = frame2 + Duration::from_millis(16);
        scheduler.run(&make_frame_time(frame3, 16, 2));

        // Accumulator had ~0.67 * 8.33ms leftover, plus 16ms = ~2.59 steps = 2
        let recorded = times.borrow();
        assert_eq!(recorded.len(), 3); // 1 + 2

        // Verify steps are exactly fixed_dt apart
        let dt = recorded[1] - recorded[0];
        assert!((dt - scheduler.fixed_dt as f64).abs() < 0.0001);
    }

    #[test]
    fn test_scheduler_catchup_cap() {
        let config = HighFreqSchedulerConfig {
            sched_hz: 120.0,
            max_catchup_steps: 4,
        };
        let mut scheduler = HighFreqScheduler::new(config);

        let step_count = Rc::new(RefCell::new(0u32));
        let step_clone = step_count.clone();

        scheduler.register_hfj(move |_cx| {
            *step_clone.borrow_mut() += 1;
        });

        let start = Instant::now();

        // First frame: init
        scheduler.run(&make_frame_time(start, 0, 0));

        // Simulate 80ms stall (would be ~9-10 steps at 120Hz)
        let frame2 = start + Duration::from_millis(80);
        scheduler.run(&make_frame_time(frame2, 80, 1));

        // Should be capped to max_catchup_steps (4)
        assert_eq!(*step_count.borrow(), 4);

        // Check dropped_steps incremented
        assert!(scheduler.signals().dropped_steps > 0);
    }

    #[test]
    fn test_scheduler_virtual_time_monotonic() {
        let mut scheduler = HighFreqScheduler::default_120hz();

        let times = Rc::new(RefCell::new(Vec::new()));
        let times_clone = times.clone();

        scheduler.register_hfj(move |cx| {
            times_clone.borrow_mut().push(cx.virtual_time);
        });

        let start = Instant::now();
        scheduler.run(&make_frame_time(start, 0, 0));

        // Run several frames
        for i in 1..10 {
            let frame = start + Duration::from_millis(i * 10);
            scheduler.run(&make_frame_time(frame, 10, i));
        }

        // Verify monotonic
        let recorded = times.borrow();
        for i in 1..recorded.len() {
            assert!(recorded[i] > recorded[i - 1]);
            // Verify exact fixed_dt increments
            let diff = recorded[i] - recorded[i - 1];
            assert!((diff - scheduler.fixed_dt as f64).abs() < 0.0001);
        }
    }

    #[test]
    fn test_scheduler_job_removal() {
        let mut scheduler = HighFreqScheduler::default_120hz();

        let count1 = Rc::new(RefCell::new(0u32));
        let count2 = Rc::new(RefCell::new(0u32));
        let c1 = count1.clone();
        let c2 = count2.clone();

        let id1 = scheduler.register_hfj(move |_| {
            *c1.borrow_mut() += 1;
        });
        let _id2 = scheduler.register_hfj(move |_| {
            *c2.borrow_mut() += 1;
        });

        let start = Instant::now();
        scheduler.run(&make_frame_time(start, 0, 0));

        let frame2 = start + Duration::from_millis(10);
        scheduler.run(&make_frame_time(frame2, 10, 1));

        let before1 = *count1.borrow();
        let before2 = *count2.borrow();
        assert!(before1 > 0);
        assert!(before2 > 0);

        // Remove first job
        scheduler.remove(id1);

        let frame3 = frame2 + Duration::from_millis(10);
        scheduler.run(&make_frame_time(frame3, 10, 2));

        // First job should not increment, second should
        assert_eq!(*count1.borrow(), before1);
        assert!(*count2.borrow() > before2);
    }

    #[test]
    fn test_scheduler_no_jobs_is_noop() {
        let mut scheduler = HighFreqScheduler::default_120hz();

        let start = Instant::now();
        scheduler.run(&make_frame_time(start, 0, 0));

        let frame2 = start + Duration::from_millis(16);
        scheduler.run(&make_frame_time(frame2, 16, 1));

        assert!(!scheduler.has_jobs());
        assert_eq!(scheduler.virtual_time().0, 0.0);
        assert_eq!(scheduler.signals().total_steps, 0);
        assert_eq!(scheduler.signals().dropped_steps, 0);
        assert_eq!(scheduler.accumulator, 0.0);
        assert!(scheduler.last_tick.is_none());
    }
}
