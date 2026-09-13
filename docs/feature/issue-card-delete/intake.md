# Intake — issue-card-delete

Produced by `/nw:new`. This is the wizard handoff to DISCUSS, not a requirements
document — DISCUSS owns stories and acceptance criteria.

## Request (verbatim)

> add a card delete feature. I should be able to delete from the popup and the
> full page views

## Wizard outcome

| Question | Answer |
|---|---|
| Feature ID | `issue-card-delete` |
| Change type | New functionality — no issue-delete path exists today |
| Requirements readiness | Clear in the requester's head; nothing written |
| Classification | **Cross-cutting** — UI, `foundry-app` handlers, `foundry-services` seam, `foundry-store`, likely outbox/SSE and change-history |
| Greenfield/brownfield | **Brownfield** — 9 crates, 50+ prior features |
| Starting wave | `/nw:discuss` |

## Stated constraint — carry into DISCUSS

The requester picked **the `comment-edit-delete` approach**. That feature is the
precedent to follow, not merely to consider. Its decisions that plausibly
transfer:

| Precedent | Where | Transfers as |
|---|---|---|
| Soft tombstone: `deleted_at` + `deleted_by`, GC deferred | `comment-edit-delete` ADR-007 (D2) | Issue delete is a soft delete, not a row removal |
| `410 Gone` for tombstoned, `404` for never-existed | `comment-edit-delete` wave-decisions D6 | Status semantics for a deleted issue's detail page |
| CSRF on `DELETE` via `HX-CSRF` header (empty htmx body) | `comment-edit-delete` ADR-009 | The delete POST/DELETE clears `csrf_middleware` unchanged |
| New `event_type` values, `schema_version` stays 1 | `comment-edit-delete` ADR-008 (D3) | Fan-out of the delete to other viewers |
| Forward-only migration; never edit a shipped one | slice-1 ADR-003 | A new migration (current head: 0015) |

DISCUSS should confirm each transfer rather than assume it — an issue is an
aggregate root with comments, attachments, and a change timeline hanging off
it, whereas a comment is a leaf. The cascade question is genuinely open.

## The two surfaces named in the request

| "popup" | `crates/foundry-app/templates/partials/issue_edit_modal.html` — htmx-swapped into `#modal-root` by `issues::show_edit_form` |
| "full page" | `crates/foundry-app/templates/issue.html` — `views::IssuePage`, rendered by `show_issue` |

A third surface exists and is NOT in scope unless DISCUSS says otherwise: the
board card itself (`partials/issue_card.html`). Note the board is where a
deleted card must visibly disappear from, even if the delete is not *initiated*
there.

Confirm-dialog precedent for the UI: `partials/delete_lane_modal.html`
(board-lane-overflow-menu), which also established the `⋯` overflow menu as the
home for destructive lane actions.

## Open questions for DISCUSS

1. What happens to the issue's comments, attachments, and change-history
   timeline on delete? (Aggregate cascade — the leaf-vs-root gap from the
   precedent.)
2. Who may delete — author only, any project member, workspace admin?
   `comment-edit-delete` split author-vs-admin; issues may differ.
3. Is there an undo/restore, or is the tombstone terminal from the user's view?
4. Does the delete emit a change-history entry, and can it, if the issue that
   owns the timeline is itself tombstoned?
5. Where does the affordance live on each surface — inline destructive button,
   or behind a `⋯` overflow menu as lanes do?
6. Board reconciliation: after deleting from the full page, what does an open
   board in another tab do?
