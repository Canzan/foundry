# Upstream Changes — keyboard-shortcut-bindings (DESIGN wave)

> Adjustments DESIGN makes to DISCUSS assumptions, and to decisions recorded elsewhere in the repo.
> **DISCUSS artifacts are NOT modified**; this file records the deltas + rationale (verbatim quote →
> resolution). Six entries: **one amendment to a locked decision (D2)**, **one reversal of a prior recorded
> repo decision**, **three factual corrections**, and **one property clarification**.

## 1. CORRECTION — the harness already serves on a real TCP port (ODD-9 is much cheaper than priced)

**DISCUSS assumed** (`acceptance-criteria.md`, ODD-9; `outcome-kpis.md`):

> *"the shipped `InProcHarness` + reqwest + `scraper` (`us_12_keyboard_nav.rs:48-56`) is **port-to-port**
> and can only assert server contracts."*

and the DESIGN brief sharpened it to: *"the app must be served on a real port for a real browser, which the
in-process port-to-port harness does not do today."*

**The second half is false.** Verified: `InProcHarness::spawn` (`support/harness.rs:287,322-440`) →
`spawn_app_with_listener` → `spawn_app` (`foundry-app/src/lib.rs:726-746`) already binds
`TcpListener::bind("127.0.0.1:0")` and runs `axum::serve` on a tokio task, exposing
`base_url()` → `http://127.0.0.1:{ephemeral}` (`harness.rs:442-444`). `tower::oneshot` is used **only** in
`foundry-app`'s own unit tests (`foundry-app/tests/csrf_middleware.rs:92,108,125`), never in the acceptance
crate.

**"In-process" means *same OS process*, not *no socket*.** The suite is port-to-port in the sense that it
speaks HTTP over a real socket with `reqwest` — its limitation is that **`reqwest` cannot press a key**,
not that there is no origin to point a browser at.

**Resolution (ADR-007):** `BrowserHarness = InProcHarness (unchanged) + a fantoccini session → base_url()`.
**No serving plumbing is added.** This makes ODD-9 substantially cheaper than DISCUSS priced it, and — more
importantly — keeps **one** app-construction path, so the browser lane and the port-to-port suite cannot
diverge. The DISCUSS conclusion (*a browser-capable driver is required, and it is the root cause*) is
**entirely correct and unchanged**; only the cost estimate moves.

## 2. REVERSAL — the repo's recorded "no-Playwright" decision is overturned (ODD-9)

**Recorded verbatim** at `crates/foundry-acceptance/tests/features/us-12-keyboard-nav.feature:18-23`:

> *"Pure browser interaction (the actual `c` / `/` / `j` / `k` / `Enter` / `?` key handling, the modal focus
> management, the highlight after a realtime swap) lives in alpine.js and is **OUT of automated scope per
> the JTBD-backend-MVP no-Playwright decision**. The @manual scenario at the bottom is the QA drill script
> (precedent: US-01's @manual entry in slice 1)."*

**This is the root cause, and it is written down.** DISCUSS inferred the instrument was missing; it is
better than that — the repo *decided* to omit it, deliberately, and then shipped a help page advertising
seven shortcuts against an `@manual` drill (`:87-95`) that evidently never ran or never failed. The
decision named a *tool* (Playwright) and delivered the absence of a *capability* (browser testing).

**New position (ADR-007):** browser interaction is **in** automated scope, via a Rust-native W3C WebDriver
lane (`fantoccini` + chromedriver) inside `cargo xtask ci`. **This is not the adoption of Playwright** —
the original decision conflated "Playwright" with "browser testing", and reversing it means the
Rust-native lane the repo can own, in the gate it already trusts.

**Consequences to execute (slice 05, ADR-007 §5):** the `@manual` drill (`:87-95`) is superseded and
retired; the module-doc paragraph (`:18-23`) states a decision that is no longer true and is rewritten.
`AGENTS.md`'s rule applies in full: the lane lives in `cargo xtask ci`, **never in `ci.yml` alone**.

## 3. AMENDMENT to locked decision D2 — the surface scope is BOARD ONLY

**DISCUSS locked, verbatim** (`wave-decisions.md`, D2):

> *"[D2] **Surface scoping**: `c`/`j`/`k`/`Enter` on the **board + issue list**; `?` and `Esc` **global** on
> any signed-in page. Matches the team+project scoping of the shipped routes (`keyboard.rs:62-95`)."*

DISCUSS itself flagged the collision (ODD-6/R8) and read "issue list" as the search-results fragment,
pending confirmation.

**New assumption (user-ratified, ADR-005):** **BOARD ONLY.** The "issue list" is **dropped as a surface** —
it does not exist. `j`/`k`/`Enter` walk **board cards**. `/` reveals a search box **on the board**; `j`/`k`
then walk the **search results**, which are an overlay *on the board*, not a separate surface with its own
scope.

**Rationale:** verified — `…/issues` is registered **POST-only** (`lib.rs:487-490`); there is no `/issues`
GET, no backlog, no list page; `report.html:6` lists change events, not issues. Scoping to a surface that
does not exist would either (a) invent an issue-list page — a different feature, explicitly out of scope —
or (b) quietly redefine "issue list" to mean something else, which is the reinterpretation DISCUSS
deliberately refused to make on its own authority.

**Delta vs the DISCUSS reading:** DISCUSS treated the search-results list as *the second issue-key-bearing
surface* with its own scope. The amendment treats search as **a panel on the board**. This is a real
difference, and it is what makes ADR-005's `Enter` resolution possible (see §4): because the panel
**overlays** the board rather than replacing it, the board's cards stay in the DOM and remain the single
open path.

**No FR/NFR/BR is weakened.** FR-8/FR-9 read "visible cards"; BR-3 reads "a surface with a project context
and/or issue cards" — both are satisfied by the board plus its search panel.

## 4. CORRECTION — search results have no `hx-get`, so US-06's stated mechanism is unimplementable there

**DISCUSS assumed** (`user-stories.md` US-06 Solution; AC-06.1):

> *"Bind `Enter` … to activate the **selected** card's shipped `hx-get={edit_url}` → `#modal-root`
> (`issue_card.html:1`) — identical to a pointer click, so there is exactly one open path and no
> divergence."*

and scoped `j`/`k`/`Enter` over the search-results list (ODD-6).

**Verified false for search results.** `partials/search_results.html:4` renders
`li.search-result[data-issue-key]` containing **only** `.key` and `.title` spans — **no `hx-get`, no
`edit_url`, no `data-state-url`**. The view-model carries key + title only (`keyboard.rs:233-239`). As
shipped, **a selected search result has no open path at all**. US-06's mechanism works on the board and
cannot work on a result row.

**Resolution (ADR-005 §4):** `Enter` resolves `selectedKey` → `article.issue-card[data-issue-key=K]` →
activates **that** card's shipped `hx-get`. One rule for both surfaces (on the board the card *is* the
selected element), possible because the search panel overlays the board (§3).

**The rejected alternative is why this is a correction and not a nuance:** adding `edit_url` to the search
view-model + `hx-get` to the partial would have been the obvious fix, and it would have **breached D10/AC-X.4**
(*"the only server-side changes are removals"*) *and* AC-06.5 (*"exactly one open path"*) simultaneously.
The requirement's **intent** — one open path, converged with the pointer — is honoured **more** strictly by
resolution than by the mechanism the requirement literally named.

**Named edge, newly surfaced:** the board renders only `{backlog, todo, in_progress, done}`
(`projects.rs:49,933-941`); search returns every issue. An issue in any other state is findable but has no
card → `Enter` is a **no-op** (consistent with FR-9's "no selection ⇒ no-op"). Pinned for DISTILL.

## 5. CORRECTION — the `#kb-items` delta map is wider than DISCUSS recorded (and contains a vacuity trap)

**DISCUSS recorded** (`requirements.md` R1, `wave-decisions.md` D12, `shared-artifacts-registry.md`):

> *"Retirement deletes: `board.html:12`, the builder `projects.rs:881-891`, the view-model field
> `views.rs:256`, unit tests `projects.rs:1039-1110`, and **both acceptance assertions**
> (`us_12_keyboard_nav.rs:334-360`, `feature_b_web_tier.rs:568-572`)."*

The list is directionally right and **incomplete in three ways** (verified site-by-site, ADR-008):

1. **Two Gherkin feature files were not counted.** `us-b01-styled-board.feature:60-66` is a **whole
   scenario** ("The board preserves the hidden keyboard-navigation order") that must go;
   `us-12-keyboard-nav.feature:50` is a **single step** inside a scenario that must **survive**. Deleting
   the Rust step fns without their Gherkin steps yields undefined-step failures, not a clean pass.
2. **The unit tests are `1037-1075` and `1086-1134`** (not `1039-1110`), and **neither is deletable
   whole** — both carry coverage beyond the carrier (asset links, `data-column` placement) that must be
   preserved.
3. **`projects.rs:1110` is a vacuity trap.** `let visible = html.split(r#"id="kb-items""#).next().unwrap();`
   — once the carrier is gone this returns the **whole page** and the test **still passes** while no longer
   asserting what it claims. `visible` must be repointed at the full HTML in the same change.

Also: **`issue_key_string` stays live** (second caller at `projects.rs:912`); do not remove it. And two
prose sites DISCUSS did not list (`projects.rs:794`, `views.rs:9`) mention the carrier.

**The decision (retire whole) is unchanged and confirmed** — only the map is corrected. Trap 3 deserves the
emphasis it gets in ADR-008: this feature exists **because a test was green while the thing it named was
absent**, and leaving line 1110 would reproduce that exact pattern inside the change that removes it.

## 6. CLARIFICATION — what "zero server delta" precisely means (D10 / FR-11 / AC-X.4)

**DISCUSS states** (`wave-decisions.md` D10; AC-X.4):

> *"[D10] **Zero server delta (FR-11).** … This feature adds **no route, no endpoint, no migration**. The
> only server-side changes are **removals**: the retired `#kb-items` builder (ODD-1) and the stale
> "alpine.js" doc comment (`keyboard.rs:1-30`, ODD-2, R9)."*

**FR-11 as written is honoured exactly and strictly**: **zero** new routes, endpoints, migrations, or
handler-behaviour changes. `keyboard.rs`'s three handlers, the `HX-Request` fork, CSRF minting,
`filter_matches`, and every route registration are **byte-for-byte untouched**. The design deliberately
chose JS-created hosts over template mounts (ADR-003) and `Enter`-via-the-board-card over a search
view-model field (ADR-005) **specifically to keep this property**.

**But AC-X.4's stricter phrasing — *"the only server-side changes are removals"* — cannot hold literally**,
for three unavoidable additive edits. Recorded rather than glossed:

1. **`base.html` gains one line**: `<script src="/static/js/keyboard.js" defer></script>`. The layer cannot
   load otherwise. (`base.html:7`'s Alpine tag is simultaneously **removed** — ADR-001.)
2. **The stylesheet gains rules** (ring, overlay host, search panel). NFR-7's WCAG-AA non-colour-alone ring
   is not achievable without CSS. This triggers the **inherited** hand-maintained content-hash re-hash:
   edit → sha256 → `git mv` → update `base.html:5` **and** `lib.rs:284`, as one atomic commit, guarded by
   the asset-resolution probe (`navigation-bar-linear-ui/design/adr-004`). **Not re-decided here.**
3. **Test infrastructure** (ADR-007): the `@needs-browser` lane, `BrowserHarness`, the `fantoccini`
   dev-dependency, the `xtask` preflight and its env-injection fix.

**#3 is explicitly not a server delta** — it is test infrastructure, adds nothing to the binary, and
touches no handler. Called out here so the property is not misread as violated when a reviewer sees a new
dependency in `Cargo.toml`.

**Proposed precise restatement of AC-X.4 for DISTILL** (the intent, made checkable):

> **Zero server delta.** No route, endpoint, migration, or handler-behaviour change. The only server-side
> *behaviour* changes are removals (the `#kb-items` builder + view-model field) and doc corrections
> (`keyboard.rs:1-30`). Additive template/asset edits are limited to: one `<script>` tag, the Alpine tag's
> removal, and the stylesheet + its mandatory re-hash. Test infrastructure is out of scope of this property.

## Predecessor lineage

None — this feature has no predecessor and no successor carve-out. There are **no DISCOVER or DIVERGE
artifacts** (`docs/feature/keyboard-shortcut-bindings/{discover,diverge}/` do not exist); the job statement
and personas were established directly in DISCUSS and folded inline per house convention (D14). All nine
ODDs are resolved here (`wave-decisions.md` carries the index) and **no ODD is deferred to DISTILL**.
ODD-7's Tab-to-focus cost was **escalated to the user** rather than silently absorbed, and is now
**ratified and closed**: Option A accepted (2026-07-15) — D-4 stands, the one-time Tab-to-the-board is an
**accepted, documented cost**, and KPI-4 is met **conditionally on it**, a qualifier that must accompany
every KPI-4 claim (ADR-006). Two AC re-shaping notes (`Cmd+C` clipboard, IME `send_keys`) were **not** part
of that ratification and remain **open for DISTILL** (`wave-decisions.md`).
