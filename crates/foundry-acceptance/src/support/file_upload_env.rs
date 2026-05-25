//! Process-wide override for the US-11 attachment-size cap.
//!
//! `std::env::set_var` is `unsafe` on Rust 1.85 (thread-safety) and the
//! acceptance crate carries `#![forbid(unsafe_code)]`. This atomic
//! sits in for the env var: the harness reads it inside
//! `InProcHarness::spawn` and falls back to
//! `foundry_app::DEFAULT_FILE_UPLOAD_MAX_MB` when unset.
//!
//! The override is process-wide and **last-write-wins**. Slice-3
//! cucumber-rs runs at `max_concurrent_scenarios=8`; every US-11
//! scenario sets the same value (10 MB per Background), so concurrent
//! writes from sibling scenarios collapse to a single observable
//! value. Future stories that need divergent caps per scenario will
//! need to widen this from a single `AtomicU64` to a per-thread slot
//! or thread the value through AppState explicitly.

use std::sync::atomic::{AtomicU64, Ordering};

/// Sentinel for "no override set". The cap is in MB; 0 is not a
/// meaningful production value, so we can use it as the "unset" flag.
const UNSET: u64 = 0;

static OVERRIDE_MB: AtomicU64 = AtomicU64::new(UNSET);

/// Pin the override. Called by the US-11 `Given the FILE_UPLOAD_MAX_MB
/// env var is set to N` step.
pub fn override_file_upload_max_mb(mb: u64) {
    OVERRIDE_MB.store(mb, Ordering::SeqCst);
}

/// Read the override. Returns `None` when no scenario has pinned one.
pub fn current_file_upload_max_mb() -> Option<u64> {
    let v = OVERRIDE_MB.load(Ordering::SeqCst);
    if v == UNSET {
        None
    } else {
        Some(v)
    }
}
