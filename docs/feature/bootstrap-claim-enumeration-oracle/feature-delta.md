# Feature Delta — bootstrap-claim-enumeration-oracle

## Wave: DISTILL

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DISCUSS#US-01 | A colliding-email bootstrap claim is refused byte-identically to a token refusal (never a 500) | n/a | Closes the downstream email-existence enumeration oracle on an already-shipped surface |
| DISCUSS#US-02 | A colliding submit leaves the bootstrap token UNCONSUMED (reusable) | n/a | Fixes the secondary defect — a legitimate typo no longer burns the one-time link |
| DISCUSS#US-03 | Happy path (fresh email → 303 + full seed) and genuine non-23505 errors (→ 500) are unchanged | n/a | Regression-guards the shipped claim flow; keeps the narrow-catch honest |
| DESIGN#D1 | One atomic tx `claim_bootstrap_and_create_workspace` replaces the two-call sequence; token unconsumed on collision | D1 | Store scaffold + enum added; handler rewire deferred to DELIVER (see decision below) |
| DESIGN#D1a | `Refused` and `EmailCollision` both map to the SAME `bootstrap_refusal_page()` | D1a | Enum distinguishes them only for tracing, never the response |

### [REF] Scenario list with tags

Executable SSOT: `crates/foundry-acceptance/tests/features/bootstrap-claim-enumeration-oracle.feature`
(a separate file that does NOT re-assert the us-05 net, mirroring the us-r06 pattern).

| # | Scenario | Story | Tags | RED/GREEN today |
|---|----------|-------|------|-----------------|
| 1 | A fresh-email claim still seeds the workspace and the first instance admin | US-03 | `@real-io @us-03 @regression` | GREEN (guard) |
| 2 | Colliding email, expired token, and unknown token are refused indistinguishably | US-01 | `@real-io @us-01 @error @nfr-sec-01 @security-regression` | RED |
| 3 | A colliding submit leaves the bootstrap token unconsumed | US-02 | `@real-io @us-02 @error` | RED |
| 4 | After a collision the token is reusable with a corrected email | US-02 | `@real-io @us-02 @error` | RED |

Error/edge ratio: 3 of 4 scenarios (75%) — exceeds the 40% floor.

### [REF] Adapter coverage table

| Driven adapter | @real-io scenario | Covered by |
|---|---|---|
| Postgres `bootstrap_tokens` (guarded claim + `used_at`) | YES | Sc 3 (unconsumed query), Sc 2/4 (claim drives the guard) |
| Postgres `users` (`email_lower` UNIQUE 23505 collision) | YES | Sc 2 (collision → refusal), Sc 4 (fresh email → insert succeeds) |
| Postgres `workspaces` / `instance_admins` seed | YES | Sc 1, Sc 4 (workspace + first instance admin) |
| FakeClock (token TTL/expiry) | YES | Background mint + Sc 2 expired-token arm |
| Driving port: HTTP `POST /bootstrap` (real in-proc axum) | YES | all 4 scenarios |

No new driven adapter is introduced (C-2: store method + handler rewire only), so no
new `@adapter-integration` real-IO seam beyond the shipped Postgres one.

### [REF] Scaffolds (RED-ready, `__SCAFFOLD__`)

- `crates/foundry-store/src/lib.rs` — `pub enum BootstrapClaimOutcome { Consumed{..}, Refused, EmailCollision }`
  (models `MemberConsumeOutcome`) + `pub async fn claim_bootstrap_and_create_workspace(...) -> Result<BootstrapClaimOutcome, StoreError>`
  with `unimplemented!(...)` body. Compiles (`pub`, no dead-code lint; clippy `-D warnings` clean).
- `crates/foundry-acceptance/src/steps/feature_bootstrap_enum_oracle.rs` — new step module
  (registered in `src/lib.rs` `mod steps`, force-linked in `tests/acceptance.rs`).

### [REF] Test placement

`crates/foundry-acceptance/tests/features/*.feature` + `src/steps/feature_*.rs` — the
repo's single cucumber-rs acceptance convention (precedent: `feature_member_invites`,
the collision-arm sibling this feature mirrors). Layer 3, example-based (Mandates 9 +
11): real Postgres + real HTTP, no PBT, sad paths enumerated explicitly.

### [REF] Two-tier decision

Tier A only. Tier B (state-machine PBT) is correctly SKIPPED: the journey is 1–2
chained steps per scenario over a config-shaped surface (a single claim POST), not a
≥3-step domain-rich journey. Matches the polyglot matrix (Rust → cucumber-rs,
example-based at this layer).

### [REF] Step vocabulary reuse (Pillars 1 + 2)

New step fns are collision-specific only; the token-refusal vocabulary is reused
verbatim from `us_05_bootstrap.rs` (cucumber matches steps globally): Background mint,
`the admin has already claimed …`, expired/unknown submit, the 3-arm byte-identity +
reveal-nothing Thens, and the dashboard-redirect Then. New: the collision-claim
Given/When, `… remains unconsumed`, the retry When, and `… exists with a first
instance admin`.

### Decision — handler rewire deferred to DELIVER

DISTILL did NOT rewire `POST /bootstrap` (which would break the shipped us-05 net via
the panic scaffold). The new scenarios are RED against the current wrong behavior;
DELIVER implements the tx AND rewires. Full rationale + the NFR-3 narrow-catch
store-test placement: `distill/red-classification.md`.

### [REF] Verification commands

```
# RED (new feature): 4 scenarios (1 passed, 3 failed) — 3 RED = MISSING_FUNCTIONALITY
FOUNDRY_ACCEPTANCE_TAGS=bootstrap-enum-oracle cargo test -p foundry-acceptance --test acceptance
# Regression net intact: 13 scenarios (13 passed)
FOUNDRY_ACCEPTANCE_TAGS=us-05 cargo test -p foundry-acceptance --test acceptance
```
