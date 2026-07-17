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

### RESOLUTION — Option 1, ratified by the user 2026-07-16

**Narrow guard 4's domain to text-producing keys.** `Esc` is dispatched from text-entry contexts;
every other key remains inert there. Verified before ratifying: `keyboard.rs:75` advertises `Esc` as
literally **"Close modal"**, and both modals autofocus a text input (`new_issue_modal.html:6`,
`issue_edit_modal.html:6`) — so ADR-002 as written made the one key whose *advertised purpose is to
close a modal* the one key that cannot close a modal.

**ADR-002's decision stands; its *wording* was wrong.** The chain's stated rationale is
*"let the text-entry context handle the key natively"*. That is satisfied for `c` / `/` / `j` / `k` /
`?` (they insert characters) and for `Enter` (the browser submits the form). `Esc` inserts no
character and has no native behaviour in a text input — Foundry's modals are `div`s, not `<dialog>`,
so nothing else consumes it. Suppressing `Esc` therefore protects no keystroke and costs US-07
entirely. **`Esc` is the one key for which "inert" and "let the browser handle it" are not the same
outcome** — the rule was never really "suppress everything", it was "a text-entry context keeps the
keys it can consume", and guard 4 simply said it imprecisely.

**BR-2 amendment**: "no exemptions" now reads as *no per-shortcut exemptions and no call-site checks*.
The predicate stays ONE structural chain evaluated ONCE before dispatch; the narrowing is a property
of the **predicate's domain**, not a carve-out bolted onto a call site. If an implementation ends up
with `if (key === "Escape")` scattered at dispatch sites, that is the failure mode BR-2 forbids and
this ratification does NOT authorise it.

**Unblocks**: AC-02.6 (`Leaving the text field re-enables the shortcuts immediately`, slice 02) and
both slice-03 `Esc` scenarios (`Escape closes the new-issue modal and returns to the board`,
`Escape closes one layer at a time`).

**Process note**: this is slice 02's learning hypothesis resolving *as designed*, not an
implementation defect. The slice named the disproof condition in advance — *"if any of the seven turns
out to need a bespoke carve-out"* — and pre-committed the response: *"the honest response is **not** to
ship carve-outs"*. Step 02-01 honoured that, implemented ADR-002 exactly as specified, observed the
failure directly rather than theorising, and escalated instead of quietly greening its own scenario
with an exemption. The hypothesis did its job.

**Process note**: step 02-01 shipped the two scenarios that are genuinely achievable (the
`@paired-assertion` guard and the typing scenario), with the guard implemented per ADR-002 and its
revert-reds-it falsification proven. It did not force the third green, and did not touch slice 03's
scope to do so.

### Implementation note — how the narrowing was expressed (step 02-01 re-dispatch, 2026-07-16)

Guard 4 became `isTextEntry(event.target) && isConsumableByTextEntry(event.key)`. The second
conjunct IS the narrowing, and it is a question about the **key's** relationship to a text field,
not about our shortcut list:

```js
function isConsumableByTextEntry(key) {
  if (typeof key !== "string") return true;              // unknown ⇒ fail closed
  return key.length === 1 || NATIVE_TEXT_ENTRY_KEYS.indexOf(key) !== -1;
}
```

- **`key.length === 1`** — the key produces a character. Covers `c`, `/`, `j`, `k`, `?` *and every
  character key that is not a shortcut*, which is the point: the domain is defined by the platform,
  not by `SHORTCUTS`.
- **`NATIVE_TEXT_ENTRY_KEYS`** — the non-character keys a text input acts on natively (`Enter`
  submits — ADR-002's own Consequences name this; plus `Tab`, `Backspace`, `Delete`, `Insert`, the
  four arrows, `Home`/`End`, `PageUp`/`PageDown`, which move the caret or edit the value). A list of
  platform facts. A future binding on one of these fails CLOSED rather than stealing caret movement.
- **`Escape` is never named.** It is dispatched from a text field because it satisfies neither arm —
  it produces no character and a text input does nothing with it. That is the narrowing doing its
  own work, which is what makes this a domain property rather than a carve-out. `grep '"Escape"'` on
  the guard path returns nothing; the only `key === "Escape"` in the file is a branch of the
  **dispatch table**, where naming keys is the entire job.

The chain is still ONE function, evaluated ONCE, with `dispatch` unreachable around it.

**One scope disclosure**: AC-02.6's Given requires `Esc` to actually close the new-issue modal (the
step waits for `#modal-root:empty`) — narrowing the predicate alone leaves the scenario red, because
`dispatch`'s `Escape` arm only closed the help overlay. So `closeTopLayer()` was added: help if open,
else the modal. Only that much — `Esc`'s layered contract and its own scenarios remain slice 03's,
still `@pending`, and none were touched. Per "no code without a requiring test", this test required
it.

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

---

## UI-4 — The acceptance runner is GREEN over undefined steps (found at step 02-02)

**Source**: `crates/foundry-acceptance/tests/acceptance.rs:278` vs `design/adr-007-browser-e2e-harness.md` §4.

`execution_has_failed()` counts **failed steps, parsing errors and hook errors — and nothing else**.
cucumber-rs reports a step with no matching step definition as **skipped**, not failed. Therefore an
un-`@pending`'d scenario whose steps are unwired **executes nothing and the run exits 0**.

Observed directly at 02-02's RED_ACCEPTANCE: all three target scenarios "passed" while executing
nothing, before their step definitions existed.

**This is ADR-007 §4's own failure mode ("probe, then refuse — NEVER skip") living inside the runner
built to prevent it.** It is the same shape as the bug this entire feature exists to close: a green
suite over an absent thing. The instrument has the disease it was built to diagnose.

**Why it is urgent rather than academic**: slices 03-05 un-`@pending` **28 more scenarios**. Any one
of them whose steps are unwired — a regex typo is enough — passes silently. The feature's own
`@needs-browser` lane offers no protection against this, because the lane never runs the step.

**Candidate fix**: cucumber-rs exposes `.fail_on_skipped()`, which makes skipped steps fail the run;
alternatively assert `writer.skipped_steps() == 0` beside the existing failure check. Either makes the
runner refuse instead of skip. **Risk to assess before applying**: the change affects the shared runner
for all 62 features / 514 scenarios. `@pending` / `@manual` scenarios are *filtered out* rather than
skipped, so they should be unaffected — but that must be verified empirically, not assumed.

**Status**: **FIXED 2026-07-16** (user-ratified: fix before slices 03-05 land 28 more scenarios).

**Mechanism**: `.fail_on_skipped()` added to **all four** lane arms in `acceptance.rs`. Chosen over
`skipped_steps() == 0` on evidence, not preference: in cucumber **0.21.1**, `Skipped` is emitted from
exactly ONE site (`runner/basic.rs` — the "no step definition matched" arm), so **skipped ≡ undefined
step**; a panicking step takes the `StepPanicked` branch and `emit_failed_events` emits nothing for
`ExecutionFailure::StepSkipped`, so steps after a failure are never marked skipped. `fail_on_skipped`
also maps `Step::Skipped` → `Step::Failed(NotFound)`, which renders the feature path, line, scenario
and step — a bare count could not name *which* step was undefined.

**Risk cleared empirically before applying**: both lanes reported **0 skipped steps**, so no
currently-green scenario relied on skipping and the change could not alter any passing run.
`@allow.skipped` (fail_on_skipped's own exemption tag) appears nowhere in the repo, so the fix has no
silent escape hatch. `@pending`/`@manual`/etc. are removed by `.filter_run(...)` before events are
generated — confirmed by the 0-skipped counts rather than assumed.

**Proven, not asserted** — the same deliberate break (a step regex typo, `documents` → `docum3nts`)
run against both runners:

| Runner | Result |
|---|---|
| **Before the fix** | `1 scenario (1 skipped)`, `1 step (1 skipped)` — no panic, **exit 0**. The scenario executed NOTHING and the suite reported success. |
| **After the fix** | `✘ Step doesn't match any function` / `Defined: tests/features/us-r07-completion-check.feature:35:5` → panic → **run fails**. |

**No regression**: default lane **514/514 scenarios, 3695/3695 steps**; browser lane **11/11, 79/79**;
fmt/clippy/deny clean.

**Process note**: the crafter dispatched for this work **blocked correctly and was right to**. An
active deliver session arms `SessionGuardPolicy`, which blocks subagent writes to `src/`/`tests/`
without a DES-monitored signal; the orchestrator's `<!-- DES-ENFORCEMENT : exempt -->` marker does NOT
grant exemption from that control (it is read only by `des_enforcement_policy.py`, which gates step-id
Task validation — the write guard in `pre_write_handler.py` never looks at it). When one subagent write
slipped through while an equivalent one was blocked, the crafter treated it as a racy signal leak
rather than permission and refused to permute delegation shapes until one landed — correctly declining
to evade the guard by trial and error. It delivered the complete API analysis and risk clearance; the
orchestrator applied the four-line change and ran the before/after demonstration it was blocked from
performing.

---

## UI-5 — Guard 1 (IME) is unfalsifiable by the entire automated suite (found at step 02-02)

**Source**: `design/adr-002-guard-predicate.md` guard 1 vs the scenario
`A key delivered mid IME composition does not fire a shortcut`.

Step 02-02 ran the falsification honestly and reported the result rather than the hoped-for one:

| Broken | Result |
|---|---|
| guard 2 + `event.shiftKey` | `@shift` **REDS** ✅ |
| guard 2 removed | `@modifier` **REDS** ✅ |
| **guard 1 removed entirely** | `@ime` **STAYS GREEN** ❌ — and so does the full 11-scenario lane |

**Why**: the scenario sends `c` mid-composition into the **title field**. But `c` in a text field is
already inert via **guard 4** (`isTextEntry && isConsumableByTextEntry("c")` — `c` is a character, so
the field consumes it). The scenario therefore cannot distinguish guard 1 from guard 4. The behaviour
it asserts is real and *is* protected — but **not by the arm the scenario names**. The crafter verified
guard 1 was genuinely deleted (not a no-op edit) before concluding.

**Guard 1 only bites for a key that guard 4 lets through mid-composition** — i.e. a key a text field
cannot consume. After UI-3's narrowing that set is essentially **`Escape`**, which produces no
character and which an IME uses to **cancel composition**. An `Escape` arriving mid-composition should
cancel the composition, NOT close the modal — that is the scenario that would actually falsify guard 1.

**Resolution owner**: DISTILL — changing the scenario's key is an acceptance-design decision, not a
crafter's. Residual real-IME risk continues to be carried explicitly by the `@manual` scenario per
ADR-007 honest limit 1 (WebDriver `send_keys` cannot produce real composition; 02-02 drives it via a
JS-dispatched `CompositionEvent` + `KeyboardEvent{isComposing:true, keyCode:229}`, and the Gherkin
marks it as simulated).

**Status**: OPEN. Guard 1 is correct and should stay (it protects Mei's real IME); it simply has no
automated proof. Do not delete it on the strength of "no test covers it" — that would be the wrong
lesson from this finding.

---

## UI-6 — `closeSearch()` does not bump `searchSequence`: a real intermittent red, not a theoretical race (found at 05-01)

**Source**: `crates/foundry-app/static/js/keyboard.js` — `closeSearch()` vs `runSearch()`'s monotonic
request token (added at 04-01 to make last-request-win).

04-02's crafter flagged this as a possible edge and — correctly, under "no code without a requiring
test" — left it, reporting it as theoretical: *"no test requires it and it never fired across many
runs"*. The orchestrator repeated that framing to 05-01. **Both were wrong, and 05-01 proved it.**

`closeSearch()` hides the panel and clears its results but does **not** bump `searchSequence`. An
in-flight `runSearch` fetch therefore still considers itself current when it lands, and re-mounts
results into the now-hidden panel. Slice 04's **shipped, un-`@pending`'d** scenario
`Escape leaves search and restores the board` asserts the results are gone, so it **reds** — observed
at roughly **2 runs in 3**:

```
`Esc` hid the panel but left its results mounted…
  left:  <ul class="search-results"><li … data-issue-key="AUTH-2">…
  right: ""
```

**A test did require it all along.** The assertion was already correct and already committed; only the
window was narrow. 05-01 widened it incidentally (its overflow scenario seeds 40 issues, adding
contention) but did not cause it — the defect shipped at 04-01/04-02.

**The lesson worth keeping**: "no test requires it" was a claim about the tests, and it was not
checked against them. This feature exists because a written claim outlived the code it described; a
claim about coverage deserves the same scepticism as a claim about behaviour.

**Fix**: bump `searchSequence` in `closeSearch()` so an in-flight reply is recognised as stale and
discarded — the same mechanism `runSearch` already uses. One line, in the existing token discipline,
not a new special case.

**Status**: assigned to step **04-03** (a repair step, added to the roadmap after the fact).

---

## UI-7 — ADR-005 §3's `j`/`k`-over-search-results is unreachable under ADR-002 guard 4 (found at step 05-04)

**Source**: `design/adr-005-search-surface-and-enter-resolution.md` §3 (and its own §4 Probe) vs.
`design/adr-002-guard-predicate.md` guard 4, as narrowed by UI-3.

**The contradiction, stated once**: ADR-005 §3 requires that *"when the panel is open, `j`/`k` walk
**only** `li.search-result` rows"*, and its Probe spells the flow out: *"press `/`, type `AUTH-2`,
`j`, `Enter`"*. But `/` **focuses the search box** (ADR-005 §2, and the whole point of FR-7), so the
`j` that follows is delivered to a **text-entry context**. Guard 4 makes any key inert there when the
field can consume it, and `j` is a single character. **The `j` never reaches the dispatch table.**

ADR-005 §2 already states the premise in its own words — *"`search` is absent from
`NON_TEXT_INPUT_TYPES`, so this box is a text-entry context to guard 4"* — and relies on it for
AC-04.5 (a `/` typed into the box inserts literally). The same property that makes AC-04.5 work with
no code makes §3 unreachable. Both ADRs are individually correct; they cannot both hold.

**Observed directly at 05-04's RED_ACCEPTANCE, not theorised** — the step definitions are written,
correct, and red on the shared Given:

```
✘ Given Mei has searched the board for "AUTH-2" and selected the result with "j"
    `j` did not move the ring onto the AUTH-2 result row (the ringed row is None, and the
    search box now reads "AUTH-2j").
```

`"AUTH-2j"` is the whole finding: the press was consumed by the box as a character.

**It is not only `j`.** Guard 4's `NATIVE_TEXT_ENTRY_KEYS` also names `Enter`, `Tab` and all four
arrows. So from the focused search box, **no key can drive selection or open a result** — `k`, the
arrows and `Enter` are inert there for the same reason. `Escape` alone is dispatched (UI-3's
narrowing), and it closes the panel. ADR-005's search→select→open flow has no reachable input at all.

**What it blocks** (both returned to `@pending` at 05-04, step definitions retained and red):
- **`Enter from the search results opens the same modal as clicking the board card`** (AC-06.5,
  `@one-open-path @critical`) — the ADR-005 §4 proof.
- **`Enter is a no-op for a found issue that the board does not render`** (the ADR-005 §4 named edge).
  Its *own* Given is green — AUTH-9 seeds in `cancelled` (`0001_init.sql:72` permits it,
  `DEFAULT_COLUMNS` at `projects.rs:49` renders only the other four), search finds it, the board
  renders no card. It is the **shared** "selected the result with `j`" Given that cannot be reached.

**Not resolved here.** Choosing between these amends a locked ADR, which is DESIGN's call:

1. **Blur the box once results arrive, or require a `Tab` out of it** — `Tab` is consumed natively by
   the field, so the browser moves focus and `j` then arrives at `body`. ADR-006 has already ratified
   exactly this shape for the board (*"an AT user must Tab here ONCE before `j`/`k` arrive"* — an
   ACCEPTED COST, carried into the help copy via `SELECTION_INSTRUCTION`). It costs a keystroke and it
   needs the help overlay to say so. It is a UX decision with no ADR today, which is why 05-04 did not
   simply write it into the step definition and green its own scenario.
2. **Narrow guard 4 further, by surface** — e.g. dispatch `j`/`k` when the search panel is open. This
   is a **per-shortcut, per-surface carve-out on the guard path** and is precisely the BR-2 failure
   UI-3's ratification explicitly does **not** authorise (*"if an implementation ends up with
   `if (key === "Escape")` scattered at dispatch sites, that is the failure mode BR-2 forbids"*).
   Recorded so the option is explicit rather than assumed away; it looks wrong.
3. **Navigate the results with a key a text field cannot consume** — the set is essentially empty
   after `NATIVE_TEXT_ENTRY_KEYS`; every plausible candidate is already a caret movement.
4. **Drop `j`/`k`/`Enter` from the search surface** and let the results be pointer-only, with `Esc` to
   leave. Honest, and it makes AC-06.5's cross-surface proof unnecessary rather than unreachable — but
   it concedes a US-06 acceptance criterion.

**Recommendation (not a decision)**: option 1. It is the only one that keeps both ADRs byte-for-byte
— guard 4 is untouched and ADR-005 §3 becomes reachable — and the precedent for its cost is already
ratified one ADR over. Option 2 buys the same behaviour by breaking the crux; option 4 pays for a
sentence with a requirement.

**Note for whoever resolves it**: `Enter`'s **resolution** (ADR-005 §4 — `selectedKey` → the board
card → its own shipped `hx-get`) is **already built and green** on the board surface (step 05-03).
Nothing in ADR-005 §4 is in doubt; only the input path that puts the ring on a result row. The
`@one-open-path` proof's step definitions are written and comparable byte-for-byte against the
pointer's modal — they run the keyboard path, reload, click AUTH-2's card, and assert `#modal-root`'s
markup is identical. They should need no change once the Given is reachable.

**Process note**: 05-04 shipped the one scenario that is genuinely achievable (`@htmx-swap`, with its
shared `htmx:afterSwap` hook and its falsification proven) and did not force the other two. It did not
add a guard carve-out to green its own scenario, and it did not quietly insert a `Tab` into the Given
to manufacture a reachable flow — either would have been the crafter making a design decision inside a
step definition. Same posture as 02-01 with UI-3.
