# Slice 04 — `/` finds an issue without the mouse (and doesn't type a slash)

**Goal**: Mei half-remembers an issue called something like *session*. She presses `/`; the search box takes
focus **and is empty**. She types `session`; `AUTH-2 — Session cookie not cleared on sign-out` appears. A search
for `zzz` says so rather than showing a blank void.
**Story**: US-04.

**IN scope**
- `/` (guarded by slice 02) on a surface with a **team+project context** → move focus to the search input **and
  suppress the default keypress** so the "/" character is **not** inserted (FR-7).
- Results from the shipped `GET /team/{team_slug}/project/{project_slug}/search?q=` (`keyboard.rs:160-202`,
  route `lib.rs:495-498`) rendering `ul.search-results` with `li.search-result[data-issue-key]`
  (`partials/search_results.html:4`).
- Honour the shipped matching semantics as-is: **exact key** `AUTH-2` → that issue only
  (`filter_matches:217-224`); **case-insensitive title substring** (`:226-231`); **empty query returns every
  issue** (`:213-215`).
- Honour the shipped **empty state** `ul.search-results[data-empty="true"]` — "nothing matched" must be
  distinguishable from "no query" and from "search is broken".
- Once focused, shortcut characters typed into the box are inserted **literally** (the slice-02 guard, FR-2) —
  including `/` itself (`and/or`).
- `Esc` leaves search and restores the board (FR-5).
- **The search box itself**: the board renders **no search input today** (`board.html` has none). Where it lives
  and what it looks like is a DESIGN call (**ODD-6**) — this slice needs a target for `/` to focus.

**OUT of scope**: `j`/`k`/`Enter` **over the results list** (slice 05 — this slice **creates** that surface but
does not select within it); changing the matching semantics, the fragment, or the route; a search-results *page*
(none exists and none is being built).

**Learning hypothesis**: disproves *"`/` can focus a search input and suppress its own character, and the shipped
search fragment — with its exact-key, substring, and `data-empty` semantics already built — is sufficient for a
keyboard-driven find with no server change"* — if suppressing the default on the focusing keypress proves
unreliable (the classic bug: field focused **and** "/" typed into it, so the first search is always for
`/session`), if the empty-query-returns-everything behaviour (`:213-215`) is wrong for a live-typing UX, or if
there is nowhere coherent to put the search box on the board (ODD-6).

> **This slice hands slice 05 a second target.** The results list carries `data-issue-key` on every
> `li.search-result`, making it the **second issue-key-bearing surface** in the whole app (the board's cards are
> the first). That is the most defensible reading of the locked "board + issue list" scope — **verified: no
> issue-list page exists**; `…/issues` is registered **POST-only** (`lib.rs:487-490`). Sequencing 04 before 05
> means selection lands on a richer target (ODD-6, R8).

**Seams**: `search_issues` (`keyboard.rs:160-202`), `filter_matches` (`:208-231`), route `lib.rs:495-498`,
`partials/search_results.html:4`; `project_context` from the board's own markup (`board.html:6`); the dispatch
layer (slice 01) + guards (slice 02).
**Dependencies**: slice 01, **slice 02 (hard — `/` is a typed character)**. DESIGN **ODD-6** (where the search
box lives; the "issue list" scope reading — **blocking for this slice**), **ODD-9** (browser driver).
**Effort**: ~1 day.
**KPI**: KPI-1 **4/7 → 5/7**. KPI-3 — **0** mouse actions to find an issue.
