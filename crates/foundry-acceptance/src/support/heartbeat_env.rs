//! Per-scenario override for the SSE heartbeat interval.
//!
//! Production default is `DEFAULT_SSE_HEARTBEAT_MS` (25s). The US-09
//! heartbeat scenario asserts ≥2 keepalives in 700ms of quiet — that
//! is only feasible with a shortened interval. A process-global atomic
//! gives the override scope: a scenario calls `override_heartbeat_ms`
//! BEFORE its first `InProcHarness::spawn` and clears it via
//! `clear_heartbeat_override` in the world's per-scenario reset.
//!
//! Reading order: harness.rs spawn reads `current_heartbeat_ms()` and
//! plumbs it into `AppState.sse_heartbeat_ms`. Scenarios that do NOT
//! call `override_heartbeat_ms` get the default (and never observe a
//! heartbeat, because no scenario runs >25s).

use std::sync::atomic::{AtomicU64, Ordering};

/// Sentinel meaning "no override" — interpreted by `current_heartbeat_ms`
/// as "fall back to production default".
const NO_OVERRIDE: u64 = 0;

static OVERRIDE_HEARTBEAT_MS: AtomicU64 = AtomicU64::new(NO_OVERRIDE);

/// Set the heartbeat interval (ms) to use for the NEXT spawned harness.
/// Persists until `clear_heartbeat_override` is called.
pub fn override_heartbeat_ms(ms: u64) {
    OVERRIDE_HEARTBEAT_MS.store(ms.max(1), Ordering::SeqCst);
}

/// Clear the override so subsequent scenarios get the production default.
pub fn clear_heartbeat_override() {
    OVERRIDE_HEARTBEAT_MS.store(NO_OVERRIDE, Ordering::SeqCst);
}

/// Read the current override, if any. Returns `None` when no scenario
/// has set a value — harness.rs then defaults to `DEFAULT_SSE_HEARTBEAT_MS`.
pub fn current_heartbeat_ms() -> Option<u64> {
    match OVERRIDE_HEARTBEAT_MS.load(Ordering::SeqCst) {
        NO_OVERRIDE => None,
        v => Some(v),
    }
}
