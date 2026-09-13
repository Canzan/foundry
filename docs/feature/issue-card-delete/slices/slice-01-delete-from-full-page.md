# Slice 01 — Delete an issue from its full page

**Story**: US-ICD-01 | **Estimate**: 1 day | **Depends on**: nothing unshipped

## Goal

An issue can be deleted from its full page, through a confirm dialog that says
what goes with it, taking its comments, attachments and history with it and
nothing else.

## IN scope

- Route pair `GET`+`POST /team/{team}/project/{project}/issues/{n}/delete`, registered beside the shipped `issues/{n}/edit` pair and under the same `csrf_middleware` layer (D4).
- New `partials/delete_issue_modal.html`, mirroring `delete_lane_modal.html`'s contract: bare fragment into `#modal-root`, `[data-action="close-modal"]` as the only close, hidden `_csrf`, `[data-error-slot]`, and the shipped "this cannot be undone" sentence (D3).
- The **delete write port** — one use case behind both surfaces (Driving Port 1/3): team-membership gate, and a result that distinguishes *deleted* from *was not there* so the handler can map the latter to the uniform 404 (DDD-7, DDD-10).
- **NEW `crates/foundry-store/src/issue_delete.rs`** (DDD-1): `delete_issues_with_outbox(tx, ctx, cards)` — the shared primitive — plus `Store::delete_issue_with_outbox`, which owns a transaction for the single-card case. The primitive performs **zero lookups**: it receives a resolved `IssueDeleteContext { workspace_id, project_id, key_prefix }`. Slice 01 lands the primitive with its `DELETE` + cascade behaviour; **slice 03 lights up its outbox emit and its second caller.**
- **Removal** of the dead `Store::delete_issue_cascade` from `attachments.rs` (zero production callers, verified by grep).
- The **dialog read port** (Driving Port 2): issue key plus live comment and attachment counts, advisory only (D13).
- A Delete control in the `issue.html` header beside `<h1>`, with a plain `href` fallback so the whole path works with scripting disabled (D6, AC-1.11).
- Success = redirect to `/team/{t}/project/{p}` (D8).
- Refusals: foreign team, foreign project, absent issue, non-member, signed-out → uniform non-enumerable 404 on **both** verbs; tokenless POST refused by the middleware (D10).
- Acceptance scenarios in the HTTP lane, plus a scripting-disabled scenario for AC-1.11.
- Post-scenario guard query: zero `comments`, `issue_attachments` or `issue_change_events` rows referencing a non-existent issue.

## OUT of scope

- Any change to the edit popup (slice 02) or any outbox/SSE emit (slice 03).
- The board's OOB refresh — this slice's success is a redirect.
- Rewiring `lanes.rs::delete_cards_permanently` onto the primitive (slice 03, DDD-2) — slice 01 leaves the lane fate untouched.
- Bulk delete, keyboard binding, `/api/v1` DELETE, undo.

## Learning hypothesis

**Disproves, if it fails:** that a `&mut Transaction`-taking primitive serves
both the single-card and the batch caller without either bending. DESIGN settled
the shape (DDD-1/DDD-4: one primitive, two transaction owners) but slice 01
builds only the single-card owner, so the batch caller's fit is asserted rather
than demonstrated until slice 03. The specific risk is borrowing: `lanes.rs`
holds `&mut tx` across its `FOR UPDATE` reads and its retry loop, and the
primitive must interleave with that without forcing `delete_lane_with_fate` to
restructure. If it does force a restructure, ADR-BOARD-LANE-002's transaction
shape is in play — which is exactly the thing that ADR promises is unchanged, so
learning it here (one caller, estimate still movable) rather than in slice 03 is
the point of building the primitive first.

The schema-cascade half carries no real risk: `ON DELETE CASCADE` on 0004/0005/
0013 is shipped and already exercised by the lane fate. It is asserted, not
investigated.

**Confirms, if it succeeds:** the primitive is the general shape for every future
issue-removal surface — the popup, a `/api/v1` DELETE, a keyboard binding — and
slices 02 and 03 are pure adapter and fan-out work on a proven seam.

## Acceptance criteria

AC-1.1 … AC-1.12 (see `feature-delta.md` US-ICD-01).

## Production data

Scenarios run against a seeded Backend/auth project with real lane rows and real
issue rows (AUTH-41/42/43), AUTH-42 carrying 3 real comment rows and 1 real
attachment row with bytes — not synthetic fixtures. The orphaned-children guard
query and the shipped zero-laneless guard both run after every mutating scenario.

## Dogfood moment

Same day: delete a genuine duplicate from one of the operator's own boards
through the full page, and confirm from psql that the issue, its comments and
its attachment are gone and that its siblings are untouched.

## Dependencies

None unshipped. Reuses `resolve_member_project`, `find_issue_by_team_project_number`,
`count_comments_for_issue`, `ensure_csrf_cookie`, `csrf_middleware`,
`#modal-root`, the `delete_lane_modal.html` markup contract, the
`new_issue_modal_page.html` no-JS carrier pattern (DDD-6), and the HTTP
acceptance lane.

## Pre-slice SPIKE

**Not required.** Pre-requisite 1 is **settled** — DESIGN chose one shared emit
in a new `issue_delete.rs` (DDD-1/DDD-2, ADR-ISSUE-DELETE-001). Read that ADR
before writing the primitive's signature: it must accept `&mut Transaction` from
the outset, even though this slice's only caller owns its own.
