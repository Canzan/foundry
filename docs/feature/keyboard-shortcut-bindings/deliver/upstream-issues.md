# DELIVER — Upstream Issues

Back-propagation from the DELIVER wave to prior waves. Per the contract, prior-wave
documents are NOT edited; the delta and its rationale are recorded here.

---

## UI-1 — ADR-001 §2's "zero Alpine consumers" precondition is FALSE (found at step 01-03)

**Source**: `docs/feature/keyboard-shortcut-bindings/design/adr-001-vanilla-dispatch-layer.md` §2

**What DESIGN asserted** (and what step 01-03 carried forward as a precondition):

> Zero app consumers is VERIFIED (`x-data` / `x-on:` / `x-model` / `x-show` / `x-init` /
> `@click` / `Alpine` across `templates/` return zero hits)

**What is actually true**: that grep is correct but **too narrow**. It searched for Alpine
*directives in templates* — of which there are genuinely zero. It never searched for
**contract tests asserting the vendored asset is served**. Three exist, all currently green
and none `@pending`:

| Consumer | What it asserts |
|---|---|
| `crates/foundry-acceptance/tests/features/us-b02-vendored-assets.feature:31` | `@walking_skeleton` — "The vendored htmx, **Alpine**, and stylesheet are served by the binary" |
| `crates/foundry-acceptance/tests/features/us-b01-styled-board.feature:48` | "the board loads the vendored htmx **and Alpine** scripts from the application's own static path" |
| `crates/foundry-app/src/lib.rs:300` | `assert!(static_cache_control_value("/static/vendor/alpine.min.js").contains("immutable"))` |

Plus stale prose referencing alpine.js at `projects.rs:748,795,882`, `csrf.rs:39,163`, and
`views.rs`, which the step's zero-hit `alpine` grep gate also catches.

So: Alpine has **zero runtime consumers** and **three contract consumers**. Deleting the
asset reds a *delivered* feature's walking skeleton. The distinction the ADR missed is
between "nothing uses it" and "nothing asserts it exists".

**Why this matters beyond the mechanics**: `projects.rs:882` reads *"the alpine.js j/k
handler walks"* — a third stale comment describing a handler that never existed, alongside
`keyboard.rs:1-29` and `keyboard.js:16`. The same failure mode this feature exists to close
(a written claim outliving, or preceding, the code it describes) is present in the
codebase in at least three places.

**Resolution (ratified by the user, 2026-07-16)**: **Retire Alpine and amend the two shipped
contracts** to assert only htmx. Rationale: the scenarios assert a dependency that does
nothing — they are testing a fiction, and keeping them green would preserve 44 KB of dead
framework on every page load in order to satisfy an assertion about its own presence. The
`AGENTS.md` dead-code policy ("Remove dead/legacy code outright — do not leave it inert")
governs. ADR-001 §2's *conclusion* (drop Alpine) stands; only its *precondition* (zero
consumers) was wrong, and the correction widens the blast radius rather than reversing it.

**Scope delta to step 01-03** (authorized by the orchestrator, disclosed here):
- `crates/foundry-acceptance/tests/features/us-b01-styled-board.feature` — drop the Alpine arm
- `crates/foundry-acceptance/tests/features/us-b02-vendored-assets.feature` — drop the Alpine arm, retitle the scenario
- `crates/foundry-acceptance/src/steps/` — whichever step module backs those two scenarios
- `crates/foundry-app/src/lib.rs` — remove the alpine.min.js cache assertion at :300
- `crates/foundry-app/src/csrf.rs`, `views.rs`, `projects.rs` — correct stale alpine.js prose

**Process note**: step 01-03 was correctly BLOCKED, not failed, by the crafter. It logged
RED_ACCEPTANCE / RED_UNIT / GREEN / COMMIT as `SKIPPED / BLOCKED_BY_DEPENDENCY` rather than
marking them EXECUTED to satisfy a hook demanding recovery — which would have been the exact
audit fraud the DES rules forbid. Zero source files were modified. The step instructed
"if DELIVER finds ANY Alpine consumer this ADR missed, STOP and re-open ADR-001 rather than
working around it", and that is what happened.

---

## UI-3 — ADR-002's no-exemptions guard makes `Esc` unable to leave any text field (found at step 02-01)

**Source**: `design/adr-002-guard-predicate.md` (the Decision table, guard 4) vs. US-07 / FR-5 /
`slices/slice-03-create-issue-and-escape-modal.md:14`.

**The contradiction, stated once**: ADR-002 requires the guard chain be evaluated before dispatch
"for **all seven**, with **no exemptions**", and guard 4 makes any key INERT when
`isTextEntry(event.target)`. `Esc` is one of the seven. **Every modal Foundry renders autofocuses a
text input** (`new_issue_modal.html:6` `input[name=title][autofocus]`; `issue_edit_modal.html:4`).
Therefore, the instant a modal opens, `Esc` is delivered to a text-entry context and is suppressed —
so **`Esc` can never close the modal it just opened**, and the user cannot leave the field by the
means every AC names.

This is not a speculative reading. Step 02-01 implemented ADR-002's chain **exactly as specified** and
observed it directly:

```
✘ Given Mei has typed in the title field and then pressed "Esc" to leave it
    "Esc" pressed in the title field did not close the new-issue modal, so Mei never
    left the text-entry context and the re-enablement this scenario asserts (AC-02.6)
    cannot be exercised at all.
```

**What it blocks**:
- **`Leaving the text field re-enables the shortcuts immediately`** (AC-02.6, slice 02) — returned to
  `@pending` at step 02-01. Its step definitions are written, correct, and red on the Given.
- **`Escape closes the new-issue modal and returns to the board`** (slice 03) — will hit the identical
  wall, since its Given is "Mei has opened the new-issue modal by pressing `c`" and the title is
  autofocused.
- **`Escape closes one layer at a time`** (slice 03, `@critical`) — same.

**Why guard 4's own rationale does not reach `Esc`**: the verdict column reads *"INERT — the character
is typed"*. The chain's principle is *let the text-entry context handle the key natively*, which is
exactly right for `c`, `/`, `j`, `k` and `?` (they insert characters) and for `Enter` (the browser
submits the form — ADR-002 names this as evidence the guard is structural). **`Esc` inserts no
character and has no native behaviour in a text input.** Suppressing it protects no keystroke and
costs US-07 entirely. `Esc` is the one key for which "inert" and "let the browser handle it" are not
the same outcome.

**This is slice 02's learning hypothesis resolving — not an implementation defect.** The slice doc
states the disproof condition in its own words: *"if any of the seven turns out to need a bespoke
carve-out (which would mean the guard is not structural and BR-2 is wrong)"*. `Esc` is that key. The
slice also pre-commits the response: *"the honest response is **not** to ship carve-outs"* — which is
why step 02-01 did **not** add an `Esc` exemption to green its own scenario.

**Not resolved here.** Three candidate resolutions are visible, and choosing between them is a DESIGN
decision (it amends the crux ADR or an AC), not a crafter's:

1. **Narrow guard 4's domain to text-producing keys.** `Esc` (and only `Esc`) is dispatched from text
   contexts. Defensible as a *refinement of the predicate's stated rationale* rather than a
   per-shortcut carve-out — the rule becomes "a text-entry context keeps the keys it can consume",
   which is still one structural chain with no call-site checks. Costs: BR-2's "no exemptions" wording
   must be amended to say what it means.
2. **Amend AC-02.6 / US-07 to leave the field by a non-`Esc` means** (Tab, or clicking the board). Keeps
   ADR-002 byte-for-byte, but concedes that `Esc` — an advertised shortcut — does nothing from the
   surface users most want to escape, which BR-1 (bound == advertised) then has to account for.
3. **Drop `Esc` from `SHORTCUTS`.** The response slice 02 pre-authorises for a disproven hypothesis
   ("unbind the character keys and shrink the advertised list so the help page stays truthful"). Almost
   certainly wrong here — `Esc` works fine outside text fields, and slice 01 ships it — but recorded so
   the option is explicit rather than assumed away.

**Recommendation (not a decision)**: option 1. It is the only one that keeps the advertised behaviour
and the structural shape, and the amendment it needs is to wording that already does not match its own
rationale. Options 2 and 3 both make the product worse to protect a sentence.

**Process note**: step 02-01 shipped the two scenarios that are genuinely achievable (the
`@paired-assertion` guard and the typing scenario), with the guard implemented per ADR-002 and its
revert-reds-it falsification proven. It did not force the third green, and did not touch slice 03's
scope to do so.

---

## UI-2 — Roadmap step 01-02 claims a scenario that cannot pass at slice 01's scope (found at step 01-02)

**Source**: `docs/feature/keyboard-shortcut-bindings/deliver/roadmap.json` step `01-02`

The scenario **"The overlay lists exactly the seven advertised shortcuts and each is bound"**
was assigned to step 01-02. Its middle arm asserts *"every shortcut it lists is bound and
does something"* — which requires **all seven** keys bound. Slice 01 binds **two** (`?` and
`Esc`); `slice-01-dispatch-layer-and-help-overlay.md` states its own scope as
"KPI-1 advertised-to-working 0/7 → 2/7" and "no character key is bound here".

The roadmap step and the DESIGN slice scope genuinely conflict. The crafter's only two paths
were both violations — bind all seven speculatively (code with no requiring test, contradicting
the slice's stated OUT-of-scope), or assert only over the implemented subset (weakening the
test to force a pass). It escalated instead, correctly.

**Resolution**: the scenario stays `@pending` through slice 01 and **moves to slice 05**,
where all seven keys are bound and the bound-equals-advertised invariant (BR-1, KPI-5) can
hold honestly. To be applied to `roadmap.json` before slice 05 executes.

**Related drift risk**: `keyboard.js:16` comments that bound == advertised "by construction",
but `:73` hardcodes `event.key === "?"`. Nothing structurally prevents drift today — the
claim is aspirational. The invariant is enforced by *that scenario* (a test), not by
construction. When the scenario lands in slice 05, either the comment must be corrected to
say so, or the derive-from-`SHORTCUTS` design must actually be built.

**The roadmap review did not catch either issue**: it verified every scenario had a step, but
not that each step's scenarios *could pass at that step's slice scope*. Worth adding to the
roadmap quality gate.
