<!-- markdownlint-disable MD024 -->
# Feature Delta: board-lane-management

New projects start with the lanes Backlog, In-Progress and Done; any lane can
be deleted from the board, and deleting a lane that holds cards asks the
operator — in a dialog — whether to move those cards to another lane or delete
them.

## Wave: DISCUSS

### [REF] Prior Wave Consultation

| Artifact | Status | Note |
|---|---|---|
| `docs/product/jobs.yaml` | ✓ | Read. Three prior jobs; new job `job-board-lane-shaping` appended by this wave. |
| `docs/product/architecture/brief.md` | ✓ | Read. Invariants inherited below: slugs-are-identity, dialog close mechanism (BR-4), auth seam untouched. |
| `docs/product/vision.md` | ⊘ | Does not exist. |
| `docs/product/journeys/` | ⊘ | Directory does not exist. |
| `docs/project-brief.md` | ⊘ | Does not exist. |
| `docs/stakeholders.yaml` | ⊘ | Does not exist. |
| DISCOVER artifacts for this feature | ⊘ | None. Noted as risk: job grounded in code + operator context, not interviews. |
| DIVERGE artifacts for this feature | ⊘ | None. JTBD run inside this wave (below). |

### [REF] Persona

**Priya Raman — self-hosting operator and team member on her own boards.**
Runs foundry on her own cluster; member of team Backend (workspace Canzan
Labs), where the "Identity Platform" project (AUTH) lives, and creator of new
small boards for everything else. Her workflow has three stages — things she
might do, the thing she is doing, things done — but every board she creates
renders four columns, and "Todo" sits permanently empty between Backlog and
In-Progress. Marco (signed in, not a member of team Backend) is the authz foil.

- Persona ID: `persona-instance-operator` (same operator persona as prior
  features; no separate personas file exists in this repo).

### [REF] JTBD

**job_id: `job-board-lane-shaping`** (appended to `docs/product/jobs.yaml`)

One-liner: *When my board renders lanes my workflow never uses, I want new
boards to start with Backlog, In-Progress and Done and the power to delete a
lane — deciding on the spot whether its cards move or go — so each board reads
as my actual process, not the tool's default.*

All four stories below trace N:1 to this job.

### [REF] Locked Decisions

| ID | Decision | Rationale / source |
|---|---|---|
| D1 | **Premise corrections (code contradicts the working brief):** (a) the board renders **four** hardcoded columns — Backlog, Todo, In-Progress, Done (`DEFAULT_COLUMNS`, `projects.rs:49`) — not the five states of the DB CHECK; (b) `cancelled` is DB-valid and API-settable (`normalize_state`, `foundry-services/src/issues.rs:60`) but renders on **no** board column and appears in **no** edit-dialog option — cancelled issues are invisible on the board (search-only; the acceptance suite depends on this via `UNRENDERED_STATE = "cancelled"`, `keyboard_shortcut_bindings.rs:3891`); (c) there is **no keyboard state-move binding** — `keyboard.js` does navigation/selection only; state changes flow through the edit dialog, drag-and-drop, and `/api/v1` PATCH. All requirements below build on this corrected picture. | Code reading: `projects.rs:49,878-942`, `issue_edit_modal.html:13-18`, `keyboard.js` (no `data-state-url` consumer), `0001_init.sql:71-72`. |
| D2 | **Lanes become per-project data.** A deletable, per-project-varying lane set cannot be expressed by the hardcoded CHECK enum + `DEFAULT_COLUMNS` const. DISCUSS pins the observable requirements (each board renders its own lane list; dialog and API accept exactly that project's lanes); the storage shape (lane table vs hybrid, CHECK relaxation) is DESIGN's. | The feature request is unimplementable against `0001_init.sql:71-72` + `projects.rs:49` as data-free constants. |
| D3 | **Scope: per project.** Lanes are defined per project, not per team or workspace. Different boards serve different workflows (an ops board vs a reading list), and everything lane-adjacent today is already project-scoped: column render, `(project_id, state, position)` ordering (0012), the change report. | Persona need + `0012_issue_position.sql`. |
| D4 | **New defaults: Backlog, In-Progress, Done — in that order.** Projects created after this feature start with exactly these three lanes ("Todo" is dropped from defaults, per the user's words). | Feature request verbatim. |
| D5 | **Grandfathering, zero-surprise:** every existing project keeps the four lanes it renders today (Backlog, Todo, In-Progress, Done) — first render after migration is unchanged (mirrors 0012's zero-shuffle precedent). Additionally, a **Cancelled** lane is granted only to projects holding ≥1 cancelled issue at migration time, so no issue is left in a state with no lane (surfacing today's invisible cards is a fix, not a regression). Migration never deletes; the operator slims old boards herself with the new delete feature. | D1(b); watch-item R5 precedent in `0012_issue_position.sql`. |
| D6 | **Lane minimum and new-issue landing:** a project must always keep at least one lane; deleting the sole remaining lane is refused with an inline reason. New issues land in the project's **first (leftmost) lane** — replacing the hardcoded `DEFAULT 'backlog'` as an observable rule. Deleting the leftmost lane simply promotes the next lane; no lane is otherwise special or protected. | Simplest coherent rule; avoids a special "undeletable default lane" concept. |
| D7 | **Delete-time prompt (the user's words, pinned):** deleting a lane holding N ≥ 1 cards opens a dialog in `#modal-root` stating the lane name and exact card count, offering exactly two actions: **"Move all N to …"** (a picker listing the surviving lanes, leftmost preselected) or **"Delete all N permanently"**. An **empty** lane gets a confirm-only dialog (no fate choice to make — still a confirm, because removing a lane is destructive configuration). Card deletion is **permanent** (hard delete; cascades take comments, attachments and change history with each issue — the store's `delete_issue_cascade` shape; the comments-tombstone precedent is comment-scoped and does not extend here) and the dialog copy says so. ×/Esc cancels and changes nothing. Moved cards append to the **bottom** of the destination lane preserving their relative order (0012 position semantics), and each moved card writes one `status` change event in the same transaction (0013). Deleted cards' history vanishes with them (cascade FK design — accepted). | Feature request verbatim + `adr-modal-close-001`, `0006` (comment-scoped tombstone), `attachments.rs:184` (`delete_issue_cascade`), `0012`, `0013`. |
| D8 | **Ripple surfaces pinned in ACs:** board column render, drag-and-drop `data-column` targets, edit-dialog Status options, `/api/v1` PATCH state validation, change-report labels, and the new-issue landing lane all derive from the project's lane set — never from a static list. An unknown lane on any write path is a 422 validation refusal. `keyboard.js` board navigation already walks `[data-column]` generically and must keep working with any lane count. | D2; `normalize_state` DD10 precedent (one normalisation shared by HTML + JSON adapters). |
| D9 | **Out of scope: add, rename, reorder.** Deletion alone serves the validated job (slimming default-heavy boards). Consequence accepted and carried in dialog copy: a deleted lane cannot be recreated yet ("This cannot be undone"). "Add lane" is flagged as the natural successor feature (see triggered suggestions). | Orchestrator recommendation confirmed against the job: nothing in it needs add/rename/reorder. |
| D10 | **Authz:** lane deletion uses the same gate as every board mutation — team membership; outsiders and the signed-out get the uniform non-enumerable 404. The delete trigger and the dialog's confirm form are htmx mutating requests and MUST carry `_csrf`. No new role axis (lead-only lane admin) is introduced. | `is_team_member` gate on all board writes; repo CSRF contract. |
| D11 | **Walking skeleton: YES.** The enum→data shift runs under every board read/write; slice 01 is the thin end-to-end slice that swaps the foundation while existing boards render byte-identically — with one deliberate visible outcome (stranded cancelled issues surface, D5) so the skeleton is user-demonstrable, not infrastructure-only. | Orchestrator delegation; D1(b)/D5. |

### [REF] Journey (lightweight, happy path)

Emotional arc: **Problem Relief** — mild persistent irritation (a dead Todo
column on every board) → focused tension at the prompt ("what happens to my
cards?") → relief and ownership (the board reads as her flow; every card
explicitly accounted for).

```text
[Trigger]                     [Step 1]                        [Step 2]                            [Goal]
Todo has sat empty on    →    Priya clicks the delete    →    Dialog: "Delete lane 'Todo' —  →    Board shows Backlog,
"Identity Platform"           control on the Todo             it holds 3 issues." Move all         In-Progress, Done.
for months; AUTH-9 is         column header                   3 to [Backlog ▾] / Delete all        AUTH-12/15/18 sit at the
invisible in cancelled        Feels: focused                  3 permanently / ×                    bottom of Backlog; change
Feels: irritated              Sees: per-lane delete           Feels: tension, then safety —        report shows the moves.
("not my workflow")           affordance, count               the count and the choice are         Feels: relieved, in control
                                                              explicit. Picks "Move".              Sees: no dead column
```

Error paths: last-lane delete → 422 reason inline; non-member POST → uniform
404; ×/Esc at the prompt → nothing changes, no events written.

### [REF] Scope Assessment: PASS — 4 stories, 1 bounded context, estimated 3.5 days

Signals checked: 4 stories (≤10) | one module cluster (board surface:
`foundry-app` projects/issues + one store/services seam; `/api/v1` touched only
in its validation) | walking skeleton = 1 slice (≤5 integration points) |
~3.5 days total | one user outcome (boards match the operator's workflow).
No oversized signal fired; no feature split proposed.

### [REF] Shared Artifacts

| Artifact | Source of truth | Consumers | Risk |
|---|---|---|---|
| Project lane set (labels, slugs, order) | Per-project lane data (DESIGN owns shape) — replaces `DEFAULT_COLUMNS` + CHECK enum as UI source | Board columns, dnd `data-column` targets, edit-dialog Status options, `/api/v1` state validation, change-report labels, new-issue landing lane, delete-dialog destination picker | HIGH — any consumer still reading a static list re-creates invisible states (D1b) |
| Lane slug per lane | Existing state slugs (`backlog`, `todo`, `in_progress`, `done`, `cancelled`) carried over 1:1 at migration; no new slugs mintable (D9: no add) | `issues.state` values, `data-column` attributes, dnd POST body, API `state` field, 0013 event values | HIGH — slug drift would orphan issues from lanes |
| Card count in delete dialog | Live per-lane count at dialog render; re-resolved server-side at confirm (US-BLM-04 scenario 5) | Dialog copy ("it holds 3 issues"), bulk move/delete execution | MEDIUM — a stale count must never strand a card |
| CSRF token | `foundry_csrf` cookie + hidden `_csrf` field | Lane delete trigger, dialog confirm form (both htmx mutating) | HIGH — missing `_csrf` = silent 403 |
| Uniform 404 page | `resource_not_found_page` | Lane-delete GET/POST for non-members and the signed-out | MEDIUM — divergent error shape becomes an enumeration oracle |
| Card position within lane | `issues.position` per `(project_id, state)` (0012) | Board render order, dnd `after` semantics, bulk-move append order (D7) | MEDIUM — moved cards must keep relative order |

### [REF] User Stories

#### US-BLM-01: Every issue has a visible lane — lanes become the project's own data

##### Elevator Pitch

- **Before:** every board renders the same four hardcoded columns, and an issue moved to `cancelled` via the API vanishes from every board (findable only through search).
- **After:** opening `/team/backend/project/identity-platform` renders columns from that project's own lane list — the same four columns as yesterday, plus a "Cancelled" column holding AUTH-9 "Legacy SSO spike", visible for the first time.
- **Decision enabled:** what to do with long-invisible cards (triage AUTH-9 from the board) — and, from now on, which lanes each board should keep at all.

##### Problem

Priya is a team member on "Identity Platform" (AUTH). The board's columns are
a Rust constant and a DB CHECK, not data: she cannot change them, and issue
AUTH-9 — cancelled via the API months ago — renders on no board at all. She
finds it untrustworthy that the board silently shows a subset of the project's
issues, and the hardcoded lane set blocks the two behaviors she actually wants
(three-lane defaults, lane deletion).

##### Domain Examples

1. **Happy path (byte-identical grandfather)** — "Identity Platform" has AUTH-7 in `backlog`, AUTH-12/15/18 in `todo`, AUTH-3 in `in_progress`, AUTH-1 in `done`, none cancelled. After migration, the board renders Backlog, Todo, In-Progress, Done in the same order with every card in the same column at the same position. No Cancelled column appears.
2. **Edge (stranded card surfaces)** — "Homelab Ops" (OPS) holds OPS-9 "Replace UPS battery" in `cancelled` (set long ago via a machine token). After migration its board shows a fifth column, Cancelled, holding OPS-9; the edit dialog for OPS-9 now offers Cancelled among its Status options.
3. **Error/boundary (API respects the lane set)** — "Identity Platform" has no cancelled issues, so no Cancelled lane. A machine client PATCHes AUTH-7 to `"cancelled"`: 422 validation refusal, AUTH-7 unchanged — no card can enter a state its board cannot show.

##### UAT Scenarios (BDD)

###### Scenario: Existing boards render unchanged when lanes become data

- Given "Identity Platform" holds issues in backlog, todo, in_progress and done, and none in cancelled
- When Priya opens the board after the upgrade
- Then the columns are exactly Backlog, Todo, In-Progress, Done in that order
- And every card sits in the same column at the same position as before the upgrade

###### Scenario: A long-invisible cancelled issue gets a visible lane

- Given OPS-9 "Replace UPS battery" on "Homelab Ops" is in cancelled and today renders on no board column
- When Priya opens the "Homelab Ops" board after the upgrade
- Then a Cancelled column renders after Done, holding OPS-9

###### Scenario: The edit dialog offers exactly the board's lanes

- Given the "Homelab Ops" board renders Backlog, Todo, In-Progress, Done, Cancelled
- When Priya opens the edit dialog for OPS-9
- Then the Status options are exactly those five lanes, in board order

###### Scenario: A write to a lane the board does not have is refused

- Given "Identity Platform" has no Cancelled lane
- When a machine client PATCHes AUTH-7's state to "cancelled" via /api/v1
- Then the request is refused as a validation error (422) and AUTH-7 is unchanged

###### Scenario: Drag-and-drop still lands cards exactly where they are dropped

- Given the "Identity Platform" board with AUTH-12 in Todo
- When Priya drags AUTH-12 to the top of In-Progress
- Then AUTH-12 renders at the top of In-Progress, persists there on reload, and the change report records the status change

##### Acceptance Criteria

- [ ] Board columns, edit-dialog Status options, and `/api/v1` state validation all derive from the project's lane set; no static lane list remains on any read or write surface (D2/D8).
- [ ] Migration grandfathers every existing project with Backlog, Todo, In-Progress, Done, adding Cancelled only where ≥1 cancelled issue exists; first render is byte-identical for projects without cancelled issues (D5).
- [ ] After migration, zero issues are in a state with no rendered lane, and no write path (dialog, dnd, API) can put one there — unknown lane → 422 (D8).
- [ ] Existing dnd, card ordering (0012 position semantics), and change-event writes (0013) behave exactly as before against data-driven lanes.

##### Size

1 day | 5 scenarios | job_id: `job-board-lane-shaping`

#### US-BLM-02: New projects start with Backlog, In-Progress, Done

##### Elevator Pitch

- **Before:** every project Priya creates opens with four columns, and "Todo" starts its life as a dead column she scans past forever.
- **After:** creating "Reading List" (READ) and opening its board shows exactly three columns — Backlog, In-Progress, Done — and pressing `c` files READ-1 "Dune" straight into Backlog.
- **Decision enabled:** whether a new board needs any lane surgery at all — for her three-stage workflow, the answer becomes "no" at creation time.

##### Problem

Priya's workflow has three stages. Every board foundry creates has four
columns, so every new project starts with a lane she must mentally filter out.
She finds it wearing that the tool's default overrides her actual process on
day one of every project.

##### Domain Examples

1. **Happy path** — Priya creates "Reading List" (READ, team Backend). The new board renders exactly Backlog, In-Progress, Done, in that order, with the familiar empty-state prompt.
2. **Edge (first issue lands leftmost)** — She presses `c` and files READ-1 "Dune". It appears in Backlog — the leftmost lane (D6), not a hardcoded state.
3. **Boundary (API matches the three lanes)** — A machine client PATCHes READ-1 to `"in_progress"`: 200, card moves. The same client PATCHes it to `"todo"`: 422 — "Reading List" has no such lane.

##### UAT Scenarios (BDD)

###### Scenario: A new project's board opens with the three default lanes

- Given Priya creates project "Reading List" (READ) in team Backend
- When she opens its board
- Then the columns are exactly Backlog, In-Progress, Done in that order

###### Scenario: The first issue lands in the leftmost lane

- Given the fresh "Reading List" board
- When Priya files READ-1 "Dune"
- Then READ-1 appears as a card in Backlog

###### Scenario: The edit dialog offers exactly the three lanes

- Given READ-1 exists on "Reading List"
- When Priya opens its edit dialog
- Then the Status options are exactly Backlog, In-Progress, Done

###### Scenario: The API accepts the board's lanes and refuses the rest

- Given READ-1 is in Backlog
- When a machine client PATCHes its state to "in_progress" and then to "todo"
- Then the first request succeeds and moves the card, and the second is refused as a validation error with READ-1 still In-Progress

##### Acceptance Criteria

- [ ] Project creation seeds exactly the lanes Backlog, In-Progress, Done in that order (D4); no Todo or Cancelled lane exists on a new project.
- [ ] New issues land in the project's leftmost lane (D6) — observable as READ-1 appearing in Backlog.
- [ ] Dialog options and API validation for a new project reflect exactly its three lanes (D8).

##### Size

0.5 day | 4 scenarios | job_id: `job-board-lane-shaping`

#### US-BLM-03: Delete an empty lane

##### Elevator Pitch

- **Before:** the dead "Todo" column on grandfathered boards cannot be removed by any means short of a schema change.
- **After:** clicking the delete control on the empty Todo column of "Homelab Ops" and confirming in the dialog removes the column without a reload — the board now reads Backlog, In-Progress, Done, Cancelled.
- **Decision enabled:** which lanes each board keeps — decided per board, on the board, with a confirm that states what is being removed.

##### Problem

After US-BLM-01, Priya's grandfathered boards still carry the Todo lane her
workflow never uses. It holds nothing; it means nothing; she cannot remove it.
Every glance at every old board pays the same small tax.

##### Domain Examples

1. **Happy path** — Todo on "Homelab Ops" holds no issues. Priya clicks its delete control; a dialog confirms "Delete lane 'Todo'? It holds no issues. This cannot be undone." She confirms; the column disappears without a reload and stays gone on reload. The edit dialogs and API no longer offer `todo`.
2. **Edge (last lane refused)** — Priya has pared an experimental board "Scratch" (SCR) down to a single lane, Done. Deleting Done is refused: "A board needs at least one lane", inline, lane untouched.
3. **Edge (leftmost deleted → landing lane follows)** — On "Reading List" she deletes the empty Backlog lane; lanes are now In-Progress, Done. Filing READ-4 lands it in In-Progress — the new leftmost (D6).
4. **Error/authz** — Marco (signed in, not a member of team Backend) forges the lane-delete POST for Todo on "Homelab Ops": uniform 404, lane untouched.

##### UAT Scenarios (BDD)

###### Scenario: An empty lane disappears after an explicit confirm

- Given the Todo lane on "Homelab Ops" holds no issues
- When Priya clicks the lane's delete control and confirms in the dialog
- Then the Todo column is gone without a full page reload and remains gone on reload
- And the edit dialog no longer offers Todo and the API refuses "todo" for this project

###### Scenario: Backing out of the confirm leaves the board untouched

- Given the delete-lane dialog is open for Todo
- When Priya dismisses it with the × (or Esc)
- Then the dialog closes and the Todo column is still on the board

###### Scenario: The last lane cannot be deleted

- Given project "Scratch" has exactly one lane, Done
- When Priya attempts to delete Done
- Then she is refused with the inline reason "A board needs at least one lane" and the lane remains

###### Scenario: New issues follow the leftmost surviving lane

- Given Priya deleted Backlog on "Reading List", leaving In-Progress and Done
- When she files READ-4 "Children of Time"
- Then READ-4 appears in In-Progress

###### Scenario: Only team members can delete a lane

- Given Marco is signed in but not a member of team Backend
- When Marco sends the lane-delete request for Todo on "Homelab Ops" directly
- Then he receives the same uniform 404 page a never-existed path returns and the lane is untouched

##### Acceptance Criteria

- [ ] Each rendered lane carries a delete affordance; for an empty lane it opens a confirm-only dialog in `#modal-root` that closes via the declarative `data-action="close-modal"` mechanism — no new Esc listener (D7, BR-4).
- [ ] Confirmed deletion removes the lane from board render, dialog options, dnd targets and API validation, via an htmx swap without a full reload; persisted across reloads (D8).
- [ ] Deleting the sole remaining lane is refused with 422 and an inline reason routed into the dialog's `[data-error-slot]`; the lane survives (D6).
- [ ] After the leftmost lane is deleted, new issues land in the new leftmost lane (D6).
- [ ] Non-member/signed-out delete requests get the uniform non-enumerable 404 and change nothing; the trigger and confirm form carry `_csrf` (D10).

##### Size

1 day | 5 scenarios | job_id: `job-board-lane-shaping`

#### US-BLM-04: Deleting a full lane asks — move the cards, or delete them

##### Elevator Pitch

- **Before:** there is no way to remove a lane that holds cards, and no user-facing way to bulk-move or bulk-delete the cards in a lane at all.
- **After:** clicking delete on the Todo column of "Identity Platform" opens "Delete lane 'Todo' — it holds 3 issues", offering "Move all 3 to [Backlog ▾]" or "Delete all 3 permanently"; choosing Move leaves AUTH-12/15/18 at the bottom of Backlog and the Todo column gone.
- **Decision enabled:** the fate of every card in a dying lane — moved where she says, or knowingly deleted — decided card-count-in-hand, never silently.

##### Problem

Priya wants Todo gone from "Identity Platform", but it holds three real issues
(AUTH-12, AUTH-15, AUTH-18). She finds it unacceptable — and the job's core
anxiety — that removing a lane could silently eat or strand its cards the way
cancelled issues were stranded before US-BLM-01. She needs the tool to make
the fate of those cards her explicit, counted decision.

##### Domain Examples

1. **Move** — Todo on "Identity Platform" holds AUTH-12, AUTH-15, AUTH-18 (top to bottom). Priya deletes the lane and picks "Move all 3 to Backlog". The Todo column vanishes; the three cards append to the bottom of Backlog in the same relative order; the change report shows three status changes Todo → Backlog attributed to Priya.
2. **Delete** — On "Scratch" (SCR), the Done lane holds SCR-2 and SCR-5, both worthless spikes. She deletes the lane and picks "Delete all 2 permanently" (the dialog says it cannot be undone). Lane and both cards are gone from the board and from search.
3. **Edge (race on the count)** — While Priya's dialog says "3 issues", her own machine-token automation files AUTH-21 into Todo. She confirms "Move all to Backlog": all four cards in the lane at confirm time move — no card is stranded, even though the dialog's count was momentarily stale.

##### UAT Scenarios (BDD)

###### Scenario: Cards move to the chosen lane and keep their order

- Given Todo on "Identity Platform" holds AUTH-12, AUTH-15, AUTH-18 top to bottom
- When Priya deletes the Todo lane and chooses "Move all 3 to Backlog"
- Then the Todo column is gone and AUTH-12, AUTH-15, AUTH-18 sit at the bottom of Backlog in that relative order
- And the change report shows a status change Todo → Backlog for each of the three, attributed to Priya

###### Scenario: Cards are deleted only by an explicit, counted, permanent choice

- Given the Done lane on "Scratch" holds SCR-2 and SCR-5
- When Priya deletes the lane, reads "it holds 2 issues" and "this cannot be undone", and chooses "Delete all 2 permanently"
- Then the lane and both cards are gone from the board and neither issue is findable in search

###### Scenario: The prompt offers only surviving lanes as destinations

- Given "Identity Platform" has lanes Backlog, Todo, In-Progress, Done
- When Priya opens the delete dialog for Todo
- Then the destination picker lists exactly Backlog, In-Progress, Done with Backlog (the leftmost) preselected

###### Scenario: Backing out of the prompt changes nothing

- Given the delete dialog for Todo is open, showing its 3 issues
- When Priya dismisses it with Esc
- Then the Todo lane and all three cards are untouched and the change report records nothing

###### Scenario: A card filed mid-decision is still accounted for

- Given Priya's dialog for Todo says "3 issues" and AUTH-21 lands in Todo before she confirms
- When she confirms "Move all to Backlog"
- Then all four cards that were in Todo at confirm time are in Backlog and none is stranded laneless

##### Acceptance Criteria

- [ ] Deleting a lane with N ≥ 1 cards opens a dialog stating the lane name and live card count, with exactly two actions — move-all to a picked surviving lane (leftmost preselected) or permanent delete-all — plus ×/Esc cancel (D7).
- [ ] Move: cards append to the destination lane's bottom preserving relative order (0012 positions), each writing one `status` change event in the same transaction (0013); the lane is removed in the same operation — no intermediate laneless state observable (D7/D8).
- [ ] Delete: cards are permanently removed (with their comments, attachments and history, per cascade) and disappear from board and search; the dialog copy states permanence before the choice (D7).
- [ ] The fate applies to the lane's cards as resolved at confirm time, atomically — a concurrent filing can never strand a card (shared-artifact "card count").
- [ ] Cancel leaves lane, cards, positions and change history byte-identical (D7).
- [ ] Same authz/CSRF/error contracts as US-BLM-03 (D10).

##### Size

1 day | 5 scenarios | job_id: `job-board-lane-shaping`

### [REF] System Constraints

- Every lane-consuming surface (board render, dnd targets, dialog options, API validation, report labels, new-issue landing) reads the project's lane data; no static lane list may survive anywhere in the UI or adapters (D2/D8).
- The delete dialog is a `div.modal` htmx-swapped into `#modal-root`; close is declarative `data-action="close-modal"` only — registering a second `Escape` listener violates BR-4 (`adr-modal-close-001`).
- Mutating htmx triggers carry `_csrf`; refusals are 422 + bare fragment into `[data-error-slot]` (`form-errors.js` contract); authz refusals are the uniform non-enumerable 404 — never 401/403.
- Bulk moves write 0013 `status` events append-only in the same transaction as the mutation; lane delete + card fate are one atomic operation.
- Migrations are forward-only; next number is 0015; the grandfather backfill must be zero-surprise for boards without cancelled issues (D5), mirroring 0012's zero-shuffle discipline.
- Test lanes: HTTP acceptance lane for status/fragment/persistence contracts; fantoccini `@needs-browser` lane for dialog interaction, dnd and error-slot routing; per-feature mutation testing ≥80%.
- Known suite impact: `keyboard-shortcut-bindings.feature` line ~410 and `UNRENDERED_STATE = "cancelled"` (`keyboard_shortcut_bindings.rs:3891`) premise-break by design — after US-BLM-01 no state is unrendered. DELIVER must re-premise or retire that edge (tracked in Inherited commitments).

### [REF] Outcome KPIs

Objective: every board renders exactly the lanes its operator's workflow uses,
and no issue is ever invisible or silently destroyed along the way.

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| 1 | Board operators | Trim grandfathered boards to their real workflow (delete or start using the Todo lane) | ≥50% of pre-existing projects within 30 days of release | 0% possible (lane set is hardcoded) | SQL over lane rows + issue states; single-operator self-report | Leading |
| 2 | Board operators | Encounter zero invisible issues (state with no rendered lane) | 0, permanently | Every cancelled issue today (D1b) | Acceptance suite (US-BLM-01) + SQL guard query | Guardrail |
| 3 | Project creators | Start new projects needing zero lane surgery | 100% of post-release projects created with exactly 3 lanes | 0% (all start with 4 rendered + 1 hidden state) | SQL: lane count per post-release project; acceptance suite | Leading |
| 4 | Board operators | Lose cards to lane deletion only by the explicit counted "delete permanently" choice | 0 cards removed without that dialog choice | n/a (no deletion path exists) | `@needs-browser` acceptance lane (US-BLM-04); absence of missing-card reports | Guardrail |

Homelab-scale honesty: single-digit-operator instance; KPIs are verified by the
acceptance suite and SQL against the store, not analytics tooling.

### [REF] DoD

- All UAT scenarios green in the HTTP lane; dialog/dnd/error-slot scenarios green in the `@needs-browser` lane.
- Migration 0015 applies cleanly to a database with live data; boards without cancelled issues render byte-identically before/after (US-BLM-01 scenario 1 as the gate).
- Zero issues in a laneless state, provable by query, after every scenario run.
- Lane delete round-trip demonstrated live: grandfathered board → delete empty Todo → delete full lane choosing Move → change report shows the bulk move.
- `cargo xtask ci` green (check-arch, deny, mutation ≥80% on touched code); merged to main.

### [REF] Out of Scope

- Adding, renaming, or reordering lanes (D9 — flagged as successor feature).
- WIP limits, lane colors, per-lane settings of any kind.
- Per-team or per-workspace lane templates (D3: per-project only).
- Role-gated lane administration (lead/admin-only deletion) — D10 keeps the existing team-membership gate.
- A per-issue user-facing delete affordance (US-BLM-04 introduces bulk delete only as a lane-fate; single-issue delete is a separate feature).
- Undo/restore of deleted lanes or cards.
- Any change to priority values, issue keys, slugs, or URLs.

### [REF] WS Strategy

Walking skeleton locked IN (D11): US-BLM-01 is the skeleton — the enum→data
shift proven end-to-end (migration → board render → dialog → dnd → API) under
byte-identical behavior for untouched boards, with the cancelled-lane surfacing
as its demonstrable outcome. Delivery order US-BLM-01 → 02 → 03 → 04; every
later slice is UI + one write port on top of the skeleton's lane data.

### [REF] Driving Ports

New behavior the adapters need from the core (DESIGN owns shapes and placement):

1. **Lane list read** — per project: label, slug, order. Feeds board render, dialog options, dnd targets, API validation, report labels, delete-dialog picker.
2. **Per-project state validation** — replaces the static `normalize_state` acceptance set with "is this one of the project's lanes?" on every write path (HTML dialog, dnd POST, `/api/v1` PATCH), one normalisation shared by both adapters (DD10 precedent).
3. **Default-lane seeding** — project creation seeds Backlog, In-Progress, Done (D4).
4. **Grandfather migration (one-shot, 0015)** — four lanes per existing project + Cancelled where ≥1 cancelled issue exists (D5).
5. **New-issue landing rule** — leftmost lane replaces the hardcoded `'backlog'` default (D6).
6. **Delete-lane write, one atomic operation with three arms** — refuse-if-last (D6); delete-empty; delete-with-fate: move-all (append positions at destination bottom + one 0013 `status` event per card, same transaction) or delete-all (cascade removal per card, `delete_issue_cascade` shape) (D7).

### [REF] Pre-requisites

None outstanding: board, dialogs (`#modal-root` + declarative close), dnd,
`form-errors.js`, CSRF middleware, uniform-404 idiom, 0012/0013 substrates and
both test lanes are all shipped. The DB CHECK relaxation on `issues.state` is a
DESIGN-owned consequence of D2, sequenced inside US-BLM-01.

### [REF] DoR Validation

| DoR Item | US-BLM-01 | US-BLM-02 | US-BLM-03 | US-BLM-04 | Evidence |
|---|---|---|---|---|---|
| 1. Problem in domain language | PASS | PASS | PASS | PASS | Each Problem names Priya's concrete pain (dead Todo, invisible AUTH-9/OPS-9, uneatable lane) |
| 2. Persona specific | PASS | PASS | PASS | PASS | Priya Raman, team Backend member/operator; Marco as non-member foil |
| 3. 3+ domain examples, real data | PASS | PASS | PASS | PASS | Identity Platform/AUTH, Homelab Ops/OPS, Reading List/READ, Scratch/SCR; AUTH-7/9/12/15/18/21, OPS-9, READ-1/4, SCR-2/5 |
| 4. UAT 3–7 scenarios G/W/T | PASS (5) | PASS (4) | PASS (5) | PASS (5) | Embedded above; business-outcome titles throughout |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS | Each AC maps to ≥1 scenario and a D-decision |
| 6. Right-sized | PASS 1d | PASS 0.5d | PASS 1d | PASS 1d | ≤1 day per slice, 4–5 scenarios each |
| 7. Technical notes/constraints | PASS | PASS | PASS | PASS | System Constraints + Driving Ports + D1–D11 |
| 8. Dependencies tracked | PASS | PASS | PASS | PASS | 02–04 depend on 01 (lane data); 04 depends on 03 (delete affordance + dialog frame) |
| 9. Outcome KPIs measurable | PASS | PASS | PASS | PASS | KPI table with baselines and store-verifiable measurement |

DoR Status: PASSED (all 9 items, all 4 stories). Peer review by
`nw-product-owner-reviewer` not invoked in this lean subagent run — the
orchestrator gates handoff (same precedent as instance-admin-project-rename).

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| brief.md#names-are-labels | Slugs and issue keys are immutable identity; lane operations never touch project slugs, card URLs or issue keys | ADR-PROJECT-RENAME-001 | Lane slugs carry over the existing state values 1:1 (D5); no URL contains a lane |
| brief.md#dialog-layers | One close mechanism, declarative triggers, single `Escape` owner (BR-4) | ADR-MODAL-CLOSE-001 | Delete dialog (US-BLM-03/04) is template-only close wiring; no new listeners |
| 0012_issue_position.sql | Contiguous position per (project, state); zero-shuffle backfill discipline | n/a | Bulk move appends at destination bottom preserving order (D7); 0015 grandfather mirrors zero-shuffle (D5) |
| 0013_issue_change_events.sql | Append-only, same-transaction, one row per changed field | n/a | Each bulk-moved card writes one `status` event; cancel writes nothing (US-BLM-04) |
| Repo CSRF/error idioms | `_csrf` on mutating htmx triggers; 422 + `[data-error-slot]` fragment; uniform non-enumerable 404 | n/a | D10 + last-lane refusal AC (US-BLM-03) |
| foundry-store `delete_issue_cascade` | Issue removal is hard cascade (comments, attachments, history go with it) | n/a | D7 pins permanent deletion to this shape; no issue tombstone invented |
| Acceptance suite (keyboard-shortcut-bindings) | `UNRENDERED_STATE = "cancelled"` premise: exactly one state renders on no board | n/a | Premise-breaks by design after US-BLM-01; DELIVER re-premises or retires that edge |
| foundry-services DD10 | One state normalisation shared by HTML and JSON adapters | n/a | Per-project lane validation (Driving Port 2) must keep the single-seam property |

### [REF] Triggered suggestions (ask-intelligent, not expanded)

1. **Add / rename / reorder lane** — deletion is one-way without add (D9); the natural successor feature once an operator over-trims.
2. **Role-gated lane administration** — restricting lane deletion to team leads would be a new authz axis on board surfaces (D10 declined it here).
3. **Single-issue delete** — US-BLM-04 ships the first user-facing issue deletion, but only as a lane-fate; a per-card delete affordance is an obvious follow-on.
4. **WIP limits per lane** — adjacent kanban capability unlocked by lanes-as-data.
5. **Outcomes registry** — `docs/product/outcomes/registry.yaml` exists; the DISTILL wave should append this feature's KPIs per its convention.

## Wave: DISTILL

Agent: nw-acceptance-designer (Quinn) | Date: 2026-08-22 | Mode: scaffolded RED (ADR-025 — DISTILL authors ALL acceptance tests as per-scenario `@pending`; DELIVER only un-pends, one at a time). Lang: rust. Policy: inherit.

### [REF] Prior Wave Consultation

| Artifact | Status |
|---|---|
| `feature-delta.md` DISCUSS (D1-D11, US-BLM-01..04, 19 UAT scenarios 5/4/5/5) | ✓ read |
| `slices/slice-01..04` | ✓ read (counts verified 5/4/5/5) |
| `design/{architecture-design,component-boundaries,data-models,technology-stack}.md` | ✓ read |
| `docs/product/architecture/adr-board-lane-001/-002.md`, `brief.md` (lanes invariant) | ✓ read |
| `docs/product/jobs.yaml` (`job-board-lane-shaping`), `outcomes/registry.yaml` | ✓ read |
| `docs/architecture/atdd-infrastructure-policy.md` | ✓ read (inherit; lane rows appended) |
| `wave-decisions.md` (discuss/design/devops), DEVOPS artifacts, `journeys/`, `kpi-contracts.yaml` | ⊘ none (lean contract; no `@kpi` scenarios — warning logged) |

**Reconciliation: PASSED — 0 contradictions.** DESIGN's four recorded refinements REFINE, not contradict, DISCUSS: (1) `CreatedIssue.state` hardcoded echo = seventh static-list consumer (refines D8); (2) report labels for dead slugs fall back to `humanize_state` (refines D8); (3) the delete TRIGGER is a safe GET without `_csrf`, only the confirm POST mutates (refines D10 wording); (4) lane routes answer a same-workspace non-member with the uniform 404 while the board page 403s — deliberate asymmetry (refines D10). All four are pinned by scenarios/step oracles.

### [REF] Scenario table

24 scenarios in `crates/foundry-acceptance/tests/features/board-lane-management.feature`, ALL `@pending`. RED classification from one run with `@pending` stripped (`FOUNDRY_ACCEPTANCE_TAGS=blm`): **24/24 failed, all MISSING_FUNCTIONALITY, zero BROKEN, zero false-GREEN** (two false-GREENs were caught and fixed during classification — see Oracle discipline).

| Scenario | Slice | Lane | Tags (beyond @blm @pending) | RED classification |
|---|---|---|---|---|
| Existing boards render unchanged when lanes become data | 01 | HTTP | @us-blm-01 @walking_skeleton @driving_port @real-io | MF(schema): `lanes` relation absent (0015 DELIVER-owned) |
| A long-invisible cancelled issue gets a visible lane | 01 | HTTP | @us-blm-01 @driving_port @real-io | MF(schema) |
| The edit dialog offers exactly the board's lanes | 01 | HTTP | @us-blm-01 | MF(schema) |
| A write to a lane the board does not have is refused | 01 | HTTP+API | @us-blm-01 @error | MF(schema) |
| Drag-and-drop still lands cards exactly where they are dropped | 01 | HTTP | @us-blm-01 @real-io | MF(schema) |
| The upgrade grandfathers every existing board and can run twice safely | 01 | migration | @us-blm-01 @real-io @adapter-integration | MF(migration): canonical set lacks 0015 |
| A new project's board opens with the three default lanes | 02 | HTTP | @us-blm-02 @driving_port @real-io | MF(seeding): no lane rows on a created project |
| The first issue lands in the leftmost lane | 02 | HTTP | @us-blm-02 | MF(seeding) |
| The filing reply names the lane the issue actually landed in | 02 | API | @us-blm-02 @edge | MF(seeding) |
| The edit dialog of a new project offers exactly the three lanes | 02 | HTTP | @us-blm-02 | MF(seeding) |
| The board's lanes bound what any client may set | 02 | API | @us-blm-02 @error | MF(behaviour): PATCH "todo" answered 200, must be 422 — per-project validation absent (purest RED: assertion-level) |
| An empty lane disappears after an explicit confirm | 03 | HTTP | @us-blm-03 @driving_port @real-io | MF(schema) |
| Asking for the confirm dialog changes nothing by itself | 03 | HTTP | @us-blm-03 @edge | MF(schema) |
| The last lane cannot be deleted | 03 | HTTP | @us-blm-03 @error | MF(schema) |
| New issues follow the leftmost surviving lane | 03 | HTTP+API | @us-blm-03 @edge | MF(seeding) |
| Only team members can delete a lane | 03 | HTTP | @us-blm-03 @error @security | MF(schema) |
| A delete that does not carry the board's matching token is refused | 03 | HTTP | @us-blm-03 @error @security | MF(schema) |
| Cards move to the chosen lane and keep their order | 04 | HTTP | @us-blm-04 @driving_port @real-io | MF(schema) |
| Cards are deleted only by an explicit, counted, permanent choice | 04 | HTTP | @us-blm-04 @real-io | MF(schema) |
| The prompt offers only surviving lanes as destinations | 04 | HTTP | @us-blm-04 @edge | MF(schema) |
| Walking away from the prompt changes nothing | 04 | HTTP | @us-blm-04 @edge | MF(schema) |
| A card filed mid-decision is still accounted for | 04 | HTTP+API | @us-blm-04 @error @edge @real-io | MF(schema) |
| Deleting a full lane from the board is one visible, counted decision | 04 | browser | @us-blm-04 @needs-browser @driving_port @real-io | MF(schema) — fails before the browser opens; once 0015+markup land, the fantoccini legs run (chromedriver 151 = Chrome 151 on this host) |
| The delete dialog closes like every other dialog, leaving the board alone | 03/04 | browser | @us-blm-03 @needs-browser @edge @real-io | MF(schema), same note |

Error/edge/security share: 12/24 = 50% (≥40% target). Exactly ONE `@walking_skeleton` (D11).

### [REF] Adapter / driving-port coverage

Every DESIGN route/port has ≥1 scenario:

| DESIGN route / port | Scenario(s) |
|---|---|
| GET `/team/{t}/project/{p}/lanes/{l}/delete` (dialog) | safe-GET, picker, empty-confirm, Marco-GET, both browser scenarios |
| POST `…/lanes/{l}/delete` (confirm, `fate`+`destination`) | empty-delete, last-lane, move-fate, delete-fate, race, Marco-POST, CSRF leg, browser move |
| GET board (`board_view` → columns from lanes) | WS, cancelled-lane, three-defaults, every reload oracle |
| Edit-dialog GET (`IssueEditView.lanes`) | five-lanes dialog, three-lanes dialog, no-longer-offers-Todo |
| POST `…/issues/{n}/state` (dnd, `after` absent = top) | drag-and-drop scenario |
| `/api/v1` PATCH state (`validate_project_lane` via `change_issue_state`) | cancelled-refused, two-move, no-longer-offers-Todo, race |
| `/api/v1` POST issues (leftmost landing + `CreatedIssue.state` echo) | filing-reply-echo, leftmost-surviving, race |
| POST `/team/{t}/projects` (creation seeds 3 lanes) | three-defaults chain (real driving port, not SQL seed) |
| GET `…/report` (label resolution + fallback) | dnd report, move-fate report |
| GET `…/search` | delete-fate findability |
| Migration 0015 (grandfather + FK, `run_migrations_from_dir`) | migration oracle (staged 0001..0014 → canonical ×2) |
| `Store::delete_lane_with_fate` / `delete_lane_dialog` / `delete_lane` / `classify_lane_delete` / `list_project_lanes` / `board_view` / `validate_project_lane` | exercised through the routes above (port-to-port; internals never invoked directly) |

### [REF] Scaffolds (Mandate 7 — RED not BROKEN)

Marker: `SCAFFOLD: true`. Store/services bodies `panic!("… RED scaffold …")`; app handlers return a clean **501** (the `admin_tokens` precedent — a panic aborts the axum connection and masks the assertion; mounting the routes NOW also keeps the authz scenarios honest, since an unrouted path would answer the exact uniform 404 they assert).

| File | Contents |
|---|---|
| `crates/foundry-store/src/lanes.rs` (new) | `LaneRow`, `LaneDeleteFate`, `LaneDeleteOutcome`, `Store::{list_project_lanes, delete_lane_with_fate}` |
| `crates/foundry-store/src/lib.rs` | `pub mod lanes` + re-exports |
| `crates/foundry-services/src/lanes.rs` (new) | `LaneFate`, `LaneDialogView`, `DeleteLaneSuccess`, `DeleteLaneError`, `LaneDeleteDecision`, `delete_lane_dialog`, `delete_lane`, `classify_lane_delete` (pure heart, `classify_rename` idiom) |
| `crates/foundry-services/src/lib.rs` | `BoardLane`, `BoardView`, `board::board_view`, `pub mod lanes` |
| `crates/foundry-services/src/issues.rs` | `validate_project_lane` (DD10 per-project seam) |
| `crates/foundry-app/src/lanes.rs` (new) | `show_delete_lane_dialog`, `submit_delete_lane` (501 bodies) |
| `crates/foundry-app/src/lib.rs` | lane routes mounted UNDER `csrf_middleware` + `session_layer` |

Deliberately NOT scaffolded (behaviour-changing, DELIVER-owned): migration `0015_project_lanes.sql`; `insert_project` lane seeding; `insert_issue_with_outbox` leftmost resolution + `InsertedIssue` return type; `DEFAULT_COLUMNS`/`column_label_to_state` deletion; all template changes (`board.html` delete affordance + `id="board-columns"`, `partials/delete_lane_modal.html`, `issue_edit_modal.html` options loop); `IssueEditView.lanes`; report label resolution; the check-arch no-static-lane-list rule. Workspace green: `cargo check` / `fmt` / `clippy -D warnings` / `check-arch` / full acceptance default lane all pass with everything `@pending`.

### [REF] Test placement

`crates/foundry-acceptance/tests/features/board-lane-management.feature` + `crates/foundry-acceptance/src/steps/feature_board_lane_management.rs` (registered in `src/lib.rs` + `tests/acceptance.rs`) + `blm_*` world fields in `src/world.rs` — mirroring the freshest precedent (`instance-admin-project-rename`): same in-process axum + real session/CSRF + shared-testcontainer/per-scenario-schema harness; browser lane via `support::browser_harness` (fantoccini, chromedriver probe-then-refuse); migration oracle via `support::test_migration::stage_subset(14)` + `fresh_schema_pool_no_migrations` + `run_migrations_from_dir` (mwt-slice-05 precedent); machine legs via the Feature-A fixed-EdDSA-bearer idiom.

### [REF] Oracle discipline

1. **Lane-list oracle**: board/dialog expectations read lane rows BACK FROM the DB — the steps module holds NO static expected-column list (it would go green over the exact D8 static-list consumers). 2. **Move-fate ordering**: destination card order captured BEFORE confirm; after = before ++ moved, from stored rows AND rendered column. 3. **State-delta**: mutating scenarios snapshot the declared universe — lane rows (slug,label,position), issue rows (key,lane,position), change-event + outbox counts — and assert fail-closed (move fate: ONLY moved cards' (lane,position) may change). 4. **Non-enumerability**: refusals byte-identical to a never-existed path, GET and POST; the lane-route-404-vs-board-403 asymmetry pinned as chosen. 5. **Zero-laneless guard query** after every mutation. 6. **False-GREENs caught by the classification run and fixed**: (a) create-project fixture omitted `key_prefix` (422 — wrong RED, fixed); (b) two leftmost-landing scenarios passed against the legacy hardcoded `'backlog'` — oracles strengthened to derive "leftmost" from the lane ROWS (`position ASC LIMIT 1`), which reds pre-0015 and stays meaningful after. 7. **Race honesty**: the in-lane race leg drives the committed-before-confirm interleaving through real ports; the narrower mid-transaction window (insert between membership snapshot and lane DELETE) is NOT deterministically reachable from any port — the FK makes it a bounded retry, and the pinned observable is the zero-laneless guard. Documented as untestable in-lane; DELIVER may optionally add a store-level pause-seam test if it finds one worth the cost.

### [REF] Re-premised keyboard scenario (pre-registered risk)

`keyboard-shortcut-bindings` `@named-edge` "Enter is a no-op for a found issue that the board does not render" (premise: `UNRENDERED_STATE = "cancelled"`, `keyboard_shortcut_bindings.rs:3891`) — **RETIRED** this wave. No re-premise exists: post-slice-01 every state has a rendered lane and the composite FK makes findable-but-cardless structurally unreachable (KPI 2); post-0015 the Given's seed INSERT itself would be FK-refused, so the scenario could only falsely fail DELIVER. Removed: the scenario block (retirement note left in place in the `.feature`), the `UNRENDERED_STATE` const, and its dedicated Given step (`auth9_exists_in_an_unrendered_state`). Shared steps (`no modal opens`, `the browser does not navigate away`) survive — still used by three other scenarios. Successor coverage: "A long-invisible cancelled issue gets a visible lane" (the transformed premise: the card now EXISTS) + the surviving FR-9 no-selection no-op scenarios.

### [REF] Pre-requisites for DELIVER

1. **0015 sequencing**: migration 0015 is the FIRST deliverable of slice 01 — 15 of 24 scenarios red on the absent `lanes` relation before anything else can move. Verify the `issues_state_check` auto-generated constraint name via `pg_constraint` on live-data copy before relying on it (Earned Trust). 2. The acceptance suite seeds projects both via SQL (grandfather Givens seed lane rows explicitly — the post-migration shape) and via the real create port (fresh-board chain, which needs `insert_project` seeding); OTHER existing suites that seed projects via raw SQL will hit the FK when their seeded issues reference lanes that don't exist — DELIVER must sweep existing step modules' project/issue seeding when 0015 lands (known suite impact, System Constraints). 3. Un-pend order: slice 01 → 02 → 03 → 04, walking skeleton first; browser scenarios last within their slice. 4. `classify_lane_delete` is the property-test target (pure heart — layers 1-2 PBT per Mandate 9); acceptance stays example-only (layer 3+). 5. The check-arch no-static-lane-list rule (architecture-design.md §8) is DELIVER's to implement; the two exemptions are pinned. 6. Per-feature mutation ≥80% on touched code (DoD).

### [REF] SSOT / policy updates

- `docs/product/outcomes/registry.yaml`: OUT-3 (operation: board view from lane data), OUT-4 (operation: two-fate lane delete), OUT-5 (invariant: ≥1 lane + zero-laneless FK).
- `docs/architecture/atdd-infrastructure-policy.md`: Driving row (lane-delete web surface + oracles), Driven-internal row (`lanes` table + two-fate tx + migration oracle mechanism). No new fakes — every port in scope is driving (HTTP/browser) or driven-internal (Postgres), REAL per the Architecture of Reference.

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DISCUSS#D7/D8 | Lane delete + card fate one atomic operation; every lane surface derives from lane data | ADR-BOARD-LANE-002 | Scenario oracles read lane rows from the store and assert same-transaction events/outbox; no test-local lane list exists to mask a static-list consumer |
| DISCUSS#D10 + DESIGN refinement 3/4 | Safe-GET trigger without `_csrf`; uniform 404 on both lane-route verbs; deliberate 403-vs-404 asymmetry vs the board page | n/a | Dedicated scenarios pin the tokenless-POST refusal, the byte-identical GET+POST 404, and the asymmetry as chosen behaviour |
| DISCUSS#D11 | Slice 01 is the walking skeleton with the cancelled-lane surfacing as its demo moment | n/a | Exactly one `@walking_skeleton` scenario; the cancelled-card scenario is the stakeholder-demoable outcome |
| DESIGN component-boundaries §2-4 | Port signatures and scraper markers are the seam contract | n/a | Scaffolds reproduce the signatures verbatim; step constants pin `data-modal`/`data-lane-count`/`fate`/`destination`/`data-error-slot`/`data-lane-delete` |
| ADR-BOARD-LANE-001 | Composite FK is the no-stranded-card invariant | n/a | Migration oracle asserts `fk_issues_lane` exists AND bites; zero-laneless guard query runs after every mutating scenario |
| brief.md#dialog-layers (BR-4) | One close mechanism, declarative triggers only | ADR-MODAL-CLOSE-001 | Browser scenario closes via `data-action="close-modal"` and Esc through the single owner; HTTP oracle asserts the attribute is in the dialog markup |
| Acceptance suite (keyboard-shortcut-bindings) | `UNRENDERED_STATE` premise breaks by design | n/a | Retired this wave with recorded rationale (see Re-premised section); successor coverage named |
| 0012/0013 | Contiguous positions; append-only same-tx events | n/a | Move-fate oracle asserts append positions `C..C+N-1`, one status event + one outbox row per card; cancel asserts zero writes |
