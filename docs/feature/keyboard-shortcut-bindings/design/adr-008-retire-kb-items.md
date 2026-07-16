# ADR-008: Retire `#kb-items` whole — the verified delta (ODD-1)

## Status
Accepted — 2026-07-15 (Morgan, DESIGN wave). Feature-local. Resolves **ODD-1** (blocking, slice 05) and
**Risk R1**. Confirms DISCUSS **D12**. **Deliberately deletes currently-green tests.**

## Context
`board.html:12` emits `<ul id="kb-items" hidden aria-hidden="true">` with one `li[data-issue-key]` per
issue, sorted **ascending by issue number across all columns** (`projects.rs:881-891`, view-model
`views.rs:253-256`). The intent comment (`projects.rs:881-885`) is explicit:

> *"the visible board renders most-recent-first (DESC); the alpine.js j/k handler walks this hidden list …
> so pressing `j` moves 'to the next-older issue' consistently no matter which column the user is in."*

The suite calls it *"the source of truth for the keyboard navigation order"* (`us_12_keyboard_nav.rs:339-341`)
and asserts the ordering — **green today**.

The locked model (D-4) says the opposite: `j`/`k` walk **visible cards in DOM order**, ring, `scrollIntoView`.
They cannot both hold:
1. **Order differs.** `#kb-items` is flat ASC-by-number; the visible board is column-grouped and
   DESC-within-column (`projects.rs:864-879`). Walking one is observably not walking the other.
2. **A `hidden` element cannot take a ring or be scrolled to.** A ring renders nothing; `scrollIntoView`
   does nothing. The locked model *requires* the visible `.issue-card`.
3. **`aria-hidden="true"`** hides it from assistive tech, so it cannot carry the ADR-006 story either.

And the decisive fact: **it has zero browser consumers, and always has.** The handler it was built for was
never written. It is dead on arrival. AGENTS.md: *"Remove dead/legacy code outright — do not leave it
inert."*

## Decision
**Delete it whole.** Confirmed, and verified site-by-site rather than inherited from the DISCUSS map —
**which was incomplete in three ways** (below).

| # | Site | Action |
|---|---|---|
| 1 | `foundry-app/templates/board.html:12` | Delete the whole line (the carrier) |
| 2 | `foundry-app/src/projects.rs:881-891` (+ blank 892) | Delete the builder: comment, `sorted_issues`, `sort_by_key`, the `kb_items` map/collect |
| 3 | `foundry-app/src/projects.rs:900` | Delete the `kb_items,` field from the `BoardPage { … }` literal (893-902) |
| 4 | `foundry-app/src/projects.rs:794` | Prose: `build_board_page`'s doc mentions "the hidden `#kb-items` ASC carrier" |
| 5 | `foundry-app/src/views.rs:253-256` | Delete the doc comment + `pub kb_items: Vec<String>,` from `struct BoardPage` |
| 6 | `foundry-app/src/views.rs:9` | Prose: module doc lists `#kb-items` among render-contract markers |
| 7 | `foundry-app/src/projects.rs:1037-1075` | **Not deletable whole** — see trap A |
| 8 | `foundry-app/src/projects.rs:1086-1134` | **Do not delete** — see trap B (the vacuity trap) |
| 9 | `foundry-acceptance/src/steps/us_12_keyboard_nav.rs:331-362` | Delete `#[then(…ascending issue-number order…)]` + `async fn data_issue_key_ascending_order` whole |
| 10 | `foundry-acceptance/src/steps/us_12_keyboard_nav.rs:11-14` | Delete the module-doc bullet describing the `<ul id="kb-items">` contract (it is the 4th of 4) |
| 11 | `foundry-acceptance/src/steps/feature_b_web_tier.rs:563-581` | Delete step fn `board_kb_order` **whole** (the attribute at 563-565 through the body; DISCUSS cited only the 568-572 core) |
| 12 | `foundry-acceptance/tests/features/us-12-keyboard-nav.feature:50` | Delete **only** line 50. Steps 46-49 assert `data-issue-key` presence generally and survive; the scenario (44-50) stays |
| 13 | `foundry-acceptance/tests/features/us-b01-styled-board.feature:60-66` | Delete the scenario **whole** ("The board preserves the hidden keyboard-navigation order") — its only `Then` is the carrier assertion, so nothing remains |

**Three corrections to the DISCUSS map**, each verified:

- **There are TWO feature files, not one.** DISCUSS named the two Rust step files but not the Gherkin.
  `us-b01-styled-board.feature:60-66` is a whole scenario nobody had counted; `us-12-keyboard-nav.feature:50`
  is a single step inside a scenario that must **survive**. Deleting a step fn without its Gherkin step
  yields a *parse/undefined-step* failure, not a clean pass — so #12/#13 are not optional.
- **The unit tests are NOT `projects.rs:1039-1110`** (the DISCUSS range) and are **not deletable whole**.
  Actual: `1037-1075` and `1086-1134`. Both do real work beyond the carrier.
- **`issue_key_string` stays.** It has two callers — the carrier builder (`:890`) **and** `issue_card`
  (`:912`, populating `IssueCard.key`). It remains live; do **not** remove it, and expect no dead-code
  warning to prompt you.

### Trap A — `projects.rs:1037-1075` must be *edited*, not deleted
`populated_board_renders_assets_cards_and_ascending_keyboard_carrier` also asserts the `/static` asset
links (1051-1062) and `data-column` card placement (1064-1068) — **coverage that must not be lost with the
carrier**. Delete only the carrier assertions (body 1070-1074) and the carrier clause of the doc (1039);
**rename** the fn (it lies once the carrier is gone).

### Trap B — `projects.rs:1110` is a **vacuity trap** (the one to get wrong)
`each_issue_lands_in_exactly_its_state_column` slices the carrier off the page before asserting:
```rust
let visible = html.split(r#"id="kb-items""#).next().unwrap();
```
Once the carrier is gone, `split(…).next()` returns **the entire page** and the test **still passes** — but
it is no longer asserting what it claims (it would now pass even if cards leaked outside their columns,
because the intended exclusion silently became a no-op). `visible` must be repointed at the full HTML
(`&html`) **as part of this change**, and the carrier comment at 1105-1110 removed.

> This is the trap worth naming loudest: the *whole feature* exists because a test was green while the
> thing it named was absent. Deleting the carrier and leaving line 1110 would reproduce that pattern **in
> the very change that removes it**.

## Alternatives Considered
- **Keep `#kb-items` and walk it (honour the shipped carrier, keep the green test)** — REJECTED. It
  contradicts the locked D-4 on three independent counts (order, `hidden`, `aria-hidden`), any one fatal:
  you cannot ring or scroll to a hidden element, and "selection follows the eyes" is the only model a user
  can predict. Choosing the carrier would mean choosing the test over the user.
- **Keep the carrier as a redundant ordering hint, unused** — REJECTED. Directly against AGENTS.md
  (*"do not leave it inert"*), and it is *worse* than ordinary dead code: it is dead code that **actively
  misleads**, carrying a comment asserting it is the navigation source of truth. The next reader would
  wire j/k to it and reintroduce the bug.
- **Retire the carrier but keep the ASC acceptance assertions against the visible board** — REJECTED. The
  visible board is DESC-within-column **by deliberate design** (`projects.rs:856-879`); asserting ASC over
  it would red immediately and correctly. The assertion is not salvageable — it pins a contract the locked
  UX contradicts.
- **Deprecate now, delete later** — REJECTED. Pre-stable, no backwards-compatibility obligation
  (AGENTS.md); "later" is how `#kb-items` reached today.

## Consequences
- **Positive**: the contradiction is gone. One navigation order — the one the user sees — and one place it
  lives: the DOM. AC-05.6's grep litmus (`kb-items`/`kb_items` → **zero** hits under `crates/`) becomes the
  simple, checkable statement of doneness.
- **Positive**: the board sheds a hidden, `aria-hidden` list of every issue key on every render — dead
  markup on the hot page.
- **Negative / accepted — this deliberately deletes passing tests.** Two acceptance assertions
  (`us_12_keyboard_nav.rs:331-362`, `feature_b_web_tier.rs:563-581`), one whole Gherkin scenario
  (`us-b01-styled-board.feature:60-66`), one Gherkin step (`us-12-keyboard-nav.feature:50`), and unit
  assertions. **This is a decision, not an accident.** Those tests pin a contract the locked UX
  contradicts, for a consumer that never existed. **Their coverage is not lost — it is replaced and
  strengthened**: the browser lane (ADR-007) asserts the *actual* user-observable navigation order, which
  is what the carrier's assertion was a proxy for and could never verify.
- **Negative / accepted**: `BoardPage` loses a public field. Blast radius is verified minimal — exactly one
  struct literal (`projects.rs:893`); the unit tests construct via the `render_board` helper, so **no other
  call site breaks**.
- **Probe (Earned Trust)**: the deletion is proved by a **grep litmus** (zero hits), not by belief. Trap B
  is proved by the repointed `visible` continuing to assert column placement — if someone deletes the
  carrier and leaves `split(…)`, the litmus passes but the test is vacuous, which is why trap B is called
  out as a first-class step and not a cleanup detail.
