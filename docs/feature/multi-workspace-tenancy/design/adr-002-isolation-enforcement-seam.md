# ADR-002 — Isolation enforcement seam (where tenant scoping is impossible to forget)

## Status
Proposed.

## Context
The security-critical NFR is that NO read/write escapes the acting workspace (NFR-MWT-SEC-01) and
that scoping happens at a single auditable seam, not re-derived ad hoc in each handler
(NFR-MWT-SEC-06). The risk register's top-rated risk is "a surface forgets to scope by
`workspace_id`, leaking A's data to B." The shipped code already scopes every tenant query by
`workspace_id` and authz lives in `foundry-services` (the boundary guard forbids ad-hoc authz in
the API). The open question is how to make *forgetting to scope* structurally hard, not merely
conventionally avoided.

## Options considered
- **(a) Thread `workspace_id` through every store method (current style), unchanged.** Pro: zero
  new abstraction; matches shipped code exactly. Con: relies on every future author remembering to
  pass the *resolved* workspace and not a parsed one — convention, not enforcement.
- **(b) A scoped-repository/store wrapper bound to the acting workspace** (`Store::for_workspace(id)`
  returning a handle whose methods omit the workspace arg). Pro: can't call a tenant method without
  a workspace. Con: a large refactor of the shipped `Store` surface; risks the dependency-direction
  guard and the "boring monolith" taste; high blast radius for a feature that should be additive.
- **(c) Keep (a)'s explicit threading + introduce a resolved `ActingWorkspace` newtype that handlers
  consume INSTEAD of a parsed `Uuid`, PLUS a NEW `check-arch` AST rule** that flags a tenant-scoped
  store call in an adapter fed anything other than a resolved acting workspace.

## Decision
**(c).** Two cheap, orthogonal mechanisms over the shipped scoping:
1. **`ActingWorkspace(Uuid)` newtype** produced ONLY by the resolution seam (ADR-001). Handlers
   take `ActingWorkspace`, not a `Uuid` parsed from path/query/body. This makes "the workspace came
   from the trusted seam" the only well-typed path — a client-supplied id is a type error at the
   call boundary.
2. **`check-arch` LAYER-1e tenant-scoping rule** (EXTEND `xtask/src/check_arch.rs`). An AST/source
   walk that flags an adapter constructing a tenant-scoped workspace id from request input (e.g. a
   `Uuid::parse` of a path/query param passed into a `*_in_workspace` / workspace-scoped store call)
   rather than from the resolved `ActingWorkspace`. Mirrors the shipped no-mint and ad-hoc-authz
   detectors: one detector function + one acceptance gold test that plants a violation and proves
   the guard bites (Principle 12c self-application). `import-linter`-style import-graph tools were
   considered and rejected: this is a *method-argument-provenance* check, not an import-graph check.

The wrapper (option b) is explicitly rejected as too large a refactor for an additive feature.

## Consequences
- **Positive**: forgetting to scope (or trusting a client-supplied workspace) becomes a build-time
  failure, not a runtime leak — directly answers the top risk and NFR-MWT-SEC-06. Cheap: a newtype
  + one guard rule + gold test, no `Store` refactor. The shipped scoping is reused verbatim.
- **Negative**: the new guard is heuristic (AST, like the shipped detectors) — it can have false
  positives on unusual code shapes; mitigated by the gold-test + the NAMING-the-line discipline the
  existing guard already uses. The `ActingWorkspace` newtype touches handler signatures (mechanical).
- **Risk if rule is too strict**: it could block a legitimate admin/provisioning path that needs a
  literal workspace id — scoped by an allow-list of the provisioning use-case (ADR-004).

## Earned Trust (Principle 12)
The dependency we refuse to take on faith is "every future handler remembers to scope." The probe
is the gold test that plants an un-scoped tenant query and asserts `check-arch` exits non-zero —
the guard that verifies the guard. Wire (the rule) → probe (the gold test) → use (CI lane).

## Slice alignment
The `ActingWorkspace` newtype lands in Slice 1 (walking skeleton). The guard rule is most valuable
once multiple surfaces exist — proposed to land with Slice 2 (web boundary) and extended to cover
the API in Slice 3. Slice 4's adversarial matrix is the behavioral counterpart to the structural guard.
