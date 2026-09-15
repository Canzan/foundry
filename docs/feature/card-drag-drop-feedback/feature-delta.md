<!-- markdownlint-disable MD024 -->
# Feature Delta — card-drag-drop-feedback

Make the board's card drag trustworthy and legible. Drops keep working after the
board refreshes in place (the included bug, slice 01). The lane under a dragged
card lights up. A marker shows the exact slot the card will land in. Empty
lanes gain or lose their "No issues yet" placeholder truthfully.

Feature type: **user-facing**. Browser JS and CSS on the shipped board, plus the
acceptance steps that drive it; no handler, service, store or migration change
is expected (D9). Predecessors: `issue-status-move` (the card drag),
`card-ranking-within-status` (the positional drop), `board-lane-reorder` (the
`.lane-drop-indicator` precedent), `issue-card-delete` (whose popup delete,
`87282d3`, turned the bug into an everyday event). Evidence:
`rca-drag-after-board-replace.md` in this directory.

## Wave: DISCUSS / [REF] Prior Wave Consultation

| Source | Read | What it settled |
|---|---|---|
| `docs/product/jobs.yaml` | ✓ | No job covers moving a card. `job-board-lane-shaping` owns lane verbs, `job-issue-card-delete` owns card removal. The card move shipped in `issue-status-move` / `card-ranking-within-status`, which predate the SSOT and were deliberately not back-filled. This feature validates a **new** job, `job-board-card-move`. |
| `docs/product/journeys/journey-issue-card-delete.yaml` step 6 | ✓ | **Load-bearing.** Its first failure mode reads, verbatim: "The refresh re-renders columns but drops card drag bindings or resets the lane menu, trading a delete for a broken board." The failure was named and then shipped, because nothing drags after a refresh. See *Changed Assumptions*. |
| `docs/product/journeys/journey-theme-adoption.yaml` | ✓ | Unrelated (theming). No board or card-move journey exists; this wave creates `journey-card-drag-drop`. |
| `docs/product/personas/persona-instance-operator.yaml` | ✓ | Priya Raman reused ("works the board daily with both mouse and the shipped keyboard bindings"). No new persona. |
| `docs/product/outcomes/registry.yaml` | ✓ | OUT-3: dnd targets consume the lane set. OUT-11: the popup delete answers with the OOB `#board-columns` refresh. Prior features register OUT rows in **DISTILL**, so this wave adds none. |
| `docs/feature/issue-card-delete/feature-delta.md` + `slices/slice-02-delete-from-popup.md` | ✓ | Slice 02's hypothesis explicitly named "disturb card drag bindings" as the way it could fail, then recorded success. Format, persona use and the Backend/auth fixture (AUTH-41/42/43) are reused. |
| `docs/feature/board-lane-reorder/feature-delta.md` + `slices/slice-03-edge-autoscroll.md` | ✓ | D3 (HTML5 drag emits nothing on touch), D10 (a Pointer Events drag needs a `closeTopLayer()` arm), D16 (card and lane gestures must never become each other). The `.lane-drop-indicator` precedent and its teardown-on-every-exit AC. The Homelab Ops fixture (OPS). |
| `docs/feature/card-ranking-within-status/discuss/wave-decisions.md` + `slices/slice-02-cross-status-positional.md` | ✓ | D5 "extend the shipped `board-dnd.js` (no second JS file)"; D6 rank is set by drag only. The `after` wire format is theirs. |
| `docs/feature/issue-status-move/discuss/wave-decisions.md` + `slices/slice-02-drag-and-drop.md` | ✓ | D3: DnD is progressive enhancement; the edit dialog's Status is the non-pointer path. |
| `…/card-ranking-within-status/feature-delta.md`, `…/issue-status-move/feature-delta.md` | ⊘ | Not present. Both use the legacy multi-file layout; their DISCUSS `wave-decisions.md` was read instead. |
| `crates/foundry-app/static/js/board-dnd.js` | ✓ | `dragstart` is delegated on `document` (line 67); `dragover`/`drop` are bound **per lane at load** (lines 86-146). The only per-lane load-time binding in `static/js/`. |
| `static/js/board-lane-dnd.js`, `static/js/board-live.js` | ✓ | `applyBoard` replaces `#board-columns` client-side (144-152). `board-live.js` states the rule this feature generalises: "hold nothing. Every event re-queries the live document." |
| `templates/partials/board_columns.html`, `oob/board_columns_oob.html`, `issue_card.html` | ✓ | The placeholder `<p class="empty">No issues yet — press <kbd>c</kbd> to file the first one.</p>` (line 39) is the single source of its copy. One partial serves both render paths. |
| `crates/foundry-app/src/issues.rs` `ChangeStateForm` | ✓ | `after` absent or empty ⇒ drop at the **top** of the column (lines 180-185). |
| `static/css/foundry.52ad52fa.css` | ✓ | The palette has **no hue accent**: `--cz-black` is an inverting neutral (101-132). `--cz-line-strong` measures 1.49:1 against the page (87-89), yet `.lane-drop-indicator` is painted with it (1322-1327). |
| `crates/foundry-acceptance/src/steps/feature_board_lane_reorder.rs:1137` | ✓ | `drag_a_card` calls `open_board_in_browser` first, so listeners are always fresh. That is why no shipped scenario can see the bug. |
| `docs/product/vision.md`, `docs/project-brief.md`, `docs/stakeholders.yaml` | ⊘ | Not present in this repo. |
| `docs/feature/card-drag-drop-feedback/discover/`, `diverge/` | ⊘ | No DISCOVER or DIVERGE wave ran. The RCA is the evidence base. |

No DISCUSS decision contradicts prior evidence. One prior-wave *assumption* is
corrected by the RCA (*Changed Assumptions*).

## Wave: DISCUSS / [REF] Persona

**Priya Raman** (`persona-instance-operator`): the self-hosting operator who works
her boards daily with the mouse and the keyboard bindings. She drags cards
between lanes and to exact slots many times a session. Since `issue-card-delete`
shipped she also deletes duplicates from the card popup mid-triage. Since then,
the next drag after that delete is refused by every lane, and so is the next
drag after any lane edit, insert, move or delete. Her workaround is a reload,
which she has started doing reflexively before moving anything. **Marco**
(`persona-team-member-foil`) is not needed: no authz surface changes.

## Wave: DISCUSS / [REF] JTBD

**job_id: `job-board-card-move`**, a NEW job appended to `docs/product/jobs.yaml`.

One-liner: *When a card is in the wrong lane or the wrong place in its lane, I
want to drag it to exactly the spot I mean and see, before I let go, which lane
and which slot will take it, so the board reads in the order the work is really
in, without checking afterwards whether the drop landed or reloading first in
case it will not.*

| Force | |
|---|---|
| **Push** | Drops are silently refused after any in-place board refresh until a reload. The refresh comes from a popup card delete (since `87282d3`) or any lane edit, insert, move or delete. Nothing says which lane will take the card or where in it. "No issues yet" lingers after a drop into an empty lane and is missing from a lane just emptied. |
| **Pull** | What every board tool does: the lane lights up, a line opens between two cards, and the card drops into that gap. |
| **Anxiety** | That the new highlights flicker or linger after the drag. That a file dragged in from the desktop starts moving cards. That the marker shows one place and the card lands in another. |
| **Habit** | She already drags daily, and cards already land at the slot under the cursor. Feedback must add to that gesture and change nothing about it: same drop semantics, same request, same revert. |

Opportunity: **importance high** (the board's core daily gesture), **satisfaction
low** (silently broken after everyday actions). All four stories trace N:1 to
`job-board-card-move`.

## Wave: DISCUSS / [REF] Locked Decisions

| ID | Decision | Rationale / source |
|---|---|---|
| D1 | **User-facing, brownfield, no walking skeleton, lightweight journey, JTBD on.** | User decisions 1-4 (2026-09-13). |
| D2 | **The bug is slice 01, test-first.** Its first scenario is a `@needs-browser` regression. It replaces `#board-columns` in place, then drags **without navigating or reloading**. Its oracle: the synthetic `dragover` dispatched on the replacement lane is `defaultPrevented`. It must be **RED on HEAD (`aa8a6f6`) for that reason** before any production change. It must not reuse `drag_a_card`, whose reload re-binds listeners and hides the bug. | RCA §Test gap; `feature_board_lane_reorder.rs:1137-1138`. |
| D3 | **Replace-proof by construction.** Every card-drag behaviour resolves its lane from the live document **at event time**: accepting a drop, lane activation, the marker, placeholder upkeep. Nothing is bound per lane at load. No lane or card node is held across events, except the one dragged card and its origin for the life of one drag. | RCA §Fix direction; `board-live.js` constraint 2; ADR-BOARD-LANE-005 rule 2 ("DOM-derived state"). |
| D4 | **Only a card drag that began on this page is a card drag, and the board swallows every other drop.** A file, a text selection, a link, or a card dragged in from another foundry tab never activates a lane, shows a marker, changes a placeholder or sends a request. Dropped anywhere on the board (`#board-columns`), it **does nothing**: no move and no navigation (the tab must not open the file). This holds on a fresh load **and** after any board replace. Today only the lanes of a freshly loaded board swallow such a drop, by accident of the per-lane binding; after a replace, and between lanes, the browser's default applies. | User decision Q2 (2026-09-13); user constraint; RCA ("guard on `dragged !== null`"). |
| D5 | **Every drag ends clean.** All exits leave zero activated lanes and zero markers, and clear the in-flight drag state: drop on a lane, drop anywhere else, `Escape`, and a refused or failed POST. A later drag is therefore never mistaken for the last card. `Escape` here is the **browser's native drag cancel**: no `keydown` listener is added and `closeTopLayer()` keeps sole ownership (BR-4 untouched). This contrasts with `board-lane-reorder` D10, whose Pointer Events drag needed an arm. | RCA ("reset state on `dragend`"); `brief.md` §dialog-layers. |
| D6 | **Lane activation uses existing colour tokens only.** The palette has no hue accent, so "the lane changes colour" means a visible change of the lane's surface and boundary drawn from existing `--cz-*` tokens. It must be legible in both palettes, with the boundary **≥3:1 against the page** (WCAG 1.4.11). `--cz-line-strong` alone cannot carry it (1.49:1). Activation must not reflow: no card or column moves or resizes when a lane activates. **No new hue token**: activation is a stronger surface and boundary change from existing tokens, and slice 02 stays ~0.5d. | User decision Q1 (2026-09-13); `foundry.css:87-89, 101-132`; check-arch S1. |
| D7 | **The marker and the landing are one computation.** One slot calculation (today's `insertBeforeTarget` + `neighbourAbove`) gives three things: the slot the marker shows, the slot the card lands in optimistically, and the `after` key the POST sends. They cannot disagree. The dragged card's own slot is never counted as a neighbour. | `board-dnd.js:36-58, 115-125`. |
| D8 | **The marker is a rule between cards, not a ghost card**, the shape of `.lane-drop-indicator`. Showing it must not move the slot under a still pointer (no oscillation). Its colour does **not** copy the precedent: it must reach ≥3:1 against the lane surface in both palettes. | `foundry.css:1320-1327`; the precedent's own comment ("so the board's own geometry does not shift"). |
| D9 | **The drop contract is unchanged.** Same `POST /team/{t}/project/{p}/issues/{n}/state`, same `state` + `after` (omitted ⇒ top), same `x-csrf-token` header, same optimistic move and exact-origin revert. No handler, service, store or migration change. | `issues.rs:177-215`; `reposition_issue_with_outbox` (gap-free `0..N-1`). |
| D10 | **Empty lanes are truthful on both sides of a drop.** A drop into an empty lane removes its placeholder. A drag that takes a lane's last card out gives it one. A refused or failed drop restores both lanes exactly. The placeholder's words have **one source**, the server template. The client never carries a second copy that could drift. Hovering never changes a placeholder; only a drop does, or a card leaving the lane by a remote delete (D16). | `board_columns.html:39`; RCA finding 3. |
| D11 | **After any successful drag, a reload changes nothing visible in the lanes involved.** The optimistic board must equal the server's render. This is the oracle for D7 and D10. | `issues.rs` persists order; a reload is ground truth. |
| D12 | **Synthetic events prove wiring; a human proves feel.** The WebDriver lane can only dispatch synthetic drag events. So every slice names a **manual real-browser dogfood check** on a real board and records it in the slice's delivery notes. `@needs-browser` scenarios run in `all` / `cargo xtask ci`, not in the default lane. | User constraint; `tests/acceptance.rs`. |
| D13 | **CSS: tokens only, rehashed per slice.** Every CSS change renames `foundry.<hash>.css` across `base.html`, the `lib.rs` tests and `static/VENDOR.md` in the same change. Slices 02 and 03, and 04 if styled, each carry their own rename, because each ships on its own. | check-arch S1; precedent `1d91ad8`. |
| D14 | **Pointer gesture only.** HTML5 drag emits nothing on touch, and cards have no keyboard move; the edit dialog's Status is the non-pointer path. This feature adds feedback to the gesture that exists. It adds no touch drag, keyboard move or live-region announcement. | `board-lane-reorder` D3; `issue-status-move` D3. |
| D15 | **No build-time guard; the refresh-then-drag browser scenarios are the guard.** No check-arch rule forbids per-lane listeners bound at load. The US-CDF-01 regression and its outline, plus the "works on lanes replaced or inserted in place" scenario each feedback slice carries, fail on exactly the behaviour a rule would police: a drop, highlight, marker or placeholder that stops working after a refresh. A static rule would police one code shape (`querySelectorAll("[data-column]")` + `addEventListener` at load). It would miss other shapes of the same fault, such as a stored lane node, and add a check-arch rule to maintain for one script. | User decision Q4 (2026-09-13); RCA root-cause item 5. |
| D16 | **A lane emptied by a remote delete also shows its placeholder**, folded into slice 04. `board-live.js` drops a card on `IssueDeleted`. If that leaves the lane holding no card, the lane shows "No issues yet" from the same single source as D10, re-queried from the live document ("hold nothing"). Slice 04 grows from 0.5d to 0.75d, still within the 1-day cap. *DESIGN note: DDD-5 makes this behaviour declarative, with `board-live.js` unchanged, so slice 04 returns to 0.5d. The behaviour D16 requires is unchanged.* | User decision Q3 (2026-09-13); `board-live.js:82-92`. |

## Wave: DISCUSS / [REF] Journey — `journey-card-drag-drop` (lightweight)

**Persona:** Priya Raman · **Job:** `job-board-card-move` · **Platform:** web ·
SSOT: `docs/product/journeys/journey-card-drag-drop.yaml`

Arc: **Problem Relief.** Distrust → oriented → precise → confident → trust.

| # | Step | Sees | Feels |
|---|---|---|---|
| 0 | She has just deleted duplicate AUTH-42 from its popup; the board refreshed in place | Backlog: AUTH-41, AUTH-43 · In-Progress: AUTH-3, AUTH-12, AUTH-19 · Done: AUTH-7 | **Wary**: last time, the next drag was refused until she reloaded |
| 1 | Picks up AUTH-41 | The card lifts (browser drag image) | Neutral |
| 2 | Carries it over In-Progress | In-Progress is activated; Backlog, which she left, is quiet | **Oriented**: the board is answering |
| 3 | Aims between AUTH-3 and AUTH-12 | A line opens between those two cards | **Precise**: she knows where it goes before letting go |
| 4 | Releases | AUTH-41 sits where the line was; nothing is lit; no line remains | **Confident** |
| 5 | Later reloads, or glances back | Same order: AUTH-3, AUTH-41, AUTH-12, AUTH-19 | **Trust**: no reflex reload next time |

```text
+-- Identity Platform (AUTH) --------------------------------------------+
|  BACKLOG           ##IN-PROGRESS########     DONE                       |
|  +-----------+     # +-----------+     #     +-----------+              |
|  | AUTH-43   |     # | AUTH-3    |     #     | AUTH-7    |              |
|  +-----------+     # +-----------+     #     +-----------+              |
|                    # =============  <--# marker: AUTH-41 lands here      |
|   (AUTH-41 is      # +-----------+     #     after = AUTH-3             |
|    being dragged)  # | AUTH-12   |     #                                |
|                    # +-----------+     #                                |
|                    # | AUTH-19   |     #  ## = activated lane (D6)      |
|                    ########################                             |
+-------------------------------------------------------------------------+
```

| Error path | What she sees |
|---|---|
| Board replaced in place before the drag (**the bug**) | The drop is claimed and lands exactly as on a fresh load (US-CDF-01) |
| A file (`keys.png` from Finder), a text selection, or AUTH-41 dragged from a second tab | Nothing lights, no line, no move, and the tab does not open the file, on a fresh load or after a refresh (D4) |
| `Escape`, or release over the page header | The card stays in its origin slot; nothing lit; no line; placeholders unchanged (D5) |
| Refused (another operator deleted the card: uniform 404) or network failure | The card returns to its exact origin slot; both lanes' placeholders are as before (D9, D10) |
| Drop into an empty lane / last card dragged out | Placeholder removed / placeholder shown (US-CDF-04) |
| Another tab deletes the last card in a lane | That lane shows its placeholder, without a reload (US-CDF-04, D16) |

## Wave: DISCUSS / [REF] Scope Assessment: PASS — 4 stories, 1 context, ~2.5 days (DESIGN-revised from ~2.75)

**4 stories** (≤10). **One bounded context**: the board page's browser tier
(`board-dnd.js`, `board-live.js`, the stylesheet, possibly `board_columns.html`), with no server
context touched (D9). **No walking skeleton.** **~2.5 days** (≪2 weeks; DISCUSS said ~2.75, and DESIGN DDD-5 took slice 04 from 0.75d to 0.5d). The
four outcomes do ship separately (one signal), but they share one gesture and
one job. One signal fired against a threshold of 2+, so no split is proposed.

## Wave: DISCUSS / [REF] Shared Artifacts

| Artifact | Single source of truth | Consumers | Risk |
|---|---|---|---|
| `#board-columns` | `board.html` + `oob/board_columns_oob.html` (server); `board-lane-dnd.js::applyBoard` (client) | **Replaced in place** by: popup card delete, lane edit / insert / delete, lane move from the ⋯ menu. A lane-header drag only rearranges the lanes in place (DISTILL Upstream Issue #1) | **HIGHEST**: the root cause. Anything captured before a replace is detached after it (D3) |
| `${lane_slug}` (`data-column`) | `lanes` rows → `board_columns.html` | Lane resolution at event time; the POST's `state` | HIGH: must be read from the live lane under the pointer, never from a stored node |
| `${after_key}` | `data-issue-key` of the card immediately above the computed slot | Marker position, optimistic landing, POST `after` | HIGH: one computation for all three (D7) or the marker lies |
| In-flight drag state (card, origin, origin-next) | Set on a `dragstart` from an `.issue-card` on this page | The D4 guard, activation, marker, revert, placeholder restore | HIGH: must clear on every exit (D5) or a later foreign drag is mistaken for a card |
| Activated-lane / marker presence | The live DOM (a class / an element), never a stored handle | Teardown on every exit; DOM-count oracles | MEDIUM: a stored handle is detached by a replace, and teardown then no-ops with feedback on screen |
| Placeholder copy | `board_columns.html:39` | Fresh render, OOB render, client-side restore after a drag, remote-delete upkeep in `board-live.js` (D16) | MEDIUM: a hard-coded client copy drifts from the template (D10) |
| CSRF token | `foundry_csrf` cookie → `x-csrf-token` header | The drop POST (unchanged) | LOW: untouched by D9 |
| Stylesheet | `static/css/foundry.<hash>.css` | `base.html`, `lib.rs` tests, `static/VENDOR.md` | MEDIUM: the rename must land in one change (D13) |
| Colour tokens | `:root` + the two dark-palette regions | Activation surface/boundary, marker | MEDIUM: literals are a check-arch S1 failure; contrast must be measured in both palettes |

## Wave: DISCUSS / [REF] User Stories

### US-CDF-01: Card drops keep working after the board refreshes in place

`job_id: job-board-card-move` · Slice 01 · **Bug, test-first**

#### Elevator Pitch

- **Before:** after Priya deletes a card from its popup, or edits, inserts, moves or deletes a lane, every lane refuses the next card she drags until she reloads the page.
- **After:** on the Identity Platform board, delete AUTH-42 from its popup, then drag AUTH-41 from Backlog onto In-Progress without reloading → sees AUTH-41 land in In-Progress, still there after a reload.
- **Decision enabled:** she can triage in one sitting (delete, reshape, move) without deciding whether to reload first "just in case".

#### Problem

The card drag was written before the board could refresh itself. It wires each
lane once, at page load. Five everyday actions now replace the lanes in place,
and the replacements are never wired, so they refuse every drop. There is no
error and no request, and the card just snaps back. `87282d3` made the trigger
something Priya does several times a session.

#### Domain Examples

1. **Popup delete, then move.** Priya deletes duplicate AUTH-42 from its popup; the board refreshes. She drags AUTH-41 from Backlog to In-Progress. Today: refused. After: it lands and persists.
2. **Lane menu, then move into a lane that did not exist at load.** On Homelab Ops she uses ⋯ **Insert list after** on In-Progress to add "Review", then drags OPS-7 into Review. Today: refused (RCA case C). After: accepted.
3. **Foreign drags.** She drags `keys.png` from Finder onto Done, or drags AUTH-41 in from the same board open in a second tab, and drops it. Nothing moves, no request is sent, and the tab stays on the board rather than opening the file. This holds on a fresh load and after a refresh alike.

#### UAT Scenarios

```gherkin
@us-cdf-01 @needs-browser @driving_port @real-io
Scenario: A card can still be dropped after the board rearranges itself in place
  Given Homelab Ops is open in a browser with lanes Backlog, Staging, In-Progress and Done
  And OPS-7 is in In-Progress
  And Priya has moved Staging right from its lane menu, so the board refreshed without a reload
  When Priya drags OPS-7 over Done without reloading the page
  Then Done accepts the drag
  And after dropping, OPS-7 is in Done and is still in Done after a reload

@us-cdf-01 @needs-browser @driving_port @real-io
Scenario Outline: Every in-place board refresh leaves every lane accepting drops
  Given the Identity Platform board is open in a browser
  And Priya <refreshes the board in place>
  When Priya drags AUTH-41 into <destination> without reloading the page
  Then AUTH-41 is in <destination>, and still there after a reload
  Examples:
    | refreshes the board in place                     | destination |
    | deletes AUTH-42 from its popup                   | In-Progress |
    | renames Done to "Shipped" from its lane menu     | Shipped     |
    | inserts a list "Review" after In-Progress        | Review      |
    | deletes the Staging list, moving its cards       | Done        |
    | drags the Done header left of In-Progress        | Done        |

@us-cdf-01 @needs-browser
Scenario: A freshly loaded board drags exactly as before
  Given the Identity Platform board has just been loaded
  When Priya drags AUTH-41 between AUTH-3 and AUTH-12 in In-Progress
  Then the move request carries state "in_progress" and names AUTH-3 as the card above
  And AUTH-41 sits between AUTH-3 and AUTH-12 after a reload

@us-cdf-01 @needs-browser @error
Scenario Outline: Something dragged in from outside the page is swallowed by the board
  Given the Identity Platform board is open in a browser, <board state>
  When <something> is dragged from outside the page and dropped on <where>
  Then nothing on the board moves and no move request is sent
  And the tab still shows the Identity Platform board
  Examples:
    | board state                                | something                         | where                                 |
    | freshly loaded                             | a file "keys.png"                 | Done                                  |
    | after Priya deleted AUTH-42 from its popup | a file "keys.png"                 | Done                                  |
    | after Priya deleted AUTH-42 from its popup | a text selection from another app | the gap between Backlog and In-Progress |

@us-cdf-01 @needs-browser @error
Scenario: A cancelled card drag is not mistaken for the next drag
  Given Priya started dragging AUTH-41 and cancelled it with Escape
  When a file "keys.png" is then dragged over In-Progress and released
  Then nothing on the board moves and no move request is sent
```

#### Acceptance Criteria

- **AC-1.1** After an in-place board replacement, a card drag over any lane is claimed by that lane and the drop moves the card, with no page reload. This holds for all 5 in-place board refreshes: popup card delete; lane edit, insert and delete; lane move from the ⋯ menu. It also holds after the 1 lane-header reorder, kept as a robustness guard: a header drag rearranges the existing lanes without replacing them (DISTILL Upstream Issue #1). In the browser lane the oracle is that the synthetic `dragover` on the replacement lane is `defaultPrevented`.
- **AC-1.2** The first scenario above is written first and is **RED on HEAD** because the `dragover` is not `defaultPrevented`, recorded in DISTILL's RED classification. No step in it navigates or reloads between the replace and the drag (D2).
- **AC-1.3** A lane that exists only after the replace (inserted from the ⋯ menu) is a valid drop target immediately.
- **AC-1.4** The drop contract is byte-identical: same URL, `state=<slug>`, `after=<key above>` (omitted at the top), same `x-csrf-token` header; same optimistic move; same exact-origin revert on non-2xx or network error (D9).
- **AC-1.5** A drag that did not begin on an `.issue-card` on this page never moves a card and never produces a request: a file, a text selection, or a card from another tab. Dropped anywhere on the board, it is **swallowed**: the tab does not navigate or open the file. This holds on a fresh load and after each in-place refresh. In the browser lane the oracle is a synthetic `dragover` and `drop` carrying a `File` in the `DataTransfer`: both are `defaultPrevented`, no request is logged, and the URL is unchanged. The real Finder drop is confirmed at dogfood (D4, D12).
- **AC-1.6** In-flight drag state is cleared on every drag end: drop on a lane, drop elsewhere, `Escape` (D5).
- **AC-1.7** The shipped card-drag scenarios (`board-lane-reorder.feature:229-235`, `keyboard_shortcut_bindings.rs:3013`), lane-drag scenarios and popup-delete scenarios stay green and **unmodified**.

#### Technical Notes

`board-dnd.js:86-146` binds per lane at load; `dragstart` (line 67) is already
delegated. The RCA's fix direction is event-time lane resolution
(`closest("[data-column]")`), a guard on an in-flight card, and a reset on drag
end. It was proven in the containerised Chrome repro with the POST contract
unchanged. Four of the five in-place refreshes are htmx OOB swaps from the server
(`issues.rs` `submit_delete`, `lanes.rs` edit/insert/delete). The fifth, the ⋯
menu Move, is applied client-side (`board-lane-dnd.js::applyBoard`) from
`lanes.rs` move's response. The header drag reorders the existing lane nodes and
replaces nothing (DISTILL Upstream Issue #1). Swallowing a foreign drop means the board
claims the drag without acting on it. The mechanism is DESIGN's; the observable
requirement is AC-1.5.

### US-CDF-02: See which lane will take the card

`job_id: job-board-card-move` · Slice 02

#### Elevator Pitch

- **Before:** while dragging a card, nothing on the board says which lane will take it. She finds out by letting go.
- **After:** drag OPS-3 over the Done lane on Homelab Ops → sees Done visibly activate, and nothing else lit. Move on to Staging → Done goes quiet and Staging activates.
- **Decision enabled:** whether to release here or keep moving: judged before the drop, not corrected after it.

#### Problem

On a four-lane board with narrow columns, the lane boundary is a hairline, and
the drop lands in whichever lane the pointer is over at release. Priya aims by
feel and checks afterwards. The lane under the card should answer "me" while
the card is over it.

#### Domain Examples

1. **Happy path.** Dragging OPS-3 from Backlog across Staging to Done: Staging activates then clears; Done activates; releasing clears everything.
2. **Over its own cards.** Crossing OPS-9 inside Done keeps Done active; it does not blink off and on as the pointer passes over a card.
3. **Foreign drag.** `keys.png` dragged over Done lights nothing (D4).

#### UAT Scenarios

```gherkin
@us-cdf-02 @needs-browser
Scenario: The lane under a dragged card lights up, and only that lane
  Given Homelab Ops is open with OPS-3 in Backlog and OPS-9 in Done
  When Priya drags OPS-3 over Done
  Then Done is shown as activated
  And no other lane is shown as activated

@us-cdf-02 @needs-browser
Scenario: The highlight follows the card from lane to lane
  Given Priya is dragging OPS-3 over Done
  When she moves it on over Staging
  Then Staging is shown as activated and Done is not

@us-cdf-02 @needs-browser
Scenario: A lane stays lit while the card passes over the cards inside it
  Given Priya is dragging OPS-3 over Done
  When the pointer passes over OPS-9 inside Done
  Then Done is still shown as activated

@us-cdf-02 @needs-browser
Scenario Outline: Every way a drag ends leaves no lane lit
  Given Priya is dragging OPS-3 over Done
  When she <ends the drag>
  Then no lane is shown as activated
  Examples:
    | ends the drag                    |
    | drops it on Done                 |
    | presses Escape                   |
    | releases it over the page header |

@us-cdf-02 @needs-browser @error
Scenario: A file dragged over a lane lights nothing
  Given Homelab Ops is open
  When a file "keys.png" is dragged from outside the page over Done
  Then no lane is shown as activated

@us-cdf-02 @needs-browser @real-io
Scenario: Lanes still light up after the board rearranges itself
  Given Priya has moved Staging right from its lane menu without reloading
  When she drags OPS-3 over Done
  Then Done is shown as activated
```

#### Acceptance Criteria

- **AC-2.1** While a card drag is over a lane, exactly that lane is activated; while it is over no lane (including outside the window), none is.
- **AC-2.2** Passing over a card inside the activated lane does not deactivate it.
- **AC-2.3** Every exit (drop, `Escape`, release outside any lane, refused or failed POST) leaves **zero** activated lanes (D5).
- **AC-2.4** Foreign drags never activate a lane (D4).
- **AC-2.5** The activated state is distinguishable in **both** palettes, drawn from existing `--cz-*` tokens only, with no new hue token. Its boundary measures ≥3:1 against the page in both, by the suite's contrast oracle (D6).
- **AC-2.6** Activating a lane moves no card and resizes no column: bounding rectangles before and after activation are identical.
- **AC-2.7** Activation works on lanes that were replaced or inserted in place (D3).
- **AC-2.8** The stylesheet is renamed to its new content hash across `base.html`, the `lib.rs` tests and `static/VENDOR.md` in the same change (D13).

#### Technical Notes

This is the classic HTML5 DnD trap: `dragleave` fires when the pointer enters a
**child** of the lane. AC-2.2 is the requirement; DESIGN picks the mechanism.
Real-browser flicker is only observable by hand (D12).

### US-CDF-03: See exactly where in the lane the card will land

`job_id: job-board-card-move` · Slice 03

#### Elevator Pitch

- **Before:** the card lands at the slot under the cursor, but that slot is invisible until release. Placing AUTH-41 "second" in In-Progress takes a drop, a look and often a second drag.
- **After:** drag AUTH-41 over In-Progress between AUTH-3 and AUTH-12 on the Identity Platform board → sees a line open between those two cards; release → AUTH-41 is exactly there, and still there after a reload.
- **Decision enabled:** which card AUTH-41 should follow, chosen against the lane's real order before committing.

#### Problem

Positional insert already works end to end (`insertBeforeTarget` → `after=<key>`
→ `reposition_issue_with_outbox`). What is missing is the one piece of
information that makes it usable: where the card will go.

#### Domain Examples

1. **Between two cards.** In-Progress reads AUTH-3, AUTH-12, AUTH-19. The pointer sits below AUTH-3's midpoint and above AUTH-12's. The marker shows between them, and the card lands there with `after=AUTH-3`.
2. **At the ends.** Above AUTH-3's midpoint → marker above AUTH-3, the card lands first, and no `after` is sent. Below AUTH-19 → the card lands last.
3. **Within its own lane.** Dragging AUTH-19 up between AUTH-3 and AUTH-12 shows the marker there; AUTH-19's own old slot is never offered as a neighbour.

#### UAT Scenarios

```gherkin
@us-cdf-03 @needs-browser
Scenario: A marker shows the slot between two cards
  Given In-Progress holds AUTH-3, AUTH-12 and AUTH-19 in that order
  When Priya drags AUTH-41 over In-Progress between AUTH-3 and AUTH-12
  Then a marker shows between AUTH-3 and AUTH-12
  And it is the only marker on the board

@us-cdf-03 @needs-browser
Scenario: The card lands exactly where the marker was
  Given the marker shows between AUTH-3 and AUTH-12
  When Priya releases AUTH-41
  Then In-Progress reads AUTH-3, AUTH-41, AUTH-12, AUTH-19
  And the move request names AUTH-3 as the card above
  And a reload shows the same order

@us-cdf-03 @needs-browser
Scenario Outline: The marker reaches both ends of a lane
  Given In-Progress holds AUTH-3, AUTH-12 and AUTH-19 in that order
  When Priya drags AUTH-41 over In-Progress <where>
  Then the marker shows <marker>
  And on release AUTH-41 is <position> and a reload agrees
  Examples:
    | where                 | marker         | position | 
    | above AUTH-3's middle | above AUTH-3   | first    |
    | below AUTH-19         | below AUTH-19  | last     |

@us-cdf-03 @needs-browser
Scenario: Reordering inside a lane never offers the card's own slot
  Given In-Progress holds AUTH-3, AUTH-12 and AUTH-19 in that order
  When Priya drags AUTH-19 up between AUTH-3 and AUTH-12
  Then the marker shows between AUTH-3 and AUTH-12
  And on release In-Progress reads AUTH-3, AUTH-19, AUTH-12

@us-cdf-03 @needs-browser
Scenario: The marker never outlives the drag
  Given the marker shows between AUTH-3 and AUTH-12
  When Priya moves the card out over the page header and presses Escape
  Then no marker shows anywhere and AUTH-41 is back in Backlog where it started

@us-cdf-03 @needs-browser @error
Scenario: A refused drop returns the card and leaves no marker
  Given another operator deleted AUTH-41 after Priya's board loaded
  When Priya drags AUTH-41 between AUTH-3 and AUTH-12 and releases
  Then AUTH-41 returns to its exact origin slot in Backlog
  And no marker shows anywhere
```

#### Acceptance Criteria

- **AC-3.1** During a card drag over a lane, exactly one marker on the board shows the slot the card would take; none shows while over no lane.
- **AC-3.2** The marker's slot, the optimistic landing slot and the POST's `after` come from one computation and always agree; `after` is omitted for the top slot (D7, D9).
- **AC-3.3** After a successful drop, a reload shows the lane in the same order the drop produced (D11).
- **AC-3.4** The dragged card's own slot is never a neighbour for the marker or for `after`.
- **AC-3.5** A pointer held still keeps the marker in one slot: repeated `dragover` at the same coordinates never moves it (D8).
- **AC-3.6** Every exit (drop, `Escape`, release outside any lane, refused or failed POST) leaves **zero** markers (D5).
- **AC-3.7** The marker is drawn from existing `--cz-*` tokens only and measures ≥3:1 against the lane surface in both palettes; the stylesheet is re-hashed per D13 (D8).
- **AC-3.8** Foreign drags show no marker (D4); the marker works on lanes replaced or inserted in place (D3).

#### Technical Notes

`.lane-drop-indicator` is the shape precedent, not the colour precedent (D8).
Oscillation risk: an in-flow marker shifts the cards below it, which moves the
midpoints that `insertBeforeTarget` reads.

### US-CDF-04: Empty lanes read truthfully during and after a drag

`job_id: job-board-card-move` · Slice 04

#### Elevator Pitch

- **Before:** drop OPS-7 into the empty Staging lane and Staging shows OPS-7 *and* "No issues yet"; the In-Progress lane it just emptied shows nothing at all until a reload.
- **After:** drag OPS-7 from In-Progress into the empty Staging lane on Homelab Ops → sees Staging show OPS-7 with no placeholder, and In-Progress show "No issues yet — press c to file the first one."
- **Decision enabled:** which lanes are genuinely empty (where she has nothing in flight), read straight off the board without a reload.

#### Problem

The placeholder is server-rendered and never touched by the drag. So a drop
into an empty lane leaves it contradicting itself, and a lane emptied by a drag
looks broken rather than empty. Both correct themselves only on reload. The same happens to a lane whose last
card is deleted from another tab: the live board drops the card and leaves the
lane blank.

#### Domain Examples

1. **Into an empty lane.** Staging (empty) gets OPS-7; its placeholder disappears.
2. **Last card out.** In-Progress held only OPS-7; after the drag it shows the placeholder, in the same words a freshly loaded empty lane shows.
3. **Refusal.** The server refuses the move (another operator deleted OPS-7). OPS-7 returns to In-Progress, In-Progress shows no placeholder, and Staging's placeholder is back.
4. **Remote delete.** Priya has Homelab Ops open on her second monitor. On her laptop she deletes OPS-7, the only card in In-Progress. The second board drops OPS-7 and In-Progress shows the placeholder, with no reload.

#### UAT Scenarios

```gherkin
@us-cdf-04 @needs-browser
Scenario: Dropping into an empty lane removes its "No issues yet" line
  Given Homelab Ops has an empty Staging lane showing "No issues yet"
  When Priya drags OPS-3 from Backlog into Staging
  Then Staging shows OPS-3 and no "No issues yet" line

@us-cdf-04 @needs-browser
Scenario: A lane emptied by a drag says it is empty
  Given In-Progress holds only OPS-7
  When Priya drags OPS-7 into Done
  Then In-Progress shows "No issues yet — press c to file the first one."
  And a reload shows In-Progress exactly the same

@us-cdf-04 @needs-browser
Scenario: One drag updates both lanes at once
  Given Staging is empty and In-Progress holds only OPS-7
  When Priya drags OPS-7 into Staging
  Then Staging shows OPS-7 without the placeholder
  And In-Progress shows the placeholder
  And a reload shows both lanes exactly the same

@us-cdf-04 @needs-browser @error
Scenario: A refused drop puts both lanes back as they were
  Given Staging is empty and In-Progress holds only OPS-7
  And another operator deleted OPS-7 after Priya's board loaded
  When Priya drags OPS-7 into Staging
  Then OPS-7 is back in In-Progress and In-Progress shows no placeholder
  And Staging shows its placeholder again

@us-cdf-04 @needs-browser
Scenario: Hovering over an empty lane changes nothing until the drop
  Given Staging is empty and shows its placeholder
  When Priya drags OPS-3 over Staging and then presses Escape
  Then Staging still shows its placeholder, unchanged

@us-cdf-04 @needs-browser
Scenario: A lane emptied by a delete in another tab says it is empty
  Given Priya has Homelab Ops open in two tabs and In-Progress holds only OPS-7
  When she deletes OPS-7 from its popup in the first tab
  Then the second tab drops OPS-7 without a reload
  And In-Progress in the second tab shows "No issues yet — press c to file the first one."
  And every other lane in the second tab is unchanged
```

#### Acceptance Criteria

- **AC-4.1** After any drag, no lane shows the placeholder while holding a card, and every lane holding no card shows it (D10).
- **AC-4.2** The placeholder a drag produces is identical in text and markup to the one the server renders for an empty lane; the client carries no second copy of the words (D10).
- **AC-4.3** A refused or failed drop restores both lanes, cards and placeholders, to exactly their pre-drag state.
- **AC-4.4** Hovering, cancelling, or a foreign drag never adds or removes a placeholder; only a drop does, or a remote delete of the lane's last card (AC-4.7).
- **AC-4.5** After any successful drag, a reload changes nothing visible in either lane involved (D11).
- **AC-4.6** Works on lanes replaced or inserted in place (D3).
- **AC-4.7** When another tab's delete removes a lane's last card (`IssueDeleted` via `board-live.js`), that lane shows the placeholder without a reload, from the same single source as AC-4.2. Lanes that still hold cards are untouched (D16).

#### Technical Notes

`neighbourAbove` already skips non-card siblings, so a placeholder never becomes
`after`. Whether the client gets the placeholder from a shared-partial
`<template>` or elsewhere is a DESIGN choice. The constraint is one source,
rendered byte-identically in both render paths (`board_columns.html` is shared,
`board-lane-overflow-menu` D14). `board-live.js` becomes the second module that
writes placeholders. It uses the same single source and the same "hold nothing"
rule, and still consumes only `IssueDeleted`.

## Wave: DISCUSS / [REF] System Constraints

- Every card-drag behaviour survives a `#board-columns` replace: resolve the lane at event time, never bind per lane at load, hold no lane node across events (D3).
- Only a card drag that began on this page activates, marks or drops. Every other drop on the board is swallowed, with no move and no navigation, on a fresh load and after a refresh (D4).
- No check-arch rule for replace-proofing; the refresh-then-drag browser scenarios are the guard (D15).
- The drop POST contract is unchanged; no server, store or migration change (D9).
- CSS uses `--cz-*` tokens only (check-arch S1). Any CSS edit renames `foundry.<hash>.css` across `base.html`, the `lib.rs` tests and `static/VENDOR.md` together (D13).
- `Escape` keeps one owner, `closeTopLayer()`. A native drag's cancel is the browser's own, and this feature adds no `keydown` listener (D5, BR-4).
- `board_columns.html` stays byte-identical across the full page and the OOB refresh.
- Object-oriented project paradigm (`CLAUDE.md`). One owner holds the in-flight drag state and tears it down on every exit; DESIGN shapes it inside the existing `board-dnd.js` (card-ranking D5: no second card-drag script).
- Test lanes: `@needs-browser` (synthetic drag events via WebDriver) is excluded from the default lane and runs in `all` / `cargo xtask ci`. Each slice records a manual real-browser dogfood check (D12).
- Per-feature mutation testing, ≥80% kill rate on modified files. The expected change set is JS, CSS, templates and acceptance steps. If no Rust production code changes, DELIVER records the gate as not applicable, with the file list, rather than reporting a kill rate.

## Wave: DISCUSS / [REF] Outcome KPIs

Objective: the card drag lands where Priya aims, shows where that is before she
lets go, and never needs a reload to work.

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|---|---|---|---|---|---|
| 1 | Board operators | Drop a card after the board refreshed in place | **100%** of card drags over a lane claimed and persisted after each of the **5** in-place refreshes, plus the lane-header reorder as a guard (DISTILL Upstream Issue #1) | **0%**: every lane refuses (RCA cases B, C) | `@needs-browser` regression scenario + outline (US-CDF-01): `dragover` `defaultPrevented`, card state after reload | Leading (north star) |
| 2 | Priya | Stop reloading "just in case" before dragging | **0** refused drops over **5** working days of daily use after slice 01 ships | Refused after every popup delete since `87282d3`; reload is the workaround | Her dogfood report (single-operator instance; persona records this as a valid measure) | Leading |
| 3 | Board operators | See which lane will take the card | Exactly **1** activated lane while over a lane, **0** otherwise, in **100%** of drag scenarios | **0**: no activation exists | DOM count at each step of US-CDF-02 scenarios | Leading |
| 4 | Board operators | Land a card where the marker showed | **100%** of drops land in the marker's slot, and a reload agrees | n/a: no marker | US-CDF-03 scenarios compare marker neighbours, post-drop DOM, POST `after`, post-reload order | Leading |
| 5 | Board operators | Read empty lanes truthfully | **0** lanes with a placeholder and a card; **0** empty lanes without one, after every drag including refusals, and after every remote delete | **3** known violations (RCA finding 3, plus a lane emptied by a remote delete) | Post-scenario check on every lane + reload equality (US-CDF-04) | Guardrail |
| 6 | Board operators | Never have an outside drag move a card or take the tab off the board | **0** activations, **0** markers, **0** requests and **0** navigations for file, text or other-tab drags dropped on the board, on a fresh load and after a refresh | Fresh load: only the lanes swallow a foreign drop. After a refresh, and between lanes, the browser's default applies (it may open the file) | US-CDF-01 #4 outline, US-CDF-02 #5 + request log + URL check | Guardrail |
| 7 | Board operators | Never see feedback outlive the drag | **0** activated lanes and **0** markers after every exit path | n/a | DOM count after each exit-path example | Guardrail |
| 8 | Board operators | Keep the shipped drag working as it does | **100%** of shipped card-drag, lane-drag and popup-delete scenarios green **unmodified**; POST body shape byte-identical | Shipped | Existing suite + request-body assertion (AC-1.4) | Guardrail |

Homelab-scale honesty: no analytics tooling. KPIs are acceptance assertions,
store reads after reload, and Priya's own report, the same posture every prior
feature recorded. **Hypothesis:** we believe replace-proof drag handling plus
lane and slot feedback will let Priya move cards without pre-emptive reloads or
corrective second drags. We will know when KPI 1 is 100% in CI and KPI 2 holds
for a working week.

## Wave: DISCUSS / [REF] DoD

1. All UAT scenarios green in the `all` lane (`cargo xtask ci`); the slice-01 regression was recorded RED on HEAD for the stated reason before its fix (AC-1.2).
2. The shipped card-drag, lane-drag and popup-delete scenarios are green and unmodified (KPI 8).
3. The DOM oracles hold after every drag scenario: zero activated lanes, zero markers, placeholder truthful (KPIs 5, 7).
4. The reload-equality oracle holds after every successful drag scenario (D11).
5. The POST body is byte-identical to today's; no handler, service, store or migration change (D9).
6. Activation and marker measure ≥3:1 in both palettes with tokens only; the stylesheet hash rename is complete in each CSS-touching slice (D6, D8, D13).
7. Each slice's manual real-browser dogfood check is recorded in its delivery notes (D12).
8. Per-feature mutation gate: ≥80% on modified Rust production files, or recorded as not applicable with the file list.
9. `cargo xtask ci` green (check-arch, deny); merged to main.

## Wave: DISCUSS / [REF] Out of Scope

- A touch drag for cards, or moving the card drag onto Pointer Events (`board-lane-reorder`'s recorded successor).
- A keyboard or assistive-technology card move, and live-region announcements of the drop target (D14).
- Auto-scroll during a card drag, vertical or horizontal.
- Any change to the drop request, the `/state` endpoint, ordering, persistence or realtime fan-out (D9).
- Suppressing a no-op drop (a card released into its own slot still POSTs, as today).
- A new colour token or hue for activation (D6: neutral, existing tokens only).
- A build-time (check-arch) rule against per-lane load-time listeners (D15).
- **Tracked follow-up:** fix the lane-reorder `.lane-drop-indicator` contrast (1.49:1 → ≥3:1). It is painted with `--cz-line-strong`, below WCAG 1.4.11's 3:1 for non-text contrast. It is a `board-lane-reorder` asset and is fixed separately; the new card marker does not copy its colour (D8).
- Multi-card drag; drag between boards; live updates of other operators' moves.

## Wave: DISCUSS / [REF] WS Strategy

**No walking skeleton** (D1, user decision 2). The card drag, positional
persist, OOB refresh, stylesheet pipeline and browser lane are all shipped. The
one new abstraction, event-time lane resolution (D3), ships **first**, inside
slice 01, where the bug demands it. The three feedback slices then ride it
rather than each inventing it.

## Wave: DISCUSS / [REF] Story Map and Slices

Backbone (Priya's activities): **Pick up → Carry across lanes → Aim within a
lane → Release → Trust the result.**

| Pick up | Carry | Aim | Release | Trust |
|---|---|---|---|---|
| Only a card from this page starts a card drag (01) | Lanes accept after any in-place refresh (01) | — | Drop persists; revert on refusal (shipped; 01 keeps it) | No reload needed (01) |
| | Lane under the card activates (02) | Marker shows the slot (03) | Lands on the marker (03) | Reload agrees (03) |
| | | | Placeholders update both sides (04) | Empty lanes read true, remote deletes included (04) |

| Slice | Story | Goal | Est. | Learning hypothesis (disproves, if it fails) |
|---|---|---|---|---|
| 01 | US-CDF-01 | Drops work after any in-place board refresh, proven test-first | 0.75d | That all five in-place refreshes share one cause (the header reorder is a guard, DISTILL Upstream Issue #1), and that event-time lane resolution fixes every one |
| 02 | US-CDF-02 | The lane under a dragged card visibly activates and clears on every exit | 0.5d | That a steady (flicker-free) activation is achievable over the lane's own cards, and that a neutral-token change reads as "activating" |
| 03 | US-CDF-03 | A marker shows the exact slot, and the card lands on it | 0.75d | That a marker can sit among the cards without shifting the geometry the slot calculation reads |
| 04 | US-CDF-04 | Empty lanes gain or lose their placeholder truthfully, including on revert and when another tab deletes a lane's last card | 0.5d (DESIGN; was 0.75d) | That a declarative rule (placeholder rendered in every lane, displayed by CSS `:has()` only when the lane has no card) keeps every lane truthful, with no script writing placeholders and no handler change (DDD-5) |

### Priority Rationale

- **01 first**, and non-negotiable: it is a live regression in the core gesture, its fix is the abstraction every later slice needs (D3), and it is test-first by user decision. If 02-04 slipped entirely, the job's most painful force (Push) would still be gone.
- **02 before 03**: it is the smallest, it answers the user's first ask, and it lays down the exit-path teardown in its simplest form. 03 then extends that teardown to a positioned element.
- **03 before 04**: 03 carries the highest remaining uncertainty (oscillation), so a failure there costs least early. 04 is the most mechanical.
- 02, 03 and 04 depend only on 01, so they can be reordered if a dogfood finding demands it.

### Carpaccio taste tests

- **4+ new components?** No slice adds one; all extend `board-dnd.js` + the stylesheet.
- **New abstraction first?** Yes: event-time resolution ships in 01.
- **Does each slice disprove something?** Yes; see the table above.
- **Production data?** Each slice's dogfood check runs on Priya's real Homelab Ops board in a real browser, because synthetic events cannot prove feel (D12).
- **Identical except for scale?** No. **Merging 02+03 was weighed and rejected:** it would save one stylesheet rename, but the two carry different hypotheses (flicker vs geometry) and would ship ~1.25d, over the 1-day cap.

Slice briefs: `docs/feature/card-drag-drop-feedback/slices/slice-0{1..4}-*.md`.

## Wave: DISCUSS / [REF] Driving Ports

All inbound surfaces are shipped; this feature adds none.

1. **The card drag gesture** on `GET /team/{team_slug}/project/{project_slug}`: a drag starting on `article.issue-card[draggable]`, carried over and released on `section.column[data-column]`.
2. **`POST /team/{t}/project/{p}/issues/{n}/state`** (`state`, `after`, `x-csrf-token`): unchanged (D9).
3. **The five in-place board refreshes** (inputs to US-CDF-01, not changed by it): `POST …/issues/{n}/delete` (htmx, OOB); `…/lanes/{slug}/edit`, `…/insert/{side}`, `…/delete` (OOB); `…/lanes/{slug}/move` from the ⋯ menu (fetch + `applyBoard`). Plus **one lane-header reorder**: the header drag posts the same move but rearranges the existing lane nodes without replacing them, kept as a robustness guard (DISTILL Upstream Issue #1).

## Wave: DISCUSS / [REF] Pre-requisites

None outstanding. Shipped and relied on: `board-dnd.js` and its positional drop;
`ChangeStateForm.after` and `reposition_issue_with_outbox`; the placeholder in
`board_columns.html`; the OOB refresh and `applyBoard`; the
`.lane-drop-indicator` shape precedent; the contrast oracle; the stylesheet
hash pipeline; the `@needs-browser` lane and its synthetic-`DragEvent` idiom
(`feature_board_lane_reorder.rs:1143-1151`). The RCA's repro harness lived in a
session scratchpad and was not committed. DISTILL rebuilds the regression in the
acceptance lane.

## Wave: DISCUSS / [REF] DoR Validation

| DoR Item | US-CDF-01 | US-CDF-02 | US-CDF-03 | US-CDF-04 | Evidence |
|---|---|---|---|---|---|
| 1. Problem in domain language | PASS | PASS | PASS | PASS | "every lane refuses the next card"; "finds out by letting go"; "invisible until release"; "contradicting itself" |
| 2. Persona specific | PASS | PASS | PASS | PASS | Priya Raman (`persona-instance-operator`), daily mouse-and-keyboard board user, deletes from the popup mid-triage |
| 3. 3+ domain examples, real data | PASS | PASS | PASS | PASS | Identity Platform AUTH-3/12/19/41/42/43; Homelab Ops OPS-3/7/9 across Backlog/Staging/In-Progress/Done; `keys.png` |
| 4. UAT 3-7 scenarios G/W/T | PASS (5) | PASS (6) | PASS (6) | PASS (6) | Embedded; outlines count once |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS | Each AC maps to ≥1 scenario and a D-decision |
| 6. Right-sized | PASS 0.75d | PASS 0.5d | PASS 0.75d | PASS 0.5d | ≤1 day each; ~2.5d total (US-CDF-04 revised 0.75d → 0.5d by DESIGN DDD-5; still PASS) |
| 7. Technical notes / constraints | PASS | PASS | PASS | PASS | Per-story notes + System Constraints + D1-D16 |
| 8. Dependencies tracked | PASS | PASS | PASS | PASS | 01 depends on nothing unshipped; 02-04 depend on 01 only |
| 9. Outcome KPIs measurable | PASS | PASS | PASS | PASS | 8 KPIs, numeric targets, baselines, measurement per row |

**DoR Status: PASSED** (9/9, all four stories). Every non-`@infrastructure`
story has an Elevator Pitch (none is `@infrastructure`), and every slice holds a
user-visible story. Requirements completeness: **0.97**. The user resolved Q1-Q5 on
2026-09-13 (D4, D6, D15, D16, Out of Scope). The residual is real-browser
behaviour that synthetic events cannot prove (flicker, oscillation, a real
Finder file drop), carried by each slice's dogfood check (D12). Per-wave peer
review skipped by instruction; the consolidated review fires at end of DISTILL.

## Wave: DISCUSS / [REF] Inherited commitments

| Origin | Commitment | Impact here |
|---|---|---|
| `issue-status-move` D3 | DnD is progressive enhancement; the edit dialog is the non-pointer path | D14 |
| `card-ranking-within-status` D5 / ADR-002 | Extend `board-dnd.js`; `after` = key above, absent ⇒ top | D7, D9 |
| `board-lane-overflow-menu` D14 | `board_columns.html` byte-identical across both render paths | AC-4.2 placeholder source |
| ADR-BOARD-LANE-005 rule 2; `board-live.js` constraint 2 | DOM-derived state; hold nothing across a replace | D3 |
| `board-lane-reorder` D3, D16; slice-03 AC-3.4 | HTML5 drag emits nothing on touch; card and lane gestures never become each other; indicator torn down on every exit | D14, AC-1.7, D5 |
| `brief.md` §dialog-layers (BR-4) | `Escape` has one owner | D5: no listener added |
| `canzan-theme-system` ADR-CANZAN-THEME-004 | Colour enters at the token seam; assets hash-honest | D6, D8, D13 |
| `fix-comment-delete-csrf` | HTTP-lane token injection can mask a browser 403 | The drop's CSRF is proven in the browser lane by US-CDF-01 #1 |

## Wave: DISCUSS / [REF] Changed Assumptions

**Source documents:** `docs/product/journeys/journey-issue-card-delete.yaml` (step 6)
and `docs/feature/issue-card-delete/slices/slice-02-delete-from-popup.md`.

**Original assumption, verbatim.** The journey's step 6 named the failure:

> The refresh re-renders columns but drops card drag bindings or resets the lane
> menu, trading a delete for a broken board.

and slice 02's hypothesis recorded, as its success condition:

> **Confirms, if it succeeds:** `#board-columns` is the board's single refresh
> unit for any mutation, and any future card-level operation […] can reuse it
> without inventing a fragment.

**New assumption.** `#board-columns` **is** still the board's single refresh
unit, and nothing here narrows it. But reusing it is safe only for board
behaviour that resolves its targets at event time. The slice-02 success was
recorded without a scenario that dragged after the refresh, so the named
failure mode shipped. From this feature on, "survives a board replace" is a
tested property of every board behaviour (D3, US-CDF-01), not an assumption.

**Rationale.** RCA root-cause chain items 2-5. The prior documents are not
edited. The SSOT journey receives a `changelog` entry pointing here.

## Wave: DISCUSS / [REF] Triggered suggestions (`ask-intelligent`)

Density resolved to **lean + ask-intelligent** (`~/.nwave/global-config.json`).

| Trigger | Fired | Detection |
|---|---|---|
| AC ambiguity | ✓ | US-CDF-02, -03 and -04 share the "every exit path leaves nothing behind" and "foreign drag does nothing" criteria. Real-browser semantics differ from synthetic events exactly there (`dragleave` over children, `dragend` ordering, the browser default for a foreign drop), so readers could disagree. → `gherkin-scenarios` |
| Cross-context complexity | ✓ | Three technologies: browser JS, CSS (token and contrast rules), Rust (the WebDriver acceptance harness; Askama if the placeholder source moves into the template). → `alternatives-considered` |
| Multi-stakeholder need | ✗ | One persona |
| Compliance / regulatory | ✗ | No regulatory terms |
| WS strategy = D | ✗ | No walking skeleton |

Offered to the user through the coordinator. Suggested expansions: **`gherkin-scenarios`**
and **`alternatives-considered`**. The latter is most valuable on D8 (rule vs
ghost card) and D15 (scenarios vs a check-arch rule). D4 and D6 are now user
decisions, and it would record what they rejected. No `DocumentationDensityEvent` was emitted; prior features
record the telemetry helper as absent from this installation, and a hand-written
row is forbidden.

User choices (2026-09-13): `gherkin-scenarios` **expanded**, rendered below under
*[HOW] Gherkin Scenarios*; `alternatives-considered` **skipped**.

Telemetry for these choices was **not emitted**. The helper the skill names,
`scripts/shared/telemetry.py:write_density_event`, is not in this installation.
The installed `nwave-ai` 3.15.1 ships only the `DocumentationDensityEvent`
dataclass, at `nWave/lib/python/des/domain/telemetry/documentation_density_event.py`.
Writing the JSONL by hand is forbidden. Two events are owed and unrecorded:
`choice=expand` for `gherkin-scenarios`, and `choice=skip` for
`alternatives-considered` (`feature_id=card-drag-drop-feedback`, `wave=DISCUSS`).

## Wave: DISCUSS / [HOW] Gherkin Scenarios

Rendered at the user's request: the `gherkin-scenarios` expansion, fired by the
AC-ambiguity trigger. It covers the happy path and the key error paths for each
story. These are **DISCUSS-level** scenarios. DISTILL turns them into executable
ones (expected home:
`crates/foundry-acceptance/tests/features/card-drag-drop-feedback.feature`) and
may merge them with the per-story UAT above.

Conventions, matching the shipped feature files:

- **Tags:** `@cdf` for the feature and `@us-cdf-NN` for the story. `@needs-browser` marks browser-lane scenarios, which are excluded from the default lane and run in `all` / `cargo xtask ci`. `@driving_port` marks a scenario driven through the real UI entry point. `@real-io` means a real server and store (persistence checked by reload). `@error` marks an error path.
- **Oracles** (the browser lane dispatches synthetic drag events, D12):
  - "accepts the drag": the synthetic `dragover` on the lane now on screen is `defaultPrevented`.
  - "swallowed" / "no card moves": the synthetic `dragover` and `drop` carrying a `File` are both `defaultPrevented`, no request is logged, and the URL is unchanged.
  - "shown as activated" and "marker": read from the live DOM, counting exactly one or none.
  - "after a reload": navigate to the board and read it again.
  - Feel (flicker, oscillation, a real Finder drop) is each slice's manual dogfood check.

```gherkin
@cdf
Feature: Card drag-and-drop feedback
  Priya drags cards on her boards many times a session. A drag must keep working
  after the board refreshes in place, show which lane and which slot will take
  the card before she lets go, and leave every lane's "No issues yet" line
  truthful — while anything dragged in from outside the page does nothing.

  Background:
    Given Priya is signed in and is a member of the teams that own "Identity Platform" (AUTH) and "Homelab Ops" (OPS)
    And "Identity Platform" has lanes Backlog, In-Progress and Done
    And its Backlog holds AUTH-41, AUTH-42 and AUTH-43, its In-Progress holds AUTH-3, AUTH-12 and AUTH-19 in that order, and its Done holds AUTH-7
    And "Homelab Ops" has lanes Backlog, Staging, In-Progress and Done
    And its Backlog holds OPS-3, its Staging is empty, its In-Progress holds only OPS-7, and its Done holds OPS-9

  # ---- US-CDF-01: drops keep working after the board refreshes in place ----
  # No step between a refresh and the drag may navigate or reload (D2).

  @us-cdf-01 @needs-browser @driving_port @real-io
  Scenario: A card can still be dropped after the board rearranges itself in place
    # Written first. RED on HEAD because the dragover is not defaultPrevented.
    Given Homelab Ops is open in a browser
    And Priya has moved Staging right from its lane menu, so the board refreshed without a reload
    When Priya drags OPS-7 from In-Progress over Done without reloading the page
    Then Done accepts the drag
    And after she drops it, OPS-7 is in Done
    And after a reload OPS-7 is still in Done

  @us-cdf-01 @needs-browser @driving_port @real-io
  Scenario Outline: Every way the board refreshes in place leaves every lane accepting drops
    Given the Identity Platform board is open in a browser
    And Priya <refreshes the board in place> without reloading
    When Priya drags AUTH-41 into <destination> and drops it
    Then AUTH-41 is in <destination>
    And after a reload AUTH-41 is still in <destination>
    Examples:
      | refreshes the board in place                          | destination |
      | deletes AUTH-42 from its popup                        | In-Progress |
      | renames Done to "Shipped" from its lane menu          | Shipped     |
      | inserts a list "Review" after In-Progress             | Review      |
      | deletes the Done list, moving its cards into Backlog  | In-Progress |
      | drags the Done header to the left of In-Progress      | Done        |

  @us-cdf-01 @needs-browser @real-io
  Scenario: A card dropped at an exact slot after a refresh keeps that slot
    Given the Identity Platform board is open in a browser
    And Priya has deleted AUTH-42 from its popup without reloading
    When Priya drags AUTH-41 between AUTH-3 and AUTH-12 and drops it
    Then the move request names AUTH-3 as the card above
    And after a reload In-Progress reads AUTH-3, AUTH-41, AUTH-12, AUTH-19

  @us-cdf-01 @needs-browser @real-io
  Scenario: A freshly loaded board drags exactly as it did before
    Given the Identity Platform board has just been loaded
    When Priya drags AUTH-41 to the top of In-Progress and drops it
    Then the move request carries the destination lane and names no card above
    And after a reload AUTH-41 is first in In-Progress

  @us-cdf-01 @needs-browser @error
  Scenario Outline: Something dragged in from outside the page is swallowed by the board
    Given the Identity Platform board is open in a browser, <board state>
    When <something> is dragged in from outside the page and dropped on <where>
    Then no card moves and no move request is sent
    And the tab still shows the Identity Platform board
    Examples:
      | board state                                | something                          | where                                   |
      | freshly loaded                             | a file "keys.png"                  | Done                                    |
      | after Priya deleted AUTH-42 from its popup | a file "keys.png"                  | Done                                    |
      | after Priya deleted AUTH-42 from its popup | a text selection from another app  | the gap between Backlog and In-Progress |
      | freshly loaded                             | a text selection from another app  | the empty space below AUTH-7 in Done    |

  @us-cdf-01 @needs-browser @error
  Scenario: A card dragged in from another tab moves nothing in this one
    # Modelled in the browser lane as a drag with no dragstart on this page.
    Given the Identity Platform board is open in two tabs
    When Priya drags AUTH-41 out of the second tab and drops it on Done in the first
    Then no card moves in either tab and no move request is sent

  @us-cdf-01 @needs-browser @error
  Scenario: A cancelled card drag is not mistaken for the next drag
    Given the Identity Platform board is open in a browser
    And Priya started dragging AUTH-41 and cancelled it with Escape
    When a file "keys.png" is then dragged in from outside the page and dropped on In-Progress
    Then no card moves and no move request is sent
    And AUTH-41 is still in its slot in Backlog

  @us-cdf-01 @needs-browser @error @real-io
  Scenario: A drop the server refuses after a refresh puts the card back
    Given the Identity Platform board is open in a browser
    And Priya has deleted AUTH-42 from its popup without reloading
    And another operator deleted AUTH-43 after Priya's board loaded
    When Priya drags AUTH-43 into Done and drops it
    Then AUTH-43 returns to its original slot in Backlog

  # ---- US-CDF-02: the lane under a dragged card activates ----

  @us-cdf-02 @needs-browser
  Scenario: The lane under a dragged card lights up, and only that lane
    Given Homelab Ops is open in a browser
    When Priya drags OPS-3 over Done
    Then Done is shown as activated
    And no other lane is shown as activated

  @us-cdf-02 @needs-browser
  Scenario: The highlight follows the card from lane to lane
    Given Priya is dragging OPS-3 over Done
    When she moves it over Staging
    Then Staging is shown as activated and Done is not

  @us-cdf-02 @needs-browser
  Scenario: A lane stays lit while the card passes over the cards inside it
    Given Priya is dragging OPS-3 over Done
    When the pointer passes over OPS-9 inside Done
    Then Done is still shown as activated

  @us-cdf-02 @needs-browser
  Scenario: Nothing is lit while the card is over no lane
    Given Priya is dragging OPS-3 over Done
    When she moves it over the page header
    Then no lane is shown as activated

  @us-cdf-02 @needs-browser @error
  Scenario Outline: Every way a drag ends leaves no lane lit
    Given Priya is dragging OPS-3 over Done
    When <the drag ends>
    Then no lane is shown as activated
    Examples:
      | the drag ends                                         |
      | she drops it on Done                                  |
      | she presses Escape                                    |
      | she releases it over the page header                  |
      | she drops it on Done and the server refuses the move  |

  @us-cdf-02 @needs-browser @error
  Scenario: Something dragged in from outside the page lights nothing
    Given Homelab Ops is open in a browser
    When a file "keys.png" is dragged in from outside the page over Done
    Then no lane is shown as activated

  @us-cdf-02 @needs-browser @real-io
  Scenario: Lanes still light up after the board refreshes in place
    Given Homelab Ops is open in a browser
    And Priya has inserted a list "Review" after In-Progress from its lane menu, without reloading
    When she drags OPS-3 over Review
    Then Review is shown as activated

  @us-cdf-02 @needs-browser
  Scenario Outline: The activated lane is legible in both palettes and moves nothing
    Given the device is set to <palette>
    And Homelab Ops is open in a browser
    When Priya drags OPS-3 over Done
    Then the activated lane's boundary measures at least 3:1 against the page
    And no card and no column has moved or changed size
    Examples:
      | palette |
      | light   |
      | dark    |

  # ---- US-CDF-03: a marker shows exactly where the card will land ----

  @us-cdf-03 @needs-browser
  Scenario: A marker shows the slot between two cards
    Given the Identity Platform board is open in a browser
    When Priya drags AUTH-41 over In-Progress between AUTH-3 and AUTH-12
    Then a marker shows between AUTH-3 and AUTH-12
    And it is the only marker on the board

  @us-cdf-03 @needs-browser @driving_port @real-io
  Scenario: The card lands exactly where the marker showed, and a reload agrees
    Given the Identity Platform board is open in a browser
    And the marker shows between AUTH-3 and AUTH-12 while Priya drags AUTH-41
    When she drops it
    Then In-Progress reads AUTH-3, AUTH-41, AUTH-12, AUTH-19
    And the move request names AUTH-3 as the card above
    And after a reload In-Progress reads AUTH-3, AUTH-41, AUTH-12, AUTH-19

  @us-cdf-03 @needs-browser @real-io
  Scenario Outline: The marker reaches both ends of a lane
    Given the Identity Platform board is open in a browser
    When Priya drags AUTH-41 over In-Progress <where>
    Then the marker shows <marker>
    And after she drops it and reloads, AUTH-41 is <position> in In-Progress
    Examples:
      | where                       | marker        | position |
      | above the middle of AUTH-3  | above AUTH-3  | first    |
      | below AUTH-19               | below AUTH-19 | last     |

  @us-cdf-03 @needs-browser @real-io
  Scenario: Reordering inside a lane never offers the card's own slot
    Given the Identity Platform board is open in a browser
    When Priya drags AUTH-19 up between AUTH-3 and AUTH-12
    Then the marker shows between AUTH-3 and AUTH-12
    And after she drops it and reloads, In-Progress reads AUTH-3, AUTH-19, AUTH-12

  @us-cdf-03 @needs-browser
  Scenario: A still pointer keeps the marker in one place
    Given the Identity Platform board is open in a browser
    And the marker shows between AUTH-3 and AUTH-12 while Priya drags AUTH-41
    When the drag reports the same pointer position several more times
    Then the marker still shows between AUTH-3 and AUTH-12

  @us-cdf-03 @needs-browser
  Scenario: The marker moves with the card to another lane
    Given the Identity Platform board is open in a browser
    And the marker shows between AUTH-3 and AUTH-12 while Priya drags AUTH-41
    When she moves the card over Done below AUTH-7
    Then the only marker on the board shows below AUTH-7

  @us-cdf-03 @needs-browser @error
  Scenario Outline: The marker never outlives the drag
    Given the Identity Platform board is open in a browser
    And the marker shows between AUTH-3 and AUTH-12 while Priya drags AUTH-41
    When <the drag ends>
    Then no marker shows anywhere on the board
    And AUTH-41 is <where AUTH-41 is>
    Examples:
      | the drag ends                                 | where AUTH-41 is             |
      | she drops it                                  | between AUTH-3 and AUTH-12   |
      | she presses Escape                            | back in its slot in Backlog  |
      | she releases it over the page header          | back in its slot in Backlog  |
      | she drops it and the server refuses the move  | back in its slot in Backlog  |

  @us-cdf-03 @needs-browser @error
  Scenario: Something dragged in from outside the page shows no marker
    Given the Identity Platform board is open in a browser
    When a file "keys.png" is dragged in from outside the page over In-Progress
    Then no marker shows anywhere on the board

  @us-cdf-03 @needs-browser @real-io
  Scenario: The marker works on a board that refreshed in place
    Given the Identity Platform board is open in a browser
    And Priya has deleted AUTH-42 from its popup without reloading
    When she drags AUTH-41 over In-Progress between AUTH-12 and AUTH-19
    Then a marker shows between AUTH-12 and AUTH-19
    And after she drops it and reloads, In-Progress reads AUTH-3, AUTH-12, AUTH-41, AUTH-19

  @us-cdf-03 @needs-browser
  Scenario Outline: The marker is legible in both palettes
    Given the device is set to <palette>
    And the Identity Platform board is open in a browser
    When Priya drags AUTH-41 over In-Progress between AUTH-3 and AUTH-12
    Then the marker measures at least 3:1 against the lane behind it
    Examples:
      | palette |
      | light   |
      | dark    |

  # ---- US-CDF-04: empty lanes read truthfully ----
  # "The placeholder" is the server's own "No issues yet — press c to file the
  # first one." line. A reload is the oracle for truthfulness.

  @us-cdf-04 @needs-browser @real-io
  Scenario: Dropping into an empty lane removes its placeholder
    Given Homelab Ops is open in a browser and Staging shows the placeholder
    When Priya drags OPS-3 from Backlog into Staging and drops it
    Then Staging shows OPS-3 and no placeholder
    And after a reload Staging looks exactly the same

  @us-cdf-04 @needs-browser @real-io
  Scenario: A lane emptied by a drag shows the placeholder
    Given Homelab Ops is open in a browser
    When Priya drags OPS-7, the only card in In-Progress, into Done and drops it
    Then In-Progress shows the placeholder, with the same words and markup a freshly loaded empty lane shows
    And after a reload In-Progress looks exactly the same

  @us-cdf-04 @needs-browser @driving_port @real-io
  Scenario: One drag updates both lanes at once
    Given Homelab Ops is open in a browser
    When Priya drags OPS-7 from In-Progress into the empty Staging lane and drops it
    Then Staging shows OPS-7 and no placeholder
    And In-Progress shows the placeholder
    And after a reload both lanes look exactly the same

  @us-cdf-04 @needs-browser @error @real-io
  Scenario: A refused drop puts both lanes back as they were
    Given Homelab Ops is open in a browser
    And another operator deleted OPS-7 after Priya's board loaded
    When Priya drags OPS-7 from In-Progress into Staging and drops it
    Then OPS-7 is back in In-Progress and In-Progress shows no placeholder
    And Staging shows its placeholder again

  @us-cdf-04 @needs-browser @error
  Scenario Outline: Only a drop changes a placeholder
    Given Homelab Ops is open in a browser and Staging shows the placeholder
    When <something happens over Staging>
    Then Staging still shows its placeholder, unchanged
    Examples:
      | something happens over Staging                                        |
      | Priya drags OPS-3 over Staging and presses Escape                     |
      | Priya drags OPS-3 over Staging and releases it over the page header   |
      | a file "keys.png" is dragged in from outside and dropped on Staging   |

  @us-cdf-04 @needs-browser @real-io
  Scenario: A lane emptied by a delete in another tab shows the placeholder
    Given Homelab Ops is open in two tabs
    When Priya deletes OPS-7, the only card in In-Progress, from its popup in the first tab
    Then the second tab drops OPS-7 without a reload
    And In-Progress in the second tab shows the placeholder
    And every other lane in the second tab is unchanged

  @us-cdf-04 @needs-browser @real-io
  Scenario: A delete in another tab that leaves cards behind adds no placeholder
    Given Homelab Ops is open in two tabs and Done holds OPS-9 and OPS-11
    When Priya deletes OPS-9 from its popup in the first tab
    Then Done in the second tab shows OPS-11 and no placeholder

  @us-cdf-04 @needs-browser @real-io
  Scenario: Placeholders stay truthful on a board that refreshed in place
    Given Homelab Ops is open in a browser
    And Priya has moved Staging right from its lane menu without reloading
    When she drags OPS-7 from In-Progress into Staging and drops it
    Then Staging shows OPS-7 and no placeholder
    And In-Progress shows the placeholder
```

## Wave: DISCUSS / [REF] Open Questions

None outstanding. The user resolved all five on 2026-09-13:

| # | Question | Resolution |
|---|---|---|
| Q1 | Neutral or hue for lane activation | Neutral, existing tokens only, ≥3:1, no new token; slice 02 stays ~0.5d → **D6** |
| Q2 | A foreign file or text drop on the board | Swallowed: no move, no navigation, on a fresh load and after a refresh → **D4**, AC-1.5 |
| Q3 | A lane emptied by a remote delete | Folded into slice 04 (0.5d → 0.75d; DESIGN DDD-5 → 0.5d) → **D16**, AC-4.7 |
| Q4 | A build-time guard for replace-proofing | No check-arch rule; the refresh-then-drag scenarios are the guard → **D15** |
| Q5 | `.lane-drop-indicator` at 1.49:1 | Out of scope; a separate follow-up → *Out of Scope* |

<!-- ===================================================================== -->

## Wave: DESIGN / [REF] Prior Wave Consultation

DESIGN scope **Domain** (@nw-ddd-architect is the only architect). Mode **Propose**:
every real choice below was proposed as the recommended option. **The user accepted
all nine (DDD-1..9) as recommended on 2026-09-13.** Paradigm **object-oriented** (`CLAUDE.md`, unchanged). Density lean +
ask-intelligent. Rigor `adr-025-scaffolded-red`, per-wave review skipped, no-commit.

| Source | Read | What it settled for DESIGN |
|---|---|---|
| This file, DISCUSS D1-D16, US-CDF-01..04, KPIs, `[HOW] Gherkin` | ✓ | Primary input. No DESIGN decision contradicts it; four assumptions are refined (*Changed Assumptions*). |
| `slices/slice-01..04-*.md` | ✓ | Slice 03's optional spike is conditional on DESIGN *not* choosing a zero-footprint marker. DDD-4 chooses one, so the spike is moot. |
| `rca-drag-after-board-replace.md` | ✓ | Fix direction adopted (delegate, event-time lane, session guard, reset on `dragend`) and generalised in DDD-1/2/6. |
| `docs/product/architecture/brief.md` | ✓ | No `## Domain Model` existed, so this wave creates it (extends the file). Binding: §dialog-layers (BR-4), §"The board updates itself for one event", §colour seam (S1/S2), §lanes (two drag mechanisms). |
| `adr-board-lane-005`, `adr-board-lane-007` | ✓ in full | 005 rule 2 (DOM-derived state) generalised by ADR-BOARD-CARD-001. 007: cards keep HTML5 DnD; lane and card drags share no code. |
| `adr-board-lane-006`, `adr-issue-delete-001`, `adr-modal-close-001`, `adr-canzan-theme-004` | ✓ in full | Replace triggers (lane move, popup delete); BR-4; token seam and **DB6 (no Node)**, which decides DDD-9. |
| The other 13 ADRs (`oidc-001..003`, `project-rename-001/002`, `board-lane-001..004`, `canzan-theme-001..003`, `issue-delete-002`) | ✓ indexed | Subjects (auth, rename, lane schema, fonts, asset guard, delete verb) lie outside this client-side domain; titles checked, bodies not opened. |
| `docs/product/journeys/journey-card-drag-drop.yaml` | ✓ | Its shared-artifact registry seeds the ubiquitous language (brief §Domain Model). |
| `docs/product/jobs.yaml` `job-board-card-move` | ✓ | Unchanged. |
| `docs/product/outcomes/registry.yaml` | ✓ | OUT-3 (dnd targets consume the lane set), OUT-11 (popup delete → OOB refresh). |
| `docs/feature/card-drag-drop-feedback/spike/` | ⊘ | Not found. No SPIKE ran. |
| `docs/feature/card-drag-drop-feedback/discuss/{wave-decisions,user-stories,story-map,outcome-kpis}.md` | ⊘ | Not found. The single `feature-delta.md` layout carries all four. |
| Code: `board-dnd.js`, `board-lane-dnd.js`, `board-live.js`, `keyboard.js` (`closeTopLayer`, `BOARD_CARD_SELECTOR`), `board.html`, `board_columns.html`, `board_columns_oob.html`, `issue_card.html`, `base.html`, `foundry.52ad52fa.css`, `issues.rs` `ChangeStateForm`, `lib.rs` static cache tests, `feature_board_lane_reorder.rs` (`open_board_in_browser`, `drag_a_card`), `keyboard_shortcut_bindings.rs:2998-3040` | ✓ | Read directly; CodeGraph was not reachable from this agent (no shell or MCP tool). |
| `foundry-store` `reposition_issue_with_outbox` | ✓ by contract | Not opened. DDD-10 leaves it untouched; its contract (gap-free `0..N-1`) is as DISCUSS D9 and the RCA record. |

**Code findings that shape the design**

- **F1.** `base.html:49` loads `board-dnd.js` on **every** page. Every delegated listener must first establish that the event is inside `#board-columns`. This is the same constraint `board-live.js` records as its constraint 1.
- **F2.** The shipped revert, `from.insertBefore(card, fromNext)` (`board-dnd.js:137,142`), holds nodes past the drag's life. If `board-live.js` removes `fromNext` before the POST answers, `insertBefore` throws `NotFoundError` and the card is not reverted. If a replace lands first, the card reverts into a detached lane. This is latent today; DDD-7 closes it.
- **F3.** `.column` is already `position: relative` (`css:1146`), so a zero-footprint marker needs no new containing block.
- **F4.** App JS is served `no-cache` at a stable URL (`lib.rs:318-333`). Only CSS edits need the D13 hash rename.
- **F5.** The board placeholder `<p class="empty">` is matched by no CSS rule. `.empty-state` (`css:455`, commented "Inside a lane") is used by no template under `crates/foundry-app`. This is out of scope (slice 04 excludes restyling); see OQ-4.
- **F6.** No shipped test asserts the placeholder's absence. A search of `crates/` for "No issues yet", "file the first one" and `kbd>c` finds only the template and one **presence** assertion: `projects.rs:1157` `empty_board_renders_inviting_empty_state_guidance` checks that an *empty* board contains "press" and "file the first". DDD-5 still renders the placeholder there, so the test stays green and no shipped oracle changes (KPI 8).

## Wave: DESIGN / [REF] DDD List

| ID | Decision | Verdict | Options weighed → pick | One-line rationale |
|---|---|---|---|---|
| DDD-1 | **One `CardDragSession`** object in `board-dnd.js` is the single owner of the in-flight drag, with lifecycle `start → over → drop \| end`. For one drag it holds the dragged card node plus its origin **identity**: card key, origin lane slug, origin next and previous keys. It never holds a lane, the active lane or the marker. | accepted (user, 2026-09-13) | A: the shipped three module variables · **B: session object** · C: session held only in DOM attributes | B is the repo's OOP shape (`board-lane-dnd.js`'s `drag` object) with one teardown owner. C loses the session if a replace lands mid-drag. |
| DDD-2 | **Replace-proof by delegation.** `dragstart`, `dragover`, `dragleave`, `drop` and `dragend` are all on `document`. Each first resolves `closest('#board-columns')` and returns if it is null (F1). It then resolves the lane with `closest('[data-column]')` **at event time**. Recorded as a **new ADR-BOARD-CARD-001**, not an amendment. | accepted (user, 2026-09-13) | **A: delegate on `document`** · B: keep per-lane binding and re-bind after each replace (`htmx:afterSwap` + an `applyBoard` hook) · C: delegate on `#board-columns` | B re-creates the RCA's fault: five refreshes over two swap mechanisms, and any future trigger can be forgotten. C's host is itself what gets replaced. ADRs are immutable and ADR-BOARD-LANE-005 is scoped to the lane menu, so the board-wide rule needs its own record. |
| DDD-3 | **Lane activation is recomputed on every `dragover`.** The lane under the pointer gets `data-card-drop-target` and every other lane loses it; the DOM is written only when the lane changes. A document `dragleave` with `relatedTarget === null` *schedules* a clear for the next frame, and any `dragover` cancels it: that is "left the window". The boundary is an **inset `outline` in `--cz-muted`**, which takes no layout (the `.kb-selected` idiom) and measures 5.89:1 light and 6.38:1 dark against the page. | accepted (user, 2026-09-13) | A: `dragenter`/`dragleave` depth counter · B: `dragleave` + `relatedTarget` containment · **C: `dragover` is the only activator** | A drifts when a leave is lost (a node removed mid-drag, a replace), and the lane stays lit: D5's failure. B depends on `relatedTarget`, which synthetic events always set but some engines report null for drag events, so it passes CI and flickers in a real browser. C converges on the next `dragover` and has no count to corrupt. |
| DDD-4 | **The marker is one zero-footprint element that records the slot.** `div[data-card-drop-marker]` is absolutely positioned inside the active lane, in the 8px gap between cards, with `pointer-events: none` and `--cz-black` (≈13.8:1 light, ≈16.4:1 dark against `--cz-bg-2`, computed at design time). It carries the slot as `data-before-key` (empty means the end). **On drop the card lands at the live marker's slot.** The drop recomputes with the same `slotFor` only when no marker sits in the drop lane. | accepted (user, 2026-09-13) | A: one in-flow element (the `.lane-drop-indicator` shape) · **B: one absolutely positioned element carrying the slot** · C: `::before`/`::after` on the neighbouring card via a class | B cannot move geometry, so AC-3.5 holds by construction. Because the marker *is* the slot, marker and landing cannot disagree even if the pointer moves between the last `dragover` and the release (D7). A shifts every card below it and needs a hysteresis argument to rule out oscillation. C needs three class variants (before a card, after the last, empty lane) and pseudo-elements are invisible to `querySelectorAll`. |
| DDD-5 | **The placeholder is always rendered and shown by CSS.** `board_columns.html` renders the existing `<p class="empty">` in **every** lane. One rule pair shows it only in a lane holding no card: `.column > .empty { display: none }` and `.column:not(:has(> .issue-card)) > .empty { display: block }`. **No JS writes placeholders**, so the optimistic drop, the revert and `board-live.js`'s remote delete are all truthful by construction. | accepted (user, 2026-09-13) | A: a `<template>` in the partial, cloned by both writers · B: the words in a data attribute · C: a server fragment route · **D: always rendered, visibility by `:has()`** | D has zero writers, so nothing can drift and there is no restore logic. The single source is the server's own node (AC-4.2), and `board-live.js` stays byte-identical (D16). B loses the `<kbd>` markup, which is a second copy. C is a handler change (D9). A is the fallback if the user declines `:has()`. |
| DDD-6 | **A foreign drag is any drag with no session.** The session is the only discriminator and `dataTransfer.types` is never inspected. Inside `#board-columns` with no session, `dragover` is `preventDefault()`-ed with `dropEffect = "none"`, and `drop` is `preventDefault()`-ed and ignored. Outside `#board-columns`, `board-dnd.js` does nothing. | accepted (user, 2026-09-13) | A: inspect types (`Files`, `text/uri-list`) · **B: session only**, with sub-choice **`dropEffect "none"`** or `"move"` | A cannot tell a card dragged from another foundry tab (plain text, like our own) from a real card drag. `"none"` also shows the no-drop cursor, an honest affordance. If dogfood finds a browser that navigates on a `"none"` drop, fall back to accepting the drop and cancelling it (ADR-BOARD-CARD-001). |
| DDD-7 | **The pending move holds no nodes.** The session ends at drop, so a second drag may start before the response. The POST callbacks resolve the card by `data-issue-key` and the origin lane by slug, then the slot by next key, then previous key, then the end, all **at response time**. If the card is no longer on the live board, the revert is skipped, because a replace has already rendered server truth. | accepted (user, 2026-09-13) | A: held nodes (shipped) · **B: identity, re-resolved at response time** · C: held nodes while `isConnected`, identity otherwise | The POST outlives the drag, and D3 allows held nodes only for a drag's life. F2 shows the held-node revert throws or no-ops after a remote delete or a replace. C is two code paths for one outcome. |
| DDD-8 | **Test seams for the `@needs-browser` lane.** Six seams, (a) to (f), listed below. | accepted (user, 2026-09-13) | A: extend `drag_a_card` · **B: new non-reloading steps + a shared harness helper** | A reloads first, which re-binds the listeners and hides the bug (D2, RCA §Test gap). Editing it would modify a shipped scenario (AC-1.7). |
| DDD-9 | **Mutation gate.** cargo-mutants is recorded as **not applicable**, with the modified-file list, because no Rust production file changes; it mutates `.rs` only, not Askama `.html`. JS gets a **named fault-injection table** instead (below), recorded in each slice's delivery notes. | accepted (user, 2026-09-13) | A: N/A only · B: Stryker (JS mutation) run in a container · **C: N/A + a fault-injection table** | B needs Node and a `package.json`, which DB6 forbids (ADR-CANZAN-THEME-004 alt. B), and every mutant would need a full browser-lane run. A leaves the JS's tests unexamined. C is the house's injected-violation gold-test idiom, applied to JS. |
| DDD-10 | **No server, handler, service, store or migration change.** `ChangeStateForm`, `submit_state_change`, `change_issue_state` and `reposition_issue_with_outbox` are untouched. The only server-side artefact edited is the Askama partial (DDD-5), which stays byte-identical across both render paths because it is one file. | confirmed | — | D9 holds. A template edit is not a handler change. |
| DDD-11 | **Neither Event Sourcing nor CQRS.** The session is ephemeral browser state with no history of value. The server's move already writes its change event and outbox row. | confirmed | — | Fails every ES trigger: no audit need, no temporal query, one view. |
| DDD-12 | **No `closeTopLayer()` arm and no `keydown` listener.** A native drag's `Escape` is the browser's cancel and arrives as `dragend` without `drop`. The page receives no key events during a native drag. | confirmed | — | D5 and BR-4. This differs from ADR-BOARD-LANE-007, whose Pointer Events drag needs an arm. |

**DDD-8 detail (the seams DISTILL builds on).**
(a) New steps module `feature_card_drag_drop_feedback.rs`. Opening the board is a *Given*, and no drag *When* calls `open_board_in_browser`.
(b) A shared synthetic-drag helper in `support/browser_harness.rs`, generalising the `fire()` idiom at `keyboard_shortcut_bindings.rs:3028`. Its phases are `start(key)`, `over(lane, aim)`, `leave(lane, related)`, `drop(lane, aim)`, `end(key)` and `foreign(kind, target)`. Each phase returns the event's `defaultPrevented`. `aim` is resolved from live geometry: the gap midpoint for "between A and B", `top + 1` for "above A's middle", `bottom + 4` for "below the last card". `foreign` builds a `DataTransfer` carrying `new File([…], "keys.png")` or `text/plain`, with **no** `dragstart`.
(c) `Escape` and "release over the header" are modelled as `dragend` on the card with no `drop`, which is exactly what a real browser delivers.
(d) The request log is a page-side `fetch` spy installed via WebDriver `execute` before the drag. WebDriver-executed scripts are not bound by the page CSP.
(e) A **no-reload mark**: set `window.__cdfMark` before the replace and assert it still exists at the drag. A reload clears it, so D2 becomes an oracle rather than a convention.
(f) The DOM hooks are the state itself: `[data-card-drop-target]` and `[data-card-drop-marker][data-before-key]`. The placeholder oracle reads computed visibility, not presence (DDD-5). **AC-2.2's oracle is read between the `dragleave` and the next `dragover`.** A naive clear-on-every-leave fails there and passes everywhere else.

**DDD-9 fault-injection table.** Apply each fault by hand, see the named scenario RED, then revert.

| # | Fault | Must RED |
|---|---|---|
| M1 | Bind `dragover`/`drop` per lane at load (HEAD's shape) | US-CDF-01 regression and outline (not the header-reorder guard, which M1 cannot red: the lane nodes survive; DISTILL Upstream Issue #1) |
| M2 | Drop the session-null guard | Foreign-drag outline; "card dragged in from another tab" |
| M3 | Omit `preventDefault()` on a foreign `drop` | Swallow outline (`defaultPrevented` oracle) |
| M4 | Skip teardown on `dragend` | "Every way a drag ends leaves no lane lit" (02); "The marker never outlives the drag" (03); "A cancelled card drag is not mistaken for the next drag" (01). *Names aligned to the feature file by DISTILL.* |
| M5 | Recompute the drop slot at an offset `clientY` instead of reading the marker | "The card lands exactly where the marker showed" |
| M6 | Count the dragged card as a neighbour | "Never offers the card's own slot" |
| M7 | Remove the `:has()` rule pair | US-CDF-04 both-lanes and remote-delete scenarios |
| M8 | Skip the revert on non-2xx | Refused-drop scenarios (01, 03, 04) |
| M9 | Clear activation on every `dragleave` | "A lane stays lit while the card passes over the cards inside it" |

## Wave: DESIGN / [REF] Component Decomposition

| Component | Path | Change | Slice |
|---|---|---|---|
| `CardDragSession` + delegated listeners + identity revert | `crates/foundry-app/static/js/board-dnd.js` | EXTEND (the per-lane loop at 86-146 is replaced) | 01 |
| Lane activation | `board-dnd.js`; stylesheet `.column[data-card-drop-target]` | EXTEND | 02 |
| `slotFor` (was `insertBeforeTarget`) + marker | `board-dnd.js`; stylesheet `[data-card-drop-marker]` | EXTEND | 03 |
| Placeholder visibility | `templates/partials/board_columns.html:39` (render in every lane); stylesheet `:has()` pair | EXTEND | 04 |
| Stylesheet + rename sites | `static/css/foundry.52ad52fa.css` → `foundry.<new>.css`; `templates/base.html`; `src/lib.rs` tests; `static/VENDOR.md` | EXTEND + rename per CSS slice (D13) | 02, 03, 04 |
| `board-live.js` | `static/js/board-live.js` | UNCHANGED (DDD-5) | — |
| `board-lane-dnd.js` (`applyBoard`, indicator) | `static/js/board-lane-dnd.js` | UNCHANGED (a trigger only) | — |
| `keyboard.js` | `static/js/keyboard.js` | UNCHANGED (DDD-12) | — |
| `board.html`, `oob/board_columns_oob.html`, `issue_card.html` | `templates/…` | UNCHANGED | — |
| `/state` handler, service, store | `issues.rs`, `foundry-services`, `foundry-store` | UNCHANGED (DDD-10) | — |
| Card-drag feedback steps | `crates/foundry-acceptance/src/steps/feature_card_drag_drop_feedback.rs` (+ `mod` line) | CREATE NEW | 01-04 |
| Synthetic-drag helper, `fetch` spy, no-reload mark | `crates/foundry-acceptance/src/support/browser_harness.rs` | EXTEND | 01 |
| Feature file | `crates/foundry-acceptance/tests/features/card-drag-drop-feedback.feature` | CREATE NEW (DISTILL) | 01-04 |

## Wave: DESIGN / [REF] Driving Ports

1. **The card drag gesture**, as DOM events delegated on `document` and scoped to `#board-columns`: `dragstart` (from `article.issue-card`), `dragover`, `dragleave`, `drop`, `dragend`. The gesture is unchanged; the listeners move.
2. **The five in-place board refreshes** (inputs, unchanged; DISCUSS Driving Port 3): popup delete; lane edit, insert and delete; lane move from the ⋯ menu. Delegation means none of them needs a hook. The **lane-header drag** is named separately: an in-place rearrangement that moves the existing lane nodes, which delegated listeners also survive (DISTILL Upstream Issue #1).
3. **`IssueDeleted` on `/events`** → `board-live.js` (unchanged). Its placeholder consequence is now declarative (DDD-5).

## Wave: DESIGN / [REF] Driven Ports and Adapters

| Port | Adapter | Change |
|---|---|---|
| Move request `POST /team/{t}/project/{p}/issues/{n}/state` (`state`, `after`, `x-csrf-token`) | `fetch` in `board-dnd.js` → `submit_state_change` → `change_issue_state` → `reposition_issue_with_outbox` | None; the body is byte-identical (D9, AC-1.4) |
| The live board document (the only state store for lanes, activation and marker) | `document.querySelector*` at event time | New reads and writes of two data attributes and one element |
| Presentation | `--cz-*` tokens at the seam | New rules below the seam, tokens only (S1); no new token (D6) |

## Wave: DESIGN / [REF] Technology Choices

No new technology, dependency or runtime. Recorded, not selected:

- Browser JS: hand-authored ES5-style IIFE, `addEventListener` only, CSP-safe, no bundler, no Node (DB6).
- The card drag stays native HTML5 drag-and-drop (ADR-BOARD-LANE-007).
- CSS: the `:has()` relational pseudo-class is used for the first time in this stylesheet (DDD-5). It is Baseline widely available: Chrome/Edge 105, Safari 15.4, Firefox 121.
- Server Rust/axum/Askama unchanged. Acceptance: cucumber-rs + fantoccini WebDriver against the containerised `selenium/standalone-chrome` (unchanged).

## Wave: DESIGN / [REF] Decisions Table

| ID | Decision | Status |
|---|---|---|
| DDD-1 | One `CardDragSession` owns the in-flight drag; origin held as identity | accepted (user, 2026-09-13) |
| DDD-2 | All five drag listeners delegated on `document`, lane resolved at event time; ADR-BOARD-CARD-001 | accepted (user, 2026-09-13) |
| DDD-3 | Activation recomputed on every `dragover`; deferred leave-window clear; inset outline in `--cz-muted` | accepted (user, 2026-09-13) |
| DDD-4 | One zero-footprint marker carrying `data-before-key`; the drop lands at the live marker's slot | accepted (user, 2026-09-13) |
| DDD-5 | Placeholder rendered in every lane, shown by `:has()`; zero JS writers | accepted (user, 2026-09-13) |
| DDD-6 | Session-only foreign-drag discriminator; swallow inside `#board-columns` with `dropEffect "none"` | accepted (user, 2026-09-13) |
| DDD-7 | Pending move re-resolves card, lane and slot by identity at response time | accepted (user, 2026-09-13) |
| DDD-8 | Non-reloading steps, shared harness drag helper, `fetch` spy, no-reload mark, data-attribute hooks | accepted (user, 2026-09-13) |
| DDD-9 | cargo-mutants N/A with the file list, plus the M1-M9 fault-injection table | accepted (user, 2026-09-13) |
| DDD-10 | No server, handler, service, store or migration change | confirmed |
| DDD-11 | No ES/CQRS | confirmed |
| DDD-12 | No `closeTopLayer()` arm, no `keydown` listener | confirmed |

ADRs: `adr-board-card-001-replace-proof-drag-session.md` (DDD-1/2/6/7),
`adr-board-card-002-dragover-activation-and-slot-marker.md` (DDD-3/4),
`adr-board-card-003-placeholder-shown-by-css.md` (DDD-5), all **Accepted (2026-09-13)**.

## Wave: DESIGN / [REF] Reuse Analysis

| Existing Component | File | Overlap | Decision | Justification |
|---|---|---|---|---|
| Card drag module | `static/js/board-dnd.js` | The whole card drag | EXTEND | card-ranking D5: no second card-drag script. The session refactors its three module variables. |
| `insertBeforeTarget` | `board-dnd.js:36-48` | Slot computation | EXTEND (renamed `slotFor`, same midpoint rule) | D7: one computation, which now also places the marker. |
| `neighbourAbove` | `board-dnd.js:52-58` | `after` key | REUSE unchanged | Already skips non-card siblings, so the always-present placeholder and the marker are skipped for free. |
| Shipped `fetch` + revert | `board-dnd.js:126-144` | Move request, revert | EXTEND | Body unchanged; the revert re-resolves by identity (DDD-7, F2). |
| `showIndicator`/`clearIndicator` + `.lane-drop-indicator` | `board-lane-dnd.js:73-94`; `css:1322-1327` | "Show where it lands" lifecycle | CREATE NEW (the card marker) | ADR-BOARD-LANE-007: the two drags "share a DOM region and no code". They also differ in axis (vertical inside a lane vs horizontal across the board), footprint (zero vs in-flow) and contrast obligation (≥3:1 vs its 1.49:1). Sharing would couple two modules ADR-007 keeps apart. The shape is the only precedent taken. |
| `data-lane-dragging` DOM-marker idiom | `board-lane-dnd.js:117`; `keyboard.js:283` | DOM-derived drag state | EXTEND (pattern) | Same idiom for `data-card-drop-target` and `data-card-drop-marker`. |
| `dropCard` | `board-live.js:82-92` | Removes cards, which changes lane emptiness | UNCHANGED | DDD-5 makes emptiness declarative, so D16 is met with zero edits. |
| `applyBoard` | `board-lane-dnd.js:144-152` | Replaces `#board-columns` | UNCHANGED | Delegation needs no post-replace hook. |
| `closeTopLayer` drag arm | `keyboard.js:283-289` | `Escape` cancels a drag | NOT EXTENDED | DDD-12: a native drag cancels itself. |
| `.kb-selected` outline | `css:448-451` | Emphasis ring with no layout | EXTEND (pattern) | An outline takes no layout, so AC-2.6 holds. |
| Placeholder `<p class="empty">` | `board_columns.html:39` | Placeholder single source | EXTEND | Rendered in every lane; visibility by CSS. |
| `drag_a_card` | `feature_board_lane_reorder.rs:1137` | Synthetic card-drag step | NOT REUSED; CREATE NEW steps | It calls `open_board_in_browser` first, which hides the bug (D2, RCA). Changing it would modify a shipped scenario (AC-1.7). |
| `fire()` synthetic `DragEvent` idiom | `keyboard_shortcut_bindings.rs:3028-3034` | Synthetic dispatch | EXTEND into a `browser_harness` helper | Otherwise a third copy of the idiom would be written. |
| Contrast oracle | Acceptance suite (canzan-theme) | ≥3:1 measurement | REUSE | D6 and D8 already name it. |
| Steps module per feature | `steps/feature_*.rs` | Step definitions | CREATE NEW `feature_card_drag_drop_feedback.rs` | House convention is one steps module per feature, and no existing module owns card-drag feedback. |

Zero unjustified CREATE NEW. The three CREATE NEW rows are the card marker (ADR-007
evidence), the steps module (convention) and the feature file (DISTILL's own output).

## Wave: DESIGN / [REF] Outcome Collision Check

`nwave-ai outcomes check-delta docs/feature/card-drag-drop-feedback/feature-delta.md`
exited **0**. Output, verbatim: `2 outcomes checked, 0 collisions found across 0 outcomes`.

"Across 0 outcomes" suggests the CLI compared the delta against no registry rows.
The registry mixes two row schemas (OUT-1..10 use `title`/`input_shape`, OUT-11/12
use `summary`/`inputs`), and that is a plausible cause. A **manual keyword check**
against `docs/product/outcomes/registry.yaml` therefore corroborates the gate. This
feature's candidate contracts are (a) "a card drag is claimed and lands after any
in-place board replace; foreign drags are swallowed" and (b) "a lane displays its
placeholder if and only if it holds no card".

- OUT-3 (`board`, `lanes`, "dnd targets … consume the same lane set"): **related, not a duplicate**. Candidate (a) consumes that lane set at event time. Link it as `related: [OUT-3]`.
- OUT-6 (`dom-derived`, `escape`, `br-4`): a shared principle, but a distinct contract (the lane menu). Link it as `related: [OUT-6]`.
- OUT-9/OUT-10 (lane move) and OUT-11/OUT-12 (issue delete) are replace *triggers* only. No overlap.

Result: **no collision.** DISTILL registers the OUT rows (house practice, OQ-6) with
those `related` links.

## Wave: DESIGN / [REF] Open Questions

| # | Question | Deferred to |
|---|---|---|
| OQ-1 | The activated lane's **surface** token. The boundary is settled (DDD-3), but `--cz-surface` would erase card elevation in the light palette. DELIVER measures candidates and dogfood judges "reads as activating" (D6, slice 02 hypothesis b). | DELIVER slice 02 |
| OQ-2 | Whether `dropEffect = "none"` swallows a real Finder file in Chrome, Firefox and Safari. The fallback is recorded in ADR-BOARD-CARD-001. | DELIVER slice 01 dogfood |
| OQ-3 | A foreign file dropped **outside** `#board-columns` (the page header) keeps the browser default and may open the file, because D4 scopes the swallow to the board. Widening it to the whole document is a user decision, not taken here. | User |
| OQ-4 | F5: the board placeholder has no style rule, and `.empty-state` appears dead. Out of scope (slice 04 excludes restyling). | Follow-up |
| OQ-5 | ~~`:has()` (DDD-5 D) or the `<template>` fallback (A)~~ **Resolved (user, 2026-09-13): `:has()` accepted; `<template>` is the documented fallback only** (ADR-BOARD-CARD-003). | Closed |
| OQ-6 | OUT rows for this feature. | DISTILL (house practice) |

## Wave: DESIGN / [REF] Changed Assumptions

1. **Placeholder writers.** Source: this file, US-CDF-04 *Technical Notes*, verbatim:
   > Whether the client gets the placeholder from a shared-partial `<template>` or elsewhere is a DESIGN choice. […] `board-live.js` becomes the second module that writes placeholders. It uses the same single source and the same "hold nothing" rule, and still consumes only `IssueDeleted`.

   **New:** no module writes placeholders, and the client never obtains one. Each lane already holds the server's node and CSS shows it exactly when the lane holds no card (DDD-5). `board-live.js` is unchanged. AC-4.1..4.7 are unchanged. **Applied (user acceptance, 2026-09-13):** slice 04 is re-estimated from 0.75d to **0.5d**, the feature total from ~2.75d to **~2.5d**, and `slices/slice-04-empty-lane-placeholder.md` IN scope and hypothesis are updated.
2. **Placeholder consumers.** Source: *[REF] Shared Artifacts*, verbatim:
   > | Placeholder copy | `board_columns.html:39` | Fresh render, OOB render, client-side restore after a drag, remote-delete upkeep in `board-live.js` (D16) | MEDIUM: a hard-coded client copy drifts from the template (D10) |

   **New:** the consumers are the two server render paths only. The drift risk is retired rather than mitigated, because there is no client copy and no client restore.
3. **What a drag may hold.** Source: D3, verbatim:
   > No lane or card node is held across events, except the one dragged card and its origin for the life of one drag.

   **New (a tightening):** the origin is held as identity (lane slug, neighbour keys), not as a node. The pending move that outlives the drag holds nothing (DDD-7, F2).
4. **Slice 03 spike.** Source: `slices/slice-03-insertion-marker.md`, verbatim:
   > Optional, ≤1 hour: measure whether a 2–3px in-flow rule causes oscillation at a card midpoint in Chrome and Firefox. Run only if DESIGN has not already chosen a zero-footprint marker.

   **New:** DDD-4 chooses a zero-footprint marker, so the spike is not run.

Oracle note for DISTILL, not a story change: "no placeholder" means the placeholder is
**not displayed** (computed style), not that it is absent from the DOM. No story or AC
wording changes, so `design/upstream-changes.md` is not written.

## Wave: DESIGN / [REF] Wave Decisions

### Key Decisions (all accepted by the user, 2026-09-13)
- [DDD-1] One `CardDragSession` owns the in-flight drag, holding the dragged card for one drag and its origin as keys. One owner tears down on every exit (see: `adr-board-card-001-replace-proof-drag-session.md`).
- [DDD-2] All five drag listeners are delegated on `document`, scoped to `#board-columns`, and resolve the lane at event time. Replace-proof by construction; nothing is bound per lane at load (see: ADR-BOARD-CARD-001).
- [DDD-3] Lane activation is recomputed on every `dragover`, with a deferred leave-window clear and an inset outline in `--cz-muted`. There is no counter to drift and no layout change (see: `adr-board-card-002-dragover-activation-and-slot-marker.md`).
- [DDD-4] One zero-footprint positioned marker is the slot (`data-before-key`), and the drop lands at it. Marker and landing cannot disagree, and the slice 03 spike is not run (see: ADR-BOARD-CARD-002).
- [DDD-5] The server renders the placeholder in every lane; CSS `:has()` displays it only when the lane holds no card. Zero writers; `board-live.js` unchanged; `<template>` is the documented fallback only (see: `adr-board-card-003-placeholder-shown-by-css.md`).
- [DDD-6] A foreign drag is any drag with no session. Inside `#board-columns` it is swallowed (`dragover` and `drop` `defaultPrevented`, `dropEffect "none"`), because the payload cannot tell another tab's card from ours (see: ADR-BOARD-CARD-001).
- [DDD-7] A refused or failed move re-resolves card, lane and slot by key at response time, which survives a replace or remote delete (F2) (see: ADR-BOARD-CARD-001).
- [DDD-8] Non-reloading steps, a shared synthetic-drag helper in `browser_harness`, a `fetch` spy and a no-reload proof mark. D2 becomes an oracle (see: this file, DDD-8 detail).
- [DDD-9] cargo-mutants recorded N/A with the file list, plus the M1-M9 named-fault table, each fault turning a named scenario red. There is no Rust to mutate, and Node is barred by DB6 (see: this file, DDD-9 table).
- [DDD-10..12] No server, handler, store or migration change; no ES/CQRS; no `closeTopLayer()` arm (confirmed).

### Architecture Summary
- Pattern: unchanged modular monolith, ports-and-adapters. This feature is confined to the browser tier (Board Interaction, a supporting subdomain) and conforms to the server's published contracts (`POST …/state`; the board DOM contract).
- Paradigm: OOP (`CLAUDE.md`).
- Key components: `board-dnd.js` (`CardDragSession`, delegated listeners, `slotFor`, drop feedback, `PendingMove`); `partials/board_columns.html` (placeholder in every lane); the stylesheet (activation, marker, `:has()` rule); `feature_card_drag_drop_feedback.rs` + a `browser_harness` helper.

### Reuse Analysis
See *[REF] Reuse Analysis* above: 15 rows, 3 CREATE NEW, each justified (the card marker per ADR-BOARD-LANE-007; the steps module by house convention; the DISTILL feature file).

### Technology Stack
- No new technology. Hand-authored ES5-style JS, no Node (DB6). Native HTML5 drag-and-drop for cards (ADR-BOARD-LANE-007). CSS `:has()` is used for the first time; it is Baseline widely available.

### Constraints Established
- A board behaviour binds no listener to a node inside `#board-columns` and holds no such node past the event, or past the drag for the one dragged card (ADR-BOARD-CARD-001).
- Drop feedback state lives in the DOM (`data-card-drop-target`, `[data-card-drop-marker]`), and teardown is an idempotent query.
- Placeholder visibility is a stylesheet fact; no script writes placeholders (ADR-BOARD-CARD-003).
- The move request is byte-identical to the shipped one.

### Upstream Changes
- No story or AC wording changes, so no `design/upstream-changes.md`.
- Slice 04: 0.75d → 0.5d. IN scope now says no script writes placeholders and `board-live.js` is unchanged. Feature total ~2.75d → ~2.5d. DoR row 6 updated (still PASS).
- Slice 03: the optional spike is marked not run (DDD-4).
- Oracle note for DISTILL: "no placeholder" means not displayed.
- OQ-1..OQ-4 stay open for DELIVER and dogfood. OQ-5 is closed.

## Wave: DESIGN / [REF] Triggered suggestions (`ask-intelligent`)

| Trigger | Fired | Suggested expansion |
|---|---|---|
| A contested choice with a platform-feature dependency (DDD-5 `:has()`) | ✓ | `trade-off-analysis` (DDD-5 D vs A, DDD-4 B vs A) |
| Alternatives already recorded in three ADRs | ✓ | `rejected-alternatives` (would lift them into this file) |
| Complex subsystem | ✗ | `c4-component-diagrams` is not needed: brief §Domain Model carries the one component view that earns its place |

Not rendered (lean). Telemetry is not emitted: the helper
`scripts/shared/telemetry.py:write_density_event` is absent from this installation,
as DISCUSS recorded.

<!-- ===================================================================== -->

## Wave: DEVOPS / [REF] Prior Wave Consultation

Apex (@nw-platform-architect), 2026-09-13. Density **lean + ask-intelligent**
(`~/.nwave/global-config.json`). Rigor `adr-025-scaffolded-red`, no-commit mode
(`.nwave/des-config.json`). Per-wave review is skipped: none of the contract's
triggers fires (no new deploy target, CI framework, observability rewrite or
security change). Decisions 1-9 are **carried forward, not re-asked**.

| Source | Read | What it settled for DEVOPS |
|---|---|---|
| This file, DISCUSS (D1-D16, US-CDF-01..04, KPIs 1-8, `[HOW] Gherkin`, DoD) | ✓ | The KPIs drive *Monitoring contracts*. D12 sets the lanes, D13 the rename, D2 the no-reload rule. |
| This file, DESIGN (DDD-1..12, component table, Reuse Analysis, Wave Decisions) | ✓ | DDD-5 `:has()` sets platform coverage. DDD-8 gives the seams, DDD-9 + M1-M9 the mutation gate, DDD-10 "no server change", F4 "JS is `no-cache`". |
| `discuss/outcome-kpis.md` | ⊘ | Legacy layout; the KPI table is in this file |
| `design/*` | ⊘ | Legacy layout; DESIGN is in this file |
| `slices/slice-01..04-*.md` | ✓ | Per-slice dogfood checks and production data |
| `adr-board-card-001/002/003-*.md` | ✓ | 001: swallow fallback (`"move"` + cancel). 002: DOM hooks. 003: Baseline versions, silent degradation, `<template>` fallback. |
| `brief.md` `## Domain Model` | ✓ | Invariants 1-8 are the oracle list. The deployment topology is unchanged, so `brief.md` is not edited. |
| `docs/product/kpi-contracts.yaml` | ⊘ | Absent: `issue-card-delete` (:1100) and `instance-admin-project-rename` (:370) recorded it missing. **Created by this wave.** |
| `rca-drag-after-board-replace.md` | ⊘ not read | DESIGN consumed it; not a DEVOPS input |
| Platform sources for Decisions 1-9 | ✓ | See next table |

**Contradiction check: none.** No DEVOPS decision contradicts DESIGN. Three
prior-wave *statements* are refined; see *[REF] Changed Assumptions*.

## Wave: DEVOPS / [REF] Decisions 1-9 (carried forward)

| # | Decision | Value | Carried forward from |
|---|---|---|---|
| 1 | Deployment target | **On-premise, self-hosted** (homelab Kubernetes). The image is built from `main`. | `issue-edit-modal-close-icon/feature-delta.md` DEVOPS `prod` row; `foundry-devops/devops/plan.md` |
| 2 | Container orchestration | **Kubernetes**, plain YAML in `deploy/k8s/` (no Helm/Kustomize, ADR-102), plus `docker-compose.yml` for single host | `foundry-devops/devops/plan.md` §Scope, §Deferred |
| 3 | CI/CD platform | **GitHub Actions** `.github/workflows/ci.yml` runs one command, `cargo xtask ci` (the authoritative gate). A **Forgejo** mirror, `.forgejo/workflows/ci.yml`, runs split jobs. | `ci.yml:1-18,97-98`; `issue-edit-modal-close-icon` DEVOPS env matrix |
| 4 | Existing infrastructure | **Yes, both.** This feature changes none of it (DDD-10). | `foundry-devops/devops/plan.md`; this file DDD-10 |
| 5 | Observability and logging | **Prometheus + Grafana, with Loki/Promtail logs** (`docker-compose.observability.yml`, `observability/`). **No new signal** for this feature. | `foundry-devops/devops/plan.md` §Scope; `issue-edit-modal-close-icon` DEVOPS "Observability stack" |
| 6 | Deployment strategy | **Rolling**: `RollingUpdate`, 3 replicas, `maxSurge: 1`, `maxUnavailable: 0`. Rollback is `git revert` + the next image. | `deploy/k8s/foundry-deployment.yaml:24-31`; `issue-edit-modal-close-icon` DEVOPS "Deployment strategy" |
| 7 | Continuous learning | **No.** No feature flags, A/B tests or canary analysis; alerting is left to operators. | `foundry-devops/devops/plan.md` §Deferred (ADR-104); `issue-edit-modal-close-icon` DEVOPS ("no feature flag needed") |
| 8 | Git branching | **Trunk-based on `main`.** CI runs on push and PR to `main`; images come from `main` and `v*` tags. | `issue-edit-modal-close-icon` DEVOPS "Branching strategy"; `ci.yml:22-26`; `release.yml:30-33` |
| 9 | Mutation testing | **Per-feature, ≥80% kill rate.** For this feature: cargo-mutants N/A + the M1-M9 named-fault table. | Project `CLAUDE.md` `## Mutation Testing Strategy` (not rewritten); this file DDD-9 |

## Wave: DEVOPS / [REF] Environment matrix

| Environment | Where | What of this feature runs there | Preconditions |
|---|---|---|---|
| `clean` (default lane) | `cargo test -p foundry-acceptance --release --test acceptance` with `FOUNDRY_ACCEPTANCE_TAGS` unset, and `cargo xtask smoke` | **None** of the `@needs-browser` scenarios, because the default lane excludes them (`acceptance.rs:293-302`, re-read at the final review; DISTILL's two force-link lines shifted it from 291-300). It does run KPI 8's non-browser shipped scenarios, `projects.rs:1157`, the `lib.rs` cache-policy tests and check-arch. | Docker daemon (testcontainers Postgres) |
| `browser-lane` | `cargo xtask ci`, whose last step sets `FOUNDRY_ACCEPTANCE_TAGS=all` (`xtask/src/main.rs:217-228`), plus targeted `FOUNDRY_ACCEPTANCE_TAGS=cdf` or `us-cdf-0N` (`acceptance.rs:231-258`). Runs on the developer workstation and `ci-github`. | Every US-CDF scenario, and the M1-M9 fault runs | See *Pre-requisites* P1-P6. **Chrome floor: ≥105**, the first Chrome with CSS `:has()` (ADR-BOARD-CARD-003; Edge 105, Safari 15.4, Firefox 121). The lane image floats at `:latest` (observed 151.0.7922.108) and every run records `browserVersion`; pinning the image is a tracked follow-up. |
| `dogfood` | Real host browsers against `./run.sh` or `./restart.sh` on `http://localhost:3000` | Each slice's manual check (D12): flicker, oscillation, a real Finder file drop, `:has()` in a real engine | Chrome and Firefox for every slice. **Safari** for the slice-01 swallow check (OQ-2) and slice 04. Light and dark palettes. P4, P5, P7. |
| `ci-github` | `.github/workflows/ci.yml`, ubuntu-latest | Same as `browser-lane`; the authoritative gate | `FOUNDRY_XTASK_INCLUDE_DOCKER=1` is already set (`ci.yml:71`); dockerd is present |
| `ci-forgejo` | `.forgejo/workflows/ci.yml:84-97` | **Cannot run the browser lane.** Its `rust:1.85-slim` container mounts `docker.sock` but ships no `docker` CLI, so `ensure_chromedriver` panics (`browser_harness.rs:237-252`). **Not a KPI gate.** | n/a (carried-forward caveat) |
| `prod` | Self-hosted k8s (`deploy/k8s/`) | Nothing is measured there (no runtime telemetry) | None new. Operational note: check the homelab VM's liveness from `~/.lima/<vm>/ha.stderr.log`, because `limactl list` can report `Running` for a dead VM. |

Machine form: `docs/feature/card-drag-drop-feedback/environments.yaml`.

## Wave: DEVOPS / [REF] CI/CD pipeline outline

Unchanged. `cargo xtask ci` is the single source of truth, and CI runs exactly
that command (`ci.yml:1-18`). Stages run in order and stop at the first red.
The stages that bear on this feature are **bold**.

1. Preflights: `DOCKER_HOST` from `docker context`; `.env` seeded (`main.rs:64-115`). The chromedriver preflight is **retired** (`main.rs:128-141`).
2. `cargo fmt --check` · 3. `cargo clippy -D warnings`
4. **`xtask check-arch`**, which enforces four things the D13 rename relies on: S1 (no colour literal below the token seam, `check_arch.rs:1310-1376`), content-hashed filename = its own sha256 prefix, the `VENDOR.md` sha256 recomputes, and every `/static` reference resolves.
5. `cargo build --release`
6. **`cargo test --workspace` (excl. acceptance)**: the `lib.rs` cache tests (hashed-CSS literals, JS `no-cache`) and `projects.rs:1157` (placeholder present on an empty board).
7. `cargo deny check`
8. **`cargo test -p foundry-acceptance --release`, `FOUNDRY_ACCEPTANCE_TAGS=all`**. This includes `@needs-browser`, so **every US-CDF scenario runs here**, and so do KPI 8's shipped scenarios. It runs **only when Docker is included** (`main.rs:79-102,217-228`); see P1.

| When | Command | Runs this feature's scenarios? |
|---|---|---|
| DELIVER edit loop | `FOUNDRY_ACCEPTANCE_TAGS=us-cdf-0N cargo test -p foundry-acceptance --release --test acceptance` | Yes, that story's (`@pending` excluded) |
| Pre-commit | `cargo xtask smoke` (stages 2, 3, 4, 6) | No, but it catches a broken rename or colour literal |
| Pre-push | `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` (~15-20 min; run in the background) | Yes, all of them |
| Push / PR to `main` | GitHub `ci.yml` (same command) | Yes: the authoritative verdict |
| Push / PR to `main` | Forgejo `ci.yml` | No (browser lane cannot run) |
| After merge | `release.yml` (GHCR `:main` / `:sha-*`; `v*`) and Forgejo `build-and-publish.yml` (zot), then k8s rolling update | n/a |

No pipeline file changes.

## Wave: DEVOPS / [REF] Monitoring contracts

There is no production telemetry (DISCUSS homelab posture, D9 byte-identical POST).
Each KPI is measured by a **browser-lane scenario** (a verdict in CI), a **named
fault** that proves the scenario can fail, or a **dogfood** check recorded in the
slice's delivery notes. "Alert" means stage 8 goes red. SSOT:
`docs/product/kpi-contracts.yaml`.

| KPI | Target | Scenario oracle (browser-lane) | Named fault | Dogfood |
|---|---|---|---|---|
| 1 North star: drops work after a replace | 100% over the 5 in-place refreshes, plus the lane-header reorder as a guard (DISTILL Upstream Issue #1) | US-CDF-01 regression + refresh outline + header-reorder guard scenario: the synthetic `dragover` on the replacement lane is `defaultPrevented`; card state after a reload; `window.__cdfMark` survives (no reload) | M1 | Slice 01: delete from the popup, Move list right, Insert list after, then drag with no reload |
| 2 No "just in case" reloads | 0 refused drops over 5 working days | — | — | **The only instrument.** Log format, one row per working day: `date | refused drops | pre-emptive reloads | notes`. Recorded in the slice-01 delivery notes, then carried into the finalize `docs/evolution/` entry. Priya keeps a daily log from the day slice 01 is on her instance: refused drops and pre-emptive reloads. It goes in slice-01 delivery notes and in the finalize `docs/evolution/` entry (the canzan-theme precedent for measured KPIs). |
| 3 Which lane will take it | Exactly 1 activated lane over a lane, 0 otherwise | US-CDF-02: count of `[data-card-drop-target]`. The AC-2.2 count is read **between** `dragleave` and the next `dragover` (DDD-8f). | M9, M4 | Slice 02: slow drag across three lanes and their cards, in both palettes; watch for flicker |
| 4 Lands on the marker | 100% | US-CDF-03: the marker's `data-before-key` = post-drop DOM = fetch-spy `after` = post-reload order | M5, M6 | Slice 03: hold still at a card boundary; drop top, middle and bottom, reloading each time |
| 5 Truthful empty lanes (guardrail) | 0 lanes with a placeholder and a card; 0 empty lanes without one | US-CDF-04: computed `display` of `.column > .empty` in every lane, reload equality, two-tab SSE delete | M7, M8 | Slice 04: all three checks, including in Safari |
| 6 Foreign drags inert (guardrail) | 0 activations, markers, requests or navigations | Foreign-drag outline, other-tab scenario, US-CDF-02/03 file scenarios: `defaultPrevented` on `dragover` and `drop`, fetch-spy count 0, URL unchanged | M2, M3 | Slice 01: a real Finder `keys.png` on a lane and on the gap between lanes, on a fresh load and after a refresh, in Chrome, Firefox and Safari (OQ-2) |
| 7 Feedback never outlives the drag (guardrail) | 0 activated lanes and 0 markers after every exit | Exit-path outlines (02, 03): both counts are 0 | M4 | Slices 02 and 03: `Escape` mid-drag; release over the header |
| 8 Shipped drag unchanged (guardrail) | 100% green, **unmodified** | `board-lane-reorder.feature:229-235`, `keyboard_shortcut_bindings.rs:3013`, lane-drag and popup-delete scenarios green in stage 8. `git diff aa8a6f6 --stat` on those files is empty. The fetch-spy body is byte-identical (AC-1.4). | M8 (revert) | — |

## Wave: DEVOPS / [REF] Deployment strategy

**Rollback first.** Each slice lands as one commit when the user commits
(D13; slice 04's partial and its `:has()` rule **must** share that commit, or
every lane shows "No issues yet"). To roll back, `git revert` that commit. The
next image from `main` (GHCR via `release.yml`, zot via `build-and-publish.yml`)
then rolls out through the existing `RollingUpdate` (3 replicas, `maxSurge: 1`,
`maxUnavailable: 0`). The revert is total: there is no migration, no stored data
and no config (DDD-10). The browser follows without intervention. The reverted
`base.html` names the previous CSS hash, a distinct immutable URL, and
`board-dnd.js` is `no-cache`, so the next load revalidates it. During the
mixed-version window, a request for a hash one pod lacks gets a `no-store` 404
(`lib.rs:336-371`), so the next load heals. That window is a transient unstyled
page, as with every CSS change today.

Revert order is 04 → 03 → 02 → 01. Slices 02-04 ride slice 01's session, and
reverting 01 reopens the RCA bug. The rollback target is exercised: slice 01's
RED-on-HEAD classification runs the suite on `aa8a6f6`, the exact tree a full
revert restores. **Rolling is kept, and canary and flags are rejected.** This is
a single-operator instance with a client-only change. The gates are the browser
lane plus dogfood, and a flag would be a second code path in `board-dnd.js`.

## Wave: DEVOPS / [REF] Mutation testing strategy

**Per-feature, ≥80%** (`CLAUDE.md`). **This feature, per DESIGN DDD-9 (accepted by the user):**

1. **cargo-mutants: not applicable.** Record it in DELIVER Phase 5 with the file
   list from `git diff --name-only aa8a6f6 -- '*.rs'` plus `git status --porcelain`
   (new files are untracked). Expected: `crates/foundry-app/src/lib.rs` (cache-test
   literals only) and `crates/foundry-acceptance/**` (test code). **If any other
   production `.rs` appears, N/A is void.** In that case run `cargo mutants --in-diff`
   on it, as `issue-card-delete` did (cargo-mutants 25.3.1).
2. **M1-M9 named faults (JS and CSS).** Each fault must turn its named scenario red.
   Procedure per fault:
   1. The slice is GREEN; the binary is built and warm (P2); Docker is up; no other lane is running.
   2. Snapshot the target: `cp crates/foundry-app/static/js/board-dnd.js "$SCRATCH/board-dnd.js.green"` (for M7, the stylesheet).
   3. Hand-seed **one** fault. **No rebuild is needed**: `/static` is served from disk (`lib.rs:410`, `ServeDir::new(static_dir())`), and every lane invocation starts a fresh Chrome container, so its cache is empty. **Never run `cargo xtask ci` with a fault seeded**, because check-arch would flag the CSS hash.
   4. Run `FOUNDRY_ACCEPTANCE_TAGS=<story tag> cargo test -p foundry-acceptance --release --test acceptance`.
   5. **Killed** means the named scenario fails on its oracle assertion. A harness panic, a WebDriver session timeout or a subprocess timeout is **not** a kill; re-run it.
   6. Restore with `cp` back, prove it with `cmp`, re-run the tag, and see GREEN.
   7. Record one row: `fault | slice | file:lines seeded | tag | named scenario | first failing assertion (verbatim) | Chrome browserVersion (WebDriver capabilities) | cmp clean | GREEN re-run`.

   | Fault | Run in slice | Tag | Must RED |
   |---|---|---|---|
   | M1 per-lane binding at load | 01 | `us-cdf-01` | Regression + outline |
   | M2 no session-null guard | 01 | `us-cdf-01` | Foreign outline; other-tab scenario |
   | M3 no `preventDefault()` on a foreign `drop` | 01 | `us-cdf-01` | Swallow outline |
   | M4 no teardown on `dragend` | 03 (covers 01, 02, 03) | `cdf` | Exit-path outlines; cancelled-drag scenario |
   | M5 drop slot at an offset `clientY` | 03 | `us-cdf-03` | Lands where the marker showed |
   | M6 dragged card counted as a neighbour | 03 | `us-cdf-03` | Never offers its own slot |
   | M7 `:has()` pair removed | 04 | `us-cdf-04` | Both-lanes and remote-delete scenarios |
   | M8 no revert on non-2xx | 04 (covers 01, 03, 04) | `cdf` | Refused-drop scenarios |
   | M9 clear on every `dragleave` | 02 | `us-cdf-02` | Lane stays lit over its cards |

   **Gate: 9/9 killed.** A survivor is a test gap that gets fixed in the slice (M9's
   trap is DDD-8f). It is measured, not argued away, as `issue-card-delete` did.

## Wave: DEVOPS / [REF] Observability stack

Carried forward and unchanged. **Logs:** foundry → Promtail → Loki. **Metrics:**
Prometheus scrapes `foundry:9090` for Grafana's "Foundry Overview". **Traces:**
none. The `/state` POST is untouched (DDD-10), so its existing server signals
are unchanged. **The browser tier sends no telemetry**, and this feature adds none.
KPI signals are the CI verdict and the dogfood notes.

## Wave: DEVOPS / [REF] Branching strategy

Trunk-based on `main` (carried forward). DELIVER works in no-commit mode. When the
user commits, each slice is **one commit** carrying its CSS rename (D13). Slice 04's
template and CSS go in the same commit. `cargo xtask smoke` runs pre-commit and
`FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` runs pre-push. CI triggers on push
and PR to `main`, and the release image comes from `main` and `v*` tags.

## Wave: DEVOPS / [REF] Coexistence matrix

| Must keep working | Held by |
|---|---|
| Lane drag (`board-lane-dnd.js`, Pointer Events) and `.lane-drop-indicator` | ADR-BOARD-LANE-007 (no shared code); shipped lane-drag scenarios, unmodified (AC-1.7) |
| `Escape` single owner, `closeTopLayer()` (BR-4) | DDD-12: no `keydown` listener in `board-dnd.js`; the shipped `@layered` scenarios |
| Keyboard card selection (`.board .issue-card[data-issue-key]`, `.kb-selected`) | Distinct hooks (`data-card-drop-target` on lanes); `keyboard_shortcut_bindings.rs:3013` unmodified |
| `board-live.js` SSE `IssueDeleted` | Unchanged (DDD-5); `issue-card-delete` two-window scenarios; US-CDF-04 remote scenarios |
| The five `#board-columns` in-place refreshes (htmx OOB, `applyBoard`) and the lane-header reorder | Unchanged inputs; the US-CDF-01 refresh outline and its header-reorder guard scenario |
| Non-board pages that load `board-dnd.js` (`base.html:49`) | F1 guard (`closest('#board-columns')` returns off the board); the existing page suites |
| Token seam S1/S2, hash = filename, `VENDOR.md` sha256, `/static` refs | check-arch in smoke and ci; the D13 rename in the same commit |
| Cache policy: JS `no-cache`, hashed CSS `immutable`, errors `no-store` | `lib.rs:318-333,336-371`; literals updated per rename |
| `projects.rs:1157` empty-board guidance | The placeholder is still rendered (ADR-BOARD-CARD-003) |
| Dev loop (`run.sh`, `restart.sh`, `stop.sh`) and the observability compose stack | Untouched |

## Wave: DEVOPS / [REF] Pre-requisites

- **P1 Docker must be included, not auto-detected.** If `docker info` fails, `cargo xtask ci` skips the **whole** acceptance step with only a stderr note and still exits green (`main.rs:85-102,217-228`). Every US-CDF scenario would go unrun. Pre-push runs with `FOUNDRY_XTASK_INCLUDE_DOCKER=1`, so a dead daemon fails instead.
- **P2 Warm binary, lane alone.** Build first, then exec the freshly built `foundry` binary 2-3 times until `/usr/bin/time -p` shows `real 0.00`. On this Mac, syspolicyd stalls a new binary in dyld at 0% CPU, which looks like a hang; `sample <pid>` showing only `_dyld_start` confirms it. Run no lane concurrently with clippy or another lane. The full lane exceeds the 10-minute tool cap, so run it in the background.
- **P3 Containerised browser only.** Chrome comes from `selenium/standalone-chrome`, which bundles a matched driver (`browser_harness.rs:205-209`). Never `brew install` a driver or Chrome. The tag is **`:latest`, a floating tag**, so every RED/GREEN observation records the Chrome `browserVersion`.
- **P4 No reload between the replace and the drag**, in every lane. The browser lane proves it with `window.__cdfMark` (DDD-8e). At dogfood, reload **once** after a JS edit, *before* the sequence (JS is `no-cache`), then never during it.
- **P5 Template and CSS edits need `./restart.sh`.** Askama templates are compiled into the binary, and `base.html` names the CSS hash. JS edits need only a reload (P4).
- **P6 `@pending` never runs** in any lane (`acceptance.rs:223-227,252-255,303-305`). The AC-1.2 RED-on-HEAD classification un-pends the scenario to observe it.
- **P7 No JS hash is needed.** `board-dnd.js` is served `no-cache` at a stable URL (`lib.rs:257-272`), guarded by `app_js_revalidates_while_hashed_css_and_vendored_libs_stay_immutable` (`lib.rs:318-333`). Only CSS edits rename (D13). This confirms DESIGN F4.
- **P8 No Node** (DB6): no Stryker, no JS test runner.

## Wave: DEVOPS / [REF] Platform coverage

| Surface | Covered by | Floor |
|---|---|---|
| Synthetic drag events (wiring) | `browser-lane`: containerised Chrome only, version floating (P3) | — |
| Real drag feel, Finder drop, `:has()` in a real engine | `dogfood` on macOS: Chrome and Firefox (every slice), Safari (slices 01 and 04) | — |
| `:has()` placeholder rule (DDD-5) | Baseline widely available | **Chrome/Edge ≥105, Safari ≥15.4, Firefox ≥121** |
| Below the floor | ADR-BOARD-CARD-003: the placeholder never shows (silent, the pre-feature behaviour for an emptied lane). Activation (outline, attribute selector) and the marker (absolute positioning) add no new floor. | — |
| `<template>` fallback | Documented only (ADR-BOARD-CARD-003, Alternatives row 1). It is adopted only if M7 or dogfood shows a supported engine displaying wrongly; that goes back to the user and brings slice 04 back to 0.75d. | — |
| Touch, keyboard move | Out of scope (D14) | — |

## Wave: DEVOPS / [REF] Changed Assumptions

1. **Browser-lane prerequisite.** The carried-forward source,
   `docs/feature/issue-edit-modal-close-icon/environments.yaml` (`dev-test`), says:
   > chromedriver + Chrome/Chromium on PATH with MATCHING major versions (xtask preflight 3 probes and refuses; the browser lane never soft-skips)

   **New:** preflight 3 is retired (`main.rs:128-141`). The prerequisite is now a
   reachable Docker daemon, because the lane runs `selenium/standalone-chrome`, which
   still probes and refuses (`browser_harness.rs:237-252`). The history file is not edited.
2. **"The browser lane runs in `cargo xtask ci`."** Source: this file, D12:
   > `@needs-browser` scenarios run in `all` / `cargo xtask ci`, not in the default lane.

   **New:** that is true only when Docker is included. An undetected daemon skips the
   acceptance step silently (P1), so the pre-push gate for this feature is
   `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`.
3. **Slice 01 dogfood browsers.** Source: `slices/slice-01-drop-after-board-replace.md`:
   > on Priya's own instance in Chrome and in Firefox

   and DESIGN OQ-2: "…swallows a real Finder file in Chrome, Firefox and Safari".
   **New:** the `dogfood` environment includes Safari for the slice-01 swallow check,
   resolving OQ-2 as DESIGN framed it. Slice 04 also uses Safari, to see `:has()` in
   a third engine. No story or AC changes.

No change affects the architecture, so `devops/upstream-changes.md` is not written.

## Wave: DEVOPS / [REF] Wave Decisions

### Key Decisions
- [DEVOPS-1] Decisions 1-9 carried forward unchanged (see *Decisions 1-9*); nothing re-asked.
- [DEVOPS-2] Three verification environments: `clean`, `browser-lane` and `dogfood`. `ci-github` is the authoritative browser gate; `ci-forgejo` cannot run it (see: `environments.yaml`).
- [DEVOPS-3] KPIs are acceptance-, fault- and dogfood-verified, with no runtime telemetry. KPI 2 is dogfood-only (see: `docs/product/kpi-contracts.yaml`).
- [DEVOPS-4] Mutation: cargo-mutants N/A with the file list; M1-M9 run by the procedure above; gate 9/9 (DDD-9).
- [DEVOPS-5] Rollback: revert per slice, 04 → 01; rolling update; no flag or canary.

### Infrastructure Summary
- Deployment: self-hosted k8s, rolling (3 / surge 1 / unavailable 0); image from `main`.
- CI/CD: GitHub Actions running `cargo xtask ci` (Forgejo mirror); trunk-based.
- Observability: Prometheus + Grafana + Loki, unchanged; no client telemetry.
- Mutation testing: per-feature; N/A for Rust; the M1-M9 named-fault table for JS and CSS.

### Constraints Established
- Pre-push is `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`; a green run with a skipped acceptance step is not a pass.
- Every browser-lane observation records the Chrome `browserVersion` (the image tag floats).
- Slice 04's partial edit and its `:has()` rule land in one commit.
- Faults are seeded only against the targeted lane, never under `cargo xtask ci`; a timeout is never a kill.
- No reload between replace and drag, in any environment.

### Upstream Changes
- None to DESIGN. Three statements are refined (see *Changed Assumptions*); there is no `upstream-changes.md`.

## Wave: DEVOPS / [REF] Triggered suggestions (`ask-intelligent`)

| Trigger | Fired | Suggested expansion |
|---|---|---|
| A KPI with no automated instrument (KPI 2) | ✓ | `kpi-instrumentation-recipes`: a dogfood log template for KPI 2 and per-KPI oracle recipes |
| Known lane failure modes (P1-P3: docker auto-skip, syspolicyd, floating Chrome) | ✓ | `runbook-drafts`: triage for a red or flaky browser lane |
| Cost, alternative targets, DR, pipeline YAML, deep observability | ✗ | No infrastructure change |

Not rendered (lean). No expansion choice was made in this wave, so no
`DocumentationDensityEvent` is owed. The helper is absent anyway, as DISCUSS recorded.

<!-- ===================================================================== -->

## Wave: DISTILL / [REF] Prior Wave Consultation

Quinn (@nw-acceptance-designer), 2026-09-13. Rigor `adr-025-scaffolded-red`,
no-commit mode. Density lean + ask-intelligent. `--policy=inherit`. `[lang-mode] rust`
(`Cargo.toml`); `[policy-mode] inherit`.

| Source | Read | What it settled for DISTILL |
|---|---|---|
| This file: DISCUSS D1-D16, US-CDF-01..04, AC-1.1..4.7, KPIs 1-8, `[HOW] Gherkin Scenarios` | + | The scenario set. The `[HOW]` expansion is the starting point, merged with each story's UAT. |
| This file: DESIGN DDD-1..12, component table, Reuse Analysis, DDD-8 seams (a)-(f), DDD-9 M1-M9 | + | The DOM hooks the oracles read, the harness shape, and the scenario names the fault table pins. |
| This file: DEVOPS environment matrix, CI outline, KPI→instrument map, mutation procedure, P1-P8 | + | Lane invocation, the warm-binary rule, Chrome version recording, `@pending` never runs. |
| `docs/feature/card-drag-drop-feedback/{discuss,design,devops}/wave-decisions.md` | - | Legacy layout, not present. The single `feature-delta.md` carries all three waves. |
| `environments.yaml`, `slices/slice-0{1,2,3,4}-*.md`, `rca-drag-after-board-replace.md` | + | Production data per slice; RCA cases A-C are the oracle's origin. |
| `docs/product/journeys/journey-card-drag-drop.yaml` | + | 9 step `failure_modes` + 5 `error_paths`, each mapped to a scenario (see *Adapter coverage*). |
| `docs/product/architecture/brief.md` `## Domain Model` | + | Invariants 1-8 are the oracle list; ubiquitous language used verbatim. No `## For Acceptance Designer` section exists; the DESIGN *Driving Ports* list stands in. |
| `adr-board-card-001/002/003` | + | DDD-8f: AC-2.2 read between `dragleave` and the next `dragover`. ADR-003: "no placeholder" = not displayed. |
| `docs/product/kpi-contracts.yaml` | + | Refined here with scenario links, gate and window per KPI. |
| `docs/product/outcomes/registry.yaml` | + | OUT-3, OUT-6, OUT-11, OUT-12; three new rows registered (see *Outcomes registered*). |
| `docs/architecture/atdd-infrastructure-policy.md` | + | Every port in scope is already a row (see *Infrastructure policy*). |
| `feature_board_lane_reorder.rs` (`drag_a_card`, `open_board_in_browser`, `drag_column`), `keyboard_shortcut_bindings.rs:3013` (`fire()`), `feature_issue_card_delete.rs` (real-browser popup delete, second window), `feature_board_lane_overflow_menu.rs` (lane menu), `browser_harness.rs`, `tests/acceptance.rs` | + | Reused by pattern; none edited except `browser_harness.rs` (extended, DDD-8b). |

### Wave-Decision Reconciliation: PASSED — 0 contradictions

DISCUSS D1-D16, DESIGN DDD-1..12 and DEVOPS 1-9 were checked pairwise. DESIGN's
four *Changed Assumptions* (placeholder writers, consumers, what a drag holds, the
slice-03 spike) and DEVOPS's three are refinements that DISCUSS pre-authorised or
that change no story or AC. None is a contradiction.

**One shared premise is factually wrong, in every wave alike, and is recorded as a
finding rather than a contradiction.** DISCUSS AC-1.1, KPI 1, DESIGN *Driving
Ports* 2 and DEVOPS *Monitoring contracts* all counted the lane **header drag** as an
in-place `#board-columns` replace. It is not: `board-lane-dnd.js` `pointerup`
(lines 248-299) moves the existing `section.column` node with `insertBefore` and
never calls `applyBoard`. Only the ⋯ menu's Move items do (lines 170-202). A moved
node keeps HEAD's per-lane listeners, so the header drag never broke card drops.
The measured RED classification below confirms this. Resolved at the final review:
every wave now says 5 in-place board refreshes + 1 lane-header reorder, and the
header drag has its own guard scenario (#2b). See *Upstream issues*.

## Wave: DISTILL / [REF] Scenario list with tags

`crates/foundry-acceptance/tests/features/card-drag-drop-feedback.feature`, feature
tag `@cdf`. **35 scenarios (outlines count once), 54 examples executed. Every one
is `@pending`**, and every one is `@needs-browser`.

| # | Scenario | Tags (besides `@cdf @needs-browser @pending`) | Examples |
|---|---|---|---|
| 1 | A card can still be dropped after the board rearranges itself in place | `@us-cdf-01 @driving_port @real-io @kpi` | 1 |
| 2 | Every way the board refreshes in place leaves every lane accepting drops | `@us-cdf-01 @driving_port @real-io @kpi` | 4 |
| 2b | Rearranging the lanes by dragging a header leaves every lane accepting drops (a guard, not a refresh) | `@us-cdf-01 @driving_port @real-io @kpi` | 1 |
| 3 | A card dropped at an exact slot after a refresh keeps that slot | `@us-cdf-01 @real-io` | 1 |
| 4 | A freshly loaded board drags exactly as it did before | `@us-cdf-01 @real-io @kpi` | 1 |
| 5 | Something dragged in from outside the page is swallowed by the board | `@us-cdf-01 @error @kpi` | 5 |
| 6 | A card dragged in from another tab moves nothing in this one | `@us-cdf-01 @error @kpi` | 1 |
| 7 | A cancelled card drag is not mistaken for the next drag | `@us-cdf-01 @error` | 1 |
| 8 | A drop the server refuses after a refresh puts the card back | `@us-cdf-01 @error @real-io` | 1 |
| 9 | The lane under a dragged card lights up, and only that lane | `@us-cdf-02 @kpi` | 1 |
| 10 | The highlight follows the card from lane to lane | `@us-cdf-02` | 1 |
| 11 | A lane stays lit while the card passes over the cards inside it | `@us-cdf-02 @edge` | 1 |
| 12 | Nothing is lit while the card is over no lane | `@us-cdf-02 @edge` | 2 |
| 13 | Every way a drag ends leaves no lane lit | `@us-cdf-02 @error @kpi` | 4 |
| 14 | Something dragged in from outside the page lights nothing | `@us-cdf-02 @error @kpi` | 1 |
| 15 | Lanes still light up after the board refreshes in place | `@us-cdf-02 @real-io` | 1 |
| 16 | The activated lane is legible in both palettes and moves nothing | `@us-cdf-02` | 2 |
| 17 | A marker shows the slot between two cards | `@us-cdf-03` | 1 |
| 18 | The card lands exactly where the marker showed, and a reload agrees | `@us-cdf-03 @driving_port @real-io @kpi` | 1 |
| 19 | The marker reaches both ends of a lane | `@us-cdf-03 @real-io @edge` | 2 |
| 20 | Reordering inside a lane never offers the card's own slot | `@us-cdf-03 @real-io @edge` | 1 |
| 21 | A still pointer keeps the marker in one place | `@us-cdf-03 @edge` | 1 |
| 22 | The marker moves with the card to another lane | `@us-cdf-03` | 1 |
| 23 | The marker never outlives the drag | `@us-cdf-03 @error @kpi` | 4 |
| 24 | Something dragged in from outside the page shows no marker | `@us-cdf-03 @error @kpi` | 1 |
| 25 | The marker works on a board that refreshed in place | `@us-cdf-03 @real-io` | 1 |
| 26 | The marker is legible in both palettes | `@us-cdf-03` | 2 |
| 27 | Dropping into an empty lane removes its placeholder | `@us-cdf-04 @real-io` | 1 |
| 28 | A lane emptied by a drag shows the placeholder | `@us-cdf-04 @real-io` | 1 |
| 29 | One drag updates both lanes at once | `@us-cdf-04 @driving_port @real-io @kpi` | 1 |
| 30 | A refused drop puts both lanes back as they were | `@us-cdf-04 @error @real-io` | 1 |
| 31 | Only a drop changes a placeholder | `@us-cdf-04 @error` | 3 |
| 32 | A lane emptied by a delete in another tab shows the placeholder | `@us-cdf-04 @real-io @kpi` | 1 |
| 33 | A delete in another tab that leaves cards behind adds no placeholder | `@us-cdf-04 @real-io @edge` | 1 |
| 34 | Placeholders stay truthful on a board that refreshed in place | `@us-cdf-04 @real-io` | 1 |

| Story | Scenarios | Examples | `@error` | `@edge` |
|---|---|---|---|---|
| US-CDF-01 | 9 | 16 | 8 | 0 |
| US-CDF-02 | 8 | 13 | 5 | 3 |
| US-CDF-03 | 10 | 15 | 5 | 4 |
| US-CDF-04 | 8 | 10 | 4 | 1 |
| **Total** | **35** | **54** | **22 (41%)** | **8** |

Error paths are **22/54 = 41%** of examples, above the 40% target. Counting edge
cases as well, the non-happy share is **30/54 = 56%**.

**AC coverage.** AC-1.1 (#1, #2; #2b as the reorder guard), AC-1.2 (#1, recorded below), AC-1.3 (#2 Review
row), AC-1.4 (#3, #4: exact body bytes and `x-csrf-token`), AC-1.5 (#5, #6),
AC-1.6 (#7, #13), AC-1.7 (KPI 8; no shipped file edited). AC-2.1 (#9, #10, #12),
AC-2.2 (#11), AC-2.3 (#13), AC-2.4 (#14), AC-2.5 and AC-2.6 (#16), AC-2.7 (#15),
AC-2.8 (DELIVER's rename; guarded by `lib.rs` tests and check-arch, not a
scenario). AC-3.1 (#17, #22), AC-3.2 (#18, #19), AC-3.3 (#18, #19, #20, #25),
AC-3.4 (#20), AC-3.5 (#21), AC-3.6 (#23), AC-3.7 (#26), AC-3.8 (#24, #25). AC-4.1
(#27, #28, #29), AC-4.2 (#28: markup equal to the server's empty-lane node),
AC-4.3 (#30), AC-4.4 (#31), AC-4.5 (#27, #28, #29), AC-4.6 (#34), AC-4.7 (#32,
#33). The DISCUSS `[HOW]` cases are all present: all five in-place refreshes (#1's
lane-menu Move plus #2's four rows) and the lane-header reorder guard (#2b), every activation exit including a refused
drop (#13), the marker slot equal to the reload (#18-#20, #25), placeholders on
the target lane, the emptied lane, a refused drop and a two-tab delete
(#27-#34), and foreign file and text drops swallowed on a fresh load and after a
replace (#5).

**DDD-9 names.** M1 #1, #2 (not #2b) · M2 #5, #6 · M3 #5 · M4 #13, #23, #7 · M5 #18 · M6 #20 ·
M7 #29, #32 · M8 #8, #23 (refused row), #30 · M9 #11. The M4 row of the DDD-9
table was edited to quote the exact titles, because "Every way a drag ends (02,
03)" named no US-CDF-03 scenario. No other row changed.

**No shipped scenario is modified** (KPI 8). `board-lane-reorder.feature`,
`keyboard-shortcut-bindings.feature`, `issue-card-delete.feature`,
`board-lane-overflow-menu.feature` and their step modules are untouched. The only
edit outside this feature's own files is visibility: `feature_canzan_theme.rs`'s
`Rgb`, `parse_colour`, `hex` and `contrast_ratio` became `pub(crate)` so the
contrast oracle is reused rather than copied (DESIGN Reuse Analysis, "Contrast
oracle — REUSE").

## Wave: DISTILL / [REF] WS strategy

**No walking skeleton** (DISCUSS D1). The file carries no `@walking_skeleton` tag,
and that absence is a decision. The Architecture of Reference applies unchanged,
and the project policy supplies each mechanism:

| Port class | Port here | Treatment | Mechanism (policy row) |
|---|---|---|---|
| Driving | The card drag gesture on the board page; the five in-place refreshes and the lane-header reorder; `IssueDeleted` on `/events` | Real adapter | Real headless Chrome (containerised `selenium/standalone-chrome`) against the in-process `spawn_app()` origin; synthetic `DragEvent`s into the real `board-dnd.js` listeners |
| Driven internal | `POST …/issues/{n}/state` → `change_issue_state` → `reposition_issue_with_outbox`; `issues`/`lanes` rows; outbox → SSE | Real | Shared testcontainers Postgres, per-scenario schema; persistence proven by a real reload |
| Driven external / non-deterministic | None in scope | — | The feature has no clock, mail or third-party port |

**Tier B: not applicable.** The journey is chained, but the SUT is browser JS, and
DB6 forbids Node, so no in-memory composition of it can exist in this repo.
Mandates 9 and 11 put every browser scenario at layer 4+ anyway (example-only, sad
paths enumerated). Property-shaped invariants (marker = landing = `after`) are
pinned by examples here and by the M5/M6 named faults.

**State-delta port.** `tests/common/state_delta.rs` was **not** bootstrapped. No
prior Rust feature in this repo did; each step module carries its own universe
snapshot. That follows the policy's inherit rule. Browser scenarios are layer 4+,
where Mandate 8 permits traditional assertions. The "nothing else changed"
universe is still asserted where it matters: the whole board's card layout before
and after a foreign drop, every lane's look before and after a reload, every other
lane in the second tab, and every card and column rectangle for AC-2.6.

## Wave: DISTILL / [REF] Adapter coverage

| Adapter | `@real-io` scenario | Covered by |
|---|---|---|
| `board-dnd.js` card drag (browser, delegated or per-lane) | YES | #1, #2, #3, #4 (the real script, real events, real POST) |
| `fetch` → `POST …/state` → `change_issue_state` → `reposition_issue_with_outbox` | YES | #3 and #18 (exact slot survives a reload); #4 (body byte-identical) |
| Server refusal path (uniform 404) → client revert | YES | #8, #30, #13/#23 refused rows (real store-level delete, real 404) |
| htmx OOB `#board-columns` (popup delete) | YES | #1, #2 row 1, #3, #5, #8, #25 |
| htmx OOB `#board-columns` (lane edit, insert, delete) | YES | #2 rows 2-4, #15 |
| `board-lane-dnd.js::applyBoard` (lane-menu Move) | YES | #1, #34 |
| `board-lane-dnd.js` header drag (Pointer Events) | YES | #2b (a guard: not a replace, see *Upstream issues*) |
| Stylesheet (activation outline, marker, `:has()` placeholder rule) | YES | #16, #26 (computed contrast), #27-#34 (computed display) |
| `partials/board_columns.html` (placeholder in every lane) | YES | #28 (displayed markup equals the server's empty-lane node) |
| outbox → LISTEN → SSE → `board-live.js` | YES | #32, #33 (two real browser sessions) |

Zero `NO — MISSING` rows.

**Journey failure modes → scenarios.** Step 1: foreign treated as a card drag
(#5, #6, #14, #24); stale cancelled state (#7). Step 2: replaced lane refuses
(#1, #2, #15, #25, #34); activation blinks over its cards (#11). Step 3: marker ≠
landing (#18, #19); oscillation (#21). Step 4: placeholder beside the card (#27,
#29); emptied lane silent (#28, #29). Step 5: optimistic ≠ server (#3, #18, #20,
#27-#29 reload equality). Error paths: drag-after-board-replace (#1, #2);
foreign-drag (#5, #6); cancelled (#7, #13, #23); refused (#8, #30, refused rows);
remote-empty (#32).

## Wave: DISTILL / [REF] Scaffolds

**None, by design** (DESIGN DDD-10). No Rust production module is added or
imported by the steps. The steps drive only shipped seams: the board page, the
popup delete, the ⋯ menu and header drag, `POST …/state`, `/events` and
`board-live.js`. The production change (`board-dnd.js`, the stylesheet,
`board_columns.html`) is DELIVER's and is not scaffolded, so zero
`SCAFFOLD: true` / `__SCAFFOLD__` markers exist for this feature. The RED is the
missing browser behaviour, observed through the DESIGN-pinned hooks
(`[data-card-drop-target]`, `[data-card-drop-marker][data-before-key]`, computed
display of `.column > .empty`, a claimed `dragover`). Each failing assertion says
`MISSING_FUNCTIONALITY` and names the story and decision.

Test infrastructure added (not production):

| File | Change |
|---|---|
| `crates/foundry-acceptance/src/support/browser_harness.rs` | EXTEND (DDD-8b/d/e): the synthetic card-drag kit (`drag_start`, `drag_start_foreign`, `drag_enter`, `drag_over`, `drag_leave`, `drag_drop`, `drag_end`; `DragSpot`, `ForeignPayload`), the `fetch` spy (`install_drag_observers`, `spied_requests`, `SpiedRequest`) and the no-reload mark (`assert_not_reloaded`). `drop` fires only on a claimed `dragover`, as a real browser does. |
| `crates/foundry-acceptance/src/steps/feature_card_drag_drop_feedback.rs` | CREATE (DDD-8a) |
| `crates/foundry-acceptance/src/world.rs` | EXTEND: `cdf_*` fields |
| `crates/foundry-acceptance/src/lib.rs`, `tests/acceptance.rs` | Register and force-link the module |
| `crates/foundry-acceptance/src/steps/feature_canzan_theme.rs` | Four helpers made `pub(crate)` for reuse; no behaviour change |

## Wave: DISTILL / [REF] Test placement

`crates/foundry-acceptance/tests/features/card-drag-drop-feedback.feature` +
`crates/foundry-acceptance/src/steps/feature_card_drag_drop_feedback.rs`,
registered in `src/lib.rs` and force-linked in `tests/acceptance.rs`. This is the
precedent of every shipped feature (one `.feature` + one `steps/feature_*.rs`).
This is the Rust row of the polyglot matrix, where `@pending` is the skip marker
and the runner excludes it in all three lanes. Step phrases are workspace-wide
names in cucumber-rs's single registry, and none collides with a shipped phrase
(verified: the run reports no ambiguous step).

## Wave: DISTILL / [REF] Driving adapter coverage

| Driving port (DESIGN) | Exercised through its own protocol by |
|---|---|
| Card drag gesture (`dragstart` on `article.issue-card`, `dragover`/`dragleave`/`drop`/`dragend`) | Every scenario: real `DragEvent`s with a real `DataTransfer` into the real listeners, in real Chrome, after a real WebDriver sign-in |
| Popup card delete (htmx, OOB) | #1, #2, #3, #5, #8, #25, #32, #33: real clicks on the card, Delete and the confirm |
| Lane edit / insert / delete (⋯ menu → dialog → OOB) | #2 rows 2-4, #15: real menu clicks, typed names, real submit |
| Lane move from the ⋯ menu (`fetch` + `applyBoard`) | #1, #34 |
| Lane header drag (Pointer Events) | #2b (reorder guard) |
| `IssueDeleted` on `/events` → `board-live.js` | #32, #33 |

Zero uncovered entry points. There is no CLI or `/api/v1` surface for this feature.

## Wave: DISTILL / [REF] Pre-requisites

- DESIGN driving ports and DOM hooks: DDD-2 (delegated listeners), DDD-3 (`data-card-drop-target`), DDD-4 (`[data-card-drop-marker][data-before-key]`), DDD-5 (placeholder in every lane, shown by `:has()`), DDD-8 seams. If DELIVER renames a hook, the stylesheet, `board-dnd.js` and the step module's constants move in one change.
- DEVOPS `browser-lane`: Docker reachable, `selenium/standalone-chrome:latest` (record `browserVersion`), warm binary, lane run alone (P1-P3). `@pending` never runs (P6), so DELIVER un-pends one slice's scenarios by removing the tag.
- Run a story: `FOUNDRY_ACCEPTANCE_TAGS=us-cdf-0N cargo test -p foundry-acceptance --release --test acceptance` (or `=cdf` / `=kpi`). The default lane never runs them (`@needs-browser`).

## Wave: DISTILL / [REF] Infrastructure policy (`--policy=inherit`)

Every port in scope already has a row: the in-process HTTP driving port, Postgres
(driven internal) and headless Chrome (containerised). **No row was added**, as
instructed. One mechanism detail is new and would belong to the Chrome row, or to a
new *Driving* row, if the user wants it recorded:

| Port | Mechanism | Note |
|---|---|---|
| Card drag gesture (native HTML5 DnD) — card-drag-drop-feedback | Synthetic `DragEvent`s with one shared `DataTransfer` per drag, dispatched by `browser_harness`'s drag kit to the element under the resolved point; `drop` only after a claimed `dragover`; `Escape` and release outside modelled as `dragend` without `drop`; a page-side `fetch` spy and `window.__cdfMark` no-reload mark installed by WebDriver `execute` | WebDriver pointer actions never start a native drag in Chrome. Real feel is each slice's dogfood (D12). |

## Wave: DISTILL / [REF] Outcomes registered

`nwave-ai outcomes register` (nwave-ai 3.15.1) exited **0** three times. The CLI
has no `related` flag, so the `related` and `artifact` fields were set by hand in
the registry's own format. As a side effect the CLI re-flowed one wrapped line of
OUT-4's `invariant_note`; the YAML value is unchanged.

| id | kind | What it pins | related |
|---|---|---|---|
| **OUT-13** | invariant | A card drag is claimed and lands after any in-place board replace, and a foreign drag is swallowed; the POST is byte-identical; the revert is by identity | OUT-3, OUT-6 |
| **OUT-14** | invariant | Marker slot = landing slot = POST `after`; exactly one activated lane and one marker during a drag, zero after every exit | OUT-3, OUT-6, OUT-13 |
| **OUT-15** | invariant | A lane displays its placeholder if and only if it holds no card; zero script writers | OUT-3, OUT-6, OUT-12 |

## Wave: DISTILL / [REF] Upstream issues

1. **The header drag is not a board replace** (see *Reconciliation*). AC-1.1,
   KPI 1, the DISCUSS and DESIGN *Driving Ports*, DEVOPS *Monitoring contracts* and
   *Coexistence*, slice 01, `environments.yaml`, `kpi-contracts.yaml`, the journey,
   `brief.md` and ADR-BOARD-CARD-001 counted it among the replace triggers.
   **Resolved at the final review (2026-09-13):** each now says 5 in-place board
   refreshes + 1 lane-header reorder, kept as a robustness guard, with a note citing
   this issue. The row left the refresh outline for its own guard scenario (#2b),
   which is still expected-GREEN. M1 cannot kill it, because the lane node survives.
2. **KPI 2 has no acceptance instrument**, as DEVOPS recorded. It stays `gate: soft`.

## Wave: DISTILL / [REF] Wave Decisions

- [DISTILL-1] All 35 scenarios (54 examples) authored `@pending`, and none is walking-skeleton (D1). DELIVER un-pends per slice: `@us-cdf-01` first.
- [DISTILL-2] The first scenario combines both refreshes the user named, a popup delete and the lane-menu Move, before a drag with no reload. `drag_a_card` is not reused (D2).
- [DISTILL-3] One synthetic-drag kit in `browser_harness`, honouring the browser's protocol (no `drop` on an unclaimed target). A kit that fired `drop` regardless would let HEAD's unwired lanes look as though they had accepted.
- [DISTILL-4] A refresh is proven: the Given waits until `#board-columns` is a different node. The header-reorder guard (#2b) asserts the persisted reorder instead, because it replaces nothing.
- [DISTILL-5] Negative oracles carry a positive control in the same scenario (#14, #24, #31), so they are RED for the right reason on HEAD rather than vacuously GREEN. "Returns to its slot" first proves the move was sent and refused.
- [DISTILL-6] "Another operator deleted X" is a store-level delete that announces nothing: the window before the live board hears of it. A real delete would make `board-live.js` remove the card before the drag.
- [DISTILL-7] No production scaffold (DDD-10); no state-delta port bootstrap (Rust precedent); no Tier B (browser JS, DB6); no policy row added.

## Wave: DISTILL / [REF] RED classification

**Gate PASSED: 54 examples — 47 RED (MISSING_FUNCTIONALITY), 7 expected-GREEN,
0 BROKEN, 0 false-GREEN.**

Observed on HEAD `aa8a6f6` (working tree: this feature's docs and test files only;
no production file changed) on 2026-09-13. Lane:
`FOUNDRY_ACCEPTANCE_TAGS=cdf cargo test -p foundry-acceptance --release --test acceptance`,
with the file's `@pending` tags stripped for the run and then restored from a copy,
proved with `cmp`. The browser was `selenium/standalone-chrome:latest` =
`sha256:cd778b6f38d9…`, **Google Chrome 151.0.7922.108 / ChromeDriver
151.0.7922.108**, on Docker Desktop 29.7.2 (macOS). The binaries were built, then
warmed until `/usr/bin/time -p` read `real 0.00`, before the lane ran alone. Totals:
54 scenarios (7 passed, 47 failed), 328 steps (281 passed, 47 failed), 0 parse
errors, 0 hook errors, 0 unmatched or ambiguous steps.

**AC-1.2 (the user's regression) is RED for the stated reason.** Scenario #1's first
failing assertion, verbatim: `MISSING_FUNCTIONALITY: Done did not claim the card
drag: the synthetic dragover on the lane now on screen was NOT defaultPrevented.`
It fails after a popup delete and a lane-menu Move, with `window.__cdfMark` still in
place, so no reload happened, and no move request was sent.

| # | Scenario (example) | Class | First failing assertion / reason |
|---|---|---|---|
| 1 | Dropped after the board rearranges itself (popup delete + menu Move) | RED | `Done did not claim the card drag` (`dragover` not `defaultPrevented`) |
| 2 | Refresh outline: popup delete / rename / insert / delete-list | RED ×4 | `the lane under AUTH-41 did not accept the drop (dragover defaultPrevented = false …)` |
| 2b | Rearranging the lanes by dragging a header (guard, not a refresh) | expected-GREEN | The header drag moves the existing lane node and never replaces `#board-columns` (`board-lane-dnd.js:248-299`), so HEAD's per-lane listeners survive. A guard row; see *Upstream issues* |
| 3 | Exact slot after a refresh | RED | Lane did not accept the drop |
| 4 | Freshly loaded board drags as before | expected-GREEN | Shipped behaviour (RCA case A). Body is exactly `state=in_progress` with an `x-csrf-token`, and the reload agrees. Guards KPI 8 |
| 5 | Swallow outline: fresh, file on Done | expected-GREEN | HEAD's per-lane `dragover`/`drop` bound at load prevent both and act on nothing (`dragged` is null), so a fresh lane swallows by accident: the KPI 6 baseline. M2/M3 turn it red once the delegation lands |
| 5 | Swallow outline: fresh, file on the gap between lanes | RED | `the board did not swallow the foreign drop` (the gap is `#board-columns`, which HEAD never claims) |
| 5 | Swallow outline: after popup delete, file on Done / text on the gap | RED ×2 | `the board did not swallow the foreign drop` |
| 5 | Swallow outline: fresh, text below AUTH-7 in Done | expected-GREEN | As the fresh file-on-Done row: a lane present at load |
| 6 | Card dragged in from another tab | expected-GREEN | HEAD's drop handler returns when no card was picked up on this page (`if (!card …)`), so a fresh board ignores it. M2 targets it |
| 7 | Cancelled drag not mistaken for the next | RED | `a card MOVED on a drop that was not a card drag begun on this page` (HEAD never clears `dragged` on `dragend`, so the file drop moved AUTH-41 and POSTed) |
| 8 | Refused after a refresh puts the card back | RED | Lane did not accept the drop |
| 9 | Lane under the card lights up | RED | `Done is not shown as activated … ([data-card-drop-target] on lanes: [])` |
| 10, 11, 12 ×2, 13 ×4 | Highlight follows / stays lit / nothing lit ×2 / every drag end ×4 | RED ×8 | Same, at the Given `Priya is dragging OPS-3 over Done` (the precondition itself is the missing feature) |
| 14 | File dragged over a lane lights nothing | RED | The positive control: `a card dragged over Done straight afterwards` is not activated |
| 15 | Lanes light after a refresh (Review inserted) | RED | `Review is not shown as activated` |
| 16 ×2 | Activated lane legible, light / dark | RED ×2 | `no lane is activated to measure` |
| 17, 19 ×2, 20, 25 | Marker between / both ends ×2 / own slot / after a refresh | RED ×5 | `no marker shows where the card would land ([data-card-drop-marker] …)` |
| 18, 21, 22, 23 ×4 | Lands where the marker showed / still pointer / moves lanes / never outlives ×4 | RED ×7 | Same, at the Given `the marker shows between AUTH-3 and AUTH-12 while Priya drags AUTH-41` |
| 24 | File shows no marker | RED | The positive control: `a card dragged over In-Progress shows no marker` |
| 26 ×2 | Marker legible, light / dark | RED ×2 | `no marker shows to measure` |
| 27, 29 | Drop into empty lane / both lanes | RED ×2 | `Staging still displays the "No issues yet" placeholder beside a card` |
| 28 | Lane emptied by a drag | RED | `In-Progress holds no card yet does not display the … placeholder` |
| 30 | Refused drop puts both lanes back | expected-GREEN | The exact-origin revert shipped in `issue-status-move`, and on HEAD nothing touches placeholders, so both lanes end truthful. The step first proves the move was sent and refused (404). M8 kills it today |
| 31 ×3 | Only a drop changes a placeholder (Escape / header / file) | RED ×3 | The positive control: after `Priya drops OPS-3 on Staging`, Staging `still displays the … placeholder beside a card` |
| 32 | Lane emptied by a delete in another tab | RED | `In-Progress holds no card yet does not display the … placeholder` (the second tab dropped OPS-7 live, with no reload) |
| 33 | Remote delete leaving cards adds no placeholder | expected-GREEN | `board-live.js` removes only the card, and a lane that had cards at render has no placeholder. The step first proves OPS-9 left the second tab live. M7 guards it once DDD-5 renders a placeholder in every lane |
| 34 | Placeholders after a refresh | RED | Lane did not accept the drop |

By story: US-CDF-01 **11 RED / 5 expected-GREEN**; US-CDF-02 **13 / 0**;
US-CDF-03 **15 / 0**; US-CDF-04 **8 / 2**.

**The gate earned its place. The first run was BROKEN and was not accepted.**

1. **BROKEN ×8, a test bug: "Step doesn't match any function."** Outline headers
   with spaces (`<board state>`, `<something happens over Staging>`, and four more)
   are not substituted by cucumber-rs 0.21 / gherkin 0.14. The step text reached
   the matcher as the literal placeholder. No shipped feature file uses a spaced
   header, which is why nothing warned. All six headers were renamed to single
   tokens (`board_state`, `happening`, `refresh_in_place`, `leaving`, `drag_end`,
   `auth_41_ends_up`). The example values, and so the business wording, are
   unchanged.
2. **Environmental ×10**, not classified: the testcontainers Postgres start timed out
   once (`WaitContainer(StartupTimeout)`), and nine examples hit Selenium session
   timeouts or a script timeout (`New session request timed out`, `No nodes support
   the capabilities`) in the cold-start burst of the first run after a build. All
   ten ran clean in run 2 with no change.
3. **One assertion message was sharpened, not changed.** "A cancelled card drag is
   not mistaken for the next drag" failed on its real oracle (`no card may move`),
   and the message now says why: `MISSING_FUNCTIONALITY`, plus the stale in-flight
   card.

**Anti-vacuity, applied before the run.** Four scenarios would have been green on
HEAD only because the feedback does not exist yet: "a file lights nothing", "a file
shows no marker", and "hover, cancel or a file never changes the placeholder". Each
carries a positive control, so each is RED for the right reason. Every "returns to
its slot" oracle first asserts the move was sent and refused, because a card that
never moved "returns" trivially. The drag kit fires `drop` only on a claimed
`dragover`. A kit that did not would have let HEAD's unwired replacement lanes
"accept" a drop, and scenarios #3, #8 and #34 would have been false-GREEN.

**Default lane result: GREEN.** `cargo test -p foundry-acceptance --release --test
acceptance` (tags unset), run alone after the `cdf` runs: **632 scenarios (632
passed), 4387 steps (4387 passed)**. That equals the baseline issue-card-delete
recorded, so nothing shipped regressed. This feature's 54 examples are `@pending`
and `@needs-browser`, and the default lane runs neither. `cargo clippy -p
foundry-acceptance --all-targets --release -- -D warnings` and `cargo fmt --check`
are clean. The first clippy pass flagged a spaced doc list, an 8-argument step (a
targeted `#[allow]`, house precedent) and two tuple fields (now the
`world::CdfLaneLook` alias). None changes behaviour.

## Wave: DISTILL / [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DISCUSS#D2 | The regression drags with no reload after the replace and is RED on HEAD for the `dragover` reason | DDD-8 | `window.__cdfMark` is asserted at every drag; the RED is recorded below with its first failing assertion |
| DISCUSS#D4 | Foreign drags are swallowed on a fresh load and after a refresh | DDD-6 | The oracle is `dragover` and `drop` both `defaultPrevented`, no move request, and the URL unchanged, across five examples |
| DISCUSS#D11 | A reload changes nothing visible after a successful drag | n/a | Every successful-drop scenario ends in a reload-equality or reload-order assertion |
| DESIGN#DDD-5 | "No placeholder" means not displayed | DDD-5 | Placeholder oracles read computed `display`, never DOM presence |
| DESIGN#DDD-8f | AC-2.2 is read between `dragleave` and the next `dragover` | DDD-8 | #11 dispatches `dragenter` + `dragleave` and reads before any `dragover`, so M9 is killable |
| DESIGN#DDD-9 | Every M1-M9 fault must turn a named scenario red | DDD-9 | Each named scenario exists under its table name; M4 names aligned |
| DEVOPS#P3 | Every browser-lane observation records Chrome's `browserVersion` | n/a | Recorded with the RED classification |
| DISCUSS#KPI 8 | The shipped card-drag scenarios stay unmodified | n/a | No shipped `.feature` or scenario step edited |

<!-- ===================================================================== -->

## Wave: DELIVER / [REF] Implementation Summary

Apex (@nw-platform-architect), DELIVER Phase 7 finalize, 2026-09-14. **Sources:**
- `deliver/roadmap.json` and `deliver/execution-log.json`;
- the four slice delivery notes;
- `deliver/mutation/mutation-report.md`, including its post-refactor re-run;
- `deliver/closing-notes.md`;
- `git log aa8a6f6..90ed631`.

All 8 roadmap steps are GREEN, across 4 phases with one per slice, run in order 01 → 04.

- **The session.** The card drag is one `CardDragSession` behind five listeners delegated
  on `document` and scoped to `#board-columns`. Lanes, the activated lane and the marker
  are resolved from the live document at event time. So all five in-place board refreshes
  leave every lane accepting drops, and so does the lane-header reorder guard (US-CDF-01,
  ADR-BOARD-CARD-001).
- **The revert.** A refused or failed move reverts by identity (card key, lane slug,
  neighbour keys), re-resolved at response time.
- **Foreign drags.** A drag with no session is claimed and swallowed with `dropEffect
  "none"`.
- **Activation.** Recomputed on every `dragover`: `data-card-drop-target`, an inset
  `--cz-muted` outline on `--cz-bg`.
- **The marker.** One zero-footprint `[data-card-drop-marker][data-before-key]` records
  the slot, and the drop lands at it (ADR-BOARD-CARD-002).
- **The placeholder.** The partial renders it in every lane, and a `:has()` rule pair
  shows it only in a lane holding no card. There are zero script writers, and
  `board-live.js` is untouched (ADR-BOARD-CARD-003). Step 04-02 proved that hypothesis
  with no production edit.

There is no handler, service, store or migration change, and the move request is
byte-identical (D9, DDD-10).

**Open questions at close:**
- OQ-1: resolved. The activated surface is `--cz-bg`.
- OQ-2: open. The real Finder drop is owed at dogfood.
- OQ-3 and OQ-4: open follow-ups.
- OQ-5: closed.
- OQ-6: done (OUT-13/14/15).

**How it landed.** The wave ran in no-commit mode: every COMMIT phase is logged
`APPROVED_SKIP`. The concurrent `foundry` session then committed it as **one** commit,
`3ee56fa`. On top sits `90ed631` (rustls 0.23.40 → 0.23.45, RUSTSEC-2026-0285), which can
be reverted separately. Not pushed.

**A deviation from DEVOPS, recorded here.** DEVOPS planned one commit per slice, with
revert order 04 → 01 (*Deployment strategy*). The wave landed as a single commit, so the
rollback unit is now the whole feature. `git revert 3ee56fa` restores `aa8a6f6`'s board
tree, which is the tree DISTILL's RED classification exercised, and it keeps slice 04's
partial and its `:has()` rule together. A per-slice rollback would now need a hand-made
partial revert.

## Wave: DELIVER / [REF] Files Modified

From `git diff --name-status aa8a6f6 90ed631` (40 paths, +8585 / −124), plus the
uncommitted changes.

**Production (6):**

| Path | Status | Change |
|---|---|---|
| `crates/foundry-app/static/js/board-dnd.js` | M | The session, delegation, activation, marker and identity revert (slices 01-03); refactored in Phase 3; sha256 `bfd0f143…` |
| `crates/foundry-app/static/css/foundry.52ad52fa.css` → `foundry.f7c36a08.css` | R094 | The activation, marker and `:has()` rules (+53). The per-slice names `ed2e1ba7` and `54eb7a9b` collapse into one net rename in history |
| `crates/foundry-app/templates/partials/board_columns.html` | M | The placeholder in every lane, and the header comment |
| `crates/foundry-app/templates/base.html` | M | The stylesheet `<link>` (D13) |
| `crates/foundry-app/static/VENDOR.md` | M | The hash row and notes (D13) |
| `crates/foundry-app/src/lib.rs` | M | Only the three hashed-name literals in `static_cache_policy_tests`. This is test code inside a production file, and it is why cargo-mutants is N/A |

**Dependency (separate commit `90ed631`):** `Cargo.lock`: rustls 0.23.40 → 0.23.45 and
rustls-webpki 0.103.13 → 0.103.15. Lockfile only, with no manifest or API change.

**Tests (7):**

| Path | Status | Change |
|---|---|---|
| `crates/foundry-acceptance/tests/features/card-drag-drop-feedback.feature` | A | 35 scenario declarations |
| `crates/foundry-acceptance/src/steps/feature_card_drag_drop_feedback.rs` | A | The step module |
| `crates/foundry-acceptance/src/support/browser_harness.rs` | M | The synthetic-drag kit, the `fetch` spy, the no-reload mark, the writable foreign `dropEffect`, and `run_kit_at` |
| `crates/foundry-acceptance/src/world.rs` | M | Card-drag world state (`CdfLaneLook`), including the dead `cdf_marker_before` field the review flagged |
| `crates/foundry-acceptance/src/lib.rs` | M | The `mod` line |
| `crates/foundry-acceptance/tests/acceptance.rs` | M | Force-link lines |
| `crates/foundry-acceptance/src/steps/feature_canzan_theme.rs` | M | **Visibility only.** `Rgb`, `parse_colour`, `hex` and `contrast_ratio` become `pub(crate)`, so the contrast oracle can be reused (DESIGN *Reuse Analysis*). It is a shipped step module outside KPI 8's list, and no step or assertion changed |

KPI 8's shipped files are byte-identical to `aa8a6f6`: `git diff --stat aa8a6f6 90ed631`
is empty for them. They are five feature files (`board-lane-reorder`,
`board-lane-management`, `board-lane-overflow-menu`, `issue-card-delete`,
`keyboard-shortcut-bindings`) and their five step modules.

**Docs:**
- **Feature workspace (all new).** `feature-delta.md`, `rca-drag-after-board-replace.md`,
  `environments.yaml` and `slices/slice-01..04-*.md`. In `deliver/`: `roadmap.json`,
  `execution-log.json`, `.develop-progress.json`, `slice-01..04-delivery-notes.md` and
  `mutation/mutation-report.md`.
- **SSOT.**
  - ADR-BOARD-CARD-001/002/003 (A).
  - `architecture/brief.md` (M): `## Domain Model` bootstrapped.
  - `jobs.yaml` (M): `job-board-card-move`.
  - `journeys/journey-card-drag-drop.yaml` (A).
  - `journeys/journey-issue-card-delete.yaml` (M): a changelog entry pointing to
    *Changed Assumptions*.
  - `kpi-contracts.yaml` (A): created by DEVOPS.
  - `outcomes/registry.yaml` (M): OUT-13/14/15.
  - `personas/persona-instance-operator.yaml` (M): one line.
- **`CONTEXT.md`** (M): written by the concurrent session.

**Uncommitted at finalize (in neither commit):**
- `deliver/mutation/mutation-report.md`: modified, adding § "Post-refactor re-run (final
  code, tip 90ed631)".
- `deliver/closing-notes.md`: new and untracked.
- **This finalize pass:**
  - `docs/evolution/2026-09-14-card-drag-drop-feedback.md`: new and untracked.
  - This DELIVER record in `feature-delta.md`.
  - `docs/product/architecture/brief.md`: the shipped component inventory.
  - `docs/product/kpi-contracts.yaml`: the measured values.

## Wave: DELIVER / [REF] Scenarios Green

Units, once: 35 scenario declarations (9 Scenario Outlines) execute as 54 examples.
cucumber's `[Summary]` line calls each executed example a "scenario". Commit `3ee56fa`'s
"35 acceptance scenarios" counts declarations.

| Scope | Declarations | Examples | Final |
|---|---|---|---|
| `us-cdf-01` | 9 | 16 | 16/16 |
| `us-cdf-02` | 8 | 13 | 13/13 |
| `us-cdf-03` | 10 | 15 | 15/15 |
| `us-cdf-04` | 8 | 10 | 10/10 |
| **`cdf`** | **35** | **54** | **cdf 54/54 examples, 400/400 steps, `EXIT 0`**: tip `90ed631`, post-refactor, fresh release build, warm binary, lane run alone, load ~5 |

- **Per-story finals.** These are from the post-refactor mutation restores: `us-cdf-01`
  16/16 (127 steps), `us-cdf-02` 13/13 (85), `us-cdf-03` 15/15 (113) and `us-cdf-04`
  10/10 (75).
- **`@pending`.** Zero tags remain. The one `@pending` string in the file is its header
  comment at line 6, which is now stale.
- **Default lane.** 632/632 at 04-02, on the re-run. The first run's 631/632 was the sqlx
  `'\0'` flake.
- **Guards at slice 04.** blr 26/26, kb 38/38, icd 36/36, blm 24/24 and blo 25/25.
- **The `all` lane** (stage 8 of `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`) on the
  tip: **825/825 scenarios, 5757/5757 steps** (attempt #4 on `868090f`, all gates green).

## Wave: DELIVER / [REF] DoD Check

Against the 9 items of *DISCUSS / [REF] DoD*.

| # | DoD item | Status | Evidence |
|---|---|---|---|
| 1 | All UAT scenarios green in the `all` lane (`cargo xtask ci`); the slice-01 regression recorded RED on HEAD for the stated reason before its fix (AC-1.2) | RED on HEAD: **MET**. `all` lane: **MET**, 825/825 at `868090f` | DISTILL's RED classification on `aa8a6f6` (Chrome 151.0.7922.108): #1 failed `Done did not claim the card drag…` with `window.__cdfMark` intact. Step 01-01 recorded it RED again before any production edit. Targeted `cdf`: 54/54 examples, 400/400 steps at `90ed631` |
| 2 | Shipped card-drag, lane-drag and popup-delete scenarios green and unmodified (KPI 8) | Unmodified: **MET**. Green: **MET per tag at slice 04**; the `all`-lane re-run on the tip is part of the gate in row 1 | `git diff --stat aa8a6f6 90ed631` is empty on the five shipped feature files and their five step modules. blr 26/26, kb 38/38, icd 36/36, blm 24/24, blo 25/25. **Exception, after the fact:** the closing follow-up modifies `keyboard_shortcut_bindings.rs` (step code only; the `.feature` is untouched): a user-approved, test-only fix for a pre-existing `kb` focus race that failed the closing gate intermittently with the pre-refactor `board-dnd.js` too (`closing-notes.md` § Phase 3.5) |
| 3 | DOM oracles hold after every drag scenario: zero activated lanes, zero markers, truthful placeholders (KPIs 5, 7) | **MET** | `cdf` 54/54. The exit-path outlines are green (8 rows), as is the cancelled-drag scenario. M4, M7 and M8 were killed before and after the refactor |
| 4 | The reload-equality oracle holds after every successful drag (D11) | **MET** | Every successful-drop scenario ends in a reload assertion (DISTILL *Inherited commitments*, D11). M5's collateral reds fell on reload oracles, so they bite |
| 5 | POST body byte-identical; no handler, service, store or migration change (D9) | **MET** | The fetch-spy body is exactly `state=in_progress` plus `x-csrf-token` in "A freshly loaded board drags exactly as it did before". The diff holds no production `.rs` beyond the `lib.rs` test literals, and no migration. The partial is a template (DDD-10) |
| 6 | Activation and marker ≥3:1 in both palettes, tokens only; the rename complete in each CSS-touching slice (D6, D8, D13) | **MET** | Outline 5.89:1 / 6.38:1 against the page. Marker 14.70:1 / 17.19:1 against the activated lane. Renames: `52ad52fa` → `ed2e1ba7` (02) → `54eb7a9b` (03) → `f7c36a08` (04), each with `base.html`, the `lib.rs` literals and `VENDOR.md`. check-arch green. The single commit shows one net rename |
| 7 | Each slice's manual real-browser dogfood check recorded in its delivery notes (D12) | **MET (recorded)**; the real-browser items are owed to the user | § Dogfood in `slice-01..04-delivery-notes.md`; the owed list is under *Demo Evidence* below |
| 8 | Per-feature mutation gate: ≥80% on modified Rust production files, or N/A with the file list | **MET** | cargo-mutants N/A, with the file list: the only production `.rs` change is the three `lib.rs` literals. M1-M9: 9/9 pre-refactor and 9/9 on the final code |
| 9 | `cargo xtask ci` green (check-arch, deny); merged to main | CI: **MET**. `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` is all green at `868090f` (attempt #4). Merge: **NOT DONE**; not pushed, by the user's choice (AGENTS.md gates the push) | smoke and check-arch green at the tip. deny: RUSTSEC-2026-0285 fixed by `90ed631` |

## Wave: DELIVER / [REF] Demo Evidence

Per-slice dogfood (D12). Run by the orchestrator in Chrome, against the `./restart.sh` dev
server, on `/team/general/project/sandbox`, 2026-09-14.

| Slice | What was shown | How |
|---|---|---|
| 01 | Two in-place replaces, ⋯ **Move list right** then **left**. Each was followed by a drag of GEN-2 into an empty lane with no reload: both were POSTed (`state=done`, `state=backlog`) and persisted after a reload. A foreign `File` on a lane and on the gap, on a fresh load and after a replace: `defaultPrevented`, 0 requests, URL unchanged | Real mouse; synthetic `File` |
| 02 | Only the lane under the card lit, in light (a `rgb(92,100,95)` outline on `rgb(251,251,249)`) and in dark. Moving on lit only the next lane. The header and `Escape` left zero lit. Real-mouse drops left zero lit after each, and persisted | Synthetic `DragEvent`s into the real listeners, because a mid-drag highlight cannot be captured; real-mouse drops |
| 03 | Exactly one marker in the gap (`data-before-key=GEN-3`, 2px, absolute, `pointer-events: none`, top 307 within 303-311), in light and dark. The own-slot hover gave marker `before=""`, never naming the dragged card. `Escape` left 0 markers and 0 lit lanes | Synthetic only. **Real-mouse drags were not verified**: the automation tool delivered partial or zero DOM events |
| 04 | GEN-4, GEN-3 and GEN-2 dropped into Done and back. The placeholder's computed `display` flipped exactly with emptiness, and a reload matched each time. All six drops were claimed, and the board was left as found | Synthetic `DragEvent`s; every drop sent the real `POST …/state` |

**A correction carried from slice 01.** The `dropEffect "none"` reading in its dogfood
proves nothing: Chrome returns `"none"` for any script-built `DataTransfer`. The no-drop
answer is proven instead by the swallow outline's writable-`dropEffect` step and by M2's
kill.

**Still owed to the user:**
1. A real Finder `keys.png` dropped on a lane and on the gap, on a fresh load and after a refresh, in Chrome, Firefox and Safari (OQ-2). If a browser opens the file, apply ADR-BOARD-CARD-001's fallback.
2. A real-mouse reorder within a lane (drop at the marker, then reload), in Chrome and Firefox.
3. Real-mouse feel in Firefox and Safari, including whether the highlight flickers when it crosses a lane's own cards.
4. By hand: a remote delete that empties a lane, and a refused drop. Also `:has()` checked in Firefox and Safari.
5. The KPI 2 log: 5 working days from when slice 01 is on the instance. The table in `slice-01-delivery-notes.md` is empty.
6. Delete the temporary issues GEN-3 and GEN-4 in Sandbox. The orchestrator does not hard-delete data.

Stakeholder sign-off is the user's to give, after these items.

## Wave: DELIVER / [REF] Quality Gates

| Gate | Outcome | Evidence |
|---|---|---|
| Phase 3: refactor (L1-L4; L5 and L6 clean) | Done, behaviour-preserving | `board-dnd.js` 395 → 426 lines (sha256 `7e633e63` → `bfd0f143`): named hooks, `keyOf`, `otherCards`, `nearestCard`, `keepOneMarkerIn`, `moveBody`. In the step module: `DropPlace`, a split `refresh_in_place` and merged probes. Verified at `90ed631`: `cdf` 54/54 (400/400) and `cargo xtask smoke` green. The two earlier attempts (load 95-126, then ~24) produced infrastructure failures only and were not counted |
| Phase 4: adversarial review | **APPROVED**; 0 blocking; zero Testing-Theater patterns | Three claims discounted (below). Non-blocking: a runbook for the fault procedure, the sqlx `'\0'` flake, and the dead `cdf_marker_before` field |
| Phase 5: mutation, pre-refactor | **9/9** killed per slice (3/3, 1/1, 3/3, 2/2) | Named scenarios survived under M2, M6 and M7 at the first gate. All three were closed by strengthening their scenarios, then re-seeded red, with no assertion weakened. `mutation-report.md` § FEATURE VERDICT |
| Phase 5: mutation, post-refactor | **9/9** killed on `90ed631`; no survivors, infra errors or repeated runs | § "Post-refactor re-run". The M6 and M3 seeds were re-derived with the same semantics. Every restore was `cp` + `cmp` |
| cargo-mutants | N/A | The only production `.rs` change is the three `lib.rs` cache-test literals |
| Phase 6: integrity | `des-verify-integrity` exit 0: "All 8 steps have complete DES traces" | 47 events. The first 04-02 GREEN `FAIL` is kept in the log, followed by `PASS` |
| `cargo xtask smoke` and check-arch | Green at the tip | hash = filename, the VENDOR sha256 recomputes, `/static` references resolve, S1 |
| `cargo deny` | RUSTSEC-2026-0285 fixed by `90ed631` | A `spin` yanked-version **warning** remains; it does not fail the gate |
| Phase 3.5: `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` | **GREEN** on `868090f` (attempt #4): all gates; acceptance 825/825 scenarios, 5757/5757 steps | Attempts #1-#3 failed on causes outside the branch: `grant_super_admin` PoolTimedOut; the `kb` focus race, fixed in `868090f`; and `SSLRequest 0x48` at load 95. See `closing-notes.md` § Phase 3.5 |

**The reviewer's discounted claims, unverified or wrong, from `closing-notes.md`:**
- that M1-M9 had been re-seeded on the post-refactor code (not at that time; that was
  done afterwards, in Phase 5);
- that the post-refactor runs were green (none had completed);
- that pre-`:has()` browsers show the placeholder everywhere (ADR-BOARD-CARD-003: the
  `display: none` default wins, so it never shows).

## Wave: DELIVER / [REF] Commit Record

- **`3ee56fa`** `feat(board): make card drag-and-drop show where the card will land`. This
  is the wave, committed by the concurrent `foundry` session, and the refactor landed
  inside it before its verification had finished.
  - Its message ends with a **"NOT YET GATED"** paragraph. `cargo xtask ci` passed fmt,
    clippy, check-arch and `build --release`. It then failed 5 tests in `foundry-services`'
    `delete_lane_use_case` with `start postgres container: WaitContainer(StartupTimeout)`,
    which was the local Docker daemon, not the feature. So it was committed but not pushed.
  - **The final gate in *Quality Gates* supersedes that paragraph.** The orchestrator
    records the outcome here: **GREEN** at `868090f`, attempt #4 (825/825).
  - No commit is edited or amended; the message stays as written. The committed
    `CONTEXT.md` ("NOT PUSHED — the gate is not green") is superseded the same way. It
    is the orchestrator's handoff and is not edited here.
- **`90ed631`** `fix(deps): bump rustls to 0.23.45 for RUSTSEC-2026-0285`. Lockfile only,
  by the same session, and separate on purpose so it can be reverted on its own. This
  session first misattributed it to `3ee56fa` by diffing `aa8a6f6..HEAD` after the tip had
  moved. That is corrected; see the evolution doc, lesson 4.
- **`868090f`** `test(kb): wait for title focus before typing into the new-issue modal`.
  Test-only, user-approved, outside this feature's roadmap. It fixes a pre-existing `kb`
  focus race that failed closing-gate attempt #2 intermittently, with the pre-refactor
  `board-dnd.js` as well. Root cause traced, 10/10 `kb` runs green, and the `autofocus`
  fault still reds four scenarios (`closing-notes.md` § Phase 3.5).
- **The closing-docs commit** (this record, the evolution doc, closing notes and SSOT
  updates) follows `868090f`. The user chose two local commits, with no push.

## Wave: DELIVER / [REF] Pre-requisites for Finalize

1. **Untracked paths need an explicit `git add`.** They are `deliver/closing-notes.md` and `docs/evolution/2026-09-14-card-drag-drop-feedback.md`. `git commit -a` would miss them (`issue-card-delete`'s lesson).
2. **Every gate marker is filled** (done at finalize): in this file, in the evolution doc, and in `kpi-contracts.yaml` `final_gate`.
3. **Session markers, as the user decided.** `.nwave/des/deliver-session.json` is removed at
   the end, as approved. These are left for the user:
   - `.nwave/des/des-task-active`
   - `.nwave/des/des-task-active-card-drag-drop-feedback--`
   - `deliver/.develop-progress.json` (committed in `3ee56fa`)

   `.nwave/` is git-ignored, so none of the `.nwave/des` files is tracked.
4. **Push and merge (DoD 9)** are gated by AGENTS.md and by the user's no-push choice.
5. **Stale text, not edited here** (each is shipped or outside this pass's remit):
   - The `.feature` header (line 6) says every scenario is scaffolded `@pending`.
   - `feature_issue_card_delete.rs:1571` names `foundry.52ad52fa.css`. It was left by design, for KPI 8.
   - `slice-04-delivery-notes.md` still shows 04-02 smoke as `_pending_`, and the M7 closure as "being strengthened". Both are done.
   - `roadmap.json` has `validation.status: approved`, while its notes say "status stays pending_review" and `approved_at` is null.
6. **Owed telemetry:** two DISCUSS `DocumentationDensityEvent`s. The helper is absent from this installation.
7. **Follow-ups carried forward:**
   - the `.lane-drop-indicator` contrast (1.49:1);
   - the new-issue ordering bug (`position DEFAULT 0`);
   - pinning `selenium/standalone-chrome`;
   - the fault-procedure runbook;
   - the dead `cdf_marker_before` field;
   - OQ-3 and OQ-4.
