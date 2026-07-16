# DESIGN Decisions — keyboard-shortcut-bindings (the missing client layer)

> Morgan (nw-solution-architect), DESIGN wave, Propose mode. Resolves **all nine** DISCUSS ODDs into
> contracts + eight feature-local ADRs. Paradigm inherited (Rust ports-and-adapters modular monolith on the
> server; vanilla document-delegated IIFE JS in the web tier) — **not re-decided**. Deliverables:
> `architecture.md`, `adr-001..008`, this file, `upstream-changes.md`. **Nothing is deferred to DISTILL.**
> ODD-7's residual was escalated and is now **ratified and closed** (Option A, user, 2026-07-15 — D-4
> stands; the Tab cost is accepted; KPI-4 is conditional on it). Two AC re-shaping notes (`Cmd+C`
> clipboard, IME `send_keys`) remain **open for DISTILL**.

## Key Decisions

- **[DD1] Vanilla, not Alpine — and Alpine is dropped.** One IIFE, one `document`-delegated `keydown`, the
  `board-dnd.js:67` idiom. Alpine is vendored and loaded (`base.html:7`) with **zero** app consumers, so per
  AGENTS.md it is carry, not insurance: the tag and the vendored asset go. The stale "alpine.js handlers"
  doc (`keyboard.rs:1-30`) is corrected in the same change (R9). (ADR-001)
- **[DD2] The guard chain is structural, not per-shortcut.** Four steps, in order, before the dispatch table
  is reachable at all: composition (`isComposing || keyCode===229`) → modifier (`ctrl||meta||alt`, **Shift
  excluded** — BR-7) → `defaultPrevented` → text-entry. `isTextEntry` uses `isContentEditable` (catches
  inherited editability) and an allow-list of **non**-text `INPUT` types (fails **closed** on unknown/future
  types). (ADR-002 — **the crux**)
- **[DD3] `?` gets its own JS-created host; `#modal-root` is untouched — and BR-4 forced it.** Help rendered
  into `#modal-root` (`hx-swap="innerHTML"`) would **destroy an open modal**, but US-07 requires help to
  close *over* a still-open modal. So `keyboard.js` appends `#kb-overlay-root` to `<body>` on any page. The
  Esc layer stack is **derived from the DOM**, never stored — a stored array is exactly what an htmx swap
  desyncs. **ODD-8 confirmed: the full-page help links stay** (the no-JS path). (ADR-003)
- **[DD4] Selection is a KEY.** `selectedKey: string|null`, never an index (a drag silently re-points it at
  a different issue — disqualifying) and never a node ref (an htmx re-render detaches it). The ring is
  **derived** and re-projected on `htmx:afterSwap`. **Drag coexistence (NFR-8), "resets on navigation"
  (BR-5), and "Esc never clears selection" (AC-07.3) all cost zero code** — they fall out of the
  representation. `board-dnd.js` is unchanged. (ADR-004)
- **[DD5] Board only; search is a panel ON the board; navigation is modal, selection identity is shared.**
  `/` reveals + focuses a JS-injected panel (plus a pointer-clickable control, so `/` is an accelerator, not
  the only path) and `preventDefault()`s its own slash. `j`/`k` walk the panel's rows when open, else board
  cards — never merged. **`Enter` always resolves `selectedKey` → the board card → that card's shipped
  `hx-get`**, because search results carry **no `hx-get`** (a DISCUSS miss — `upstream-changes.md` §4).
  **Amends D2** (§3). (ADR-005)
- **[DD6] `aria-activedescendant` on a focusable ARIA composite — because a live region answers the wrong
  question.** In screen-reader browse mode `j`/`k` are quick-nav keys **intercepted before any listener
  runs**: the keys never arrive, so there is nothing to announce. Keys reach the page only in focus mode,
  which needs DOM focus on a composite. JS applies `role=listbox`/`option` + `tabindex=0` +
  `aria-activedescendant` (cards **already** carry `id="issue-KEY"`), re-applied on swap. **Honours D-4** —
  one tab stop on one container is not roving tabindex. **Ratified by the user (Option A, 2026-07-15);
  D-4 stands.** The one-time Tab-to-the-board is an **accepted, documented cost**, and KPI-4 is met
  **conditionally on it** — a qualifier that must travel with every KPI-4 claim. (ADR-006)
- **[DD7] A real-browser lane via fantoccini, and it must NEVER silently skip.** `BrowserHarness =
  InProcHarness (unchanged) + a fantoccini session → base_url()` — **the harness already serves a real TCP
  origin** (`upstream-changes.md` §1), so no serving plumbing is added. `@needs-browser` is excluded from
  the fast loop but **included in `all`**, which is what `cargo xtask ci` runs; a missing/skewed
  chromedriver is a **hard failure with an install hint**, mirroring `pg_dump_at_least_16()`. Probe, then
  refuse. **Reverses the repo's recorded "no-Playwright" decision** (§2). (ADR-007)
- **[DD8] Retire `#kb-items` whole — confirmed, with a corrected 13-site map.** Two Gherkin feature files
  DISCUSS missed, two unit tests that are **not** deletable whole, and a **vacuity trap** at
  `projects.rs:1110` whose naive removal leaves a test passing for the wrong reason — the very pattern this
  feature exists to end. `issue_key_string` **stays**. (ADR-008)
- **[DD9] Zero server delta, stated precisely.** Zero routes/endpoints/migrations/handler changes — honoured
  strictly, and DD3/DD5 chose their mechanisms specifically to keep it. The unavoidable additive edits (one
  `<script>` tag; the stylesheet + its **inherited** hand-maintained re-hash; test infra) are enumerated in
  `upstream-changes.md` §6, which proposes a checkable restatement of AC-X.4.

## ODD → ADR resolution map

| ODD | Question | Resolution | ADR |
|-----|----------|------------|-----|
| ODD-1 | Retire `#kb-items`? (blocking, slice 05) | **Yes — delete whole.** 13 verified sites, 2 feature files, 2 traps. Confirmed, map corrected | ADR-008 |
| ODD-2 | Vanilla vs Alpine | **Vanilla** IIFE + document delegation; **Alpine dropped** (zero consumers); doc corrected (R9) | ADR-001 |
| ODD-3 | The global mount point (blocking, slice 01) | **Neither hoist nor reuse `#modal-root`** — a JS-created `#kb-overlay-root`; forced by BR-4's layering. Esc stack DOM-derived | ADR-003 |
| ODD-4 | The guard predicate (blocking, slice 02) | The exact 4-step chain + `isTextEntry`; `isComposing`+`keyCode 229`; Shift not a suppressor | ADR-002 |
| ODD-5 | Selection survival across swaps/drags | **By issue-key**, ring derived, re-projected on `htmx:afterSwap`; Esc stack DOM-derived | ADR-004 (+ ADR-003) |
| ODD-6 | Surface scope + the search box (blocking, slice 04) | **Board only**; JS-injected panel + pointer control; modal navigation, shared key; `Enter` via the board card | ADR-005 |
| ODD-7 | A11y of ring selection (blocking KPI-4, slice 05) | `aria-activedescendant` on a focusable composite; **not** a live region (browse-mode interception). **RATIFIED (Option A, user, 2026-07-15); D-4 stands.** Tab cost accepted; KPI-4 conditional | ADR-006 |
| ODD-8 | Fate of the full-page help links | **Keep** — confirmed. The no-JS path (NFR-4); dead-code policy does not reach live consumers | ADR-003 |
| ODD-9 | The harness cannot press keys (blocking, all) | **fantoccini + chromedriver**, `@needs-browser` in `cargo xtask ci`; reuse `InProcHarness`; probe-then-refuse | ADR-007 |

## RESOLVED — ODD-7 / D-4 / KPI-4 (ratified by the user, 2026-07-15)

**Escalated as a genuine choice; now closed. Option A accepted. D-4 stands, upheld on review.** No open
residual remains on ODD-7.

**The finding that decided it** (retained because it is the load-bearing argument, not a footnote): in
screen-reader browse mode, `j`/`k` are quick-navigation keys consumed by the AT **before any page listener
runs**. **A live region cannot fix this** — it would announce a selection that can never change, producing
a feature that reviews as accessible and is inert in use. Keys arrive only in focus mode, which requires
DOM focus on a composite widget. That admits exactly two mechanisms: `aria-activedescendant` (chosen) or
roving tabindex (rejected by D-4).

| Option | Cost | Effect on KPI-4 | Verdict |
|---|---|---|---|
| **A. Accept** — ship ADR-006's `aria-activedescendant` on a focusable composite; document "Tab to the board, then `j`/`k`" in the help copy | One Tab press for AT users; D-4 intact | Met **conditionally** | **ACCEPTED (user, 2026-07-15)** |
| **B. Reopen D-4** — adopt roving tabindex | The board focus-model rewrite D-4 priced as too large; must be re-proven against `board-dnd.js` (NFR-8) | Met **unconditionally** | **REJECTED** — trade-off retained in ADR-006 *Alternatives Considered* for whoever revisits it |

**The accepted cost, stated plainly**: an AT user must **Tab to the board once** before `j`/`k` arrive.
Inside the composite everything works (keys dispatch, `aria-activedescendant` announces, `aria-selected`
exposes state, `Enter` opens). Sighted keyboard users are unaffected. This is an **accepted, documented
cost — not an open residual.**

**KPI-4 is met CONDITIONALLY on that Tab, and the condition must be stated wherever KPI-4 is claimed —
never buried.** A bare "KPI-4 met" is a misstatement of what shipped; the qualifier is **"once the board is
focused"**. Two obligations follow, both on **slice 05**:
- Every slice report / KPI roll-up / a11y claim citing KPI-4 carries the qualifier.
- The help overlay's own copy documents *"Tab to the board, then `j`/`k`"* — the instruction must reach the
  user, not just live in an ADR.

If the Tab cost is ever judged unacceptable, D-4 must be reopened; ADR-006 already makes that case.

## Constraints for DISTILL / DELIVER (what acceptance must pin)

- **Typing is never captured (NFR-2, KPI-2 — the highest-risk invariant).** The `@property`
  `@paired-assertion` must **not** be split: *first* prove the layer is live (press `c` outside a field →
  modal opens), *then* prove the guard (type `"cjk/?"` into the field → exactly those characters, nothing
  fires). **`[data-kb-ready]` (ADR-001) is the concrete hook** for the "layer is live" precondition. This is
  **revert-reds-it**, not reds-on-`main` — preserve D15's deliberate inversion; do **not** "fix" it.
- **Two ACs must be re-shaped before they can be written (ADR-007, `upstream-changes.md` §4):**
  1. **AC-02.3 (`Cmd+C`)** — clipboard reads are not viable headless. Assert **non-activation** (no modal)
     + `defaultPrevented === false` for **both** `Ctrl` and `Meta`. Do **not** assert "the text was copied"
     (and note Linux CI's copy chord is `Ctrl`, not `Meta`).
  2. **The IME clause (ODD-4)** — WebDriver `send_keys` cannot produce composition. Drive it with a
     JS-dispatched `CompositionEvent` + `KeyboardEvent{isComposing:true}` via `client.execute()`. It
     exercises our predicate truthfully but is **not** a real IME — an honest, named limit.
- **Bound set == advertised set (BR-1, KPI-5)** — a `@property` enumerating the overlay's `dt[data-shortcut]`
  values and asserting each is bound. Both sides derive from `SHORTCUTS`.
- **Layered `Esc` on the board (AC-07.1)** — `c`, then `?`, then `Esc`: help gone **AND `#modal-root` still
  populated**. This is the scenario that reds if anyone collapses the two hosts back into one (ADR-003).
- **Selection is a key, provable two ways (ADR-004)** — (a) drag the selected card to another column: the
  ring is still on **that key**, not on whatever occupies the old slot; (b) the `@htmx-swap` property (file
  via `c`, then `j`/`Enter`, no reload). Together they red on any switch to an index.
- **One open path (AC-06.5)** — `/` → `AUTH-2` → `j` → `Enter` produces **the same modal a pointer click on
  AUTH-2's card produces**. Plus the classic-bug guard: after `/`, the input is focused **and empty**.
- **`Enter` edge (newly surfaced, ADR-005)** — an issue in a state the board does not render
  (`{backlog,todo,in_progress,done}`, `projects.rs:49,933-941`) is findable via search but has **no card** →
  `Enter` is a **no-op**. Pin it.
- **`#kb-items` is gone (AC-05.6)** — a **grep litmus**: `kb-items`/`kb_items` returns **zero** hits under
  `crates/`. Plus **trap B**: `projects.rs:1110`'s `visible` must be repointed at the full HTML, or the test
  passes vacuously (ADR-008).
- **No-JS intact (NFR-4, BR-6)** — every existing no-JS scenario passes unchanged; the sidebar help link and
  the "New issue" full-page form still work. **Named limit**: search stays JS-only (the route has no
  full-page fork) — nothing regresses, and a fork is a recommended out-of-scope follow-up.
- **Drag parity (NFR-8)** — every existing drag scenario passes unchanged; `board-dnd.js` is untouched.
- **A11y (NFR-7, KPI-4)** — ring ≥3:1 non-text contrast, never colour alone; `aria-selected` exposed;
  `aria-activedescendant` tracks; an automated a11y check gates slice 05.
- **The lane refuses, never skips (ADR-007)** — a missing/skewed chromedriver **fails** `cargo xtask ci`
  with an install hint. **A skipped browser lane is indistinguishable from the bug it exists to prevent.**

## Per-slice architecture notes

- **Slice 01 (`?` + `Esc`, the dispatch layer)** — ADR-001 (vanilla IIFE, `defer`, `[data-kb-ready]`,
  **drop Alpine**), ADR-003 (`#kb-overlay-root`, the DOM-derived stack, ODD-8 keep). **ADR-007 lands FIRST
  or nothing here is assertable** — the lane is the precondition for every slice. Stylesheet + re-hash
  begin here (overlay rules). KPI-1 0/7 → 2/7.
- **Slice 02 (the guards)** — ADR-002 whole. No new key is bound; capability is deliberately zero. The
  `@property` `@paired-assertion` + the two re-shaped ACs above. KPI-2.
- **Slice 03 (`c` + Esc-closes-modal)** — reuses `board.html:6`'s own `hx-get` (never reconstructed); zero
  client CSRF work (`keyboard.rs:94` + `new_issue_modal.html:4` — **if DELIVER writes CSRF code here,
  something is wrong**). Layered-Esc proves ADR-003. KPI-1 → 4/7.
- **Slice 04 (`/` search)** — ADR-005 panel + pointer control + slash suppression; shipped fragment
  semantics honoured as-is (exact-key, substring, `data-empty`). Creates the second selectable surface.
  KPI-1 → 5/7.
- **Slice 05 (`j`/`k`/`Enter`)** — ADR-004 (key-based selection), ADR-005 (modality + `Enter` resolution),
  ADR-006 (ARIA composite + the escalated residual), ADR-008 (the 13-site retirement + both traps). Retire
  the `@manual` drill (ADR-007 §5). KPI-1 → **7/7 — the promise fully kept**.

## Handoff to DISTILL (acceptance-designer)

- **Architecture + contracts**: `architecture.md` (C4 L1+L2+L3, the resolved guard/host/selection/search/
  a11y/harness contracts, reuse-vs-new, enforcement rules), eight ADRs with ≥2 real alternatives each,
  `upstream-changes.md` (six deltas incl. one D2 amendment + one reversal).
- **External integrations**: **none.** The layer talks only to Foundry's own same-origin routes — **no
  contract-test annotation is owed to platform-architect.** The one new external dependency is a
  **build/test-time substrate** (chromedriver/Chrome), whose contract is enforced by the ADR-007 probe, which
  is the right instrument for a driver binary.
- **Paradigm for software-crafter**: **Rust** (server — untouched here beyond removals + a doc fix) and
  **vanilla ES5-compatible JS** (client — the feature). No framework, no build step, no bundler, no `eval`.
  The crafter owns all internal structure (function decomposition, the panel's markup, the exact CSS, wait
  helpers) during GREEN/REFACTOR; the contracts above are the boundary.
- **The three invariants the suite must guard as revert-reds-it litmuses**: (1) **typing is never captured**
  (the paired `@property` — a regression here is worse than shipping nothing); (2) **bound == advertised**
  (both derive from `SHORTCUTS`, so the original bug cannot recur); (3) **the browser lane runs** — it is in
  `all`, it probes, and it refuses rather than skips.
- **Owed to platform-architect (DEVOPS)**: chromedriver + Chrome in the CI image, version-matched, with the
  `xtask` preflight as the contract. Nothing else — production is untouched, zero migrations, no new metric
  series.

## Peer Review

- **Status**: NOT RUN — `nw-solution-architect-reviewer` was not invoked in this pass.
- **Consequence**: the DESIGN → DISTILL gate is **not** formally cleared by a reviewer. The artifacts are
  complete and every ODD is resolved, but the house workflow's peer-review step (max 2 iterations, address
  critical/high) has not been executed for this feature. **Recommend running it before DISTILL begins**, with
  particular attention to ADR-006 (the escalated a11y residual) and ADR-007 (the reversal of a recorded repo
  decision + a new host prerequisite) — the two decisions with the widest blast radius.
- **Handoff to DISTILL (acceptance-designer): READY, pending the peer review above.** ODD-7 is **ratified
  and closed** (Option A, 2026-07-15). The two AC re-shaping notes (`Cmd+C` clipboard, IME `send_keys`)
  remain **open for DISTILL** — they were not part of that ratification.
