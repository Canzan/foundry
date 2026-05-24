//! Clock port — abstracted so tests can advance time without sleeping.
//!
//! In addition to `now()`, the port exposes `sleep()` so brute-force-delay
//! style code paths can `await state.clock.sleep(Duration::from_secs(5))`
//! without test suites blocking on wall-clock. `MockClock::sleep` records
//! the requested duration and returns immediately; production
//! [`SystemClock`] forwards to `tokio::time::sleep`.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Provides "now" and a side-effecting `sleep` used for brute-force
/// throttling (NFR-SEC-02).
#[async_trait]
pub trait Clock: Send + Sync + Debug + 'static {
    fn now(&self) -> time::OffsetDateTime;
    /// Wait for `duration` before continuing. Production sleeps; tests
    /// record the request and return immediately.
    async fn sleep(&self, duration: Duration);
}

#[derive(Debug, Default, Clone)]
pub struct SystemClock;

#[async_trait]
impl Clock for SystemClock {
    fn now(&self) -> time::OffsetDateTime {
        time::OffsetDateTime::now_utc()
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// A sleep request that the brute-force delay path issued. Tests use
/// `MockClock::recorded_sleeps()` to assert the NFR-SEC-02 contract
/// ("the handler scheduled a >= 4500ms wait") without the test thread
/// actually blocking for 5 seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedSleep {
    pub duration: Duration,
}

/// Test-only clock. Tracks current time in unix-nanoseconds; `advance`
/// shifts it deterministically. `sleep` records the requested duration
/// instead of blocking — see [`Clock::sleep`] doc above.
#[derive(Debug)]
pub struct MockClock {
    epoch_nanos: AtomicI64,
    sleeps: Mutex<Vec<RecordedSleep>>,
}

impl MockClock {
    pub fn new(now: time::OffsetDateTime) -> Arc<Self> {
        Arc::new(Self {
            epoch_nanos: AtomicI64::new(now.unix_timestamp_nanos() as i64),
            sleeps: Mutex::new(Vec::new()),
        })
    }

    pub fn at(now: time::OffsetDateTime) -> Arc<Self> {
        Self::new(now)
    }

    pub fn advance(&self, by: Duration) {
        let delta = by.as_nanos() as i64;
        self.epoch_nanos.fetch_add(delta, Ordering::SeqCst);
    }

    pub fn rewind(&self, by: Duration) {
        let delta = by.as_nanos() as i64;
        self.epoch_nanos.fetch_sub(delta, Ordering::SeqCst);
    }

    pub fn set(&self, now: time::OffsetDateTime) {
        self.epoch_nanos
            .store(now.unix_timestamp_nanos() as i64, Ordering::SeqCst);
    }

    /// Read every sleep the SUT requested through this clock so far.
    pub fn recorded_sleeps(&self) -> Vec<RecordedSleep> {
        self.sleeps.lock().expect("recorded_sleeps mutex").clone()
    }
}

#[async_trait]
impl Clock for MockClock {
    fn now(&self) -> time::OffsetDateTime {
        let nanos = self.epoch_nanos.load(Ordering::SeqCst) as i128;
        time::OffsetDateTime::from_unix_timestamp_nanos(nanos)
            .expect("nanos in OffsetDateTime range")
    }

    async fn sleep(&self, duration: Duration) {
        self.sleeps
            .lock()
            .expect("recorded_sleeps mutex")
            .push(RecordedSleep { duration });
        // Do NOT actually sleep — tests want to observe the request,
        // not the wall-clock blocking.
    }
}
