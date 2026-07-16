<!-- markdownlint-disable MD024 -->

# User Stories — keyboard-shortcut-bindings (the missing client layer)

## System Constraints (cross-cutting — apply to every story)

- **The help overlay is the contract.** `SHORTCUTS` (`crates/foundry-app/src/keyboard.rs:48-56`) is the single
  source of truth for the seven keys; the bound set equals the rendered list, exactly (BR-1). An acceptance test
  already asserts the list is complete.
- **Guards precede every dispatch (BR-2).** No shortcut fires while the user is typing into a text-entry context
  (FR-2, NFR-2), and no shortcut fires with `Ctrl`/`Cmd`/`Alt` held (FR-3, NFR-3). No shortcut is exempt.
  `Shift` is **not** a suppressor — `?` is `Shift+/` (BR-7).
- **Browser-observable ACs only (NFR-1).** The shipped port-to-port suite (`us_12_keyboard_nav.rs`) proves the
  *server contracts* and is **green today while the feature is absent**. Every AC below asserts
  **key-pressed → observable outcome**. Any scenario that passes unchanged on `main` is rejected on sight.
- **Zero new server surface (FR-11).** All three routes are shipped and routed (`lib.rs:491-498`, `:536`). This
  feature adds **no route, no endpoint, no migration**. It is client-only.
- **Progressive enhancement (BR-6, NFR-4).** No shortcut is the only path to any action; the no-JS full-page
  fallbacks (`keyboard.rs:96-104`, sidebar link `sidebar.html:13`) must not regress.
- **Vendored + CSP-safe + house idiom (NFR-5).** External same-origin script under `static/js/`, `defer` from
  `base.html:6-9`, no inline handlers, document-level delegation — the `board-dnd.js` / `csrf-upload.js` pattern.
  No CDN. (Vanilla vs Alpine is **ODD-2**.)
- **Selection is ephemeral (BR-5).** Client-only, never persisted, resets on navigation. It walks **visible
  cards** (`article.issue-card`, `issue_card.html:1`), **not** the hidden `#kb-items` carrier — which the locked
  model retires (**ODD-1**, Risk R1).
- **Surface scoping (BR-3).** `c`/`/`/`j`/`k`/`Enter` are active where they mean something (board, search
  results); `?` and `Esc` are global on any signed-in page. The global `?` has **no mount point** on non-board
  pages today (`#modal-root` is `board.html:13` only) — **ODD-3**.
- **JTBD traceability.** JTBD is folded inline (no `docs/product/` SSOT — see `requirements.md`). This is a
  **user-facing** feature, so `infrastructure-only` is **not available**. Every story carries
  `job_id: fast-keyboard-issue-flow`.
- **Personas are the repo's own harness identities** — **Mei Tanaka** (`mei@acme.com`) and **Hiroshi Sato**
  (`hiroshi@acme.com`), registered at `us_12_keyboard_nav.rs:58-64`, so every example is concrete and runnable.

---

## US-01: Press `?` and actually see the shortcut list, right where I am

`job_id: fast-keyboard-issue-flow`

### Elevator Pitch
- **Before**: Mei Tanaka is on the AUTH board and wants to know the shortcuts. Pressing `?` does **nothing** —
  there is no handler. Her only route is the sidebar's "Keyboard shortcuts" link (`sidebar.html:13`), which
  **navigates her off the board** to a full page. To learn how to move faster she must first stop working.
- **After**: Mei presses `?` on the board. The shipped `/keyboard-help` fragment
  (`section.keyboard-help[role="dialog"]`, `keyboard_help.html:1`) appears as an **overlay over the board** —
  she reads `c — Create issue`, `j — Next`, `Enter — Open selected` with her board still behind it. `Esc`
  dismisses it and she is exactly where she was.
- **Decision enabled**: Mei can learn (and re-check) the shortcut vocabulary **without losing her place**, so
  the shortcuts are discoverable at the moment she needs them rather than a page-trip away.

### Problem
The help overlay is the product's own advertisement for the keyboard layer, and it is only reachable by a
full-page navigation — the single most flow-breaking action, to learn how to preserve flow. Meanwhile `?`, which
the help page itself lists as "Show this help", does nothing at all.

### Who
- A **keyboard-first maintainer** (Mei) on any signed-in page | knows shortcuts exist because the product says
  so | wants the list without leaving her work | motivated by discoverability at the point of need.

### Solution
Bind `?` (`Shift+/`, BR-7) at the document level to fetch the shipped **public** `GET /keyboard-help`
(`keyboard.rs:259`, route `lib.rs:536`) and render the returned bare `role="dialog"` fragment as an overlay on
the current page; `Esc` closes it (BR-4). The route is public by design (`keyboard.rs:19-24`: the bootstrap GETs
it once and caches it), so help works even on the sign-in page. The sidebar/dashboard full-page links stay as
the no-JS path (ODD-8, NFR-4). This story stands up the **guarded dispatch layer** the other six keys reuse.

### Domain Examples
1. **Happy path** — Mei, on the AUTH board, presses `?`; the help overlay appears over the board listing all
   seven shortcuts (`c`, `/`, `j`, `k`, `Enter`, `?`, `Esc`); the board is still visible behind it; `Esc`
   dismisses it and no navigation occurred.
2. **Edge: not on the board** — Mei presses `?` on the dashboard (`/`), a page with no `#modal-root`
   (`app_shell.html` has none — ODD-3); the overlay still appears, because `?` is global (BR-3).
3. **Boundary: the list matches the source** — the overlay Mei sees lists exactly the seven entries in
   `SHORTCUTS` (`keyboard.rs:48-56`) — no more, no fewer (BR-1).

### UAT Scenarios (BDD)
#### Scenario: Pressing the help key shows the shortcut list over the current page
Given Mei is signed in and viewing the AUTH project board
When Mei presses "?"
Then the keyboard shortcut list appears as an overlay over the board
And the board is still visible behind it
And the browser did not navigate away from the board

#### Scenario: The help overlay lists every advertised shortcut
Given Mei has opened the help overlay
When Mei reads it
Then it lists a description for each of "c", "/", "j", "k", "Enter", "?" and "Esc"

#### Scenario: The help overlay is available away from the board
Given Mei is signed in and viewing the dashboard
When Mei presses "?"
Then the keyboard shortcut list appears as an overlay over the dashboard

#### Scenario: Dismissing the help returns Mei exactly where she was
Given Mei has the help overlay open over the AUTH board
When Mei presses "Esc"
Then the help overlay closes
And Mei is still on the AUTH board with nothing else changed

### Acceptance Criteria
- [ ] Pressing `?` on a signed-in page renders the `/keyboard-help` fragment as an overlay **over the current page**, with no navigation (FR-4).
- [ ] The overlay lists exactly the seven entries in `SHORTCUTS` — the bound set equals the rendered set (BR-1, FR-1).
- [ ] `?` works on a page without a board (the global scope, BR-3) — the mount point is resolved per ODD-3.
- [ ] `Esc` closes the help overlay and restores the underlying page unchanged (FR-5, BR-4).
- [ ] The sidebar/dashboard full-page `/keyboard-help` links still work with scripting disabled (NFR-4, ODD-8).
- [ ] The scenario **fails on unmodified `main`** — pressing `?` today does nothing (NFR-1).

### Outcome KPIs
- **Who**: signed-in maintainers who want the shortcut list
- **Does what**: open the shortcut help without leaving their current page
- **By how much**: 100% of `?` presses on a signed-in page show the overlay in place; 0 navigations away
- **Measured by**: browser-level scenario (key press → overlay present → URL unchanged), revert-reds-it
- **Baseline**: 0% — `?` is unbound today; help is a full-page navigation only

### Technical Notes
- Reuses the shipped public `GET /keyboard-help` (`keyboard.rs:259`, `lib.rs:536`) + bare fragment (`keyboard_help.html:1`). **No new route.**
- Stands up the guarded dispatch layer (FR-2/FR-3 guards) that US-02..US-07 reuse; house idiom NFR-5; delegation NFR-6.
- **ODD-2** (vanilla vs Alpine — `keyboard.rs`'s doc says "alpine.js"; house pattern is vanilla). **ODD-3** (the global mount point: `#modal-root` is `board.html:13` only, absent from `app_shell.html`). **ODD-8** (fate of the sidebar link).
- Deliberately **not** a walking skeleton (D-8) — brownfield; the server contract is shipped. This is a thin real capability.

---

## US-02: Type the letter "c" into a title without filing a new issue

`job_id: fast-keyboard-issue-flow`

### Elevator Pitch
- **Before**: the moment `c` becomes live, a naive binding fires on **every** `c` keypress — including while
  Mei is typing `"cache invalidation on login"` into the new-issue title field. The first character opens a
  second modal, the rest scatter. A shortcut layer that eats keystrokes is **strictly worse than no shortcuts
  at all**, and it is the default outcome of the obvious implementation.
- **After**: Mei types `"cache invalidation on login"` into the title field and gets exactly
  `"cache invalidation on login"`. No modal opens. She types `j`, `k`, `/` and `?` into the description and gets
  the literal characters. She hits `Cmd+C` to copy an issue key and it **copies** — it does not file an issue.
  The shortcuts wake up again the instant she leaves the field.
- **Decision enabled**: Mei can trust the keyboard layer enough to leave it on — she can type prose and use
  system chords without the product fighting her, which is the precondition for every other shortcut being
  usable at all.

### Problem
Every one of the seven shortcuts is a **plain printable character or a bare key** (`c`, `/`, `j`, `k`, `Enter`,
`?`, `Esc`). Those are exactly the characters people type. Without an explicit guard evaluated **before**
dispatch, binding them makes text entry impossible and hijacks `Cmd+C`. This is the highest-risk detail in the
feature (NFR-2, Risk R2) and the reason the layer needs a real design rather than seven `if` statements.

### Who
- **Every user of every shortcut** (Mei, Hiroshi) | typing issue titles, descriptions, search queries, comments |
  motivated by *not* having their keystrokes stolen | this is the story that makes the other six safe.

### Solution
Evaluate two guards **before any shortcut dispatch** (BR-2): a **text-input guard** — the event target is an
`input`, `textarea`, `contenteditable`, or equivalent text-entry context ⇒ do nothing (FR-2); and a **modifier
guard** — `Ctrl`, `Cmd`(Meta), or `Alt` held ⇒ do nothing (FR-3), while `Shift` is explicitly **not** a
suppressor because `?` is `Shift+/` (BR-7). The exact predicate — the element/role set, and **IME composition**
(`isComposing`), which matters concretely for a Japanese-input user like Mei — is **ODD-4**.

### Domain Examples
1. **Happy path** — Mei opens the new-issue modal and types `"cache invalidation on login"` into the title; the
   field contains exactly that string; no second modal opened; no selection moved.
2. **Edge: every shortcut char at once** — Mei types the literal `"cjk/?"` into the description field; the field
   contains `"cjk/?"`; no modal, no search focus, no selection change (the NFR-2 litmus).
3. **Boundary: modifier chord** — Mei selects the text `AUTH-2` and presses `Cmd+C`; the text is copied to the
   clipboard and the new-issue modal does **not** open (FR-3, NFR-3).
4. **Boundary: guard releases** — Mei presses `Esc` to leave the field, then presses `c`; the new-issue modal
   opens normally — the guard suppresses only while typing, it does not disable the layer.

### UAT Scenarios (BDD)
#### Scenario: Typing shortcut letters into a title inserts them instead of firing shortcuts
Given Mei is signed in with the new-issue modal open on the AUTH board
When Mei types "cache invalidation on login" into the title field
Then the title field contains "cache invalidation on login"
And no additional modal was opened
And no card selection changed

#### Scenario: Every shortcut character can be typed into a text field
Given Mei is typing in the description field of the new-issue modal
When Mei types the characters "cjk/?"
Then the description field contains "cjk/?"
And no modal opened, no search box was focused, and no selection moved

#### Scenario: A copy chord copies instead of creating an issue
Given Mei is viewing the AUTH board with the issue key "AUTH-2" selected as text
When Mei presses "Cmd+C"
Then the text is copied
And the new-issue modal does not open

#### Scenario: Shortcuts work again once Mei leaves the text field
Given Mei has finished typing in the title field and has left it
When Mei presses "c"
Then the new-issue modal opens

### Acceptance Criteria
- [ ] No shortcut fires while the event target is a text-entry context (`input`, `textarea`, `contenteditable`, or equivalent) — typing `c`/`j`/`k`/`/`/`?` inserts the literal characters (FR-2, NFR-2).
- [ ] No shortcut fires while `Ctrl`, `Cmd`(Meta) or `Alt` is held; `Cmd+C`/`Ctrl+C` still copy (FR-3, NFR-3).
- [ ] `Shift` is **not** treated as a suppressor — `?` (`Shift+/`) still fires outside a text field (BR-7).
- [ ] The guards are evaluated **before** dispatch and apply to **all seven** shortcuts with no exemptions (BR-2).
- [ ] Leaving the text field re-enables the shortcuts immediately — the guard is contextual, not a global toggle.
- [ ] A regression that lets a shortcut fire during typing **reds a dedicated `@property` litmus** (NFR-2).
- [ ] The scenarios **fail on unmodified `main`** in the sense that they pin the guard the layer must ship with (NFR-1).

### Outcome KPIs
- **Who**: maintainers typing into any Foundry text field while the shortcut layer is live
- **Does what**: type shortcut characters and system chords without triggering shortcuts
- **By how much**: **0** shortcut activations from a keystroke aimed at a text field; 0 hijacked `Cmd`/`Ctrl`/`Alt` chords (hard guardrail)
- **Measured by**: the text-input-guard `@property` litmus + the modifier-guard scenario, both browser-level
- **Baseline**: N/A today (no shortcuts are bound, so nothing is captured) — establishes the invariant **as** the layer is introduced

### Technical Notes
- **The highest-risk requirement in the feature** (NFR-2, Risk R2). Guard-before-dispatch is a structural rule (BR-2), not a per-shortcut check.
- **ODD-4** pins the exact predicate: element/role set, `contenteditable`, `select`, `role="textbox"`, and **IME composition** (`isComposing`) — directly relevant to a Japanese-input user (Mei Tanaka).
- Depends on US-01 (the dispatch layer it guards). No new persistence, no new route.
- Guardrail story: it delivers **user-visible value** (typing works) and makes US-03..US-07 safe to ship.

---

## US-03: Press `c` and file an issue without touching the mouse

`job_id: fast-keyboard-issue-flow`

### Elevator Pitch
- **Before**: Mei is triaging the AUTH board and spots a bug. The help page told her `c` creates an issue. She
  presses `c`. **Nothing happens.** She reaches for the mouse, hunts for the "New issue" button
  (`board.html:6`), and clicks it — the exact mouse round-trip the shortcut exists to remove.
- **After**: Mei presses `c` on the AUTH board. The shipped new-issue modal
  (`GET /team/{team}/project/{slug}/issues/new` with `HX-Request: true` → `keyboard.rs:96-101`) opens over the
  board with the **title field already focused** (`input[name=title][autofocus]`). She types the title, submits,
  and the card appears. `Esc` backs out at any point.
- **Decision enabled**: Mei can capture a bug the instant she sees it, at the speed she thinks, so triage
  ideas don't die in the round-trip to the mouse.

### Problem
`c` is the single most-used shortcut in every issue tracker and the first entry in Foundry's own help list. It
is unbound. The server route, the modal fragment, the CSRF cookie, and the autofocused title input all exist and
are routed — only the keypress that reaches them is missing.

### Who
- A **keyboard-first maintainer** (Mei) on a project board | has a bug in mind right now | wants it filed in one
  keystroke | motivated by capture speed and staying in flow.

### Solution
Bind `c` (guarded per US-02) on surfaces with a **team+project context** to trigger the shipped htmx path —
`GET /team/{team_slug}/project/{project_slug}/issues/new` with `HX-Request: true`, swapping the returned bare
fragment into the modal mount (`#modal-root`, `board.html:13`) — exactly what the "New issue" button already
does (`board.html:6`). The full-page fallback (`keyboard.rs:102-104`) is untouched for no-JS (NFR-4, BR-6).
`Esc` closes the modal (US-07). `c` needs a project context, so on a page without one it does nothing (BR-3,
ODD-6).

### Domain Examples
1. **Happy path** — Mei, on the AUTH board, presses `c`; the new-issue modal opens over the board with the title
   field focused; she types `"Session cookie not cleared on sign-out"`, submits, and the card appears on the board.
2. **Edge: no project context** — Mei presses `c` on the dashboard, which has no team+project; nothing happens
   (no modal, no error, no navigation) — the route requires a team+project (`keyboard.rs:62-95`, BR-3).
3. **Boundary: guard holds** — Mei presses `c` while the title field of an already-open modal is focused; the
   letter `c` is typed into the title and no second modal opens (US-02, FR-2).
4. **Boundary: no-JS unaffected** — with scripting disabled, the "New issue" button still navigates to the
   full-page form and posts to `…/issues` (NFR-4).

### UAT Scenarios (BDD)
#### Scenario: Pressing the create key opens the new-issue modal on the board
Given Mei is signed in and viewing the AUTH project board
When Mei presses "c"
Then the new-issue modal opens over the board
And the title field is focused and ready for typing

#### Scenario: Mei files an issue entirely from the keyboard
Given Mei has opened the new-issue modal by pressing "c"
When Mei types "Session cookie not cleared on sign-out" and submits the form
Then a new issue with that title appears on the AUTH board

#### Scenario: The create key does nothing where there is no project
Given Mei is signed in and viewing the dashboard
When Mei presses "c"
Then no modal opens
And the browser does not navigate away

#### Scenario: Filing without a mouse leaves the no-JS path working
Given scripting is disabled in Mei's browser
When Mei activates the "New issue" button on the AUTH board
Then the full-page new-issue form is shown
And submitting it creates the issue

### Acceptance Criteria
- [ ] Pressing `c` on a board opens the new-issue modal over the page via the shipped htmx fragment path, with the title field focused (FR-6).
- [ ] The issue can be filed end-to-end from the keyboard — the created card appears on the board.
- [ ] `c` does nothing on a page with no team+project context — no modal, no error, no navigation (BR-3, ODD-6).
- [ ] `c` never fires while typing or with a modifier held (US-02 guards apply — BR-2).
- [ ] The no-JS full-page fallback (`keyboard.rs:102-104`) and the "New issue" button are unchanged (NFR-4, BR-6).
- [ ] The scenario **fails on unmodified `main`** — pressing `c` today does nothing (NFR-1).

### Outcome KPIs
- **Who**: maintainers triaging a project board
- **Does what**: open the new-issue modal from the keyboard instead of a mouse round-trip to the button
- **By how much**: 100% of `c` presses on a board open the modal with the title focused; **0** mouse actions required to file an issue
- **Measured by**: browser-level scenario (key press → modal present → title focused → submit → card on board)
- **Baseline**: 0% — `c` is unbound; filing an issue requires a pointer (or the no-JS full page)

### Technical Notes
- Reuses `show_new_issue_modal` (`keyboard.rs:62`, route `lib.rs:491-494`) + `#modal-root` (`board.html:13`) + the same `hx-get` the button already uses (`board.html:6`). **No new route.**
- CSRF is already minted server-side by `ensure_csrf_cookie` (`keyboard.rs:94`) — no client CSRF work needed (unlike `board-dnd.js`, which mirrors the cookie into a header).
- Depends on US-01 (dispatch layer) + US-02 (guards). Pairs with US-07 (`Esc` closes it).
- **ODD-6** — `c`'s surface scope; note `…/issues` is registered **POST-only** (`lib.rs:487-490`), so no issue-list page exists.

---

## US-04: Press `/` and search the board without reaching for the mouse

`job_id: fast-keyboard-issue-flow`

### Elevator Pitch
- **Before**: Mei knows the bug is called something like "session cookie" but not which issue it is. `/` — the
  universal "search" key, and the second entry in Foundry's help list — does **nothing**. She mouses to the
  search box, if she can find it.
- **After**: Mei presses `/` on the AUTH board. The search input takes focus, **and the "/" character is not
  typed into it** (the classic bug). She types `session`; the shipped `GET …/search?q=session` fragment
  (`ul.search-results` with `li.search-result[data-issue-key]`, `search_results.html:4`) lists `AUTH-2 Session
  cookie not cleared on sign-out`. A search with no matches shows the empty state
  (`ul.search-results[data-empty="true"]`) rather than a blank void.
- **Decision enabled**: Mei can jump from "I half-remember an issue" to "I'm looking at it" without leaving the
  keyboard, so finding an existing issue stops being slower than filing a duplicate.

### Problem
`/` is unbound, so the shipped search endpoint — with its exact-key matching (`AUTH-2`), case-insensitive title
substring matching, and a real empty state (`keyboard.rs:208-231`) — is unreachable by the key that is supposed
to reach it. Worse, the naive binding focuses the field *and* types "/" into it, so the user's first search is
always for `/session`.

### Who
- A **keyboard-first maintainer** (Mei) on a project board | half-remembers an issue's title or key | wants to
  find it without a pointer | motivated by not filing a duplicate she could have found.

### Solution
Bind `/` (guarded per US-02) to move focus to the search input and **suppress the default keypress** so the "/"
character is not inserted (FR-7). Results come from the shipped `GET …/search?q=` fragment (`keyboard.rs:160`,
route `lib.rs:495-498`), which already handles exact-key and substring matching and emits
`ul.search-results[data-empty="true"]` for no matches. The results list is the second issue-key-bearing surface,
so `j`/`k`/`Enter` apply to it too (US-05/US-06, ODD-6). `Esc` closes search (US-07).

### Domain Examples
1. **Happy path** — Mei, on the AUTH board, presses `/`; the search input takes focus and is **empty** (no "/"
   character); she types `session`; the results list shows `AUTH-2 — Session cookie not cleared on sign-out`.
2. **Edge: exact key** — Mei presses `/` and types `AUTH-2`; the exact-key path (`keyboard.rs:217-224`) returns
   precisely issue AUTH-2, not every issue whose title contains "auth".
3. **Boundary: no matches** — Mei searches `zzz`; the empty state (`ul.search-results[data-empty="true"]`) is
   shown — she can tell "nothing matched" apart from "search is broken".
4. **Boundary: typing "/" in the box** — once focused, Mei types `and/or` into the search field; the literal
   `and/or` is entered and focus is not re-grabbed (US-02 guard, FR-2).

### UAT Scenarios (BDD)
#### Scenario: Pressing the search key focuses the search box without typing a slash
Given Mei is signed in and viewing the AUTH project board
When Mei presses "/"
Then the search input is focused
And the search input is empty

#### Scenario: Mei finds an issue by typing part of its title
Given Mei has focused the search box by pressing "/"
When Mei types "session"
Then the results list shows the issue "Session cookie not cleared on sign-out"

#### Scenario: Mei finds an issue by its exact key
Given Mei has focused the search box by pressing "/"
When Mei types "AUTH-2"
Then the results list shows exactly the issue AUTH-2

#### Scenario: A search that matches nothing says so
Given Mei has focused the search box by pressing "/"
When Mei types "zzz"
Then the results list shows an empty state indicating nothing matched

### Acceptance Criteria
- [ ] Pressing `/` on a board focuses the search input and the "/" character is **not** inserted into it (FR-7).
- [ ] Typing a title substring lists matching issues from the shipped search fragment; an exact key (`AUTH-2`) returns exactly that issue.
- [ ] A query with no matches renders the shipped empty state (`ul.search-results[data-empty="true"]`), distinguishable from "no query".
- [ ] Once the search box is focused, shortcut characters typed into it are inserted literally (US-02 guard, FR-2).
- [ ] `Esc` closes/leaves search and restores the board (US-07, FR-5).
- [ ] The scenario **fails on unmodified `main`** — pressing `/` today does nothing (NFR-1).

### Outcome KPIs
- **Who**: maintainers looking for an existing issue on a board
- **Does what**: reach search and find an issue from the keyboard
- **By how much**: 100% of `/` presses focus the search box with **0** stray "/" characters inserted; 0 mouse actions to reach search
- **Measured by**: browser-level scenario (key press → search focused → field empty → query → results present)
- **Baseline**: 0% — `/` is unbound; the shipped search endpoint is unreachable by keyboard

### Technical Notes
- Reuses `search_issues` (`keyboard.rs:160`, route `lib.rs:495-498`) + `filter_matches` (`:208-231`) + `search_results.html:4`. **No new route.**
- The "don't type the slash" detail requires suppressing the default on the focusing keypress — a classic, cheap bug if missed.
- The results list is the **second issue-key-bearing surface**, so it is in `j`/`k`/`Enter` scope (ODD-6).
- Depends on US-01 (dispatch) + US-02 (guards). Search input location/markup is a DESIGN detail (the board has no search box today — ODD-6).

---

## US-05: Walk the board with `j` and `k` and see where I am

`job_id: fast-keyboard-issue-flow`

### Elevator Pitch
- **Before**: `j` and `k` — "Next" and "Previous" in Foundry's own help list — do **nothing**. There is no
  selection concept in the browser at all. To move between cards Mei moves a mouse and reads with her eyes;
  there is no keyboard notion of "the current issue", so `Enter` has nothing to open either.
- **After**: Mei presses `j` on the AUTH board. The first visible card takes a **ring highlight**. `j` again
  moves to the next visible card **in the order she sees them**; `k` walks back. A selection below the fold
  **scrolls into view**. She can see exactly which issue is current at all times, and it is always the one her
  eyes are on — not a hidden list's idea of it.
- **Decision enabled**: Mei always knows which issue is "current", so she can move down a column and act on the
  right issue with confidence — the precondition for `Enter` meaning anything.

### Problem
There is no selection model in the client. `j`/`k` are advertised and unbound, and `Enter` ("Open selected") is
meaningless without a "selected". The board **does** ship a hidden `#kb-items` carrier
(`board.html:12`) built for this — but it is `hidden aria-hidden="true"` and sorted **ascending by issue
number across all columns**, while the visible board is **column-grouped and descending**. A ring highlight and
`scrollIntoView` are **meaningless on a hidden element**, so the carrier cannot serve the model the user needs
(ODD-1, Risk R1).

### Who
- A **keyboard-first maintainer** (Mei) on a project board with several cards | wants to move between issues
  without a pointer | needs to *see* which one is current | motivated by predictability — selection must follow
  her eyes.

### Solution
Maintain a client-only, ephemeral **selection** (BR-5) over the **visible cards** (`article.issue-card`,
`issue_card.html:1`) in **DOM order**. `j` moves to the next, `k` to the previous; the selected card takes a
**ring highlight** and is **scrolled into view**; selection **resets on navigation**. Handlers are
document-delegated so htmx swaps don't break them (NFR-6, the `board-dnd.js:67` idiom). This **retires the
`#kb-items` carrier** and, per the repo's dead-code policy, deletes it along with its builder
(`projects.rs:881-891`), view-model field (`views.rs:256`), unit tests (`:1039-1110`), and its acceptance
assertions (`us_12_keyboard_nav.rs:334-360`, `feature_b_web_tier.rs:568-572`) — **ODD-1**. The a11y mechanism
for announcing a non-focus selection is **ODD-7** (NFR-7).

### Domain Examples
1. **Happy path** — Mei, on the AUTH board showing `AUTH-3`, `AUTH-2`, `AUTH-1`, presses `j`; `AUTH-3` (the
   first **visible** card) takes the ring; `j` again selects `AUTH-2`; `k` returns to `AUTH-3`.
2. **Edge: selection follows the eyes, not the hidden carrier** — the visible board is column-grouped and
   DESC-within-column while `#kb-items` is ASC-by-number; pressing `j` twice from the top lands on the **second
   card Mei can see**, which is *not* what the hidden carrier's order would give (ODD-1).
3. **Boundary: below the fold** — the AUTH board has 30 cards; Mei holds `j` past the visible area; the selected
   card is scrolled into view and the ring stays visible.
4. **Boundary: bounds + empty** — `k` at the first card stays at the first (no wrap unless DESIGN chooses
   otherwise, FR-8); on an empty column/board (`board.html:9` renders `"No issues yet — press c to file the
   first one."`) `j` selects nothing and does not error.
5. **Boundary: coexists with drag** — Mei drags `AUTH-2` to another column with the mouse; the drag works
   unchanged (`board-dnd.js`) and selection is left coherent, not stale (NFR-8, ODD-5).

### UAT Scenarios (BDD)
#### Scenario: The next key selects the first visible card and highlights it
Given Mei is signed in and viewing the AUTH board showing issues AUTH-3, AUTH-2 and AUTH-1
When Mei presses "j"
Then the first visible card is highlighted as selected

#### Scenario: Next and previous walk the cards in the order Mei sees them
Given Mei has selected the first visible card on the AUTH board
When Mei presses "j" and then "k"
Then the selection moves to the second visible card and back to the first
And the selection order matches the order the cards appear on screen

#### Scenario: A selection below the fold scrolls into view
Given Mei is viewing the AUTH board with more cards than fit on screen
When Mei presses "j" repeatedly until the selection passes the bottom of the viewport
Then the selected card is scrolled into view and its highlight is visible

#### Scenario: Moving previous from the first card stays put
Given Mei has the first visible card selected
When Mei presses "k"
Then the first card remains selected
And no error occurs

#### Scenario: Dragging a card with the mouse leaves selection coherent
Given Mei has a card selected on the AUTH board
When Mei drags that card into another column with the mouse
Then the drag completes as it does today
And no stale highlight is left behind

### Acceptance Criteria
- [ ] `j` / `k` move a selection to the next / previous **visible card in DOM order**; the order matches what the user sees (FR-8).
- [ ] The selected card shows a **ring highlight** that is visible, meets WCAG 2.1 AA contrast, and does not rely on colour alone (NFR-7).
- [ ] A selection outside the viewport is **scrolled into view** (FR-8).
- [ ] Selection is bounded (no wrap past first/last unless DESIGN chooses otherwise) and is a no-op on an empty board.
- [ ] Selection **resets on navigation** and is never persisted or sent to the server (BR-5).
- [ ] The hidden `#kb-items` carrier is **retired and removed** — carrier, builder, view-model field, unit tests, and its two acceptance assertions — per the dead-code policy (ODD-1).
- [ ] Selection changes are conveyed to assistive technology by an explicit mechanism (ODD-7, NFR-7) — not left to chance.
- [ ] Drag-and-drop keeps working and selection stays coherent across a drag and across an htmx swap (NFR-6, NFR-8, ODD-5).
- [ ] The scenarios **fail on unmodified `main`** — `j`/`k` today do nothing (NFR-1).

### Outcome KPIs
- **Who**: maintainers moving between issues on a board
- **Does what**: move a visible selection between cards from the keyboard and always see which is current
- **By how much**: 100% of `j`/`k` presses move the selection to the adjacent **visible** card and keep it in view; 0 mouse actions to change the current issue
- **Measured by**: browser-level scenario (key press → ring on expected card → card in viewport), revert-reds-it
- **Baseline**: 0% — there is no selection concept in the client at all today

### Technical Notes
- Walks visible `article.issue-card` (`issue_card.html:1`) — **not** `#kb-items` (`board.html:12`). This is the locked model (D-4) and it **retires the carrier** (ODD-1, Risk R1) including a currently-green assertion.
- Document-delegated handlers per `board-dnd.js:67` so htmx-swapped cards need no re-wiring (NFR-6, FR-10).
- **ODD-5** (selection survival across an htmx swap: by issue-key vs index vs reset). **ODD-7** (a11y mechanism for non-focus selection — the explicit trade-off of rejecting roving tabindex, D-4).
- Depends on US-01 (dispatch) + US-02 (guards). Enables US-06 (`Enter`). No new route, no persistence.

---

## US-06: Press `Enter` to open the issue I have selected

`job_id: fast-keyboard-issue-flow`

### Elevator Pitch
- **Before**: even once `j`/`k` move a selection, `Enter` — "Open selected" in the help list — does **nothing**.
  Mei can walk to the right card and then must reach for the mouse to actually open it, which throws away the
  entire point of having walked there.
- **After**: Mei presses `j` twice to ring `AUTH-2`, presses `Enter`, and the issue edit modal for `AUTH-2`
  opens — the same modal a mouse click produces, because it triggers the card's own shipped
  `hx-get={edit_url}` → `#modal-root` (`issue_card.html:1`). `Esc` closes it and her selection is still on
  `AUTH-2`.
- **Decision enabled**: Mei can complete the whole loop — find, select, open, act, close — without ever leaving
  the keyboard, which is what makes the `j`/`k` selection worth having.

### Problem
`Enter` closes the loop that `j`/`k` opens. Without it, selection is a decoration: the user can move a
highlight but not act on it, and must reverse into the mouse at the final step — the most annoying possible
place to lose flow.

### Who
- A **keyboard-first maintainer** (Mei) with a card selected | wants to open it now | motivated by completing
  the keyboard loop rather than abandoning it at the last step.

### Solution
Bind `Enter` (guarded per US-02) to activate the **selected** card's shipped `hx-get={edit_url}` →
`#modal-root` (`issue_card.html:1`) — identical to a pointer click, so there is exactly one open path and no
divergence. With no selection, `Enter` does nothing (FR-9). `Enter` inside a form/text field submits as normal
(the US-02 text-input guard makes this automatic, not a special case).

### Domain Examples
1. **Happy path** — Mei presses `j` twice to select `AUTH-2`, then `Enter`; the edit modal for `AUTH-2` opens
   over the board — the same modal a click produces.
2. **Edge: no selection** — Mei loads the board and presses `Enter` without pressing `j` first; nothing happens
   (no modal, no navigation, no error) (FR-9).
3. **Boundary: `Enter` in a form still submits** — Mei has the new-issue modal open and presses `Enter` in the
   title field; the form submits normally and no card is opened underneath (US-02 guard, FR-2).
4. **Boundary: selection after close** — Mei opens `AUTH-2` with `Enter`, presses `Esc`; the modal closes and
   `AUTH-2` is still selected, so `j` moves on to the next card (US-07, FR-5).

### UAT Scenarios (BDD)
#### Scenario: Pressing enter opens the selected issue
Given Mei is viewing the AUTH board and has selected AUTH-2 with the "j" key
When Mei presses "Enter"
Then the issue modal for AUTH-2 opens over the board

#### Scenario: Enter with nothing selected does nothing
Given Mei is viewing the AUTH board and has not selected any card
When Mei presses "Enter"
Then no modal opens
And the browser does not navigate away

#### Scenario: Enter inside a form still submits the form
Given Mei has the new-issue modal open with a title typed into it
When Mei presses "Enter" in the title field
Then the form is submitted
And no issue card is opened behind the modal

#### Scenario: Closing the opened issue leaves the selection intact
Given Mei has opened AUTH-2 by pressing "Enter"
When Mei presses "Esc"
Then the modal closes
And AUTH-2 is still selected on the board

### Acceptance Criteria
- [ ] `Enter` opens the **selected** card via that card's shipped `hx-get={edit_url}` → `#modal-root` — the same result as a pointer click (FR-9).
- [ ] `Enter` with no selection does nothing — no modal, no navigation, no error (FR-9).
- [ ] `Enter` inside a text field/form submits normally and does not open a card (US-02 guard, FR-2).
- [ ] After `Esc` closes the opened modal, the selection is still on the same card (FR-5, US-07).
- [ ] There is exactly one open path — keyboard and pointer converge on the card's own `hx-get` (no divergent behaviour).
- [ ] The scenario **fails on unmodified `main`** — `Enter` today does nothing (NFR-1).

### Outcome KPIs
- **Who**: maintainers who have selected an issue with `j`/`k`
- **Does what**: open the selected issue from the keyboard, completing the find→select→open loop
- **By how much**: 100% of `Enter` presses with a selection open exactly that issue; **0** mouse actions needed to complete the loop
- **Measured by**: browser-level scenario (select → Enter → correct issue modal present)
- **Baseline**: 0% — `Enter` is unbound and there is no selection to open

### Technical Notes
- Reuses the card's shipped `hx-get={edit_url}` → `#modal-root` (`issue_card.html:1`) — the identical path the pointer click uses. **No new route.**
- Depends on US-05 (the selection it opens) + US-02 (guards). Pairs with US-07 (`Esc`).
- The `Enter`-in-a-form case is handled by the US-02 text-input guard rather than a special case — a design consequence of BR-2 worth noting.

---

## US-07: Press `Esc` to get out of anything, and land somewhere sane

`job_id: fast-keyboard-issue-flow`

### Elevator Pitch
- **Before**: `Esc` — "Close modal" in the help list — is **unbound**. Once Mei opens the new-issue modal she is
  in it: the only way out is to find and click a close control (or the browser's back button, which leaves the
  board entirely). The one key every user reflexively reaches for to escape does nothing.
- **After**: Mei presses `Esc` and the topmost layer closes — the help overlay, the new-issue modal, or search —
  one layer per press, landing her on the page beneath with her **selection intact**. With nothing open, `Esc`
  is a harmless no-op; it never navigates her away by surprise.
- **Decision enabled**: Mei can commit to any keyboard action knowing she can always back out of it in one
  keystroke, which is what makes the other six shortcuts safe to press in the first place.

### Problem
Every other shortcut in this feature **opens something** (`c` a modal, `?` an overlay, `/` search, `Enter` an
issue). Without `Esc`, each one is a one-way door. An escape hatch is what converts "I might press `c` by
accident" from a problem into a non-event — and `Esc` is also the key users press hardest when they feel stuck,
so doing nothing is the worst possible response.

### Who
- A **keyboard-first maintainer** (Mei) with a modal/overlay/search open | wants out, now | motivated by
  reversibility — the confidence to use the other shortcuts at all.

### Solution
Bind `Esc` globally (BR-3) to close the **topmost** open layer only, one per press (BR-4): help overlay
(US-01), new-issue modal (US-03), issue modal (US-06), or search (US-04). Restore the page beneath with the
**selection state intact** (FR-5, BR-5) — `Esc` must not silently clear the selection Mei worked to place. With
nothing open, `Esc` is a **no-op** and never navigates.

### Domain Examples
1. **Happy path** — Mei has the new-issue modal open (from `c`) and presses `Esc`; the modal closes and she is
   back on the AUTH board with nothing else changed.
2. **Edge: layered** — Mei has the new-issue modal open and presses `?` to check a shortcut, then `Esc`; the
   **help overlay** closes and the new-issue modal is **still open**; a second `Esc` closes the modal (BR-4).
3. **Boundary: nothing open** — Mei presses `Esc` on the board with nothing open; nothing happens — no
   navigation, no cleared selection, no error (FR-5).
4. **Boundary: selection survives** — Mei selects `AUTH-2` (`j`), opens it (`Enter`), presses `Esc`; the modal
   closes and `AUTH-2` is **still selected**, so `j` continues from there (FR-5, US-06).
5. **Boundary: search** — Mei presses `/`, types a query, then `Esc`; search closes and the board is restored.

### UAT Scenarios (BDD)
#### Scenario: Escape closes the new-issue modal and returns to the board
Given Mei has opened the new-issue modal by pressing "c"
When Mei presses "Esc"
Then the modal closes
And Mei is back on the AUTH board with nothing else changed

#### Scenario: Escape closes one layer at a time
Given Mei has the new-issue modal open and has pressed "?" to show the help overlay
When Mei presses "Esc"
Then the help overlay closes
And the new-issue modal is still open

#### Scenario: Escape with nothing open does nothing
Given Mei is viewing the AUTH board with no modal or overlay open
When Mei presses "Esc"
Then nothing happens
And the browser does not navigate away

#### Scenario: Escape does not throw away the selection
Given Mei has selected AUTH-2 and opened it by pressing "Enter"
When Mei presses "Esc"
Then the modal closes
And AUTH-2 is still selected so "j" moves to the next card

#### Scenario: Escape leaves search and restores the board
Given Mei has focused search by pressing "/" and typed a query
When Mei presses "Esc"
Then search closes and the board is restored

### Acceptance Criteria
- [ ] `Esc` closes the **topmost** open layer (help overlay, new-issue modal, issue modal, or search), one layer per press (FR-5, BR-4).
- [ ] `Esc` with nothing open is a harmless no-op — no navigation, no error (FR-5).
- [ ] `Esc` restores the page beneath with the **selection intact** — it never silently clears selection (FR-5, BR-5).
- [ ] `Esc` is global — it works on any signed-in page, not only the board (BR-3).
- [ ] `Esc` never navigates the browser away from the current page.
- [ ] The scenario **fails on unmodified `main`** — `Esc` today does nothing (NFR-1).

### Outcome KPIs
- **Who**: maintainers with a modal, overlay, or search open
- **Does what**: dismiss the topmost layer from the keyboard and land on the page beneath with selection intact
- **By how much**: 100% of `Esc` presses close exactly one layer; **0** unintended navigations; **0** selections silently cleared
- **Measured by**: browser-level scenarios (layered close, no-op case, selection-survives case), revert-reds-it
- **Baseline**: 0% — `Esc` is unbound; modals have no keyboard exit

### Technical Notes
- Global scope (BR-3) shares the mount-point question with US-01 — `#modal-root` is `board.html:13` only, absent from `app_shell.html` (**ODD-3**).
- Layer precedence (BR-4) implies the layer tracks what is open; interaction with htmx swaps that replace `#modal-root` content is **ODD-5**.
- Depends on US-01 (dispatch layer). Pairs with US-03/US-04/US-06 (the layers it closes). No new route, no persistence.
- `Esc` completes the advertised seven — with this story the bound set equals `SHORTCUTS` exactly (BR-1, FR-1).
