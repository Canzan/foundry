# Component Boundaries — board-lane-overflow-menu

Dependency direction is inward only: `foundry-app` → `foundry-services` →
`foundry-store` → Postgres, with `foundry-core` as a leaf of pure functions. No
new crate; no edge reversed.

## §1 Browser — the column header

### §1.1 Markup contract (`partials/board_columns.html`)

Authored **once**; both render paths inherit it (D14 — the partial is shared by
`board.html` and `partials/oob/board_columns_oob.html`, which must stay
byte-identical).

| Selector | Role | Status |
|---|---|---|
| `section.column[data-column="{slug}"]` | Column root; dnd + keyboard-nav contract | **unchanged** — do not touch |
| `section.column > h3` | Lane label; `markComposite()` reads it for `aria-label` | **unchanged** |
| `button[data-action="toggle-lane-menu"][data-lane="{slug}"]` | The `⋯` trigger | **NEW** |
| `[data-lane-menu="{slug}"]` | The menu container, hidden until opened | **NEW** |
| `[data-lane-menu] a\|button` ×4 | Edit / Insert before / Insert after / Delete | **NEW** |
| `button[data-lane-delete="{slug}"]` | The old `×` | **REMOVED** (D3) |

Removing `[data-lane-delete]` premise-breaks two shipped browser scenarios
(`feature_board_lane_management.rs:2407`, used at :2427 and :2565), which must
open the menu first. Deliberate and tracked (D13) — US-BLO-01 owns it.

The four menu items carry the same `hx-get` → `#modal-root` shape the old `×`
carried, so reaching a dialog is unchanged in kind.

### §1.2 `keyboard.js` — what may and may not be added

**MAY:** one new arm in `closeTopLayer()`; two new branches in the **existing**
delegated `click` listener (`keyboard.js:870`); a `menuIsOpen()` predicate that
queries the DOM; a `closeMenu()` that clears the open state and returns focus to
its trigger.

**MUST NOT:** register any listener for `Escape` (BR-4 — `closeTopLayer()` is
the single owner); register a second document `click` listener; store the open
menu in a variable.

The stored-handle prohibition is not style. `#board-columns` is replaced
wholesale by the OOB refresh, so a stored node reference survives as a detached
element: `Escape` would then no-op while a menu is on screen — the precise
failure ADR-003 §2 describes and the reason `modalIsOpen()` asks
`childElementCount` instead of tracking opens.

### §1.3 Arm order

```
help (#kb-overlay-root) → modal (#modal-root) → MENU → search panel → no-op
```

Menu and modal are mutually exclusive (choosing an item closes the menu and
swaps a dialog in), so the ordering is defensive rather than load-bearing — but
it must be deterministic, and it must be covered by the `@layered` scenario
shape that already guards the other three arms.

## §2 `foundry-app` — HTTP adapter

### §2.1 Routes (beside the shipped delete route, same layer stack)

```
GET  /team/{t}/project/{p}/lanes/{lane}/edit            show_edit_lane_dialog
POST /team/{t}/project/{p}/lanes/{lane}/edit            submit_edit_lane
GET  /team/{t}/project/{p}/lanes/{lane}/insert/{side}   show_insert_lane_dialog
POST /team/{t}/project/{p}/lanes/{lane}/insert/{side}   submit_insert_lane
```

Mounted **under** `csrf::csrf_middleware` + `session_layer`, exactly as
`lib.rs:605-608` mounts delete. Refusals on **both** verbs are the uniform
non-enumerable 404 — including an unrecognised `{side}`, which must not be
distinguishable from an unknown lane.

Lane slugs in the path are immutable identity: handlers resolve them against
stored rows and never derive them (`brief.md` §names-are-labels; `fn slugify(`
under `crates/foundry-app/src` is a `check-arch` build failure).

### §2.2 `views.rs`

`BoardColumn` gains four action URLs, built in `board_columns()` from the
**validated path slugs already in scope** — never from `slugify(name)`. This is
the shipped idiom: `board_columns()` already builds `edit_url` and `state_url`
for each `IssueCard` the same way.

Two new view models (`EditLaneModal`, `InsertLaneModal`) follow
`DeleteLaneModal`'s shape: fully resolved, infallible render, `action` + `csrf`
+ a `[data-error-slot]`.

### §2.3 What `foundry-app` must NOT do

Compute a slug; compute a position; decide collision outcomes; hold a lane list.
All four belong below the adapter, and the last is a `check-arch` rule.

## §3 `foundry-services` — use cases

| Function | Returns | Notes |
|---|---|---|
| `edit_lane_dialog` | current label + survivors | mirrors `delete_lane_dialog` |
| `rename_lane` | `Result<(), RenameLaneError>` | label-only |
| `insert_lane_dialog` | anchor lane + side | advisory read |
| `insert_lane` | `Result<(), InsertLaneError>` | wraps the locked store tx |

**One validation seam** serves both writes (Driving Port 3, the DD10 property
that already holds for state normalisation):

- label non-blank after trim, ≤ 64 (bound enforced here, **not** only by the DB CHECK)
- insert only: mint via `foundry_core::lane_slug`; empty result → refuse (D7)
- insert only: slug uniqueness — pre-checked **inside the store's lock**, so the
  operator gets the D7 copy rather than a raw `duplicate key` error

Error → HTTP mapping, matching the shipped delete path:

| Error | Response |
|---|---|
| `NotFound` (lane, project, non-member, signed-out) | uniform 404 |
| `LabelBlank` / `LabelTooLong` / `SlugEmpty` / `SlugTaken` | 422 + bare fragment → `[data-error-slot]` |
| anything else | `internal_error` |

## §4 `foundry-store` — repository

| Function | Shape |
|---|---|
| `rename_lane(project_id, slug, new_label)` | single `UPDATE ... SET label`; no lock |
| `insert_lane_at(project_id, anchor_slug, side, new_slug, new_label)` | **one transaction**: `FOR UPDATE` → resolve anchor by identity → capture position → slug pre-check → shift → insert |

The insert's ordering is not negotiable and each step was proven in the spike
(`architecture-design.md` §1):

- **`FOR UPDATE` first** — without it, concurrent inserts hand the loser a raw duplicate-key error (test 4). With it, both serialize cleanly (test 5b).
- **Anchor by identity, position captured before the shift** — reading the anchor's position *after* shifting is the trap that made the first spike attempt fail, twice.
- **Shift with a plain `UPDATE`** — no `SET CONSTRAINTS`; `DEFERRABLE INITIALLY IMMEDIATE` already defers to end-of-statement (tests 1 vs 2).

## §5 `foundry-core` — pure leaf

`lane_slug(&str) -> String`, sibling to `slugify`. Underscore separator,
`^[a-z]` anchored, `lane_` prefix when normalisation is digit-leading, empty on
no usable characters. Pure, total, no IO — property-testable alongside the
existing `slugify_is_a_fixed_point` / `slugify_output_is_url_safe` properties.

## §6 Test boundaries

| Lane | Owns |
|---|---|
| Unit (`foundry-core`) | `lane_slug` table + properties (fixed-point; output always matches the lane CHECK or is empty) |
| Unit (`foundry-services`) | validation seam; error mapping |
| Integration (`foundry-store`) | the locked insert transaction, **including a concurrent-insert test** — the spike's test 5b becomes a real test |
| HTTP acceptance | status codes, fragments, persistence, uniform 404, CSRF refusal |
| `@needs-browser` | menu open/close, `Escape` through the single owner, focus return, click-outside, error-slot routing, **and menu-open-then-OOB-refresh-then-Escape** (the stored-handle trap) |

The concurrent-insert store test and the OOB-then-Escape browser scenario both
exist because the spike found the failure modes they cover. Neither was in the
DISCUSS acceptance criteria.
