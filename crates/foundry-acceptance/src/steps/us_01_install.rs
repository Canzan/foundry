//! US-01 step definitions — docker-compose harness.
//!
//! All three @docker-compose scenarios are wired here. The @manual
//! scenario carries no steps (the test runner skips it via tag filter).

use crate::support::compose_harness::{read_compose_yml, ComposeStack};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use std::time::Duration;

// Background ------------------------------------------------------------

#[given(regex = r"^an empty working directory with a Foundry checkout and a default `\.env`$")]
async fn empty_working_dir(world: &mut FoundryWorld) {
    world.compose = None;
    world.compose_bootstrap_url = None;
    world.admin_already_claimed = false;
}

#[given(regex = r"^no Foundry containers or volumes exist on this machine$")]
async fn no_containers(_world: &mut FoundryWorld) {
    // ComposeStack uses a unique project name per scenario, so prior
    // runs cannot interfere. Nothing to do.
}

// Scenario 1 — fresh install -------------------------------------------

#[when(regex = r"^the operator starts the Foundry stack with `docker compose up -d`$")]
async fn compose_up(world: &mut FoundryWorld) {
    let stack = ComposeStack::new();
    stack.up_detached().expect("docker compose up");
    world.compose = Some(stack);
}

#[then(regex = r"^within (\d+) seconds the foundry container reports healthy on `/healthz`$")]
async fn foundry_healthy(world: &mut FoundryWorld, timeout_seconds: u64) {
    let stack = world.compose.as_ref().expect("compose stack present");
    stack
        .wait_for_foundry_healthy(Duration::from_secs(timeout_seconds))
        .await
        .expect("foundry /healthz healthy");
}

#[then(regex = r"^the postgres container reports healthy$")]
async fn postgres_healthy(world: &mut FoundryWorld) {
    let stack = world.compose.as_ref().expect("compose stack present");
    stack
        .assert_service_healthy("postgres")
        .expect("postgres healthy");
}

#[then(
    regex = r"^the foundry container logs contain exactly one line beginning with `\[BOOTSTRAP\]`$"
)]
async fn one_bootstrap_log_line(world: &mut FoundryWorld) {
    let stack = world.compose.as_ref().expect("compose stack present");
    let lines = stack.bootstrap_lines().expect("read logs");
    assert_eq!(
        lines.len(),
        1,
        "expected exactly one [BOOTSTRAP] line, got {}: {:#?}",
        lines.len(),
        lines
    );
    world.compose_bootstrap_url = Some(lines[0].clone());
}

#[then(regex = r"^that line contains a URL with a token query parameter$")]
async fn bootstrap_url_has_token(world: &mut FoundryWorld) {
    let line = world
        .compose_bootstrap_url
        .as_ref()
        .expect("bootstrap line captured by prior step");
    let has_url = line.contains("http://") || line.contains("https://");
    assert!(has_url, "bootstrap line missing URL: {line}");
    assert!(
        line.contains("token="),
        "bootstrap line missing token query parameter: {line}"
    );
}

// Scenario 2 — re-run idempotent ---------------------------------------

#[given(regex = r"^the operator has already claimed admin from a prior install$")]
async fn admin_already_claimed_prior_install(world: &mut FoundryWorld) {
    let stack = ComposeStack::new();
    stack.up_detached().expect("initial docker compose up");
    stack
        .wait_for_foundry_healthy(Duration::from_secs(300))
        .await
        .expect("initial foundry /healthz healthy");
    stack.pre_claim_admin().expect("insert pre-claim workspace");
    let baseline = stack.bootstrap_lines().expect("read logs");
    let mut stack = stack;
    stack.initial_bootstrap_lines = baseline;
    world.compose = Some(stack);
    world.admin_already_claimed = true;
}

#[when(regex = r"^the operator runs `docker compose up -d` a second time$")]
async fn compose_up_again(world: &mut FoundryWorld) {
    let stack = world
        .compose
        .as_ref()
        .expect("compose stack from given step");
    stack
        .restart_service("foundry")
        .expect("restart foundry service");
}

#[then(regex = r"^the foundry container reports healthy on `/healthz` within (\d+) seconds$")]
async fn foundry_healthy_within(world: &mut FoundryWorld, timeout_seconds: u64) {
    let stack = world.compose.as_ref().expect("compose stack present");
    stack
        .wait_for_foundry_healthy(Duration::from_secs(timeout_seconds))
        .await
        .expect("foundry /healthz healthy");
}

#[then(
    regex = r"^the foundry container logs contain zero new lines beginning with `\[BOOTSTRAP\]`$"
)]
async fn zero_new_bootstrap_lines(world: &mut FoundryWorld) {
    let stack = world.compose.as_ref().expect("compose stack present");
    let now = stack.bootstrap_lines().expect("read logs");
    let baseline = &stack.initial_bootstrap_lines;
    let new_lines: Vec<_> = now.iter().filter(|l| !baseline.contains(l)).collect();
    assert!(
        new_lines.is_empty(),
        "expected zero new [BOOTSTRAP] lines after re-run, got: {new_lines:#?}"
    );
}

// Scenario 3 — no host-bind volumes ------------------------------------

#[when(regex = r"^the operator inspects the foundry service definition in `docker-compose\.yml`$")]
async fn inspect_compose_yml(_world: &mut FoundryWorld) {
    // No side effect — scenario 3 is a pure-file inspection.
}

#[then(regex = r"^the foundry service declares zero host-bind mounts under `volumes`$")]
async fn no_host_binds(_world: &mut FoundryWorld) {
    let yml = read_compose_yml().expect("read docker-compose.yml");
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&yml).expect("parse docker-compose.yml");
    let foundry = doc
        .get("services")
        .and_then(|s| s.get("foundry"))
        .expect("services.foundry present");
    if let Some(volumes) = foundry.get("volumes") {
        let list = volumes.as_sequence().cloned().unwrap_or_default();
        for entry in list {
            match entry {
                serde_yaml_ng::Value::String(s) => {
                    let first = s.split(':').next().unwrap_or("");
                    assert!(
                        !(first.starts_with('.') || first.starts_with('/')),
                        "foundry service has a host-bind volume: {s:?}"
                    );
                }
                serde_yaml_ng::Value::Mapping(m) => {
                    if let Some(t) = m.get(serde_yaml_ng::Value::String("type".into())) {
                        assert_ne!(
                            t.as_str(),
                            Some("bind"),
                            "foundry service has a long-form bind volume: {m:?}"
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

#[then(regex = r"^the only persistent volume is a named volume backing postgres$")]
async fn only_named_volume(_world: &mut FoundryWorld) {
    let yml = read_compose_yml().expect("read docker-compose.yml");
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&yml).expect("parse docker-compose.yml");
    let top_volumes = doc.get("volumes");
    let names: Vec<String> = top_volumes
        .and_then(|v| v.as_mapping())
        .map(|m| {
            m.iter()
                .filter_map(|(k, _)| k.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        names.len(),
        1,
        "expected exactly one named volume, got: {names:?}"
    );
    let only = &names[0];
    let postgres = doc
        .get("services")
        .and_then(|s| s.get("postgres"))
        .and_then(|p| p.get("volumes"))
        .and_then(|v| v.as_sequence())
        .expect("postgres.volumes present");
    let backs_postgres = postgres.iter().any(|entry| match entry {
        serde_yaml_ng::Value::String(s) => s.split(':').next() == Some(only.as_str()),
        serde_yaml_ng::Value::Mapping(m) => {
            m.get(serde_yaml_ng::Value::String("source".into()))
                .and_then(|s| s.as_str())
                == Some(only.as_str())
        }
        _ => false,
    });
    assert!(
        backs_postgres,
        "named volume {only:?} does not back the postgres service"
    );
}
