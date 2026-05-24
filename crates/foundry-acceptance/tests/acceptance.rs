//! Cucumber-rs entry point.
//!
//! Default invocation excludes `@manual` (cannot be automated) and
//! `@docker-compose` (slow; requires Docker daemon + ports).
//!
//! To run the docker-compose set explicitly:
//!   FOUNDRY_ACCEPTANCE_TAGS=docker-compose \
//!     cargo test -p foundry-acceptance --test acceptance
//!
//! To run everything except @manual:
//!   FOUNDRY_ACCEPTANCE_TAGS=all \
//!     cargo test -p foundry-acceptance --test acceptance

use cucumber::World;
use foundry_acceptance::world::FoundryWorld;

// Force-link the step modules so `inventory::submit!` items are not
// stripped from the static archive when the test binary is linked.
#[allow(unused_imports)]
use foundry_acceptance::steps::us_01_install as _us_01;
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
use foundry_acceptance::steps::us_10_comments as _us_10;
#[allow(unused_imports)]
use foundry_acceptance::steps::us_12_keyboard_nav as _us_12;

#[tokio::main]
async fn main() {
    let mode = std::env::var("FOUNDRY_ACCEPTANCE_TAGS").unwrap_or_default();
    let features_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/features");
    match mode.as_str() {
        "docker-compose" => {
            // Run only @docker-compose, exclude @manual.
            FoundryWorld::cucumber()
                .filter_run_and_exit(features_path, |feat, _rule, scenario| {
                    let has = |t: &str| {
                        scenario.tags.iter().any(|x| x == t) || feat.tags.iter().any(|x| x == t)
                    };
                    has("docker-compose") && !has("manual")
                })
                .await;
        }
        "all" => {
            // Exclude only @manual.
            FoundryWorld::cucumber()
                .filter_run_and_exit(features_path, |feat, _rule, scenario| {
                    let has = |t: &str| {
                        scenario.tags.iter().any(|x| x == t) || feat.tags.iter().any(|x| x == t)
                    };
                    !has("manual")
                })
                .await;
        }
        _ => {
            // Default: exclude both @manual and @docker-compose.
            // Cap scenario concurrency to 8 so the per-scenario
            // `LISTEN issue_events` listener tasks don't all pile up
            // on the shared Postgres container (slice 2 added these;
            // the default 64 caused intermittent missed-event failures
            // under CI load).
            FoundryWorld::cucumber()
                .max_concurrent_scenarios(8)
                .filter_run_and_exit(features_path, |feat, _rule, scenario| {
                    let has = |t: &str| {
                        scenario.tags.iter().any(|x| x == t) || feat.tags.iter().any(|x| x == t)
                    };
                    !has("manual") && !has("docker-compose")
                })
                .await;
        }
    }
}
