# Evolution — keyboard-shortcut-bindings (the client layer the help page had been promising for years)

**Finalized**: 2026-07-17
**Commits**: DELIVER `3e3aa84` → `73ebff2` (31 commits; `e7841d0` is the pre-feature baseline) — 15
DES-monitored 5-phase TDD steps across 5 thin slices, plus one out-of-band runner fix (`b93e16b`) and one
repair step added to the roadmap after the fact (`c3a18d1`, step 04-03). Trunk-based; repo legacy multi-file
convention; DES-monitored (5-phase contract, exempt at finalize). Feature dir PRESERVED.
**Wave coverage**: full DISCUSS → DESIGN → DISTILL → DELIVER. DESIGN resolved all nine ODDs into eight
feature-local ADRs. `des-verify-integrity` exit 0; all 15 steps carry complete 5-phase traces
(PREPARE → RED_ACCEPTANCE → RED_UNIT → GREEN → COMMIT). **RED_UNIT is `SKIPPED`/`NOT_APPLICABLE` on all
fifteen** — the feature is 915 lines of browser JS in a workspace with no JS unit harness, and the browser
lane IS the port for the client layer (ADR-007). Recorded, not glossed.
**Scope**: Foundry's help overlay advertised **seven** keyboard shortcuts — `c` create, `/` search, `j`/`k`
navigate, `Enter` open, `?` help, `Esc` close — as a shipped constant (`SHORTCUTS`, `keyboard.rs:48-56`)
rendered into a real `<dl>` the user reads. **None were bound.** The entire client-side keyboard layer was
never written, while the server routes shipped, routed and green. This feature adds it: `static/js/keyboard.js`
— a vanilla, document-delegated, guarded dispatch layer — **plus the instrument that would have caught the
gap** (a real headless-Chrome lane in `cargo xtask ci`). ZERO new routes, ZERO endpoints, ZERO migrations
(latest remains `0014_notification_unsubscribes`), ZERO new app crates. ONE new runtime artifact, ONE new
dev-dependency (`fantoccini`), ONE host prerequisite (chromedriver).

## Milestone — the help page tells the truth. KPI-1 advertised-to-working: 0/7 → 7/7

The user's original request was *"bind the `c` key"*. Investigation found all seven dead. The product did not
have a missing feature; it had **a documented commitment that failed on contact** — and a 100%-green,
port-to-port acceptance suite that **could not press a key** and therefore could not tell.

Worse: **the absence was decided, not missed.** `us-12-keyboard-nav.feature:18-23` carried a recorded
"no-Playwright decision" putting key handling *"OUT of automated scope"* and describing handlers living
*"in alpine.js"* — code that was never written. `keyboard.rs`'s own module doc opened *"Three routes that back
the alpine.js keyboard-shortcut handlers"*. The decision named a **tool** and delivered the absence of a
**capability**, then an `@manual` QA drill (`:87-95`) that evidently never ran or never failed stood in for it.

**The recurring disease, named once and then found everywhere: a written claim outliving, or preceding, the
code it describes.** It was in the module doc, in the feature file, in `projects.rs:882`'s *"the alpine.js j/k
handler walks"*, in `keyboard.js:16`'s own "bound == advertised by construction", in ADR-001's precondition, in
ADR-002's guard, in ADR-005 §3, in ADR-008's trap-B mechanism — and, at the end, in the adversarial review of
this very feature. This archive is mostly about that.

## What shipped

| Slice | Steps | Commits | Delivered |
|---|---|---|---|
| 01 — the instrument proof: browser lane + dispatch layer + `?` + `Esc` | 01-01, 01-02, 01-03 | `3e3aa84`, `c3b6db0`, `10dba43` | `BrowserHarness` = `InProcHarness` (unchanged) + a fantoccini session → `base_url()`; the `@needs-browser` lane; `keyboard.js` as a vanilla document-delegated IIFE with `[data-kb-ready]`; `?` renders the shipped public `/keyboard-help` fragment into a JS-created `#kb-overlay-root`; `Esc` peels the topmost layer. **Alpine retired** (33 sites, `alpine.min.js` deleted) and two shipped contracts amended. KPI-1 0/7 → **2/7** |
| — | — | `b93e16b` | **The runner refuses undefined steps instead of skipping them** (`.fail_on_skipped()` on all four lane arms) — UI-4, fixed out-of-band before slices 03-05 landed 28 more scenarios |
| 02 — the guards (no new key bound; capability deliberately zero) | 02-01, 02-02 | `a72e1b0`, `d63a64e`, `77f986a` | ADR-002's four-step chain evaluated ONCE before dispatch: composition (`isComposing \|\| keyCode===229`) → modifier (`ctrl\|\|meta\|\|alt`; **Shift is not a suppressor** — `?` IS `Shift+/`) → `defaultPrevented` → text-entry. Guard 4 **narrowed** to `isTextEntry(target) && isConsumableByTextEntry(key)` (UI-3). KPI-2 |
| 03 — `c` files an issue; `Esc` peels one layer per press | 03-01, 03-02 | `f3a8b90`, `20aea2a` | `c` drives the shipped htmx new-issue path via the board's OWN `hx-get` — **zero client CSRF code written**, exactly as DD3 predicted. Layered `Esc`: help closes over a still-open modal, proving ADR-003's two-host split. KPI-1 → **4/7** |
| 04 — `/` reveals a board search panel | 04-01, 04-02, **04-03** | `3cee6f0`, `7cad86c`, `c3a18d1` | `/` reveals + focuses a JS-injected panel (plus a pointer-clickable control, so `/` is an accelerator not the only path) and `preventDefault()`s its own slash — the classic stray-slash bug, reproduced under mutation. `Esc` hides it. **04-03 is a repair step** for UI-6's real intermittent red. KPI-1 → **5/7** |
| 05 — `j`/`k`/`Enter`, a11y, and the retirement | 05-01..05-05 | `2c889be`, `9a7f5da`, `0d5fc40`, `b5d8e44`, `b905fa1`, `03560e9` | Key-based `selectedKey` + derived ring; ARIA composite (`role=listbox/option`, `tabindex=0`, `aria-activedescendant`) re-applied on `htmx:afterSwap`; `Enter` resolves `selectedKey` → the board card → **that card's own shipped `hx-get`**; `Tab`-from-search-box (UI-7); `#kb-items` retired whole. KPI-1 → **7/7 — the promise kept** |
| — | — | `73ebff2` | **Comments that outlived their code, corrected.** The us-12 no-Playwright paragraph and its superseded `@manual` drill deleted; the "alpine.js handlers" credits corrected. Historical notes deliberately kept — *the distinction is tense: recording that something was once true is honest; asserting it still is, is the bug* |

### The guard chain (ADR-002 — the crux)

One function, evaluated once, with `dispatch` unreachable around it. **The narrowing that matters** (UI-3):
guard 4 is `isTextEntry(event.target) && isConsumableByTextEntry(event.key)`, where consumability is a
**platform fact** — `key.length === 1` (the key produces a character) or membership in
`NATIVE_TEXT_ENTRY_KEYS` (`Enter`, `Tab`, `Backspace`, `Delete`, `Insert`, the four arrows, `Home`/`End`,
`PageUp`/`PageDown` — keys a text input acts on natively). Unknown key types fail **closed**.

**`Escape` is never named on the guard path.** It dispatches from a text field because it satisfies neither
arm — it produces no character and a text input does nothing with it. That is what makes this a property of
the predicate's **domain** rather than a per-shortcut carve-out, and it is why BR-2 still holds:
`grep '"Escape"'` on the guard path returns nothing; the only `key === "Escape"` in the file is a branch of
the **dispatch table**, where naming keys is the entire job.

### The layer stack (ADR-003)

`#modal-root` exists only at `board.html:13` and htmx swaps it with `innerHTML`, so rendering `?` into it
would **destroy an open new-issue modal** — but US-07 requires one `Esc` to close help with the modal still
open. Help therefore gets its own JS-created `#kb-overlay-root` appended to `<body>` on any page; zero
template delta. The stack is **derived from the DOM at `Esc` time, never stored** — a stored array is exactly
what an htmx swap desyncs. The full-page `/keyboard-help` links stay: they are the no-JS path, and the
dead-code policy does not reach live consumers.

### Key-based selection (ADR-004)

`selectedKey: string | null`. Not an index (a drag silently re-points it at a **different issue** — the user
acts on the wrong card: disqualifying) and not a node ref (an htmx re-render detaches it). The ring is
**derived**, re-projected on `htmx:afterSwap`. Drag coexistence, "`Esc` never clears selection", and "resets
on navigation" all cost **zero code** — they fall out of the representation. `board-dnd.js` is untouched.

### `Enter` resolution (ADR-005)

Search results carry **only** `.key` + `.title` — no `hx-get`, no `edit_url` (`search_results.html:4`). DISCUSS
did not catch this; US-06's stated mechanism was **unimplementable on a result row**. Resolution: `Enter` maps
`selectedKey` → `article.issue-card[data-issue-key=K]` → activates **that** card's shipped `hx-get`. One rule,
both surfaces, possible because the panel **overlays** the board rather than replacing it — and it honours the
requirement's *intent* (one open path, converged with the pointer) more strictly than the mechanism the
requirement literally named. Adding `edit_url` to the search view-model would have breached zero-server-delta
and "exactly one open path" simultaneously.

### The browser lane (ADR-007 — the root-cause fix)

`fantoccini` + chromedriver against `InProcHarness::base_url()`. **The harness already served a real TCP
origin** — `TcpListener::bind("127.0.0.1:0")` + `axum::serve`; "in-process" meant *same OS process*, not *no
socket*. The premise that a browser needed new serving plumbing was **false**, which made ODD-9 far cheaper
than DISCUSS priced it and keeps ONE app-construction path so the two lanes cannot diverge.

**Probe, then refuse — never skip.** A lane that silently skips on a missing chromedriver recreates the exact
failure mode the feature exists to close. The lane probes (session up, key round-trips, `[data-kb-ready]`
appears, **and Mei is still signed in after a real browser handles the `Secure` cookie over plain HTTP** — the
harness only ever inspected the header text, never whether a browser would send it back) and fails loudly with
an install hint. Waits are conditions, never sleeps.

## Decisions realized (ADRs)

| # | Decision | Status |
|---|---|---|
| ADR-001 | Vanilla document-delegated IIFE (the `board-dnd.js:67` idiom); `[data-kb-ready]`; **drop Alpine**; correct the stale doc | IMPLEMENTED — **precondition amended by DELIVER (UI-1)** |
| ADR-002 | The four-step guard chain, `isTextEntry`, IME `isComposing`+`keyCode 229`, Shift excluded as a suppressor | IMPLEMENTED — **guard 4's domain narrowed by DELIVER (UI-3)**; decision stood, wording was wrong |
| ADR-003 | JS-created `#kb-overlay-root` (forced by BR-4's layering); DOM-derived `Esc` stack; keep the no-JS help links | IMPLEMENTED — unamended |
| ADR-004 | `selectedKey` by issue-key; ring as derived state; drag + swap coherence for free | IMPLEMENTED — unamended |
| ADR-005 | Board-only; injected search panel; modal navigation with shared selection identity; `Enter` via the board card | IMPLEMENTED — **§3 was unreachable under guard 4; resolved by an explicit `Tab` (UI-7)**. §4's resolution was never in doubt |
| ADR-006 | `aria-activedescendant` on a focusable ARIA composite; NOT a live region (browse-mode interception); the ratified Tab cost | IMPLEMENTED — its ratified `Tab` cost became the precedent UI-7 leaned on |
| ADR-007 | fantoccini + chromedriver; reuse `InProcHarness`; `@needs-browser` in `cargo xtask ci`; probe-then-refuse; **reverses the recorded no-Playwright decision** | IMPLEMENTED — and §4's own failure mode was found living inside the runner (UI-4) |
| ADR-008 | Retire `#kb-items` whole — 13 verified sites, 2 feature files, 2 traps | IMPLEMENTED — **trap B's stated mechanism proved INVERTED by measurement; ADR needs that correction** |
| — | KPI-4 is met **conditionally on one Tab to the board** (Option A, ratified 2026-07-15; D-4 stands). The qualifier travels with every KPI-4 claim, and the help overlay's own copy says so | IMPLEMENTED |
| — | ZERO routes, ZERO endpoints, ZERO migrations, ZERO handler-behaviour change | IMPLEMENTED |

## Seven defects found during execution — several inside the ADRs. Zero were found by the three prior reviews.

Crafters **blocked five times**, and **every block was correct**. Each time the step's only two paths were
both violations (write code with no requiring test, or weaken a test to force a pass), and each time it
escalated instead. That is the single most reusable fact in this archive.

**UI-1 — ADR-001's "zero Alpine consumers" was false (01-03).** The ADR's grep searched for Alpine
*directives in templates* — of which there were genuinely zero — but never for **contract tests asserting the
vendored asset is served**. Three existed, all green, none `@pending`, including a **shipped
`@walking_skeleton`**. Alpine had zero **runtime** consumers and three **contract** consumers. The distinction
the ADR missed is between *"nothing uses it"* and *"nothing asserts it exists"*. Retired anyway (33 sites) and
the two shipped contracts amended: they asserted a dependency that does nothing — keeping them green would
preserve 44 KB of dead framework on every page load **in order to satisfy an assertion about its own
presence**. The conclusion stood; only the precondition was wrong, and the correction *widened* the blast
radius rather than reversing it. Step 01-03 logged its four phases `SKIPPED / BLOCKED_BY_DEPENDENCY` rather
than marking them EXECUTED to satisfy a hook demanding recovery — which would have been the exact audit fraud
the DES rules forbid. Zero source files modified.

**UI-2 — a roadmap step claimed a scenario unpassable at its slice's scope (01-02).** *"The overlay lists
exactly the seven advertised shortcuts and each is bound"* requires 7/7; slice 01 binds 2/7. Moved to 05-05.
**The roadmap review verified every scenario had a step, but not that each step's scenarios could pass at that
step's slice scope** — worth adding to the roadmap quality gate.

**UI-3 — ADR-002 contradicted itself (02-01).** Guard 4 made every key inert in a text field *"with no
exemptions"*. But `Esc` is one of the seven, `keyboard.rs:75` advertises it as literally **"Close modal"**, and
**every modal Foundry renders autofocuses its title input**. So **`Esc` could never close the modal it just
opened**. Step 02-01 implemented the chain exactly as specified and observed the wall directly rather than
theorising it. This was slice 02's learning hypothesis **resolving as designed**: the slice named the disproof
condition in advance (*"if any of the seven turns out to need a bespoke carve-out"*) and pre-committed the
response (*"the honest response is **not** to ship carve-outs"*). The crafter honoured it and did not green its
own scenario with an exemption. Resolved by narrowing the guard's *domain* — a predicate property, not a
carve-out.

**UI-4 — the acceptance runner was GREEN over undefined steps (02-02).** `execution_has_failed()` counted
failed steps, parsing errors and hook errors — and nothing else. cucumber-rs reports a step with **no matching
step definition** as *skipped*. So an un-`@pending`'d scenario with unwired steps **executed nothing and the run
exited 0**. **This is ADR-007 §4's own failure mode ("probe, then refuse — NEVER skip") living inside the runner
built to prevent it. The instrument had the disease it was built to diagnose.** Urgent, not academic: slices
03-05 were about to un-`@pending` 28 more scenarios, and a regex typo was enough.

Proven, not asserted — the same deliberate break (`documents` → `docum3nts`) against both runners:

| Runner | Result |
|---|---|
| Before | `1 scenario (1 skipped)`, `1 step (1 skipped)` — no panic, **exit 0**. The scenario executed NOTHING and the suite reported success. |
| After `.fail_on_skipped()` | `✘ Step doesn't match any function` / `Defined: …us-r07-completion-check.feature:35:5` → panic → **run fails**, by name. |

Risk cleared **empirically** before applying, not assumed: both lanes reported **0 skipped steps**, so no
green scenario relied on skipping; `@pending`/`@manual` are removed by `.filter_run(...)` before events are
generated; `@allow.skipped` (the fix's own escape hatch) appears nowhere in the repo. It caught **three real
unwired steps on its first outing**.

**UI-5 — guard 1 (IME) is unfalsifiable by the entire suite. OPEN.** Deleting it leaves `@ime` green — and the
whole lane — because `c` mid-composition is already inert via guard 4. The behaviour the scenario asserts is
real and **is** protected, but **not by the arm the scenario names**. Guard 1 only bites for a key guard 4 lets
through mid-composition — after UI-3's narrowing, essentially `Escape`, which an IME uses to **cancel
composition**; that is the scenario that would actually falsify it. **Guard 1 is correct and stays. "No test
covers it" is grounds for a better test, not for deletion** — that would be the wrong lesson from this finding.

**UI-6 — a race twice dismissed as "theoretical" was intermittently red on `main` (05-01).** `closeSearch()`
hid the panel but did not bump `searchSequence`, so an in-flight `runSearch` reply still considered itself
current and re-mounted results into the now-hidden panel. Slice 04's **shipped, un-`@pending`'d** scenario
already asserted the results were gone. Measured: **4 reds / 26 runs** on the tree as shipped; reverting the
fix: **5/12**; with it: **0/10**. **"No test requires it" was a claim about the tests, and nobody checked it
against the tests.** This feature exists because a written claim outlived the code it described; **a claim
about coverage deserves the same scepticism as a claim about behaviour.** The fix is one line inside the
existing token discipline, not a new special case.

**UI-7 — ADR-005 §3 was unreachable under guard 4 (05-04).** `/` **focuses the search box**, so the `j` that
follows is delivered to a text-entry context and consumed as a character — the RED_ACCEPTANCE output
`the search box now reads "AUTH-2j"` is the whole finding. And it is not only `j`:
`NATIVE_TEXT_ENTRY_KEYS` also names `Enter`, `Tab` and the arrows, so **no key could drive selection from that
box at all**; `Escape` alone dispatched, and it closes the panel. Both ADRs were individually correct and could
not both hold. Resolved by an explicit `Tab` — **the cost ADR-006 had already ratified for the board** — with
guard 4 untouched and both ADRs byte-for-byte. The rejected alternative (dispatch `j`/`k` when the panel is
open) is a per-shortcut, per-surface carve-out on the guard path: precisely the BR-2 failure UI-3's
ratification explicitly refuses to authorise.

## The single most important finding — a green can be an artefact of the instrument

UI-7's resolution first chose **blur-on-results** (the system doing the `Tab` on Mei's behalf rather than
charging her a keystroke). It was implemented **exactly as ratified**, and **the batched search lane ran 6/6
GREEN**.

It was measured anyway. **At a human's typing pace, blur-on-results turns the query `and/or` into `ao`.** The
box blurs the instant the FIRST character's results land, so every later keystroke goes to `body`: `n` and `d`
fell on the floor, the `/` was **dispatched as a shortcut** (re-opening and re-focusing the box), and only the
trailing `o` reached the field. Not merely "fighting incremental typing" — **AC-04.5 destroyed** (a typed `/`
must stay literal), and the query unreachable past its first character.

**The lane could not see it, and that is the finding worth keeping.** WebDriver's `send_keys("cookie")` types
all six characters in a single command with no network round-trip between them, so the fetch for `"c"` lands
*after* typing finishes. **The batched lane structurally cannot observe a blur that strands a human.** The
defect was found only by typing at **150ms/char**. The green was an artefact of the instrument, not evidence
about the product.

So the probe was kept: `mei_types_into_search` types one character per 150ms at `active_element()` — never
re-finding the box, since a `find(box).send_keys(..)` per character would re-focus it and paper over exactly
the defect being hunted. Reverting it would restore a scenario that cannot falsify the design we just
rejected. The fallback clause's condition was met; `Tab` it is; no carve-out was bought. And the cost is
written down **where the user meets it** — `SELECTION_INSTRUCTION` gained a second sentence, rendered through
the same `p.kb-selection-instruction` element ADR-006's board `Tab` already uses, asserted by a scenario that
reads the **rendered overlay** rather than the Rust constant (a constant the template forgot to render would
satisfy any assertion made against the string). *A cost that is not written down is a cost the user pays
twice.*

## ADR-008's trap B — the ADR's mechanism was inverted, and the experiment found it

05-05 ran the trap-B experiment rather than trusting the ADR. ADR-008 claimed that leaving
`let visible = html.split(r#"id="kb-items""#).next().unwrap();` after deleting the carrier would make
`each_issue_lands_in_exactly_its_state_column` *"pass vacuously, blind to cards leaking outside their
columns"* — and named it *"the trap worth naming loudest"*, on the grounds that leaving it would reproduce
this feature's own disease inside the change that removes it.

**It does not survive measurement.** With the carrier absent, `split(needle).next()` returns the **whole page**
— which is exactly what the repoint returns. The two forms are **behaviourally identical**. Proved by leaking
AUTH-1 into every column and running both: **BOTH red, same assertion**. The old slice narrowed the last
column's region, so **removing the carrier *widens* the test rather than blinding it**. The repoint is still
correct hygiene — dead code, a comment that lied, and a latent re-narrowing the day any element takes that id
— **but it was never a vacuity fix**. ADR-008 needs that correction so the next reader does not inherit the
wrong mechanism.

## Two tests that could not falsify, strengthened

- **`@drag-coexistence` (05-02)** stayed green with an **index-based** implementation — the very
  representation ADR-004 rejected as disqualifying.
- **`Enter`-in-a-form (05-03)** stayed green with its guard **deleted**.

Both were caught **by running the mutation, not by review**. Both are now red under their mutation. Same
posture as 05-04's 150ms probe: a scenario that cannot fail is not evidence.

## Mutation testing — the gate was structurally inapplicable, and that was recorded rather than faked

`cargo-mutants` mutates **Rust**. The feature is **915 lines of `keyboard.js`**; the Rust delta is ~41 net
logic lines of plumbing, a `const`, and the `#kb-items` **deletions**. Running the gate would have reported a
percentage over the plumbing while the 915 lines that *are* the feature went entirely untested. **That number
would have been worse than no number: it would have looked like assurance.** Given this feature exists because
a green suite stood over absent code, publishing a meaningless kill rate would have been the same error in a
new costume.

**Mutation testing was performed instead by hand, per step, against the real code — and it found surviving
mutants.** 21 mutations recorded; the JS ones are reach `cargo-mutants` does not have. Highlights: deleting the
`<script>` tag reds the lane probe; rebinding `?` → `&` reds all four slice-01 scenarios; stubbing guard 4 reds
the paired assertion; adding `shiftKey` to guard 2 reds `@shift` (it catches the obvious wrong
implementation); removing `/`'s `preventDefault()` reds AC-04.1 (the classic stray-slash bug, reproduced);
a half-hook (ring only, no ARIA) reds both `@htmx-swap` and `@a11y`; `Enter` opening the *correct* issue by a
second path still reds `@one-open-path` (the byte-identical modal comparison).

**Survivors, recorded not hidden**: guard 1 (UI-5, OPEN), plus the two that survived until the tests were
strengthened. If a Rust-side mutation gate is ever wanted here, the honest target is the **acceptance step
definitions and the `xtask` preflight**, not `keyboard.js`; Stryker would be the real instrument for the 915
lines, and adding a JS toolchain is a project decision, not a step's.

## Tooling defects fixed en route

- **`des-init-log` omitted `project_id`**, deadlocking every DES subagent at stop. Patched in **both** copies
  of `init_log.py`. **A `pipx upgrade nwave-ai` reverts it.**
- **chromedriver from brew ships the *latest* driver** and skews against the installed Chrome. The `xtask`
  preflight now asserts a **major-version match**, not mere presence — the same posture as
  `pg_dump_at_least_16()`.

## Deviations (recorded honestly)

1. **Zero unit tests; RED_UNIT `NOT_APPLICABLE` on all 15 steps.** The production delta is browser-resident JS
   and the workspace has no JS unit harness (no `package.json`/vitest/jest); adding one is a project decision,
   not a step's. The `@needs-browser` lane is the port-to-port test at that boundary. Every step recorded the
   rationale explicitly and proved falsification by experiment instead. The only Rust deltas were **removals**
   and view-model plumbing.
2. **Step 01-03's no-JS scenario was green-by-inheritance, disclosed.** It passes on the pre-removal tree
   because the shipped sidebar link + public `/keyboard-help` route already satisfy the no-JS path (ADR-003
   keeps them deliberately). It is a **regression guard for the Alpine subtraction**, not a driver of new
   production code. No business-logic RED was observed, so falsification was proven separately (removing the
   sidebar link reds it: 4 passed/1 failed; restoring returns 5/5). Honest caveat also recorded: the probe
   failed at the Given (`.board` absent) rather than the targeted When, so it proves the scenario detects the
   path's removal but is **imprecise about where** — not fully explained.
3. **DESIGN's peer review was never run.** `wave-decisions.md` records the gate as NOT RUN and recommends
   running it before DISTILL, *"with particular attention to ADR-006 and ADR-007 — the two decisions with the
   widest blast radius"*. It was not run. **Of the seven upstream issues, five were defects inside DESIGN
   artifacts** (ADR-001's precondition, ADR-002's guard, ADR-005 §3, ADR-008's trap-B mechanism, and the
   roadmap's slice-scope conflict). Whether a Haiku reviewer would have caught any is unknowable; that the
   gate was skipped is not.
4. **The adversarial review of this feature made the feature's own error.** Verdict:
   **approved_with_findings**. Its one MEDIUM finding (D1 — stale alpine prose in `projects.rs`) was **verified
   a FALSE POSITIVE**: it cited **UI-1's *historical* site list** as if it were current code. Zero hits on the
   actual tree. Worth recording plainly — the review of a feature about claims outliving code cited a claim
   that had outlived its code.
5. **One stale comment knowingly left.** `feature_mwt_slice_04_non_enumerability.rs:390` still says *"a real
   browser's alpine/htmx hook"*. It belongs to another feature and the deliver-session write guard blocked the
   edit; **left rather than routed around**.
6. **AC-05.6's grep litmus is not a raw grep, and cannot be.** *"Zero hits under `crates/`"* is unsatisfiable by
   construction — the litmus's own scenario lives under `crates/` and must name what it searches for. It reads
   every carrier-bearing file (`rs`/`html`/`js`) and demands zero there, with needles assembled via `concat!`
   so the litmus is **inside its own search** rather than excluded from it. Gherkin prose is out of scope
   because it can carry no field, markup or selector — which costs nothing and is checkable. It red on `main`
   against 33 sites, **including this change's own first drafts**.

## Still open (tracked, not closed)

- **UI-5 — guard 1 (IME) has no automated proof.** OPEN. Retargeting the scenario at `Escape` (which an IME
  uses to cancel composition) is **DISTILL's** call. **Do not delete the guard.** Residual real-IME risk stays
  carried by the `@manual` drill per ADR-007's honest limit 1 — WebDriver `send_keys` cannot produce real
  composition; the automated scenario drives a JS-dispatched `CompositionEvent` +
  `KeyboardEvent{isComposing:true, keyCode:229}`, and the Gherkin marks it as simulated.
- **ADR-008's trap-B mechanism is inverted** and still reads that way in the ADR. Correct it or the next reader
  inherits it.
- **The search panel is unstyled.** `keyboard.js` injects `#kb-search-panel`, but the stylesheet
  (`foundry.eb0e86f8.css`) carries exactly one feature rule — `.kb-selected`. The panel and the overlay host
  have no CSS. Functional, not finished.
- **Two known lane flakes**: leaked postgres containers, and the no-JS scenario at roughly **2-3 in 10**.
- **Search has no no-JS path** — the route returns a bare fragment with no full-page fork. **Nothing
  regresses** (the pointer-clickable Search control is injected alongside), and this feature does not create
  one. A full-page search fork is a recommended follow-up, out of scope.
- **KPI-4 is met CONDITIONALLY on one Tab to the board.** The qualifier is not a footnote; a bare "KPI-4 met"
  is a misstatement of what shipped. Eliminating the cost requires reopening D-4 for roving tabindex; ADR-006
  already makes that case for whoever revisits it.
- **A JS mutation tool (Stryker)** would be the real instrument for the 915 lines. Out of scope; recorded
  rather than silently skipped.

## Verification

- **KPI-1 (north star)**: advertised-to-working **0/7 → 7/7**. KPI-2 (typing never captured), KPI-4 (a11y,
  *conditionally on the Tab*), KPI-5 (bound == advertised) all hold as revert-reds-it litmuses.
- **Browser lane**: **38/38 scenarios, 263/263 steps** against real headless Chrome 150 (fantoccini +
  chromedriver). The feature file carries 40 scenarios: those 38, plus the `@grep-litmus @real-io` retirement
  proof (which runs in the **default** lane — it greps the source tree, not a browser), plus one
  `@pending @manual` drill (the real-IME residual), excluded from every lane by design.
- **Default lane**: **514/514 scenarios, 3692/3692 steps** (was 3695 — three steps net removed with the
  carrier). Unit: 55/55.
- **Gates**: `cargo fmt --all --check`, `cargo clippy --all-targets --release -- -D warnings`,
  `cargo deny check`, `cargo xtask check-arch` — all clean.
- **DES**: all **15** steps have complete 5-phase traces; `des-verify-integrity` **exit 0**. Three steps were
  re-dispatched after a correct block (01-03, 02-01, 05-04).
- **Cost**: ZERO routes, ZERO endpoints, ZERO migrations (latest remains `0014_notification_unsubscribes`),
  ZERO new app crates, ZERO production infra. ONE runtime artifact (`keyboard.js`, zero dependencies), ONE
  dev-dependency (`fantoccini`), ONE host prerequisite (Chrome + a version-matched chromedriver on `PATH`,
  enforced by the `cargo xtask ci` preflight exactly as the PostgreSQL 16 client is). Alpine **deleted** —
  44 KB off every page load, and the tree's only `unsafe-eval`-shaped dependency gone. **Production is
  untouched.**
- **Litmus**: `alpine` and `kb-items`/`kb_items` return zero hits in code on the finished tree; the only
  remaining mentions are deliberate Gherkin history, a `16-alpine` testcontainers image tag, and the one
  disclosed cross-feature comment (deviation 5).
- **Finalize**: feature dir PRESERVED (wave matrix). Trunk-based — commit performed by the orchestrator; no PR.

## Lessons

1. **A green can be an artefact of the instrument.** The origin (a port-to-port suite that cannot press a
   key), UI-4 (a runner green over undefined steps), UI-6 (a race the tests already forbade), and the
   blur-on-arrival 6/6 are the *same finding* in four costumes. Ask what the instrument is structurally
   incapable of seeing before reading its verdict as evidence.
2. **A claim about coverage deserves the same scepticism as a claim about behaviour.** "No test requires it"
   and "zero consumers" and "passes vacuously" were all claims about the tests. All three were wrong. All
   three were checkable in minutes. **Run the experiment; do not trust the ADR — not even your own.**
3. **The distinction is tense.** Recording that something was once true is honest; asserting it still is, is
   the bug. That is why `73ebff2` deleted the no-Playwright paragraph but *kept* the historical notes.
4. **Escalate; do not green your own scenario.** Five blocks, five correct. Every one had an available
   shortcut — an `Esc` exemption, a guard carve-out, a `Tab` quietly inserted into a Given, an assertion
   narrowed to the implemented subset — and every one was a design decision being made inside a step
   definition. The learning hypotheses that named their own disproof conditions **in advance** are what made
   refusing cheap.
5. **A precondition is not a conclusion.** ADR-001's conclusion (drop Alpine) was right; its precondition was
   false. UI-1 *widened* the change rather than reversing it. Check the two separately.
6. **Narrow the domain, don't carve out the case.** UI-3's resolution works because consumability is a
   **platform fact** about the key, not a fact about our shortcut list. `Escape` dispatches because it
   satisfies neither arm — **never because it is named**. That is the difference between a structural predicate
   and seven scattered `if`s, and it is what let UI-7 be resolved without touching the guard at all.
7. **A cost that is not written down is a cost the user pays twice.** ADR-006's Tab and UI-7's Tab both reached
   the help overlay's own copy, asserted against the **rendered** DOM — because a constant the template forgot
   to render satisfies any assertion made against the string.
8. **The roadmap quality gate should verify that each step's scenarios can pass at that step's slice scope**,
   not merely that every scenario has a step (UI-2).
9. **Do not publish a metric your tooling cannot honestly produce.** The mutation gate was structurally
   inapplicable; reporting a kill rate over 41 lines of plumbing would have been assurance theatre over the
   915 that mattered.
</content>
