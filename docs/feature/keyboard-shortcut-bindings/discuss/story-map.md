# Story Map: keyboard-shortcut-bindings

## User: Mei Tanaka (`mei@acme.com`) — keyboard-first maintainer, the repo's own harness identity
## Goal: work the AUTH board — learn, file, find, move, open, escape — without leaving the home row

## Backbone

| LEARN the shortcuts | TYPE safely | FILE an issue | FIND an issue | MOVE between issues | OPEN an issue | ESCAPE |
|---|---|---|---|---|---|---|
| `?` overlay in place (US-01) | text-input guard (US-02) | `c` opens modal (US-03) | `/` focuses search (US-04) | `j`/`k` walk visible cards (US-05) | `Enter` opens selected (US-06) | `Esc` closes topmost (US-07) |
| lists exactly `SHORTCUTS` | modifier guard (US-02) | title autofocused (US-03) | no stray "/" typed (US-04) | ring highlight (US-05) | one open path = the card's `hx-get` (US-06) | selection survives (US-07) |
| global — any signed-in page | guard releases on blur (US-02) | no project ⇒ silent no-op (US-03) | exact-key + substring (US-04) | `scrollIntoView` (US-05) | no selection ⇒ no-op (US-06) | nothing open ⇒ no-op (US-07) |
| no-JS full-page link stays | IME composition (ODD-4) | no-JS fallback intact (US-03) | empty state honoured (US-04) | retires `#kb-items` (US-05, ODD-1) | `Enter` in a form submits (US-06) | one layer per press (US-07) |
| | | | results are a j/k surface (ODD-6) | a11y announcement (ODD-7) | | never navigates (US-07) |

---

## No Walking Skeleton — and why that is correct here

**Locked: D-8.** This is **brownfield**, and the usual reason for a walking skeleton does not apply.

A walking skeleton exists to prove an untested end-to-end path connects. Here **the path is already shipped and
routed**: all three server routes exist (`lib.rs:491-498`, `:536`), return the right fragments, are authorized,
and are covered by six green acceptance scenarios. There is no integration risk to de-risk at the seam — the
seam works.

The gap is **exactly one layer**: the browser-side code that presses against it. So slice 01 is not a skeleton
but a **thin real capability** (`?` + `Esc`), chosen because it is the smallest change that both stands up the
dispatch layer and re-establishes the product's broken promise.

> **The inverted risk.** Normally the skeleton de-risks "do the pieces connect?". Here the pieces demonstrably
> connect and the suite is green — while the feature is **100% absent to the user**. The risk is not
> integration; it is that **the test suite cannot see the feature at all** (NFR-1, ODD-9). That is what the
> slicing must attack.

---

## Release Slices (sliced by outcome, not by shortcut)

### Slice 01 — "The product stops lying" (US-01)
**Tasks**: the guarded dispatch layer + `?` renders the shipped help fragment as an overlay + `Esc` closes it.
**Outcome**: Mei presses the key whose whole job is to *display the promise*, and the promise appears — in place,
without losing her board.
**Why first**: the arc starts at **betrayed**, not anxious. The product made a written promise and broke it.
The first thing shipped must **re-establish credibility**, and `?` is the only shortcut where the promise and its
fulfilment are the same keystroke. It also stands up the dispatch layer every other slice reuses, and it forces
**ODD-3** (the global mount point) into the open immediately rather than at slice 04.
**KPI**: KPI-1 (advertised-to-working ratio: 0/7 → 2/7).

### Slice 02 — "Typing still works" (US-02)
**Tasks**: the text-input guard + the modifier guard, proven across all shortcut characters.
**Outcome**: Mei types `"cache invalidation on login"` and gets exactly that. `Cmd+C` copies.
**Why second — before `c` is ever bound**: this is the **cliff**. Every one of the seven is a plain printable
character or bare key, i.e. exactly what people type. A layer that eats keystrokes is **strictly worse than
shipping nothing**. Binding `c` before the guard exists would ship the worse-than-nothing state, however briefly.
**We deliberately order the risk before the reward.**
**KPI**: KPI-2 (0 captured keystrokes — hard guardrail).

### Slice 03 — "File without the mouse" (US-03 + the `Esc`-closes-modal half of US-07)
**Tasks**: `c` → the shipped htmx new-issue path → `#modal-root`; `Esc` closes the modal.
**Outcome**: Mei sees a bug and files it in one keystroke, and can back out of it in one more.
**Why third**: `c` is the highest-value single shortcut (the first entry in the help list, the most-used key in
any tracker) and it is now **safe** to bind because slice 02 shipped the guard. Pairing it with `Esc` means it is
never a one-way door.
**KPI**: KPI-1 (2/7 → 4/7), KPI-3 (0 mouse actions to file).

### Slice 04 — "Find without the mouse" (US-04)
**Tasks**: `/` focuses search (suppressing the "/" character); results from the shipped search fragment.
**Outcome**: Mei goes from "I half-remember an issue called *session*" to looking at it, without a pointer.
**Why fourth**: independent of selection, and it completes the *acting* capabilities. It also **creates the
second j/k surface** (the results list) that slice 05 then covers — so the ordering hands slice 05 a strictly
richer target (ODD-6).
**KPI**: KPI-1 (4/7 → 5/7), KPI-3.

### Slice 05 — "Move and open without the mouse" (US-05 + US-06)
**Tasks**: `j`/`k` selection over visible cards + ring + `scrollIntoView`; `Enter` opens the selected card; the
`#kb-items` retirement.
**Outcome**: Mei walks the board with her eyes and hands in the same place and opens the right issue — the loop
closes.
**Why last**: it carries the most uncertainty (**ODD-1** the carrier collision, **ODD-5** swap survival, **ODD-7**
a11y) and it depends on the guard and the dispatch layer both being proven. `j`/`k` and `Enter` ship **together**
because selection without `Enter` is decoration — the user can move a highlight but not act on it, which is not a
shippable outcome. This is also where a **currently-green test gets deliberately deleted**.
**KPI**: KPI-1 (5/7 → **7/7 — the promise fully kept**), KPI-3, KPI-4 (a11y).

---

## Slicing rationale — why not "one slice per shortcut"?

Seven shortcuts, five slices. The mapping is **by user outcome**, not by key:

- **`?` and `Esc` are split across slices 01/03** rather than shipped as one "global keys" slice. `Esc` is not a
  capability — it is the **escape hatch for whatever a slice opens**. Slice 01 opens an overlay, so `Esc` closes
  overlays there; slice 03 opens a modal, so `Esc` closes modals there. Shipping `Esc` for layers that don't yet
  exist would be building for an imagined future.
- **The guard (US-02) is its own slice** even though it is "just" a precondition. It is the only slice whose
  user-visible value is *the absence of a harm* — Mei can type. It earns a slice because it is the highest-risk
  requirement (NFR-2, R2) and because it must land **before** the first character-key binding.
- **`j`/`k` + `Enter` share a slice** because they are one outcome ("move and open"). Splitting them would ship
  a slice whose only value is a highlight that does nothing.

## Anti-pattern check

| Anti-pattern | Status |
|---|---|
| Feature-first slicing | **Avoided** — each slice is a user outcome ("the product stops lying", "typing still works", "file without the mouse"), not a component |
| No walking skeleton | **N/A and deliberate** (D-8) — brownfield; the end-to-end path is shipped, routed and green. Recorded, not skipped by accident |
| Fat walking skeleton | **N/A** |
| Effort-based priority | **Avoided** — slice 02 (the guard) is not the easiest, it is the riskiest, and it is second. Slice 05 (highest value: `j`/`k`/`Enter`) is last **because of dependency and uncertainty**, not effort |
| Orphan stories | **Avoided** — all 7 stories trace to `job_id: fast-keyboard-issue-flow` and to KPI-1..KPI-4 |
| Activity gaps | **Avoided** — every backbone activity is covered by exactly one slice; after slice 05 all seven advertised shortcuts work |
| Infrastructure-only slice | **Avoided** — every slice contains at least one user-visible value story (see `prioritization.md` taste tests) |
