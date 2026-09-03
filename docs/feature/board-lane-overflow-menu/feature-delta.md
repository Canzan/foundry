# Feature Delta — board-lane-overflow-menu

Per-column overflow menu (`⋯`) on the board, carrying **Edit list**, **Insert
list before**, **Insert list after** and **Delete list** — replacing today's
permanently-visible `×` delete button.

Feature type: **cross-cutting** (Askama template + CSS + `keyboard.js` + app
handlers + services use cases + store queries). Predecessor:
`board-lane-management`, whose **D9** pre-registered this feature by name.

## Wave: DISCUSS

### [REF] Prior Wave Consultation

| Source | Read | What it settled |
|---|---|---|
| `docs/product/jobs.yaml` | ✓ | `job-board-lane-shaping` already exists. This feature **extends** that job rather than minting a new one — same job, more verbs. |
| `docs/product/architecture/brief.md` §lanes-are-per-project-data | ✓ | "Lane slugs are immutable identity, labels mutable display." Settles Edit = label-only. Also: no adapter may hold a static lane list (check-arch rule). |
| `docs/product/architecture/brief.md` §dialog-layers | ✓ | BR-4: one close mechanism (`keyboard.js::closeModal()`), `Escape` has exactly one owner (`closeTopLayer()`), new close affordances are **attributes, never listeners**. |
| `docs/product/architecture/brief.md` §names-are-labels | ✓ | `fn slugify(` is banned under `crates/foundry-app/src` by check-arch. Slug minting happens once, at creation, below the adapter. |
| `docs/feature/board-lane-management/feature-delta.md` | ✓ | D1–D11 + Inherited commitments. **D9**: "Adding, renaming, or reordering lanes — flagged as successor feature." This is it. |
| `docs/product/personas/persona-instance-operator.yaml` | ✓ | Priya Raman reused; no new persona minted. |
| `docs/product/journeys/journey-theme-adoption.yaml` | ✓ | Unrelated (theming). No lane journey exists to extend. |
| `docs/product/vision.md`, `docs/project-brief.md`, `docs/stakeholders.yaml` | ⊘ | Not present in this repo. |
| `docs/feature/board-lane-overflow-menu/discover/`, `diverge/` | ⊘ | No DISCOVER or DIVERGE wave ran. Requirements were clear at intake. |

No contradiction with prior evidence: this feature is the successor D9 named.

### [REF] Persona

**Priya Raman — self-hosting operator and team member on her own boards.**
Same persona as `board-lane-management` (`persona-instance-operator`). She has
now used lane deletion for a while and lives with its two consequences: the
board carries a permanently-visible destructive `×` in every column header,
and a lane she deletes cannot be brought back. **Marco** (signed in, not a
member of team Backend) remains the authz foil.

### [REF] JTBD

**job_id: `job-board-lane-shaping`** — *extended*, not replaced.

One-liner: *When my board's lanes have drifted from how the work actually
flows, I want to rename a lane, insert one where it belongs, and delete one —
all from the same unobtrusive control — so the board keeps matching my process
without the only visible affordance being a destructive `×`.*

The shipped job story reads "…and the power to delete a lane". This feature
widens the same job from **delete-only** to **shape**: rename, insert, delete.
`jobs.yaml` is updated in place (widened `job_story`, added forces, second
`validated_by` entry) rather than growing a competing near-duplicate job. All
three stories below trace N:1 to `job-board-lane-shaping`.

### [REF] Locked Decisions

| ID | Decision | Rationale / source |
|---|---|---|
| D1 | **Archive is rejected; the destructive verb stays "Delete".** The reference screenshot shows Trello's Archive-list; foundry has no archive concept anywhere in the domain (every `archiv*` hit under `crates/` is backup-tarball code — `pg_backup.rs`, `verify_export.rs`). Introducing a soft-archived lane state would mean a new column, an un-archive surface, and a second way for a card to be invisible — the exact failure `board-lane-management` D1(b) removed. | User decision at intake, after the option was offered and declined. |
| D2 | **Menu contents are exactly four items:** Edit list · Insert list before · Insert list after · Delete list. The screenshot's "Archive cards", "Archive list" and "Sort by" are out of scope — the first two are archive (D1), the third is a new ordering concept with no backend. | User decision at intake. |
| D3 | **`⋯` replaces `×` outright.** The column header carries one trigger, not two. The `×` is not kept as a shortcut: keeping it would leave the destructive action one misclick from the header while the menu exists, which is the specific thing this feature removes. | Feature request verbatim ("uses ⋯ instead of X"). |
| D4 | **Edit list renames `label` only; `slug` is frozen.** Not a choice made here — an inherited invariant. `issues.state` holds the lane **slug** under composite FK `fk_issues_lane (project_id, state) → lanes (project_id, slug)`; a slug rename would rewrite every issue row in the lane under that FK. The board renders `label`, so a rename is fully visible without touching identity. | `brief.md` §lanes: *"Lane slugs are immutable identity, labels mutable display — the names-are-labels invariant extends to lanes."* `0015_project_lanes.sql:12-23`. |
| D5 | **Insert asks for the name in a dialog** in `#modal-root` — one text field plus a create action — reusing the delete dialog's shipped frame (CSRF `_csrf`, `[data-error-slot]` 422 routing, declarative `data-action="close-modal"`). No inline-editable placeholder lane: that is a new interaction pattern with no precedent in foundry and needs an abandon rule for a lane never named. | User decision; `delete_lane_modal.html` precedent; `form-errors.js` contract. |
| D6 | **Lane slugs are NOT minted by `foundry_core::slugify`.** That function emits hyphens (`slugify("Auth v2") == "auth-v2"`, `foundry-core/src/lib.rs:300`) but the lane CHECK is `^[a-z][a-z0-9_]*$` — underscores only, must start with a letter. The shipped lane is `in_progress`, never `in-progress`. Insert therefore needs a lane-slug mint (underscore-separated, letter-anchored). DISCUSS pins the observable rule; DESIGN owns placement — but it must live below `crates/foundry-app/src`, where `fn slugify(` is a check-arch build failure. | `foundry-core/src/lib.rs:270-303`; `0015_project_lanes.sql:16`; `brief.md` §names-are-labels. |
| D7 | **Slug collisions and empty normalisations are refused inline, never auto-suffixed.** A second lane named "Done" on a board that has one → 422 into `[data-error-slot]` with a reason naming the conflict. A name normalising to nothing ("`...`", "`!!!`", "`  `") → 422 asking for letters or numbers. No `done_2` auto-suffixing: slugs are immutable identity (D4), so a silently-suffixed slug is a permanent artifact that drifts from its label forever. | Refusal idiom matches the shipped last-lane refusal (`LAST_LANE_MESSAGE`); `brief.md` §names-are-labels. |
| D8 | **Insert defers position shuffling to the deferred-constraint window, not a migration.** `lanes` carries `UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE` (`0015:22`). A naive `UPDATE lanes SET position = position + 1 WHERE position >= n` risks a mid-statement violation under `IMMEDIATE`; the constraint was declared `DEFERRABLE` precisely so a transaction can `SET CONSTRAINTS ... DEFERRED` and shuffle freely. **No schema change is expected; the migration counter should stay at 0015.** DESIGN confirms the mechanism and MUST prove it against live-shaped data before relying on it. | `0015_project_lanes.sql:22`; PostgreSQL deferrable-unique semantics. Flagged as this feature's single highest-uncertainty point. |
| D9 | **The menu is a dismissible layer that does NOT register its own `Escape` listener.** BR-4 is unviolable by construction: `Escape` has exactly one owner (`closeTopLayer()`), and a second listener would peel two layers per press. The menu must therefore either live inside the existing layer mechanism or extend `closeTopLayer()`'s notion of "top layer" — one owner either way. DISCUSS pins the observable requirement (one `Escape` closes the menu and leaves the board untouched; a second does nothing); DESIGN owns the mechanism. | `brief.md` §dialog-layers; `adr-modal-close-001-declarative-close-trigger.md`. |
| D10 | **Full menu semantics for keyboard and AT.** Menu button pattern: the `⋯` control opens the menu, focus moves into it, items are reachable without a pointer, `Escape` closes and returns focus to the `⋯` button. The board is already a `listbox` with one deliberate tab stop and a documented ARIA posture (ADR-006, `keyboard.js::markComposite`); a pointer-only menu would regress against that posture in the same region. | User decision at intake; ADR-006; `keyboard.js:635-660`. |
| D11 | **Authz and CSRF unchanged.** Every menu action is a board mutation gated by team membership; outsiders and the signed-out get the uniform non-enumerable 404 on both verbs. Menu-opening is client-side only (no request). The Edit and Insert dialogs are safe GETs; their confirms are `_csrf`-bearing POSTs. No new role axis. | `board-lane-management` D10, carried verbatim. |
| D12 | **No walking skeleton.** The predecessor's WS existed to swap the lane enum→data foundation under every board read/write. That foundation is shipped: `lanes` rows, `validate_project_lane`, the composite FK, the check-arch no-static-list rule. Every story here is UI plus one write port on top of shipped substrate. Slice 01 is still thin and end-to-end, but it is not a skeleton. | `board-lane-management` D11/DELIVER Inherited commitments; nothing in this feature shifts a foundation. |
| D13 | **Two shipped browser scenarios premise-break by design.** `feature_board_lane_management.rs:2407` clicks `button[data-lane-delete="{slug}"]` directly (used at :2427 and :2565). Once Delete moves behind the menu, that click must first open the menu. This is a deliberate, tracked premise change — not a regression — and US-BLO-01 owns re-premising it. | `feature_board_lane_management.rs:2405-2427, 2565`. |
| D14 | **`board_columns.html` changes land in both render paths at once.** The partial is shared by `board.html` (full page) and `partials/oob/board_columns_oob.html` (the lane-delete confirm's out-of-band refresh), and the two MUST stay byte-identical. Any menu markup is authored once in the partial. | `board_columns.html:1-8`; `board.html:7-11`. |

### [REF] Journey (lightweight, happy path)

Emotional arc: **Latent unease → deliberate control.** A permanently-armed `×`
in every column header (mild background anxiety) → an unobtrusive `⋯` that
asks what she wants (neutral) → the board reshaped without ever having been
one misclick from destruction (confidence).

```text
[Trigger]                    [Step 1]                      [Step 2]                        [Goal]
"Homelab Ops" grew a    →    Priya clicks ⋯ on the    →    Menu: Edit list /          →    Board reads Backlog,
"Staging" stage between      In-Progress column            Insert list before /             Staging, In-Progress,
Backlog and In-Progress.     header                        Insert list after /              Done. No card moved.
The board has no lane        Feels: neutral —              Delete list                      Feels: in control
for it, and the only         the control asks,             Picks "Insert list before",      Sees: her flow, and a
header control is a ×.       it does not threaten          types "Staging", creates         header that is not armed
Feels: uneasy                Sees: ⋯, no armed ×           Feels: deliberate
```

Error paths: name collides with an existing lane → 422 inline, dialog stays
open, nothing created. Name normalises to empty → 422 inline. Non-member POST
→ uniform 404. `Escape` at the menu → menu closes, focus returns to `⋯`,
board unchanged. `Escape` at a dialog → dialog closes, nothing written.

### [REF] Scope Assessment: PASS — 3 stories, 1 bounded context, ~3 days

Signals checked: **3 stories** (≤10, no fire) | **one bounded context** — the
board surface (`foundry-app` board/lanes + the `foundry-services`/`foundry-store`
lane seam), no second context (no fire) | **no walking skeleton** (D12), so the
>5-integration-point signal cannot fire | **~3 days** total (≪2 weeks, no fire)
| **one user outcome** — the board keeps matching the operator's flow; the
three verbs are not independently shippable value (a menu with one item, or a
rename with no way to reach it, is half a thing) (no fire).

Zero oversized signals fired (threshold is 2+). No split proposed.

Three distinct technologies are touched (Rust/Askama, browser JS/CSS, SQL) —
this fires the `ask-intelligent` cross-context trigger, surfaced at wave end,
but it is not an oversizing signal.

### [REF] Shared Artifacts

| Artifact | Source of truth | Consumers | Risk |
|---|---|---|---|
| Project lane set (label, slug, position) | `lanes` rows — already the SSOT since 0015 | Board columns, `⋯` menu render, dnd `data-column`, edit-dialog Status options, `/api/v1` validation, report labels, delete-dialog picker, **insert-position arithmetic (new)** | HIGH — the check-arch no-static-list rule makes a regression a build failure, but new insert/rename write paths must read lane rows, never a cached list |
| Lane `slug` | Minted once at lane creation; immutable thereafter (D4/D6) | `issues.state` values, `data-column`, dnd POST body, API `state`, 0013 event values, `data-lane-*` menu attributes | HIGH — rename must provably not touch it; insert must mint one that satisfies `^[a-z][a-z0-9_]*$` and is unique per project |
| Lane `position` | `lanes.position`, `UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE` | Board column order, insert-before/after arithmetic | HIGH — the single highest-uncertainty point (D8); a failed shuffle either errors or silently reorders a board |
| Open-menu identity ("which column's menu is open") | Client-side DOM state only — no server round-trip, no stored flag | `keyboard.js` menu open/close, focus return target | MEDIUM — must be DOM-derived like `#modal-root` emptiness (`brief.md` §dialog-layers), never a stored flag that can desync across htmx swaps |
| `Escape` ownership | `keyboard.js::closeTopLayer()` — exactly one owner (BR-4) | Menu dismiss, dialog dismiss | HIGH — a second listener peels two layers per press; unviolable by construction only if the menu uses the existing mechanism (D9) |
| CSRF token | `foundry_csrf` cookie + hidden `_csrf` | Edit-confirm POST, Insert-confirm POST, Delete-confirm POST (shipped) | HIGH — a missing `_csrf` is a silent 403, the exact defect `fix-comment-delete-csrf` shipped to close |
| Content-hashed stylesheet | `static/css/foundry.<hash>.css`, referenced by `base.html`, recorded in `static/VENDOR.md` | Menu styling | MEDIUM — menu CSS requires the re-hash procedure; a stale hash row has bitten this repo before |

### [REF] User Stories

#### US-BLO-01: The column header stops being armed — one `⋯` menu, Delete inside it

##### Elevator Pitch

- **Before:** every column header on every board carries a permanently-visible `×` whose single click opens the destructive delete-lane dialog; there is nowhere to put any other lane operation.
- **After:** clicking `⋯` on the "Homelab Ops" In-Progress column header opens a menu listing **Edit list · Insert list before · Insert list after · Delete list**; choosing Delete list opens the same shipped delete dialog, unchanged.
- **Decision enabled:** which lane operation she actually wants — chosen from a list, rather than being offered only the irreversible one.

##### Problem

Priya's board arms a destructive control in every column header, permanently.
Nothing else about a lane is reachable from the board at all: the affordance
budget is spent entirely on delete. She wants the header to ask what she
wants, not to threaten.

##### Domain Examples

1. **Happy path** — "Homelab Ops" (OPS) renders Backlog, In-Progress, Done. Priya clicks `⋯` on In-Progress: the menu lists exactly the four items. She picks Delete list; the shipped dialog opens naming In-Progress and its live card count. Nothing about the delete behaviour changed.
2. **Edge (dismissal)** — With the In-Progress menu open, Priya presses `Escape`. The menu closes, focus returns to the `⋯` button, and the board is byte-identical to before she opened it. A second `Escape` does nothing (no dialog is open, and only one layer peels per press).
3. **Error/boundary (authz)** — Marco, signed in but not a member of team Backend, sends the lane-delete confirm for a lane on "Homelab Ops" directly. He gets the uniform non-enumerable 404, byte-identical to a never-existed path. The menu is client-side and grants him nothing.

##### UAT Scenarios (BDD)

###### Scenario: The column header offers a menu, not an armed delete
- Given the "Homelab Ops" board renders Backlog, In-Progress and Done
- When Priya opens the board
- Then each column header carries one `⋯` menu trigger
- And no column header carries a `×` delete control

###### Scenario: The menu lists exactly the four lane operations
- Given the "Homelab Ops" board is open
- When Priya activates the `⋯` trigger on the In-Progress column
- Then the menu lists exactly: Edit list, Insert list before, Insert list after, Delete list

###### Scenario: Delete list reaches the shipped dialog unchanged
- Given the In-Progress lane on "Homelab Ops" holds 2 issues
- When Priya activates `⋯` on In-Progress and chooses Delete list
- Then the delete-lane dialog opens naming In-Progress and its count of 2
- And the dialog offers the same move-or-delete fate it offered before this feature

###### Scenario: Escape closes the menu and returns focus, changing nothing
- Given the In-Progress column's menu is open on "Homelab Ops"
- When Priya presses Escape
- Then the menu closes and focus returns to the In-Progress `⋯` trigger
- And the board renders exactly as it did before the menu was opened

###### Scenario: The menu is reachable and operable without a pointer
- Given the "Homelab Ops" board is open
- When Priya reaches the In-Progress `⋯` trigger by keyboard and activates it
- Then the menu opens and its items are reachable by keyboard in listed order

###### Scenario: A non-member gets the uniform 404 on the lane-delete confirm
- Given Marco is signed in and is not a member of team Backend
- When Marco sends the lane-delete confirm for In-Progress on "Homelab Ops" directly
- Then the response is the uniform non-enumerable 404 and the lane is unchanged

##### Acceptance Criteria

- AC-1.1 — Every rendered column header carries exactly one menu trigger and zero `×` delete controls (D3). Verified in both render paths, since `board_columns.html` is shared (D14).
- AC-1.2 — The menu's items are exactly the four of D2, in that order, rendered from the column's lane data (never a static list — check-arch rule).
- AC-1.3 — Delete list reaches the existing `show_delete_lane_dialog` GET; the dialog's copy, fate arms and confirm POST are unchanged from `board-lane-management`.
- AC-1.4 — `Escape` closes the menu, returns focus to the trigger, and leaves the board unchanged. Exactly one layer peels per press; `keyboard.js` registers **no** second `Escape` listener (D9, BR-4).
- AC-1.5 — The trigger is keyboard-reachable and the menu's items are keyboard-operable, with focus returning to the trigger on close (D10).
- AC-1.6 — Authz and CSRF behaviour on the delete path is byte-identical to before (D11); a non-member's confirm POST is indistinguishable from a never-existed path.
- AC-1.7 — The two shipped browser scenarios that click `button[data-lane-delete]` directly are re-premised to open the menu first, and are green (D13).

**Estimate: 1 day.** Dependencies: none (sits on shipped substrate).

---

#### US-BLO-02: Rename a lane whose name has drifted, without touching a single card

##### Elevator Pitch

- **Before:** a lane's label is fixed at creation; correcting "In-Progress" to "Doing" is impossible from the board, and impossible anywhere else short of SQL.
- **After:** `⋯ → Edit list` on the In-Progress column opens a dialog with the current name; typing "Doing" and saving re-renders the column as **Doing** with all its cards in place.
- **Decision enabled:** whether a lane's name still describes the stage — and correcting it the moment the answer is no, instead of tolerating a stale board.

##### Problem

Priya's "Homelab Ops" board has a lane labelled "In-Progress" that the team
now calls "Doing" in standup. The board and the conversation have drifted
apart, and the only fix available is a production SQL session — the same trap
`instance-admin-project-rename` removed for project names, still present for
lanes.

##### Domain Examples

1. **Happy path** — "Homelab Ops" In-Progress holds OPS-3 and OPS-7. Priya renames it to "Doing". The column header reads Doing; OPS-3 and OPS-7 sit in the same column at the same positions; their issue keys and URLs are unchanged.
2. **Edge (identity untouched)** — After the rename, `issues.state` for OPS-3 still reads `in_progress`, the column's `data-column` attribute still reads `in_progress`, dragging OPS-3 still targets `in_progress`, and the `/api/v1` PATCH accepting `"in_progress"` still succeeds. Only the displayed label moved.
3. **Error/boundary (label bounds)** — Priya submits an empty name, and a 65-character name. Both are refused inline; the lane keeps its current label. (`lanes.label` is `CHECK (length(label) BETWEEN 1 AND 64)`.)

##### UAT Scenarios (BDD)

###### Scenario: Renaming a lane changes the header and nothing else
- Given the In-Progress lane on "Homelab Ops" holds OPS-3 and OPS-7
- When Priya opens `⋯ → Edit list` on In-Progress and saves the name "Doing"
- Then the column header reads Doing
- And OPS-3 and OPS-7 sit in that same column at the same positions

###### Scenario: A rename never touches lane identity
- Given the In-Progress lane on "Homelab Ops" has been renamed to Doing
- When Priya drags OPS-3 within the board and a machine client PATCHes OPS-7 to "in_progress"
- Then both succeed against the lane slug in_progress
- And every issue key and card URL is unchanged

###### Scenario: The edit dialog opens showing the lane's current name
- Given the "Homelab Ops" board renders a lane labelled Doing
- When Priya opens `⋯ → Edit list` on that column
- Then the dialog's name field contains Doing

###### Scenario: An empty or over-long name is refused inline
- Given the edit dialog for the Doing lane is open
- When Priya submits an empty name
- Then the refusal reason renders in the dialog and the lane is still labelled Doing

###### Scenario: A non-member cannot rename a lane
- Given Marco is signed in and is not a member of team Backend
- When Marco sends the lane-rename confirm for a lane on "Homelab Ops" directly
- Then the response is the uniform non-enumerable 404 and the label is unchanged

##### Acceptance Criteria

- AC-2.1 — `⋯ → Edit list` opens a dialog in `#modal-root` pre-filled with the lane's current `label`, closable by the declarative `data-action="close-modal"` trigger and by `Escape` through the single BR-4 owner.
- AC-2.2 — Saving updates `lanes.label` only. `lanes.slug`, `lanes.position`, every `issues.state`, every issue key and every card URL are provably unchanged (D4).
- AC-2.3 — The renamed label appears in every label-consuming surface that reads lane rows: board header, delete-dialog copy and destination picker, edit-dialog Status options.
- AC-2.4 — An empty label, a whitespace-only label, and a label longer than 64 characters are each refused with a reason routed to `[data-error-slot]`; the lane is unchanged. The bound is enforced below the adapter, not only by the DB CHECK.
- AC-2.5 — The confirm is a `_csrf`-bearing POST; a tokenless POST is refused before the handler runs. A non-member's GET and POST are both the uniform 404 (D11).
- AC-2.6 — Renaming a lane to a label another lane already uses is **permitted** — labels are display, not identity (D4); only slugs are unique. (Contrast US-BLO-03 AC-3.5, where a *new* lane's derived slug must be unique.)

**Estimate: 1 day.** Dependencies: US-BLO-01 (the menu is the only entry point).

---

#### US-BLO-03: Insert a lane exactly where the stage belongs

##### Elevator Pitch

- **Before:** lanes can only be deleted. A board that over-trimmed, or whose workflow grew a stage, cannot get a lane back — "This cannot be undone" is literal.
- **After:** `⋯ → Insert list before` on the In-Progress column opens a dialog; typing "Staging" and creating renders a new **Staging** column between Backlog and In-Progress, empty, with every existing card untouched.
- **Decision enabled:** whether to model a real workflow stage on the board at all — a decision that was previously one-way and therefore not worth making.

##### Problem

Deletion without insertion is one-way. Priya trimmed "Homelab Ops" to three
lanes months ago; the work has since grown a review stage, and the board
cannot represent it. The lane set is now a ratchet that only tightens.

##### Domain Examples

1. **Happy path (before)** — "Homelab Ops" renders Backlog, In-Progress, Done. `⋯ → Insert list before` on In-Progress, named "Staging", yields Backlog, **Staging**, In-Progress, Done. Staging is empty; OPS-3, OPS-7 and OPS-9 have not moved.
2. **Edge (after, at the end)** — `⋯ → Insert list after` on the last column (Done) named "Archive Box" appends a fourth lane at the far right. Positions stay contiguous, and the new-issue landing rule still names the leftmost lane (Backlog), unchanged.
3. **Error/boundary (collision and empty)** — On a board that already has a Done lane, inserting a lane named "Done" is refused inline naming the conflict; nothing is created. A lane named "`...`" is refused inline asking for letters or numbers. In both cases the dialog stays open with the typed text.

##### UAT Scenarios (BDD)

###### Scenario: Inserting before a lane places the new lane immediately to its left
- Given the "Homelab Ops" board renders Backlog, In-Progress, Done
- When Priya opens `⋯ → Insert list before` on In-Progress and creates a lane named "Staging"
- Then the board renders Backlog, Staging, In-Progress, Done in that order
- And the Staging column is empty and no existing card has moved

###### Scenario: Inserting after the last lane appends at the far right
- Given the "Homelab Ops" board renders Backlog, In-Progress, Done
- When Priya opens `⋯ → Insert list after` on Done and creates a lane named "Archive Box"
- Then the board renders Backlog, In-Progress, Done, Archive Box in that order
- And new issues still land in Backlog

###### Scenario: An inserted lane is a fully working lane immediately
- Given a Staging lane has just been inserted on "Homelab Ops"
- When Priya drags OPS-3 into Staging and opens OPS-3's edit dialog
- Then the drag succeeds and Staging appears among the dialog's Status options
- And a machine client may PATCH an issue to the Staging lane's slug

###### Scenario: A name colliding with an existing lane is refused inline
- Given the "Homelab Ops" board already renders a lane named Done
- When Priya tries to insert a lane named "Done"
- Then the refusal reason names the conflict and renders in the dialog
- And the board still renders exactly the lanes it did before

###### Scenario: A name with no usable characters is refused inline
- Given the insert dialog is open on "Homelab Ops"
- When Priya submits the name "..."
- Then the refusal asks for a name using letters or numbers and no lane is created

###### Scenario: A non-member cannot insert a lane
- Given Marco is signed in and is not a member of team Backend
- When Marco sends the lane-insert confirm for "Homelab Ops" directly
- Then the response is the uniform non-enumerable 404 and the lane set is unchanged

##### Acceptance Criteria

- AC-3.1 — `⋯ → Insert list before` and `⋯ → Insert list after` each open a dialog in `#modal-root` with one name field, closable declaratively and by `Escape` through the single BR-4 owner (D5, D9).
- AC-3.2 — The new lane lands at exactly the position the chosen item names (immediately left of, or immediately right of, the originating column). Positions across the project remain contiguous and unique afterwards.
- AC-3.3 — **No existing card changes lane or position**, and no `0013` change event is written by an insert. Insertion is a lane-set operation only.
- AC-3.4 — The insert and its position shuffle are **one transaction**. A failure leaves the lane set byte-identical (D8).
- AC-3.5 — The lane's slug is minted once from the submitted name, satisfies `^[a-z][a-z0-9_]*$`, and is unique within the project. It is never re-derived afterwards (D4, D6).
- AC-3.6 — A name whose slug collides with an existing lane, and a name normalising to an empty slug, are each refused with a reason routed to `[data-error-slot]`; nothing is created and no position is shuffled (D7).
- AC-3.7 — The inserted lane is immediately a first-class lane on every lane-consuming surface: board column, dnd `data-column` target, edit-dialog Status option, `/api/v1` accepted state, delete-dialog destination, report label.
- AC-3.8 — The confirm is a `_csrf`-bearing POST; tokenless is refused pre-handler; a non-member's GET and POST are both the uniform 404 (D11).
- AC-3.9 — No new migration is required; the schema counter stays at 0015 (D8). If DESIGN finds otherwise, that is a reportable premise break, not a silent migration.

**Estimate: 1.5 days** — the position shuffle (D8) carries this feature's only
real uncertainty. Dependencies: US-BLO-01 (menu entry point). Independent of
US-BLO-02.

### [REF] System Constraints

- Lane slugs are immutable identity, labels are mutable display; the composite FK `(project_id, state) → lanes(project_id, slug)` is the no-stranded-card invariant and must hold after every operation here (`brief.md` §lanes).
- No adapter may hold a static lane list — `cargo xtask check-arch` fails the build. The menu's items are fixed, but the *lanes* they act on are always read from lane rows.
- `fn slugify(` under `crates/foundry-app/src` fails the build; lane-slug minting lives below the adapter (D6, `brief.md` §names-are-labels).
- `Escape` has exactly one owner, `closeTopLayer()`; new close affordances are declarative attributes, never listeners (BR-4, `adr-modal-close-001`).
- Dialogs are `div.modal` fragments htmx-swapped into `#modal-root`; "closed" is DOM-derived (host empty), never a stored flag.
- Mutating htmx triggers carry `_csrf`; validation refusals are 422 + bare fragment into `[data-error-slot]` (`form-errors.js`); authz refusals are the uniform non-enumerable 404 — never 401/403.
- `board_columns.html` is shared by the full page and the OOB refresh; both must render byte-identical markup (D14).
- Menu CSS lands in the content-hashed stylesheet; the re-hash procedure and `static/VENDOR.md` row must be updated together.
- Migrations are forward-only; the next number would be 0016, but D8 expects none.
- Test lanes: HTTP acceptance lane for status/fragment/persistence; fantoccini `@needs-browser` lane for menu interaction, focus and error-slot routing; per-feature mutation testing ≥80%.
- Known suite impact: `feature_board_lane_management.rs:2405-2427, 2565` premise-break by design (D13).

### [REF] Outcome KPIs

Objective: the board keeps matching the operator's workflow in both directions
— tightening and loosening — and the header never arms a destructive action by
default.

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|---|---|---|---|---|---|
| 1 | Board operators | Reach a destructive lane action only via an explicit menu selection | 100% of delete-dialog opens preceded by a menu activation; 0 armed `×` controls rendered anywhere | 0% — every column header renders an armed `×` today | Acceptance suite (US-BLO-01 AC-1.1/1.3); grep for `lane-delete` markup in rendered board HTML | Guardrail |
| 2 | Board operators | Correct a drifted lane name without a database session | ≥1 rename performed via the UI within 30 days; 0 SQL `UPDATE lanes SET label` sessions | 0 possible — no rename path exists in any layer | SQL over `lanes.label` vs seed values; single-operator self-report | Leading |
| 3 | Board operators | Recover from over-trimming by inserting a lane | Lane deletion is no longer one-way for 100% of projects | 0% — `board-lane-management` D9 shipped delete without add | Acceptance suite (US-BLO-03); lane-count-over-time query per project | Leading |
| 4 | Board operators | Keep every card where they left it across a rename or insert | 0 cards change lane or position as a result of a rename or an insert | n/a — neither operation exists | Acceptance suite (AC-2.2, AC-3.3) + zero-laneless guard query after every scenario | Guardrail |
| 5 | Board operators | Keep every lane slug stable for the life of the lane | 0 slug mutations after creation, permanently | n/a — no rename path exists to threaten it | SQL diff of `lanes.slug` before/after each mutating scenario; check-arch | Guardrail |

Homelab-scale honesty: single-digit-operator instance; KPIs are verified by the
acceptance suite and SQL against the store, not analytics tooling — the same
posture the predecessor recorded.

### [REF] DoD

- All UAT scenarios green in the HTTP lane; menu interaction, focus-return and error-slot scenarios green in the `@needs-browser` lane.
- Zero issues in a laneless state, provable by query, after every scenario run (the shipped guard, unchanged).
- `lanes.slug` provably unmutated across every rename and insert scenario (KPI 5).
- The two re-premised `board-lane-management` browser scenarios (D13) green, opening the menu first.
- No `×` delete control renders in either board render path (full page and OOB), verified against both.
- Live round-trip demonstrated: open `⋯` → rename a lane → insert a lane before it → delete a lane through the menu → board reads correctly and no card moved except by the delete's own fate choice.
- Stylesheet re-hashed and `static/VENDOR.md` row updated in the same commit as the menu CSS.
- `cargo xtask ci` green (check-arch, deny, mutation ≥80% on touched code); merged to main.

### [REF] Out of Scope

- **Archive** of lanes or cards, in any form (D1). No soft-hidden state is introduced.
- **Sort by** (the screenshot's last item) — a new ordering concept with no backend.
- **Archive cards / move all cards** as a standalone menu item — bulk card movement remains available only as a lane-delete fate, exactly as shipped.
- **Reordering existing lanes** (drag a column, "move list left/right"). Insert places a *new* lane; it does not relocate an existing one. Flagged as the natural successor.
- Undo/restore of deleted lanes or cards.
- WIP limits, lane colours, per-lane settings of any kind.
- Per-team or per-workspace lane templates (predecessor D3: per-project only).
- Role-gated lane administration (predecessor D10 declined it; D11 carries that forward).
- Any change to project slugs, issue keys, card URLs, or lane slugs after creation.
- Changing the delete dialog's own behaviour, copy or fate arms — US-BLO-01 only changes how it is *reached*.

### [REF] WS Strategy

**No walking skeleton (D12).** The lanes-as-data foundation this feature builds
on shipped with `board-lane-management` and is enforced by a check-arch rule.
Every story is UI plus one write port on shipped substrate.

Delivery order **US-BLO-01 → US-BLO-02 → US-BLO-03**. US-BLO-01 first because
it is the only entry point the other two have; 02 before 03 because rename is
the lower-uncertainty write and establishes the lane-write seam that insert
then extends. 02 and 03 are otherwise independent and could be reordered if
the position-shuffle question (D8) needs to be answered early.

### [REF] Driving Ports

New behaviour the adapters need from the core (DESIGN owns shapes and placement):

1. **Lane rename write** — set `lanes.label` for one lane of one project, under the team-membership gate, with the 1–64 bound enforced below the adapter. Must provably not touch `slug` or `position`.
2. **Lane insert write** — mint a lane slug from a submitted name (underscore-separated, letter-anchored, unique per project — **not** `foundry_core::slugify`, D6), insert at a named position relative to an existing lane, and shuffle later positions, all in one transaction (D8).
3. **Lane-name validation** — one seam shared by insert (slug uniqueness + non-empty normalisation) and rename (label bounds), mirroring the DD10 "one normalisation shared by both adapters" property already established for state validation.
4. **Lane read for the dialogs** — current label for the edit dialog's pre-fill; the project's lane set and the originating lane's position for insert arithmetic. Both already available via `list_project_lanes`.

The delete write port is unchanged and shipped.

### [REF] Pre-requisites

None outstanding. Everything this feature needs is shipped: `lanes` rows and
the composite FK (0015), `list_project_lanes`, `validate_project_lane`, the
`#modal-root` dialog frame with declarative close (`adr-modal-close-001`),
`form-errors.js` 4xx routing, CSRF middleware, the uniform-404 idiom, the
content-hashed stylesheet pipeline, and both test lanes.

One **open technical question** carried into DESIGN as this feature's single
highest-uncertainty item: **D8** — confirming that the `DEFERRABLE` unique
position constraint absorbs a mid-board insert inside one transaction without a
schema change. DESIGN must prove this against live-shaped data before US-BLO-03
is planned; if it does not hold, US-BLO-03 grows a migration and its estimate
moves.

### [REF] DoR Validation

| DoR Item | US-BLO-01 | US-BLO-02 | US-BLO-03 | Evidence |
|---|---|---|---|---|
| 1. Problem in domain language | PASS | PASS | PASS | Armed `×` in every header; a lane name drifted from standup; deletion is a one-way ratchet |
| 2. Persona specific | PASS | PASS | PASS | Priya Raman (`persona-instance-operator`); Marco as non-member foil |
| 3. 3+ domain examples, real data | PASS | PASS | PASS | Homelab Ops/OPS with OPS-3/7/9; lanes Backlog, In-Progress, Done, Staging, Archive Box |
| 4. UAT 3–7 scenarios G/W/T | PASS (6) | PASS (5) | PASS (6) | Embedded above |
| 5. AC derived from UAT | PASS | PASS | PASS | Each AC maps to ≥1 scenario and a D-decision |
| 6. Right-sized | PASS 1d | PASS 1d | PASS 1.5d | ≤1 day each except 03, whose slice brief carries the D8 spike |
| 7. Technical notes/constraints | PASS | PASS | PASS | System Constraints + Driving Ports + D1–D14 |
| 8. Dependencies tracked | PASS | PASS | PASS | 02 and 03 both depend on 01 (menu is the only entry point); 02 ⊥ 03 |
| 9. Outcome KPIs measurable | PASS | PASS | PASS | 5-row KPI table with baselines and store-verifiable measurement |

**DoR Status: PASSED** (9/9, all three stories). Requirements completeness:
**0.97** — the one residual is D8's mechanism, deliberately left to DESIGN with
a named failure consequence (AC-3.9) rather than guessed here.

Per-wave peer review (`nw-product-owner-reviewer`) **not invoked** — none of the
four triggers fired (no DoR ambiguity, JTBD is a shipped validated job, no
vendor-neutrality risk, user did not request). The mandatory consolidated
review fires at end of DISTILL.

### [REF] Inherited commitments

| Origin | Commitment | ADR | Impact here |
|---|---|---|---|
| `brief.md` §names-are-labels | Slugs are immutable identity, names are mutable labels; `fn slugify(` banned under `foundry-app/src` | ADR-PROJECT-RENAME-001/002 | D4 (rename is label-only); D6 (lane-slug mint lives below the adapter) |
| `brief.md` §lanes | Lane slugs immutable, labels mutable; composite FK is the no-stranded-card invariant; no static lane list in any adapter | ADR-BOARD-LANE-001 | D4, AC-2.2, AC-3.7, KPI 5 |
| `brief.md` §dialog-layers | One close mechanism; `Escape` has exactly one owner; close affordances are attributes, not listeners (BR-4) | ADR-MODAL-CLOSE-001 | D9, AC-1.4, AC-2.1, AC-3.1 |
| `board-lane-management` D9 | Add/rename/reorder deferred; successor feature pre-registered | n/a | This feature is that successor; reorder remains deferred (Out of Scope) |
| `board-lane-management` D7 | Lane delete + card fate is one atomic, counted operation | ADR-BOARD-LANE-002 | Unchanged — US-BLO-01 changes only how the dialog is reached |
| `board-lane-management` D10 | Team-membership gate; uniform non-enumerable 404; `_csrf` on mutating htmx triggers | n/a | D11, AC-1.6, AC-2.5, AC-3.8 |
| `board-lane-management` D6 | A project always keeps ≥1 lane; new issues land leftmost | n/a | Insert never threatens the minimum; AC-3.2 keeps leftmost landing correct |
| `0012_issue_position.sql` | Contiguous positions; zero-shuffle discipline | n/a | AC-3.3 — an insert shuffles *lane* positions and zero *issue* rows |
| `0013_issue_change_events.sql` | Append-only, same-transaction, one row per changed field | n/a | AC-3.3 — a lane insert writes no issue change event; a rename writes none either |
| `fix-comment-delete-csrf` | Every mutating htmx trigger carries CSRF; HTTP-lane token injection can mask a real browser 403 | n/a | AC-2.5, AC-3.8 must be proven in the browser lane, not only the HTTP lane |
| `canzan-theme-system` | Colour enters at one token seam; assets are hash-honest by construction | ADR-CANZAN-THEME-004 | Menu CSS uses existing tokens and must survive both palettes; re-hash procedure in DoD |
| `foundry-services` DD10 | One normalisation shared by HTML and JSON adapters | n/a | Driving Port 3 — one lane-name validation seam for insert and rename |

### [REF] Triggered suggestions (ask-intelligent)

One trigger fired — **cross-context complexity**: this feature spans three
distinct technologies (Rust/Askama server rendering, browser JS/CSS, SQL/schema
semantics). Suggested expansion: `alternatives-considered`.

Deferred successors (not expansions, recorded for the backlog):

1. **Reorder lanes** — drag a column, or "move list left/right" in this same menu. The menu is now the natural home for it, and the position-shuffle machinery from D8 is exactly what it needs.
2. **Undo a lane delete** — insert makes deletion recoverable *by hand*; a real undo is still absent.
3. **Sort by** — the screenshot's remaining item; needs a card-ordering concept the board does not have.
4. **WIP limits per lane** — still unlocked by lanes-as-data, still unbuilt.

## Wave: DESIGN

### [REF] Prior Wave Consultation

| Source | Read | Effect |
|---|---|---|
| This file, `## Wave: DISCUSS` | ✓ | D1–D14 consumed; D8 was the gate DESIGN had to clear |
| `docs/product/architecture/brief.md` §lanes / §dialog-layers / §names-are-labels | ✓ | Three standing invariants constrain every decision below |
| `adr-board-lane-001`, `-002`, `adr-modal-close-001`, ADR-003 (keyboard layers) | ✓ | `FOR UPDATE` idiom reused; BR-4 and the DOM-derived-stack rule are binding |
| `crates/foundry-store/migrations/0015_project_lanes.sql` | ✓ | The `DEFERRABLE` declaration turned out to be the whole answer to D8 |
| `crates/foundry-app/static/js/keyboard.js` (layer machinery, delegated listeners) | ✓ | Menu designed as a fourth arm, not a component |
| `crates/foundry-app/src/lib.rs:598-608` (lane route mounting) | ✓ | New routes mount identically, under the same middleware stack |
| `crates/foundry-core/src/lib.rs:253-303` (`slugify` + its check-arch note) | ✓ | Proved unusable for lane slugs |

### [REF] D8 resolved by measurement — the DISCUSS premise was wrong in our favour

DISCUSS ranked D8 the feature's only real uncertainty and required DESIGN to
prove it before slice 03 could be planned. It was **run** against a disposable
`postgres:16-alpine` container (the tag `harness.rs:76` pins to production)
carrying a faithful 0015 reproduction with the composite FK and live-shaped data.
Eight tests; full table in `design/architecture-design.md` §1.

**Corrected premise:** `DEFERRABLE INITIALLY IMMEDIATE` does not mean "checked
immediately, per row" — it means **checked after each statement**. DISCUSS
assumed a `SET CONSTRAINTS ... DEFERRED` window would be needed. It is not; no
such call appears anywhere in this feature. The naive bulk shift commits as-is.

Three results now bind the design:

1. **No migration. The counter stays at 0015** — AC-3.9 satisfied by measurement.
2. **`DEFERRABLE` on `0015:22` is load-bearing.** The identical statement against a non-deferrable constraint fails with `duplicate key value violates unique constraint`. A later "cleanup" migration dropping it breaks lane insert while every existing test stays green.
3. **Concurrency needs a lock, which DISCUSS never asked about.** Unguarded concurrent inserts hand the loser a raw duplicate-key error. With the shipped `FOR UPDATE` idiom and an identity-resolved anchor, both commit cleanly — positions contiguous, zero issue rows touched.

The spike also caught a trap by accident: reading the anchor's position *after*
the shift fails. **The anchor is resolved by lane identity inside the lock and
its position captured before the shift** — the same shape as the predecessor's
D7 ("the count is advisory; the fate binds at confirm time").

### [REF] Design Decisions

| ID | Decision | Source |
|---|---|---|
| DD1 | **No migration; 0015 stands.** Insert shuffles with a plain `UPDATE` inside one `FOR UPDATE` transaction. | ADR-BOARD-LANE-003; spike tests 1–5b |
| DD2 | **`DEFERRABLE` is a guarded schema fact.** A `check-arch` rule pinning the keyword is recommended to DELIVER; until it exists, ADR-003 is the only guard. | ADR-BOARD-LANE-003; `architecture-design.md` §6 |
| DD3 | **`foundry_core::lane_slug`** mints lane slugs; `slugify` is unusable (emits hyphens, rejected by `lanes_slug_check` — verified). Digit-leading names take a `lane_` prefix, a disclosed deviation from D7 justified because the label is preserved and the slug is never surfaced. | ADR-BOARD-LANE-004; spike test 8 |
| DD4 | **The menu is a fourth arm of `closeTopLayer()`**, registering no listeners of its own; `menuIsOpen()` is DOM-derived; both click behaviours are branches of the existing delegated listener. | ADR-BOARD-LANE-005; BR-4 |
| DD5 | **Arm order:** help → modal → menu → search → no-op. Mutually exclusive in practice, deterministic by contract. | ADR-BOARD-LANE-005 |
| DD6 | **Routes:** `…/lanes/{lane}/edit` and `…/lanes/{lane}/insert/{side}`, both verbs, mounted under the existing `csrf_middleware` + `session_layer` beside delete. An unrecognised `{side}` is the uniform 404, never a 400. | `component-boundaries.md` §2.1 |
| DD7 | **Rename needs no lock and no transaction** beyond the implicit one — a single `UPDATE ... SET label`. Last-write-wins on a display label is acceptable; no invariant depends on it. | `architecture-design.md` §5.1 |
| DD8 | **Slug collision is pre-checked inside the insert's lock**, so the operator gets D7's refusal copy, never the raw `duplicate key` error the DB produces otherwise. | Spike test 7 |
| DD9 | **Duplicate labels are permitted** (two lanes may both read "Doing"); only slugs are unique. Confirms AC-2.6 against the real schema. | Spike test 7 |
| DD10 | **`BoardColumn` carries the four action URLs**, built in `board_columns()` from validated path slugs — the shipped idiom already used for `IssueCard.edit_url`. Never `slugify(name)`. | `component-boundaries.md` §2.2 |
| DD11 | **Two tests exist because the spike found their failure modes**, and neither was in the DISCUSS ACs: a store-level *concurrent insert* test, and a browser scenario that opens a menu, triggers an OOB `#board-columns` refresh, then presses `Escape`. | Spike test 4; ADR-BOARD-LANE-005 rule 2 |
| DD12 | **Paradigm unchanged.** Rust is multi-paradigm, but 48 features of established practice settle it. No paradigm section written to `CLAUDE.md`. | Repo history |

### [REF] C4 Diagrams

System Context, Container, and a Component diagram for the layer subsystem — all
Mermaid, in `design/architecture-design.md` §2–§4. The Component diagram is
included because the menu is the one place this feature can violate a standing
architectural rule (BR-4), so the arm structure is worth drawing.

### [REF] Artifacts

| Path | Contents |
|---|---|
| `design/architecture-design.md` | D8 spike results, C4 ×3, write paths, slug minting, the `DEFERRABLE` guard, reuse table, residual risks |
| `design/component-boundaries.md` | Markup contract, `keyboard.js` may/must-not, routes, service seam, store shapes, test-lane ownership |
| `design/data-models.md` | Zero schema delta, per-operation column impact, position invariants, slug rules, label-vs-slug uniqueness |
| `design/technology-stack.md` | Nothing new adopted; rejected options |
| `docs/product/architecture/adr-board-lane-003-deferrable-position-shuffle.md` | The insert transaction and why no migration |
| `docs/product/architecture/adr-board-lane-004-lane-slug-mint.md` | `lane_slug` and the prefix deviation |
| `docs/product/architecture/adr-board-lane-005-overflow-menu-as-layer-arm.md` | The menu as a `closeTopLayer()` arm |

### [REF] SSOT updates (back-propagation)

`docs/product/architecture/brief.md` — two sections extended, neither rewritten:

- **§Lanes are per-project data** gains the shape-in-place consequences: the `DEFERRABLE` keyword is load-bearing (with the measured failure mode of dropping it), and lane slugs come from `lane_slug`, never `slugify`.
- **§Dialog layers close by one mechanism** gains the extension beyond dialogs: the overflow menu is an arm, not a component; open state is DOM-derived; `keyboard.js` holds exactly one `keydown` and one `click` document listener, and more than that is a violation.

### [REF] Changed Assumptions

**Original (DISCUSS, D8):**

> "Insert defers position shuffling to the deferred-constraint window… the
> constraint was declared `DEFERRABLE` precisely so a transaction can
> `SET CONSTRAINTS ... DEFERRED` and shuffle freely."

**New assumption:** No `SET CONSTRAINTS` is needed. `DEFERRABLE INITIALLY
IMMEDIATE` already checks at end-of-statement, so a plain `UPDATE` suffices.
**Rationale:** measured directly — the naive shift commits against the shipped
constraint and fails against a non-deferrable one. DISCUSS reached the right
conclusion (no migration) through the wrong mechanism.

**Second change — a gap, not a correction.** DISCUSS specified no concurrency
requirement for insert. The spike showed the unguarded path hands the operator a
raw Postgres error under concurrent inserts. `FOR UPDATE` plus an
identity-resolved anchor is added to the design, and DD11 adds the two tests
that hold it. DISCUSS's acceptance criteria are unchanged and still correct —
they were incomplete, not wrong.

DISCUSS documents are not modified (back-propagation contract); this section is
the record.

### [REF] Pre-requisites for DISTILL

None outstanding. The feature's one open question is closed by measurement, and
nothing else was deferred to a later wave.

DISTILL should note that three acceptance obligations arrived in DESIGN rather
than DISCUSS and must appear in the scenario table: the concurrent-insert store
test (DD11), the menu-open → OOB-refresh → `Escape` browser scenario (DD11), and
the `@layered` scenario's fourth arm (DD5).

## Wave: DISTILL

Mode: **scaffolded RED** (ADR-025 — DISTILL authors ALL acceptance tests as
per-scenario `@pending`; DELIVER only un-pends, one at a time, and never
re-authors). Lang: rust. Framework: **cucumber-rs** (the skill's
pytest-bdd/SpecFlow options do not apply to this repo). Integration approach:
**testcontainers + real services** — real Postgres 16 via the shared
testcontainer with a per-scenario schema, the real tower-sessions store, the
real double-submit CSRF middleware, the in-process axum router, REAL registered
EdDSA bearers for machine legs, and real headless Chrome via fantoccini for the
browser lane. Infrastructure testing: **no** (functional acceptance only;
`cargo xtask ci` owns the CI gate).

### [REF] Prior Wave Consultation

| Artifact | Status |
|---|---|
| `feature-delta.md` DISCUSS (D1–D14, US-BLO-01..03, 17 UAT scenarios 6/5/6) | ✓ read |
| `feature-delta.md` DESIGN (DD1–DD12, incl. the three scenarios DESIGN added) | ✓ read |
| `design/{architecture-design,component-boundaries,data-models,technology-stack}.md` | ✓ read |
| `docs/product/architecture/adr-board-lane-003/-004/-005.md`, `brief.md` | ✓ read |
| `docs/product/jobs.yaml` (`job-board-lane-shaping`), `outcomes/registry.yaml` | ✓ read (rows appended) |
| `docs/architecture/atdd-infrastructure-policy.md` | ✓ read (row appended) |
| `slices/slice-01..03` | ✓ read |

**Reconciliation: PASSED — 0 contradictions.** DESIGN refines DISCUSS in three
places and contradicts it in none: (1) D8's *mechanism* was corrected by
measurement (no `SET CONSTRAINTS` needed) while its *conclusion* (no migration)
stood; (2) a concurrency requirement was ADDED that DISCUSS never specified;
(3) D4's label-only rename was confirmed to be an inherited invariant rather
than a feature choice. All three are pinned by scenarios below.

### [REF] Scenario table

23 scenarios in `crates/foundry-acceptance/tests/features/board-lane-overflow-menu.feature`,
ALL `@pending`. RED classification from a run with `@pending` stripped
(`FOUNDRY_ACCEPTANCE_TAGS=blo`): **23/23 failed, all MISSING_FUNCTIONALITY,
zero BROKEN, zero false-GREEN.**

Classification: 7 × MF(markup) — the `⋯` trigger does not exist; 6 ×
MF(port-501) — the handler is a clean-501 scaffold; 10 × MF(behaviour) — the
assertion bites because the write did not take effect. The last group is the
purest RED (assertion-level, not structural).

| Scenario | Slice | Lane | Tags (beyond `@blo @pending`) | RED classification |
|---|---|---|---|---|
| The column header offers a menu, not an armed delete | 01 | HTTP | `@us-blo-01 @driving_port @real-io` | MF(markup): no `data-action="toggle-lane-menu"` in any column |
| The menu lists exactly the four lane operations | 01 | HTTP | `@us-blo-01 @driving_port` | MF(markup): no `[data-lane-menu]` container |
| Delete list reaches the shipped dialog unchanged | 01 | browser | `@us-blo-01 @needs-browser @driving_port @real-io` | MF(markup): trigger absent in a real Chrome |
| Escape closes the menu and returns focus, changing nothing | 01 | browser | `@us-blo-01 @needs-browser @edge @real-io` | MF(markup) |
| The menu is reachable and operable without a pointer | 01 | browser | `@us-blo-01 @needs-browser @edge` | MF(markup) |
| A non-member reaching the lane routes gets the uniform not-found | 01 | HTTP | `@us-blo-01 @error @security` | MF(behaviour): scaffold answers 501, not the uniform 404 |
| Renaming a lane changes the header and nothing else | 02 | HTTP | `@us-blo-02 @driving_port @real-io` | MF(behaviour): header still reads In-Progress |
| A rename never touches lane identity | 02 | HTTP+API | `@us-blo-02 @edge @real-io` | MF(behaviour) |
| The edit dialog opens showing the lane's current name | 02 | HTTP | `@us-blo-02` | MF(port-501) |
| An empty or over-long lane name is refused inline | 02 | HTTP | `@us-blo-02 @error` | MF(behaviour): 501, not 422 |
| A rename is refused without the board's token and accepted with it | 02 | HTTP | `@us-blo-02 @error @security` | MF(behaviour): the tokened leg 501s |
| Two lanes may carry the same label because labels are not identity | 02 | HTTP | `@us-blo-02 @edge` | MF(behaviour) |
| Inserting before a lane places the new lane immediately to its left | 03 | HTTP | `@us-blo-03 @driving_port @real-io` | MF(behaviour) |
| Inserting after the last lane appends at the far right | 03 | HTTP | `@us-blo-03 @edge @real-io` | MF(behaviour) |
| An inserted lane is a fully working lane immediately | 03 | HTTP+API | `@us-blo-03 @driving_port @real-io` | MF(port-501) |
| A name whose slug collides with an existing lane is refused inline | 03 | HTTP | `@us-blo-03 @error` | MF(behaviour) |
| A name with no usable characters is refused inline | 03 | HTTP | `@us-blo-03 @error @edge` | MF(behaviour) |
| A lane name that cannot start a slug still becomes a working lane | 03 | HTTP+API | `@us-blo-03 @edge` | MF(behaviour): `2024 Review` not rendered |
| An unrecognised insert side is indistinguishable from an unknown lane | 03 | HTTP | `@us-blo-03 @error @security` | MF(port-501) |
| A non-member cannot insert a lane | 03 | HTTP | `@us-blo-03 @error @security` | MF(port-501) |
| **Two operators inserting at the same anchor both land** | 03 | HTTP | `@us-blo-03 @concurrency @real-io @adapter-integration` | MF(port-501) — *added by DESIGN* |
| **The menu survives the board refreshing underneath it** | 01 | browser | `@us-blo-01 @needs-browser @layered @edge @real-io` | MF(markup) — *added by DESIGN* |
| **Escape peels one layer at a time with a menu open** | 01 | browser | `@us-blo-01 @needs-browser @layered @edge` | MF(markup) — *added by DESIGN* |

Error/edge/security/concurrency/layered share: **16/23 = 69%** (≥40% target).
`@walking_skeleton` count: **0** — deliberate, per D12. Its absence is a
decision, not an omission, and the feature file says so in prose so a future
reader does not "fix" it.

### [REF] The classification run earned its keep — three defects in the TESTS

The RED run is not a formality here; it found three real problems in the suite
itself, all of which would otherwise have reached DELIVER as noise:

1. **A false-GREEN.** "A rename that does not carry the board's matching token
   is refused" **passed** on the first run — because the shipped CSRF middleware
   refuses a tokenless POST to *any* mounted route, so the scenario tested the
   middleware, not this feature. Fixed by making it a **differential** oracle:
   the same rename is now submitted twice, without the token and then with it,
   and the tokened leg must be **accepted and take effect**. It reds now, and
   after DELIVER it proves the token is what mattered.
2. **A BROKEN oracle (18 scenarios).** The state-delta snapshot counted a table
   named `notification_outbox`, which does not exist — the table is `outbox`.
   Every affected scenario was failing for the wrong reason.
3. **A BROKEN harness call (18 scenarios).** The local `priya_get`/`priya_post`
   wrappers passed a full URL to `signed_in_get`/`signed_in_post`, which prepend
   `harness.base_url()` themselves — producing `InvalidPort` before any
   assertion ran. Corrected to the house convention: pass a **path**.

Zero of the 23 scenarios now fail for a reason other than the feature being
absent.

### [REF] Adapter / driving-port coverage

| DESIGN route / port | Scenario(s) |
|---|---|
| GET `…/lanes/{l}/edit` (dialog) | edit-dialog pre-fill; Marco's route sweep |
| POST `…/lanes/{l}/edit` (rename confirm) | rename-changes-header; identity-untouched; bad-names; CSRF differential; duplicate-labels |
| GET `…/lanes/{l}/insert/{side}` (dialog) | unrecognised-side; Marco's insert GET |
| POST `…/lanes/{l}/insert/{side}` (insert confirm) | before; after; working-lane; collision; unusable names; digit-leading; concurrency; Marco's insert POST |
| GET `…/lanes/{l}/delete` (shipped, now menu-reached) | Delete-list-reaches-dialog (browser); Marco's route sweep |
| GET board (`board_view` → columns) | menu-trigger-per-column; four-items; every lane-order oracle |
| POST `…/issues/{n}/state` (dnd) | identity-untouched; move-into-Staging |
| `/api/v1` PATCH state | identity-untouched; machine-move-to-Staging; machine-move-to-2024-Review |
| `/api/v1` POST issues (leftmost landing) | insert-after-Done leftmost oracle |
| `keyboard.js::closeTopLayer()` (the new arm) | Escape-closes-and-returns-focus; menu-survives-OOB; Escape-peels-one-layer |
| `foundry_core::lane_slug` | digit-leading scenario asserts the RULE (`^[a-z][a-z0-9_]*$`), never a spelling |
| `Store::{rename_lane, insert_lane_at}` | exercised through the routes above — port-to-port; internals never invoked directly |

### [REF] Scaffolds (Mandate 7 — RED not BROKEN)

Marker: `SCAFFOLD: true`. Store/services/core bodies `panic!`; app handlers
return a clean **501** (the `admin_tokens` precedent — a panic aborts the axum
connection and masks the assertion). Routes are mounted **now**, not at DELIVER,
precisely so the authz scenarios stay honest: an unrouted path answers the exact
uniform 404 they assert, and would pass for the wrong reason.

| File | Contents |
|---|---|
| `crates/foundry-core/src/lib.rs` | `lane_slug` (sibling of `slugify`; underscores, letter-anchored) |
| `crates/foundry-store/src/lanes.rs` | `LaneSide`, `LaneInsertOutcome`, `Store::{rename_lane, insert_lane_at}` |
| `crates/foundry-services/src/lanes.rs` | `EditLaneView`, `InsertLaneView`, `LaneSideView`, `RenameLaneError`, `InsertLaneError`, `LaneStoreFailure`, `edit_lane_dialog`, `rename_lane`, `insert_lane_dialog`, `insert_lane` |
| `crates/foundry-app/src/lanes.rs` | `parse_lane_side`, `EditLaneForm`, `InsertLaneForm`, four handlers (501 bodies) |
| `crates/foundry-app/src/lib.rs` | the two new routes mounted UNDER `csrf_middleware` + `session_layer` |

Deliberately **NOT** scaffolded (behaviour-changing, DELIVER-owned): all
template changes (`board_columns.html` menu markup and `×` removal, the edit and
insert dialog partials); `BoardColumn`'s action-URL fields; the menu's
`keyboard.js` arm; menu CSS and the stylesheet re-hash; the recommended
`check-arch` rule pinning `DEFERRABLE`.

**Workspace green with everything `@pending`:** `cargo check --workspace
--all-targets` clean, `cargo fmt --all --check` clean, `cargo clippy --workspace
--all-targets -- -D warnings` clean, `cargo xtask check-arch` PASSED, and the
full default acceptance lane **572/572 scenarios, 4035/4035 steps green**.

### [REF] Test placement

`crates/foundry-acceptance/tests/features/board-lane-overflow-menu.feature` +
`crates/foundry-acceptance/src/steps/feature_board_lane_overflow_menu.rs`
(registered in `src/lib.rs` and force-linked in `tests/acceptance.rs`) + `blo_*`
world fields in `src/world.rs`. Browser lane via `support::browser_harness`
(fantoccini, chromedriver probe-then-refuse).

**Step-name collision sweep:** cucumber-rs registers step regexes globally
across all 1,434 shipped definitions, so a duplicate phrase is an ambiguous
match at runtime, not a compile error. All 74 of this file's step phrases were
matched against every existing regex; three collided with
`feature_board_lane_management` (`Priya is a member of team Backend…`, `…opens
the "X" board`, `no issue on any board is without a lane`) and were rephrased.
Re-verified: zero collisions.

### [REF] Oracle discipline

1. **Lane-list oracle** — every lane expectation reads lane rows back from the
   DB. No static expected-lane list exists in the module; one would go green
   over exactly the static-list consumers the `check-arch` rule forbids.
2. **Contiguity** — `assert_contiguous` checks positions are `0..n-1`. Postgres
   enforces only uniqueness, so a gap is invisible to the schema and merely
   cosmetic to `ORDER BY position`. This assertion is the system's only guard.
3. **Identity** — rename oracles compare `slug`, `position` and every
   `issues.state` from the **store**, not the DOM. A DOM-only assertion would
   pass over a rename that also rewrote issue states.
4. **Rendered-vs-stored agreement** — lane-order oracles assert the rendered
   `data-column` sequence equals the stored rows in position order, so a static
   list driving the columns cannot hide behind a correct database.
5. **State-delta, fail-closed** — a rename and an insert must each move ZERO
   issue rows and write ZERO change events and ZERO outbox rows.
6. **Non-enumerability** — refusals compared byte-identical to a never-existed
   path, both verbs. An unrecognised `{side}` is included: it must be
   indistinguishable from an unknown lane, never a 400.
7. **Slug rule, not slug spelling** — the digit-leading scenario asserts the
   minted slug satisfies `^[a-z][a-z0-9_]*$` and is underscore-separated. Which
   exact string DELIVER mints is DELIVER's to choose within the rule.
8. **Lane resolution by label** — `lane_slug_named` looks a lane's slug up by
   its label rather than guessing it, so the suite never hard-codes a slug the
   implementation has not chosen.
9. **Zero-laneless guard** after every mutating scenario.
10. **Concurrency honesty** — the `@concurrency` scenario drives two REAL
    concurrent confirms through the real adapter and asserts BOTH commit and
    that neither body contains `duplicate key`. It exists because the DESIGN
    spike measured the unguarded path failing exactly that way.

### [REF] Pre-requisites for DELIVER

1. **Un-pend order:** slice 01 → 02 → 03; within a slice, browser scenarios last.
   There is no walking skeleton to sequence first (D12).
2. **Slice 01 is the gate for the other two** — every Edit/Insert scenario
   reaches its dialog through the menu, so the markup lands first.
3. **`board_columns.html` is shared** with `partials/oob/board_columns_oob.html`;
   the menu markup must be authored once and both paths verified (D14).
4. **The stylesheet re-hash** (`static/css/foundry.<hash>.css` + `base.html` +
   `static/VENDOR.md`) must land in the same commit as the menu CSS. `check-arch`
   R2/R3 fail the build on a stale hash row.
5. **Two `board-lane-management` browser scenarios premise-break by design**
   (`feature_board_lane_management.rs:2407`, used at :2427 and :2565): they click
   `button[data-lane-delete]` directly, which D3 removes. US-BLO-01 owns
   re-premising them to open the menu first. They are currently GREEN and will
   red the moment the `×` is removed — expected, tracked, not a regression.
6. **`lane_slug` is the property-test target** (pure heart — layers 1–2 PBT per
   Mandate 9): a fixed point, and output that always either satisfies
   `^[a-z][a-z0-9_]*$` or is empty. Acceptance stays example-only.
7. **The `check-arch` `DEFERRABLE` rule** (`architecture-design.md` §6) is
   DELIVER's to implement. Until it exists, ADR-BOARD-LANE-003 is the only thing
   standing between a tidy-up migration and a silently broken insert.
8. Per-feature mutation ≥80% on touched code (DoD).

### [REF] SSOT / policy updates

- `docs/product/outcomes/registry.yaml`: **OUT-6** (operation: the overflow menu
  as a `closeTopLayer()` arm), **OUT-7** (operation: label-only rename),
  **OUT-8** (operation: locked insert with the deferrable shift).
- `docs/architecture/atdd-infrastructure-policy.md`: driven-internal row for
  `Store::{rename_lane, insert_lane_at}` + `lane_slug`, recording that there is
  **no migration** and why the `DEFERRABLE` keyword is load-bearing. No new
  fakes — every port in scope is driving (HTTP/browser) or driven-internal
  (Postgres), REAL per the Architecture of Reference.

### [REF] Inherited commitments

| Origin | Commitment | ADR | Impact |
|---|---|---|---|
| DESIGN#DD1/DD2 | Insert shuffles under the deferrable constraint; no migration; `DEFERRABLE` is load-bearing | ADR-BOARD-LANE-003 | The `@concurrency` scenario drives two real concurrent confirms; the ATDD policy row records why no migration exists |
| DESIGN#DD3 | `lane_slug`, never `slugify` | ADR-BOARD-LANE-004 | The digit-leading scenario asserts the CHECK rule, not a spelling; `lane_slug` is the PBT target |
| DESIGN#DD4/DD5 | The menu is an arm of `closeTopLayer()`; open-state DOM-derived | ADR-BOARD-LANE-005 | Two `@layered` browser scenarios: one-layer-per-press, and menu-survives-OOB-refresh |
| DISCUSS#D3 | `⋯` replaces `×` outright | n/a | A dedicated oracle asserts `[data-lane-delete]` is GONE from the rendered board |
| DISCUSS#D4 | Rename is label-only; slug frozen | ADR-PROJECT-RENAME-001 (extended) | Identity asserted from the STORE across dnd + `/api/v1` legs |
| DISCUSS#D7 | Collisions and empty slugs refused inline, never auto-suffixed | n/a | The collision oracle also asserts the raw `duplicate key` text never reaches the operator |
| DISCUSS#D11 | Team-membership gate; uniform non-enumerable 404 both verbs | n/a | Marco sweeps all three lane routes × both verbs against a never-existed baseline |
| DISCUSS#D12 | No walking skeleton | n/a | Zero `@walking_skeleton` tags, stated in the feature file so it is not "fixed" later |
| DISCUSS#D14 | `board_columns.html` shared with the OOB partial | n/a | Menu-trigger oracle runs per column over the rendered board; DELIVER verifies both paths |
| `fix-comment-delete-csrf` | HTTP-lane token injection can mask a real browser 403 | n/a | The CSRF scenario is a differential (without → refused, with → accepted), and the browser lane drives a real confirm |
| brief.md#dialog-layers (BR-4) | One close mechanism, one `Escape` owner, no new listeners | ADR-MODAL-CLOSE-001 | `@layered` scenario reds if a second `Escape` listener peels two layers |
| ADR-BOARD-LANE-001 | Composite FK is the no-stranded-card invariant | n/a | Zero-laneless guard after every mutating scenario |

## Wave: DELIVER

Mode: **ADR-025 scaffolded RED** — DISTILL authored all 23 scenarios `@pending`
and classified them red; DELIVER un-pended them slice by slice and never
re-authored a scenario's intent. Paradigm: object-oriented/imperative
(unchanged; 48 features of precedent, no `## Development Paradigm` section
written to `CLAUDE.md`). Mutation strategy: per-feature, gate ≥80%.

**Executed by the orchestrator directly rather than dispatched to crafter
subagents.** Phase provenance is logged truthfully in
`deliver/execution-log.json`; `.nwave/des-config.json` records the phase set
this delivery actually ran (`PREPARE`, `RED_ACCEPTANCE`, `GREEN`), which is the
mechanism the integrity verifier documents for ADR-025 projects. `RED_UNIT` is
logged only for the three steps that genuinely added unit tests; `COMMIT` did
not run (see Commit status below).

### [REF] Implementation summary

| Step | What landed |
|---|---|
| 01-01 | `board_columns.html`: one `⋯` trigger + a four-item `[data-lane-menu]` per column; the armed `×` removed. `BoardColumn` carries the four action URLs, built in `board_columns()` from validated path slugs. |
| 01-02 | `keyboard.js`: the menu as `closeTopLayer()`'s **third arm**, DOM-derived open state, two branches on the **existing** delegated click listener, focus return, `aria-expanded` sync. Menu CSS on existing tokens; stylesheet re-hashed `41b3395b` → `78a05f58`. |
| 01-03 | `feature_board_lane_management.rs`: `click_lane_delete` re-premised to open the menu first (D13). Zero `[data-lane-delete]` selectors remain. |
| 02-01 | `validate_lane_label` (the shared seam), `Store::rename_lane` (label-only `UPDATE`), `services::rename_lane`. |
| 02-02 | `edit_lane_modal.html`, `EditLaneModal`, the edit dialog GET + rename confirm POST. |
| 03-01 | `foundry_core::lane_slug` + `is_valid_lane_slug`, with 6 example tests and 3 properties. |
| 03-02 | `Store::insert_lane_at` — the locked transaction exactly as ADR-BOARD-LANE-003 specifies. |
| 03-03 | `insert_lane_modal.html`, `InsertLaneModal`, the insert dialog GET + confirm POST, `services::insert_lane`. |

**No migration. The schema counter stays at 0015**, as DESIGN predicted by
measurement.

### [REF] Scenarios green

- **`board-lane-overflow-menu`: 23/23**, zero `@pending` remaining.
- **`board-lane-management`: 24/24** through the re-premised menu route.
- **Full acceptance suite: 68 features, 590 scenarios, 4141 steps — all green.**
- Unit/integration: 252 passing across the workspace.

### [REF] A real production defect the acceptance lane caught

`normalize_state` (`issues.rs`) folded every incoming state against a **closed,
hardcoded five-slug set**, and its own doc called that set *"the closed,
unmintable canonical slug set (D9)"*. D9 was `board-lane-management`'s decision
that lanes could not be added. **This feature makes slugs mintable, so that
premise became false the moment Insert shipped** — and until it was fixed, a
freshly inserted lane rendered a column on the board that **every write path
rejected**: dnd, the edit dialog, and `/api/v1` all answered "Invalid issue
state".

Fixed in `resolve_lane` by matching the project's **own** lane slugs first and
keeping the alias fold as a strictly-additive fallback. Both legacy aliases stay
load-bearing and are pinned by tests: `"in-progress"` (hyphenated — a spelling
no lane slug can ever have, since the CHECK forbids hyphens) is sent by shipped
feature files, and `"canceled"` is the one-l spelling.

This is the exact class of failure `board-lane-management`'s own D1 was written
about: an inherited assumption that stays true only while its premise holds.
Review would not have found it — it took a scenario that inserts a lane and then
tries to put a card in it.

### [REF] What mutation testing changed

| Scope | Result |
|---|---|
| `foundry-core` (`lane_slug`, `is_valid_lane_slug`) | **11/11 caught (100%)** after closing one gap |
| `foundry-services` (`validate_lane_label`, `resolve_lane`, `normalize_state`) | **12/12 caught (100%)** after removing dead code |

Both 100% figures are *after* acting on what the first runs reported, and both
findings were real:

1. **A surviving mutant in `is_valid_lane_slug`** — replacing the
   `^[a-z]` first-character guard with `true` survived everything. That function
   *is* this codebase's statement of the DB CHECK, and it was only ever exercised
   through `lane_slug`, which by construction never emits a bad first character.
   Two direct tests now pin the anchor and the tail alphabet.
2. **Three `normalize_state` arms turned out to be dead.** `"backlog"`,
   `"todo"` and `"done"` were identity mappings, and the exact-match arm added
   in the `resolve_lane` fix reaches them first — so deleting any of them broke
   nothing. Removed, leaving only the two aliases whose spelling is not a legal
   lane slug. Mutation testing did not just measure the tests here; it found
   code that had stopped doing anything.

**Honest scope note:** `resolve_lane_project` (shipped `board-lane-management`
code that this feature calls but did not write) showed two survivors under a
`--lib`-only run. Its coverage is real but lives in
`foundry-services/tests/delete_lane_use_case.rs` and the acceptance lane, both
excluded from that command for runtime reasons. Not a gap this feature opened,
and not counted in the figures above either way.

### [REF] Test defects the runs found (in the tests, not the feature)

Recorded because each would otherwise have been a silent hole:

1. **A false-GREEN at DISTILL** — the CSRF scenario passed over a feature that
   did not exist, because the shipped middleware refuses a tokenless POST to any
   mounted route. Rebuilt as a differential (without the token → refused; with
   it → accepted **and takes effect**).
2. **Two broken oracles at DISTILL** — a `notification_outbox` table that does
   not exist (it is `outbox`), and helpers passing a full URL to `signed_in_post`,
   which prepends `base_url()` itself.
3. **Over-broad menu selectors** — `//*[@data-lane-menu]//…` matches the same
   item in *every* column and returned Backlog's hidden one, giving
   `ElementNotInteractable`. Scoped to the open menu.
4. **Two races** — asserting on the page immediately after an async `hx-get`,
   and treating `wait_for_kb_ready` (which only asserts keyboard.js *initialised*)
   as proof the help overlay had opened.
5. **A scenario that asserted a card move without seeding a card.**
6. **An authz sweep that sent a malformed body to the shipped delete route**,
   so it measured form-parse order rather than the D11 refusal contract.
7. **A fail-open probe** — `menu_is_open` swallowed WebDriver errors as `false`,
   which would have *passed* the negative assertions it failed to evaluate. Now
   fail-closed: a probe that cannot run is a test failure.

### [REF] Quality gates

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | CLEAN |
| `cargo clippy --workspace --all-targets -- -D warnings` | CLEAN |
| `cargo xtask check-arch` | **PASSED** — incl. no-static-lane-list, single-slugify, every `/static` reference resolves, content-hash = own sha256 prefix, `VENDOR.md` recomputes, no colour literal outside token regions, token-set parity across all three regions |
| Full acceptance suite | 590/590 scenarios, 4141/4141 steps |
| Workspace unit/integration | 252 passing |
| Mutation (per-feature, ≥80%) | **100%** on both scoped runs |
| DES integrity | All 8 steps have complete traces |

`check-arch` earned its place twice: it caught three stylesheet references in
`lib.rs` that the rename would otherwise have left dangling, and it enforced the
`VENDOR.md` hash row.

### [REF] Commit status

**Nothing was committed or pushed.** The working tree holds the full change
(20 modified files, 8 new). The user has not asked for commits, and the repo has
precedent for a no-commit delivery (`issue-edit-modal-close-icon`). Pre-commit
gate, when wanted, is the full `cargo xtask ci`.

### [REF] Notable findings

1. **The `DEFERRABLE` measurement paid off end to end.** DESIGN predicted no
   migration by running the operation; DELIVER wrote the transaction exactly as
   measured and it worked first time, including the concurrency scenario. The
   recommended `check-arch` rule pinning that keyword is **not yet implemented**
   and is carried forward — until it exists, ADR-BOARD-LANE-003 is the only guard
   against a tidy-up migration silently breaking Insert.
2. **A shipped 422-before-authz shape, examined and left alone.** The delete
   route answers a fate-less POST with 422 before consulting the project, for any
   signed-in caller. It leaks nothing (the answer does not depend on whether the
   lane or project exists), so it is not an enumeration oracle — but it is worth
   recording as examined rather than missed.
3. **Two of the three slices needed no new tests beyond DISTILL's.** The
   scenarios written before any code existed described the feature accurately
   enough that DELIVER's job really was un-pending. The exceptions are recorded
   above and were all test defects, not missing coverage.

### [REF] Carried forward

- **`check-arch` rule pinning `DEFERRABLE`** on the `lanes` position constraint (`architecture-design.md` §6). Recommended by DESIGN, not implemented.
- **Reordering lanes** — the menu is now the natural home, and the position-shuffle machinery from 03-02 is exactly what it needs.
- **Undo a lane delete** — Insert makes deletion recoverable *by hand*; a real undo is still absent.
- **Sort by** — the screenshot's remaining item; needs a card-ordering concept the board does not have.
- Pre-existing, untouched: the `keyboard-shortcut-bindings` UI-5 IME retarget, ADR-008 trap-B inversion, and `#kb-search-panel` / `#kb-overlay-root` having no CSS.
