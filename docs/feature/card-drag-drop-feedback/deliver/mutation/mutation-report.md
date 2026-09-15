# Mutation report: card-drag-drop-feedback (DELIVER Phase 5)

## Why named faults, not a mutation tool (DDD-9)

The project gate is per-feature mutation testing with a kill rate of at least 80% (`CLAUDE.md`).
This feature changes no production `.rs`. Its production change is browser JS
(`crates/foundry-app/static/js/board-dnd.js`), with CSS coming in slice 04.

- DB6 forbids Node, so Stryker is unavailable.
- cargo-mutants mutates only `.rs` files.

DESIGN DDD-9 (accepted by the user) replaces a tool with the hand-seeded faults M1-M9 from
`feature-delta.md` (DEVOPS, "Mutation testing strategy").

**Procedure for each fault:**
1. Seed **one** fault in the working tree.
2. Run its story tag on the browser lane.
3. Classify the fault. **Killed** means the named scenario failed on its oracle assertion.
   Timeouts and harness panics don't count.
4. Restore the file by `cp` from the green snapshot and prove it with `cmp`.
5. Re-run the tag and confirm GREEN.

No git command ever touched the working tree (the feature is uncommitted).

Green snapshot of `board-dnd.js`: sha256 `308f4bc3de8c8e8b9b5abb9ecab63b9829180e31c0f20aa1fa800d48c029cc75`.

## cargo-mutants: not applicable

`git diff --name-only aa8a6f6 -- '*.rs'`:

```
crates/foundry-acceptance/src/lib.rs
crates/foundry-acceptance/src/support/browser_harness.rs
crates/foundry-acceptance/src/steps/feature_canzan_theme.rs
crates/foundry-acceptance/src/world.rs
crates/foundry-acceptance/tests/acceptance.rs
```

Untracked `.rs` (`git status --porcelain`):

```
crates/foundry-acceptance/src/steps/feature_card_drag_drop_feedback.rs
```

All of these files are under `crates/foundry-acceptance/**` (test code), and no production
`.rs` has changed. `crates/foundry-app/src/lib.rs`, which DEVOPS expected to see for its
cache-test literals, is not modified yet. **cargo-mutants N/A stands** as of slice 01.
Re-check this list at the later slices, since the D13 CSS rename will touch `lib.rs` literals.

## Slice 01 (M1-M3)

**Run conditions:**
- **Lane:** `FOUNDRY_ACCEPTANCE_TAGS=us-cdf-01 cargo test -p foundry-acceptance --release --test acceptance`.
  This is 9 scenarios, or 16 examples.
- **Chrome:** `selenium/standalone-chrome:latest`, image `sha256:cd778b6f…ec4d`
  (created 2026-08-11), `google-chrome --version` = **Google Chrome 151.0.7922.108**.
  The harness does not log `browserVersion`, so this was read from the lane image.
- **Validity:** no run hit a WebDriver, Postgres or subprocess timeout. Each kill below is an
  oracle assertion.

Line numbers refer to the faulted file. The diffs are against the green snapshot.

| Fault | Slice | file:lines seeded | Tag | Named scenario(s) | First failing assertion (verbatim) | Chrome | cmp clean | GREEN re-run |
|---|---|---|---|---|---|---|---|---|
| M1 per-lane binding at load | 01 | `board-dnd.js:219` (captures `#board-columns [data-column]` at init), `:223-226` (`dragover` returns before `preventDefault` for a lane not in that set), `:238-241` (the same in `drop`) | `us-cdf-01` | "A card can still be dropped after the board rearranges itself in place" **RED**. "Every way the board refreshes in place leaves every lane accepting drops" **RED, 4/4 rows**. | `assertion `left == right` failed: MISSING_FUNCTIONALITY: Done did not claim the card drag: the synthetic dragover on the lane now on screen was NOT defaultPrevented. The board was refreshed in place without a reload, and board-dnd.js bound its dragover/drop to the lanes present at page load (rca-drag-after-board-replace.md, AC-1.1)` with `left: Some(false)`, `right: Some(true)` | 151.0.7922.108 | yes | 16/16 examples, 122/122 steps |
| M2 no session-null guard | 01 | `board-dnd.js:225` (`dropEffect = "move"` unconditionally), `:237-242` (with no session, `drop` builds a `CardDragSession` from the card named by `text/plain` instead of returning) | `us-cdf-01` | "A card dragged in from another tab moves nothing in this one" **RED**. "Something dragged in from outside the page is swallowed by the board": **GREEN, 5/5 rows at the first gate. After the gap was closed (2026-09-14), RED on 5/5 rows**, on the new "the board shows that it will not take the drop" step (see the closed gap below). | Gate run (other-tab): `assertion `left == right` failed: MISSING_FUNCTIONALITY: a card MOVED on a drop that was not a card drag begun on this page (D4, AC-1.5/1.6). On HEAD the in-flight card survives a cancelled drag, so the next foreign drop is mistaken for it` with `left: [("backlog", ["AUTH-42", "AUTH-43"]), ("in_progress", ["AUTH-3", "AUTH-12", "AUTH-19"]), ("done", ["AUTH-7", "AUTH-41"])]`. Gap-closure re-seed (swallow row 1, first to fail): `assertion `left == right` failed: MISSING_FUNCTIONALITY: the board offered to take a drop it can only swallow: the foreign dragover's dropEffect was not "none", so a real browser shows a move cursor for something that names no card on this page (DDD-6, ADR-BOARD-CARD-001, AC-1.5)` with `left: Some("move")`, `right: Some("none")` | 151.0.7922.108 | yes | Gate: 16/16 examples, 122/122 steps. After closure: 16/16 examples, 127/127 steps |
| M3 no `preventDefault()` on a foreign `drop` | 01 | `board-dnd.js:233` removed and `:238` added. `preventDefault()` moves below the foreign early return, so card drops still call it. | `us-cdf-01` | "Something dragged in from outside the page is swallowed by the board" **RED, 5/5 rows**. (The other-tab scenario also went RED on its "must swallow" assertion.) | `assertion `left == right` failed: MISSING_FUNCTIONALITY: the board did not swallow the foreign drop (dragover claimed, drop claimed) — a real browser would open the file in this tab (D4, AC-1.5)` with `left: (Some(true), Some(false))`, `right: (Some(true), Some(true))` | 151.0.7922.108 | yes | 16/16 examples, 122/122 steps |

### Collateral reds

**M1.** The header-reorder guard, "Rearranging the lanes by dragging a header leaves every lane
accepting drops", stayed **GREEN**, as designed: the lane nodes survive a header drag.

Four other examples also went red, all on the same "did not claim/accept" oracle:
- "A card dropped at an exact slot after a refresh keeps that slot"
- "A drop the server refuses after a refresh puts the card back"
- Swallow-outline row 3 (a file on Done after a popup delete): the dragover and the drop were
  both unclaimed, `(Some(false), Some(false))`.

In total, M1 turned 8 of the 16 examples red.

**M3.** Six of the 16 examples went red:
- Swallow-outline rows 1-5.
- The other-tab scenario, on `the first tab must swallow the drop, not leave it to the browser (D4)`
  with `left: Some(false)`.

### M2 gap: CLOSED (DISTILL, 2026-09-14)

**Status.** The user chose to add the assertion. The swallow outline now kills M2 on all 5 rows.
The original analysis follows, then the closure.

**Original analysis (slice 01 gate).** The swallow outline could not observe the session-null
guard. Its oracle checked four things:
- the board is unchanged;
- the move-request count is unchanged;
- the URL is unchanged;
- `dragover` and `drop` are both `defaultPrevented`.

M2 affects none of those four for any of the outline's payloads. A file or an arbitrary text
selection names no card, so even a drop treated as a card drag finds nothing to move. Both events
are still prevented.

For a payload with no card key, the guard's only observable effect is
`dataTransfer.dropEffect` on `dragover`, which should be `"none"` but became `"move"`. No oracle
reads it. In a real browser `"none"` is the no-drop cursor, and it stops the drop firing at all.

M2 is killed at the fault level, by the other-tab scenario, whose payload carries a card key.
The swallow-outline half of its "Must RED" row is not met. The candidate fix belongs to the test
owner, and no test was edited here: have the swallow oracle also assert that `dropEffect` is
`"none"` after the foreign `dragover`.

About the seed: deleting the `if (!current) return;` line alone would throw on a null session. That
would be a crash mutant, not "treated as a card drag", and it would leave both scenarios GREEN. The
seed therefore resolves a card the only way a no-session drag can name one, by its `text/plain` key.
This is the way the other-tab scenario models a card from another tab.

**Closure: what was added.** A fifth line on the swallow outline, `And the board shows that it
will not take the drop` (`card-drag-drop-feedback.feature:117`). It asserts that the foreign
`dragover` over the board left `dropEffect == "none"` (DDD-6, ADR-BOARD-CARD-001). It is a
separate Then because the no-drop cursor is its own user-visible outcome, distinct from "the tab
still shows the board" (no navigation). Only acceptance code changed:
- the step: `feature_card_drag_drop_feedback.rs`, `then_board_refuses_the_drop`;
- the drag kit: `browser_harness.rs`, where `kit.over` records `lastOverEffect` and
  `last_drop_effect()` reads it back;
- the feature line above.

No shipped feature file or step module and no production file changed. The phrase matches no
other pattern among the workspace's 2113 step patterns.

**Closure: the first attempt was vacuous.** Chrome ignores `dropEffect` writes on a
script-constructed `new DataTransfer()`, which is not a real drag data store, and always reads
back `"none"`. The first version of the step went **GREEN on 5/5 rows with M2 seeded**, even
though M2 writes `"move"`.

The kit now gives the foreign transfer an own, writable `dropEffect` property, as a real drag's
store has, starting at `"copy"`: a browser offers an outside drag as a copy before the page
answers. A `"none"` read back after `dragover` is therefore the board's own answer, and GREEN
cannot pass by default.

**Closure: M2 re-seed.**
- **Seed.** Re-seeded from `<scratchpad>/M2.diff`. The diff against the green snapshot was
  byte-identical to the recorded one.
- **Before the seed.** `board-dnd.js.m2check` was taken, and `board-dnd.js.green` was
  `cmp`-identical to the working tree (sha256 `308f4bc3…cc75`).
- **Result.** us-cdf-01: **16 examples (10 passed, 6 failed), 127 steps (121 passed, 6 failed)**.

| Swallow row (board state / something / where) | Result under M2 | Failing step |
|---|---|---|
| 1. freshly loaded / file "keys.png" / Done | **RED** | the board shows that it will not take the drop: `left: Some("move")`, `right: Some("none")` |
| 2. freshly loaded / file "keys.png" / gap Backlog–In-Progress | **RED** | same, `Some("move")` vs `Some("none")` |
| 3. after popup delete / file "keys.png" / Done | **RED** | same, `Some("move")` vs `Some("none")` |
| 4. after popup delete / text selection / gap Backlog–In-Progress | **RED** | same, `Some("move")` vs `Some("none")` |
| 5. freshly loaded / text selection / empty space below AUTH-7 in Done | **RED** | same, `Some("move")` vs `Some("none")` |

The sixth red was the other-tab scenario, on the same assertion as at the gate.

**After the re-seed.** The file was restored by `cp` from the green snapshot, `cmp` was clean
(sha256 `308f4bc3…cc75`), and the us-cdf-01 re-run was **GREEN: 16/16 examples, 127/127 steps**.

### Slice 01 gate verdict

**3/3 killed** at the fault level: M1, M2 and M3 each turned a named scenario red on its
oracle assertion.

M2 now reddens **both** of its named scenarios: the other-tab scenario, and all 5 rows of the
swallow outline (2026-09-14 gap closure above). The original caveat, that M2 reddened only one
of its two named scenarios, no longer holds.

### Final state after slice 01

- `cmp <scratchpad>/board-dnd.js.green crates/foundry-app/static/js/board-dnd.js`: **clean**
  (sha256 `308f4bc3…cc75`, identical to the pre-gate snapshot).
- Final us-cdf-01 run: **16 scenarios (16 passed), 122 steps (122 passed)**. That covers all 9
  scenarios.
- After the M2 gap closure (2026-09-14), the final us-cdf-01 run is **16 scenarios (16 passed), 127
  steps (127 passed)**: the swallow outline's 5 rows each gained one step. `cmp` against
  `board-dnd.js.green` is **clean** (sha256 `308f4bc3…cc75`).

## Slice 02 (M9)

**Run conditions (2026-09-14):**
- **Lane:** `FOUNDRY_ACCEPTANCE_TAGS=us-cdf-02 cargo test -p foundry-acceptance --release --test acceptance`.
  This is 8 scenarios, or 13 examples.
- **Chrome:** `selenium/standalone-chrome:latest`, image `sha256:cd778b6f…ec4d`
  (created 2026-08-11), `google-chrome --version` = **Google Chrome 151.0.7922.108**.
  As in slice 01, this was read from the lane image, because the harness does not log `browserVersion`.
- **Preconditions:** Docker was up, no other lane or clippy was running, and the binary was warm
  (`/usr/bin/time -p target/release/foundry` read `real 0.00` after one warm-up exec). No rebuild
  was needed, because `/static` is served from disk.
- **Fresh snapshot:** `board-dnd.js` had changed since slice 01, so a new snapshot was taken:
  `<scratchpad>/board-dnd.js.green02`, sha256
  `921cd1cde55fdebc908bd79ed795711c6e69ee937688fb8553fc35d650a7d6c6`.
- **Validity:** the seeded run hit no WebDriver, Postgres or subprocess timeout. The kill is an
  oracle assertion.

**Seed** (`<scratchpad>/M9.diff`, against `board-dnd.js.green02`). This is the naive
clear-on-every-leave that ADR-BOARD-CARD-002 rejects: any `dragleave` during a session clears
activation immediately and unconditionally, ahead of the null-`relatedTarget` next-frame path.

```diff
@@ -286,6 +286,9 @@
     // A card drag leaving into nothing (`relatedTarget` null) may have left the
     // window: clear on the next frame, unless a dragover cancels it first.
     document.addEventListener("dragleave", function (event) {
+      if (session) {
+        activate(null);
+      }
       if (!boardOf(event.target) || !session || event.relatedTarget) {
         return;
       }
```

| Fault | Slice | file:lines seeded | Tag | Named scenario | First failing assertion (verbatim) | Chrome | cmp clean | GREEN re-run |
|---|---|---|---|---|---|---|---|---|
| M9 clear on every `dragleave` | 02 | `board-dnd.js:289-291` (inserted at the top of the `dragleave` listener: `if (session) { activate(null); }`) | `us-cdf-02` | "A lane stays lit while the card passes over the cards inside it" **RED**, on `Then Done is still shown as activated` (`card-drag-drop-feedback.feature:174`, step `feature_card_drag_drop_feedback.rs:1721`) | `MISSING_FUNCTIONALITY: Done is not shown as activated while the card is over it ([data-card-drop-target] on lanes: []; US-CDF-02, DDD-3)` | 151.0.7922.108 | yes (sha256 `921cd1cd…d6c6`) | 13/13 examples, 85/85 steps |

**Under the fault:** us-cdf-02 ran **13 examples (12 passed, 1 failed), 85 steps (84 passed,
1 failed)**. The only red was the named scenario. The oracle is read between the `dragleave` and
the next `dragover` (DDD-8f), and it caught the flicker as designed.

| us-cdf-02 scenario | Examples | Result under M9 |
|---|---|---|
| The lane under a dragged card lights up, and only that lane | 1 | GREEN |
| The highlight follows the card from lane to lane | 1 | GREEN |
| **A lane stays lit while the card passes over the cards inside it** | 1 | **RED** |
| Nothing is lit while the card is over no lane | 2 | GREEN, 2/2 |
| Every way a drag ends leaves no lane lit | 4 | GREEN, 4/4 |
| Something dragged in from outside the page lights nothing | 1 | GREEN |
| Lanes still light up after the board refreshes in place | 1 | GREEN |
| The activated lane is legible in both palettes and moves nothing | 2 | GREEN, 2/2 |

The other scenarios staying GREEN is expected. Each of them reads the oracle after a `dragover`,
and in the seeded code `dragover` is still the activator, so it re-lights the lane. The faulty
clear only shows in the window before that `dragover`.

**After the seed.** The file was restored by `cp` from `board-dnd.js.green02`. `cmp` was clean,
with sha256 `921cd1cd…d6c6`. The us-cdf-02 re-run was **GREEN: 13/13 examples, 85/85 steps**.

**cargo-mutants re-check (slice 02).** `git diff --name-only aa8a6f6 -- '*.rs'` now also lists
`crates/foundry-app/src/lib.rs`. Its diff is only the D13 CSS-rename literals in the cache tests:
three `foundry.52ad52fa.css` → `foundry.ed2e1ba7.css` strings. This is the change DEVOPS expected.
No other production `.rs` changed, and the untracked `.rs` is still only
`feature_card_drag_drop_feedback.rs`. **cargo-mutants N/A stands** as of slice 02.

### Slice 02 gate verdict

**1/1 killed.** M9 turned its named scenario red on its oracle assertion.

## Slice 03 (M4-M6)

**Run conditions (2026-09-14):**
- **Lanes:** M4 used `FOUNDRY_ACCEPTANCE_TAGS=cdf` (44 scenarios executed, with the 8 US-CDF-04
  scenarios still `@pending`). M5 and M6 used `FOUNDRY_ACCEPTANCE_TAGS=us-cdf-03` (10 scenarios, or
  15 examples). Every run was `cargo test -p foundry-acceptance --release --test acceptance`.
- **Chrome:** `selenium/standalone-chrome:latest`, image `sha256:cd778b6f…ec4d`
  (created 2026-08-11), `google-chrome --version` = **Google Chrome 151.0.7922.108**. As in the
  earlier slices, this was read from the lane image.
- **Preconditions:** Docker was up. No other lane or clippy was running. The binary was warm
  (`/usr/bin/time -p target/release/foundry --help` read `real 0.00`). No rebuild was needed.
- **Fresh snapshot:** `board-dnd.js` had changed since slice 02, so a new snapshot was taken:
  `<scratchpad>/board-dnd.js.green03`, sha256
  `7e633e63cb7fa8543e001f08274bc532fd6504b95173edd77eef1450ac6123e8`.
- **Validity:** none of the seeded runs hit a WebDriver, Postgres or subprocess timeout. One
  restore re-run did (M5, below). It was re-run and is not counted.

**Seeds** (`<scratchpad>/M4.diff`, `M5.diff`, `M6.diff`, each against `board-dnd.js.green03`).
Line numbers refer to the green file.

```diff
# M4: dragend no longer tears down. The drop-path end() (:365) and the stale-dragstart end() (:323) are intact.
@@ -384,7 +384,7 @@
-    document.addEventListener("dragend", endSession);
+    document.addEventListener("dragend", function () {});

# M5: the drop recomputes its slot one card height (48px) below the pointer and ignores the marker.
@@ -361,7 +361,7 @@
-      var before = lane ? current.landingIn(lane, event.clientY) : null;
+      var before = lane ? slotFor(lane, event.clientY + 48, current.card) : null;

# M6: slotFor no longer skips the dragged card.
@@ -82,9 +82,6 @@
     for (var i = 0; i < cards.length; i++) {
-      if (cards[i] === card) {
-        continue;
-      }
       var rect = cards[i].getBoundingClientRect();
```

The M5 offset of 48px is one card box: 10px + 10px padding, a 2px border and one text line
(`.issue-card`). From the gap between AUTH-3 and AUTH-12, it crosses AUTH-12's midpoint but not
AUTH-19's.

| Fault | Slice | file:lines seeded | Tag | Named scenario(s) | First failing assertion (verbatim) | Chrome | cmp clean | GREEN re-run |
|---|---|---|---|---|---|---|---|---|
| M4 no teardown on `dragend` | 03 (covers 01, 02, 03) | `board-dnd.js:387` (`dragend` listener made a no-op) | `cdf` | "A cancelled card drag is not mistaken for the next drag" **RED**. "Every way a drag ends leaves no lane lit" **RED** (the Escape row). "The marker never outlives the drag" **RED** (the Escape row). | Cancelled drag, `Then no card moves and no move request is sent` (`:140`): `assertion `left == right` failed: MISSING_FUNCTIONALITY: a card MOVED on a drop that was not a card drag begun on this page (D4, AC-1.5/1.6). On HEAD the in-flight card survives a cancelled drag, so the next foreign drop is mistaken for it` with `left: [("backlog", ["AUTH-42", "AUTH-43"]), ("in_progress", ["AUTH-3", "AUTH-12", "AUTH-19", "AUTH-41"]), ("done", ["AUTH-7"])]`, `right: [("backlog", ["AUTH-41", "AUTH-42", "AUTH-43"]), ("in_progress", ["AUTH-3", "AUTH-12", "AUTH-19"]), ("done", ["AUTH-7"])]`. Lane lit, `Then no lane is shown as activated` (`:191`): `no lane may stay activated: ["done"]`. Marker, `Then no marker shows anywhere on the board` (`:285`): `no marker may outlive or precede a card drag: [("in_progress", "AUTH-12", 307.6875)]` | 151.0.7922.108 | yes (sha256 `7e633e63…23e8`) | `cdf` 44/44 scenarios, 324/324 steps |
| M5 drop slot at an offset `clientY` | 03 | `board-dnd.js:364` (the drop's `landingIn` replaced by `slotFor(lane, event.clientY + 48, …)`) | `us-cdf-03` | "The card lands exactly where the marker showed, and a reload agrees" **RED**, on `Then In-Progress reads AUTH-3, AUTH-41, AUTH-12, AUTH-19` (`:243`) | `assertion `left == right` failed: In-Progress on screen` with `left: ["AUTH-3", "AUTH-12", "AUTH-41", "AUTH-19"]`, `right: ["AUTH-3", "AUTH-41", "AUTH-12", "AUTH-19"]` | 151.0.7922.108 | yes (sha256 `7e633e63…23e8`) | `us-cdf-03` 15/15 examples, 112/112 steps (second attempt; see below) |
| M6 dragged card counted as a neighbour | 03 | `board-dnd.js:85-87` (the `cards[i] === card` skip in `slotFor` removed) | `us-cdf-03` | "Reordering inside a lane never offers the card's own slot": **GREEN (SURVIVED) at the first gate. After the gap was closed (2026-09-14), RED**, on the new "the marker was never shown above AUTH-19 itself while she dragged it" step (see the closed gap below). | Gate run: none (`15 scenarios (15 passed)`, `112 steps (112 passed)`, `EXIT=0`). Gap-closure re-seed (`:264`): `the dragged card's own slot was offered: hover 0 showed the marker above AUTH-19 itself (data-before-key names the dragged card; AC-3.4, invariant 5): [("in_progress", "AUTH-19", 359.6875)]` | 151.0.7922.108 (gate); same lane image for the closure, version not re-read | yes (sha256 `7e633e63…23e8`) | After closure: `us-cdf-03` 15/15 examples, 113/113 steps; `cdf` 44/44 scenarios, 325/325 steps |

**M4 under the fault:** `cdf` ran **44 scenarios (41 passed, 3 failed), 322 steps (319 passed,
3 failed)**. The only reds were the three named scenarios, and each one fell on its oracle.

Within the two exit-path outlines, only the **Escape** rows went red, and the other rows stayed
GREEN. That is expected, because only the Escape row ends a drag with `dragend` alone:
- **The drop rows** ("she drops it", "…on Done", "…and the server refuses the move") are torn
  down by the drop handler's own `end()` (`:365`), which M4 leaves in place.
- **The header-release rows** ("she releases it over the page header") send a session `dragover`
  over the header before the release (`when_drag_ends`, `feature_card_drag_drop_feedback.rs:1282-1284`).
  A session `dragover` over no lane already calls `activate(null)` and `showMarker(null, …)`, so
  nothing is left for `dragend` to clear.

The stale-`dragstart` teardown did not mask the cancelled-drag scenario: its next drag is a
foreign file with no `dragstart`, so the surviving session took the drop and moved AUTH-41.

**M5 under the fault:** us-cdf-03 ran **15 examples (10 passed, 5 failed), 110 steps (105 passed,
5 failed)**. The named scenario fell on the on-screen order, its first oracle after the drop.
Four other examples also went red, each on its landing or reload oracle:
- "The marker reaches both ends of a lane", row "above the middle of AUTH-3" (`:252`): `AUTH-41 must be first in In-Progress after a reload; it reads ["AUTH-3", "AUTH-41", "AUTH-12", "AUTH-19"]`.
- "Reordering inside a lane never offers the card's own slot" (`:264`): `In-Progress after a reload`, `left: ["AUTH-3", "AUTH-12", "AUTH-19"]`.
- "The marker never outlives the drag", row "she drops it" (`:286`): `AUTH-41's neighbours`, `left: (Some("AUTH-12"), Some("AUTH-19"))`.
- "The marker works on a board that refreshed in place" (`:308`): `In-Progress after a reload`, `left: ["AUTH-3", "AUTH-12", "AUTH-19", "AUTH-41"]`.

**M5 restore re-run.** The first re-run after the `cp` (`cmp` clean) went red in the first
scenario's Background, on `postgres container port: PortNotExposed { … port: Tcp(5432) }`. That
is a Postgres container start failure, not an oracle, so it was re-run with the file untouched.
The second attempt was **GREEN: 15/15 examples, 112/112 steps**.

### M6: SURVIVED at the gate; gap CLOSED (DISTILL, 2026-09-14)

**What was observed at the gate.** With the skip removed, us-cdf-03 was **GREEN: 15 scenarios (15 passed),
112 steps (112 passed)**, with no infra error. The named scenario did not go red. The fault was
live: `/static` is served from disk with `no-cache`, and the M4 and M5 seeds went red the same
way.

**Why it survives (from the code; not an observed trace).**
- `slotFor` walks the lane's cards in DOM order and returns the first one whose midpoint is below
  `y`. The named scenario makes one session `dragover`, at `Between(AUTH-3, AUTH-12)`
  (`when_drags_up_between`, `feature_card_drag_drop_feedback.rs:1158-1168`).
- That point is above AUTH-12's midpoint, so the loop returns AUTH-12 before it reaches AUTH-19,
  the dragged card. The skip is never exercised, and seeded and green code agree.
- Removing the skip changes `slotFor` only when `y` lies between the previous card's midpoint and
  the **dragged card's own** midpoint. There, the seeded code returns the dragged card itself, and
  the green code returns the card after it (or the end).
- Every other us-cdf-03 scenario drags AUTH-41 out of Backlog into another lane, so the dragged
  card is never among the lane's cards.

**What the fault can and cannot change.** The two answers, "before the card itself" and "before
the card after it", land the card in the same place. `insertBefore(card, card)` is a no-op under
the DOM pre-insert rule ("if child is node, set child to node's next sibling"). The on-screen
order, the reload order and the POST's `after` are therefore identical under M6. **Only the marker
can show the fault:**
- its `data-before-key` names the dragged card instead of the next card (or `""`);
- for a card that is not last in its lane, the marker also sits one gap higher: above the
  dragged card instead of below it.

**Candidate fix (for the test owner; no test was edited).** Give "Reordering inside a lane never
offers the card's own slot" a session `dragover` in the dragged card's own region, such as
the upper half of the card being dragged. Then assert that the marker's `data-before-key`
never names the dragged card. A landing or reload oracle cannot kill M6 (see above).

**Gap closure (DISTILL, 2026-09-14).** The candidate fix was applied. No shipped feature file,
step module or production file was touched, and the browser harness did not change.
- **Scenario** (`card-drag-drop-feedback.feature:260-265`). Its title is unchanged. The When now reads
  `When Priya drags AUTH-19 up from its own slot to between AUTH-3 and AUTH-12`, and a new Then was
  added: `And the marker was never shown above AUTH-19 itself while she dragged it` (`:264`). The
  existing oracles are kept: the marker between AUTH-3 and AUTH-12, and the reload order AUTH-3,
  AUTH-19, AUTH-12.
- **When** (`when_drags_up_from_own_slot`, `feature_card_drag_drop_feedback.rs:1166`). It sends
  two session `dragover`s and records the markers after each one:
  - first at `DragSpot::AboveMiddleOf(AUTH-19)`, which is one pixel below the dragged card's own top
    edge;
  - then at `Between(AUTH-3, AUTH-12)`.

  The first aim is proved from live geometry. The kit's `lastPoint.y` must be below the midline of
  the card above (AUTH-12) and above AUTH-19's own midline. That is exactly the band where M6's
  `slotFor` returns the dragged card. A layout change cannot make the scenario vacuous without a
  red on that guard.
- **Then** (`then_marker_never_above_itself`, `:1979`). Every recorded hover must show a marker,
  and no recorded marker's `data-before-key` may name the dragged card.
- **No position oracle was added.** On the green code, the own-slot hover shows the END slot
  (`data-before-key=""`), whose midline is `slotMidline`'s "last card other than the dragged
  one", AUTH-12. That is the gap directly above AUTH-19. So "not in the gap above its own slot"
  would be false on the green code, and the key oracle alone carries AC-3.4.
- **Collision check:** each new phrase matches exactly one of the workspace's 2114 step
  patterns (its own).

**Closure: M6 re-seed.**
- **Seed.** Re-seeded with `patch` from `<scratchpad>/M6.diff`. The diff between the seeded file and
  `board-dnd.js.green03` was byte-identical to the recorded hunk. The binary was warm (`real 0.00`).
- **Result.** `us-cdf-03`: **15 scenarios (14 passed, 1 failed), 112 steps (111 passed, 1 failed),
  `EXIT=101`**, with no infra error. The only red was the named scenario, on the new step. Its
  preceding steps (the When with its geometry guard, and "the marker shows between AUTH-3 and
  AUTH-12") passed. Verbatim:
  `the dragged card's own slot was offered: hover 0 showed the marker above AUTH-19 itself (data-before-key names the dragged card; AC-3.4, invariant 5): [("in_progress", "AUTH-19", 359.6875)]`.
- **Restore.** `cp <scratchpad>/board-dnd.js.green03 …/board-dnd.js`, then `cmp` clean
  (sha256 `7e633e63…`). The re-run was **GREEN: 15 scenarios (15 passed), 113 steps (113 passed),
  `EXIT=0`**.

**cargo-mutants re-check (slice 03).** `git diff --name-only aa8a6f6 -- '*.rs'`:

```
crates/foundry-acceptance/src/lib.rs
crates/foundry-acceptance/src/steps/feature_canzan_theme.rs
crates/foundry-acceptance/src/support/browser_harness.rs
crates/foundry-acceptance/src/world.rs
crates/foundry-acceptance/tests/acceptance.rs
crates/foundry-app/src/lib.rs
```

The only untracked `.rs` is `crates/foundry-acceptance/src/steps/feature_card_drag_drop_feedback.rs`.
The only production `.rs` is `crates/foundry-app/src/lib.rs`. Its diff is three cache-test
literals, `foundry.52ad52fa.css` → `foundry.54eb7a9b.css` (the D13 rename, in
`static_cache_policy_tests`). **cargo-mutants N/A stands** as of slice 03.

### Slice 03 gate verdict

**3/3 killed. The gate is met** (after the 2026-09-14 M6 gap closure). M4 and M5 each turned
their named scenarios red on their oracle assertions at the first gate. There, M6 survived (2/3).
Per the procedure, the survivor was a test gap to close in the slice. The scenario was strengthened
(see the gap closure above), and a re-seed now turns it red on the new AC-3.4 assertion. No
existing assertion was weakened or removed.

### Final state after slice 03

- `cmp <scratchpad>/board-dnd.js.green03 crates/foundry-app/static/js/board-dnd.js`: **clean**
  (sha256 `7e633e63…23e8`, identical to the pre-gate snapshot).
- Final `cdf` run after the M6 restore (first gate): **44 scenarios (44 passed), 324 steps
  (324 passed)**, `EXIT=0`. It includes all 15 us-cdf-03 examples, so it also confirmed GREEN
  after M6.
- After the M6 gap closure (2026-09-14): `cmp` **clean** again (sha256 `7e633e63…`). The final `cdf`
  run was **44 scenarios (44 passed), 325 steps (325 passed)**, `EXIT=0`. That is one step more than
  before, from the new AC-3.4 step.
- No git command touched the working tree. Every restore was a `cp` from the snapshot.

## Slice 04 (M7-M8)

**Run conditions (2026-09-14):**
- **Lanes:** M7 used `FOUNDRY_ACCEPTANCE_TAGS=us-cdf-04` (8 scenarios, or 10 examples). M8 used
  `FOUNDRY_ACCEPTANCE_TAGS=cdf` (54 examples across slices 01-04, with no `@pending` left). Every run was
  `cargo test -p foundry-acceptance --release --test acceptance`.
- **Chrome:** `selenium/standalone-chrome:latest`, image `sha256:cd778b6f…ec4d`
  (created 2026-08-11), `google-chrome --version` = **Google Chrome 151.0.7922.108**. As in the
  earlier slices, this was read from the lane image, because the harness does not log `browserVersion`.
- **Preconditions:** Docker was up. No other lane or clippy was running (`pgrep -f
  'target/release/deps/acceptance'` was empty). The binary was warm (`/usr/bin/time -p
  target/release/foundry --help` read `real 0.00`). No rebuild was needed, because `/static` is
  served from disk.
- **Fresh snapshots:**
  - `<scratchpad>/foundry.f7c36a08.css.green04`, sha256
    `f7c36a0871d8ab594b6b8767d957b4fbb3eb977b81226feda2b1ed80fb87a3eb`;
  - `<scratchpad>/board-dnd.js.green04`, sha256
    `7e633e63cb7fa8543e001f08274bc532fd6504b95173edd77eef1450ac6123e8`. This is `cmp`-identical to
    `board-dnd.js.green03`, so the JS has not changed since slice 03.

**Seeds** (`<scratchpad>/M7.diff`, `M8.diff`, each against its green04 snapshot). Line numbers refer
to the green file. The stylesheet keeps its name for M7: only its contents changed.

```diff
# M7: the :has() placeholder pair removed, both rules (foundry.f7c36a08.css:409-415).
@@ -406,14 +406,7 @@
    121) the placeholder never shows, which is silence rather than a lie. */
-.column > .empty {
-  display: none;
-}
 
-.column:not(:has(> .issue-card)) > .empty {
-  display: block;
-}
-
 /* THE EYEBROW IDIOM — …

# M8: no revert on non-2xx (board-dnd.js:274). The network-error path (.catch, :277-279) is KEPT as is.
@@ -271,7 +271,6 @@
       .then(function (response) {
         if (!response.ok) {
-          origin.restore(); // revert to exact origin slot
         }
       })
```

Without the pair, `p.empty` falls back to its user-agent `display: block`, so the placeholder shows
in **every** lane, including lanes that hold cards. M8 keeps the `.catch` revert because a refused
drop is a 404 response, not a network error, so the `.catch` never runs for it. Skipping it too
would change nothing that these scenarios observe.

| Fault | Slice | file:lines seeded | Tag | Named scenario(s) | First failing assertion (verbatim, per named scenario) | Chrome | cmp clean | GREEN re-run |
|---|---|---|---|---|---|---|---|---|
| M7 `:has()` pair removed | 04 | `foundry.f7c36a08.css:409-415` (both rules, `.column > .empty { display: none }` and `.column:not(:has(> .issue-card)) > .empty { display: block }`, deleted; the file was not renamed) | `us-cdf-04` | "One drag updates both lanes at once" **RED**, on `Then Staging shows OPS-7 and no placeholder` (`:347`). "A lane emptied by a delete in another tab shows the placeholder": GREEN (SURVIVOR) at the gate; **RED since the 2026-09-14 gap closure**, on `And In-Progress in the second tab does not show the placeholder while it holds OPS-7` (`:375`). See "M7 gap: CLOSED" below. | Both-lanes: `MISSING_FUNCTIONALITY: Staging still displays the "No issues yet" placeholder beside a card (US-CDF-04, DDD-5); it reads ("staging", ["OPS-7"], true, "<p class=\"empty\">No issues yet — press <kbd>c</kbd> to file the first one.</p>")`. Remote-empty (after closure): `MISSING_FUNCTIONALITY: In-Progress still displays the "No issues yet" placeholder beside a card (US-CDF-04, DDD-5); it reads ("in_progress", ["OPS-7"], true, "<p class=\"empty\">No issues yet — press <kbd>c</kbd> to file the first one.</p>")` | 151.0.7922.108 | yes (sha256 `f7c36a08…a3eb`) | `us-cdf-04` 10/10 examples, 74/74 steps, `EXIT=0` (75/75 after the closure) |
| M8 no revert on non-2xx | 04 (covers 01, 03, 04) | `board-dnd.js:274` (`origin.restore()` removed from the `!response.ok` branch; the `.catch` network-error revert at `:277-279` kept as is) | `cdf` | "A drop the server refuses after a refresh puts the card back" **RED** (`:149`). "The marker never outlives the drag", row "she drops it and the server refuses the move" **RED** (`:287`). "A refused drop puts both lanes back as they were" **RED** (`:356`). | Refresh-refused: `assertion `left == right` failed: AUTH-43 must be back in its exact origin slot` with `left: Some(("done", Some("AUTH-7"), None))`, `right: Some(("backlog", Some("AUTH-41"), None))`. Marker refused row: `assertion `left == right` failed: AUTH-41 must be back in its exact origin slot` with `left: Some(("in_progress", Some("AUTH-3"), Some("AUTH-12")))`, `right: Some(("backlog", None, Some("AUTH-42")))`. Both-lanes refused: `assertion `left == right` failed: OPS-7 must be back in its exact origin slot` with `left: Some(("staging", None, None))`, `right: Some(("in_progress", None, None))` | 151.0.7922.108 | yes (sha256 `7e633e63…23e8`) | `cdf` 54/54 scenarios, 399/399 steps, `EXIT=0` |

**M8 under the fault:** `cdf` ran **54 scenarios (51 passed, 3 failed), 398 steps (395 passed,
3 failed)**, `EXIT=101`, with no infra error. The only reds were the three named scenarios, and
each fell on the identity-revert oracle (`assert_back_at_origin`,
`feature_card_drag_drop_feedback.rs:1697`). In every case the refusal oracle before it,
`assert_refused` (the move was SENT and answered 404), passed. So each red is the missing revert
and not a missing request. The card stayed where it was optimistically dropped.

The other refused-drop row, in "Every way a drag ends leaves no lane lit", stayed **GREEN**, as
expected: that outline reads only lane activation, which the drop handler's `end()` clears whether
or not the revert runs.

### M7: which scenarios caught it

**Under the fault:** us-cdf-04 ran **10 scenarios (2 passed, 8 failed), 69 steps (61 passed,
8 failed)**, `EXIT=101`, with no infra error. Every red fell on a "no placeholder" oracle.

| us-cdf-04 scenario | Examples | Result under M7 | Failing step |
|---|---|---|---|
| Dropping into an empty lane removes its placeholder | 1 | RED | `Then Staging shows OPS-3 and no placeholder` |
| A lane emptied by a drag shows the placeholder | 1 | GREEN | none |
| **One drag updates both lanes at once** (named) | 1 | **RED** | `Then Staging shows OPS-7 and no placeholder` |
| A refused drop puts both lanes back as they were | 1 | RED | `Then OPS-7 is back in In-Progress and In-Progress shows no placeholder` |
| Only a drop changes a placeholder | 3 | RED, 3/3 | `And once Priya drops OPS-3 on Staging its placeholder is no longer displayed` (the positive control) |
| **A lane emptied by a delete in another tab shows the placeholder** (named) | 1 | **GREEN (SURVIVOR)** at the gate; **RED** after the closure | none at the gate; after the closure, `And In-Progress in the second tab does not show the placeholder while it holds OPS-7` |
| A delete in another tab that leaves cards behind adds no placeholder | 1 | RED | `Then Done in the second tab shows OPS-11 and no placeholder`: `Done holds OPS-11 and must not display the placeholder: ("done", ["OPS-11"], true, "<p class=\"empty\">No issues yet — press <kbd>c</kbd> to file the first one.</p>")` |
| Placeholders stay truthful on a board that refreshed in place | 1 | RED | `Then Staging shows OPS-7 and no placeholder` |

The two GREEN scenarios assert only that an **empty** lane displays the placeholder. M7 leaves that
true, because the placeholder now shows everywhere.

### M7 gap: CLOSED (DISTILL, 2026-09-14)

**Status.** The scenario now proves the transition, hidden while the lane holds OPS-7 and displayed
once the remote delete empties it, and M7 turns it red. The original analysis follows, then the
closure.

**What was observed (slice 04 gate).** "A lane emptied by a delete in another tab shows the placeholder" passed all
of its steps under M7. This matches the prediction made before the run. Its sibling, "A delete in
another tab that leaves cards behind adds no placeholder", went red instead.

**Why it survives (from the step code).**
- `Then In-Progress in the second tab shows the placeholder` (`then_second_tab_placeholder`,
  `feature_card_drag_drop_feedback.rs:2243`) asserts that the placeholder is displayed and the lane is
  empty. Under M7 both are true.
- `And every other lane in the second tab is unchanged` (`then_second_tab_others`, `:2249`) compares
  each lane's look before and after the delete, and requires exactly one lane to change. Under M7
  every other lane displays the placeholder **both before and after**, so the looks are equal, and
  only In-Progress (its card list) changed.
- The When's precondition (`when_deletes_in_first_tab`, `:1399-1409`) asserts only that OPS-7 is alone in
  In-Progress. It never asserts that the placeholder was **hidden** while OPS-7 was there.

So nothing in the scenario shows that the placeholder **appeared** because of the delete. It could
have been showing all along, and under M7 it was.

**Candidate fix (for the test owner; no test was edited).** Give the scenario an oracle that
reads the placeholder as hidden before the delete. Either:
- in the When's precondition, also assert that In-Progress in the second tab does not display the
  placeholder while it holds OPS-7 (`look_of(..).2 == false`); or
- add a Then that no other lane in the second tab displays the placeholder beside a card.

The first carries the scenario's own claim, that the delete **turned** the placeholder on. At the
fault level, M7 is still killed, by "One drag updates both lanes at once".

**Closure: what was added.** A new line in the scenario's setup, `And In-Progress in the second tab
does not show the placeholder while it holds OPS-7` (`card-drag-drop-feedback.feature:375`),
between `Given Homelab Ops is open in two tabs` and the When. Its step,
`given_second_tab_no_placeholder_while_holding` (`feature_card_drag_drop_feedback.rs:1034`), reads
the second tab's lane through the suite's placeholder oracle, `look_of` (computed `display`), via
`assert_placeholder(.., false)`. It asserts that the lane holds exactly `[OPS-7]` and does **not**
display the placeholder. It is a separate, visible Given rather than a hidden check in the When,
because the "before" state is half of the scenario's own claim: a reader sees the transition
proved in the Gherkin, and the When stays a pure action. Every existing assertion was kept. Only
acceptance code changed: this feature line and this step. No shipped feature file or step module
and no production file changed. The phrase matches none of the workspace's 2114 step patterns, and
the new pattern matches none of its 3757 feature step lines. Zero `@pending` tags remain.

**Closure: proof.**
- fmt, clippy, and build: `cargo fmt --check` passed. `cargo clippy -p foundry-acceptance
  --all-targets --release -- -D warnings` passed with 0 warnings. The release test binary was
  rebuilt with `--no-run`, then `target/release/foundry` was warmed to `real 0.00`.
- Real code, before the re-seed: the stylesheet was `cmp`-clean. `us-cdf-04` ran **10 scenarios
  (10 passed), 75 steps (75 passed)**, `EXIT=0`. The new step passed.
- M7 re-seeded from `<scratchpad>/M7.diff`: the seeded hunk was byte-identical to the recorded
  diff. `us-cdf-04` ran **10 scenarios (1 passed, 9 failed), 66 steps (57 passed, 9 failed)**,
  `EXIT=101`, with no infra error. The remote-empty scenario is now **RED**, on the new step
  (`Defined: tests/features/card-drag-drop-feedback.feature:375:5`, `Matched:
  crates/foundry-acceptance/src/steps/feature_card_drag_drop_feedback.rs:1034:1`):
  `MISSING_FUNCTIONALITY: In-Progress still displays the "No issues yet" placeholder beside a card
  (US-CDF-04, DDD-5); it reads ("in_progress", ["OPS-7"], true, "<p class=\"empty\">No issues yet —
  press <kbd>c</kbd> to file the first one.</p>")`. The only GREEN scenario is "A lane emptied by a
  drag shows the placeholder", which was also GREEN at the gate and is outside this gap's scope.
- Restored with `cp <scratchpad>/foundry.f7c36a08.css.green04 …`: `cmp` **clean** (sha256
  `f7c36a08…`). `us-cdf-04` then ran **10 scenarios (10 passed), 75 steps (75 passed)**, `EXIT=0`.
  `cdf` then ran **54 scenarios (54 passed), 400 steps (400 passed)**, `EXIT=0`.

**cargo-mutants re-check (slice 04).** `git diff --name-only aa8a6f6 -- '*.rs'`:

```
crates/foundry-acceptance/src/lib.rs
crates/foundry-acceptance/src/steps/feature_canzan_theme.rs
crates/foundry-acceptance/src/support/browser_harness.rs
crates/foundry-acceptance/src/world.rs
crates/foundry-acceptance/tests/acceptance.rs
crates/foundry-app/src/lib.rs
```

Untracked `.rs` (`git status --porcelain`, also with `--untracked-files=all`):

```
crates/foundry-acceptance/src/steps/feature_card_drag_drop_feedback.rs
```

The only production `.rs` is `crates/foundry-app/src/lib.rs`. Its diff is three cache-test literals
in `static_cache_policy_tests` (`:330`, `:347`, `:366`), `foundry.52ad52fa.css` → `foundry.f7c36a08.css`,
which is the final D13 rename. Everything else is under `crates/foundry-acceptance/**` (test code).
**cargo-mutants N/A stands** for the feature.

### Slice 04 gate verdict

**2/2 killed. The gate is met** (after the 2026-09-14 M7 gap closure).
- **M7** turned "One drag updates both lanes at once" red on its oracle. At the gate, its other
  named scenario, "A lane emptied by a delete in another tab shows the placeholder", **survived**.
  That gap was closed on 2026-09-14 (see "M7 gap: CLOSED" above), and the scenario now goes red on
  its new before-state oracle.
- **M8** turned all three named scenarios red on the identity-revert oracle.

### Final state after slice 04

- `cmp <scratchpad>/foundry.f7c36a08.css.green04 crates/foundry-app/static/css/foundry.f7c36a08.css`:
  **clean** (sha256 `f7c36a08…a3eb`).
- `cmp <scratchpad>/board-dnd.js.green04 crates/foundry-app/static/js/board-dnd.js`: **clean**
  (sha256 `7e633e63…23e8`, also identical to `board-dnd.js.green03`).
- Final `cdf` run (after the M8 restore): **54 scenarios (54 passed), 399 steps (399 passed)**,
  `EXIT=0`. Final `us-cdf-04` run (after the M7 restore): **10 scenarios (10 passed), 74 steps
  (74 passed)**, `EXIT=0`.
- No lane was left running, no git command touched the working tree, and every restore was a
  `cp` from the snapshot.
- After the M7 gap closure (2026-09-14): `cmp` **clean** again (sha256 `f7c36a08…`). The final
  `us-cdf-04` run is **10 scenarios (10 passed), 75 steps (75 passed)**, `EXIT=0`. The final `cdf`
  run is **54 scenarios (54 passed), 400 steps (400 passed)**, `EXIT=0`.

## FEATURE VERDICT

**9/9 killed. The per-feature mutation gate is met** (DDD-9; `CLAUDE.md` ≥80%).

| Fault | Slice | Tag | Verdict | Named scenarios still GREEN under the fault |
|---|---|---|---|---|
| M1 per-lane binding at load | 01 | `us-cdf-01` | killed | none |
| M2 no session-null guard | 01 | `us-cdf-01` | killed | none (the swallow-outline gap was closed 2026-09-14) |
| M3 no `preventDefault()` on a foreign `drop` | 01 | `us-cdf-01` | killed | none |
| M4 no teardown on `dragend` | 03 | `cdf` | killed | none |
| M5 drop slot at an offset `clientY` | 03 | `us-cdf-03` | killed | none |
| M6 dragged card counted as a neighbour | 03 | `us-cdf-03` | killed | none (the survivor at the gate was closed 2026-09-14) |
| M7 `:has()` pair removed | 04 | `us-cdf-04` | killed | none (the remote-empty survivor at the gate was closed 2026-09-14) |
| M8 no revert on non-2xx | 04 | `cdf` | killed | none |
| M9 clear on every `dragleave` | 02 | `us-cdf-02` | killed | none |

- **cargo-mutants:** N/A. The only production `.rs` change is the three cache-test literals in
  `crates/foundry-app/src/lib.rs`.
- **No open items.** The three per-scenario gaps found at the slice gates (M2, M6 and M7) were
  all closed on 2026-09-14. Under every fault, each named scenario now goes red on its oracle.
  The M7 gap was that the remote-empty scenario never observed the placeholder hidden before the
  delete. It is closed by a before-state Given (see "M7 gap: CLOSED"). The final `cdf` run is
  **54 scenarios (54 passed), 400 steps (400 passed)**, `EXIT=0`.

## Post-refactor re-run (final code, tip 90ed631)

**Why.** M1-M9 above were killed against the pre-refactor code. The Phase-3 refactor reshaped
`board-dnd.js`, adding the named hooks `CARD`, `LANE`, `KEY` and the helpers `keyOf`, `otherCards`,
`nearestCard`, `keepOneMarkerIn` and `moveBody`. It also changed the step module. So all nine faults
were re-seeded into the final code (2026-09-14, 21:36-21:49). Each seed keeps its original fault
semantics and is re-derived against the current lines.

**Units.** The feature declares 35 scenarios. Its outlines expand them into 54 executed examples.
cucumber's `[Summary]` line calls every executed example a "scenario". The counts below are
examples and steps, copied from finished logs, each with its `EXIT=` line.

**Run conditions.**
- **HEAD and tree.** HEAD was `90ed631` on branch `card-drag-drop-feedback`. No tracked file was
  modified; `git status --short --untracked-files=no` was empty. The only untracked entry was the
  orchestrator's `deliver/closing-notes.md`, which was not touched.
- **Docker and lanes.** Docker was up (29.7.2). `pgrep -f 'target/release/deps/acceptance'` was
  empty before every run.
- **Binary.** `target/release/foundry` was warm. The first probe read `real 0.36`, so it was
  re-warmed to `real 0.00`, and every run log records `real 0.00` at its start.
- **Load average.** 4-7 during the runs.
- **Lane command.** `FOUNDRY_ACCEPTANCE_TAGS=<tag> cargo test -p foundry-acceptance --release
  --test acceptance`. Each run was logged to `<scratchpad>/M<n>.final.{run,green}.log`, and the log
  header records the sha256 prefixes of both files.
- **Chrome.** The same `selenium/standalone-chrome:latest` lane. The version was not re-read in
  this run.
- **Snapshots.** Both were taken once, before the first fault, and both are byte-identical to
  `git show HEAD:<path>`:
  - `<scratchpad>/board-dnd.js.final`, sha256
    `bfd0f14327be34c7f63c36f8392d61d1a153cb43aeac696b37efa17038e9f78d`;
  - `<scratchpad>/foundry.f7c36a08.css.final`, sha256
    `f7c36a0871d8ab594b6b8767d957b4fbb3eb977b81226feda2b1ed80fb87a3eb`.
- **Seeds.** Seeded with the Edit tool, one fault at a time. Each diff against its `.final`
  snapshot is saved as `<scratchpad>/M<n>.final.diff`.
- **Restores.** Every restore was `cp` from `.final`, proven with `cmp` for both files.
- **Git.** No git command touched the index or the working tree.
- **Validity.** No seeded run and no restore run hit a WebDriver, chromedriver, Postgres
  (`StartupTimeout`/`PortNotExposed`), sqlx or subprocess timeout. Each log was grepped for these.
  No run needed repeating.

**Where the code moved.** Line numbers are in the seeded file.
- **Slice-01 listeners.** `dragover` and `drop` moved from `:219-241` to `:370` and `:388`.
- **Lane lookup.** Both listeners now resolve the lane with `closest(LANE)`.
- **Foreign drops.** A foreign drop now falls through `lane = null` to a single `!lane` return,
  instead of an explicit `if (!current) return;`.
- **The own-slot skip.** It now lives in `otherCards`, which both `slotFor` and `slotMidline`
  use. Before the refactor, each function had its own copy.
- **The revert.** It is now in `CardDragSession.prototype.dropInto`, at `:306-313`.
- **The `dragend` listener.** It is now at `:418`.

| Fault | Seed file:lines (seeded file) | Tag | Named scenario(s) → result | First failing assertion (verbatim) | cmp clean | GREEN re-run (count) |
|---|---|---|---|---|---|---|
| M1 per-lane binding at load | `board-dnd.js:370` (`boundLanes` = `#board-columns [data-column]` captured in `init`, which runs at DOMContentLoaded), `:372-375` (`dragover` returns, before activation and `preventDefault`, for a lane not in that set), `:394-397` (the same in `drop`) | `us-cdf-01` | "A card can still be dropped after the board rearranges itself in place" **RED**. "Every way the board refreshes in place leaves every lane accepting drops" **RED, 4/4 rows**. The header-reorder guard stayed **GREEN**, as designed. | `assertion `left == right` failed: MISSING_FUNCTIONALITY: Done did not claim the card drag: the synthetic dragover on the lane now on screen was NOT defaultPrevented. The board was refreshed in place without a reload, and board-dnd.js bound its dragover/drop to the lanes present at page load (rca-drag-after-board-replace.md, AC-1.1)` with `left: Some(false)`, `right: Some(true)`. Outline rows: `MISSING_FUNCTIONALITY: the lane under AUTH-41 did not accept the drop (dragover defaultPrevented = false; a real browser only fires drop on a claimed target, so drop dispatched = false). On HEAD the lanes a board refresh put on screen carry no listener (rca-drag-after-board-replace.md)` | yes | 1: 16/16 examples, 127/127 steps, `EXIT=0` |
| M2 no session-null guard | `board-dnd.js:383` (`dropEffect = "move"` unconditionally), `:394-400` (with no session, `drop` builds a `CardDragSession` from the card named by `text/plain`). The session-gated activation and marker block (`:372-377`) is unchanged, as in the pre-refactor seed. | `us-cdf-01` | "Something dragged in from outside the page is swallowed by the board" **RED, 5/5 rows**, on the `dropEffect` step (`:117`). "A card dragged in from another tab moves nothing in this one" **RED** (`:133`). | Swallow (all 5 rows): `assertion `left == right` failed: MISSING_FUNCTIONALITY: the board offered to take a drop it can only swallow: the foreign dragover's dropEffect was not "none", so a real browser shows a move cursor for something that names no card on this page (DDD-6, ADR-BOARD-CARD-001, AC-1.5)` with `left: Some("move")`, `right: Some("none")`. Other tab: `assertion `left == right` failed: MISSING_FUNCTIONALITY: a card MOVED on a drop that was not a card drag begun on this page (D4, AC-1.5/1.6). On HEAD the in-flight card survives a cancelled drag, so the next foreign drop is mistaken for it` with `left: [("backlog", ["AUTH-42", "AUTH-43"]), ("in_progress", ["AUTH-3", "AUTH-12", "AUTH-19"]), ("done", ["AUTH-7", "AUTH-41"])]` | yes | 1: 16/16, 127/127, `EXIT=0` |
| M3 no `preventDefault()` on a foreign `drop` | `board-dnd.js:392` (the unconditional `preventDefault()` removed), `:393-395` (re-added as `if (current) { event.preventDefault(); }`, so card drops still call it and only foreign drops lose it) | `us-cdf-01` | "Something dragged in from outside the page is swallowed by the board" **RED, 5/5 rows** (`:116`). The other-tab scenario also went red, on its "must swallow" assertion. | `assertion `left == right` failed: MISSING_FUNCTIONALITY: the board did not swallow the foreign drop (dragover claimed, drop claimed) — a real browser would open the file in this tab (D4, AC-1.5)` with `left: (Some(true), Some(false))`, `right: (Some(true), Some(true))` | yes | 1: 16/16, 127/127, `EXIT=0` |
| M9 clear on every `dragleave` | `board-dnd.js:406-408` (inserted at the top of the `dragleave` listener: `if (session) { activate(null); }`) | `us-cdf-02` | "A lane stays lit while the card passes over the cards inside it" **RED**, on `Then Done is still shown as activated` (`:174`, step `:1828`). It was the only red. | `MISSING_FUNCTIONALITY: Done is not shown as activated while the card is over it ([data-card-drop-target] on lanes: []; US-CDF-02, DDD-3)` | yes | 1: 13/13, 85/85, `EXIT=0` |
| M5 drop slot at an offset `clientY` | `board-dnd.js:395` (the drop's `current.landingIn(lane, event.clientY)` replaced by `slotFor(lane, event.clientY + 48, current.card)`; the marker is ignored) | `us-cdf-03` | "The card lands exactly where the marker showed, and a reload agrees" **RED**, on `Then In-Progress reads AUTH-3, AUTH-41, AUTH-12, AUTH-19` (`:243`) | `assertion `left == right` failed: In-Progress on screen` with `left: ["AUTH-3", "AUTH-12", "AUTH-41", "AUTH-19"]`, `right: ["AUTH-3", "AUTH-41", "AUTH-12", "AUTH-19"]` | yes | 1: 15/15, 113/113, `EXIT=0` |
| M6 dragged card counted as a neighbour | `board-dnd.js:105` (`slotFor` walks `lane.querySelectorAll(CARD)` instead of `otherCards(lane, card)`, so it no longer skips the dragged card; `otherCards` and its other caller, `slotMidline`, are unchanged) | `us-cdf-03` | "Reordering inside a lane never offers the card's own slot" **RED**, on `And the marker was never shown above AUTH-19 itself while she dragged it` (`:264`, step `:2028`). It was the only red. | `the dragged card's own slot was offered: hover 0 showed the marker above AUTH-19 itself (data-before-key names the dragged card; AC-3.4, invariant 5): [("in_progress", "AUTH-19", 359.6875)]` | yes | 1: 15/15, 113/113, `EXIT=0` |
| M7 `:has()` pair removed | `foundry.f7c36a08.css:409-415` (both rules deleted; contents only, no rename). The hunk body is byte-identical to the recorded `M7.diff`. | `us-cdf-04` | "One drag updates both lanes at once" **RED** (`:347`). "A lane emptied by a delete in another tab shows the placeholder" **RED** (`:375`). | Both lanes: `MISSING_FUNCTIONALITY: Staging still displays the "No issues yet" placeholder beside a card (US-CDF-04, DDD-5); it reads ("staging", ["OPS-7"], true, "<p class=\"empty\">No issues yet — press <kbd>c</kbd> to file the first one.</p>")`. Remote empty: `MISSING_FUNCTIONALITY: In-Progress still displays the "No issues yet" placeholder beside a card (US-CDF-04, DDD-5); it reads ("in_progress", ["OPS-7"], true, "<p class=\"empty\">No issues yet — press <kbd>c</kbd> to file the first one.</p>")` | yes | 1: 10/10, 75/75, `EXIT=0` |
| M4 no teardown on `dragend` | `board-dnd.js:418` (the `dragend` listener is now `function () {}`; the drop-path `endSession()` at `:396` and the stale-`dragstart` `endSession()` at `:357` are unchanged) | `cdf` | "A cancelled card drag is not mistaken for the next drag" **RED** (`:140`). "Every way a drag ends leaves no lane lit" **RED**, Escape row (`:191`). "The marker never outlives the drag" **RED**, Escape row (`:286`). The other exit rows stayed GREEN, as at the pre-refactor gate. | Cancelled drag: `assertion `left == right` failed: MISSING_FUNCTIONALITY: a card MOVED on a drop that was not a card drag begun on this page (D4, AC-1.5/1.6). On HEAD the in-flight card survives a cancelled drag, so the next foreign drop is mistaken for it` with `left: [("backlog", ["AUTH-42", "AUTH-43"]), ("in_progress", ["AUTH-3", "AUTH-12", "AUTH-19", "AUTH-41"]), ("done", ["AUTH-7"])]`. Lane lit: `no lane may stay activated: ["done"]`. Marker: `no marker may outlive or precede a card drag: [("in_progress", "AUTH-12", 307.6875)]` | yes | 1: 54/54, 400/400, `EXIT=0` |
| M8 no revert on non-2xx | `board-dnd.js:308` (`origin.restore()` removed from the `!response.ok` branch). The `.catch` network-error revert (`:311-313`) is **kept as is**: a refused drop is a 404 response, not a network error, so the `.catch` never runs for it. | `cdf` | "A drop the server refuses after a refresh puts the card back" **RED** (`:149`). "The marker never outlives the drag", refused row **RED** (`:287`). "A refused drop puts both lanes back as they were" **RED** (`:356`). The refused row of the lit-lane outline stayed GREEN, as expected: it reads only activation. | Refresh refused: `assertion `left == right` failed: AUTH-43 must be back in its exact origin slot` with `left: Some(("done", Some("AUTH-7"), None))`, `right: Some(("backlog", Some("AUTH-41"), None))`. Marker refused: `AUTH-41 must be back in its exact origin slot` with `left: Some(("in_progress", Some("AUTH-3"), Some("AUTH-12")))`, `right: Some(("backlog", None, Some("AUTH-42")))`. Both lanes: `OPS-7 must be back in its exact origin slot` with `left: Some(("staging", None, None))`, `right: Some(("in_progress", None, None))` | yes | 1: 54/54, 400/400, `EXIT=0` (also the final `cdf` run) |

**Counts under each fault** (from each finished seeded log):

| Fault | Tag | Examples | Steps | EXIT |
|---|---|---|---|---|
| M1 | `us-cdf-01` | 16 (8 passed, 8 failed) | 113 (105 passed, 8 failed) | 101 |
| M2 | `us-cdf-01` | 16 (10 passed, 6 failed) | 127 (121 passed, 6 failed) | 101 |
| M3 | `us-cdf-01` | 16 (10 passed, 6 failed) | 122 (116 passed, 6 failed) | 101 |
| M9 | `us-cdf-02` | 13 (12 passed, 1 failed) | 85 (84 passed, 1 failed) | 101 |
| M5 | `us-cdf-03` | 15 (10 passed, 5 failed) | 111 (106 passed, 5 failed) | 101 |
| M6 | `us-cdf-03` | 15 (14 passed, 1 failed) | 112 (111 passed, 1 failed) | 101 |
| M7 | `us-cdf-04` | 10 (1 passed, 9 failed) | 66 (57 passed, 9 failed) | 101 |
| M4 | `cdf` | 54 (51 passed, 3 failed) | 398 (395 passed, 3 failed) | 101 |
| M8 | `cdf` | 54 (51 passed, 3 failed) | 399 (396 passed, 3 failed) | 101 |

**Collateral reds match the pre-refactor gate.**

| Fault | Collateral reds |
|---|---|
| M1 | "A card dropped at an exact slot after a refresh keeps that slot", "A drop the server refuses after a refresh puts the card back", and swallow row 3 (`(Some(false), Some(false))`). |
| M3 | The other-tab scenario (`the first tab must swallow the drop, not leave it to the browser (D4)`, `left: Some(false)`). |
| M5 | Four landing or reload oracles: "The marker reaches both ends of a lane" (`:252`), the own-slot reload (`:265`), the marker-outlives "drops it" row (`:287`), and "The marker works on a board that refreshed in place" (`:309`). |
| M7 | Every other `us-cdf-04` scenario except "A lane emptied by a drag shows the placeholder", which asserts only an empty lane, as at the gate. |

**Survivors: none.** Every named scenario went RED on its oracle under its fault. The three gaps
closed on 2026-09-14 all hold on the final code:
- M2 reddens the swallow outline through the `dropEffect` step.
- M6 reddens the own-slot scenario through the AC-3.4 key oracle.
- M7 reddens the remote-empty scenario through its before-state Given.

**M6 seed note.** The refactor moved the dragged-card skip into `otherCards`, which `slotMidline`
also uses. Deleting the skip inside `otherCards` would also change `slotMidline`, a stronger mutant
than the recorded fault. The pre-refactor seed removed the skip from `slotFor` alone and left
`slotMidline`'s own skip intact. So the re-seed removes it from `slotFor` alone.

**Final state.**
- `cmp <scratchpad>/board-dnd.js.final crates/foundry-app/static/js/board-dnd.js`: **clean**
  (sha256 `bfd0f143…f78d`).
- `cmp <scratchpad>/foundry.f7c36a08.css.final crates/foundry-app/static/css/foundry.f7c36a08.css`:
  **clean** (sha256 `f7c36a08…a3eb`).
- `git status --short` after the last restore, before this section was written:
  `?? docs/feature/card-drag-drop-feedback/deliver/closing-notes.md` (the orchestrator's).
- `git status --short` after this section was written:
  ` M docs/feature/card-drag-drop-feedback/deliver/mutation/mutation-report.md` and
  `?? docs/feature/card-drag-drop-feedback/deliver/closing-notes.md`.
- Final `cdf` run after the last restore: **54 scenarios (54 passed), 400 steps (400 passed)**,
  `EXIT=0`.
- No lane is left running, and no git command touched the index or the working tree.

**Verdict: 9/9 killed on the final (refactored) code. The per-feature mutation gate still holds.**
