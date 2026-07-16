# Slice 05 — `j`/`k` walk the visible cards, `Enter` opens the selected one (retires `#kb-items`)

**Goal**: Mei presses `j` on the AUTH board; the first **visible** card takes a ring. `j`/`k` walk the cards **in
the order she sees them**; a selection below the fold scrolls into view; `Enter` opens the selected issue. The
loop closes — find, select, open, escape, all from the home row. **The help page now tells the truth: 7/7.**
**Stories**: US-05, US-06.

**IN scope**
- A **client-only, ephemeral selection** (BR-5) over **visible cards** — `article.issue-card` in DOM order
  (`partials/issue_card.html:1`) on the board, and `li.search-result` on the results list (slice 04, ODD-6).
  Never persisted, never sent to the server, **resets on navigation**.
- `j`/`k` move it; the selected card takes a **ring highlight** (visible, WCAG 2.1 AA contrast, **not colour
  alone**) and is **scrolled into view** (FR-8, NFR-7). Bounded — `k` at the first card stays; `j` on an empty
  board is a no-op (`board.html:9` renders the empty state).
- `Enter` activates the **selected** card's own shipped `hx-get={edit_url}` → `#modal-root`
  (`issue_card.html:1`) — **the identical path a pointer click takes**, so keyboard and mouse cannot diverge.
  No selection ⇒ no-op (FR-9). `Enter` in a form submits normally (the slice-02 guard, not a special case).
- `Esc` closes the opened modal **with the selection intact** (FR-5, BR-5).
- **Retire `#kb-items` — delete it whole** (ODD-1, and `AGENTS.md`: *"Remove dead/legacy code outright — do not
  leave it inert"*): the carrier (`board.html:12`), its builder (`projects.rs:881-891`), its view-model field
  (`views.rs:256`), its unit tests (`projects.rs:1039-1110`), and **both acceptance assertions**
  (`us_12_keyboard_nav.rs:334-360`, `feature_b_web_tier.rs:568-572`). A grep for `kb-items`/`kb_items` must
  return **zero** hits.
- Coexist with drag-and-drop: every existing drag scenario passes unchanged; no stale ring after a drag
  (NFR-8, R4).

**OUT of scope**: a roving-tabindex / native-focus rewrite of the board (**explicitly rejected**, D-4 — the a11y
consequence is carried as NFR-7 + ODD-7, not silently absorbed); persisting selection anywhere; new shortcuts.

> **This slice deliberately deletes a passing test.** The shipped `#kb-items` carrier is hidden,
> `aria-hidden="true"`, and sorted **ASC by issue number across all columns**, while the visible board is
> **column-grouped, DESC-within-column** (`projects.rs:864-885`). The acceptance suite calls it *"the source of
> truth for the keyboard navigation order"* and asserts that ordering — but a ring cannot render on a `hidden`
> element and `scrollIntoView` does nothing to it, so the carrier **cannot** serve the locked model (D-4). It
> also has **zero browser consumers** and always has: the handler it was built for was never written. Honouring
> D-4 retires it. **This is a decision, not an accident** — ODD-1, Risk R1. DESIGN must confirm before DELIVER
> removes the assertions.

**Learning hypothesis**: disproves *"a client-only selection over visible DOM-order cards, with a ring and
`scrollIntoView`, can be kept coherent across htmx swaps and drag-and-drop, be announced to assistive tech
without native focus, and fully replace the hidden `#kb-items` carrier"* — if selection cannot survive an htmx
swap that replaces cards (by issue-key vs index vs reset — **ODD-5**), if a drag leaves the model inconsistent
(`board-dnd.js` reorders the DOM under it, R4), if a ring **cannot** be announced without roving tabindex
(**ODD-7**, R3 — in which case D-4 itself must be revisited), or if retiring the carrier turns out to break a
consumer we haven't found.

**Seams**: `partials/issue_card.html:1` (`article.issue-card[data-issue-key]`, `hx-get={edit_url}`,
`hx-target="#modal-root"`, `draggable`, `data-state-url`); `board.html:9` (columns + empty state), `:12` (the
carrier being deleted), `:13` (`#modal-root`); `projects.rs:864-879` (visible order) + `:881-891` (the ASC
builder being deleted); `views.rs:256`; `board-dnd.js:36-58,67,86-146` (drag reorders the DOM under selection);
`partials/search_results.html:4` (the second surface); the dispatch layer (slice 01) + guards (slice 02).
**Dependencies**: slice 01, **slice 02 (hard — `j`/`k` are typed characters)**; slice 04 (the results surface).
DESIGN **ODD-1** (retire the carrier + delete its green assertions — **blocking**), **ODD-5** (selection survival
across swaps/drags), **ODD-7** (the a11y mechanism — **blocking for KPI-4**), **ODD-9** (browser driver).
**Effort**: ~1 day (the most uncertainty in the feature: three ODDs + a deliberate test deletion).
**KPI**: KPI-1 **5/7 → 7/7 — the promise is fully kept**. KPI-3 (0 mouse actions to move+open). KPI-4 (a11y).
