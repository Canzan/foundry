# Requirements — card-ranking-within-status

## Context

The project board (`/team/{team}/project/{project}`) renders four status columns (Backlog / Todo /
In-Progress / Done) and, since `issue-status-move` (2026-07-05), a card can be **moved between** columns by
dragging it or by picking a status in its edit dialog. But **within a column the order is not user-controllable**:
`Store::list_issues_by_project` reads `ORDER BY number DESC` (`lib.rs:1246` — "newest at the top"), and the
drag-and-drop handler always **appends the card to the end** of the target column (`board-dnd.js:79`,
`into.appendChild(card)`). There is **no persisted per-status ordering** — the `issues` table has no
rank/position column (`migrations/0001_init.sql:64`).

`issue-status-move` **explicitly deferred this**: its evolution doc lists "Within-column reorder" as the first
out-of-scope item (`docs/evolution/2026-07-05-issue-status-move.md:66`). This feature delivers it.

There is a coarse `priority` enum (urgent/high/medium/low/no_priority) on each issue, but the board neither sorts
by it nor exposes fine-grained ordering. A **manual rank** is the fine-grained "what I'll pick up next" signal
that priority can't express.

## JTBD (anchor job)

> **When** I look at a status column, **I want** to arrange its cards top-to-bottom by what I'll pick up next —
> by dragging a card to an exact slot — **so I can** make the column itself communicate priority, instead of
> reading in creation order that has nothing to do with what matters now.

Dimensions — **functional**: order work within a status; **emotional**: confidence the board reflects *my* plan,
not an accident of creation time; **social**: a glanceable, honest priority I can share with the team.

## Personas
| ID | Persona | Cares about |
|----|---------|-------------|
| P1 | A member working the board | Drag a card to a precise position in a column — within the same status or into another — and have that order stick for everyone. |

## Scope (v1) — user-confirmed 2026-07-06: BOTH within-status and cross-status positional

- **In scope**:
  - **Slice 1 (within-status reorder)**: drag a card up/down inside its **own** column to an exact slot; the new
    order **persists** and is the order every viewer sees (read path honors rank). Ships the entire rank
    machinery — persisted per-status ordering + a position-carrying persist + an ordered read.
  - **Slice 2 (cross-status positional drop)**: drag a card into a **different** column and drop it at an exact
    slot; **one gesture** sets both its status **and** its rank. Supersedes today's append-to-end drop.
- **Out of scope** (deferred): ranking across projects; auto-sort by the `priority` enum; keyboard/a11y reorder
  (candidate follow-up — see Open decisions ODD-6); touch-drag reordering polish; multi-select drag; the
  `cancelled` state (no column); per-user private ordering (rank is shared/global to the project).

## Brownfield grounding (seams — REUSE / EXTEND; DESIGN owns the rank model)

| Seam | Location | Role |
|------|----------|------|
| Board read (EXTEND) | `Store::list_issues_by_project` (`lib.rs:1238`, `ORDER BY number DESC`) | Must order by the new per-status rank (deterministic tiebreak); today orders by `number DESC`. |
| Issues table (MIGRATE) | `migrations/0001_init.sql:64` — no position column; `idx_issues_project_state` | Needs a persisted rank (column or side table). **First migration since `0011`** → next is `0012`. |
| Drop handler (EXTEND) | `board-dnd.js:63-99` — `drop` → `into.appendChild(card)` + POST `state=slug` to `data-state-url` | Must compute an **insertion index** (drop between neighbours, not always end) and post a position; keep optimistic-move + revert-on-failure; keep the `x-csrf-token` header. Reuse this file — no second JS. |
| State persist path (REUSE / EXTEND) | `POST …/issues/{n}/state` → `change_issue_state` → `update_issue_state_with_outbox` (`lib.rs:1273`) | The cross-status write. Slice 2 must also carry a position; slice 1 (no state change) needs a position write — DESIGN decides one endpoint vs a dedicated `/rank`/`/move` (ODD-2). |
| Realtime (MIRROR) | shipped outbox → SSE; `issue-status-move` broadcasts moves via SSE (ODD-4 realized) | A reorder should broadcast like a move so other viewers converge — DESIGN decides payload/consistency (ODD-4). |
| Cards + columns | `issue_card.html` `[data-issue-key]`, `[data-state-url]`, `draggable`; `board.html` `[data-column]` | Draggable cards + drop targets already exist from `issue-status-move`; the card may need a neighbour handle (`data-issue-key` already present) for a before/after wire format. |

## Constraints

- **New persisted per-status ordering is unavoidable** — this feature **adds a migration** (`0012`) and changes
  the board read from `number DESC` to rank order. This is the key delta from `issue-status-move` ("no
  migration, one persist path"); that constraint does **not** carry over.
- **Migration must backfill deterministically** — seed each issue's initial rank per `(project, state)` from the
  current `number DESC` order, so every existing board keeps its present look on first render (no visible
  shuffle). New-issue default rank must be defined (top of its column, to preserve today's "newest first" feel).
- **Tenancy / CSRF inherited** — the position write scopes by acting workspace and is CSRF-guarded exactly like
  `change_issue_state`; a reorder/drop for a foreign issue → uniform non-enumerable refusal (no 500).
- **Progressive enhancement** — rank is *set* only via the JS drag gesture (there is no natural no-JS reorder
  control, and none is required for v1). But the ranked order is *rendered for everyone*: the read path orders by
  rank, so no-JS viewers see the same order, they simply can't change it.
- **Reuse `board-dnd.js`** — extend the existing drop handler to compute the insertion index; do not add a second
  client JS file. Stay self-contained + CSP-safe (no inline handlers), as shipped.
- **Concurrency / precision** — two members reordering the same column, and long-lived boards doing many moves,
  must not corrupt order or exhaust the rank space; the chosen rank model must state its renumber/rebalance story.

## Open decisions (for DESIGN)

- **ODD-1: Rank representation** — integer rank with gap + renumber-on-collision, vs **fractional/float** rank
  (insert = midpoint), vs a **lexorank/string** rank, vs full **reindex-per-column** on every move. Trade
  precision-exhaustion / renumber cost / concurrency behaviour.
- **ODD-2: Persist path** — extend `POST /issues/{n}/state` to accept an optional position (and treat a
  same-state reorder as a position-only write), vs a **dedicated** `POST /issues/{n}/rank` (or `/move`) carrying
  `{state, position}`. Slice 1 has *no* state change; slice 2 changes state *and* position atomically.
- **ODD-3: Position wire format** — server-computed from **neighbour issue keys** (`after`/`before` the dropped
  slot — robust to races), vs a **client integer index**. `data-issue-key` is already on every card.
- **ODD-4: Realtime** — broadcast the reorder via the shipped outbox/SSE so other viewers converge (payload must
  convey position, not just a card), vs local-to-the-actor for v1. Define the convergence/consistency contract.
- **ODD-5: Migration backfill + new-issue default** — how `0012` seeds rank from the current `number DESC` order
  per `(project, state)`, and where a newly created issue lands (top of its column vs bottom).
- **ODD-6: Accessibility** — is a keyboard reorder path in scope for v1 (drag is mouse/touch only), or deferred?
  Recommended deferred, but DESIGN confirms and records the a11y gap.
