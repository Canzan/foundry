//! external-integration-plugins — the EIP-KPI-4 guardrail bench (DEVOPS
//! "EIP-KPI-4 method", UI-9, gate G3; `eip-kpi4-bench`, manual, one bounded
//! invocation per repeat):
//!
//! `timeout 90m cargo test -p foundry-acceptance --release --test eip_kpi4_bench -- --ignored --nocapture`
//!
//! Pending stub written by DISTILL 2026-10-21 (ADR-025 scaffolded RED). DELIVER
//! slice 01 step 1 (task T-1) replaces it: one release-profile `foundry`
//! subprocess on its own fresh `postgres:16-alpine` (256 MB /dev/shm); arm A no
//! routes, arm B one Slack, one Matrix and one email route with every
//! destination black-holed and the worker active; 50 warm-up calls, then 500
//! issue creates and 500 lane moves per arm at concurrency 1 and 4, arms
//! interleaved in blocks of 50; a 2,000-resample bootstrap 95% interval on the
//! ratio p95(B)/p95(A) with a fixed seed; verdict PASS (upper bound <= 1.10),
//! FAIL (lower bound > 1.10) or INCONCLUSIVE; output
//! `target/eip-kpi4/<sha>.json` with a `verdict` field. T-1 runs arm A only on
//! the unchanged code and records the baseline in `kpi-contracts.yaml`.

#[test]
#[ignore = "pending: external-integration-plugins 01-01 EIP-KPI-4 bench (T-1 baseline, then the guardrail)"]
fn eip_kpi4_bench() {
    panic!(
        "Not yet implemented -- RED scaffold (external-integration-plugins 01-01: EIP-KPI-4 bench)"
    );
}
