# Evolution — bootstrap-claim-enumeration-oracle (close the email-existence oracle in the bootstrap claim flow)

**Finalized**: 2026-07-12
**Commits**: DELIVER — `ae28418` (impl + store test + DISTILL spec + docs, `Step-ID: 01-01`), `a0b6ddb` (DES execution log for 01-01).
Trunk-based; repo legacy multi-file convention; feature dir PRESERVED. Not pushed at finalize.
**Wave coverage**: lean **DISCUSS → DESIGN → DISTILL → DELIVER** (DISCUSS/DESIGN authored lean as grounding —
the requirements and the atomic-tx design were settled by the shipped `workspace-member-invites` precedent).
DISTILL authored 4 `@real-io` acceptance scenarios (3 genuine RED verified); DELIVER greened all 4.
**Scope**: the bootstrap claim POST (`crates/foundry-app/src/bootstrap.rs`) already closed the enumeration oracle on
the **token** (unknown / used / expired render one byte-identical refusal). A second, downstream oracle remained: after
the atomic single-use token claim succeeded, `create_initial_workspace` ran, and on an email-uniqueness collision
(`users.email_lower` UNIQUE, SQLSTATE 23505) it surfaced a **distinguishable `500`** — a bootstrap-token holder could
submit different emails and learn which already map to an account, and a legitimate collision both 500'd *and* burned
the consumed token. ZERO new migration, ZERO new crate.

## Milestone — a colliding bootstrap-claim email is refused without leaking that the email exists, and the token survives

Before this feature, a valid-token claim with an already-registered email returned `500 INTERNAL_SERVER_ERROR`
(distinguishable from the `303 → /dashboard` success) and consumed the token. Now the claim path calls one atomic store
method whose email collision rolls the whole transaction back: the response is the **byte-identical**
`bootstrap_refusal_page()` used for unknown/used/expired tokens (status + body), and the bootstrap token stays
**unconsumed** so the operator can retry with a corrected email.

## What shipped

- **`Store::claim_bootstrap_and_create_workspace`** (`crates/foundry-store/src/lib.rs`) — ONE transaction:
  guarded-UPDATE consume of `bootstrap_tokens` (0 rows ⇒ rollback ⇒ `Refused`) → the users INSERT (the collision point,
  caught for SQLSTATE **23505** ALONE ⇒ rollback ⇒ `EmailCollision`, token unconsumed; any other error ⇒ `StoreError`)
  → the shared workspace seed → commit ⇒ `Consumed { workspace_id, user_id }`. Returns the new
  `BootstrapClaimOutcome` enum. Mirrors the shipped, mutation-hardened `create_member_and_consume` idiom.
- **`seed_initial_workspace` private helper** — the six seed INSERTs (workspaces, memberships, teams, projects) +
  the `instance_admins` ON CONFLICT seed, extracted and shared VERBATIM by both `create_initial_workspace`
  (public signature + its ~9 callers UNCHANGED) and the new method. No duplicated SQL, no deletion.
- **Handler rewire** (`bootstrap.rs`) — the POST claim path hashes the password (outside the tx), calls the new method,
  and maps `Consumed → 303 /dashboard` / `Refused | EmailCollision → bootstrap_refusal_page()` (never 500) /
  `Err(StoreError) → 500`. The distinguishing reason is logged via tracing only, never in the response.

## Decisions realized

- **D1 (atomic one-tx, token unconsumed on collision)** — chosen over the smaller "catch-23505-after-consume,
  single-shot" alternative because it also fixes the secondary defect (token burned on a legitimate collision) and
  reuses the proven `create_member_and_consume` shape.
- **Narrow catch** — the 23505 check wraps ONLY the users INSERT; a 23505 from any other statement (duplicate slug /
  key_prefix / instance_admins PK) or an FK/connection error propagates as `StoreError` (500), never mis-mapped to a
  refusal. Pinned by a store-scope unit test and the mutation gate.
- **No deletion of `create_initial_workspace`** — the DISTILL review counted ~9 live callers (mostly tests + the
  provision path); the seed-helper extraction lets both methods share the INSERTs without touching those callers.

## Deviations (recorded honestly)

- **DES audit tooling is broken in this environment**: `des-init-log` / `des-log-phase` / `des-verify-integrity` import
  a `des` module that **nwave-ai 3.15.1 removed**, so they raise `ModuleNotFoundError`. TDD compliance was therefore
  enforced by real RED→GREEN→COMMIT with a `Step-ID: 01-01` git trailer, the acceptance suite, `cargo xtask` gates, and
  store-scope mutation — not by the DES execution-log/integrity layer. An execution-log for 01-01 was recorded
  (`a0b6ddb`) reflecting the actually-executed phases.
- **DISTILL did not rewire the handler** (deferred to DELIVER) to avoid driving the shipped us-05 net into the panic
  scaffold; the 3 new scenarios went RED against the current wrong behavior instead. Resolved here.

## Deferred follow-ups (out of scope, tracked)

- Rate-limiting the bootstrap endpoint (timing side-channels are dominated by the ~constant password-hash cost on all
  paths; byte-level non-enumerability holds regardless).
- The remaining carried backlog: Prometheus `foundry_token_mutations_total` exporter, per-workspace backup (OD-5),
  key-rotation UX, nightly scoped mutation pass on the web adapter.

## Verification

- Store unit tests: **4/4** — Consumed (+ seed rows, token used_at set), EmailCollision (+ token `used_at IS NULL`),
  Refused (unknown token), non-23505 → `StoreError`.
- Acceptance: `bootstrap-enum-oracle` **4/4** (29 steps); `us-05-bootstrap` regression **13/13** (89 steps).
- `cargo fmt --all --check` clean; `cargo clippy --all-targets --release -D warnings` clean; `cargo xtask check-arch`
  PASSED (no new LAYER-1e line).
- Mutation (store-scope, new functions): 3 mutants → **2 caught, 1 unviable, 0 survived** (100% viable-kill);
  the narrow-catch operator mutant (`== "23505"` → `!=`) is caught.
- Reviews: DISTILL — Sentinel APPROVED (9.5/10), Atlas CONDITIONALLY APPROVED (conditions met in DELIVER);
  DELIVER implementation review APPROVED, zero defects.
