# DISCUSS Decisions — keyboard-shortcut-bindings

## Key Decisions

### Locked by the user (recorded verbatim in intent)

- [D1] **Scope = ALL SEVEN shortcuts** (`c`, `/`, `j`, `k`, `Enter`, `?`, `Esc`) — not just `c`. The help
  overlay advertises seven (`SHORTCUTS`, `keyboard.rs:48-56`); shipping one would leave six documented lies in
  place. The seven are also cheap once the guarded dispatch layer exists — the layer, not the individual keys,
  is the work.
- [D2] **Surface scoping**: `c`/`j`/`k`/`Enter` on the **board + issue list**; `?` and `Esc` **global** on any
  signed-in page. Matches the team+project scoping of the shipped routes (`keyboard.rs:62-95`).
  **⚠ Conflict found — see ODD-6/R8: there is no issue list.** Verified: `…/issues` is registered **POST-only**
  (`lib.rs:487-490`); no `/issues` GET, no backlog, no list page. The only other issue-key-bearing surface is
  the **search-results fragment** (`search_results.html:4`), which this DISCUSS reads as the intended "issue
  list". **DESIGN/PO to confirm.**
- [D3] **`?` opens the help as an OVERLAY**, not a page navigation — the server fragment already exists for
  exactly this (bare `role="dialog"`, `keyboard_help.html:1`). The existing sidebar/dashboard full-page links
  **stay** as the no-JS path (ODD-8, NFR-4).
- [D4] **Selection model**: `j`/`k` walk **visible cards in DOM order**; selected card gets a **ring
  highlight**; `Enter` opens it; selection **scrolls into view**; selection **resets on navigation**. **NOT
  roving tabindex** — and the accessibility trade-off is recorded as a first-class constraint + risk (NFR-7,
  ODD-7, R3), because ring-highlight selection is **not native focus** and screen-reader behaviour needs an
  explicit answer in DESIGN rather than silence.
  **⚠ Consequence — see ODD-1/R1: this retires the shipped `#kb-items` carrier and deletes a green test.**
- [D5] **JTBD: yes but lightweight** — one primary job (`fast-keyboard-issue-flow`), kept brief. This is
  **user-facing**, so the `infrastructure-only` escape valve is **not available**; every story carries a
  `job_id`.
- [D6] **UX research depth: LIGHTWEIGHT** — quick journey, happy-path focus. (Sad paths are still first-class
  where they are load-bearing — the guard cliff and the silent no-ops — but no extended discovery was run.)
- [D7] **Feature type: user-facing.**
- [D8] **NO walking skeleton.** Brownfield: the server contracts are shipped and routed, so the skeleton
  already exists. The gap is the **client layer only**. Slice 01 is therefore a thin real capability
  (`?` + `Esc`), not a skeleton. Recorded deliberately, not skipped by accident.

### Established during this DISCUSS pass

- [D9] **Browser-observable ACs only (NFR-1) — the anti-green-suite rule.** The shipped acceptance suite is
  **port-to-port** (reqwest + `scraper`) and asserts the *server contracts*; it **never presses a key**, which
  is why it is **green today while the feature is 100% absent**. Every AC in this feature asserts
  **key-pressed → observable outcome**, and **every new scenario must fail on unmodified `main`**. A scenario
  that passes on `main` tests the server contract, not this feature, and is rejected on sight.
- [D10] **Zero server delta (FR-11).** All three routes exist and are routed (`lib.rs:491-498`, `:536`). This
  feature adds **no route, no endpoint, no migration**. The only server-side changes are **removals**: the
  retired `#kb-items` builder (ODD-1) and the stale "alpine.js" doc comment (`keyboard.rs:1-30`, ODD-2, R9).
- [D11] **The guards ship BEFORE the first character key is bound** (slice 02 precedes slice 03), overriding
  the raw Value×Urgency/Effort order. A shortcut layer that eats keystrokes is **negative value**, not merely
  less value, and it is the **default outcome of the obvious implementation**. Riskiest-assumption-first governs.
- [D12] **Retire `#kb-items` whole, per the dead-code policy.** `AGENTS.md`: *"Remove dead/legacy code outright
  — do not leave it inert."* The carrier has **zero browser consumers** and always has (the handler it was built
  for was never written). Retirement deletes: `board.html:12`, the builder `projects.rs:881-891`, the
  view-model field `views.rs:256`, unit tests `projects.rs:1039-1110`, and **both acceptance assertions**
  (`us_12_keyboard_nav.rs:334-360`, `feature_b_web_tier.rs:568-572`). **This deliberately deletes passing
  tests** — flagged as ODD-1 for DESIGN confirmation, not executed unilaterally.
- [D13] **Personas are the repo's own harness identities** — Mei Tanaka (`mei@acme.com`) and Hiroshi Sato
  (`hiroshi@acme.com`), registered at `us_12_keyboard_nav.rs:58-64` — so every domain example is concrete and
  directly runnable rather than invented.
- [D14] **Repo legacy multi-file convention; no `docs/product/` SSOT; JTBD folded inline.** One job
  (`fast-keyboard-issue-flow`) with four forces + an ODI opportunity score lives in `requirements.md`, not a
  `jobs.yaml`. No `feature-delta.md`; no SSOT migration gate. Matches all prior features on trunk.

## Requirements Summary

- **Primary need**: make the seven shortcuts Foundry **already advertises in writing** actually work in the
  browser, guarded so they never steal a keystroke — so a keyboard-first maintainer can learn, file, find, move,
  open and escape without leaving the home row.
- **Walking skeleton**: **none** (D8) — brownfield; server contracts shipped, routed and green.
- **Feature type**: user-facing, client-only, brownfield. **One bounded context** (`foundry-app` web tier:
  `static/js/` + `templates/`), plus **removals** in `projects.rs` / `views.rs` / two acceptance step files.

## Constraints Established

- Bound set **==** `SHORTCUTS` (`keyboard.rs:48-56`) — the single source of truth (BR-1).
- Guards (text-input FR-2 + modifier FR-3) evaluated **before** dispatch, **no exemptions** (BR-2). `Shift` is
  **not** a suppressor — `?` is `Shift+/` (BR-7).
- Selection is **client-only, ephemeral, resets on navigation**, never persisted, never sent to the server (BR-5).
- `Esc` closes the **topmost** layer only, one per press; no-op when nothing is open; never navigates (BR-4).
- **Progressive enhancement** — no shortcut is the only path to any action; no-JS must not regress (BR-6, NFR-4).
- **Vendored only, no CDN**; external same-origin script, `defer`, no inline handlers; **document-level
  delegation** so htmx swaps need no re-wiring (NFR-5, NFR-6 — the `board-dnd.js:67` idiom).
- Ring highlight must meet **WCAG 2.1 AA**, not rely on colour alone, and selection changes **must** be conveyed
  to assistive tech by an explicit mechanism (NFR-7 — the named cost of rejecting roving tabindex).
- Drag-and-drop must keep working unchanged (NFR-8).
- **Every AC browser-observable; every scenario reds on `main`** (NFR-1).

## Scope Assessment: PASS — 7 stories, 1 bounded context, ~5 days across five thin slices

Right-sized; no split needed.

**Oversized→split analysis performed.** "Bind all seven shortcuts" could have read as seven features. It is
not: the **work is the guarded dispatch layer**, and the seven keys are thin consumers of it once it exists.
Splitting per-key would produce slices with no coherent user outcome (a slice whose value is "`k` works") and
would ship the dispatch layer five times. Conversely, deferring to "just `c`" (rejected per D1) would leave six
advertised-but-dead keys — the exact bug this feature exists to close.

Signals: stories **7** (≤10) | bounded contexts **1** (`foundry-app` web tier; ≤3) | walking-skeleton
integration points **N/A** (D8 — no skeleton; the server seams are shipped and green) | effort **~5 days**
(<2 weeks) | **one** coherent capability (a keyboard layer) sliced into five thin increments, each dogfoodable
in a single browser session | **zero** new routes, **zero** migrations, **zero** new components server-side. No
slice ships 4+ new components.

**Migration note**: **none.** This feature adds no migration (latest shipped remains `0014_notification_unsubscribes`).

## Elephant Carpaccio taste tests — results

| # | Taste test | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | Does every slice ship end-to-end in **≤1 day**? | **PASS** | 01 ~1d, 02 ~1d, 03 ~1d, 04 ~1d, 05 ~1d. Total ~5d. |
| 2 | Does every slice deliver **user-visible value** (demoable in a browser)? | **PASS** | 01 help in place; 02 *typing still works* (value = a harm avoided, and it is visible: Mei can type); 03 file without the mouse; 04 find without the mouse; 05 move + open. |
| 3 | Is any slice **`@infrastructure`-only**? (hard reject) | **PASS — none** | Every slice carries ≥1 user-visible story. Slice 02 is the closest call and still passes: its outcome is observable to the user (typing works, `Cmd+C` copies). |
| 4 | Does every slice carry a **named learning hypothesis** that could be disproven? | **PASS** | 01 overlay/mount/Alpine-coexistence; 02 one predicate vs all real inputs + IME; 03 htmx path reuse + CSRF-free; 04 slash suppression + fragment sufficiency; 05 selection coherence across swaps/drags + a11y without focus. |
| 5 | Could you **stop after any slice** and have shipped something honest? | **PASS** | 2/7 → 2/7(safe) → 4/7 → 5/7 → 7/7. Every stop point is coherent and nothing is left broken (see `prioritization.md` stop-check table). |
| 6 | Does any slice ship **4+ new components**? | **PASS — no** | 01 ships one script + one mount decision. 02–05 add behaviour to the existing script and reuse shipped routes/fragments. |
| 7 | Is the ordering **outcome/risk-driven**, not effort-driven? | **PASS** | Slice 02 (the guard) is promoted **ahead of higher-scoring slice 03** because binding `c` before the guard ships a worse-than-nothing state (D11). Slice 05 (highest value) is **last** by dependency + uncertainty, not effort. |
| 8 | Does every slice trace to an **outcome KPI**? | **PASS** | All five move KPI-1 (advertised-to-working ratio) and/or KPI-2/3/4. No orphan slices. |

## Handoff to DESIGN

DISCUSS deliberately leaves the genuine architecture choices open (requirements stay solution-neutral). The
solution-architect must resolve these Open Design Decisions (ODDs). **ODD-1, ODD-3, ODD-6, ODD-7 and ODD-9 are
blocking** for their slices.

- **ODD-1 — Retire `#kb-items`? (BLOCKING, slice 05).** The locked visible-DOM-order model (D-4) **contradicts**
  the shipped hidden carrier (`board.html:12`, built `projects.rs:881-891`, ASC-by-number across all columns,
  `hidden aria-hidden="true"`) which the suite calls *"the source of truth for the keyboard navigation order"*
  (`us_12_keyboard_nav.rs:339-341`). They cannot both hold: the **order differs** from the visible board
  (column-grouped, DESC-within-column, `projects.rs:864-885`), a **`hidden` element cannot take a ring or be
  scrolled to**, and `aria-hidden` hides it from assistive tech. Honouring D-4 retires it — per `AGENTS.md`
  that means **deleting it whole**, including **two currently-green acceptance assertions**
  (`us_12_keyboard_nav.rs:334-360`, `feature_b_web_tier.rs:568-572`), the builder, the view-model field
  (`views.rs:256`), and the unit tests (`projects.rs:1039-1110`). **Confirm before DELIVER removes them.**
- **ODD-2 — Vanilla vs Alpine.** `keyboard.rs:1-30` documents *"the alpine.js keyboard-shortcut handlers"*; the
  house pattern (`board-dnd.js:17`, `csrf-upload.js:19`) is **vanilla document-delegated IIFEs**; Alpine **is**
  vendored and loaded (`base.html:7`) but **unused by app code**. Pick one — and correct the stale doc comment
  in the same change (R9).
- **ODD-3 — The global mount point (BLOCKING, slice 01).** `?`/`Esc` are **global** (D-2) but `#modal-root`
  exists **only** at `board.html:13`; `app_shell.html` (7 lines) has **none**. Hoist the mount into the shell,
  or inject one on demand?
- **ODD-4 — The text-input guard predicate (BLOCKING, slice 02).** Exact element/role set (`input`, `textarea`,
  `contenteditable`, `select`, `role="textbox"`) **plus IME composition** (`isComposing`) — concrete, not
  theoretical, for a Japanese-input user (Mei). This is the highest-risk detail in the feature (NFR-2, R2).
- **ODD-5 — Selection survival across htmx swaps and drags.** By issue-key, by index, or reset? htmx swaps
  replace cards (`hx-target="#modal-root"`, and card re-renders) and `board-dnd.js:97-146` reorders the DOM
  under the selection. Also: how does `Esc`'s layer stack interact with a swap that replaces `#modal-root`?
- **ODD-6 — Surface scope + the missing "issue list" (BLOCKING, slice 04).** **The locked scope (D-2) names a
  surface that does not exist.** Verified: no `/issues` GET route, no backlog, no list page — `…/issues` is
  registered **POST-only** (`lib.rs:487-490`); `report.html:6` lists **change events**, not issues. The only
  other issue-key-bearing surface is the **search-results fragment**. This DISCUSS reads "issue list" as the
  search-results list and scopes accordingly — **confirm or correct**. Also: **where does the search box live?**
  The board renders none today.
- **ODD-7 — A11y of ring-highlight selection (BLOCKING for KPI-4, slice 05).** D-4 rejected roving tabindex, so
  the ring is **not native focus** and **nothing announces it for free**. `aria-activedescendant` on a
  container, a live region, or something else? NFR-7 requires an explicit answer, not inherited silence (R3). If
  the answer is "there isn't one", **D-4 itself must be revisited**.
- **ODD-8 — Fate of the full-page `/keyboard-help` links** (`sidebar.html:13`, `dashboard_root.html:32`) once
  `?` is an overlay. **Recommend: keep** — they are the no-JS path (NFR-4) and the route is public by design.
- **ODD-9 — The harness cannot press keys (BLOCKING, all slices).** Every AC in this feature needs a
  **browser-capable driver**; the shipped `InProcHarness` + reqwest + `scraper` (`us_12_keyboard_nav.rs:48-56`)
  is **port-to-port** and can only assert server contracts. **This is the root cause of the entire feature** —
  the instrument that would have caught the missing layer does not exist, which is exactly why nobody noticed.
  Per `AGENTS.md`, whatever driver is chosen belongs in **`cargo xtask ci`** (never `ci.yml` alone), and it must
  cope with the `@docker-compose`/`@needs-pgclient` lanes already in that gate.

**Handoff package**: `requirements.md` (context, the green-but-absent framing, scope + carve-outs, brownfield
grounding table with real `file:line`, the `#kb-items` collision, inline JTBD, FR/NFR/BR, alternatives, risk
table, glossary), `user-stories.md` (US-01..07 with `job_id` + Elevator Pitch + embedded AC),
`acceptance-criteria.md`, the journey trio (`journey-keyboard-nav-visual.md`, `.yaml`, `.feature`),
`shared-artifacts-registry.md`, `story-map.md`, `prioritization.md`, `outcome-kpis.md`, `dor-checklist.md`, and
the five slice briefs under `../slices/`.

## Upstream Changes

**None to a prior wave's assumptions** — but **two locked decisions collided with shipped reality** and are
surfaced rather than silently reinterpreted:

1. **D-4 (visible-DOM-order selection) vs the shipped `#kb-items` carrier.** The carrier is not merely
   redundant under D-4 — it is **contradicted** by it (different order; hidden so it cannot take a ring or be
   scrolled to). Honouring D-4 **deletes a currently-green acceptance assertion**. Surfaced as **ODD-1 / R1 /
   D12**, not executed unilaterally.
2. **D-2's "issue list" does not exist.** Verified: no `/issues` GET route, no list page; `…/issues` is
   POST-only (`lib.rs:487-490`). Read as the **search-results list** (the only other `data-issue-key`-bearing
   surface) and scoped accordingly. Surfaced as **ODD-6 / R8** for confirmation.

No DISCOVER or DIVERGE artifacts exist for this feature (no
`docs/feature/keyboard-shortcut-bindings/diverge/`); the job statement and personas were established directly in
this DISCUSS pass and folded into `requirements.md` (inline JTBD, no `docs/product/` SSOT — house convention).
The personas are the repo's own harness identities (D13). Every seam cited is shipped and verified by
`file:line`; the "no client keyboard layer exists" claim was verified by grep (**zero** application `keydown`
handlers — the only match is inside the vendored `alpine.min.js`; `static/js/` contains exactly `board-dnd.js`
and `csrf-upload.js`).

One honest note for DESIGN: **`keyboard.rs`'s own module doc describes handlers that do not exist** ("Three
routes that back the alpine.js keyboard-shortcut handlers"). Whatever ODD-2 decides, that doc is stale and must
be corrected in the same change (R9).

## Peer Review

- **Status**: COMPLETE (iteration 1 of max 2) — run via Task (`nw-product-owner-reviewer`, 2026-07-15).
- **Verdict**: **approved** — `critical_issues_count: 0`, `high_issues_count: 0`, `medium_issues_count: 1`
  (remediated in-iteration, see below). All four hard gates PASS: DoR 9/9 on all 7 stories; JTBD traceability
  (every story a `job_id: fast-keyboard-issue-flow`, none infrastructure-only); Dimension 0 Elevator Pitch PASS
  on all 7 — the reviewer accepted a **keypress** as a real user-invocable entry point, with concrete observable
  outputs (help overlay, focused modal, search results, ring on a card, issue modal); every slice carries a
  user-visible value story. Zero LeanUX anti-patterns. Happy-path bias: **PASS** — error handling drives the
  architecture rather than trailing it (guard before reward, D11).
- **Challenged and upheld**:
  - **Slice 02 (the guards) is not an infrastructure slice in disguise.** The judgement I most wanted contested
    was upheld: value expressed as *a harm avoided* is still user-visible — *"its outcome is observable to the
    user (typing works, `Cmd+C` copies)"*.
  - **ODD-1** (retiring `#kb-items`, deleting two green assertions) — *"justified, three-part blocked,
    DESIGN-owned"*; correctly surfaced as a decision, **not executed unilaterally**.
  - **ODD-6** (the locked scope names an "issue list" that does not exist) — *"verified against code
    (`lib.rs:487-490`), blocked, awaiting DESIGN confirmation"*; correctly surfaced, **not silently
    reinterpreted**.
- **[D15] Medium finding — REAL, and remediated in-iteration.** The reviewer found a genuine hole in
  AC-02.1/02.2: on `main` *"no shortcut fires (correct result) because no handler exists (wrong reason). Test
  passes for absence, not presence of guard."* The deeper danger it exposes: such an AC would **also pass on a
  build that binds without guarding** — precisely the regression NFR-2 exists to catch, sailing through the
  gate. **Fix applied**: US-02's ACs are reframed as **revert-reds-it regression guards, not reds-on-`main`
  ACs** (the one deliberate inversion of D9's governing rule), and restated as **paired assertions** — each
  scenario must first prove the shortcut *does* fire outside a text field before asserting it does *not* fire
  inside one, so it cannot pass vacuously. `acceptance-criteria.md` documents the inversion explicitly; the
  `@property` scenario is tagged `@paired-assertion` warning DISTILL not to split the halves. No second
  iteration needed (0 critical / 0 high).
- **DoR gate**: **PASSED** (9/9 on all 7 stories). **Handoff to DESIGN (solution-architect): CLEARED.**
