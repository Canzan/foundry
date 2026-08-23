# Architecture Brief

Product-level SSOT for foundry's architecture. Bootstrapped by the `keycloak-sso`
DESIGN wave (2026-08-21). This repo predates the SSOT model — 43 features carry their
architecture in `docs/feature/<id>/design/` or their own `feature-delta.md`, and no
migration guide exists here. This file therefore starts with the section this wave
owns and grows as later waves land; earlier features are deliberately NOT
retro-summarised, because an invented summary of shipped architecture is worse than
a missing one. Their design documents remain authoritative for their own subjects.

## Application Architecture

foundry is a modular-monolith Rust workspace in a ports-and-adapters shape. Effects
are trait-injected (`Arc<dyn Clock>`, `Notifier`, `Store`), the composition root is
`crates/foundry-app/src/main.rs`, and the driving surface is one axum router assembled
by `build_router`. Dependency direction is enforced twice: an AST source-walk
(`cargo xtask check-arch`) and a cargo-graph ban list (`deny.toml`), both run inside
`cargo xtask ci`.

### Authentication surfaces

foundry authenticates three distinct credential classes, deliberately kept apart:

| Class | Credential | Verified by | Algorithm pin |
|---|---|---|---|
| Human, local | Password (argon2) + `tower-sessions` cookie | `foundry-auth` | n/a |
| Human, federated | Keycloak ID token → linked to an existing `users` row | `foundry-oidc` | RS256 only |
| Machine | Self-issued Ed25519 JWT | `foundry-auth` | EdDSA only |

The separation is enforced at build time, not by convention. `check_jwt_alg_pin`
scans `crates/foundry-auth/src` and fails the build unless every `jsonwebtoken`
`Validation` there pins `algorithms` to EdDSA alone; `check_oidc_alg_pin` does the
symmetric job for `crates/foundry-oidc/src` with RS256. Housing both classes in one
crate would make the two pins inexpressible in one file-scoped rule, so the crate
boundary IS the security boundary. See `adr-oidc-001-crate-placement.md`.

All three converge on one seam: `signin::establish_session` resolves the active
workspace (failing closed when the user belongs to none) and writes
`SessionUser { user_id, workspace_id }`. Everything downstream — the board, `/api/v1`,
SSE, authorship — reads that struct and is indifferent to which credential produced
it. New authentication paths extend this seam; they never mint sessions themselves.

### Federated identity is additive, never a migration

Keycloak sign-in links to a foundry user that already exists, matched on the UNIQUE
`users.email_lower` and gated on the token's `email_verified` claim. It provisions
nothing. The local password path stays permanently available, because Keycloak, LLDAP
and foundry share a cluster and the tracker must open when that cluster is broken.

The same reasoning shapes startup: configuration SHAPE is validated at boot (a partial
config is `health.startup.refused`), but discovery and JWKS are fetched lazily, so an
unreachable Keycloak refuses a sign-in attempt rather than a boot. foundry's readiness
never depends on the identity provider it exists to outlive.
See `adr-oidc-003-lazy-discovery.md`.

### Names are labels; slugs are identity

A project's `name` is a mutable display label; its `slug` (and `key_prefix`,
and every issue key minted from it) is immutable URL identity, minted exactly
once at creation by `foundry_core::slugify` and never derived again. Render
paths take slugs from the validated request path or stored columns — never from
`slugify(name)` at render time (the latent defect the instance-admin-project-rename
wave removed from `build_board_page`). Enforced in `cargo xtask check-arch`:
defining `fn slugify(` under `crates/foundry-app/src` fails the build.
See `adr-project-rename-001-request-slugs-not-derived.md` and
`adr-project-rename-002-rename-write-placement.md`.

### Dialog layers close by one mechanism, many declarative triggers

Dialogs are `div.modal` fragments htmx-swaps into `#modal-root`; "closed" is a
DOM-derived state — the host is empty — never a stored flag. The one close
mechanism is `keyboard.js::closeModal()`, and `Escape` has exactly one owner,
`closeTopLayer()` (BR-4): a second `Escape` listener anywhere would race it and
peel two layers per press. New close affordances therefore never register
listeners — they are attributes. Any element inside `#modal-root` carrying
`data-action="close-modal"` is a close trigger, resolved by one
document-delegated click listener in `keyboard.js` (delegation is the house
idiom because it survives htmx swaps). Adding a close control to a future
dialog is a template-only change, and BR-4 is unviolable by construction of the
pattern. See `adr-modal-close-001-declarative-close-trigger.md`.

### Lanes are per-project data; the lane FK is the no-stranded-card invariant

Board lanes are rows (`lanes`: per-project `slug`, `label`, `position`), not
constants; `issues.state` holds the lane slug and a composite FK
`(project_id, state) → lanes(project_id, slug)` makes "every issue has a lane
its board renders" a schema fact, not a test assertion. Consequences every
future feature inherits: any path that writes `issues.state` must name one of
the project's lanes (validated through the single
`foundry_services::issues::validate_project_lane` seam — the DD10 property);
any operation that removes a lane must settle the fate of its cards in the
same transaction, because the FK blocks the lane delete while cards reference
it; a feature moving issues across projects must move lane membership in the
same statement. Lane slugs are immutable identity, labels mutable display —
the names-are-labels invariant extends to lanes. No adapter may hold a static
lane list (`cargo xtask check-arch` rule; exemptions: the store creation seed
and the `humanize_state` historical-label fallback).
See `adr-board-lane-001-issues-linkage-state-fk.md` and
`adr-board-lane-002-two-fate-delete-transaction.md`.

### Crate graph

```mermaid
graph TB
  app[foundry-app<br/>composition root, HTML handlers]
  api[foundry-api<br/>JSON adapter]
  svc[foundry-services<br/>shared use-case seam]
  oidc[foundry-oidc<br/>OIDC protocol]
  auth[foundry-auth<br/>passwords, HMAC, EdDSA tokens]
  store[foundry-store<br/>sqlx persistence]
  core[foundry-core<br/>domain types]
  app --> api
  app --> svc
  app --> oidc
  app --> auth
  app --> store
  api --> svc
  svc --> store
  oidc --> core
  auth --> core
  store --> core
```

`foundry-oidc` reaches neither `foundry-store` nor `foundry-auth`: it takes protocol
input and returns validated claims. Binding a claim to a user happens in
`foundry-app`, where the tenancy rules already live. `deny.toml` bans `foundry-oidc`
outside `foundry-app` and `foundry-acceptance`, mirroring the existing `foundry-api`
rule.
