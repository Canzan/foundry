# Definition of Ready — keyboard-shortcut-bindings

9-item hard gate. Each item must PASS with evidence before DESIGN handoff.

## US-01 — Press `?` and see the shortcut list, right where I am

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "The help overlay is the product's own advertisement for the keyboard layer, and it is only reachable by a full-page navigation — the most flow-breaking action, to learn how to preserve flow. Meanwhile `?`, which the help page itself lists as 'Show this help', does nothing." |
| 2 | User/persona with specific characteristics | PASS | Mei Tanaka (`mei@acme.com`), keyboard-first maintainer on any signed-in page; the repo's own harness identity (`us_12_keyboard_nav.rs:58-64`). |
| 3 | 3+ domain examples with real data | PASS | `?` on the AUTH board → overlay over the board, board still visible, no navigation; `?` on the dashboard (no `#modal-root`) → overlay still appears; overlay lists exactly the 7 `SHORTCUTS` entries. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (overlay-in-place, lists-all-seven, works-away-from-board, Esc-restores). |
| 5 | AC derived from UAT | PASS | AC-01.1..01.6, traced to FR-1/4/5 + BR-1/3/4 + NFR-1/4. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | Dispatch layer + one overlay + Esc; 4 scenarios; **~1 day**. Reuses a shipped public route + bare fragment. |
| 7 | Technical notes: constraints/dependencies | PASS | Reuses `show_keyboard_help` (`keyboard.rs:259`, route `lib.rs:536`) + `keyboard_help.html:1`; house idiom NFR-5; delegation NFR-6; ODD-2 (vanilla vs Alpine), ODD-3 (global mount), ODD-8 (link fate). |
| 8 | Dependencies resolved or tracked | PASS | No new route. ODD-2/3/8/9 flagged as DESIGN inputs; ODD-3 blocking for this slice. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (advertised-to-working 0/7 → 2/7), baseline 0/7. |

**US-01 DoR: PASSED**

## US-02 — Type the letter "c" into a title without filing a new issue

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Every one of the seven shortcuts is a plain printable character or bare key — exactly what people type. Without a guard evaluated before dispatch, binding them makes text entry impossible and hijacks `Cmd+C`." |
| 2 | User/persona with specific characteristics | PASS | Every user of every shortcut (Mei, Hiroshi) typing titles/descriptions/queries/comments; Mei uses a Japanese IME, making `isComposing` concrete. |
| 3 | 3+ domain examples with real data | PASS | Types `"cache invalidation on login"` → exactly that; types `"cjk/?"` → exactly `"cjk/?"`; `Cmd+C` on `AUTH-2` → copies, no modal; leaves field, presses `c` → modal opens. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (title-typing, all-chars `@property`, copy-chord, guard-releases). |
| 5 | AC derived from UAT | PASS | AC-02.1..02.7, traced to FR-2/3 + BR-2/7 + NFR-2/3. |
| 6 | Right-sized | PASS | A guard predicate + a litmus; 4 scenarios; **~1 day** (proving it across every real input surface + IME is the work). |
| 7 | Technical notes: constraints/dependencies | PASS | Guard-before-dispatch is structural (BR-2), not per-shortcut; ODD-4 pins the predicate (element/role set, `contenteditable`, `isComposing`). |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 (the layer it guards). ODD-4 blocking. Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-2 (**0** captured keystrokes, **0** hijacked chords — hard guardrail). |

**US-02 DoR: PASSED**

## US-03 — Press `c` and file an issue without touching the mouse

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "`c` is the most-used shortcut in every issue tracker and the first entry in Foundry's own help list. It is unbound. The route, fragment, CSRF cookie and autofocused title all exist — only the keypress that reaches them is missing." |
| 2 | User/persona with specific characteristics | PASS | Mei on a project board with a bug in mind right now; wants it filed in one keystroke. |
| 3 | 3+ domain examples with real data | PASS | `c` on AUTH → modal, title focused → types `"Session cookie not cleared on sign-out"` → card appears; `c` on the dashboard (no project) → nothing; `c` while the title is focused → letter typed, no second modal; no-JS button → full-page form. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (opens-modal, files-end-to-end, no-project-no-op, no-JS-intact). |
| 5 | AC derived from UAT | PASS | AC-03.1..03.6, traced to FR-6 + BR-2/3/6 + NFR-1/4. |
| 6 | Right-sized | PASS | One key → the shipped htmx path; 4 scenarios; **~1 day**. No new route, no client CSRF work. |
| 7 | Technical notes: constraints/dependencies | PASS | Reuses `show_new_issue_modal` (`keyboard.rs:62`, route `lib.rs:491-494`) + the identical `hx-get` the button uses (`board.html:6`) + `#modal-root` (`:13`); CSRF minted server-side (`:94`); no-JS fork untouched (`:96-104`). |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 + **US-02 (hard — must not bind `c` before the guard)**. ODD-5/6 tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (2/7 → 4/7), KPI-3 (**0** mouse actions to file). |

**US-03 DoR: PASSED**

## US-04 — Press `/` and search the board without reaching for the mouse

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "`/` is unbound, so the shipped search endpoint — exact-key matching, substring matching, a real empty state — is unreachable by the key that is supposed to reach it. Worse, the naive binding focuses the field *and* types '/' into it." |
| 2 | User/persona with specific characteristics | PASS | Mei on a project board, half-remembers an issue's title or key, wants it without a pointer. |
| 3 | 3+ domain examples with real data | PASS | `/` → focused **and empty** → `session` → `AUTH-2 Session cookie not cleared on sign-out`; `AUTH-2` → exactly that issue; `zzz` → the shipped empty state; `and/or` typed into the box → literal. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (focus-no-slash, substring, exact-key, empty-state). |
| 5 | AC derived from UAT | PASS | AC-04.1..04.7, traced to FR-2/5/7 + NFR-1. |
| 6 | Right-sized | PASS | One key → focus + suppress default; results are the shipped fragment; 4 scenarios; **~1 day**. |
| 7 | Technical notes: constraints/dependencies | PASS | Reuses `search_issues` (`keyboard.rs:160`) + `filter_matches` (`:208-231`) + `search_results.html:4`, route `lib.rs:495-498`. The board renders **no search box today** — its placement is ODD-6. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 + US-02. **ODD-6 blocking** (where the search box lives). Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (4/7 → 5/7), KPI-3 (**0** mouse actions to find). |

**US-04 DoR: PASSED**

## US-05 — Walk the board with `j` and `k` and see where I am

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "There is no selection model in the client. `j`/`k` are advertised and unbound, and `Enter` ('Open selected') is meaningless without a 'selected'. The shipped `#kb-items` carrier is hidden and ordered differently from the visible board, so a ring and `scrollIntoView` are meaningless on it." |
| 2 | User/persona with specific characteristics | PASS | Mei on a board with several cards; needs to *see* which is current; selection must follow her eyes. Hiroshi (mouse user) is why drag must not break. |
| 3 | 3+ domain examples with real data | PASS | AUTH-3/AUTH-2/AUTH-1 → `j` rings AUTH-3, `j` again AUTH-2, `k` back; 30 cards → scrolls into view; `k` at first → stays; empty board → no-op; Hiroshi drags the selected card → no stale ring. |
| 4 | UAT in Given/When/Then (3-7) | PASS | **6** scenarios (first-select, walk-visible-order, scroll-into-view, bounded, drag-coexists, a11y-announce) — within the 3-7 band. |
| 5 | AC derived from UAT | PASS | AC-05.1..05.9, traced to FR-8/10 + BR-5 + NFR-1/6/7/8 + ODD-1. |
| 6 | Right-sized | PASS | Selection model + ring + scroll + the carrier retirement; 6 scenarios; **~1 day**. Highest uncertainty in the feature but bounded to one surface. |
| 7 | Technical notes: constraints/dependencies | PASS | Walks visible `article.issue-card` (`issue_card.html:1`), **not** `#kb-items` (`board.html:12`); document-delegated per `board-dnd.js:67`; ODD-1 (carrier retirement incl. deleting 2 green assertions), ODD-5 (swap survival), ODD-7 (a11y mechanism). |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 + US-02; slice 04 for the second surface. **ODD-1 + ODD-7 blocking**. Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (5/7 → 7/7), KPI-3, KPI-4 (100% of moves announced; WCAG 2.1 AA, not colour alone). |

**US-05 DoR: PASSED**

## US-06 — Press `Enter` to open the issue I have selected

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "`Enter` closes the loop `j`/`k` opens. Without it, selection is a decoration: the user can move a highlight but not act on it, and must reverse into the mouse at the final step — the most annoying possible place to lose flow." |
| 2 | User/persona with specific characteristics | PASS | Mei with a card selected, wants to open it now; motivated by completing the keyboard loop. |
| 3 | 3+ domain examples with real data | PASS | `j` `j` → AUTH-2 → `Enter` → AUTH-2's modal; `Enter` with no selection → nothing; `Enter` in the title field → form submits; `Enter` then `Esc` → AUTH-2 still selected. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (opens-selected, no-selection-no-op, form-submits, selection-survives-close). |
| 5 | AC derived from UAT | PASS | AC-06.1..06.6, traced to FR-2/5/9 + BR-2/5 + NFR-1. |
| 6 | Right-sized | PASS | One key → activate the selected card's existing `hx-get`; 4 scenarios; **~0.5 day** (ships with US-05 in slice 05). |
| 7 | Technical notes: constraints/dependencies | PASS | Reuses the card's own `hx-get={edit_url}` → `#modal-root` (`issue_card.html:1`) — identical to a pointer click, one open path. The `Enter`-in-a-form case falls out of the US-02 guard rather than a special case. |
| 8 | Dependencies resolved or tracked | PASS | Depends on **US-05 (hard — no selection, nothing to open)** + US-02. Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (contributes to 7/7), KPI-3 (**0** mouse actions to complete the loop). |

**US-06 DoR: PASSED**

## US-07 — Press `Esc` to get out of anything, and land somewhere sane

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Every other shortcut opens something, so without `Esc` each is a one-way door. `Esc` is also the key users press hardest when they feel stuck, so doing nothing is the worst possible response." |
| 2 | User/persona with specific characteristics | PASS | Mei with a modal/overlay/search open; motivated by reversibility — the confidence to use the other six at all. |
| 3 | 3+ domain examples with real data | PASS | Modal open → `Esc` → back on the board; help over modal → `Esc` closes help only, modal still open; nothing open → nothing; AUTH-2 selected+opened → `Esc` → still selected; search → `Esc` → board restored. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 5 scenarios (closes-modal, one-layer-at-a-time, no-op, selection-survives, search). |
| 5 | AC derived from UAT | PASS | AC-07.1..07.6, traced to FR-1/5 + BR-3/4/5 + NFR-1. |
| 6 | Right-sized | PASS | Split across slices 01 (overlay) and 03 (modal) — `Esc` closes what each slice opens; 5 scenarios; **~0.5 day** total. |
| 7 | Technical notes: constraints/dependencies | PASS | Global scope shares the mount question with US-01 (ODD-3); layer precedence (BR-4) implies tracking what is open; interaction with htmx swaps replacing `#modal-root` is ODD-5. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 (dispatch); pairs with US-03/04/06 (the layers it closes). Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (contributes to 7/7 — completes the advertised set). |

**US-07 DoR: PASSED**

---

## Overall DoR: PASSED

All seven stories pass all 9 items. Every story carries `job_id: fast-keyboard-issue-flow`; none is
`infrastructure-only` (not available — this is a user-facing feature). Every story has an `### Elevator Pitch`
with Before/After/Decision-enabled.

**Dimension 0 (Elevator Pitch) self-check**: all 7 "After" lines are anchored to a **real user-invocable entry
point** — a **keypress** (`?`, `c`, `/`, `j`/`k`, `Enter`, `Esc`), which is as user-invocable as an entry point
gets — plus a **concrete observable output** (the help overlay over the board, the new-issue modal with the
title focused, the search results listing AUTH-2, a ring on a visible card, AUTH-2's issue modal, the modal
closing with selection intact). No internal-only entry points, no "tests pass", no internal state.

**Slice-level check**: every slice contains at least one user-visible value story — 01 (US-01, help in place),
02 (US-02, *typing still works* — value expressed as a harm avoided, which is user-visible: Mei can type),
03 (US-03, file without the mouse), 04 (US-04, find without the mouse), 05 (US-05+US-06, move and open). **No
slice is `@infrastructure`-only.**

### The open decisions below are DESIGN-wave inputs, not DoR blockers

Requirements are written solution-neutrally; each decision is tracked in `wave-decisions.md`:

- **ODD-1** — **Retire `#kb-items`?** The locked visible-DOM-order model (D-4) contradicts the shipped hidden
  ASC carrier (`board.html:12`) that the suite calls "the source of truth for the keyboard navigation order".
  Honouring D-4 **deletes two currently-green assertions** (`us_12_keyboard_nav.rs:334-360`,
  `feature_b_web_tier.rs:568-572`) plus the carrier, builder, view-model field and unit tests.
- **ODD-2** — **Vanilla vs Alpine.** `keyboard.rs:1-30` documents "the alpine.js keyboard-shortcut handlers";
  the house pattern (`board-dnd.js`, `csrf-upload.js`) is vanilla document-delegated IIFEs; Alpine is vendored
  and loaded but unused by app code. Whichever is picked, the stale doc must be corrected in the same change.
- **ODD-3** — **The global mount point.** `?`/`Esc` are global but `#modal-root` exists **only** at
  `board.html:13`; `app_shell.html` has none. Hoist it into the shell, or inject on demand?
- **ODD-4** — **The text-input guard predicate.** Exact element/role set (`input`/`textarea`/`contenteditable`/
  `select`/`role="textbox"`) plus **IME composition** (`isComposing`) — concrete for a Japanese-input user.
- **ODD-5** — **Selection survival across htmx swaps and drags.** By issue-key, by index, or reset? Cards are
  replaced by swaps and reordered by `board-dnd.js`.
- **ODD-6** — **Surface scope + the missing "issue list".** **Verified: no issue-list page exists** — `…/issues`
  is registered **POST-only** (`lib.rs:487-490`). The locked scope says "board + issue list"; the only other
  issue-key-bearing surface is the search-results fragment. Also: **where does the search box live?** The board
  renders none today.
- **ODD-7** — **A11y of ring-highlight selection.** Not native focus (D-4 rejected roving tabindex), so nothing
  announces it for free. `aria-activedescendant`, a live region, or something else? **DESIGN must answer
  explicitly rather than inherit silence** (NFR-7, R3).
- **ODD-8** — **Fate of the full-page `/keyboard-help` links** (`sidebar.html:13`, `dashboard_root.html:32`)
  once `?` is an overlay. Recommend: **keep**, as the no-JS path (NFR-4).
- **ODD-9** — **The harness cannot press keys.** Every AC here needs a **browser-capable driver**; the shipped
  `InProcHarness` + reqwest + `scraper` (`us_12_keyboard_nav.rs:48-56`) is port-to-port. **This is the root
  cause of the whole feature**: the instrument that would have caught the missing layer does not exist. Per
  `AGENTS.md`, whatever driver is chosen belongs in `cargo xtask ci`, never in `ci.yml` alone.

### Peer review (nw-product-owner-reviewer) — gate

**Result (2026-07-15, iteration 1/2): approved** — `critical_issues_count: 0`, `high_issues_count: 0`,
`medium_issues_count: 1`. All hard gates PASS: DoR 9/9 on all 7 stories; JTBD traceability (every story a
`job_id: fast-keyboard-issue-flow`, none infrastructure-only); Dimension 0 Elevator Pitch PASS on all 7 (the
reviewer accepted a **keypress** as a real user-invocable entry point, with concrete observable outputs); every
slice carries a user-visible value story — **including slice 02**, which the reviewer specifically scrutinised
and upheld: *"its outcome is observable to the user (typing works, `Cmd+C` copies)"*, value legitimately
expressed as a harm avoided. Zero LeanUX anti-patterns. ODD-1 and ODD-6 confirmed as **correctly surfaced as
blocking decisions with code verification, not executed unilaterally**. Happy-path bias: **PASS** — error
handling drives the architecture (guard before reward).

**Medium finding — REMEDIATED in this iteration.** The reviewer caught a real hole in AC-02.1/02.2: *"On `main`,
no shortcut fires (correct result) because no handler exists (wrong reason). Test passes for absence, not
presence of guard."* Worse, such an AC would **also pass on a build that binds without guarding** — the exact
regression NFR-2 exists to catch. **Fix applied**: US-02's ACs are now explicitly reframed as
**revert-reds-it regression guards, not reds-on-`main` ACs**, and are stated as **paired assertions** — each
scenario must first prove the shortcut *does* fire outside a text field (precondition: the layer is live) before
asserting it does *not* fire inside one. `acceptance-criteria.md` now documents this as the one deliberate
inversion of the governing rule; the `@property` scenario in `journey-keyboard-nav.feature` is tagged
`@paired-assertion` with a note that DISTILL must not split the halves. No second review iteration needed
(0 critical / 0 high).

**Handoff to DESIGN: CLEARED.**
