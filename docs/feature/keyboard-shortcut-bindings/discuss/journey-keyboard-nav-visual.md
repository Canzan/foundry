# Journey (visual): Keyboard Navigation — the seven advertised shortcuts, actually bound

> Feature: `keyboard-shortcut-bindings` | Personas: **Mei Tanaka** (`mei@acme.com`, the keyboard-first
> maintainer) and **Hiroshi Sato** (`hiroshi@acme.com`) — the repo's own acceptance-harness identities
> (`us_12_keyboard_nav.rs:58-64`), reused so every example is concrete and runnable.
> Goal: make the **seven shortcuts the help overlay already advertises** (`c`, `/`, `j`, `k`, `Enter`, `?`,
> `Esc` — `keyboard.rs:48-56`) actually work in the browser, guarded so they never steal a keystroke, without
> adding a single server route.
> Scope: the **client layer only**. `c`/`/`/`j`/`k`/`Enter` on the board + search results; `?`/`Esc` global.
> **No walking skeleton** (D-8) — brownfield; the server contracts are shipped and routed.

## The shape of this feature: a green suite over an absent feature

This is not a normal brownfield extension. The usual asymmetry is inverted:

```
   WHAT THE TEST SUITE SEES              WHAT MEI SEES
   ------------------------              -------------
   GET /keyboard-help      -> 200 <dl>   presses ?      -> nothing
   GET .../issues/new      -> modal      presses c      -> nothing
   GET .../search?q=x      -> results    presses /      -> nothing
   #kb-items ASC carrier   -> present    presses j/k    -> nothing
   6 scenarios GREEN                     presses Enter  -> nothing
                                         presses Esc    -> nothing
                                         ---------------------------
                                         7 of 7 advertised: ABSENT
```

The acceptance suite is **port-to-port**: it drives HTTP and parses HTML with `scraper`. It asserts the *server
contracts the client would call* — and it **never presses a key**, because nothing in the harness can. So
`GET /keyboard-help` returning a well-formed `<dl>` is green while `?` does nothing at all.

Hence this feature's hard rule (**NFR-1**): **every AC asserts browser-observable behaviour** — key pressed →
thing happens. Any scenario that a port-to-port test could satisfy on unmodified `main` is, by construction,
not an AC for this feature. Every new scenario must **fail on `main` today**.

## Why this is client-only: everything else is already shipped

Verified by grep — the only `keydown` match in the whole app is inside the **vendored** `alpine.min.js`; there
are **zero** application handlers. `static/js/` contains exactly `board-dnd.js` and `csrf-upload.js`. There is
no `keyboard.js`. Meanwhile:

- `SHORTCUTS` (`keyboard.rs:48-56`) — the seven-entry contract, with an acceptance test asserting completeness.
- `GET /keyboard-help` (`keyboard.rs:259`, route `lib.rs:536`) — **public by design** (the doc says the
  bootstrap GETs it once and caches it, so a session gate would break help on the sign-in page); returns a bare
  `section.keyboard-help[role="dialog"]` built for exactly the overlay we need.
- `GET …/issues/new` (`keyboard.rs:62`, route `lib.rs:491-494`) — bare htmx fragment on `HX-Request: true`,
  full-page no-JS fallback otherwise; CSRF cookie already minted server-side (`:94`).
- `GET …/search?q=` (`keyboard.rs:160`, route `lib.rs:495-498`) — exact-key + substring matching, with a real
  `ul.search-results[data-empty="true"]` empty state.
- `issue_card.html:1` — every card already carries `hx-get={edit_url}` → `#modal-root`. `Enter` has a target.
- `board-dnd.js:67` — the house idiom: a vanilla IIFE with **`document`-delegated** listeners *"so htmx-appended
  cards are draggable without re-wiring"*. That is precisely the pattern the keyboard layer needs (NFR-6).

**This feature adds zero routes and zero migrations.** The gap is the keypress that reaches them.

## The `#kb-items` collision — the one genuinely hard thing here

```
  SHIPPED (board.html:12, asserted green)      LOCKED MODEL (D-4)
  --------------------------------------      ------------------
  <ul id="kb-items" hidden aria-hidden>        walk VISIBLE .issue-card
    AUTH-1  <- ASC by number                     ring highlight
    AUTH-2     across ALL columns                scrollIntoView
    AUTH-3                                       selection follows the eyes
  </ul>
        |                                              |
        +--------------- CANNOT BOTH HOLD -------------+

  visible board = column-grouped, DESC-within-column (projects.rs:864-885)
  #kb-items     = flat, ASC-by-number             (projects.rs:881-891)
              ^ different ORDER, and hidden ^
```

Three ways they conflict:

1. **Order differs.** Walking one is observably not walking the other.
2. **A hidden element cannot be highlighted or scrolled to.** `#kb-items` is `hidden`; a ring on it renders
   nothing, `scrollIntoView` does nothing. The locked model *requires* the visible card.
3. **`aria-hidden="true"`** makes it invisible to assistive tech, so it cannot carry the a11y story either.

The acceptance suite calls the carrier *"the source of truth for the keyboard navigation order"*
(`us_12_keyboard_nav.rs:339-341`). Honouring **D-4 retires it** — and, per `AGENTS.md` (*"Remove dead/legacy
code outright — do not leave it inert"*), that means **deleting** the carrier (`board.html:12`), its builder
(`projects.rs:881-891`), its view-model field (`views.rs:256`), its unit tests (`projects.rs:1039-1110`), and
**two currently-green acceptance assertions** (`us_12_keyboard_nav.rs:334-360`,
`feature_b_web_tier.rs:568-572`).

> **This is a decision, not an accident.** It deletes a passing test on purpose, because that test pins a
> contract the locked UX contradicts. It is **ODD-1** and **Risk R1**, and DESIGN owns it.

## The personas, concretely

**Mei Tanaka** (`mei@acme.com`) is a maintainer who lives on the AUTH board with both hands on the home row.
She read Foundry's help page, learned there are seven shortcuts, pressed `c`, and **nothing happened**. She now
half-suspects the whole help page is decorative. She uses a Japanese IME, which makes the text-input guard's
composition handling (ODD-4) a real concern for her rather than a theoretical one.

**Hiroshi Sato** (`hiroshi@acme.com`) is her teammate, a mouse user who occasionally drags cards between
columns. He is the reason the keyboard layer must not break drag-and-drop (NFR-8) and the reason nothing may
become keyboard-only (BR-6).

## Emotional arc — Problem Relief → Flow (with one sharp cliff)

```
BETRAYED                DISTRUSTFUL             RELIEVED                IN FLOW
"the help page     -->  "ok, ? worked...   -->  "c files, / finds, -->  "hands never leave
 lists seven             but will c fire         j/k walk, Enter        the home row.
 shortcuts. I            while I'm typing        opens, Esc backs       This is what the
 pressed c.              a title?"               out. It's real."       help page promised."
 Nothing."
 betrayed / suspicious   wary (THE CLIFF)        relieved               fluent / fast
```

Mei's arc does **not** start anxious — it starts **betrayed**. The product already made her a promise in
writing and broke it. That changes the design job: the first thing shipped must **re-establish credibility**,
which is why slice 01 binds `?` (the key whose whole job is to display the promise) — the promise and its
fulfilment become the same keystroke.

Her peak tension is **the cliff at US-02**: the first time she types a title with a `c` in it. If a shortcut
fires there, the layer is worse than nothing and she will want it removed — a shortcut layer that eats
keystrokes is a strictly worse product than one that does nothing. This is why the text-input guard is not a
detail but the **highest-risk requirement** (NFR-2, Risk R2), and why it ships as its own slice **immediately
after** the layer exists and **before** `c` is ever bound (slice 02 precedes slice 03 deliberately).

The sad paths stay **calm and silent**: a shortcut on the wrong surface does nothing (no error toast), `Esc`
with nothing open does nothing, `Enter` with no selection does nothing. Silence is correct here — the user is
mid-flow and an error message would be a worse interruption than the no-op.

---

## Capability 1 — The guarded dispatch layer: `?` help + `Esc`, and the guards that make it safe

```
[Step K1: DISPATCH]        [Step K2: GUARD]           [Step K3: HELP (?)]        [Step K4: ESCAPE (Esc)]
document-delegated    -->  typing? modifier?     -->  fetch /keyboard-help  -->  close topmost layer,
keydown; bound set         -> INERT                   render as OVERLAY          selection intact
== SHORTCUTS               else -> dispatch           (no navigation)            nothing open -> no-op
  Feels: betrayed            Feels: wary->safe          Feels: credibility         Feels: reversible
  Artifacts:                 Artifacts:                 restored                   Artifacts:
   ${shortcut_set}            ${guard_verdict}          Artifacts:                  ${layer_stack}
                                                        ${help_fragment},
                                                        ${modal_mount}
```

### Step K1 — The dispatch layer (`document`-delegated, bound set == `SHORTCUTS`)

```
+-- base.html:6-9 (vendored, defer, CSP-safe) ---------------------+
|  <script src="/static/vendor/htmx.min.js" defer></script>        |
|  <script src="/static/vendor/alpine.min.js" defer></script>      |
|  <script src="/static/js/board-dnd.js" defer></script>           |
|  <script src="/static/js/csrf-upload.js" defer></script>         |
|  <script src="/static/js/keyboard.js" defer></script>  <-- NEW   |
+------------------------------------------------------------------+
   document.addEventListener("keydown", ...)   <-- delegated (NFR-6)
   bound set MUST equal SHORTCUTS (keyboard.rs:48-56)  (BR-1)
   vanilla IIFE vs Alpine x-on:keydown  ->  ODD-2
```

One delegated `keydown` at the document level — the `board-dnd.js:67` idiom — so htmx-swapped fragments need no
re-wiring (FR-10, NFR-6). The bound set is exactly `${shortcut_set}` (BR-1). **ODD-2**: `keyboard.rs`'s doc says
*"the alpine.js keyboard-shortcut handlers"*, but the house pattern (`board-dnd.js`, `csrf-upload.js`) is
**vanilla**; Alpine is vendored and loaded yet unused by app code. Whichever DESIGN picks, the stale doc comment
must be corrected in the same change (Risk R9).

### Step K2 — The guards (evaluated BEFORE any dispatch — the cliff)

```
+-- guard chain (BR-2: no shortcut is exempt) ---------------------+
|  target is input / textarea / contenteditable / … ?              |
|      -> INERT. the character is typed. (FR-2, NFR-2)             |
|  Ctrl / Cmd(Meta) / Alt held ?                                   |
|      -> INERT. Cmd+C copies. (FR-3, NFR-3)                       |
|  Shift held ?                                                    |
|      -> NOT a suppressor — ? IS Shift+/ (BR-7)                   |
|  IME composing (isComposing) ?    -> ODD-4                       |
|  else -> dispatch                                                |
+------------------------------------------------------------------+
   Mei types "cache invalidation on login"  ->  exactly that string
   Mei types "cjk/?"                        ->  exactly "cjk/?"
   Mei presses Cmd+C                        ->  copies, no modal
```

**The highest-risk detail in the feature.** Every one of the seven is a plain printable character or bare key —
exactly what people type. Without this guard, binding them makes text entry impossible: Mei cannot type the
letter `c` into a title. That is **strictly worse than shipping nothing** (NFR-2, Risk R2). The guard is
**structural** (BR-2) — a chain evaluated before dispatch, not seven scattered `if`s. `${guard_verdict}` is the
single decision every shortcut passes through. **ODD-4** pins the exact predicate, including `isComposing` —
concretely relevant to Mei's Japanese IME.

### Step K3 — `?` renders the shipped help as an OVERLAY (not a navigation)

```
+-- ? (Shift+/) -> GET /keyboard-help (public) --------------------+
|  +-- section.keyboard-help[role=dialog] ---------------+         |
|  |  Keyboard shortcuts                                 |         |
|  |    c ....... Create issue      ? ..... Show this help|        |
|  |    / ....... Search            Esc ... Close modal  |         |
|  |    j ....... Next                                   |         |
|  |    k ....... Previous                               |         |
|  |    Enter ... Open selected                          |         |
|  +-----------------------------------------------------+         |
|  ...the AUTH board, still visible behind...                      |
+------------------------------------------------------------------+
   URL UNCHANGED — overlay, not navigation (FR-4)
   the <dl> IS the shipped fragment (keyboard_help.html:1) — no new route
```

The route is **public by design** (`keyboard.rs:19-24`) so help works even on the sign-in page. `${help_fragment}`
is rendered from `SHORTCUTS`, so the overlay Mei reads and the keys that are bound are **the same list** — the
promise and its fulfilment can't drift (BR-1). The sidebar/dashboard full-page links (`sidebar.html:13`,
`dashboard_root.html:32`) stay as the no-JS path (**ODD-8**, NFR-4).

> **`?` is global — and there is nowhere to put it.** `#modal-root` exists **only** at `board.html:13`;
> `app_shell.html` (7 lines: base + sidebar + `app_content`) has **none**. So a global `?` has no mount point on
> the dashboard or any non-board page. **ODD-3**, Risk R5 — surfaced precisely *because* "global `?`" is locked.

### Step K4 — `Esc` closes the topmost layer, selection intact

```
+-- Esc: one layer per press (BR-4) -------------------------------+
|  [help overlay]                  <- Esc closes THIS only         |
|  [new-issue modal]               <- still open; 2nd Esc closes it|
|  [the board + ring on AUTH-2]    <- selection SURVIVES (BR-5)    |
|                                                                  |
|  nothing open -> NO-OP. never a navigation. (FR-5)               |
+------------------------------------------------------------------+
```

`Esc` is what makes the other six safe to press: every one of them **opens** something, so without an escape
hatch each is a one-way door. `${layer_stack}` tracks what's open so precedence is deterministic (BR-4). `Esc`
must **not** silently clear the selection Mei worked to place (FR-5, BR-5) — and with nothing open it must do
**nothing**, never navigate.

---

## Capability 2 — Acting on issues: `c` file, `/` find

```
[Step A1: CREATE (c)]                      [Step A2: SEARCH (/)]
c -> GET .../issues/new (HX-Request)  -->  / -> focus search, SUPPRESS the "/"
     -> bare fragment -> #modal-root        -> GET .../search?q= -> ul.search-results
     title autofocused                      exact-key | substring | data-empty
  Feels: capture at the speed of thought    Feels: found it without the mouse
  Artifacts: ${modal_mount},                Artifacts: ${search_results}
             ${project_context}
```

### Step A1 — `c` opens the shipped new-issue modal

```
+-- c (on a board) -> GET /team/acme/project/auth/issues/new ------+
|   HX-Request: true  ->  BARE fragment (keyboard.rs:96-101)       |
|  +-- [data-modal=new-issue][role=dialog][aria-modal] ---+        |
|  |  New issue in AUTH                                   |        |
|  |  Title: [Session cookie not cleared on sign-out|]    |        |
|  |         ^ input[name=title][autofocus]               |        |
|  |  <input type=hidden name=_csrf value=...>            |        |
|  |  action="/team/acme/project/auth/issues"             |        |
|  +------------------------------------------------------+        |
+------------------------------------------------------------------+
   NO HX-Request -> full page fallback (keyboard.rs:102-104) — NO-JS INTACT
   same hx-get the "New issue" button already uses (board.html:6)
```

`c` triggers the **identical path** the shipped button uses — one open path, no divergence. CSRF is already
minted server-side (`ensure_csrf_cookie`, `keyboard.rs:94`), so unlike `board-dnd.js` the client needs **no**
CSRF work. The `HX-Request` fork **is** the no-JS guarantee (NFR-4, BR-6): the full-page fallback is untouched.
`c` needs `${project_context}` (a team+project) — on the dashboard it does **nothing**, silently (BR-3, ODD-6).

### Step A2 — `/` focuses search without typing a slash

```
+-- / -> focus search input, preventDefault the "/" ---------------+
|  Search: [session|]        <- FOCUSED, and EMPTY (not "/")       |
|  +-- ul.search-results -------------------------------+          |
|  |  li.search-result[data-issue-key="AUTH-2"]         |          |
|  |     AUTH-2  Session cookie not cleared on sign-out |          |
|  +-----------------------------------------------------+         |
|  no match -> ul.search-results[data-empty="true"]                |
+------------------------------------------------------------------+
   exact key "AUTH-2" -> that issue only  (keyboard.rs:217-224)
   substring "session" -> case-insensitive title match  (:226-231)
```

The classic bug this must not ship: focusing the field **and** typing "/" into it, so Mei's first search is
always for `/session`. `${search_results}` is the shipped fragment — with a real empty state, so "nothing
matched" is distinguishable from "search is broken". The results list is the **second issue-key-bearing
surface**, so `j`/`k`/`Enter` apply to it too (ODD-6).

---

## Capability 3 — Moving through issues: `j`/`k` select, `Enter` open

```
[Step M1: SELECT (j/k)]                     [Step M2: OPEN (Enter)]
walk VISIBLE .issue-card in DOM order  -->  activate selected card's shipped
ring highlight + scrollIntoView              hx-get={edit_url} -> #modal-root
resets on navigation; never persisted        no selection -> no-op
  Feels: I can see where I am                Feels: the loop closes
  Artifacts: ${selection}, ${visible_cards}  Artifacts: ${modal_mount}
```

### Step M1 — `j`/`k` walk the cards Mei can SEE

```
+-- the AUTH board (what Mei sees) --------------------------------+
|  [ Todo ]              [ In Progress ]     [ Done ]              |
|  +==================+  +---------------+   +--------------+      |
|  ‖ AUTH-3  Login... ‖  | AUTH-1  CSRF..|   | AUTH-4  Docs |      |
|  +==================+  +---------------+   +--------------+      |
|   ^^^ RING = selected (j)                                        |
|  +------------------+                                            |
|  | AUTH-2  Session..|  <- j again selects THIS (next VISIBLE)    |
|  +------------------+                                            |
+------------------------------------------------------------------+
   order = what she sees (column-grouped, DESC-within-column)
   NOT #kb-items' flat ASC order  ->  the carrier is RETIRED (ODD-1)
   below the fold -> scrollIntoView; k at the top -> stays (no wrap)
```

`${selection}` is client-only and ephemeral (BR-5) — never persisted, never sent to the server, **reset on
navigation**. It walks `${visible_cards}` (`article.issue-card`, `issue_card.html:1`).

> **The a11y debt, named out loud.** A ring is **not native focus** — roving tabindex was rejected knowingly
> (D-4). So nothing announces the selection to a screen reader **for free**. NFR-7 refuses to let that be
> silence: the ring must meet WCAG 2.1 AA contrast and not rely on colour alone, **and** selection changes must
> be conveyed by an explicit mechanism (`aria-activedescendant` or equivalent). **ODD-7** forces DESIGN to
> answer it (Risk R3) rather than inherit it.

### Step M2 — `Enter` opens the selected card

```
+-- Enter -> the SELECTED card's own hx-get ------------------------+
|  article.issue-card[data-issue-key="AUTH-2"]                      |
|      hx-get="/team/acme/project/auth/issues/2/edit"               |
|      hx-target="#modal-root"  hx-swap="innerHTML"                 |
|          |                                                        |
|          v   identical to a POINTER CLICK — one open path         |
|  +-- issue modal for AUTH-2 -------------+                        |
|  |  Session cookie not cleared on sign-out|                       |
|  +----------------------------------------+                       |
+-------------------------------------------------------------------+
   no selection -> NO-OP (FR-9)
   Enter in a form -> submits normally (the US-02 guard, not a special case)
```

`Enter` closes the loop `j`/`k` opens — without it, selection is decoration. It reuses the card's **own shipped
`hx-get`**, so keyboard and pointer converge on one path and cannot diverge.

---

## Sad / error paths — first-class (and deliberately silent)

### The guard cliff (the crux — worse-than-nothing if wrong)

```
+-- Mei types into the new-issue title ----------------------------+
|  "cache invalidation on login"                                   |
|     ^ WITHOUT the guard: 'c' opens a 2nd modal, rest scatter     |
|     ^ WITH the guard:    exactly "cache invalidation on login"   |
|  "cjk/?"  -> exactly "cjk/?"   (the NFR-2 litmus)                |
|  Cmd+C    -> copies. does NOT file an issue.                     |
+------------------------------------------------------------------+
```

| # | Sad path | Trigger | What Mei sees | Handling |
|---|----------|---------|---------------|----------|
| K-E1 | **Shortcut fires while typing** | `c` in a title field | must be impossible | FR-2 + NFR-2; guard before dispatch (BR-2); `@property` litmus reds on regression (R2) |
| K-E2 | **Modifier hijack** | `Cmd+C` to copy | copies; no modal | FR-3 + NFR-3; `Ctrl`/`Cmd`/`Alt` suppress; `Shift` does **not** (BR-7, R6) |
| K-E3 | **IME composition** | Mei composes Japanese text | characters compose normally | ODD-4 (`isComposing` in the guard predicate) |
| K-E4 | **Guard over-fires** | Mei leaves the field, presses `c` | modal opens | guard is contextual, not a global toggle |

### Surface + selection sad paths (silence is the correct response)

| # | Sad path | Trigger | What Mei sees | Handling |
|---|----------|---------|---------------|----------|
| A-E1 | **`c` with no project** | `c` on the dashboard | **nothing** — no error, no navigation | route needs team+project (`keyboard.rs:62-95`); BR-3, ODD-6 |
| A-E2 | **`/` types a slash** | naive focus binding | field focused and **empty** | suppress default on the focusing keypress (FR-7) |
| A-E3 | **Search matches nothing** | query `zzz` | shipped empty state `[data-empty=true]` | distinguishable from "no query" (`keyboard.rs:213-215`) |
| M-E1 | **`Enter` with no selection** | `Enter` on a fresh board | **nothing** | FR-9 no-op |
| M-E2 | **`k` at the first card** | walking off the top | stays on the first card | bounded, no wrap (FR-8) |
| M-E3 | **`j` on an empty board** | no cards (`board.html:9` empty state) | nothing; no error | FR-8 |
| M-E4 | **Stale ring after a drag** | Hiroshi drags the selected card | drag works; selection coherent | NFR-8 + ODD-5 (R4) |
| M-E5 | **Selection lost across an htmx swap** | a card is edited/created | handlers survive (delegation) | FR-10, NFR-6; selection *survival* is ODD-5 |
| E-E1 | **`Esc` with nothing open** | reflex press | **nothing** — never a navigation | FR-5 |
| E-E2 | **`Esc` clears selection** | closing a modal | selection **intact** | FR-5 + BR-5 |
| E-E3 | **`?` on a page with no mount** | `?` on the dashboard | overlay still appears | ODD-3 (`#modal-root` is board-only) — R5 |

> All sad paths share one design intent: **the layer is inert when it should be, silent when it does nothing,
> and never steals a keystroke.** No error toasts — the user is mid-flow, and an error message would be a worse
> interruption than the no-op.

---

## Integration checkpoints

1. **Bound set == advertised set**: the keys bound in the client equal `SHORTCUTS` (`keyboard.rs:48-56`) —
   the same constant that renders the help overlay Mei reads. Promise and fulfilment cannot drift (BR-1, FR-1).
2. **Guard before dispatch**: `${guard_verdict}` is evaluated for **every** shortcut with no exemptions (BR-2).
   A litmus reds if any shortcut fires from a text-entry context or under a `Ctrl`/`Cmd`/`Alt` chord (NFR-2/3).
3. **Selection ↔ visible cards**: `${selection}` walks `${visible_cards}` in DOM order — the order Mei sees.
   The hidden `#kb-items` carrier is **retired and deleted** with its assertions (ODD-1). A litmus must red if
   selection order diverges from visible order.
4. **Selection → open**: `Enter` activates the **selected** card's own shipped `hx-get` — the identical path a
   pointer click uses. One open path; keyboard and mouse cannot diverge.
5. **Layer stack → Esc**: `${layer_stack}` determines precedence; `Esc` closes exactly one layer per press
   (BR-4) and never clears `${selection}` (BR-5) or navigates.
6. **Handlers survive swaps**: bindings are `document`-delegated (the `board-dnd.js:67` idiom), so an htmx swap
   that replaces `#modal-root` or re-renders cards needs no re-wiring (FR-10, NFR-6). Selection *survival* across
   a swap is ODD-5.
7. **No-JS intact**: the server-side `HX-Request` fork (`keyboard.rs:96-104`) is untouched; with scripting off,
   the full-page new-issue form and the sidebar help link behave exactly as today (NFR-4, BR-6).
8. **Zero server delta**: no route, endpoint, or migration is added. The only server-side change is the
   **removal** of the retired `#kb-items` builder (ODD-1) and the correction of `keyboard.rs`'s stale
   "alpine.js" doc comment (ODD-2, R9).
9. **Every scenario reds on `main`**: each new acceptance scenario **fails today** (NFR-1). One that passes
   unchanged is proof it tests the server contract, not the feature.

## Web / a11y parity note

This feature is **pure progressive enhancement** on top of surfaces that already work with a pointer and
without JS (BR-6, NFR-4). Nothing becomes keyboard-only — Hiroshi's mouse keeps working, drag-and-drop is
untouched (NFR-8), and the no-JS full-page fallbacks are unchanged. The one genuine a11y **debt** is deliberate
and named: ring-highlight selection is not native focus (D-4 rejected roving tabindex), so NFR-7 + ODD-7 require
DESIGN to specify how selection reaches assistive tech rather than let it default to silence. Mei dogfoods this
by working a real AUTH board for an afternoon without touching the mouse — and by typing a title containing the
letter `c`.
