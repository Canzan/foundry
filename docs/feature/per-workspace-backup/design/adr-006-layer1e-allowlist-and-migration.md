# ADR-006: LAYER-1e (check-arch) exemption + migration posture

## Status
Accepted

## Context
Two structural questions:
1. The shipped `check_arch.rs` LAYER-1e detector (`check_app_tenant_scoping`) flags any
   `foundry-app` handler that scopes a tenant store call by a workspace id PARSED from request input
   (path/query/body) — the "trust a client-supplied workspace" footgun (ADR-002 / NFR-MWT-SEC-06).
   The new export reader scopes by a CLI-supplied workspace id. Does it trip the detector?
2. Does the feature need a database migration?

## Decision
1. **No new allow-list line; keep the export code in `admin_cli.rs` / `foundry-store`.** LAYER-1e
   only walks `crates/foundry-app/src`, and `is_tenant_scoping_allowlisted` ALREADY exempts the
   `admin_cli` file stem (alongside `signin`, `bootstrap`, `session`, `instance_admin`). The export
   reader's workspace id is OPERATOR-TRUSTED (CLI argument, off the bearer surface, host-shell ⇒
   host trust) — categorically different from a request-parsed id. As long as the new code lives in
   `admin_cli.rs` (dispatch + run fns) and `foundry-store` (the scoped reader, which LAYER-1e does
   not scan at all), the guard stays green with NO modification. If a future refactor were to move
   any scoped export call into a NON-allow-listed `foundry-app` file, a new allow-list entry (or
   better, keeping it in admin_cli) would be required — documented here so that move is a conscious
   decision.
2. **NO migration.** The feature is read-only (export + verify): it adds no column, table, index, or
   constraint. No `0012_*.sql` is created. (Confirmed against the read-only business rule and the
   SELECT-only `Store::export_workspace` design, ADR-003.)

## Alternatives Considered
1. **Add a new `export` / `per_workspace_backup` file stem to the LAYER-1e allow-list.** Rejected:
   unnecessary — `admin_cli` is already exempt and is the correct home (mirrors
   `provision-workspace` / `grant-super-admin`, which also handle a workspace id from the operator).
   Adding a stem would widen the allow-list surface for no benefit.
2. **Route the export through a foundry-app web/handler module for symmetry with other features.**
   Rejected: violates NFR-PWB-SURF-01 (off-bearer) and would TRIP LAYER-1e. The operator CLI is the
   trusted surface by design.
3. **Add a migration to record export audit history.** Rejected: out of scope (v1 is export + verify
   only); an audit trail is a possible follow-up and would be ADDITIVE/forward-only when it lands.

## Consequences
- Positive: zero change to the boundary guard; the existing exemption is the correct fit.
- Positive: no migration -> no forward-only-migration risk, no upgrade-safety concern, trivially
  satisfies the read-only / "exporting is not a delete" business rule.
- Negative: the LAYER-1e exemption is by file stem, so the export's tenant-scoping correctness is
  NOT machine-checked by check-arch (it is exempt). Mitigation: the export's isolation correctness is
  guarded instead by the verify-export falsifiability test (AC-02.4) and the OD-PWB-2 gold test —
  the crux is proven behaviorally, which is stronger than the structural LAYER-1e check for this path.
