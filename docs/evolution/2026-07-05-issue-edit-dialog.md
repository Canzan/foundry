# Evolution — issue-edit-dialog (edit an issue's title + description from a dialog)

**Finalized**: 2026-07-05
**Commits**: DISCUSS `da1747b` → DESIGN `b356db4` → DISTILL `5c816eb` → DELIVER `9e5484f` → finalize (this).
Trunk-based, no PRs. Repo legacy multi-file convention (no SSOT); DES step-monitoring exempt. Feature dir
PRESERVED.
**Wave coverage**: the FULL pipeline WITH a real DESIGN (unlike the prior two lean features) — DISCUSS (1 story
→ 1 slice, v1 = title + description) → DESIGN (ADR-001 endpoints+swap, ADR-002 store/service+concurrency+outbox;
ODD-1..4 user-ratified) → DISTILL (6-scenario acceptance SSOT + store integration) → DELIVER (1 slice,
net-new backend).
**Scope**: a board issue card was static (only `state` was editable — no title/description update path). This
makes the card **clickable → a pre-filled edit dialog** to edit **title + description** (v1), save, and see the
card update in place — net-new backend, every piece mirroring a shipped pattern, no migration.

## Milestone — the board is editable

Clicking an issue card now opens a centered dialog pre-filled with its title + description; saving persists both
and replaces the card in place without a reload. Combined with `board-new-issue`, the board is now a working
capture-and-edit surface, not a read-only display.

## What shipped (net-new backend, all mirroring shipped patterns; no migration)

- **Store** `update_issue_details(prefix, number, title, description_md, actor_id)` — mirrors
  `update_issue_state_with_outbox` MINUS the outbox emit (ODD-4/ADR-002): lookup by `key_prefix+number` →
  `UPDATE title, description_md, updated_at`; `Ok(None)` when absent; last-write-wins (ODD-3). Plus
  `issue_edit_view` (gated pre-fill read) + `IssueEditRow`.
- **Service** `issue_service::edit_issue_details` — mirrors `change_issue_state`: `resolve_member_project`
  authz → validate title (1–256, non-empty) → store update. Plus `edit_issue_form` (gated read) + `IssueEditView`.
- **Web** — `GET …/issues/{n}/edit` (`show_edit_form`, renders the pre-filled `IssueEditModal`) + `POST
  …/issues/{n}/edit` (`submit_edit`): on htmx success, an OOB `outerHTML` card-replace keyed on
  `data-issue-key` with an empty primary body (so `#modal-root` clears → dialog closes); on non-htmx, `303` →
  board; on empty/oversized title, `bad_request_fragment("Title is required")` in the dialog; foreign/missing →
  uniform `resource_not_found_page` (ADR-003). New `IssueEditModal` view; `IssueCard`/board `issue_card()`
  gained the issue `number`/`edit_url` so the card's `hx-get` builds; the OOB-replaced card re-emits its
  `hx-get` (stays clickable, R2). Two new routes.
- **Reuse**: the board-new-issue `#modal-root` + `.modal`/`.modal-dialog` styling (no second modal system); the
  comment inline-edit flow (as a modal); the create OOB-swap idiom.

## Decisions realized (DESIGN ODD-1..4, ratified)

| # | Decision | Status |
|---|---|---|
| **ODD-1** | GET + **POST** `…/issues/{n}/edit` (POST for the no-JS fallback) | **IMPLEMENTED** |
| **ODD-2** | Save = OOB `outerHTML` card-replace keyed on `data-issue-key` + empty `#modal-root` to close | **IMPLEMENTED** |
| **ODD-3** | Last-write-wins (no `updated_at` guard) | **IMPLEMENTED** |
| **ODD-4** | No outbox/realtime emit in v1 (`update_issue_details`, not `…_with_outbox`) | **IMPLEMENTED** |

## Fidelity notes (honest)

- `update_issue_details` scopes by `key_prefix+number` (NOT workspace), exactly mirroring the shipped
  `update_issue_state_with_outbox`; **tenant safety is enforced at the service `resolve_member_project` gate**.
  The store test pins the deterministic key-scoped isolation (Acme `GEN-1` vs a foreign `FGN-1`); the
  two-workspaces-sharing-`GEN` case is a known limitation of the mirrored pattern, out of v1 scope.
- `actor_id` is threaded for signature parity with the state path (and the deferred edit-broadcast increment)
  but is unused in v1 (`_actor_id`).

## Verification

- **DELIVER**: store test 2/2; `issue-edit-dialog` 6/6 (36 steps); regressions green — `board-new-issue` 5/5,
  `us-08` 10/10, `us-b01` 4/4. fmt + release clippy clean.
- **Finalize**: `cargo xtask ci` all gates green incl. the full `@all` acceptance lane (recorded at finalize).
- **Browser dogfood**: click `GEN-1` → pre-filled dialog → edit title + description → Save → card updates in
  place, dialog closes, no reload; reopening shows the persisted title + description (R2 + persistence).

## Deferred increments (documented in DESIGN/DISCUSS)

Edit **state / priority / assignee** in the same dialog (schema supports all three — one field + its update
path each); markdown preview for the description; **optimistic-concurrency** (`updated_at` guard); **realtime
broadcast** of edits to other board viewers (emit the outbox event + SSE-consumer handling); a close
button / Esc-to-close (the deferred keyboard-interaction layer).
