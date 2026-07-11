# DISTILL — Acceptance Review: recipient-notification-preferences (v1 = recipient unsubscribe)

> Coverage self-review, harness-boundary rationale, and the one-at-a-time `@pending` strategy.

## Artifacts produced

- `crates/foundry-acceptance/tests/features/recipient-notification-preferences.feature` — 25
  scenarios, all `@pending` per-scenario, per-US tagged.
- `crates/foundry-acceptance/src/steps/feature_recipient_notification_preferences.rs` —
  compiling `panic!` scaffolds (RED-ready, Mandate 7) for every new phrase.
- Registration: `src/lib.rs` (`pub mod feature_recipient_notification_preferences;`) +
  `tests/acceptance.rs` (force-link `use ... as _feature_recipient_unsub;`).
- `distill/{test-scenarios.md, walking-skeleton.md, acceptance-review.md}`.

## Wave-decision reconciliation (HARD GATE) — PASSED

Read DISCUSS `wave-decisions.md` (D1-D11) + DESIGN `wave-decisions.md` (DD1-DD7 + ODD->ADR
map). DEVOPS not run (no `devops/` dir) — consistent with every prior trunk feature; default
infra assumptions apply, no environment matrix needed. DESIGN resolves each DISCUSS ODD
faithfully (token shape, GET-safety, suppression hook + fail-open, table keying, sibling
metric, resubscribe, multi-workspace). **0 contradictions.** No back-propagation required.

## Coverage self-review

- **Every US covered**: US-01 (4, incl. WS) · US-02 (3) · US-03 (5) · US-04 (2) · US-05 (2) ·
  US-06 (3) · US-07 (3), plus R5 fail-open (2) and the ADR-004 workspace-cascade edge (1). 25.
- **Error/edge/security ratio**: 11/25 ≈ 44% (>= 40%).
- **The two provable invariants DESIGN asked DISTILL to guard** are pinned as revert-reds-it
  litmuses: (1) mandatory > unsubscribe — the never-suppress `@property` (#5, #6, #7, #22);
  (2) infallible + await-bounded `notify()` under a failing/slow `SuppressionPolicy`, fail-open
  (#23, #24).
- **Security surface (US-03)** fully covered: tamper -> uniform refusal (#8), no existence leak
  (#9), prefetch-safe GET (#10), CSRF on the public POST (#11), no PII in logs on refusal (#12).
- **PII-free observability (US-07 / NFR-4)**: metric present + split by event (#20), no PII in
  labels/lines (#21), register-at-0 with mandatory series permanently 0 (#22).
- **Backwards-compat (NFR-7)**: empty table -> delivery unchanged (#4).
- **Idempotence (BR-8)**: double-confirm no-op (#3); resubscribe symmetry (#17, #19).

## Business-language purity (Pillar 1)

Scenario titles and Given/When/Then steps use recipient/operator domain language only —
"unsubscribe link", "muted", "suppressed", "confirm", "invitations are stopped". No HTTP
verbs, status codes, table names, or function names leak into the `.feature` file; those live
only in the step module and DELIVER's future bodies.

## Walking Skeleton strategy

The WS strategy is declared in `distill/walking-skeleton.md` (§ Walking Skeleton Strategy):
**Tier A / Strategy A (production composition root)** — real `foundry` app (in-process axum
harness + Postgres testcontainers), real `/unsubscribe` routes, real `StoreSuppression` wired
into the notifier; only the shipped `notify_recorder` transports are doubled. One thin E2E slice
proving token wiring + suppression gate + DB persistence + route dispatch.

## Harness-boundary rationale (why not full in-memory, why not full real-external)

The app + Postgres are REAL (in-process axum harness + testcontainers), so the token, `0014`
table, `SuppressionPolicy`/`StoreSuppression`, the two route surfaces, the emit sites, and the
suppression counter are all exercised through the production composition root — a
Tested-But-Unwired defect is structurally impossible. Only the DELIVERY TRANSPORTS
(SMTP/webhook/email-API/log) are in-process recording doubles (the shipped
`support::notify_recorder`), because (a) they are the driven-external/non-deterministic port
class (Architecture of Reference default = fake with output capture), and (b) a `Then` needs
to read "was this delivered or suppressed?" which the recorder exposes and a real relay does
not. The register-at-0 + bounded-label metric scenarios drive a REAL `foundry` subprocess and
scrape `/metrics`, because the in-process harness installs no Prometheus recorder — the exact
split the predecessor `notification-delivery-providers` established, so DELIVER reuses a proven
seam rather than inventing one. No infra-CI tests (DEVOPS not run) — consistent with prior
features.

## `@property` layer note (Mandate 9 / 11)

These acceptance scenarios run at layer 3 (real app + Postgres / a real subprocess). Per
Mandate 11, layer-3 sad paths are EXAMPLE-based, never PBT-generated. The `@property` tag here
follows the house cucumber-rs convention (as in the predecessor): it marks an example-based
INVARIANT litmus (never-suppress, fail-open, no-PII, register-at-0), not a Hypothesis/proptest
generator. The exhaustive allow-list check (`is_suppressible()` is exactly
`{workspace_invite, member_invite}`) and the bounded-label cardinality guard are unit-level
concerns DELIVER owns in the inner loop (per DESIGN's Architecture Enforcement section).

## One-at-a-time `@pending` strategy (the critical lesson, applied)

- `@pending` is on **every** scenario and is **per-scenario**, never feature-level. A
  feature-level `@pending` would force DELIVER to unskip all 25 at once; per-scenario lets it
  unskip one slice at a time (Outside-In), keeping exactly one RED at a time.
- `acceptance.rs` excludes `@pending` from EVERY lane (`!has("pending")` in the default, `all`,
  and single-tag branches), so this file keeps the `@all` lane GREEN until DELIVER acts. The
  step scaffolds `panic!` (would red the lane if they ran) — but they never run while tagged.
- DELIVER runs a slice with `FOUNDRY_ACCEPTANCE_TAGS=recipient-unsubscribe` (the
  feature-specific tag). Chosen deliberately over `@us-0N` because `@us-01`.. collide across
  features on trunk; `recipient-unsubscribe` selects exactly this feature's scenarios.
- Slice order (each a DELIVER TDD cycle): US-01 walking skeleton -> US-02 mandatory exempt ->
  US-03 non-enumerable/prefetch -> US-04 member_invite -> US-05 status page -> US-06 resubscribe
  -> US-07 observability; the fail-open (R5) litmuses land with US-01's suppression gate and the
  workspace-cascade edge with US-04's completed `0014` surface.

## What DELIVER must know to start slice 01

1. Remove `@pending` from the four `@us-01` scenarios (start with just the `@walking_skeleton`
   one for the tightest RED), run `FOUNDRY_ACCEPTANCE_TAGS=recipient-unsubscribe cargo test -p
   foundry-acceptance --test acceptance`, and confirm each fails for the right reason (the
   step `panic!`s "pending" until the body is written; then a genuine missing-behaviour RED).
2. Build the harness seam first (a `spawn_with_unsubscribe`-style composition root that wires
   `StoreSuppression` + the `/unsubscribe` + `/account/notifications` routes + the shipped
   recording providers), then implement token -> `0014` table -> route -> suppression gate ->
   counter to turn the skeleton GREEN.
3. Do NOT re-author these ATs in RED (ADR-025): DELIVER only unskips + writes PBT unit tests +
   implementation. The `.feature` file is the scenario SSOT.
4. Grounding seams (verified `file:line` in DESIGN `architecture.md`): emit sites
   `bootstrap.rs:266` (`user.workspace_id:226`) + `member_invites.rs:204`; token primitives
   `foundry-auth/src/lib.rs:251,260`; uniform refusal `invites_accept.rs:332-339`; CSRF
   `csrf.rs:137,54`; public route cluster `lib.rs:371-374`; authed neighbour `lib.rs:415-418`;
   `notify()` gate `notify.rs:237`; migration/store pattern `0002_sessions_and_reset.sql:20-28`
   + `store lib.rs:980,930,811`; metric seam `notify.rs:837`.

## Build gate

`PATH=/usr/bin:$PATH cargo test -p foundry-acceptance --no-run` — compile only (`@pending`
scenarios are unimplemented). The `/usr/bin` PATH prefix is REQUIRED (the pyenv shim otherwise
shadows the linker). Result recorded in the DISTILL handoff.
