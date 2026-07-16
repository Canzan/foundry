# Acceptance Criteria — keyboard-shortcut-bindings

Every AC below is derived from a UAT scenario in `user-stories.md` and traced to an FR/NFR/BR in
`requirements.md`.

## The governing rule (NFR-1): browser-observable or it isn't an AC

The shipped acceptance suite (`crates/foundry-acceptance/src/steps/us_12_keyboard_nav.rs`) is **port-to-port**:
it drives HTTP and parses HTML with `scraper`, asserting the *server contracts* the client would call. It
**never presses a key**, because nothing in the harness can. That is why `GET /keyboard-help` is green while
pressing `?` does nothing.

So every AC here asserts **key-pressed → user-observable outcome**, and:

> **Every AC in this feature MUST fail on unmodified `main`.**
> An AC that passes on `main` is testing the shipped server contract, not this feature. Reject it on sight.

**One deliberate exception, and it is the dangerous one: US-02 (the guards).** "Typing `c` opens no modal" is
**trivially true on `main`** — no handler exists, so nothing fires. Guard ACs are therefore **revert-reds-it
regression guards**, not reds-on-`main` ACs, and they must be written as **paired** assertions (the shortcut
fires *outside* the field **and** not *inside* it) so they cannot pass vacuously against a build that binds
without guarding. See the US-02 section below — this is the one place the governing rule inverts, and getting it
wrong would let the feature's highest-risk regression through the gate.

This has a tooling consequence DISTILL must resolve (**ODD-9**): the existing reqwest+scraper harness **cannot
express these ACs at all**. A browser-capable driver is required. That gap **is** the feature — it is precisely
why the layer was never noticed missing.

---

## US-01 — Press `?` and see the shortcut list, right where I am

| # | Acceptance Criterion | Source scenario | Traces to |
|---|---------------------|-----------------|-----------|
| AC-01.1 | Pressing `?` on a signed-in page renders the `/keyboard-help` fragment as an **overlay over the current page**, with no navigation | "Pressing the help key shows the shortcut list over the current page" | FR-4 |
| AC-01.2 | The overlay lists exactly the seven entries in `SHORTCUTS` — the bound set equals the rendered set | "The help overlay lists every advertised shortcut" | BR-1, FR-1 |
| AC-01.3 | `?` works on a page without a board (global scope); the mount point is resolved per ODD-3 | "The help overlay is available away from the board" | BR-3, FR-4 |
| AC-01.4 | `Esc` closes the help overlay and restores the underlying page unchanged | "Dismissing the help returns Mei exactly where she was" | FR-5, BR-4 |
| AC-01.5 | The sidebar/dashboard full-page `/keyboard-help` links still work with scripting disabled | (no-JS property) | NFR-4, ODD-8 |
| AC-01.6 | The scenario **fails on unmodified `main`** — `?` today does nothing | (governing rule) | NFR-1 |

## US-02 — Type the letter "c" into a title without filing a new issue

> **US-02 is the one story where the "must fail on `main`" rule does NOT apply — and that is a trap.**
> On unmodified `main`, "typing `c` opens no modal" is **trivially true for the wrong reason**: no handler
> exists, so nothing fires. A naively-written guard AC would therefore **pass on `main`**, pass after the guard
> ships, and — fatally — **also pass if someone ships the binding layer and forgets the guard**, because it
> would only be exercised on a build where `c` was never bound.
>
> So US-02's ACs are **revert-reds-it regression guards, not reds-on-`main` ACs**. Each is stated as a
> conditional whose precondition is a **live binding**: *given `c` is bound and fires outside a text field*,
> typing `c` inside one must still insert the character. That formulation is **vacuously true on `main`**
> (the precondition is false) and **genuinely false** on a build that binds without guarding — which is the
> exact regression NFR-2 exists to catch. The litmus must therefore run on a build where the layer is live and
> **red when the guard is removed**, not when the layer is absent.

| # | Acceptance Criterion | Source scenario | Traces to |
|---|---------------------|-----------------|-----------|
| AC-02.1 | **Given the layer is live** (`c` demonstrably opens the modal outside a text field), typing `c`/`j`/`k`/`/`/`?` **into** a text-entry context (`input`, `textarea`, `contenteditable`, or equivalent) inserts the literal characters and fires nothing. **Paired precondition assertion required**: the same scenario must first prove the shortcut *does* fire outside the field, or it is vacuous | "Typing shortcut letters into a title inserts them instead of firing shortcuts" + "Shortcuts work again once Mei leaves the text field" | FR-2, NFR-2 |
| AC-02.2 | **Given the layer is live**, typing the literal `"cjk/?"` into any field yields exactly `"cjk/?"` — no modal, no search focus, no selection change. **Reds when the guard is deleted from a bound layer** (revert-reds-it), NOT merely when the layer is absent | `@property` "No shortcut ever fires from a text-entry context" | NFR-2 |
| AC-02.3 | No shortcut fires while `Ctrl`, `Cmd`(Meta) or `Alt` is held; `Cmd+C`/`Ctrl+C` still copy | "A copy chord copies instead of creating an issue" | FR-3, NFR-3 |
| AC-02.4 | `Shift` is **not** treated as a suppressor — `?` (`Shift+/`) still fires outside a text field | (BR-7 boundary) | BR-7 |
| AC-02.5 | The guards are evaluated **before** dispatch and apply to **all seven** shortcuts with no exemptions | (structural) | BR-2 |
| AC-02.6 | Leaving the text field re-enables the shortcuts immediately — the guard is contextual, not a global toggle | "Shortcuts work again once Mei leaves the text field" | FR-2 |
| AC-02.7 | A regression that lets a shortcut fire during typing **reds a dedicated `@property` litmus** | `@property` litmus | NFR-2, R2 |

> **AC-02.2 is the single most important criterion in this feature.** A layer that eats keystrokes is strictly
> worse than shipping nothing at all.
>
> **Its pairing rule is what gives it teeth.** Every US-02 scenario must assert **both halves**: that the
> shortcut **does** fire outside the text field (the precondition — proving the layer is live) **and** that it
> **does not** fire inside one (the guard). A scenario asserting only the second half is vacuous on any build
> where the key isn't bound, which includes `main` and includes a broken half-shipped layer. **DISTILL must not
> split these halves into separate scenarios** — the pairing is the assertion.

## US-03 — Press `c` and file an issue without touching the mouse

| # | Acceptance Criterion | Source scenario | Traces to |
|---|---------------------|-----------------|-----------|
| AC-03.1 | Pressing `c` on a board opens the new-issue modal over the page via the shipped htmx fragment path, with the title field focused | "Pressing the create key opens the new-issue modal on the board" | FR-6 |
| AC-03.2 | The issue can be filed end-to-end from the keyboard — the created card appears on the board | "Mei files an issue entirely from the keyboard" | FR-6 |
| AC-03.3 | `c` does nothing on a page with no team+project context — no modal, no error, no navigation | "The create key does nothing where there is no project" | BR-3, ODD-6 |
| AC-03.4 | `c` never fires while typing or with a modifier held | (US-02 guards) | BR-2, FR-2, FR-3 |
| AC-03.5 | The no-JS full-page fallback (`keyboard.rs:102-104`) and the "New issue" button are unchanged | "Filing without a mouse leaves the no-JS path working" | NFR-4, BR-6, R7 |
| AC-03.6 | The scenario **fails on unmodified `main`** — `c` today does nothing | (governing rule) | NFR-1 |

## US-04 — Press `/` and search the board without reaching for the mouse

| # | Acceptance Criterion | Source scenario | Traces to |
|---|---------------------|-----------------|-----------|
| AC-04.1 | Pressing `/` on a board focuses the search input and the "/" character is **not** inserted into it | "Pressing the search key focuses the search box without typing a slash" | FR-7 |
| AC-04.2 | Typing a title substring lists matching issues from the shipped search fragment | "Mei finds an issue by typing part of its title" | FR-7 |
| AC-04.3 | An exact key (`AUTH-2`) returns exactly that issue | "Mei finds an issue by its exact key" | FR-7 |
| AC-04.4 | A query with no matches renders the shipped empty state (`ul.search-results[data-empty="true"]`), distinguishable from "no query" | "A search that matches nothing says so" | FR-7 |
| AC-04.5 | Once the search box is focused, shortcut characters typed into it are inserted literally | (US-02 guard) | FR-2 |
| AC-04.6 | `Esc` closes/leaves search and restores the board | "Escape leaves search and restores the board" | FR-5 |
| AC-04.7 | The scenario **fails on unmodified `main`** — `/` today does nothing | (governing rule) | NFR-1 |

## US-05 — Walk the board with `j` and `k` and see where I am

| # | Acceptance Criterion | Source scenario | Traces to |
|---|---------------------|-----------------|-----------|
| AC-05.1 | `j`/`k` move a selection to the next/previous **visible card in DOM order**; the order matches what the user sees | "Next and previous walk the cards in the order Mei sees them" | FR-8 |
| AC-05.2 | The selected card shows a **ring highlight** that is visible, meets WCAG 2.1 AA contrast, and does not rely on colour alone | "The next key selects the first visible card and highlights it" | FR-8, NFR-7 |
| AC-05.3 | A selection outside the viewport is **scrolled into view** | "A selection below the fold scrolls into view" | FR-8 |
| AC-05.4 | Selection is bounded (no wrap past first/last unless DESIGN chooses otherwise) and is a no-op on an empty board | "Moving previous from the first card stays put" | FR-8 |
| AC-05.5 | Selection **resets on navigation** and is never persisted or sent to the server | (BR-5 structural) | BR-5 |
| AC-05.6 | The hidden `#kb-items` carrier is **retired and removed** — carrier, builder, view-model field, unit tests, and its two acceptance assertions | (ODD-1 consequence) | ODD-1, R1, `AGENTS.md` |
| AC-05.7 | Selection changes are conveyed to assistive technology by an explicit mechanism — not left to chance | "Moving the selection is announced to assistive technology" | NFR-7, ODD-7, R3 |
| AC-05.8 | Drag-and-drop keeps working and selection stays coherent across a drag and across an htmx swap | "Dragging a card with the mouse leaves selection coherent" | NFR-6, NFR-8, ODD-5, R4 |
| AC-05.9 | The scenarios **fail on unmodified `main`** — `j`/`k` today do nothing | (governing rule) | NFR-1 |

## US-06 — Press `Enter` to open the issue I have selected

| # | Acceptance Criterion | Source scenario | Traces to |
|---|---------------------|-----------------|-----------|
| AC-06.1 | `Enter` opens the **selected** card via that card's shipped `hx-get={edit_url}` → `#modal-root` — the same result as a pointer click | "Pressing enter opens the selected issue" | FR-9 |
| AC-06.2 | `Enter` with no selection does nothing — no modal, no navigation, no error | "Enter with nothing selected does nothing" | FR-9 |
| AC-06.3 | `Enter` inside a text field/form submits normally and does not open a card | "Enter inside a form still submits the form" | FR-2, BR-2 |
| AC-06.4 | After `Esc` closes the opened modal, the selection is still on the same card | "Closing the opened issue leaves the selection intact" | FR-5, BR-5 |
| AC-06.5 | There is exactly one open path — keyboard and pointer converge on the card's own `hx-get` | (structural) | FR-9 |
| AC-06.6 | The scenario **fails on unmodified `main`** — `Enter` today does nothing | (governing rule) | NFR-1 |

## US-07 — Press `Esc` to get out of anything, and land somewhere sane

| # | Acceptance Criterion | Source scenario | Traces to |
|---|---------------------|-----------------|-----------|
| AC-07.1 | `Esc` closes the **topmost** open layer (help overlay, new-issue modal, issue modal, or search), one layer per press | "Escape closes one layer at a time" | FR-5, BR-4 |
| AC-07.2 | `Esc` with nothing open is a harmless no-op — no navigation, no error | "Escape with nothing open does nothing" | FR-5 |
| AC-07.3 | `Esc` restores the page beneath with the **selection intact** — it never silently clears selection | "Escape does not throw away the selection" | FR-5, BR-5 |
| AC-07.4 | `Esc` is global — it works on any signed-in page, not only the board | (BR-3 scope) | BR-3 |
| AC-07.5 | `Esc` never navigates the browser away from the current page | "Escape with nothing open does nothing" | FR-5 |
| AC-07.6 | The scenario **fails on unmodified `main`** — `Esc` today does nothing | (governing rule) | NFR-1 |

---

## Cross-cutting acceptance criteria (feature-level properties)

| # | Acceptance Criterion | Traces to |
|---|---------------------|-----------|
| AC-X.1 | **Bound set == advertised set.** Every shortcut the help overlay lists is bound and does something; no shortcut outside that list is bound. Both read from `SHORTCUTS` (`keyboard.rs:48-56`) | BR-1, FR-1 |
| AC-X.2 | **Typing is never captured.** `@property`: no shortcut fires from a text-entry context or under a `Ctrl`/`Cmd`/`Alt` chord, for any of the seven | NFR-2, NFR-3, BR-2, R2, R6 |
| AC-X.3 | **No-JS intact.** With scripting disabled the app behaves exactly as today; no advertised action is reachable only by keyboard | NFR-4, BR-6, R7 |
| AC-X.4 | **Zero server delta.** No route, endpoint, or migration is added. The only server-side changes are removals (the retired `#kb-items` builder) and the correction of `keyboard.rs`'s stale "alpine.js" doc comment | FR-11, ODD-1, ODD-2, R9 |
| AC-X.5 | **Handlers survive htmx swaps.** After filing an issue via `c` (which swaps `#modal-root` and re-renders cards), `j`/`k`/`Enter` still work with no page reload | FR-10, NFR-6, R4 |
| AC-X.6 | **Vendored + CSP-safe.** Any new script is an external same-origin file under `static/js/`, `defer` from `base.html`, with no inline handlers and no CDN | NFR-5 |
| AC-X.7 | **Drag coexistence.** Every existing drag scenario passes unchanged; a drag and the selection never corrupt each other | NFR-8, R4 |
| AC-X.8 | **Every scenario reds on `main`.** Each new acceptance scenario fails today. One that passes unchanged is proof it tests the server contract, not the feature | NFR-1 |

## Traceability summary

| Story | ACs | Scenarios | FR/NFR/BR covered |
|-------|-----|-----------|-------------------|
| US-01 | AC-01.1–01.6 | 4 | FR-1, FR-4, FR-5, BR-1, BR-3, BR-4, NFR-1, NFR-4 |
| US-02 | AC-02.1–02.7 | 4 (incl. 1 `@property`) | FR-2, FR-3, BR-2, BR-7, NFR-2, NFR-3 |
| US-03 | AC-03.1–03.6 | 4 | FR-6, BR-2, BR-3, BR-6, NFR-1, NFR-4 |
| US-04 | AC-04.1–04.7 | 4 | FR-2, FR-5, FR-7, NFR-1 |
| US-05 | AC-05.1–05.9 | 6 | FR-8, FR-10, BR-5, NFR-1, NFR-6, NFR-7, NFR-8 |
| US-06 | AC-06.1–06.6 | 4 | FR-2, FR-5, FR-9, BR-2, BR-5, NFR-1 |
| US-07 | AC-07.1–07.6 | 5 | FR-1, FR-5, BR-3, BR-4, BR-5, NFR-1 |
| Cross-cutting | AC-X.1–X.8 | 3 `@property` | FR-1, FR-10, FR-11, BR-1, BR-6, NFR-1..NFR-8 |

**Every FR (1–11), NFR (1–8) and BR (1–7) in `requirements.md` is covered by at least one AC.**

## Open questions handed to DISTILL

- **ODD-9 — the harness cannot press keys.** Every AC here needs a browser-capable driver; the shipped
  reqwest+scraper harness (`InProcHarness`, `us_12_keyboard_nav.rs:48-56`) can only assert server contracts.
  DISTILL/DESIGN must decide the driver and how it joins `cargo xtask ci` (which today runs the whole acceptance
  suite, `AGENTS.md`). **This gap is why the missing layer went unnoticed** — it is the root cause, not a
  side issue.
- **ODD-1 — retiring `#kb-items` deletes two currently-green assertions.** The DISTILL wave must consciously
  remove `us_12_keyboard_nav.rs:334-360` and `feature_b_web_tier.rs:568-572` rather than work around them.
- **ODD-7 — the a11y mechanism for non-focus selection** determines how AC-05.7 is actually asserted
  (`aria-activedescendant` vs an announced live region vs something else).
