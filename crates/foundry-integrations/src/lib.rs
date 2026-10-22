//! foundry-integrations — the pure core of external-integration-plugins
//! (DESIGN DDR-1, DDR-2, ADR-EIP-001): `ChannelKind`, `EventKind`,
//! `ContentLevel`, `ReasonCode`, `DispatchPlan`, `compose`, the per-kind
//! encoders and classifiers, `RetryPolicy`, `RouteStatus`, the plan functions
//! and `judge`.
//!
//! SCAFFOLD: true — created by DISTILL (2026-10-21) ONLY to give the core's
//! property and table tests a home as `#[ignore]`d RED stubs
//! (`tests/core_properties.rs`). There is no production code here; DELIVER
//! slice 01 step 01-04 writes it.

#![forbid(unsafe_code)]
