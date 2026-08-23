# Component Boundaries — board-lane-management

Crate/module ownership, port signatures, and markup contracts. Signatures fix the WHAT at each
seam; internal structure (GREEN/REFACTOR) is the software-crafter's.

## 1. Crate ownership map

| Crate | Owns in this feature | Does NOT own |
|---|---|---|
| `foundry-core` | Nothing new. Lane slugs stay plain strings (closed, unmintable set — D9); no newtype until add-lane exists. `slugify` untouched. | — |
| `foundry-store` | New module `lanes.rs`: `LaneRow`, `list_project_lanes`, `delete_lane_with_fate`; lane seeding inside `insert_project` (now transactional); leftmost-lane resolution inside `insert_issue_with_outbox`; migration 0015. | Any HTTP/validation concern |
| `foundry-services` | `issues::validate_project_lane` (DD10 single seam), `board::board_view`, new `lanes.rs` use-case (`delete_lane`, `delete_lane_dialog`) with a pure `classify_lane_delete` heart (the `classify_rename` idiom). | Rendering, status codes, CSRF |
| `foundry-app` | New handler module `lanes.rs` (dialog GET + confirm POST, route wiring in `lib::build_router`); `build_board_page` consumes lanes (deletes `DEFAULT_COLUMNS` + `column_label_to_state`); edit-dialog options loop; report label resolution; templates + `partials/delete_lane_modal.html`. | SQL, business rules |
| `foundry-api` | **No route or wire change.** Behavior shift (per-project 422) arrives through the shared `change_issue_state` seam. | — |
| static JS | **No change.** `board-dnd.js`, `keyboard.js`, `form-errors.js` already generic over `[data-column]` / `[data-action]` / `[data-error-slot]`. | — |

Dependency direction unchanged (`app → svc → store → core`; `api → svc`); no `deny.toml` edit.

## 2. Store ports (`foundry-store`)

```rust
// lanes.rs (new)
pub struct LaneRow {
    pub id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    pub slug: String,
    pub label: String,
    pub position: i32,
}

/// Project's lanes, board order (`ORDER BY position ASC`). Never empty for a
/// live project (creation seeds three; delete refuses the last).
pub async fn list_project_lanes(&self, project_id: uuid::Uuid)
    -> Result<Vec<LaneRow>, StoreError>;

pub enum LaneDeleteFate<'a> {
    MoveTo { destination_slug: &'a str },
    DeleteCards,
}

pub enum LaneDeleteOutcome {
    /// Lane row removed; counts for the response copy / logging.
    Deleted { moved: u64, deleted: u64 },
    /// No such lane in this project (incl. double-submit race) → uniform 404.
    LaneNotFound,
    /// count(lanes) == 1 → 422 "A board needs at least one lane".
    LastLane,
    /// fate=MoveTo and destination absent or == dying lane → 422.
    DestinationNotFound,
}

/// ONE transaction: lock lane → last-lane gate → confirm-time membership
/// (FOR UPDATE, position ASC, number DESC) → fate arm (move: state+position
/// updates + one 0013 status event + one outbox row per card; delete:
/// DELETE ... WHERE id = ANY(ids), delete_issue_cascade shape) → delete lane
/// row (FK strand-guard) → commit. Bounded internal retry (≤3) on
/// foreign_key_violation / deadlock, re-resolving membership each attempt.
/// See data-models.md §5.
pub async fn delete_lane_with_fate(
    &self,
    project_id: uuid::Uuid,
    lane_slug: &str,
    fate: LaneDeleteFate<'_>,
    actor_id: uuid::Uuid,
) -> Result<LaneDeleteOutcome, StoreError>;
```

Changed existing ports:

```rust
// insert_project — signature unchanged; body becomes a transaction that also
// seeds the three default lanes (backlog/Backlog/0, in_progress/In-Progress/1,
// done/Done/2). The seed constant lives HERE (creation template — documented
// exemption to the no-static-lane-list rule; it never renders or validates).
pub async fn insert_project(&self, /* unchanged params */) -> Result<(), ProjectInsertError>;

// insert_issue_with_outbox — return type changes: the landing lane is resolved
// inside the tx (leftmost: ORDER BY position ASC LIMIT 1) and INSERTed
// explicitly (column DEFAULT dropped by 0015). Returns what was persisted.
// Edge case (reviewer iteration 1): if the resolved leftmost lane is deleted
// concurrently (delete_lane_with_fate commits between our read and our
// INSERT's FK check), the INSERT fails with foreign_key_violation — retry the
// transaction once, re-resolving the leftmost (D6 guarantees >=1 lane always
// exists, so the retry cannot starve); a second failure maps to
// IssueInsertError::Store, honest and fully rolled back. Homelab-rare;
// documented rather than engineered further.
pub struct InsertedIssue { pub number: i32, pub state: String }
pub async fn insert_issue_with_outbox(&self, /* unchanged params */)
    -> Result<InsertedIssue, IssueInsertError>;   // was Result<i32, _>
```

## 3. Service ports (`foundry-services`)

```rust
// issues.rs — normalize_state KEEPS its alias folding (in-progress/in_progress,
// canceled/cancelled) but is demoted to a private helper of the new seam:

/// DD10 single seam, per-project: fold aliases, then require membership in the
/// project's lane set. Every write path (HTML dialog, dnd POST, JSON PATCH)
/// calls THIS; unknown lane → ServiceError::Validation { code: "invalid_state" }.
pub async fn validate_project_lane(
    store: &Store,
    project_id: uuid::Uuid,
    input: &str,
) -> Result<String, ServiceError>;

// change_issue_state — signature unchanged; swaps normalize_state for
// validate_project_lane (it already holds `project` from resolve_member_project).
// create_issue — signature unchanged; CreatedIssue.state now carries the
// ACTUAL landing slug from InsertedIssue (ripple surface 6).

// board.rs
pub struct BoardLane { pub slug: String, pub label: String }
pub struct BoardView { pub lanes: Vec<BoardLane>, pub issues: Vec<BoardIssue> }

/// Board read for the HTML adapter: ONE resolve_member_project gate, then
/// lanes (board order) + the same issue rows list_board_issues returns.
/// list_board_issues itself is unchanged (the JSON list endpoint keeps it).
pub async fn board_view(
    store: &Store, principal: &Principal, team_slug: &str, project_slug: &str,
) -> Result<BoardView, ServiceError>;

// issues.rs — edit-dialog pre-fill grows the options source:
pub struct IssueEditView {
    /* existing fields */,
    /// The project's lanes in board order — the Status <select> options (D8).
    pub lanes: Vec<BoardLane>,
}

// lanes.rs (new)
pub enum LaneFate<'a> { Move { destination: &'a str }, Delete }

pub struct LaneDialogView {
    pub lane_slug: String,
    pub lane_label: String,
    pub card_count: i64,          // advisory copy; fate binds at confirm time
    pub survivors: Vec<BoardLane>, // board order; [0] is the picker preselect
}

/// GET arm: authz (resolve_member_project idiom) → lane + live count +
/// survivors. Foreign/absent/non-member → NotFound (handler renders uniform 404).
pub async fn delete_lane_dialog(
    store: &Store, principal: &Principal,
    team_slug: &str, project_slug: &str, lane_slug: &str,
) -> Result<LaneDialogView, DeleteLaneError>;

pub struct DeleteLaneSuccess { pub surviving: Vec<BoardLane>, pub moved: u64, pub deleted: u64 }

pub enum DeleteLaneError {
    /// Foreign/absent lane|project|team, non-member, double-submit → uniform 404.
    NotFound,
    /// Sole remaining lane → 422 "A board needs at least one lane".
    LastLane,
    /// Move destination unknown or == dying lane → 422.
    UnknownDestination,
    Internal,
}

/// POST arm: authz → Store::delete_lane_with_fate → outcome mapping.
pub async fn delete_lane(
    store: &Store, principal: &Principal,
    team_slug: &str, project_slug: &str, lane_slug: &str, fate: LaneFate<'_>,
) -> Result<DeleteLaneSuccess, DeleteLaneError>;

/// PURE heart (classify_rename idiom) — property-testable without a store:
/// given (lane_exists, lane_count, fate, destination ∈ survivors?) decide
/// Proceed{arm}|NotFound|LastLane|UnknownDestination. The async fn is a thin
/// shell: reads → classify → store tx.
fn classify_lane_delete(...) -> Result<LaneDeleteDecision, DeleteLaneError>;
```

## 4. HTTP + template/markup contracts (`foundry-app`) — the acceptance scraper's surface

### Routes (added to `lib::build_router`, inside the session + CSRF middleware stack)

| Method | Path | Handler |
|---|---|---|
| GET | `/team/{team_slug}/project/{project_slug}/lanes/{lane_slug}/delete` | `lanes::show_delete_lane_dialog` — safe read, no `_csrf`; refusals → uniform 404 |
| POST | `/team/{team_slug}/project/{project_slug}/lanes/{lane_slug}/delete` | `lanes::submit_delete_lane` — form `_csrf` + `fate` (+ `destination`); CSRF middleware pre-handler |

Lane slugs in paths are immutable identity (no rename, D9) — consistent with the
slugs-are-identity invariant; handlers resolve them against stored rows, never derive them.

### Board (`board.html` + `views::BoardPage`)

- `views::BoardColumn` unchanged shape (`slug`, `label`, `cards`) — now built from
  `BoardView.lanes`; wrapper `div.board` gains `id="board-columns"` (OOB target).
- Column marker unchanged: `section.column[data-column="{slug}"] > h3` (dnd + keyboard nav
  contract preserved; `keyboard.js` untouched per D8).
- NEW per-column delete affordance inside the header:

```html
<button type="button" class="lane-delete" data-lane-delete="{{ column.slug }}"
        aria-label="Delete lane {{ column.label }}"
        hx-get="/team/{{ team_slug }}/project/{{ project_slug }}/lanes/{{ column.slug }}/delete"
        hx-target="#modal-root" hx-swap="innerHTML">&times;</button>
```

### Delete dialog (`partials/delete_lane_modal.html`, htmx-swapped into `#modal-root`)

```html
<div class="modal" role="dialog" aria-modal="true" data-modal="delete-lane"
     data-lane="{{ lane_slug }}" data-lane-count="{{ card_count }}">
  <div class="modal-dialog">
    <header class="modal-header">
      <h2>Delete lane “{{ lane_label }}”</h2>
      <button type="button" class="modal-close" aria-label="Close"
              data-action="close-modal">&times;</button>   <!-- BR-4: attribute, no listener -->
    </header>
    <form method="post" hx-post="{{ action }}" hx-target="#modal-root" hx-swap="innerHTML">
      <input type="hidden" name="_csrf" value="{{ csrf }}">
      {% if card_count == 0 %}
        <p>It holds no issues. This cannot be undone.</p>
        <button type="submit" name="fate" value="delete">Delete lane</button>
      {% else %}
        <p>It holds {{ card_count }} issue{{s}}.</p>
        <label>Move all {{ card_count }} to
          <select name="destination">
            {% for lane in survivors %}<option value="{{ lane.slug }}"
              {% if loop.first %} selected{% endif %}>{{ lane.label }}</option>{% endfor %}
          </select>
        </label>
        <button type="submit" name="fate" value="move">Move all {{ card_count }}</button>
        <button type="submit" name="fate" value="delete">
          Delete all {{ card_count }} permanently — this cannot be undone</button>
      {% endif %}
      <div data-error-slot></div>   <!-- form-errors.js routes 4xx fragments here -->
    </form>
  </div>
</div>
```

Scraper markers: `[data-modal="delete-lane"]`, `[data-lane]`, `[data-lane-count]`,
`select[name="destination"]` (survivors in board order, leftmost preselected),
`button[name="fate"][value="move"|"delete"]`, `[data-action="close-modal"]`,
`[data-error-slot]`. htmx includes the clicked submitter's `name=value` — verified in the
`@needs-browser` lane (Earned Trust table, architecture-design.md §8).

### Confirm POST responses

- **Success**: body = `<div class="board" id="board-columns" hx-swap-oob="true">…refreshed
  columns…</div>` (OOB replace, house idiom) + empty remainder → primary `#modal-root`
  `innerHTML` swap clears the dialog. No full reload; reload persists (lane data).
- **422** (`LastLane` / `UnknownDestination` / invalid form): bare
  `error_fragment.html` (`fragment_marker: "delete-lane-error"`) → `[data-error-slot]`.
- **404** (authz/absent/double-submit): uniform `resource_not_found_page`.

### Edit dialog (`partials/issue_edit_modal.html`)

Lines 13-18's hardcoded `<option>`s become:

```html
<select name="state">
  {% for lane in lanes %}<option value="{{ lane.slug }}"
    {% if selected_state == lane.slug %} selected{% endif %}>{{ lane.label }}</option>{% endfor %}
</select>
```

`views::IssueEditModal` gains `lanes: Vec<BoardLane>` from `IssueEditView.lanes`.

### Report (`projects.rs`)

`display_value` / `humanize_state` resolution order for `field == "status"`: project lane label
when the slug is a live lane, `humanize_state` fallback for dead/historical slugs
(architecture-design.md §6.5). CSV column contract (`issue,actor,field,old,new,at`, raw slugs)
unchanged.

## 5. What no component may do (negative space)

- No adapter re-acquires a static lane list — enforced by the check-arch rule
  (architecture-design.md §8); the two exemptions are the store creation seed and
  `humanize_state`-as-historical-fallback.
- No new `Escape`/click listeners for the dialog (BR-4); close is attribute-only.
- No lane admin authz axis; no add/rename/reorder port, handler, or template affordance (D9).
- `foundry-api` gains no route; the lane list is not (yet) exposed over JSON — a 422 on an
  unknown lane is the machine client's signal, matching US-BLM-01 scenario 4.
- No artifact outside `docs/feature/board-lane-management/` and
  `docs/product/architecture/`; no roadmap.json (DELIVER owns roadmaps).
