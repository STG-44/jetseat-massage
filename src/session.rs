//! Session supervision is independent of egui repainting or window visibility.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime},
};

pub const WATCHDOG: Duration = Duration::from_millis(1500);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Stopped,
    Expired,
    Suspended,
}

pub struct Supervisor {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Supervisor {
    /// Pulse returns false when its output has stopped. End is always delivered.
    pub fn start(
        duration: Duration,
        mut pulse: impl FnMut() -> bool + Send + 'static,
        mut end: impl FnMut(End) + Send + 'static,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        let handle = thread::Builder::new()
            .name("jetseat-session".into())
            .spawn(move || {
                let _activity = crate::power::Activity::begin();
                let started = Instant::now();
                let mut previous = SystemTime::now();
                let reason = loop {
                    if !flag.load(Ordering::Acquire) {
                        break End::Stopped;
                    }
                    let now = SystemTime::now();
                    if let Some(reason) = decision(started.elapsed(), duration, previous, now) {
                        break reason;
                    }
                    previous = now;
                    if !pulse() {
                        break End::Stopped;
                    }
                    thread::park_timeout(Duration::from_millis(100));
                };
                flag.store(false, Ordering::Release);
                end(reason);
            })
            .expect("start independent session supervisor");
        Self {
            running,
            thread: Some(handle),
        }
    }
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(handle) = self.thread.take() {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        self.stop();
    }
}
pub fn wall_millis() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn fresh(heartbeat: u64) -> bool {
    wall_millis()
        .checked_sub(heartbeat)
        .is_some_and(|elapsed| elapsed < WATCHDOG.as_millis() as u64)
}
fn decision(
    elapsed: Duration,
    duration: Duration,
    previous: SystemTime,
    now: SystemTime,
) -> Option<End> {
    if paused(previous, now) {
        Some(End::Suspended)
    } else if elapsed >= duration {
        Some(End::Expired)
    } else {
        None
    }
}
fn paused(previous: SystemTime, now: SystemTime) -> bool {
    now.duration_since(previous)
        .map_or(true, |elapsed| elapsed >= WATCHDOG)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    #[test]
    fn background_session_does_not_require_gui_heartbeats() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let pulses = ticks.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let _session = Supervisor::start(
            Duration::from_millis(1900),
            move || {
                pulses.fetch_add(1, Ordering::Relaxed);
                true
            },
            move |reason| {
                let _ = tx.send(reason);
            },
        );
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            End::Expired
        );
        assert!(ticks.load(Ordering::Relaxed) >= 10);
    }
    #[test]
    fn thirty_minute_timer_and_suspend_are_separate_from_window_visibility() {
        let start = SystemTime::now();
        assert!(!paused(start, start + Duration::from_millis(100)));
        assert!(paused(start, start + Duration::from_secs(120)));
        let duration = Duration::from_secs(30 * 60);
        let mut previous = start;
        // Simulate 30 minutes of background pulses without a single GUI callback.
        for tick in 0..18_000_u64 {
            let elapsed = Duration::from_millis(tick * 100);
            let now = start + elapsed;
            assert_eq!(decision(elapsed, duration, previous, now), None);
            previous = now;
        }
        assert_eq!(
            decision(duration, duration, previous, start + duration),
            Some(End::Expired)
        );
        assert_eq!(
            decision(
                Duration::from_secs(120),
                duration,
                start,
                start + Duration::from_secs(120)
            ),
            Some(End::Suspended)
        );
    }
    #[test]
    fn dropping_the_session_joins_the_supervisor_and_ends_activity() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut session = Supervisor::start(
            Duration::from_secs(1800),
            || true,
            move |reason| {
                let _ = tx.send(reason);
            },
        );
        session.stop();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            End::Stopped
        );
    }
}
