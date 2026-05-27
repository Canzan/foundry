//! Cucumber-rs entry point.
//!
//! Default invocation excludes `@manual` (cannot be automated),
//! `@manual-trigger` (slice-3 opt-in lane — docker-compose multi-replica),
//! `@docker-compose` (slow; requires Docker daemon + ports), and
//! `@slow` (slice-7 — single ~10-15s wall-clock cap scenario).
//!
//! To run the docker-compose set explicitly:
//!   FOUNDRY_ACCEPTANCE_TAGS=docker-compose \
//!     cargo test -p foundry-acceptance --test acceptance
//!
//! To run everything except @manual / @manual-trigger (includes @slow):
//!   FOUNDRY_ACCEPTANCE_TAGS=all \
//!     cargo test -p foundry-acceptance --test acceptance
//!
//! `@manual-trigger` (slice-3 US-02 docker-compose Caddy stack) is always
//! excluded by default — it requires the production-shaped Caddy +
//! 3-replica compose fixture and runs only on explicit selection.
//!
//! `@slow` (slice-7 introduction per D3 = A) gates scenarios whose
//! wall-clock cost (~10-15s) would dominate the fast-loop budget. The
//! single slice-7 `@slow` scenario is the 11k-row GC cap probe; it is
//! included in `FOUNDRY_ACCEPTANCE_TAGS=all` and any explicit `@slice7`
//! selection, but excluded from the default fast-loop.

use cucumber::World;
use foundry_acceptance::world::FoundryWorld;

// Force-link the step modules so `inventory::submit!` items are not
// stripped from the static archive when the test binary is linked.
#[allow(unused_imports)]
use foundry_acceptance::steps::handler_instrumentation as _slice6;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_01_install as _us_01;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_02_multi_replica as _us_02;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_03_backup_restore as _us_03;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_04_rolling_upgrade as _us_04;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_05_bootstrap as _us_05;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_06_signin as _us_06;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_07_project_create as _us_07;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_08_file_issue as _us_08;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_09_realtime_sse as _us_09;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_10_comment_edit_delete as _us_10_edit;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_10_comments as _us_10;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_10_tombstone_gc as _us_10_gc;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_11_attachments as _us_11;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_12_keyboard_nav as _us_12;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_13_contributor_onboarding as _us_13;

#[tokio::main]
async fn main() {
    let mode = std::env::var("FOUNDRY_ACCEPTANCE_TAGS").unwrap_or_default();
    let features_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/features");
    match mode.as_str() {
        "docker-compose" => {
            // Run only @docker-compose, exclude @manual / @manual-trigger.
            FoundryWorld::cucumber()
                .filter_run_and_exit(features_path, |feat, _rule, scenario| {
                    let has = |t: &str| {
                        scenario.tags.iter().any(|x| x == t) || feat.tags.iter().any(|x| x == t)
                    };
                    has("docker-compose") && !has("manual") && !has("manual-trigger")
                })
                .await;
        }
        "all" => {
            // Exclude @manual + @manual-trigger (slice-3 opt-in lane).
            // Cap scenario concurrency to 6 to mirror the default lane:
            // the slice-6 `db_connections_in_use` scenario relies on a
            // single subprocess saturating its own 10-conn sqlx pool with
            // 32 in-flight /readyz pounders. Under unbounded cucumber
            // concurrency, N×10 pool demand can exceed the shared
            // Postgres container's 100-connection ceiling — remote
            // acquires block, /readyz pounders hit their 2s timeout
            // before owning a connection, and the local `in_use` gauge
            // never rises above 0 across the scrape window. Matching the
            // default-lane cap restores the invariant that `@all` =
            // "default lane + the @slow + @docker-compose scenarios".
            FoundryWorld::cucumber()
                .max_concurrent_scenarios(6)
                .filter_run_and_exit(features_path, |feat, _rule, scenario| {
                    let has = |t: &str| {
                        scenario.tags.iter().any(|x| x == t) || feat.tags.iter().any(|x| x == t)
                    };
                    !has("manual") && !has("manual-trigger")
                })
                .await;
        }
        _ => {
            // Default: exclude @manual, @manual-trigger, @docker-compose,
            // and @slow (slice-7 11k-row cap scenario; ~10-15s).
            // Cap scenario concurrency to 6 so the per-scenario
            // `LISTEN issue_events` listener tasks don't all pile up
            // on the shared Postgres container, AND the US-04
            // advisory-lock-race scenarios (which each spawn 2-replica
            // concurrent migration boots holding pool connections for
            // up to 2s) do not saturate the shared container. Slice 2
            // ran at 8; slice 3 dropped to 6 to amortise the new
            // contention.
            FoundryWorld::cucumber()
                .max_concurrent_scenarios(6)
                .filter_run_and_exit(features_path, |feat, _rule, scenario| {
                    let has = |t: &str| {
                        scenario.tags.iter().any(|x| x == t) || feat.tags.iter().any(|x| x == t)
                    };
                    !has("manual")
                        && !has("manual-trigger")
                        && !has("docker-compose")
                        && !has("slow")
                })
                .await;
        }
    }
}
