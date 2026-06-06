# RED Classification — keyboard-fragments-templating

> Pre-DELIVER fail-for-the-right-reason gate. DELIVER reads this at RED-phase
> entry (ADR-025) to confirm the RED is genuine (`MISSING_FUNCTIONALITY`), not a
> test bug (`IMPORT_ERROR` / `FIXTURE_BROKEN` / `WRONG_ASSERTION`).

Run: `cargo test -p foundry-acceptance --test acceptance` (Docker up, testcontainers PG16).
Result: **174 scenarios — 173 passed, 1 failed** (1470 steps, 1469 passed, 1 failed).

## The one RED scenario

| Scenario | File:line | Classification | Evidence |
|---|---|---|---|
| No inline format!() HTML remains in the keyboard surfaces | `us-k01-keyboard-templating.feature:87` | **MISSING_FUNCTIONALITY** (correct RED) | Step reached its assertion; the assertion fired on the real source-tree count. Panic: `3 keyboard.rs site(s) still emit an inline-format!() BARE-FRAGMENT HTML literal — the move is not complete (goal: 0). Offending sites: keyboard.rs:232, keyboard.rs:245, keyboard.rs:265` |

The failure is NOT:
- `IMPORT_ERROR` — the crate compiles (`cargo check --tests` clean; the binary built and ran 1469 steps).
- `FIXTURE_BROKEN` / `SETUP_FAILURE` — the `When` step ran successfully (`✔ When the keyboard handler source is scanned for inline fragment HTML`); only the `Then` assertion fired.
- `WRONG_ASSERTION` / `OBSERVABLE_NOT_AT_PORT` — the assertion is on a port-exposed observable (the on-disk source-site count, mirroring the established `inline_full_page_sites` / `vendored_htmx_files` source-tree contracts), not an internal struct field.

It flips GREEN exactly when DELIVER removes the 3 inline literals (moves both
fragments to Askama partials). This is the intended driver of the feature.

## The 5 non-RED scenarios in this delta (all GREEN today)

| Scenario | Why GREEN now | After DELIVER |
|---|---|---|
| A search match renders inside the search-results list with a key and a title | byte-identical markup exists today (`ul.search-results`, `span.key`, `span.title`) | stays GREEN (move is byte-stable) |
| A search with no matches renders the empty search-results list | `ul.search-results[data-empty="true"]` exists today | stays GREEN |
| The keyboard-help overlay is a labelled dialog with a heading | `section.keyboard-help[role="dialog"][aria-label]` + `header>h2` exist today | stays GREEN |
| (us-12 regression net — search + help, 6 scenarios) | unchanged production code | stays GREEN (the move's correctness gate) |
| (US-R07 full-page completion guard) | `<!doctype` sites unchanged | stays GREEN (orthogonal) |

These five are **regression-net-only / net-tightening** — they assert the move
preserves the markup. They are deliberately NOT RED: a RED here would mean the
markup is already broken, which it is not. This matches the no-delta rationale
in `coverage-matrix.md`.

## Gate verdict

PASS — exactly one RED scenario, classified `MISSING_FUNCTIONALITY`. No
`IMPORT_ERROR` / `FIXTURE_BROKEN` / `WRONG_ASSERTION`. Handoff to DELIVER unblocked.
