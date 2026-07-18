# Story Map — new-issue-dialog-description

## Backbone (user activities)

```
Capture work  →  Describe it        →  File it            →  Refine it later
(press c /       (NEW: type the        (Create → card in     (edit dialog —
 New issue)       description)          Backlog)              already ships)
```

The backbone already exists end-to-end. This feature fills the **second box**, which is currently missing on
the create path and present on the refine path.

## Walking skeleton

**n/a — the skeleton ships.** The create pipe (`c` → modal → CSRF'd POST → service → `insert_issue_with_outbox`
→ OOB card into Backlog) is live and green, and `description_md` is a real column the edit path already
writes. There is no end-to-end thread to establish, only a value to thread through it. Per Decision 2
(brownfield), no skeleton slice.

## Slices

| # | Slice | Story | Persona | Value shipped | Effort |
|---|-------|-------|---------|---------------|--------|
| 01 | Description on the new-issue dialog (web) | US-01 | P1 member | Type a description while filing; it persists and round-trips | ~1 day |
| 02 | Description on the API create endpoint | US-02 | P2 integration | Scripted issues carry descriptions; rule-parity restored | ~2 h |
| 03 | Bound the description on create AND edit | US-03 | P1 / P3 operator | Paste-bombs refused with a clear message; shipped edit gap closed | ~3 h |

Briefs: `docs/feature/new-issue-dialog-description/slices/slice-0{1,2,3}-*.md`.

## Slice composition gate

Every slice contains at least one user-visible value story. **No slice is `@infrastructure`-only.** Slice 03
is validation, but it is user-visible (an error message a person reads and acts on) and enables a decision
(trim the content) — not plumbing.

## Carpaccio taste tests

| Test | Verdict |
|------|---------|
| Any slice shipping 4+ new components? | **Pass** — slice 01 adds *zero* new components; it adds a field to 2 templates, 1 view model, 1 form struct, and 2 function signatures. Slices 02/03 touch 1 struct and 1 validation helper. |
| Every slice depends on a new abstraction? | **Pass** — no new abstraction. `DESCRIPTION_MAX_LEN` is a const introduced in slice 03 *at its point of use*, not ahead of it. |
| Does any slice disprove a pre-commitment? | **Pass** — slice 01 disproves "create cleanly mirrors edit" (see its hypothesis); slice 02 disproves the rule-parity assumption if the shared service can't serve both callers. |
| Synthetic-data-only slice? | **Pass** — every slice is accepted against a real issue created through the real dialog against a real Postgres, with a browser dogfood moment. |
| 2+ slices identical except for scale? | **Pass** — three distinct surfaces (web create, API create, shared validation). |

## Prioritization

**01 → 02 → 03.**

- **01 first — highest learning leverage.** It carries all the uncertainty: threading the param through
  service + store while keeping every existing call-site byte-identical. If the mirror-the-edit-path
  assumption is wrong, it fails here, where the blast radius is one slice and the other two haven't been
  built on the assumption yet. `board-new-issue` D5's DELIVER revision is the reference class for exactly
  this risk.
- **02 second — trivial once 01 lands.** The service param exists by then; 02 is a struct field, a handler
  argument, and its tests. Sequencing it after 01 means it never pays 01's discovery cost.
- **03 last — but not optional.** It is a coherent rule change across two dialogs and reads most clearly once
  the create path exists to be bounded. It also depends on nothing from 02.

**Dogfood cadence**: each slice is dogfooded in a real browser (or via `cli.sh` for 02) the same day it lands
— consistent with the standing lesson that a green lane can be an artefact of the instrument.
