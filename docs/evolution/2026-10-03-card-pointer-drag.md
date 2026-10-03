# card-pointer-drag — evolution archive

Move the board's card drag from native HTML5 drag-and-drop onto **Pointer Events**, so a
card can be dragged by touch and pen as well as by mouse. It is a **cross-cutting,
brownfield feature in three slices**:
1. The mouse drag runs on Pointer Events with nothing visibly changed (parity).
2. On a phone, a card drags at once by its **grip**, or lifts after a **500 ms hold** on
   its text, with an arming cue.
3. A carried card held at an edge scrolls the board or the page. Every interruption
   leaves nothing behind.

Waves: DISCUSS → DESIGN → DISTILL → DELIVER, with a real-device spike between DESIGN and
slice 02. No DISCOVER, DIVERGE or DEVOPS wave ran. ADR-BOARD-LANE-007 and the
card-drag-drop-feedback (CDF) archive were the evidence base. The wave was amended once,
on 2026-09-29 (`d7dae7b`, `c60d716`), after the device run failed the whole-card hold.

The feature landed **commit per step**, on `main`:
- Slice 01 (2026-09-26) shipped in release **v0.5.0**.
- Slice 02 (2026-09-29) is on `origin/main`.
- Slice 03 plus two post-close commits (2026-10-02 local, 2026-10-03 UTC) are local at
  finalize. The orchestrator pushes them.

Final `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`: **GREEN, 880/880 scenarios
(6187/6187 steps)** at the 03-02 close (`2850ec2`), and again on the final tip `fc7ce42`
(after the harness strengthening and the refactor) on 2026-10-03, also 880/880 with the
browser lane run (*Numbers*).

## Business context

Since CDF, Priya Raman's desk drag was trustworthy: the lane lights, a marker shows the
slot, and drops survive in-place refreshes. On her phone at 390px she could drag a
**lane** header with her thumb (board-lane-reorder), but not a **card**. Her only touch
path was to open the card, change Status and Save. That cannot choose a slot and always
lands the card at the top. ADR-BOARD-LANE-007's *Consequences* had predicted she would read
this as a bug, and named this feature as its first deferred successor.

The user asked for **full convergence**: one card-drag mechanism for every pointer, not
a touch path beside a surviving HTML5 mouse path. Two constraints came with it:
- **everything CDF shipped stays**: the lit lane, the marker, landing at the marker,
  the placeholder, replace-proofing, the identity revert and the byte-identical POST;
- **the shipped Gherkin stays byte-identical** (D4). Only the test *driver* changes.

Job `job-board-card-move`, **widened** to touch and pen rather than replaced; persona
Priya Raman. North star (CPD-KPI-1): a card moved to an exact lane and slot by touch,
from a baseline of 0% (no card gesture existed on touch).

## What shipped

- **`board-dnd.js`, event layer rewritten (426 → 850 lines; the refactor took it from
  825 to 850).**
  - Delegated `document` listeners: `pointerdown/move/up/cancel`, a non-passive
    `touchstart` (grip only) and `touchmove` (aborts a hold; guards after the lift), a
    passive capture `scroll` (aborts a pending hold), `contextmenu`, and a capture-phase
    `click` guard.
  - A **`Press`** follows one pointer from `pointerdown`: travel, the hold timer, and from
    the lift its **`CardDragSession`**.
  - Lift rule: mouse 6 px; touch or pen on the grip 3 px with no timer; touch or pen on
    the body a 500 ms hold within 10 px, shown by `data-card-arming`.
  - Lane and slot resolve **from the point** (`elementFromPoint`).
  - A fixed clone **ghost** follows the pointer while the origin card stays dimmed in its
    slot.
  - An **edge scroller** works on the board sideways and on the page vertically
    (`EDGE_ZONE` 48, `EDGE_STEP` 14). It runs once per frame from the last carried point
    and re-resolves the marker after each step.
  - Own-card `dragstart` is prevented, and the HTML5 foreign swallow is kept verbatim.
  - `CardDragSession`, `Origin`, `slotFor`, activation, the marker, `dropInto` and
    `moveBody` are **kept**.
- **`keyboard.js`**: arm 3a in `closeTopLayer()`. It finds `html[data-card-dragging]` and
  dispatches `foundry:cancel-card-drag`, directly above the lane-drag arm.
- **Card markup, both sources:** `partials/issue_card.html` and
  `issues.rs::render_issue_card` each gain `<span data-card-grip aria-hidden="true"></span>`
  as the card's last child. A unit test, `board_template_and_server_rendered_card_are_identical`,
  keeps the two identical.
- **Stylesheet, tokens only:**
  - the grip: 48 px, `touch-action: none`, dot glyph in `--cz-muted`;
  - the card: `position: relative`, 56 px right padding, `min-height: 48px`,
    `user-select: none` and `-webkit-touch-callout: none`;
  - the ghost, the lifted dim, and the arming cue (scale 0.96 and opacity 0.7 over
    `var(--card-hold-ms)`, dim only under reduced motion).

  Re-hashed `f7c36a08` → `438142d2` (01-02) → `7fa13f60` (02-01) → `6b3e4436` (02-03).
  The step from `438142d2` to `f9143163` belongs to release-version-footer (`0cd73c3`), not
  to this feature.
- **`xtask/src/check_arch.rs`, DDD-22:** no `board-*.js` may register a `keydown`
  listener. Comments are stripped first, and the rule ships with gold tests that include a
  commented-out case.
- **Unchanged server.** No handler, service, store or migration change. The move request
  is byte-identical (D1, DDD-10).
- **Tests.** The new `card-pointer-drag.feature` (`@cpd`) holds 35 declarations, which run
  as 37 examples. The **shipped card-drag drivers** were re-pointed to trusted input: the
  CDF kit, `feature_board_lane_reorder.rs` `drag_a_card` and the
  `keyboard_shortcut_bindings.rs` AUTH-2 drag. Mouse, pen and keys use W3C Actions, touch
  uses CDP `Input.dispatchTouchEvent`, and foreign drags stay synthetic `DragEvent`.

## Key decisions

| Decision | What it settled |
|---|---|
| **D2 / DDD-16**: [ADR-BOARD-CARD-004](../product/architecture/adr-board-card-004-pointer-events-card-drag.md), Option A | One hand-authored Pointer Events session for every pointer. SortableJS (B, and C, which retires the lane module too) was evaluated and rejected. Its live DOM sort and per-list binding contradict the marker and replace-proof contracts, so the rejection rests on the contracts, not on the no-dependency posture. It supersedes ADR-BOARD-LANE-007's "for now, permanent" divergence and boundary leg 1 |
| **D4 / DDD-12 / DDD-12a**: the Gherkin is frozen; the driver moves | The empty `.feature` diff is the parity proof. Trusted W3C Actions are used for mouse, pen and keys. Touch uses **CDP**, because chromedriver 151's W3C touch cannot span `perform_actions` calls and its `pointerCancel` is a no-op (DISTILL U1, measured). CDF-KPI-8 was re-scoped to `.feature` files (C4) |
| **DDD-3**: resolve from the point | Touch captures `event.target` to the origin card, so `closest(LANE)` stops working. `elementFromPoint`, with the ghost at `pointer-events: none`, replaces it. Brief invariant 4 now reads "from the point" |
| **DDD-7 / DDD-22**: Escape is a `closeTopLayer()` arm | It is never a `keydown` in a drag module (BR-4). This is now a structural check-arch rule, and a DoD item |
| **DDD-19**: keep `draggable="true"`; prevent own-card `dragstart` | The device run showed iOS starting a native drag ~655 ms into a still press. Removing `draggable` would not help, because iOS then drags the text, and it would falsify `issue-status-move.feature:49` |
| **D5 amended / DDD-23..29**: grip and body hold (user, 2026-09-29) | No hold duration separates a resting-thumb swipe from a hold: at 350 ms, 14 of 30 swipes lifted on an iPhone. The grip lifts at once and its `touchstart` is prevented (DDD-25). The body keeps a 500 ms hold with an arming cue (DDD-27), and a click on the grip opens nothing (DDD-28). **Accepted trade-off:** a thumb resting ≥ 500 ms on the text before it swipes lifts the card |
| **DDD-6**: one-shot click guard, reset on the next press | No drag ever opens the edit dialog, and a stale guard never eats a genuine click. Chrome sends no click after a lifted mouse release, which makes the reset load-bearing |
| **DDD-11 / DDD-18**: vertical page auto-scroll in scope; conventions copied, not shared | Lanes never scroll internally, so a long lane means page scroll. The lane module's 6 px and 48/14 are copied (separate ways by choice) |
| **DDD-20**: htmx stays at 2.0.4 | `htmx-4-migration` is recorded as a successor. htmx has no drag-and-drop in any version |

## Steps completed

From `deliver/execution-log.json` (UTC), 41 events. Steps 01-01 to 02-03 are logged in
the legacy 5-phase form (PREPARE, RED_ACCEPTANCE, RED_UNIT `SKIPPED`/`NOT_APPLICABLE` (no
JS unit runner, DB6), GREEN, COMMIT). Steps 03-01 and 03-02 use the ADR-025 3-phase form
(RED, GREEN, COMMIT); see lesson 2. `des-verify-integrity` exits 0 over all 9 steps.

| Step | Name | First → last event | Commit |
|---|---|---|---|
| 01-01 | check-arch: no `board-*.js` registers a `keydown` listener (DDD-22) | 09-26 16:53 → 17:09 | `e92e6dd` |
| 01-02 | Atomic swap: mouse lift, carry and land on Pointer Events; `dragstart` cancelled; Escape arm; shipped drivers re-pointed in the same change | 17:18 → 19:00 | `6b711a2` |
| 01-03 | One mouse gesture means one thing: click-guard reset, primary button, card/lane boundary | 19:02 → 19:19 | `d56f245` |
| 01-04 | Foreign swallow after a pointer session; `pointercancel` revert; slice 01 close | 19:21 → 19:44 | `a9958d2` |
| 02-01 | A grip on every card, in both sources; `min-height: 48px`; re-hash | 09-29 15:00 → 16:06 | `1bcf72d` |
| 02-02 | The grip drags at once on touch and pen; a tap or click on it opens nothing | 16:07 → 16:24 | `45f8e7a` |
| 02-03 | The body hold (500 ms within 10 px), arming cue, aborts, reduced motion; slice 02 close | 16:26 → 17:08 | `666e085` |
| 03-01 | Edge auto-scroll, board and page, with marker = landing = `after` | 10-03 02:49 → 03:03 | `7e51169` |
| 03-02 | Interruptions leave nothing behind; feature close. **No production change** | 03:08 → 03:52 | `2850ec2` |

After the close: `7045cd3` (harness strengthening, test-only) and `fc7ce42` (L1-L3
refactor). Docs commits: `c683a1a` (DISCUSS and DESIGN), `524737b` (DISTILL), `cfc9d20`
(roadmap and log), `d7dae7b` (device run), `c60d716` (the amendment).

## The mutation story

The strategy is per-feature, with a gate of ≥80%. There is no JS mutation tool (DB6 bars
Node), so the faults are named and hand-seeded, one at a time, each restored and
`cmp`-verified (the CDF precedent).

**The counting rule:** each distinct named fault counts once, at its latest result.
**34/39 = 87.2%, PASS.** The unclamped-scroller mutant is equivalent (the browser clamps
`scrollLeft` itself) and is excluded. Counting it as a survivor gives 85.0%. A raw per-run
tally, which counts the re-run faults twice, gives 35/44 = 79.5%. That tally is recorded
in the slice-03 notes and is not the gate measure.

| Source | Faults | Killed |
|---|---|---|
| Slice 01 | 7 | 6 |
| Slice 02 (02-01 6, 02-02 7, 02-03 13) | 26 | 23 |
| 03-01 (clamp excluded) | 4 | 3 |
| 03-02 (new faults) | 2 | 2 |

**The five survivors, all documented and none argued away:**
1. **Click guard never armed (slice 01).** Chrome sends no click after a lifted mouse
   release, so there is nothing for the guard to eat. It can only be seen in Firefox, and
   that check is owed.
2. **Hold timer ignores `isConnected` (DISTILL U8).** No user gesture can replace the board
   while a finger is held still. It was seeded and verified by reading. The effect is
   harmless: nothing is sent or moved.
3. **and 4. The move aborts and the `scroll` abort, each on its own.** The aborts are
   redundant by design (DDD-26): WebKit stops sending `pointermove` once it pans. Each path
   alone is proven only on the device.
5. **The board's end spills into a sideways page scroll.** At 390px the page has no
   horizontal extent, so the fault cannot be seen.

**At the first 03-01 gate the kill rate was 1/5.** The crafter did not write tests to
lift the score. It routed the two test-strength survivors to `nw-acceptance-designer`.
`7045cd3` then changed the steps only, and both faults went red on named oracles, never a
timeout:
- "marker not recomputed after a scroll step" is killed by #21, #22 and #27;
- "scroller not stopped in `end()`" is killed by #26.

The kill rate was measured **before** `fc7ce42`. That refactor was verified
behaviour-preserving by every lane and by the review.

## Refactor and review

- **Phase 3 refactor (`fc7ce42`; L1-L3; L4-L6 found nothing worth doing).**
  - `board-dnd.js`: a `Press` type with `travelled()` and `lift()` replaces an object
    literal, three `travel(...)` call shapes and two duplicated session-creation sites. A
    `lifted()` predicate replaces three inline checks. `ghostOf()` is extracted from
    `lift()`. Every `CardDragSession` field is now set in the constructor instead of
    mid-`lift()`.
  - Steps: 11 duplicated no-move assertion blocks become one `assert_no_move_sent`
    helper, with the same comparison and the same messages.
  - DDD-18 rules out sharing code with `board-lane-dnd.js`.
  - Verified once at the end: `cpd` 37/37 (315 steps), cdf 54/54, blr 26/26, kb 38/38,
    check-arch and smoke green, and no `.feature` changed.
- **Phase 4 adversarial review** (`nw-software-crafter-reviewer`): **APPROVED, no
  findings.**
- **Roadmap review:** APPROVED on 2026-09-26, with 0 blockers and 0 highs. It was revised
  on 2026-09-29 for the amendment.

## Lessons and issues

### 1. A test harness that "holds still" by jittering could not have caught a missing marker recompute.

The 03-01 harness held a carried card "still" at an edge with `PointerStep::Jitter`, a
trickle of 1 px moves. Every one of those was a `pointermove`, which re-ran `track` and so
recomputed the lane and the marker itself. A scroller that never re-resolves the marker
after its own scroll step therefore passed every scenario: a seeded fault that removed the
recompute survived. Production was correct, because the step does call `track`, but the
suite could not tell.

A real finger held still sends **no** `pointermove`, so the faithful driver sends nothing
while the scroller runs. `7045cd3` made three changes:
- it holds with `hold_still`;
- it releases at the held point with no glide;
- it waits 300 ms after every exit before asserting that nothing scrolled on.

The rule now: **a driver must not generate events the real input would not.** "Robustness"
padding in a driver can stand in for exactly the production behaviour under test.

### 2. The DES rigor config and the installed logger disagreed on phase names.

`.nwave/des-config.json` listed the legacy five TDD phases. The installed `des-log-phase`
accepts only the ADR-025 canon: RED, GREEN, COMMIT. Steps 03-01 and 03-02 could not be
logged until `tdd_phases` was moved to the canon on 2026-10-03, with a dated note in the
config. `des-verify-integrity` maps the earlier PREPARE, RED_ACCEPTANCE and RED_UNIT
entries onto RED, so all 9 steps still verify. The log is therefore **mixed-form**: 01-01
to 02-03 have 5 phases, and 03-01 and 03-02 have 3. Check the logger against the config
before the first step of a wave, not mid-wave.

### 3. The device beat the design: no hold duration separates a resting swipe from a hold.

DESIGN locked a whole-card 350 ms hold for touch and made the device checklist gate
slice 02 (DDD-13). On an iPhone (iOS 26.7, Safari and Brave), step 5 passed: the post-lift
`touchmove` guard holds on WebKit. Step 1 failed. 14 of 30 swipes lifted, with thumbs
resting 407-1085 ms before they moved, and first-move times spread evenly from 25 to
1085 ms.

Emulation and the iOS Simulator start moving at once, so they cannot reproduce this (D19).
The grip and body hold were proven on the same phone (`probe-v5.html`: 13/13 grip drags;
`probe-v6.html`: 5/5 body holds at ~503 ms) **before** DISTILL re-authored slice 02. The
spike paid for itself: the failure was found in `probe-*.html` files, not in shipped
code.

### 4. iOS lies by delay, and removing `draggable` would not have helped.

With `draggable="true"`, iOS held a grip touch in its own drag interaction for 571-1505 ms
before any pointer event reached the page. With `draggable` off, it dragged the card's
**text** instead. Only a prevented `touchstart` on the grip (DDD-25) stops it, at the cost
of a second non-passive `document` touch listener.

### 5. The driver's W3C touch could not carry a gesture.

chromedriver 151's W3C touch source drops later moves and the release across
`perform_actions` calls, and its `pointerCancel` dispatches nothing. Touch is driven
through CDP `Input.dispatchTouchEvent` instead. That is the same trusted dispatch
chromedriver uses internally. A second finger is a second touch point, and `touchCancel`
is "the system takes the pointer". Every gesture first arms a capture-phase recorder
(the memory note "automation drags can deliver zero events").

### 6. A step with no production change still has to prove its scenarios discriminate.

The scenarios of 03-02 (#24-#26) were GREEN on the existing code, because the single
revert path, the hold aborts and the scroller stop were already shipped. That is only
credible if a fault turns them red, so five faults were seeded. Four went red on named
oracles, including slice 01's mouse-`pointercancel` survivor, now killed through the
shared path by #24. The fifth, "scroller not stopped", was then closed by lesson 1's
harness change.

### 7. Host faults, again, not results.

- The `sqlx-macros` dylib "mis-aligned LINKEDIT" under `strip = "symbols"` (DISTILL U6)
  failed the first 03-01 smoke at clippy. `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false`
  is the workaround (memory note).
- Testcontainers `PortNotExposed` and `Connection reset by peer` each failed one 02-03
  smoke run. `foundry-store` then ran 76/76 alone.
- A default-lane run was cut at the delivery agent's own 580 s bound (exit 124, no failure
  seen), and a rerun under 2400 s went 654/654.

A timeout is never a count.

## Numbers

| | |
|---|---|
| Acceptance (`cpd`) | 35 declarations / 37 examples; final 37/37 scenarios, 315/315 steps, 0 `@pending` tags. Per story: us-cpd-01 12/12, us-cpd-02 18/18 (16 declarations), us-cpd-03 7/7 |
| Re-driven shipped suites | cdf 54/54 (400 steps), blr 26/26, kb 38/38, us-cts-03 5/5 |
| Default lane | 654/654 scenarios, 4522/4522 steps (03-02) |
| Full CI | `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`: exit 0, all tags **880/880 scenarios, 6187/6187 steps**, browser lane ran, at the 03-02 close (`2850ec2`) and again on the final tip `fc7ce42` |
| After the close | `7045cd3` (test-only): `cpd` 37/37 and us-cpd-03 7/7 twice. `fc7ce42` (refactor): `cpd` 37/37, cdf, blr and kb green, check-arch and smoke green |
| Shipped `.feature` files | byte-identical. The only diff is the new `card-pointer-drag.feature` (D4 / KPI 2) |
| Named faults | 34/39 = **87.2%** (equivalent clamp excluded; 85.0% if counted) |
| DELIVER steps | 9, all COMMIT/PASS; 41 DES events; integrity exit 0 |
| Server change | **0**: no handler, service, store or migration change |
| Browser | Chrome 151.0.7922.108 (`selenium/standalone-chrome:latest`, image `cd778b6f38d9`) |

## Owed to the user

- **Slice 03 real-device checklist**, on iOS Safari **and** Android Chrome, plus the
  narrow desktop window (`slice-03-delivery-notes.md`). It includes a far-lane drop on an
  8-lane board at 390px (CPD-KPI-6) and a release while held at an edge.
- **Slice 02 tallies and versions.** The user reported every step PASSED on 2026-10-02.
  The OS and browser versions, the 10x/5x tallies and the **lazy-swipe misfire tally**
  (AC-2.14, CPD-KPI-3) are still blank.
- **Desktop mouse feel in Chrome and Firefox** (slice 01 checklist), including the
  **Firefox click guard**: the only place survivor 1 can be seen. It also includes a real
  Finder `keys.png` dropped straight after a drag.
- **U-2** (does the 0.7 arming dim read in both palettes?) and **U-3** (is a one-line
  card's grip easy to hit?) are unticked in the slice-02 notes.
- **CPD-KPI-7**: a 5-working-day log of touch moves, from when slice 02 is on the
  instance.

## Artifacts

- `docs/feature/card-pointer-drag/feature-delta.md`: the four-wave record, including the
  DELIVER sections.
- `docs/feature/card-pointer-drag/deliver/`: `slice-01..03-delivery-notes.md` (gates,
  named faults, survivors, device checklists), `roadmap.json` and `execution-log.json`.
- `docs/feature/card-pointer-drag/spike/`: `findings.md`, `probe*.html` and `ios-sim/`,
  the device evidence for the amendment.
- `docs/product/architecture/adr-board-card-004-pointer-events-card-drag.md` (with a
  dated implementation note); `adr-board-lane-007-pointer-events-lane-drag.md` (a dated
  supersession note).
- `docs/product/architecture/brief.md` § "Domain Model" → "The board card drag session
  (browser tier)", including its shipped pointer-build inventory (replaced at this
  finalize).
- `docs/product/journeys/journey-card-drag-drop.yaml` (the `touch` variant);
  `docs/product/jobs.yaml` § `job-board-card-move` (widened).
- `docs/product/outcomes/registry.yaml`: OUT-13 and OUT-14 amended; OUT-16 registered.
- `docs/product/kpi-contracts.yaml`: CDF-KPI-8 re-scoped; `card-pointer-drag` CPD-KPI-1..7,
  with measured values recorded at finalize.
- `crates/foundry-acceptance/tests/features/card-pointer-drag.feature`

No `design/`, `distill/` or `discuss/` sub-directories exist, so finalize Phase B had
nothing to migrate.

## Successors

- **`htmx-4-migration`** (DDD-20), once 4.x is npm `latest`.
- **A keyboard card move** (D17); **haptic lift feedback** (DDD-15).
- **`.lane-drop-indicator` contrast**, 1.49:1 → ≥3:1, carried from CDF.
- **Pin `selenium/standalone-chrome`.** It still floats at `:latest`.
- **Touch listeners only on board pages** (DDD-25 fallback (c)). Do this only if a device
  shows scroll-start latency on non-board pages.
- **A grip click that also closes an open lane menu**: U-4, accepted as-is.
- Carried from CDF: the new-issue ordering bug (`position DEFAULT 0`), CDF OQ-3 and OQ-4,
  and the testcontainers connect flakes.
