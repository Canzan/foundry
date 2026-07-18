# ADR-001 — Canonical htmx error-display: a `beforeSwap` handler + per-form error slot

**Status**: Accepted (2026-07-18) · **Source**: RCA option (d) · **Supersedes**: `board-new-issue` D3 (false premise)

## Context

htmx 2.0.4 does not swap 4xx bodies (`responseHandling` `[45]..` = `swap:false,error:true`), and the app ships
no override/extension/handler — so form validation errors returned as `400/422 + fragment` are discarded by the
browser (RCA Root Cause A). Five htmx forms are affected. The app already contains the fix *pattern* twice: the
`[data-upload-error]` slot in `csrf-upload.js`, and the vanilla-JS document-delegated handler idiom in
`board-dnd.js`/`keyboard.js`. The forms use htmx's own `hx-post`/`hx-patch`, so the fix must hook htmx's
lifecycle rather than replace them with `fetch`.

## Decision

A single global **`htmx:beforeSwap`** listener (`static/js/form-errors.js`, one vanilla IIFE, `<script defer>`
from `base.html`) that, on a `400..=499` response whose triggering element resolves to an error slot, forces
the swap into that slot:

```
evt.detail.shouldSwap = true;
evt.detail.target      = slot;   // data-error-target="#sel", else closest('form') [data-error-slot]
evt.detail.isError     = false;
```

If no slot resolves, the handler returns and htmx's default (no swap) stands. Each htmx form declares an empty
`<div data-error-slot>` (modals: inside `.modal-dialog`). The server keeps returning `400/422 + bare fragment`
**byte-identical** — so every shipped HTTP-lane oracle stays green and the change is client-side only.

This is the documented htmx idiom for handling error responses, and it mirrors the shipped `csrf-upload.js`
slot pattern exactly.

## Alternatives considered

| Option | Why rejected |
|--------|--------------|
| **(a) vendor htmx `response-targets` + `hx-target-4xx`** | Viable and declarative, but adds a vendored dependency (VENDOR.md hash-pinning, supply-chain surface) to replace ~20 lines that reuse an idiom already in the tree. Kept as the acceptable fallback if the custom handler ever proves insufficient. |
| **(b) global `responseHandling`/`beforeSwap` override that swaps ALL 4xx** | HIGH blast radius: would hijack `board-dnd.js`'s revert path, the OOB comment/attachment/state success flows, and not-found/forbidden fragments everywhere. The slot-scoped handler is the same mechanism bounded to opt-in elements. |
| **(c) return 200 + fragment** | Wrong HTTP semantics; breaks every shipped `400/422` oracle (`feature_board_new_issue.rs`, us-r01/us-r03, issue-edit-dialog, new-issue-dialog-description); erodes web/API parity (NFR-WEB-API-CON-02). |

## Consequences

- 2xx/OOB success flows and `#modal-root` close-on-success are untouched — the handler only fires on 4xx.
- CSRF-safer: swapping only the slot preserves the form + its `_csrf` for the retry.
- The 262144-char `description_too_long` message swaps into the slot like any other; bodies over the per-route
  `DefaultBodyLimit` are 413'd before the handler (separate, already-correct path).
- Every form that wants a visible error must **declare a slot** — the opt-in is what keeps the blast radius
  tiny and makes the coverage auditable (a form with no slot = no interception).
