# ADR-002 — The oracle moves to the browser: a DOM-level `@needs-browser` acceptance gate

**Status**: Accepted (2026-07-18) · **Source**: RCA Root Cause B · **Non-negotiable**

## Context

The invisible-error defect survived because the acceptance oracle asserts the HTTP **response body** (status +
a `div.error` substring), never the **rendered DOM** after the swap (RCA Root Cause B —
`feature_board_new_issue.rs:288-308` and the us-r01/us-r03/issue-edit-dialog/new-issue-dialog-description
equivalents). A correct `400 + fragment` passes those oracles while the browser shows nothing. The fix in
ADR-001 is **client-side JavaScript** — the HTTP lane never runs it, so the HTTP lane *cannot* verify the fix.
A byte-identical server response is returned before and after; only the DOM differs.

## Decision

Every scenario that proves this contract asserts the **rendered DOM in a real browser**, in the shipped
**`@needs-browser`** fantoccini lane (`support/browser_harness.rs`, `world.rs`, `acceptance.rs`; introduced by
keyboard-shortcut-bindings). The canonical scenario shape:

```
Given a member on the board
When they open the new-issue dialog, submit an empty title, and wait for the response
Then the dialog is still open
And the text "Title is required" is VISIBLE in the dialog (present in the rendered DOM, inside the error slot)
And no card was created
```

The HTTP-lane assertions (status + fragment body) are **kept** — they still guard the server contract. What is
**added** is the DOM assertion, in the browser lane, for each form the contract covers. The HTTP lane proves
"the server said the right thing"; the browser lane proves "the user saw it."

## Alternatives considered

- **Keep asserting the HTTP body only** — rejected: this is the exact hole that let the defect ship. A
  client-side fix with only a server-side oracle is untested by construction.
- **Unit-test the JS handler in isolation** — rejected as *sufficient* (the workspace has no JS unit harness,
  per keyboard-shortcut-bindings ADR-007) and insufficient in principle: the bug is the *integration* of htmx +
  the handler + the form markup + the real 4xx response. Only an end-to-end browser drive exercises it. (A JS
  unit harness could be added later but is not on this feature's path.)
- **A single smoke scenario for one form** — rejected as incomplete: each form declares its own slot and could
  regress independently; every covered form gets a DOM-oracle scenario.

## Consequences

- New scenarios are `@needs-browser` — included in `cargo xtask ci`'s `all` lane, excluded from the fast
  default lane (the lane split keyboard-shortcut-bindings established). DELIVER runs them via the fantoccini
  harness with a real chromedriver.
- The browser lane's known operational traps apply (chromedriver version-match preflight, testcontainer
  leakage → `PoolTimedOut`, no-JS flake) — carried from `[[foundry-browser-lane-fantoccini]]`; DELIVER budgets
  for them.
- This is the durable fix for RCA Root Cause B: after this feature, "the acceptance suite is green but the
  browser shows nothing" is no longer possible for a form under the contract.
