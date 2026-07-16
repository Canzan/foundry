# ADR-006: Announcing a selection that is not focus (ODD-7)

## Status
**ACCEPTED — ratified by the user 2026-07-15** (Option A). Morgan, DESIGN wave. Feature-local. Resolves
**ODD-7** (blocking for KPI-4, slice 05) and **Risk R3**. **D-4 stands, upheld on review.**

The one-time Tab-to-the-board for AT users is an **accepted, documented cost — not an open residual**.
KPI-4 is met **conditionally on that Tab**, and the condition must be **stated wherever KPI-4 is claimed**
rather than buried. The reasoning that produced this verdict is retained in full below — particularly why a
live region is inadequate, and Option B's trade-off — because someone will revisit this and deserves the
argument, not just the answer.

## Context
D-4 rejected roving tabindex, so the ring is **not native focus** and **nothing announces it for free**.
NFR-7 refuses to let that be silence and demands an explicit mechanism —
*"`aria-activedescendant` on a container, a live region, or something else"* — and adds: if the answer is
"there isn't one", **D-4 itself must be revisited**.

**The finding that decides this ADR is not about announcement.** In screen-reader **browse mode** (the
default in NVDA and JAWS), single printable letters are **quick-navigation keys** consumed by the screen
reader before any page listener runs — `k` is "next link" in JAWS/NVDA, and the letter keys generally
belong to the AT, not the document. So for a screen-reader user in browse mode, **`j` and `k` never reach
`keyboard.js` at all.**

That reframes ODD-7. The question is not "how do we announce the ring?" — it is "how do the keys arrive?"
A live region answers the wrong question: it would announce a selection that can never change. Keys reach
the page only in **focus/forms mode**, which AT enters when DOM focus lands on a form control or a
**composite widget** role. That is a hard constraint, and it admits exactly two mechanisms:
`aria-activedescendant` on a focused composite container, or roving tabindex.

The shipped markup is unusually well-suited to the first: **cards already carry `id="issue-{{ card.key }}"`**
(`issue_card.html:1`), which is `aria-activedescendant`'s hard prerequisite.

## Decision
**`aria-activedescendant` on a focusable ARIA composite, applied by `keyboard.js`, plus a ring that meets
WCAG 2.1 AA and never relies on colour alone. No live region.**

Applied at init and **re-applied on `htmx:afterSwap`** (the ADR-004 projection step already runs there):

| Element | Applied |
|---|---|
| the board region (`div.board`, `board.html:7`) | `role="listbox"`, `tabindex="0"`, `aria-label="Issues"`, `aria-activedescendant="issue-{selectedKey}"` |
| each column (`section.column`, `:9`) | `role="group"`, `aria-label="{column label}"` |
| each card (`article.issue-card`, `issue_card.html:1`) | `role="option"`, `aria-selected="true\|false"` |
| the search panel + its rows | the same `listbox`/`option` shape (ADR-005) |

- **`listbox`/`option`, not `grid`.** The interaction *is* linear single-select: `j`/`k` move next/prev
  through one sequence, one selection, no 2D arrow navigation (there is no `h`/`l`). `grid` would promise
  row/column arrow semantics the feature does not ship. `listbox` permits `group` children containing
  `option`s, which maps the columns exactly.
- **The accessible name falls out of the shipped markup.** A card's `.key` + `.title` spans compute to
  *"AUTH-2 Session cookie not cleared on sign-out"* — precisely the right announcement, with no extra
  labelling.
- **`aria-selected` is state, not just announcement** — a screen-reader user can query what is selected,
  not only hear it change once. It is also the ring's CSS hook (`[aria-selected="true"]`), so the visual
  and the semantic **cannot** drift.
- **The ring**: `outline` + `outline-offset` at ≥3:1 non-text contrast against adjacent colours (WCAG 2.1
  AA, 1.4.11) **plus** a non-colour cue (border weight/left-bar), so it never relies on colour alone
  (NFR-7). Rules live in the stylesheet ⇒ the hand-maintained content-hash re-hash applies (inherited
  procedure, `navigation-bar-linear-ui/design/adr-004`).
- **This honours D-4.** A single `tabindex="0"` on **one container** is not the roving-tabindex rewrite D-4
  rejected: it adds one tab stop, not N; it does not move tabindex among cards; it does not touch the
  cards' tab order; and it does not fight `board-dnd.js` (ARIA roles do not affect native HTML5 drag).

## Alternatives Considered
- **A visually-hidden `aria-live="polite"` region announcing each move** — REJECTED as the mechanism, and
  this is the option NFR-7 names first. It **cannot work**: in browse mode `j`/`k` are intercepted by the
  screen reader, so there is no move to announce. It would produce a feature that *appears* accessible in
  review and is inert in use — the worst outcome available. (It is also redundant *given*
  `aria-activedescendant`, which announces the active option natively; running both double-announces.)
- **`role="application"` on the board** — REJECTED. It does force focus mode and would deliver the keys,
  but it suppresses browse mode wholesale for that region, so the user loses their AT's own navigation
  inside the board and can read the cards **only** through what we choose to announce. It trades one
  accessibility problem for a worse one.
- **`role="grid"` + `gridcell`** — REJECTED: promises 2D arrow navigation the feature does not implement.
- **Roving tabindex (i.e. reopening D-4)** — **CONSIDERED AND REJECTED — the trade-off, recorded in full
  because this is the decision a future reader will want to revisit.** It is honestly the stronger
  mechanism on the merits: native focus means the keys arrive with **no** ARIA overlay and **no** Tab
  prerequisite, the card's content is read natively rather than flattened to an option name,
  `:focus-visible` gives the ring for free, and "selected" needs no invention. Its cost is what D-4 priced
  as too large: a rewrite of the board's focus model (tabindex moving among **N** cards, changing the tab
  order across the whole board) that must then be re-proven against `board-dnd.js` (NFR-8). **It is the
  only mechanism that removes the accepted Tab cost below.** Escalated to the user as Option B, weighed
  against Option A, and **rejected on review: D-4 stands and the Tab cost is accepted** (2026-07-15). If
  the Tab cost is ever judged unacceptable, **this is the alternative to reopen** — the argument above is
  the whole case for it.
- **Do nothing and accept the a11y gap** — REJECTED. NFR-7 exists specifically to forbid inheriting
  silence, and KPI-4 is a named guardrail.

## Consequences
- **Positive**: ODD-7 has a real, specific answer grounded in why the keys do or do not arrive, not a
  gesture at ARIA. The mechanism reuses shipped `id`s and shipped card content.
- **Positive**: `aria-selected` unifies the visual ring and the semantic state — one hook, no drift.
- **Positive**: zero template delta (JS applies the roles) and no conflict with drag or htmx.
- **Negative / accepted**: `keyboard.js` overlays ARIA semantics the templates do not show, so a reader of
  `board.html` cannot see that the board is a listbox. Mitigated by re-application on swap being part of the
  same projection step as the ring, and by an automated a11y check gating slice 05.
- **Negative / accepted**: `role="option"` flattens a card's internal structure for AT (an option's content
  is a name, not a document). Correct here — the card *is* just a key and a title.

## The accepted cost: one Tab, and a conditional KPI-4 (RATIFIED — Option A, 2026-07-15)

**This was escalated to the user as a genuine choice and is now closed. D-4 stands, upheld on review.**

**An AT user must Tab to the board once before `j`/`k` arrive.** Inside the composite everything works:
keys dispatch, `aria-activedescendant` announces, `aria-selected` exposes state, `Enter` opens. Outside it —
an AT user who lands on the board and presses `j` without first focusing it — **nothing happens**, exactly
as today. Sighted keyboard users are unaffected (the document-level listener fires immediately, with no
focus prerequisite).

This is the irreducible cost of D-4's rejection of roving tabindex: **a selection that is not focus cannot
receive keys that focus would have received.** It is now an **accepted, documented cost — not an open
residual.**

**The user's decision (2026-07-15): Option A — Accept.** Ship `aria-activedescendant` on the focusable
composite. Option B (reopen D-4 for roving tabindex) was weighed and **rejected**; its full trade-off is
retained under *Alternatives Considered* above, because this is the decision someone will want to revisit
and they deserve the argument, not just the verdict.

**KPI-4 is met CONDITIONALLY on that Tab — and the condition must be stated wherever KPI-4 is claimed,
never buried.** Concretely:

- KPI-4 (*"selection change is announced on 100% of `j`/`k` moves"*) holds **within focus mode**, i.e.
  once the board has DOM focus. It does **not** hold for an AT user who never focuses the board.
- Any slice report, KPI roll-up, or a11y claim that cites KPI-4 must carry the qualifier **"once the board
  is focused"**. A bare "KPI-4 met" is a misstatement of what shipped.
- **Obligation on slice 05 (DELIVER)**: document *"Tab to the board, then `j`/`k`"* in the help overlay's
  own copy — the discoverability surface the whole feature is built around. The instruction must reach the
  user, not just this ADR.

**If the Tab cost is ever judged unacceptable, D-4 must be reopened** — that remains a legitimate finding
rather than a failure of this design, and *Alternatives Considered* is where that case is already made.
