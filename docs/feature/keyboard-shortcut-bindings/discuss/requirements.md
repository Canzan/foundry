# Requirements — keyboard-shortcut-bindings (the missing client layer)

## Context

Foundry's help overlay advertises **seven keyboard shortcuts** — `c` Create issue, `/` Search, `j` Next,
`k` Previous, `Enter` Open selected, `?` Show this help, `Esc` Close modal. The list is a shipped constant
(`SHORTCUTS`, `crates/foundry-app/src/keyboard.rs:48-56`) rendered into a real `<dl>` the user can read.

**None of the seven are bound in the browser.** The entire client-side keyboard layer was never written. A
grep for `keydown` / `keyup` / `x-on:key` across `crates/foundry-app/templates/` and
`crates/foundry-app/static/js/` returns **zero application hits** — the only match anywhere is inside the
vendored `static/vendor/alpine.min.js` itself. `static/js/` contains exactly two files, `board-dnd.js` and
`csrf-upload.js`. There is no `keyboard.js`.

The **server side is complete and routed**. `keyboard.rs` module doc opens: *"Three routes that back the
alpine.js keyboard-shortcut handlers"* — handlers that do not exist. Every contract those handlers would need
is shipped, tested, and green:

- `GET …/issues/new` returns a bare htmx modal fragment (`HX-Request: true`) or a full-page no-JS fallback.
- `GET …/search?q=` returns `ul.search-results` with `li.search-result[data-issue-key]` items.
- `GET /keyboard-help` returns `section.keyboard-help[role="dialog"]` with a `dt[data-shortcut]`/`dd` per shortcut.
- `board.html:12` emits a hidden `<ul id="kb-items">` carrier *"so the j/k handler walks the right sequence"*.

So the user reads a help page promising seven shortcuts, presses `c`, and **nothing happens**. The feature is
100% absent from the user's point of view and 100% present from the test suite's point of view.

> **The honest framing — why the suite is green while the feature is missing.** The acceptance suite is
> port-to-port: it drives HTTP and parses HTML with `scraper`. It asserts the *server contracts* the client
> would call (`us_12_keyboard_nav.rs`, 6 scenarios). It **never presses a key**, because nothing in the harness
> can. `GET /keyboard-help` returning a well-formed `<dl>` proves nothing about whether `?` opens it. This
> feature therefore has a hard rule: **every acceptance criterion must assert browser-observable behaviour
> (key pressed → thing happens), not endpoint responses.** An AC that a port-to-port test could satisfy today
> is, by construction, not an AC for this feature.

## Scope

- **In scope**: binding **all seven** shortcuts (`c`, `/`, `j`, `k`, `Enter`, `?`, `Esc`) in the browser.
  - `c`, `/`, `j`, `k`, `Enter` — active on the **board** and the **search-results list** (the only two
    issue-key-bearing surfaces that exist; see ODD-6).
  - `?` and `Esc` — **global**, on any signed-in page.
  - A **text-input guard** (shortcuts inert while typing) and a **modifier guard** (`Cmd`/`Ctrl`/`Alt` held ⇒
    no fire) — the two correctness preconditions for every other shortcut.
  - A **selection model**: `j`/`k` walk visible cards in DOM order, the selected card takes a ring highlight,
    scrolls into view, `Enter` opens it, selection resets on navigation.
  - `?` renders the shipped `/keyboard-help` fragment as an **overlay** (not a page navigation).
- **Out of scope** (deliberate carve-outs):
  - **New server routes or endpoints** — all three exist and are routed (`lib.rs:492-497`, `:536`). This
    feature adds **zero** routes and **zero** migrations.
  - **User-remappable / configurable keybindings** — the `SHORTCUTS` constant stays the single source of truth.
  - **New shortcuts beyond the advertised seven** — the help list is the contract; we honour it, we don't grow it.
  - **A roving-tabindex / full native-focus rewrite of the board** — explicitly rejected (D-4); the a11y
    consequence is carried as a named constraint + risk, answered in DESIGN (ODD-7).
  - **An issue-list page** — none exists (verified); building one is a different feature (ODD-6).

## Brownfield grounding (shipped seams — reuse, do not reinvent)

| Seam | Location | Reuse / Role |
|------|----------|--------------|
| `SHORTCUTS` — the 7-entry single source of truth (`c`,`/`,`j`,`k`,`Enter`,`?`,`Esc`) | `crates/foundry-app/src/keyboard.rs:48-56` | **The contract this feature honours.** An acceptance test asserts the list is complete; the client must bind exactly this set. |
| `show_keyboard_help` — **public** `GET /keyboard-help`; returns `section.keyboard-help[role="dialog"][aria-label="Keyboard shortcuts"]` + `dt[data-shortcut]`/`dd` pairs | `keyboard.rs:259-279`; route `lib.rs:536`; partial `templates/partials/keyboard_help.html:1` | The `?` overlay body. **Public by design** — doc says the bootstrap GETs it once on page load and caches it, so a session requirement would break help on the sign-in page. |
| `show_new_issue_modal` — `GET …/issues/new`; bare htmx modal fragment when `HX-Request: true`, else full-page no-JS fallback; mints CSRF via `ensure_csrf_cookie`; form action `…/issues` | `keyboard.rs:62-110`; route `lib.rs:491-494`; partials `new_issue_modal.html` | The `c` target. The **fragment/full-page fork is the no-JS guarantee** (NFR-5) — `c` uses the htmx path, the sidebar/no-JS path keeps the full page. |
| `search_issues` — `GET …/search?q=`; `ul.search-results` + `li.search-result[data-issue-key]`; empty state `ul.search-results[data-empty="true"]` | `keyboard.rs:160-202`, `filter_matches` `:208-231`; route `lib.rs:495-498`; partial `search_results.html:4` | The `/` target + **the only other issue-key-bearing list** (ODD-6). Empty query returns every issue; `data-empty` distinguishes "no query" from "no match". |
| Hidden `#kb-items` carrier — `<ul id="kb-items" hidden aria-hidden="true">` with one `li[data-issue-key]` per issue, **ASC by issue number**, spanning all columns | `templates/board.html:12`; built `projects.rs:881-891`; view-model `views.rs:256` | **CONFLICTS with the locked selection model (D-4).** Built for the never-written handler. See "The `#kb-items` collision" below — this is ODD-1 and Risk R1. |
| `issue_card.html` — `article.issue-card[id=issue-KEY][data-issue-key]`, `draggable`, `data-state-url`, `hx-get={edit_url}` → `#modal-root` | `templates/partials/issue_card.html:1` | **The visible card `j`/`k` walk** and `Enter` opens (its `hx-get` is exactly what `Enter` must trigger). Already pointer-clickable (`cursor:pointer`). |
| Board columns — `section.column[data-column]`, cards rendered **most-recent-first (DESC)**, grouped by state | `board.html:9`; `projects.rs:864-879` | Visible DOM order = column-grouped, DESC-within-column. **Not** the `#kb-items` ASC order (Risk R1). |
| `#modal-root` — the htmx swap target for every modal | `board.html:13` **only**; **absent from `app_shell.html`** | `c`/`Esc` work on the board because the mount exists there. A **global** `?` overlay has **no mount point** on non-board pages — ODD-3. |
| `board-dnd.js` — vanilla IIFE, `"use strict"`, external same-origin (CSP-safe, no inline handlers), **`document`-delegated** `dragstart` so htmx-appended cards work without re-wiring; per-column listeners bound at `init()` | `static/js/board-dnd.js:17,60-146,149-154` | **The house JS pattern to copy** (document-level delegation ⇒ survives htmx swaps) **and** the drag interaction `j`/`k` selection must not fight (Risk R4). |
| `csrf-upload.js` — same IIFE/`DOMContentLoaded`/delegation idiom | `static/js/csrf-upload.js:19,94-108` | Confirms the house pattern is **vanilla JS, not Alpine** — despite `keyboard.rs`'s "alpine.js" doc. ODD-2. |
| `base.html` — loads vendored `htmx.min.js`, `alpine.min.js`, `board-dnd.js`, `csrf-upload.js`, all `defer` | `templates/base.html:6-9` | Where a `keyboard.js` would register. **Alpine is vendored and loaded but unused by app code.** Vendored-only, no CDN. |
| `app_shell.html` — `{% extends base %}` + sidebar + `app_content` block | `templates/app_shell.html` (7 lines) | The wrapper every signed-in page shares — the natural home for a global `?`/`Esc` layer and a shell-level `#modal-root` (ODD-3). |
| `/keyboard-help` full-page links | `templates/partials/sidebar.html:13`; `templates/dashboard_root.html:32` | The **only** way to reach help today. Their fate once `?` is an overlay is ODD-8 (recommend: keep as the no-JS path). |
| Acceptance steps pinning the server contracts (6 scenarios) incl. the ASC-order assertion on `#kb-items` | `crates/foundry-acceptance/src/steps/us_12_keyboard_nav.rs:12-14,334-360`; also `feature_b_web_tier.rs:568-572` | The **green-but-absent** suite. The ASC assertion is what ODD-1 must consciously retire or honour. |
| Harness identities `Mei` (`mei@acme.com`) / `Hiroshi` (`hiroshi@acme.com`) | `us_12_keyboard_nav.rs:58-64` | **The personas below are the repo's own test identities** — reused so examples stay concrete and runnable. |

### The `#kb-items` collision (the one genuinely hard thing here)

The shipped board emits a hidden carrier that the shipped acceptance suite asserts is *"the source of truth for
the keyboard navigation order"* (`us_12_keyboard_nav.rs:339-341`), sorted **ascending by issue number**, spanning
**all columns**, and explicitly `hidden aria-hidden="true"` (`board.html:12`). The comment at
`projects.rs:881-885` states the intent: *"the visible board renders most-recent-first (DESC); the alpine.js j/k
handler walks this hidden list … so pressing `j` moves 'to the next-older issue' consistently no matter which
column the user is in."*

The **locked selection model (D-4)** says the opposite: `j`/`k` walk **visible cards in DOM order**, the selected
card takes a **ring highlight**, and selection **scrolls into view**.

These cannot both hold:

1. **Order differs.** `#kb-items` is ASC-by-number across all columns. Visible DOM order is column-grouped,
   DESC-within-column. Walking one is observably not walking the other.
2. **A hidden element cannot be highlighted or scrolled to.** `#kb-items` is `hidden`; a ring on it renders
   nothing and `scrollIntoView` on it does nothing. The locked model *requires* the visible `.issue-card`.
3. **`aria-hidden="true"` makes it invisible to assistive tech**, so it cannot carry the a11y story either.

Honouring D-4 therefore **retires `#kb-items`**: the carrier becomes dead markup and, per the repo's dead-code
policy (`AGENTS.md` — *"Remove dead/legacy code outright — do not leave it inert"*), it must be **deleted**
along with its two acceptance assertions (`us_12_keyboard_nav.rs:334-360`, `feature_b_web_tier.rs:568-572`),
its builder (`projects.rs:881-891`), its view-model field (`views.rs:256`), and its unit tests
(`projects.rs:1039-1110`). That is a deliberate, defensible outcome — **but it is a decision, not an accident**,
and it deletes a currently-green test. It is **ODD-1** and **Risk R1**, and DESIGN owns it.

## Jobs To Be Done (inline — no `docs/product/` SSOT in this repo)

This repo deliberately does not use a `docs/product/` SSOT; JTBD is folded in here (house convention). This is a
**user-facing** feature, so the `infrastructure-only` escape valve is **not** available — every story carries a
`job_id` referencing the single job below. Kept lightweight per the locked UX-research depth (D-6).

### JOB-1 `fast-keyboard-issue-flow` — Mei Tanaka, the keyboard-first maintainer

> **When** I'm triaging a project board with both hands on the keyboard, **I want to** create, find, select and
> open issues without reaching for the mouse, **so I can** keep my flow and get through the board faster.

- **Functional**: file a new issue, search for an existing one, move a selection down/up the board, open the
  selected issue, and get back out — all from the home row.
- **Emotional**: flow and competence. The opposite — reading a help page that lists seven shortcuts, pressing
  one, and getting **silence** — is a small, specific betrayal that teaches her the product lies to her.
- **Social**: the maintainer who moves through the tracker fast, the way she does in Linear / GitHub / Vim —
  not the one hunting for the "New issue" button.
- **Four forces**:
  - **Push**: the help overlay **already promises** these seven shortcuts. She read it, tried `c`, nothing
    happened. Today every action costs a mouse round-trip.
  - **Pull**: the exact seven keys the product already advertises, actually working — `c` to file, `/` to find,
    `j`/`k`/`Enter` to move and open, `?` for help, `Esc` to get out.
  - **Anxiety**: *"if `c` is live, will it fire while I'm typing the word 'ccc' into a title field?"* — a
    shortcut layer that eats her keystrokes is **worse than none** (this is why FR-2 is the highest-risk
    requirement). And: *"will `Cmd+C` still copy?"*
  - **Habit**: `j`/`k` from Vim/Gmail; `/` to search; `?` for help; `Esc` to dismiss. She expects the
    conventions, and Foundry's own help page has already told her these are the conventions here.
- **Opportunity score (ODI)**: Importance 7, Satisfaction 0 → **Opportunity = 7 + (7−0) = 14 (very high)** —
  satisfaction is **floor**, not merely low: the advertised capability is entirely absent, so every attempt
  fails.

## Functional requirements

- **FR-1** Pressing each of the seven advertised keys (`c`, `/`, `j`, `k`, `Enter`, `?`, `Esc`) in a browser
  performs its advertised action. The bound set **equals** `SHORTCUTS` (`keyboard.rs:48-56`) — no more, no less.
- **FR-2** **Text-input guard.** No shortcut fires while the user is typing into a text-entry context (an
  `input`, `textarea`, `contenteditable`, or equivalent). Typing "c", "j", "k", "/" or "?" into the new-issue
  title field inserts those characters and triggers **nothing**. (Exact predicate + IME composition: ODD-4.)
- **FR-3** **Modifier guard.** A shortcut key pressed with `Ctrl`, `Cmd`(Meta), or `Alt` held does **not**
  fire — `Cmd+C` copies, `Ctrl+C` is not "create issue". (`Shift` is required for `?` and is not a suppressor.)
- **FR-4** `?` renders the shipped `/keyboard-help` fragment as an **overlay on the current page** — it does
  **not** navigate away. Available on any signed-in page (mount point: ODD-3).
- **FR-5** `Esc` closes whatever modal/overlay is open (new-issue modal, help overlay, search) and returns the
  user to the page beneath with a sane selection state (FR-9). With nothing open, `Esc` is a harmless no-op.
- **FR-6** `c` opens the new-issue modal for the **current project** via the shipped
  `GET …/issues/new` htmx path (`keyboard.rs:62`), rendering into the modal mount. `c` requires a team+project
  context; on a page with no project context it does nothing (ODD-6).
- **FR-7** `/` moves focus to the search input and prevents the "/" character from being typed into it; the
  results list is the shipped `ul.search-results` fragment (`keyboard.rs:160`).
- **FR-8** `j` / `k` move a **selection** to the next / previous **visible card in DOM order**. The selected
  card takes a **ring highlight** and is **scrolled into view**. Selection is bounded (no wrap-around beyond
  the first/last card unless DESIGN chooses otherwise) and starts from no-selection on first `j`.
- **FR-9** `Enter` opens the **selected** card — equivalent to activating that card's shipped
  `hx-get={edit_url}` → `#modal-root`. With no selection, `Enter` does nothing. **Selection resets on
  navigation.**
- **FR-10** The shortcut layer **survives htmx fragment swaps**: after a swap replaces cards (a new issue is
  filed, a card is edited), the shortcuts still work without re-wiring. (Selection *survival* across a swap —
  as opposed to the *handlers* surviving — is ODD-5.)
- **FR-11** The layer adds **no new server route, endpoint, or migration**. It binds keys to the three routes
  already shipped and routed (`lib.rs:491-498`, `:536`).

## Non-Functional Requirements

### NFR-1 — Browser-observable acceptance (the anti-green-suite rule)
Every AC in this feature asserts **key-pressed → user-observable outcome** in a real browser/DOM, never an
endpoint response. The existing port-to-port suite (`us_12_keyboard_nav.rs`) already proves the server contracts
and is **green today while the feature is absent** — so any AC it could satisfy unchanged is not an AC for this
feature.
- **Measurable**: every new acceptance scenario **fails today** against `main` (the layer does not exist) and
  passes only once the key binding ships. A scenario that passes on unmodified `main` is rejected on sight.

### NFR-2 — Typing is never captured (the highest-risk guarantee)
The text-input guard (FR-2) is the single highest-risk detail in the feature: a regression makes the product
**unusable** (the user cannot type the letter `c` into a title), which is strictly worse than shipping nothing.
- **Measurable**: typing the literal string `"cjk/?"` into the new-issue title field yields exactly `"cjk/?"`
  in the field, opens no modal, moves no selection, and focuses no search box. A regression reds a dedicated
  `@property` litmus.

### NFR-3 — Modifier chords are never hijacked
`Cmd+C` / `Ctrl+C` (copy), and any `Ctrl`/`Cmd`/`Alt` chord over a shortcut key, reach the browser unmodified.
- **Measurable**: `Cmd+C` with text selected copies and does **not** open the new-issue modal; no shortcut
  fires with `Ctrl`/`Cmd`/`Alt` held.

### NFR-4 — No-JS behaviour must not regress
The shipped no-JS paths stay intact: `GET …/issues/new` **without** `HX-Request` still returns the full-page
fallback (`keyboard.rs:96-104`), and `/keyboard-help` remains reachable as a full page. The layer is **pure
progressive enhancement** — with JS disabled the app behaves exactly as it does today.
- **Measurable**: with scripting disabled, the sidebar "Keyboard shortcuts" link still renders the help page and
  the new-issue full-page form still posts to `…/issues`; every existing no-JS scenario passes unchanged.

### NFR-5 — Vendored assets only, CSP-safe, house JS idiom
No CDN. Any new script is an **external same-origin file** under `static/js/`, loaded `defer` from `base.html`
alongside the existing four (`base.html:6-9`), with **no inline handlers** — matching `board-dnd.js` /
`csrf-upload.js` (`board-dnd.js:1-17`). htmx and Alpine are already vendored under `static/vendor/`.
- **Measurable**: no `<script>` inline handler and no external origin is introduced; the layer loads from
  `/static/js/`.

### NFR-6 — Handlers survive htmx swaps
Bindings use **document-level delegation** (the shipped `board-dnd.js` `dragstart` idiom, `:67`) so
htmx-swapped fragments need no re-wiring (FR-10).
- **Measurable**: after filing an issue via `c` (which swaps `#modal-root` and re-renders cards), `j`/`k`/`Enter`
  still work with no page reload.

### NFR-7 — Accessibility of ring-highlight selection (a named, deliberate trade-off)
Ring-highlight selection is **not native focus** (D-4 rejects roving tabindex). The ring must therefore be a
**visible, non-colour-alone** indicator meeting WCAG 2.1 AA contrast, and the selected card's state must be
conveyed to assistive tech by an explicit mechanism (`aria-activedescendant` on a container, or equivalent) —
**not left to chance**. The exact mechanism is **ODD-7** and DESIGN must answer it explicitly rather than
inherit silence.
- **Measurable**: with a screen reader, moving selection with `j`/`k` announces the newly selected issue; the
  ring is distinguishable without relying on colour alone; keyboard operation never traps focus.

### NFR-8 — Drag-and-drop and selection coexist
The shipped drag-and-drop (`board-dnd.js`) keeps working unchanged; selection state and a drag never corrupt
each other (a card dragged to a new column/slot does not leave a stale ring or a dangling selection index).
- **Measurable**: dragging a selected card to another column leaves selection coherent (on the same card, or
  cleanly reset — DESIGN's call per ODD-5); every existing drag scenario passes unchanged.

## Business rules

- **BR-1** `SHORTCUTS` (`keyboard.rs:48-56`) is the **single source of truth**. The bound set equals the
  rendered help list, exactly. Adding a shortcut means adding it there first.
- **BR-2** **Guards precede everything.** The text-input guard (FR-2) and modifier guard (FR-3) are evaluated
  **before** any shortcut dispatch. No shortcut is exempt.
- **BR-3** **Surface scoping**: `c`, `/`, `j`, `k`, `Enter` are active only where they mean something — a
  surface with a project context and/or issue cards (board, search results). `?` and `Esc` are **global** on any
  signed-in page.
- **BR-4** **`Esc` precedence**: `Esc` closes the **topmost** open layer only (help overlay over modal, etc.),
  one layer per press. With nothing open it is a no-op — never a navigation.
- **BR-5** **Selection is ephemeral**: it lives in the client only, is never persisted, and **resets on
  navigation**. No selection state reaches the server.
- **BR-6** **Progressive enhancement**: no shortcut is the *only* path to any action. Every action a shortcut
  triggers has a pointer/no-JS equivalent that keeps working (the "New issue" button `board.html:6`, the
  sidebar help link `sidebar.html:13`).
- **BR-7** `?` requires `Shift` (it is `Shift+/` on a US layout). `Shift` is **not** treated as a suppressing
  modifier (BR-2/FR-3 covers `Ctrl`/`Cmd`/`Alt` only).

## Alternatives considered (constraint rationale)

- **Bind all seven now** (vs ship `c` alone first): chose all seven (locked, D-1). The help overlay advertises
  seven; shipping one leaves six documented lies in place. The seven are also cheap once the guarded dispatch
  layer exists — the layer, not the individual keys, is the work.
- **Visible-card DOM order** (vs the shipped `#kb-items` ASC carrier): chose visible cards (locked, D-4). A ring
  highlight and `scrollIntoView` are **meaningless on a `hidden` element**, and "selection follows what my eyes
  see" is the only model a user can predict. The cost is retiring `#kb-items` and deleting a green test
  (ODD-1, R1) — accepted deliberately.
- **Ring highlight** (vs roving tabindex / native focus): chose ring (locked, D-4). Roving tabindex would be
  more accessible for free but is a larger rewrite of the board's focus model and fights drag-and-drop. The
  a11y gap this creates is **not waved away** — it is NFR-7 + ODD-7 + Risk R3, and DESIGN must answer it.
- **Vanilla `keyboard.js`** (vs Alpine `x-on:keydown`): **not decided here** (ODD-2). The house pattern is
  vanilla document-delegated IIFEs (`board-dnd.js`, `csrf-upload.js`); `keyboard.rs`'s doc says "alpine.js" and
  Alpine *is* vendored and loaded but unused by app code. Requirements stay solution-neutral; DESIGN picks.
- **Overlay for `?`** (vs keeping the full-page navigation): chose overlay (locked). The server already returns
  a bare `role="dialog"` fragment built for exactly this (`keyboard_help.html:1`), and navigating away from the
  board to read a shortcut list defeats the shortcut. The existing full-page link stays as the no-JS path
  (ODD-8, NFR-4).
- **No walking skeleton** (vs the usual thinnest end-to-end slice): correct here (locked, D-8) — this is
  brownfield. The server contracts are shipped and routed; the end-to-end skeleton already exists. The gap is
  exactly the client layer, so slice 01 is a **thin real capability** (`?` + `Esc`), not a skeleton.

## Risk assessment (surfaced, not managed)

| # | Risk | Category | Probability | Impact | Mitigation |
|---|------|----------|-------------|--------|------------|
| R1 | **`#kb-items` collision** — the shipped hidden ASC carrier + its green acceptance assertion contradict the locked visible-DOM-order model; honouring D-4 means **deleting a passing test** | Technical/Process | **High (certain)** | High | ODD-1 decides explicitly; `AGENTS.md` dead-code policy mandates full removal (carrier, builder `projects.rs:881-891`, view-model `views.rs:256`, unit tests `:1039-1110`, acceptance assertions `us_12_keyboard_nav.rs:334-360`, `feature_b_web_tier.rs:568-572`) rather than leaving it inert. |
| R2 | **Text-input guard fails** — a shortcut fires while typing; the user cannot type "c" into a title | UX/Correctness | Medium | **Critical** | FR-2 + NFR-2 (the highest-risk guarantee); guard evaluated before dispatch (BR-2); dedicated `@property` litmus; ODD-4 pins the predicate incl. IME composition. |
| R3 | **Ring selection is invisible to screen readers** — not native focus, so selection changes are not announced | Accessibility | **High** | Medium | NFR-7 makes the trade-off explicit and demands a mechanism; ODD-7 forces DESIGN to answer (`aria-activedescendant` or equivalent) instead of inheriting silence. Roving tabindex was rejected knowingly (D-4). |
| R4 | **Selection breaks across htmx swaps / drag-and-drop** — a swap replaces cards and leaves a stale ring or dangling index | Technical | Medium | Medium | FR-10 + NFR-6 (document-delegated handlers, the `board-dnd.js:67` idiom); NFR-8 (coexistence); ODD-5 decides key-based vs index-based selection survival. |
| R5 | **Global `?` has nowhere to render** — `#modal-root` exists only in `board.html:13`, not in `app_shell.html`, yet `?` is global | Technical | **High (certain)** | Medium | ODD-3 (move/add a shell-level mount vs inject on demand). Surfaced now precisely because "global `?`" is locked. |
| R6 | **Modifier hijack** — `Cmd+C` opens a modal instead of copying | UX | Low | High | FR-3 + NFR-3; explicit modifier guard before dispatch (BR-2); `Shift` deliberately excluded as a suppressor (BR-7). |
| R7 | **No-JS regression** — enhancing the modal path breaks the shipped full-page fallback | Technical | Low | Medium | NFR-4; the fork is server-side and untouched (`keyboard.rs:96-104`); pure progressive enhancement (BR-6); existing no-JS scenarios must pass unchanged. |
| R8 | **"Issue list" in the locked scope does not exist** — no `/issues` GET route, no list page; only the board and the search-results fragment carry `data-issue-key` | Scope | **High (certain)** | Medium | ODD-6 — verified: `lib.rs:487-490` registers `…/issues` as **POST only**. Interpreted as the search-results list; DESIGN/PO confirm. Building a list page is a different feature. |
| R9 | **Alpine vs vanilla drift** — `keyboard.rs` doc says "alpine.js handlers"; the house pattern is vanilla | Technical | Medium | Low | ODD-2; whichever DESIGN picks, the stale doc comment (`keyboard.rs:1-30`) must be corrected in the same change. |

## Glossary (ubiquitous language)

- **Shortcut** — one of the seven advertised keys in `SHORTCUTS` (`keyboard.rs:48-56`).
- **The seven** — `c` Create issue, `/` Search, `j` Next, `k` Previous, `Enter` Open selected, `?` Show this
  help, `Esc` Close modal. The help overlay's list; the contract.
- **The client layer** — the missing browser-side code that binds keys to the shipped routes. **This feature.**
- **Text-input guard** — the rule that no shortcut fires while the user is typing in a text-entry context (FR-2).
- **Modifier guard** — the rule that `Ctrl`/`Cmd`/`Alt` chords never trigger a shortcut (FR-3).
- **Selection** — the client-only, ephemeral pointer to one visible card, shown as a ring highlight; moved by
  `j`/`k`, opened by `Enter`, reset on navigation (BR-5).
- **Ring highlight** — the visible indicator on the selected card. **Not** native browser focus (NFR-7).
- **Visible card** — an `article.issue-card` rendered on the board (`issue_card.html:1`); the thing selection
  walks — as opposed to the hidden `#kb-items` carrier.
- **`#kb-items` carrier** — the hidden, `aria-hidden`, ASC-by-number list at `board.html:12`, built for the
  never-written handler; **retired** by the locked selection model (ODD-1).
- **Overlay** — a layer rendered over the current page (the `?` help), as opposed to a page navigation.
- **Green-but-absent** — the state this feature starts from: a port-to-port suite proving server contracts while
  the user-visible feature does not exist (NFR-1).
