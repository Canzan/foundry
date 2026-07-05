# DESIGN Decisions — issue-edit-dialog

## Key Decisions (ratified 2026-07-05)
- [D1] GET+POST `…/issues/{n}/edit` (POST for the no-JS fallback). (ADR-001 / ODD-1)
- [D2] Save = OOB `outerHTML` card-replace keyed on `data-issue-key` + empty `#modal-root` to close; no-JS =
  303 to board. (ADR-001 / ODD-2)
- [D3] Last-write-wins; no `updated_at` guard in v1. (ADR-002 / ODD-3)
- [D4] No outbox/realtime emit in v1 (`update_issue_details`, not `…_with_outbox`). (ADR-002 / ODD-4)
- [D5] Reuse the board-new-issue `#modal-root` + `.modal`/`.modal-dialog` styling; mirror the comment
  inline-edit flow (as a modal) + the change_issue_state service shape.
- [D6] Net-new backend: `update_issue_details` (store), `edit_issue_details` (service), `show_edit_form` +
  `submit_edit` (handlers), `IssueEditModal` (view); card-view gains issue `number`. No migration.

## Architecture Summary
Click a card → `hx-get …/issues/{n}/edit` → pre-filled `.modal` dialog in `#modal-root` → Save `hx-post`s →
service (authz + validate) → `update_issue_details` → OOB-replace the card + close. Tenancy (ADR-002/003),
CSRF, and the no-JS fallback all mirror the shipped issue/board paths.

## Constraints for DISTILL/DELIVER
- No migration; last-write-wins; no outbox. Title 1–256. Foreign issue → uniform non-enumerable 404.
- Reuse the board modal infra; do not add a second modal system.

## Handoff
To DISTILL: author the acceptance SSOT from `../discuss/acceptance-criteria.md` (wiring + pre-fill + save +
validation + tenancy + no-JS), all @pending. To DELIVER: ship slice 01.
