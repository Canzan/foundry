# Upstream Changes — form-error-display-contract (DESIGN)

## Corrects `board-new-issue` D3 (a false premise, not a code bug)

- **`board-new-issue` `discuss/wave-decisions.md` D3 says**: *"the create response's error swaps INTO the
  modal (replacing its body) — visible, board untouched, no card created."*
- **Reality (RCA-verified)**: htmx 2.0.4 does not swap 4xx bodies, and the app ships no override/extension/
  handler. The error was **never** visible; the modal just stayed as-is with the 400 body discarded. The claim
  was authored assuming a swap-on-4xx behavior htmx does not provide.
- **This feature makes the premise true** by adding the mechanism D3 always assumed (ADR-001) + a DOM-level
  oracle so the assumption can never again pass unverified (ADR-002).
- **Preservation**: no `board-new-issue` document is edited. This section is the record; the correction ships
  as behavior + tests.

## Reframes the deferred item from `new-issue-dialog-description`

- That feature's `upstream-changes.md` §Discovered flagged "app-wide invisible in-browser validation errors"
  and deferred it to its own bugfix. **This is that feature** (escalated via `/nw-bugfix` → design-first).
- When this ships, `new-issue-dialog-description`'s `description_too_long` / `title_required` fragments — which
  already return correctly at 400 — become **visible** for free, with no server change: issue create/edit are
  covered by slices 01–02. No new-issue-dialog-description document is edited.

## Notes the shipped HTTP-lane oracle limitation (not edited here)

- The HTTP-lane error-fragment steps (`feature_board_new_issue.rs` and the us-r01/us-r03/issue-edit-dialog/
  new-issue-dialog-description equivalents) assert the response body only. They are **kept** (they still guard
  the server contract) and **augmented** by the new browser-lane DOM oracle (ADR-002). No existing step is
  weakened or removed.
