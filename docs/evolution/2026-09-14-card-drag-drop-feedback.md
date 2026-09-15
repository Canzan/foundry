# card-drag-drop-feedback — evolution archive

Make the board's card drag trustworthy and legible. It is a **bugfix-driven feature in
four slices**:
1. Drops survive an in-place board refresh (the bug).
2. The lane under a dragged card lights up.
3. A marker shows the exact landing slot.
4. Empty lanes show "No issues yet" truthfully.

Waves: DISCUSS → DESIGN → DEVOPS → DISTILL → DELIVER. No DISCOVER or DIVERGE ran; the
RCA (`rca-drag-after-board-replace.md`) was the evidence base. The feature was delivered
in **no-commit mode**, with all 8 COMMIT phases `APPROVED_SKIP`. It then landed as one
commit, `3ee56fa`, made by a concurrent `foundry` session. That session's separate
`90ed631` bumps rustls for RUSTSEC-2026-0285 and can be reverted on its own. Branch
`card-drag-drop-feedback`. **Not pushed**: that was the user's choice, and AGENTS.md gates
the push anyway.

Final `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`: **GREEN** on `868090f` (attempt #4,
2026-09-15 13:46–14:00Z). Every stage passed, and the all-tags acceptance lane ran 825/825
scenarios (5757 steps). Attempts #1–#3 failed, but never on this feature:
- #1 and #3 on `90ed631`: pre-existing testcontainers connect flakes; #3 ran on a host at load 95.
- #2: the pre-existing `kb` focus race that `868090f` fixes.

The details are in `closing-notes.md` § Phase 3.5.

## Business context

**The regression: cards could not be dragged lane to lane after an in-place board
refresh.** `board-dnd.js` bound `dragover`/`drop` to each lane once, at page load. Five
everyday actions replace `#board-columns` in place:
- the popup card delete;
- lane edit, insert and delete, by htmx OOB swap;
- a lane Move from the ⋯ menu, through `applyBoard`.

The replacement lanes carried no listener, so every drop was refused silently, with no
error and no request, until a reload. `issue-card-delete`'s popup delete (`87282d3`) made
this something Priya hit several times a session. That feature's journey had even named
the failure mode ("drops card drag bindings") before shipping it.

With the fix, the user asked for three things the board had never done:

1. **Lane activation colour.** The lane under the card should visibly answer "me".
2. **Drop before or after any card.** Positional drop already worked end to end. What was
   missing was seeing the slot before letting go.
3. **Drop into an empty lane.** A drop there was already accepted, but "No issues yet"
   stayed beside the dropped card, and a lane the drag emptied showed nothing until a
   reload.

Job `job-board-card-move` (new); persona Priya Raman. North star (KPI 1): 100% of card
drags claimed and persisted after each in-place refresh, from a baseline of 0%.

## What shipped

- **`board-dnd.js`.** One `CardDragSession` sits behind five listeners delegated on
  `document` and scoped to `#board-columns`, and every lane is resolved at event time. The
  origin is held as identity: card key, lane slug and neighbour keys. A refused or failed
  move reverts by re-resolving that identity at response time. A drag with no session (a
  file, a text selection, another tab's card) is claimed and swallowed with `dropEffect
  "none"`. Activation is shown by `data-card-drop-target`. One zero-footprint
  `[data-card-drop-marker][data-before-key]` *is* the landing slot.
- **Stylesheet, tokens only.**
  - The activated lane gets `--cz-bg` with an inset `--cz-muted` outline: 5.89:1 light,
    6.38:1 dark against the page.
  - The marker is `--cz-black`: 14.70:1 light, 17.19:1 dark against the activated lane.
  - A `:has()` rule pair controls the placeholder.
  - Renamed `52ad52fa` → `ed2e1ba7` → `54eb7a9b` → `f7c36a08` across slices 02-04.
- **`board_columns.html`.** Every lane renders the placeholder, and CSS shows it only in
  a lane holding no card. No script writes one, and `board-live.js` is untouched.
- **Unchanged server.** No handler, service, store or migration change, and the move
  request is byte-identical.
- **Tests.** 35 scenario declarations (9 Scenario Outlines) execute as 54 examples, all
  `@needs-browser` under `@cdf`.

## Key decisions

| Decision | What it settled |
|---|---|
| **D2**: the bug is slice 01, test-first | The regression was recorded RED on `aa8a6f6` for the stated reason: the synthetic `dragover` on the replacement lane is not `defaultPrevented`. It deliberately does not reuse `drag_a_card`, which reloads first and so hides the bug. |
| **D3 / DDD-1 / DDD-2 / DDD-7**: ADR-BOARD-CARD-001 | Replace-proof **by construction**: delegation plus event-time resolution, one session, and an identity revert. Rejected: re-binding after each swap. Five refreshes run over two swap mechanisms, and any future trigger could be forgotten. |
| **D4 / DDD-6**: foreign drag = any drag with no session | `dataTransfer.types` is never inspected, because another foundry tab's card carries `text/plain` exactly as ours does. |
| **DDD-3 / DDD-4**: ADR-BOARD-CARD-002 | `dragover` is the only activator, so there is no depth counter to drift and no `relatedTarget` containment that passes CI but flickers in a real engine. The marker takes no space and records the slot, so the marker and the landing cannot disagree (D7). |
| **DDD-5**: ADR-BOARD-CARD-003 and **`:has()`** | The placeholder is always rendered and shown by CSS. It has zero writers, and `<template>` is the documented fallback only. This took slice 04 from 0.75d to 0.5d and made step 04-02 a step with no production edit, which proved the hypothesis. Below the `:has()` floor (Chrome/Edge 105, Safari 15.4, Firefox 121) the placeholder never shows: silence, not a lie. |
| **DDD-9**: named faults instead of a mutation tool | cargo-mutants is N/A: the only production `.rs` change is three cache-test literals. DB6 bars Node, so Stryker was out. Faults M1-M9 were hand-seeded instead. |
| **D15**: no check-arch rule | The refresh-then-drag scenarios are the guard. A static rule would police one code shape and miss others. |
| **DEVOPS** | Rolling update, no flag or canary. Pre-push must set `FOUNDRY_XTASK_INCLUDE_DOCKER=1`, because `cargo xtask ci` silently skips the browser lane when `docker info` fails. |

## Steps completed

From `deliver/execution-log.json` (UTC, 2026-09-14). Every step records `RED_UNIT`
`SKIPPED` (`NOT_APPLICABLE`: no JS unit runner, DB6) and `COMMIT` `SKIPPED`
(`APPROVED_SKIP`: no-commit mode). There are 47 events in all.
`des-verify-integrity` reports "All 8 steps have complete DES traces".

| Step | Name | PREPARE → final GREEN | Note |
|---|---|---|---|
| 01-01 | Event-time drop claiming after any refresh, with identity revert | 11:37 → 11:48 | The regression was RED first, before any production edit |
| 01-02 | Foreign drags swallowed on the board; every drag ends clean | 11:51 → 12:06 | Closes slice 01; M1-M3 |
| 02-01 | Exactly the lane under the dragged card is activated | 12:49 → 12:59 | No stylesheet edit |
| 02-02 | Activation cleared on every exit; legible outline; slice-02 rename | 13:02 → 13:26 | OQ-1 resolved: surface `--cz-bg`; M9 |
| 03-01 | One marker shows the slot; drop lands there; slice-03 rename | 13:54 → 14:07 | The biggest step (7 scenarios) |
| 03-02 | Marker never outlives the drag, ignores foreign drags, survives refreshes | 14:22 → 14:36 | M4-M6 |
| 04-01 | Placeholder in every lane, shown by `:has()`; slice-04 rename | 15:07 → 16:53 | |
| 04-02 | Refusal, hover, foreign and remote delete keep placeholders truthful | 16:56 → 17:19 | **No production edit**. The first GREEN, an honest `FAIL` (sqlx flake), is kept in the log, followed by `PASS`. M7, M8 |

## The mutation story

The gate is per-feature, at ≥80%. **Pre-refactor, per slice: 9/9 killed** (3/3, 1/1,
3/3, 2/2). But three named scenarios survived their fault at the first gate, and each was
treated as a **test gap fixed in the slice**, never argued harmless:

- **M2 (no session-null guard).** It was killed by the other-tab scenario, but the
  swallow outline stayed green on all 5 rows, because nothing read `dropEffect`. A new
  step asserts the no-drop answer. Its first version was **vacuous**: Chrome ignores
  `dropEffect` writes on a script-built `new DataTransfer()`, so the value always reads
  back `"none"`. The drag kit now gives the foreign transfer its own writable
  `dropEffect`, starting at `"copy"`.
- **M6 (dragged card counted as a neighbour).** It survived outright. The own-slot
  scenario never hovered in the band where the fault differs, and `insertBefore(card,
  card)` is a no-op, so only the marker can show it. The scenario gained a hover one
  pixel inside the dragged card's own top edge, with a live-geometry aim guard, and a key
  oracle.
- **M7 (`:has()` pair removed).** It was killed by the both-lanes scenario. The
  remote-empty scenario never saw the placeholder *hidden* before the delete, so a
  placeholder showing everywhere passed it. A visible before-state Given closed it.

Each gap was re-seeded and went red on its new assertion, and no assertion was weakened.
**Post-refactor, on the final code at `90ed631`: 9/9 killed, no survivors, no infra
errors, no repeated runs.** The seeds for M6 and M3 were re-derived because the code
moved, keeping the original fault semantics. The final `cdf` run was 54/54 examples,
400/400 steps.

## Refactor and review

- **Phase 3 refactor (L1-L4; L5 and L6 clean).** Behaviour-preserving, scoped to
  `board-dnd.js` and this feature's test code. `board-dnd.js` went from 395 to 426 lines,
  gaining the named hooks `CARD`, `LANE`, `KEY`, `BEFORE_KEY`, the shared `otherCards` and
  `nearestCard`, and `keepOneMarkerIn` and `moveBody`. The honest verification history
  had three stages:
  1. The refactor agent ran under a saturated host (load 95-126) and produced no count.
  2. The first re-run gave 14/54, and all 40 failures were infrastructure.
  3. Once the concurrent session stood down: `cdf` 54/54 (400/400 steps) and a green
     `cargo xtask smoke` at `90ed631`.
- **Phase 4 adversarial review** (`nw-software-crafter-reviewer`): **APPROVED**, 0
  blocking, zero Testing-Theater patterns. **Three claims were discounted** as unverified
  or wrong:
  - that M1-M9 had been re-seeded on the post-refactor code (not at that time; that
    happened afterwards, in Phase 5);
  - that post-refactor runs were green (none had completed);
  - that pre-`:has()` browsers show the placeholder everywhere (ADR-003: the `display:
    none` default wins, so it never shows).

## Lessons and issues

### 1. The stale-server symptom was not the bug.

The user saw every lane refuse a drop, and "restarting the server" fixed it. That reads
as a stale server or stale cached JS. The RCA ruled that out, because the served
`board-dnd.js` was byte-identical to HEAD. The restart helped only because it forced a
fresh page load, which re-bound the per-lane listeners. The real trigger was any in-place
`#board-columns` replace. No shipped scenario could see it, because `drag_a_card` always
reloads first. From this feature on, "survives a board replace" is a tested property of
every board behaviour, not an assumption. See *Changed Assumptions* in the delta.

### 2. The sqlx `'\0'` flake is not specific to us-04, and a flake must be proven per run.

- **Step 03-01.** The default lane went 630/632, with both failures in
  `us-04-rolling-upgrade`. The isolated `slice3` run was 50/50.
- **Step 04-02.** The first GREEN was an honest `FAIL`: 631/632, with the sqlx `unknown
  message type '\0'` error in `notification-delivery-providers.feature:62`. That feature
  passed 30/30 alone, and a full re-run was 632/632.

The orchestrator accepted the flake on that evidence, and the `FAIL` stays in the log. A
crafter in 03-01 had also reported 632/632 from a run that had not finished. Every count
must now come from a finished log with its `EXIT` line.

### 3. The automation tool's real-mouse drag was unreliable.

`left_click_drag` sometimes delivered a partial sequence, and sometimes zero DOM events: a
capture-phase recorder saw not even a `pointerdown`. That looks exactly like "the product
refused the drop", and it cost several rounds before a recorder proved the page never
received input. The rule now:
- arm a recorder first, and aim at measured coordinates;
- stop after 2-3 attempts;
- verify in-flight states with synthetic `DragEvent`s into the real listeners;
- hand the real-mouse check to the user.

As a result, a real-mouse reorder within a lane was **never** verified here. The same
slice also corrected an earlier dogfood reading: `dropEffect "none"` read from a synthetic
`DataTransfer` proves nothing.

### 4. Two sessions collided, and this session misattributed a commit.

A second `foundry` session worked the repo concurrently. It:
- committed the uncommitted tree as `3ee56fa`, including the then-unverified refactor;
- rewrote `CONTEXT.md`;
- ran `cargo xtask ci`;
- ran `cargo update -p rustls`, which leaves cold binaries by recompiling ring, rustls,
  sqlx and everything above them;
- began a Docker cleanup.

All of this happened while this session ran browser lanes and seeded faults. This session
then **misread the attribution**: diffing `aa8a6f6..HEAD` after the tip had moved, it
counted the rustls bump as part of `3ee56fa`. The bump is the separate `90ed631`, and the
error was corrected. Two rules follow:
- resolve and name the exact commit before attributing a change;
- check for other sessions before a lane, a fault seed, `cargo xtask ci` or any git
  write, because a fault seeded in a static file is live for any other session's CI run.

### 5. Host overload produced false failures.

These were all infrastructure, not oracles:
- load 95-126 gave no counts at all;
- load ~24 gave `cdf` 14/54, with chromedriver session timeouts, `window-size` on a dead
  session, one `WaitTimeout`, an un-rendered sign-in, and a Postgres `StartupTimeout` in
  smoke;
- `3ee56fa`'s own "NOT YET GATED" note records 5 `delete_lane_use_case` failures, all
  `WaitContainer(StartupTimeout)`, with postgres taking 13s against the normal 1-3s.

After the stand-down and the Docker cleanup, at load ~5, `cdf` ran 54/54. A timeout is
never a result: classify infrastructure against oracle failures before counting.

### 6. A pre-existing new-issue ordering bug surfaced (out of scope).

A newly filed issue appears at the bottom of its lane but jumps to the top on reload. The
cause is migration 0012's `position INTEGER NOT NULL DEFAULT 0`, combined with
`ORDER BY position ASC, number DESC`, so new cards tie at 0 and sort newest first. It was
found in the slice-03 dogfood and is not this feature's bug.

Also: DISTILL's first RED run was **BROKEN ×8** because cucumber-rs 0.21 does not
substitute outline headers that contain spaces. The headers were renamed to single tokens.
No shipped feature file uses a spaced header, which is why nothing had warned.

## Numbers

| | |
|---|---|
| Acceptance | 35 scenario declarations (9 Scenario Outlines) execute as 54 examples; final `cdf` 54/54 examples, 400/400 steps, 0 `@pending` |
| RED on `aa8a6f6` | 47 RED (MISSING_FUNCTIONALITY), 7 expected-GREEN, 0 BROKEN (Chrome 151.0.7922.108) |
| Default lane | 632/632 (unchanged from the issue-card-delete baseline) |
| Guards (KPI 8) | blr 26/26, kb 38/38, icd 36/36, blm 24/24, blo 25/25. Shipped files byte-identical to `aa8a6f6` through `90ed631`. The follow-up `868090f` then edits `keyboard_shortcut_bindings.rs` (step code only), a user-approved fix for a pre-existing focus race |
| DELIVER steps | 8, all GREEN; 47 DES events; integrity exit 0 |
| Named faults | 9/9 pre-refactor and 9/9 on the final code; 3 test gaps closed |
| Server change | **0**: no handler, service, store or migration change |
| JS | 1 file extended (`board-dnd.js`, 426 lines), 0 dependencies, no Node |
| Final CI gate | **GREEN** on `868090f` (attempt #4): all gates; acceptance 825/825 scenarios, 5757/5757 steps |

## Owed to the user

- A real Finder `keys.png` dropped on a lane and on the gap, on a fresh load and after a
  refresh, in Chrome, Firefox and Safari (OQ-2). If a browser opens the file, apply
  ADR-BOARD-CARD-001's fallback.
- A real-mouse reorder within a lane (drop at the marker, then reload), in Chrome and
  Firefox.
- Real-mouse feel in Firefox and Safari, a remote delete that empties a lane, a refused
  drop by hand, and `:has()` checked in Firefox and Safari.
- **KPI 2**: a 5-working-day log of refused drops and pre-emptive reloads, starting when
  slice 01 is on the instance. The table is in `slice-01-delivery-notes.md` and is empty.
- Delete the temporary Sandbox issues GEN-3 and GEN-4.

## Artifacts

- `docs/feature/card-drag-drop-feedback/feature-delta.md`: the five-wave record, including
  the DELIVER sections.
- `docs/feature/card-drag-drop-feedback/rca-drag-after-board-replace.md`
- `docs/feature/card-drag-drop-feedback/deliver/`: `mutation/mutation-report.md`,
  `closing-notes.md`, `slice-01..04-delivery-notes.md`, the roadmap and the DES log.
- `docs/product/architecture/adr-board-card-001-replace-proof-drag-session.md`
- `docs/product/architecture/adr-board-card-002-dragover-activation-and-slot-marker.md`
- `docs/product/architecture/adr-board-card-003-placeholder-shown-by-css.md`
- `docs/product/architecture/brief.md` § "Domain Model" → "The board card drag session
  (browser tier)", including its shipped component inventory.
- `docs/product/journeys/journey-card-drag-drop.yaml`; the `journey-issue-card-delete.yaml`
  changelog.
- `docs/product/jobs.yaml` § `job-board-card-move`; `docs/product/outcomes/registry.yaml`
  OUT-13 / OUT-14 / OUT-15.
- `docs/product/kpi-contracts.yaml`: created by this feature, with measured values
  recorded at finalize.
- `crates/foundry-acceptance/tests/features/card-drag-drop-feedback.feature`

## Successors

- **`.lane-drop-indicator` contrast**, 1.49:1 → ≥3:1. It is a `board-lane-reorder`
  asset, tracked since DISCUSS.
- **The new-issue ordering bug** (`position DEFAULT 0`).
- **Pin `selenium/standalone-chrome`.** It floats at `:latest`, and the harness does not
  log `browserVersion`.
- **A runbook for the hand-seeded fault procedure** (a review item), and removal of the
  dead `cdf_marker_before` field in `world.rs`.
- **OQ-3**: a foreign file dropped outside `#board-columns` keeps the browser default.
  Widening the swallow is a user decision.
- **OQ-4**: the board placeholder has no style rule, and `.empty-state` looks dead.
- **A touch or keyboard card move** (D14), and a general live board.
- **The testcontainers connect flakes.** These are pre-existing and branch-independent:
  - `grant_super_admin` `PoolTimedOut`;
  - sqlx `unknown message type '\0'`;
  - `SSLRequest: 0x48` at load 95.

  Each hit one closing-gate attempt and passed on a clean rerun or a quiet host.
- **Done in this close-out:** the `kb` new-issue focus race, a pre-existing intermittent
  that failed gate attempt #2. `868090f` fixes it. The steps now wait for the title field's
  focus before typing. The root cause was traced with a keydown/focusin recorder
  (`closing-notes.md` § Phase 3.5).
