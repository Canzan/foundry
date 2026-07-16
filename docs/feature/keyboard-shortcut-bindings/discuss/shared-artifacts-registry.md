# Shared Artifacts Registry — keyboard-shortcut-bindings

Every value that flows across the journey (dispatch → guard → help/escape; create → search; select → open), its
single source of truth, and its consumers. This feature is **client-only** — it adds **no route, no endpoint, no
migration** — so unlike most features here the artifacts are mostly **client state** and **shipped server
fragments** rather than database rows.

The integration-critical ones are `shortcut_set` (the promise the product already made in writing),
`guard_verdict` (the thing that makes the layer safe rather than harmful), and `visible_cards` (which **collides
with the shipped `#kb-items` carrier** — ODD-1, Risk R1).

```yaml
shared_artifacts:
  shortcut_set:
    source_of_truth: "SHORTCUTS — const &[(&str, &str)] with exactly 7 entries (c=Create issue, /=Search, j=Next, k=Previous, Enter=Open selected, ?=Show this help, Esc=Close modal), crates/foundry-app/src/keyboard.rs:48-56. An acceptance test already asserts the list is complete. Its doc says: 'Update this list when adding new shortcuts; the help overlay enumerates exactly this set.'"
    consumers:
      - "the /keyboard-help <dl> the user READS (show_keyboard_help, keyboard.rs:259-279 -> partials/keyboard_help.html:1 -> dt[data-shortcut]/dd per entry)"
      - "the client dispatch table — WHAT IS BOUND (this feature; currently EMPTY, which is the bug)"
    owner: "foundry-app (shipped) — this feature adds the second consumer, it does not change the source"
    integration_risk: "HIGH (trust) — the two consumers MUST agree. Today they maximally disagree: the overlay advertises 7 and the client binds 0. That divergence IS the feature. A future shortcut added to the overlay but not bound (or vice versa) re-opens exactly this bug (BR-1, FR-1)."
    validation: "Every shortcut the help overlay lists is bound and does something; no shortcut outside the list is bound. Both sides read SHORTCUTS, so they cannot drift by construction (AC-X.1)."

  guard_verdict:
    source_of_truth: "DERIVED per keydown, evaluated BEFORE any dispatch (BR-2): (a) is the event target a text-entry context — input / textarea / contenteditable / select / role=textbox / IME isComposing (exact predicate = ODD-4)? (b) is Ctrl / Cmd(Meta) / Alt held? Shift is explicitly NOT a suppressor because ? IS Shift+/ (BR-7). NEW — nothing like it exists today."
    consumers:
      - "EVERY shortcut dispatch — all seven, with NO exemptions (BR-2): ? (K3), Esc (K4), c (A1), / (A2), j/k (M1), Enter (M2)"
    owner: "this feature (the guard chain) — the single decision every shortcut passes through"
    integration_risk: "CRITICAL — THE highest-risk artifact in the feature. Every one of the seven is a plain printable character or bare key, i.e. exactly what people type. If the guard is wrong, binding them makes text entry IMPOSSIBLE (Mei cannot type the letter c into a title) — STRICTLY WORSE THAN SHIPPING NOTHING. A per-shortcut check (seven scattered ifs) is how this fails; it must be structural (NFR-2, R2)."
    validation: "Typing 'cjk/?' into any field yields exactly 'cjk/?' — no modal, no search focus, no selection move; Cmd+C copies. A dedicated @property litmus REDS on regression (AC-02.2, AC-X.2)."

  visible_cards:
    source_of_truth: "article.issue-card in DOM order (partials/issue_card.html:1 — carries id=issue-KEY, data-issue-key, draggable, data-state-url, hx-get={edit_url}) as rendered on the board, column-grouped and DESC-within-column (projects.rs:864-879); plus li.search-result[data-issue-key] on the search-results list (partials/search_results.html:4) — the second issue-key-bearing surface (ODD-6)."
    consumers:
      - "j/k selection order (M1) — the order the user SEES"
      - "Enter's target resolution (M2)"
    owner: "foundry-app board/search rendering (shipped) — this feature only READS the DOM order"
    integration_risk: "HIGH (correctness + process) — COLLIDES WITH ${kb_items_carrier}. The shipped hidden carrier is flat ASC-by-number across all columns; the visible cards are column-grouped DESC-within-column. Walking one is observably NOT walking the other. The locked model (D-4) picks VISIBLE, which RETIRES the carrier and DELETES two currently-GREEN assertions (ODD-1, R1)."
    validation: "Selection order equals the order cards appear on screen; a litmus REDS if selection order diverges from visible order (AC-05.1)."

  kb_items_carrier:
    source_of_truth: "<ul id='kb-items' hidden aria-hidden='true'> with one li[data-issue-key] per issue, sorted ASCENDING by issue number across ALL columns — board.html:12; built projects.rs:881-891; view-model field views.rs:256. Built for the never-written handler. The suite calls it 'the source of truth for the keyboard navigation order' (us_12_keyboard_nav.rs:339-341)."
    consumers:
      - "NONE IN THE BROWSER — the alpine.js handler it was built for was never written (verified: zero keydown handlers in app code)"
      - "two acceptance assertions ONLY: us_12_keyboard_nav.rs:334-360 (ASC order), feature_b_web_tier.rs:568-572"
      - "unit tests projects.rs:1039-1110"
    owner: "foundry-app board rendering (shipped) — RETIRED BY THIS FEATURE"
    integration_risk: "HIGH (process) — this artifact is DEAD ON ARRIVAL and always was: it has zero browser consumers. It cannot serve the locked model because (1) its ORDER differs from the visible board, (2) it is `hidden` so a ring renders nothing and scrollIntoView does nothing, and (3) aria-hidden='true' hides it from assistive tech. Honouring D-4 RETIRES it. Per AGENTS.md ('Remove dead/legacy code outright — do not leave it inert') it must be DELETED WHOLE: carrier + builder + view-model field + unit tests + BOTH acceptance assertions. This deliberately deletes PASSING TESTS (ODD-1, R1)."
    validation: "After this feature, `#kb-items` appears nowhere: not in board.html, not in projects.rs, not in views.rs, not in either acceptance step file. A grep for kb-items/kb_items returns zero hits (AC-05.6)."

  selection:
    source_of_truth: "CLIENT-ONLY, EPHEMERAL state pointing at ONE ${visible_cards} element (BR-5). Never persisted, never sent to the server, RESETS ON NAVIGATION. NEW — no selection concept exists in the client today. Survival across an htmx swap (by issue-key vs index vs reset) is ODD-5."
    consumers:
      - "the ring highlight + scrollIntoView (M1)"
      - "Enter — opens the selected card (M2)"
      - "Esc — must NOT clear it (K4)"
      - "assistive tech — via the ODD-7 mechanism (NFR-7)"
    owner: "this feature (the client selection model)"
    integration_risk: "HIGH — the card the RING SHOWS must be the card ENTER OPENS, or the user acts on the wrong issue. Esc must not silently clear it (BR-5). It must survive/reset coherently across htmx swaps and drags (ODD-5, NFR-8, R4). Because a ring is NOT native focus (D-4 rejected roving tabindex), NOTHING announces it to a screen reader for free — ODD-7 (R3)."
    validation: "The ringed card is the card Enter opens; Esc leaves selection intact; a drag leaves no stale ring; selection resets on navigation and never reaches the server (AC-05.1, AC-05.5, AC-05.8, AC-06.1, AC-07.3)."

  layer_stack:
    source_of_truth: "CLIENT-ONLY: which layers are currently open — help overlay (K3) / new-issue modal (A1) / issue modal (M2) / search (A2). Determines Esc precedence: close the TOPMOST only, one per press (BR-4). NEW."
    consumers:
      - "the Esc handler (K4) — closes topmost only"
    owner: "this feature (the layer model)"
    integration_risk: "MEDIUM — Esc must close exactly ONE layer per press (help over modal), never all at once, and never navigate when nothing is open (FR-5). Interaction with htmx swaps that replace #modal-root content out from under the stack is ODD-5."
    validation: "With help over the new-issue modal, one Esc closes help and leaves the modal open; a second closes the modal; with nothing open Esc does nothing (AC-07.1, AC-07.2)."

  help_fragment:
    source_of_truth: "GET /keyboard-help -> section.keyboard-help[role='dialog'][aria-label='Keyboard shortcuts'] with header>h2 and one dt[data-shortcut]/dd pair per entry — rendered FROM ${shortcut_set} (show_keyboard_help, keyboard.rs:259-279; partials/keyboard_help.html:1; route lib.rs:536). BARE (no base.html) so a swap is not double-wrapped. PUBLIC BY DESIGN (keyboard.rs:19-24)."
    consumers:
      - "the ? overlay (this feature — the NEW consumer)"
      - "the full-page no-JS links: sidebar.html:13, dashboard_root.html:32 (the ONLY consumers today)"
    owner: "foundry-app (shipped) — reused verbatim, NO new route"
    integration_risk: "LOW (the fragment) / MEDIUM (its placement) — the fragment is already built for exactly this overlay use (bare, role=dialog). It is PUBLIC deliberately so help works on the sign-in page; that must not be 'fixed' into a session-gated route. Whether the full-page links stay is ODD-8 (recommend: keep, as the no-JS path, NFR-4)."
    validation: "? renders this fragment over the current page with the URL unchanged; it lists exactly the 7 SHORTCUTS entries; the full-page links still work with scripting off (AC-01.1, AC-01.2, AC-01.5)."

  modal_mount:
    source_of_truth: "#modal-root — the htmx swap target. EXISTS ONLY at board.html:13. app_shell.html (7 lines: extends base + sidebar + app_content) has NONE. Already the target of board.html:6 (New issue button) and issue_card.html:1 (hx-target='#modal-root')."
    consumers:
      - "the c new-issue modal (A1) — board only, fine"
      - "the Enter issue modal (M2) — board only, fine"
      - "the ? overlay (K3) — GLOBAL, so it has NO MOUNT on non-board pages"
    owner: "foundry-app templates (shipped)"
    integration_risk: "HIGH (blocking) — the locked scope makes ? and Esc GLOBAL, but the only mount point is board-local. A global ? has nowhere to render on the dashboard or any non-board page. Resolve by hoisting #modal-root into app_shell.html, or injecting a mount on demand — ODD-3, R5. Surfaced precisely BECAUSE 'global ?' is locked."
    validation: "? renders an overlay on a non-board page (e.g. the dashboard) (AC-01.3)."

  project_context:
    source_of_truth: "the team_slug + project_slug of the current board — already embedded in the page's own hx-get URLs (board.html:6: /team/{{team_slug}}/project/{{project_slug}}/issues/new). REQUIRED by both team-scoped routes (keyboard.rs:62-95 authz: signed-in user must belong to the project's team; non-members 403, unknown team/project 404)."
    consumers:
      - "the c binding (A1) — builds the …/issues/new URL"
      - "the / binding (A2) — builds the …/search URL"
    owner: "foundry-app board rendering (shipped)"
    integration_risk: "MEDIUM — c and / are meaningless without a project. On a page with no context (the dashboard) they must do NOTHING, silently — no error, no navigation (BR-3). The client should read the context off the page's existing markup rather than reconstruct it, so it cannot disagree with what the button already does (ODD-6)."
    validation: "c on the dashboard opens no modal and does not navigate; c on a board opens the modal for THAT project (AC-03.1, AC-03.3)."

  search_results:
    source_of_truth: "GET …/search?q= -> ul.search-results with one li.search-result[data-issue-key] (containing .key + .title spans) per match; empty state ul.search-results[data-empty='true'] (search_issues, keyboard.rs:160-202; filter_matches :208-231 — exact-key PREFIX-N path :217-224, case-insensitive title substring :226-231; empty query returns EVERY issue :213-215; partial search_results.html:4; route lib.rs:495-498). BARE fragment."
    consumers:
      - "the / results list (A2)"
      - "j/k selection + Enter on the results list — the SECOND issue-key-bearing surface (ODD-6)"
    owner: "foundry-app (shipped) — reused verbatim, NO new route"
    integration_risk: "MEDIUM — the fragment already distinguishes 'no query' (every issue) from 'no match' (data-empty=true); the client must honour that distinction rather than render a blank void. Because the results carry data-issue-key, they are a legitimate j/k/Enter surface — which is the most defensible reading of the locked 'issue list' scope, since no issue-list PAGE exists (ODD-6, R8)."
    validation: "/ then 'session' lists AUTH-2; 'AUTH-2' returns exactly that issue; 'zzz' shows the empty state (AC-04.2, AC-04.3, AC-04.4)."

  keyboard_script:
    source_of_truth: "a NEW external same-origin script under static/js/ (e.g. keyboard.js), loaded `defer` from base.html:6-9 alongside the vendored htmx.min.js / alpine.min.js and the app's board-dnd.js / csrf-upload.js. Vanilla IIFE vs Alpine is ODD-2."
    consumers:
      - "every signed-in page (via base.html -> app_shell.html)"
    owner: "this feature (the client layer) — THE artifact that does not exist and is the whole feature"
    integration_risk: "MEDIUM — must follow the house idiom: external same-origin (CSP-safe, NO inline handlers), `defer`, DOCUMENT-DELEGATED listeners so htmx-swapped fragments need no re-wiring (the board-dnd.js:67 dragstart idiom, which exists for exactly this reason). Vendored only, NO CDN (NFR-5, NFR-6). NOTE: keyboard.rs:1-30 documents 'the alpine.js keyboard-shortcut handlers' while the house pattern is vanilla — whichever DESIGN picks, that stale doc must be corrected in the same change (ODD-2, R9)."
    validation: "No inline handler and no external origin is introduced; after an htmx swap (filing an issue via c) the shortcuts still work with no page reload (AC-X.5, AC-X.6)."

  csrf_token:
    source_of_truth: "the shipped double-submit CSRF token — minted SERVER-SIDE by ensure_csrf_cookie (keyboard.rs:94) and rendered into the modal fragment as <input type=hidden name=_csrf> (partials/new_issue_modal.html:4)."
    consumers:
      - "the new-issue form POST to …/issues (unchanged — the form carries it)"
    owner: "foundry-app (shipped CSRF) — reused, untouched"
    integration_risk: "LOW — worth stating explicitly: unlike board-dnd.js and csrf-upload.js (which must mirror the foundry_csrf cookie into an x-csrf-token header for fetch/multipart), the c path needs NO client CSRF work at all. The server already mints the cookie on the GET and the fragment already carries the hidden field; the form submits normally."
    validation: "Filing an issue via c succeeds with no client-side CSRF handling; the shipped double-submit check applies unchanged (AC-03.2)."
```

## Consistency checks (for DISTILL / DELIVER)

1. Does every `${variable}` in the journey mockups have a documented source above? **Yes** — all 12 tracked
   (`shortcut_set`, `guard_verdict`, `visible_cards`, `kb_items_carrier`, `selection`, `layer_stack`,
   `help_fragment`, `modal_mount`, `project_context`, `search_results`, `keyboard_script`, `csrf_token`).
2. **Promise == fulfilment**: the keys bound in the client equal the keys the help overlay lists. Both read
   `SHORTCUTS` (`keyboard.rs:48-56`), so they cannot drift. Today they maximally disagree (7 advertised, 0
   bound) — **that divergence is the feature**. (HIGH — trust)
3. **Guard before dispatch**: `${guard_verdict}` gates **all seven** shortcuts with no exemptions. Typing
   `"cjk/?"` yields exactly `"cjk/?"`; `Cmd+C` copies. (**CRITICAL** — a regression is worse than shipping nothing)
4. **Selection order == visible order**: `${selection}` walks `${visible_cards}`, not `${kb_items_carrier}`.
   The carrier is **deleted whole** — markup, builder, view-model field, unit tests, and **both** acceptance
   assertions. A grep for `kb-items` returns zero hits. (HIGH — correctness + a deliberate green-test deletion)
5. **Ring == Enter target**: the card the ring shows is the card `Enter` opens; keyboard and pointer converge on
   the card's own shipped `hx-get`. (HIGH)
6. **Esc precedence + selection safety**: `${layer_stack}` closes one layer per press; `Esc` never clears
   `${selection}` and never navigates when nothing is open. (MEDIUM)
7. **Global `?` has a mount**: `${modal_mount}` is board-only today; a global `?` needs one everywhere (ODD-3).
   (HIGH — blocking)
8. **Handlers survive swaps**: `${keyboard_script}` uses document-level delegation (the `board-dnd.js:67`
   idiom); an htmx swap needs no re-wiring. (MEDIUM)
9. **Zero server delta**: no route/endpoint/migration added. The only server-side changes are **removals** (the
   retired carrier) and the stale-doc correction. (HIGH — scope guard)
10. **No-JS intact**: `${help_fragment}`'s full-page links and the `HX-Request` fork stay working; nothing
    becomes keyboard-only. (HIGH — regression guard)
