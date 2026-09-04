//! Cross-cutting test helpers.

/// How many scenarios the lanes run at once.
///
/// ONE writer, because this number is now a CONTRACT WITH THE BROWSER
/// CONTAINER, not merely a knob. `browser_harness` must give the Selenium node
/// at least this many session slots: the node defaults to ONE and clamps
/// per-browser concurrency unless explicitly overridden, so a lane running six
/// scenarios against an un-overridden node queues five of them and the router
/// times them out. Kept here rather than in `tests/acceptance.rs` so the two
/// sides cannot drift — which is exactly how the mismatch arrived, the host
/// `chromedriver` this container replaced having served any number of sessions.
pub const MAX_CONCURRENT_SCENARIOS: usize = 6;

pub mod browser_harness;
pub mod compose_harness;
pub mod file_upload_env;
pub mod harness;
pub mod heartbeat_env;
pub mod html_assertions;
pub mod metrics_scrape;
pub mod multi_replica_harness;
pub mod notify_recorder;
pub mod oidc_issuer;
pub mod pg_backup;
pub mod readme_inspect;
pub mod round_robin_proxy;
pub mod sse_client;
pub mod test_migration;
pub mod tombstone_factory;
pub mod webhook_receiver;
