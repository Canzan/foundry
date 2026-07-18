# DESIGN Decisions — form-error-display-contract

## Key decisions

- **[D1] One canonical mechanism: a `htmx:beforeSwap` handler + opt-in per-form error slot** (ADR-001). Reuses
  the shipped `csrf-upload.js` slot idiom + the `board-dnd.js`/`keyboard.js` vanilla-JS house style. No new
  dependency; server responses byte-identical; blast radius bounded by opt-in slots.
- **[D2] The acceptance oracle moves to the browser** (ADR-002). The fix is client-side, so the HTTP lane
  can't verify it; every covered form gets a DOM-level `@needs-browser` scenario asserting the error is
  visible. Closes RCA Root Cause B (green-over-invisible-DOM).
- **[D3] Server unchanged.** Handlers keep returning `400/422 + bare fragment`. No new route/endpoint/migration
  (latest stays `0014`). Every shipped HTTP-lane oracle stays green.
- **[D4] Edges reconciled to the same contract** (drag-revert → OOB `#toast` message; comment-create → htmx +
  slot), rather than left as one-off idioms (RCA Root Cause C).
- **[D5] Corrects `board-new-issue` D3** — the "error swaps in, visible" premise was always false under htmx
  2.0.4. Recorded in `upstream-changes.md`; no shipped doc edited.
- **[D6] Paradigm unchanged**: vanilla-JS client + Rust server; `@nw-software-crafter`.

## Open design decisions (DELIVER to confirm — proposals given)

| # | Question | Proposal |
|---|----------|----------|
| ODD-1 | Slot convention | An empty `<div data-error-slot>` inside the form (generalizes `csrf-upload.js`'s `[data-upload-error]`), with an optional `data-error-target="#sel"` override for forms whose error belongs outside the form element. |
| ODD-2 | Suppress htmx's error event? | Set `isError = false` after retargeting (the error is now handled/displayed); leaves `htmx:responseError` listeners — if any are added later — a clean signal. Low stakes. |
| ODD-3 | Drag-revert message home | Reuse the shipped OOB `#toast` region (ephemeral, matches a transient drag failure) rather than a persistent slot. |
| ODD-4 | Comment-create fix shape | Convert to `hx-post` + `data-error-slot` (joins the contract) vs just styling the bare-page render. Prefer the former for consistency; DELIVER may scope it to slice 03. |

## Requirements summary (from the RCA — no DISCUSS wave)

- **Primary need**: a member who submits an invalid form must SEE why (today the form silently does nothing on
  the htmx path). App-wide across 5 broken forms + 2 edges.
- **Feature type**: user-facing, brownfield, defect remediation (escalated from `/nw-bugfix`).
- **Walking skeleton**: `form-errors.js` + the browser DOM oracle + one form (issue create) showing its error
  end-to-end in a real browser.

## Constraints established

- Server responses byte-identical (`400/422 + fragment`); HTTP-lane oracles preserved; web/API parity intact.
- The handler must NOT hijack the 2xx/OOB success flows, `board-dnd.js`'s revert, or not-found/forbidden
  responses — scoped strictly to elements that declare a slot.
- CSRF surface unchanged (slot-only swap preserves the form's `_csrf`).
- No new route/endpoint/migration/dependency.
- Browser-lane operational traps apply (`[[foundry-browser-lane-fantoccini]]`).

## Existing-system analysis (performed before design)

- Reference patterns read in-tree: `csrf-upload.js` (`showError` → `[data-upload-error]` slot), `board-dnd.js`
  (revert-on-`!ok`, discards body), `base.html` (script-defer loading order), the shipped `@needs-browser`
  fantoccini lane (`browser_harness.rs`).
- Confirmed htmx 2.0.4 default `responseHandling` (4xx → no swap) and the absence of any override/extension.
- Confirmed the HTTP-lane oracles assert response body only (`feature_board_new_issue.rs`).

## Reuse vs new

- **NEW**: `static/js/form-errors.js` (~20 lines) + one `<script defer>` in `base.html` + a `[data-error-slot]`
  in 4 form partials + browser-lane DOM-oracle scenarios.
- **REUSE**: the slot idiom (csrf-upload.js), the vanilla-JS delegated-listener style (keyboard.js/board-dnd.js),
  the OOB `#toast` (notification-preferences-ui), the fantoccini `@needs-browser` lane (keyboard-shortcut-bindings).
- **UNCHANGED**: every server handler, every error fragment, every HTTP-lane assertion.

## Handoff

**To**: DISTILL (browser-lane DOM-oracle scenarios per covered form + the retained HTTP-lane assertions) then
DELIVER. **Deliverables**: this `design/` set + the seeded RCA. Slice plan: 01 (contract + oracle + issue
create), 02 (remaining htmx forms), 03 (edges) — DELIVER may defer 03.
