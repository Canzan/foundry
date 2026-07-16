# ADR-005: Board-only scope, the injected search panel, and Enter-via-the-board-card (ODD-6)

## Status
Accepted — 2026-07-15 (Morgan, DESIGN wave). Feature-local. Resolves **ODD-6** (blocking, slice 04) and
**Risk R8**. **Amends locked decision D2** — recorded in `upstream-changes.md` §3.

## Context
Locked D2 scoped `c`/`j`/`k`/`Enter` to *"the board + issue list"*. **There is no issue list.** Verified:
`…/issues` is registered **POST-only** (`lib.rs:487-490`); there is no `/issues` GET, no backlog page;
`report.html:6` lists change events, not issues. The user has ratified **board only** and dropped the issue
list as a surface: `/` reveals a search box **on the board**, and `j`/`k` then walk the search results.

DISCUSS left three things to DESIGN: where the box renders, how it appears/hides, and whether selection
moves between board cards and result rows or the surfaces are modal with respect to each other.

**And there is a fourth, which DISCUSS did not catch and which is load-bearing.** US-06/AC-06.1 specify
that `Enter` *"opens the selected card via that card's shipped `hx-get={edit_url}` → `#modal-root`"*. That
works on the board. It **cannot** work on a search result: `partials/search_results.html:4` renders

```html
<li class="search-result" data-issue-key="{{ item.key }}"><span class="key">…</span> <span class="title">…</span></li>
```

— **no `hx-get`, no `edit_url`, no `data-state-url`**. The view-model carries only key + title
(`keyboard.rs:233-239`). As shipped, a selected search result has **no open path at all**.

## Decision
1. **Board only.** `c`, `/`, `j`, `k`, `Enter` are active on a surface with a team+project context — the
   board. On a page without one (the dashboard) they do **nothing**, silently: no modal, no error, no
   navigation (BR-3). `?`/`Esc` remain global (ADR-003). `project_context` is read off the board's **own**
   `hx-get` (`board.html:6`) rather than reconstructed, so `c` and the "New issue" button cannot disagree.
2. **The box**: `keyboard.js` injects, on any board page, (a) a `hidden` search panel and (b) a
   pointer-clickable **"Search" control** beside "New issue" (`board.html:5-6`). `/` reveals + focuses the
   panel and **`preventDefault()`s its own keypress** so the field is empty (FR-7). `Esc` hides it, clears
   the query and results, and restores the board (ADR-003 step 3). Results are the shipped
   `GET …/search?q=` fragment, honoured as-is: exact-key, case-insensitive substring, and
   `ul.search-results[data-empty="true"]` for no matches (`keyboard.rs:208-231`). Zero template delta.
3. **Navigation is MODAL; selection identity is SHARED.** When the panel is open, `j`/`k` walk **only**
   `li.search-result` rows; when closed, **only** `article.issue-card`. Never a merged sequence. But
   `selectedKey` is one key (ADR-004), so selecting `AUTH-2` in the results and pressing `Esc` leaves the
   ring on `AUTH-2`'s **board card**.
4. **`Enter` always resolves through the board card.** It maps `selectedKey` →
   `article.issue-card[data-issue-key=K]` → activates **that** card's shipped `hx-get`. One rule, both
   surfaces (on the board the card *is* the selected element). This works only because the panel
   **overlays** the board rather than replacing it — the cards stay in the DOM.
   **Named edge**: the board renders only `{backlog, todo, in_progress, done}` (`projects.rs:49,933-941`)
   while search returns every issue (`list_issues_by_project`). An issue in any other state is findable but
   has no card → `Enter` is a **no-op**, consistent with "no selection ⇒ no-op" (FR-9).

## Alternatives Considered
- **Add `edit_url` to the search view-model + `hx-get` to `search_results.html`** (the obvious fix for the
  missing open path) — REJECTED. It is a **server-side addition** (view-model field + template attribute),
  breaching D10/AC-X.4 (*"the only server-side changes are removals"*) for something the client can already
  do. It would also mint a **second** open path to maintain in lockstep with the card's, which is exactly
  what AC-06.5 (*"exactly one open path"*) forbids. Resolving through the board card is strictly smaller
  **and** strictly more faithful to the requirement.
- **Server-render the search input into `board.html`** — REJECTED, though it is the tidier markup. It is a
  template delta, and it *implies* a no-JS search path it cannot deliver: `search_issues` returns a **bare
  fragment with no full-page fork** (contrast `show_new_issue_modal:96-104`), so a no-JS form GET would
  render an unstyled orphan `<ul>`. Shipping an affordance that is broken without JS is worse than not
  shipping it. (See the honest limit below.)
- **A merged j/k sequence across board cards and result rows** — REJECTED. The results overlay the board,
  so a merged DOM order would walk the ring from a visible row onto a card the user **cannot see** —
  breaking D-4's founding principle that selection follows the eyes, and making `scrollIntoView` scroll a
  covered region. Modality gives one predicate: *the active surface is the panel if open, else the board*.
- **Separate selection state per surface (`boardKey` + `searchKey`)** — REJECTED as unnecessary. One key
  already yields the better behaviour: the cross-surface continuity in decision 3 falls out of ADR-004 for
  free, and two variables would have to be reconciled on every open/close.
- **A search-results *page*** — REJECTED and out of scope. None exists; building one is a different
  feature.
- **Keep "issue list" in scope and build it** — REJECTED. It does not exist (verified above). This is the
  amendment to D2, recorded in `upstream-changes.md` rather than silently reinterpreted.

## Consequences
- **Positive**: `Enter` has **one** implementation and **one** open path for both surfaces, converging on
  the pointer's path, at **zero server delta** — the requirement AC-06.5 asks for, obtained by resolution
  rather than duplication.
- **Positive**: `/` is an **accelerator, not the only path** — the injected control gives search a pointer
  affordance it does not have today, honouring BR-6's spirit (nothing becomes keyboard-only).
- **Positive**: modality makes `j`/`k` predictable with one rule, and the shipped `data-empty="true"` empty
  state means "nothing matched" is distinguishable from "no query" without client logic.
- **Negative / accepted — the honest limit.** **Search stays JS-only.** It has no no-JS path *today*
  (nothing links to the route at all), so nothing regresses and NFR-4 is intact — but this feature does not
  create one either. A full-page fork of `search_issues`, mirroring `show_new_issue_modal:96-104`, is the
  right fix and is a **recommended follow-up, explicitly out of scope** (it is a server change, which D10
  forbids here). Stated plainly so no one later reads "board search shipped" as "search works without JS".
- **Negative / accepted**: an issue in a non-default state is findable but not openable via `Enter` (the
  named edge). It is unreachable by pointer on the board today too — the board simply does not render it —
  so `Enter` is no worse than the status quo. Pinned as an acceptance edge rather than left to discovery.
- **Negative / accepted**: the panel's markup is JS-authored, so it is not visible in the templates a
  reader greps. Mitigated by keeping it minimal and by the browser lane asserting its observable behaviour.
- **Probe (Earned Trust)**: the scenario that would red if anyone re-added a second open path — press `/`,
  type `AUTH-2`, `j`, `Enter`, assert **the same modal a pointer click on AUTH-2's card produces**. And the
  classic-bug guard: after `/`, assert the input is focused **and empty**.
