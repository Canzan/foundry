# DISCUSS Decisions — issue-edit-dialog

## Key Decisions

- **[D1] Reuse the board-new-issue modal infrastructure**: the edit dialog swaps into the SAME `#modal-root`
  container and uses the SAME `.modal`/`.modal-dialog` overlay styling shipped by `board-new-issue`. No second
  modal system.
- **[D2] Mirror the shipped update + inline-edit patterns**: the backend mirrors `change_issue_state`
  (service authz + resolve_member_project) and `update_issue_state_with_outbox` (store); the interaction
  mirrors the comment inline-edit flow (`show_edit_form` → save → swap-updated-fragment), rendered as a modal
  instead of inline.
- **[D3] Save updates the card in place**: on a successful save the board card is replaced in place (OOB
  `outerHTML` keyed on `data-issue-key`) and the dialog closes by emptying `#modal-root` — the exact close
  mechanism board-new-issue uses. (Precise swap is ODD-2 for DESIGN.)
- **[D4] v1 = title + description only**: state/priority/assignee are deferred increments (schema supports
  them). The dialog is designed to accommodate more fields later without rework.
- **[D5] No migration**: `title` + `description_md` already exist with CHECK bounds; validation mirrors create
  (title 1–256, non-empty).
- **[D6] Net-new backend acknowledged**: unlike board-new-issue (wiring-only), this adds a store method, a
  service method, two handlers, and a view. Hence the full pipeline WITH a real DESIGN wave (endpoint verbs,
  save-swap, concurrency, outbox — ODD-1..4).
- **[D7] Repo multi-file convention**; no SSOT, no migration; DES step-monitoring exempt (lean mode).

## Requirements Summary

- **Primary need**: click an issue card → edit its title + description in a dialog → save → the card updates.
- **Walking skeleton**: the net-new title/description update path (DESIGN designs, DELIVER ships).
- **Feature type**: user-facing (UI + backend), brownfield.

## Constraints Established

- Net-new backend (store + service + handlers + view); no migration.
- Tenancy (ADR-002/003): acting-workspace scoping, uniform non-enumerable 404 for foreign issues.
- CSRF on save; no-JS fallback preserved; reuse the board-new-issue modal infra.

## Scope Assessment: PASS (with a DESIGN-may-split note)

Right-sized as ONE feature; DESIGN may split slice 01 into (backend + store test) then (dialog wiring +
OOB-replace) if the save-swap warrants it. One bounded surface (the board + issue endpoints + store).

## Handoff to DESIGN

DESIGN owns: ODD-1 endpoint verbs (`GET …/issues/{n}/edit` + `POST` vs `PATCH …/issues/{n}`); ODD-2 save
response (OOB card replace + dialog close vs board refresh); ODD-3 concurrency (last-write-wins vs
`updated_at` guard); ODD-4 whether to emit the realtime outbox event on edit now. Plus the exact
`update_issue_details_with_outbox` + `edit_issue_details` signatures and the `IssueEditModal` view shape.

## Upstream Changes

- None. Brownfield increment; requirements grounded in the shipped issue/board/comment code + the schema.
