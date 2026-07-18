# DISTILL Acceptance Review — form-error-display-contract

## Coverage vs the design contract

| Design element | Scenario(s) | Covered |
|----------------|-------------|---------|
| ADR-001 mechanism visible on issue create | S1, S2 | ✓ |
| Slot-only swap preserves `_csrf` (retry works) | S3 | ✓ |
| Blast-radius: handler fires only on 4xx | S4 | ✓ |
| Contract applies to issue edit | S5 | ✓ |
| Contract applies to comment edit | S6 | ✓ |
| Edge: drag-revert message | S7 (deferrable) | ✓ |
| Edge: comment-create not a bare page | S8 (deferrable) | ✓ |
| ADR-002: DOM-level oracle in the browser lane | ALL (every scenario is `@needs-browser`, asserts the DOM) | ✓ |

Every covered form has a DOM-visibility scenario. The 5 fully-broken RCA surfaces map to S1–S6 (create, edit,
comment edit) + the contract covering edit-state/comment-delete by the same handler; the 2 edges map to S7/S8.

## The load-bearing review point (ADR-002)

**No scenario asserts the HTTP response body — that would re-create the exact hole the feature exists to
close.** The shipped HTTP-lane oracles already assert 400/422 + fragment and are RETAINED unchanged in their
own features (they still guard the server contract). This feature's scenarios assert only the **rendered DOM in
a real browser** — the layer that distinguishes "server said the right thing" (already green) from "the user
saw it" (the bug). A scenario asserting the body here would be green against the unfixed app; a scenario
asserting the DOM is red against it (S1/S2 falsification). That asymmetry is the whole design.

## Wave-decision reconciliation

| DESIGN decision | Reflected in acceptance |
|-----------------|-------------------------|
| D1 beforeSwap + opt-in slot | S1–S6 assert the error lands in the slot inside the form/dialog |
| D2 oracle in `@needs-browser` | every scenario is `@needs-browser`, asserts DOM visibility |
| D3 server unchanged | no scenario changes or asserts a server response; the HTTP oracles are untouched |
| D4 edges reconciled | S7 (drag message), S8 (comment-create) |
| Blast-radius guard | S4 (valid submit shows no error) + falsification against an over-broad handler |
| CSRF-safe slot swap | S3 (fix + resubmit succeeds without reload) + falsification against a full-`#modal-root` replace |

## Port-to-port + independence
- Drives the real browser DOM (the user's port); asserts the driven store port (S2 no row, S3 card lands, S5
  issue unchanged). No scenario reaches an internal component.
- Each scenario seeds its own fixture via the shared Background (reused) or an explicit Given; deterministic
  (empty-title is the simplest reliable trigger — the specific 262144/262145 bound values are already covered
  by the retained new-issue-dialog-description HTTP scenarios, not re-litigated here).

## fail_on_skipped compliance
All `@pending`; `fail_on_skipped()` stays ON. When DELIVER un-@pends a scenario, a missing step FAILS the lane.

## Residual notes for DELIVER
- Browser-lane traps (`[[foundry-browser-lane-fantoccini]]`): version-match preflight, `PoolTimedOut` from
  leaked testcontainers, no-JS/timing flake. Use bounded `wait().for_element` on the `form-errors` readiness
  marker + the slot; never sleeps.
- S7 (drag refusal) and S8 (comment-create) need a deterministic server-refusal trigger (a forbidden/stale
  case) — DELIVER to design the fixture; if flaky, keep them slice-03 and defer.
- Verdict: **READY for DELIVER.** 8 browser-lane scenarios across 3 slices; every design element + falsification
  covered; the oracle is correctly at the DOM, not the HTTP body.
