# Phase 5 — Mutation Testing Assessment

**Verdict: `cargo-mutants` is structurally inapplicable to this feature's logic. Mutation testing was
performed instead by hand, per step, against the real code — and it found surviving mutants.**

## Why the standard gate does not apply

The project default is `per-feature` (no `## Mutation Testing Strategy` in `CLAUDE.md`), gating at
≥80% kill rate via `cargo-mutants`. `cargo-mutants` mutates **Rust**. This feature's delta:

| Surface | Size | Mutable by cargo-mutants? |
|---|---|---|
| `static/js/keyboard.js` — **the feature** | **915 lines** | **No** |
| Rust production delta (`foundry-app/src/*.rs`) | 154 insertions / 83 deletions; **~41 net logic lines**, mostly a `const`, view-model plumbing, and the `#kb-items` **deletions** | Yes, but near-vacuously |

Running the gate would report a percentage over ~41 lines of plumbing while the 915 lines that *are*
the feature went entirely untested. That number would be worse than no number: it would look like
assurance. Given this feature exists because a green suite stood over absent code, publishing a
meaningless kill rate would be the same error in a new costume.

## What was done instead — falsification per step, on the real logic

Every step ran explicit mutations against the shipped code and recorded whether the suite noticed.
This is mutation testing; only the tooling differs. **It is stronger than cargo-mutants would have
been here, because it mutated the JS that cargo-mutants cannot reach.**

| Mutation applied | Result |
|---|---|
| Delete `<script defer src=keyboard.js>` | lane probe **RED** (`[data-kb-ready]` never appears) |
| Rebind `?` → `&` | all four slice-01 scenarios **RED** |
| Stub guard 4 → `return false` | `@paired-assertion` **RED** |
| Add `event.shiftKey` to guard 2 | `@shift` **RED** (catches the obvious wrong implementation) |
| Remove guard 2 entirely | `@modifier` **RED** (chord files an issue) |
| **Remove guard 1 (IME) entirely** | **`@ime` STAYS GREEN — SURVIVING MUTANT (UI-5)** |
| Selection → stored index | `@drag-coexistence` **GREEN → RED only after strengthening (UI: 05-02)** |
| `Enter` out of `NATIVE_TEXT_ENTRY_KEYS` | **GREEN → RED only after strengthening (05-03)** |
| Remove `aria-activedescendant` | `@a11y` **RED** |
| Remove `tabindex="0"` | `@a11y` Given **RED** |
| Revert `SELECTION_INSTRUCTION` to generic prose | help scenario **RED** |
| Remove `/`'s `preventDefault()` | AC-04.1 **RED** (the classic stray-slash bug, reproduced) |
| Remove the `htmx:afterSwap` hook | `@htmx-swap` **RED** |
| Half-hook (ring only, no ARIA) | `@htmx-swap` **RED** + `@a11y` **RED** |
| Remove UI-6's `searchSequence` bump | `Escape leaves search` **RED 5/12 runs** (0/10 with it) |
| Remove UI-7's `Tab` from the Given | both search-open scenarios **RED** |
| Enter opens a card ≠ the ringed one | `@one-open-path` **RED** |
| Enter opens the *correct* issue by a second path | `@one-open-path` **RED** (byte-identical comparison) |
| Break a step-definition regex | run **FAILS by name** (post-`fail_on_skipped`; passed silently before) |
| Break column placement after the trap-B repoint | `each_issue_lands_in_exactly_its_state_column` **RED** |

## Surviving mutants — found, recorded, not hidden

1. **Guard 1 (IME) — UI-5, OPEN.** Deleting it leaves `@ime` green, because `c` mid-composition is
   already inert via guard 4. The behaviour is protected; **not by the arm the scenario names.** Guard 1
   is correct and must stay — "no test covers it" is not grounds for deletion, it is grounds for a
   better test. Resolution (retarget the scenario at `Escape`, which an IME uses to cancel composition)
   is DISTILL's call.
2. **Two mutants that survived until the tests were strengthened** — the drag scenario (05-02) and
   `Enter`-in-a-form (05-03). Both were caught *by running the mutation*, not by review. Both scenarios
   now red under their mutation.

## The finding that matters most

At 05-04, blur-on-arrival was implemented as ratified, and **the batched lane ran 6/6 green over a
defect that strands a real human** (`send_keys("cookie")` types six characters with no round-trip, so a
blur that destroys incremental typing is invisible to it). The defect was found only by typing at
150ms/char. **A green can be an artefact of the instrument** — which is the same lesson as UI-4, UI-6,
and the feature's own origin.

## Recommendation

If a Rust-side mutation gate is ever wanted for this feature, the honest target is the **acceptance
step definitions and `xtask` preflight**, not `keyboard.js`. A JS mutation tool (Stryker) would be the
real instrument for the 915 lines — and is out of scope here, as adding a JS toolchain is a project
decision, not a step's. Recorded rather than silently skipped.
