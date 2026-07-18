# DISTILL Acceptance Review — new-issue-dialog-description

## Coverage vs acceptance criteria

| AC | Scenario(s) | Covered |
|----|-------------|---------|
| AC-01.1 (field present, empty) | S1 | ✓ |
| AC-01.2/.3 (persist + OOB card) | S2 | ✓ |
| AC-01.4 (round-trip to edit) | S3 | ✓ |
| AC-01.5 (optional ⇒ "") | S4 | ✓ |
| AC-01.6 (empty title, no row; input kept) | S5 (HTTP) + dogfood #3 (survival) | ✓ (split by harness boundary) |
| AC-01.7 (no-JS fallback, both templates) | S6 | ✓ |
| AC-01.8 (tenancy) | S7 | ✓ |
| AC-02.1/.2 (API create + read-back) | S8 | ✓ |
| AC-02.1 (omit ⇒ back-compat) | S9 | ✓ |
| AC-02.4 (API 422 parity) | S14 | ✓ |
| AC-03.1/.2 (bound on create, no row) | S10 | ✓ |
| AC-03.3 (bound on edit, untouched) | S11 | ✓ |
| AC-03.4 (at-bound accepted) | S12 | ✓ |
| AC-03.5 (chars not bytes) | S13 | ✓ |
| Cross-feature ODD-5 (no create event) | S15 | ✓ |
| Cross-feature (first-edit old_value) | S16 | ✓ |

**Every AC maps to at least one scenario.** No orphan ACs; no scenario without an AC or invariant.

## Wave-decision reconciliation

| DESIGN decision | Reflected in acceptance |
|-----------------|-------------------------|
| ODD-1 bound = 262144 (= DB CHECK) | S12 (262144 accepted) / S10, S11, S14 (262145 refused) — exact boundary |
| ODD-2 serde default | S9 (omit ⇒ 201, "") |
| ODD-3 `description_too_long` copy | S10/S11 (web fragment) + S14 (API 422 same rule) |
| ODD-4 no echo-back handler needed | S5 asserts HTTP + no-row only; survival is dogfood #3 (htmx non-swap) |
| D2 one bound, both paths | S10 (create) + S11 (edit) share the rule; S11 documents the 500→clean-refusal change |
| D4 response contracts unchanged | S2/S4 assert the same OOB card; S8 asserts read-back, not a widened create body |

## Harness-boundary honesty (the load-bearing risk)

The single most important review point: **do not assert a visible browser error message.** Under htmx 2.0.4
the 400/422 is not swapped, so any DOM-message assertion would be red for a reason unrelated to this feature
(the deferred app-wide error-visibility defect). Every error scenario (S5, S10, S11, S14) asserts the **HTTP
response body / status** only. Input-survival (S5's real UX promise) is a **dogfood** item because htmx's
non-swap — not any code this feature writes — is what preserves it, and that is invisible over HTTP. This is
the DISTILL application of the "a green can be an artefact of the instrument" lesson: a scenario that asserted
a visible message would be a false test, and a scenario that asserted survival over HTTP would be green over
nothing.

## Port-to-port check

All scenarios drive a **driving port** (web `GET/POST …/issues[/new,/{n}/edit]` or API `POST /api/v1/…/issues`)
and assert either the HTTP contract or the **driven port** (store `description_md`, `issue_change_events`).
None reaches into an internal component. Hexagonal boundary honored.

## Independence & determinism

- Each scenario seeds its own workspace/member/project via the shared Background (reused board-new-issue
  steps) or an explicit Given; no ordering dependency between scenarios (`GEN-1` is the first key in a fresh
  project each time).
- The 262144/262145 lengths are deterministic; multi-byte content (S13) uses a fixed repeated char.
- API scenarios seed their own machine credential (reused `feature_a_programmatic` helper).

## fail_on_skipped compliance

All scenarios `@pending` (excluded from lanes). `fail_on_skipped()` stays ON: when DELIVER un-@pends a
scenario, a missing step definition FAILS the lane — it cannot pass silently over an undefined step (UI-4).

## Residual notes for DELIVER

- The 7-call-site change to `insert_issue_with_outbox` is mechanical but compile-forcing — see
  `design/wave-decisions.md` §DELIVER note. Re-grep before editing in case new callers landed.
- S11's "was a 500" is a behavior change (DB-CHECK 500 → clean refusal at the same threshold). Confirm the
  edit service validates **before** the read-old→UPDATE tx opens, so nothing is partially written.
- Verdict: **READY for DELIVER.** 16 scenarios + 3 store-integration cases; every AC and cross-feature
  invariant covered; harness boundary explicit.
