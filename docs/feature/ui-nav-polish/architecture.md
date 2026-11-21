# Architecture brief: ui\-nav\-polish

## Paradigm

procedural

## Shared records

### Decisions

None.

### Constraints

#### K1

```text
User decision 2026-11-20: fix the current-page mark for the three moved items in the UI clean-up.
```

#### K2

```text
User decision 2026-11-20: hide Invite a member and Machine tokens from operators those pages refuse.
```

#### K3

```text
User decision 2026-11-20: the UI clean-up covers groups A to D, delivered as three Requests; this one is group C.
```

#### K4

```text
User decision 2026-11-20: a control inside the left rail is exempt from the WCAG 1.4.11 boundary rule in EVERY layout, exactly as the rail links are. The exemption follows rail membership, not whether the control happens to draw a word, so it holds in the narrowed rail and at phone width where Sign out and the theme control draw only a glyph. Wherever a rail control draws its word, that word still obeys the WCAG 1.4.3 text-contrast rule against the surface behind it.
```

#### K5

```text
User decision 2026-11-20: the phone bar's one-fifth bound holds for phones 390 px wide and up; on smaller phones the bar may be taller but shows every destination.
```

### Reuse

None.

### Impact

None.

### Agreement

None.

### Not applicable

None.

### Boundaries

None.

### Observations

None.

### Equality

None.

### Constructors

None.

### Laws

None.

### Refuted

None.

### Claims

None.

### States

None.

### Models

None.

### Closure

None.

### Risks

None.

### Architectural tests

None.

### Contract tests

None.

## Slices

### S1

#### Targets

crates\/foundry\-app\/src\/nav\.rs

Extend

```text
Three NavSection variants with their predicates and constructors, the total offered_mark reconciliation called once inside resolved(), and the totality unit test extended over every variant and every section-by-rights pair.
```

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Extend

```text
Mark conditions on the three moved members, the workspace-admin condition on two of them, and the sign-out submit joins the item presentation.
```

crates\/foundry\-app\/static\/css\/foundry\.2f9e0cb1\.css

Extend

```text
Sticky rail with its scroll container on the open geometry alone, overflow: visible and an explicit paint level on the folded geometry, phone glyph geometry, the clipped workspace name and the quiet lower group. The file carries the sha256 prefix of its own bytes, so any further change renames it again and removes the old name.
```

crates\/foundry\-app\/templates\/base\.html

Extend

```text
The stylesheet link takes the new hash.
```

crates\/foundry\-app\/static\/VENDOR\.md

Extend

```text
The stylesheet row name and its recomputed sha256.
```

crates\/foundry\-app\/src\/lib\.rs

Extend

```text
The three cache-policy assertions that name the stylesheet path literally.
```

crates\/foundry\-app\/src\/teams\.rs

Extend

```text
The one teams nav assembly marks Teams.
```

crates\/foundry\-app\/src\/member\_invites\.rs

Extend

```text
Both invite renders mark Invite a member.
```

crates\/foundry\-app\/src\/admin\_tokens\.rs

Extend

```text
The prefactoring extraction, then the single site marks Machine tokens.
```

xtask\/src\/check\_arch\.rs

Extend

```text
The new mark-comes-from-the-carrier obligation beside the shipped rail-member law.
```

crates\/foundry\-acceptance\/tests\/features\/ui\-nav\-polish\.feature

#### Verification

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo test -p foundry-app nav"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo xtask check-arch"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=ui-nav-polish cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=us-cts-01 cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=all cargo test -p foundry-acceptance"
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_ui\_nav\_polish\.rs

crates\/foundry\-acceptance\/src\/steps\/mod\.rs

crates\/foundry\-acceptance\/src\/support\/rail\_reading\.rs

crates\/foundry\-acceptance\/tests\/features\/nav\-quick\-actions\.feature

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_quick\_actions\.rs

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_icon\_rail\.rs

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs

#### Public oracle

```text
Through the real public driving port, with a real session cookie: a workspace admin opens /teams, /workspace/invites, /admin/tokens, a team page under /teams, the dashboard and a board; a member with no rights beyond her workspace opens /teams and a board; an instance super-admin who administers no workspace opens the dashboard. For each rendered rail, every href it offered is then opened with the SAME cookie. In the browser lane the same GETs are rendered at the harness desk viewport, scrolled to the bottom of a page taller than the window in both rail geometries, and at the emulated 390x844 phone device; in the folded desk geometry, on a board page carrying cards, one primary nav member and one lower-group member each take keyboard focus so the revealed word overlaps a column; one press of Sign out and one press of the theme control are taken.
```

```text
On each page the one member carrying aria-current=page is the member that owns it - Teams on /teams and its team page, Invite a member on /workspace/invites, Machine tokens on /admin/tokens, Home on the dashboard, Board on the board - and never a second member and never none. The member with no rights and the instance-only super-admin are offered Home, Board and Teams and neither admin link; the workspace admin is offered all five. Every offered href answers her own cookie with a page, never the refusal 404. At desk width the whole rail box lies inside the viewport after the scroll in both geometries, with every member visible; in the folded geometry the revealed word of each focused member is VISIBLE beside the rail and painted over the column it overlaps, the centre of its revealed box hit-testing back to the word itself rather than to an ancestor, a column or a card. At the phone device the bar is at most 168.8 px high with every member visible, activatable and at least 44 px, and the workspace name is still one line that ends in an ellipsis, clipped out of the strip but not out of the document. The sign-out submit and the theme control resolve to the rail surface fill, no border their neighbour links lack and the same text tier and weight; each one draws a glyph and its own word, and that word reaches 4.5:1 against the surface behind it in both palettes, with no fill or border boundary owed; Sign out still ends the session and the theme control still changes the palette.
```

```text
The shipped build fails it today on every count: /teams, /workspace/invites and /admin/tokens mark Home, the two admin links are offered to a plain member and answer her 404, the rail scrolls away with a long page, the phone bar takes about 320 of 844 px, and the sign-out submit is a filled black slab beside bordered neighbours. Each law also fails on its own plausible wrong implementation: marking the owner member on /teams but leaving Home marked too is caught by exactly-one; naming the owner member on an admin page whose link the rail will not offer is caught by the never-zero reading; hiding the two links from everyone is caught by the workspace-admin row; hiding them by clip instead of by condition is caught by the visible-and-activatable reading; holding the rail in view with one scroll-container declaration is caught by the revealed-word hit test, which a layout-box reading cannot see; leaving the sticky rail at z-index auto is caught by the same hit test on the board page, because the rail becomes a stacking context and the later positioned column then paints over the word; capping the phone bar height with overflow is caught by the invisible-and-focusable reading; removing the workspace name with display:none on a phone is caught by the one-line-with-ellipsis reading; a quiet control whose word falls below 4.5:1 is caught by the text reading; and exempting every transparent button instead of a control with a visible word is caught by the glyph-only button that still owes a boundary.
```

##### Promises

P1

P2

P3

P4

P5

#### Prefactoring

```text
L1 to L2, in crates/foundry-app/src/admin_tokens.rs: the four renders at 70, 107, 170 and 393 each assemble the rail with the same five arguments. Extract one private async helper that assembles it once, and have the four call sites use it, with no behaviour change (still NavContext::home_for, still Home current). Then the follow-up change names the new section ONCE instead of four times, so three of the four sites cannot be forgotten.
```

```text
Every token page keeps the same rail: the same identity, the same lower-group items, the same Home mark and the same cookie-matched CSRF token in the sign-out form.
```

crates\/foundry\-acceptance\/tests\/features\/navigation\-bar\-linear\-ui\.feature

##### Verification

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=navigation-bar cargo test -p foundry-acceptance"
```

#### Records

##### Decisions

```text
Current-page mark: enum NavSection in crates/foundry-app/src/nav.rs gains Teams, Invites and Tokens beside Home and Board. The carrier stays ONE field, NavContext.active, so the marked member is derived by equality on a sum and a rail with two marks is unrepresentable in Rust. ONE PLACE RECONCILES THE MARK WITH WHAT THE RAIL OFFERS: resolved() reads the identity first, then maps the section the handler named through a total private NavSection::offered_mark(self, is_workspace_admin), which answers Home for Invites and Tokens when the rail will not offer that member and the named section otherwise. Home is always offered, so a rail with zero marks is unrepresentable as well and the marked member is always a member the document renders. The route gate and the rail read the identity separately, so an operator the gate admitted as a workspace admin can still meet a failed nav_identity read; that render offers neither admin link and marks Home, which is what failure B1 declares. Layers. Domain: none, this is presentation only. Application: each authed handler names exactly one section for its own page family, through new NavContext::teams_for, invites_for and tokens_for siblings over the existing private resolved(), which owns the neutral fail-closed identity AND the mark reconciliation, so both stay decided in one place. Adapter: partials/sidebar.html reads one predicate per item (is_teams, is_invites, is_tokens beside is_home, is_board) and emits the sidebar__item--active class and aria-current=page together. Carriers: decision = NavSection (nav.rs); per-request state = NavContext (nav.rs); effects = none; errors = none new.
```

```text
Rights-based visibility: the two admin destinations are wrapped in the same {% if nav.is_workspace_admin %} condition the shipped Integrations item already uses. No new lookup and no new authorization decision: the fact is the one is_workspace_admin value NavContext already carries from the single nav_identity read (ADR-IW-002), which is the same fact member_invites show_invite_form and the admin_tokens require_admin gate read. Teams keeps no condition because GET /teams needs only a session. Application layer supplies the fact, Adapter layer decides presence. On identity-lookup failure the neutral identity reports false, so the rail fails closed and offers only links that open.
```

```text
Sticky rail: the bare .sidebar rule takes position: sticky, top: 0, align-self: flex-start, height: 100vh and an explicit z-index of 20, so the rail stops stretching to the whole document height inside the 100vh-min .app-shell flex row and stays in view while the page scrolls under it. THE SCROLL CONTAINER BELONGS TO THE OPEN GEOMETRY ALONE: overflow-y: auto stays on the bare rule, and [data-nav-collapsed] .sidebar sets overflow: visible. A scroll container clips its scrollable overflow on BOTH axes, because a declared overflow-y of auto makes overflow-x compute to auto as well, and in the folded 72px rail the revealed word is an absolutely positioned descendant placed at left: 100% with a margin, entirely outside the rail padding box; a scrolling folded rail would therefore clip that word out of view and break the shipped nav-collapse-icon-rail promise, with nothing to scroll it back. The open 240px rail draws no flyout at all, so there the scroll container clips nothing and a rail taller than the window still scrolls inside itself. THE RAIL NEEDS ITS OWN STACKING LEVEL: position: sticky makes .sidebar a stacking context even at z-index auto, so the flyout z-index of 30 orders the word only INSIDE the rail and no longer lifts it over the page. The positioned page content - .column and .issue-card, both position: relative at z-index auto - comes later in the document, so a rail left at z-index auto paints UNDER a board column together with its revealed word. z-index: 20 puts the whole rail above that positioned content and below every overlay: the modal at 50, the keyboard-help overlay at 100 and the card drag ghost at 200 all still cover the rail. The comment beside the flyout rule states a premise that no longer holds - that a positioned descendant with a z-index paints above the non-positioned content column - because the content column IS positioned and the rail is now a stacking context; the crafter rewrites that comment to name the rail own level instead. The phone block resets position: static and height: auto, because there the rail is a top bar in a stacked shell. Adapter layer only: no Rust, no template and no class-name change.
```

```text
Phone bar height: below the 480 px breakpoint the bar draws the GLYPH geometry instead of the word geometry, and reuses the shipped folded-rail mechanism. .sidebar__icon becomes visible and .sidebar__label is clipped, never display:none, so each word stays in the accessibility tree and each member is a 44 px thumb square. The workspace name is clipped out of the flow in the same way, not removed: position absolute, a 1 px box and clip-path inset(50%), with text-overflow and overflow untouched. display:none takes the name out of the layout, and the shipped instance-admin-workspace-rename row reads the name box on a phone; a removed box measures 0 and makes that row red. The clip keeps the element in the document for the promise that reads it, for the same reason the member word is clipped. The two wrapping groups then take about two rows instead of about four. Every destination stays inside the bar, and nothing is hidden by a height cap or by overflow.
```

```text
Quiet lower group: the sign-out submit and the JS-built .theme-toggle take the presentation of their neighbour links, .sidebar__user a - transparent fill over the rail surface, --cz-muted text, 5 px radius, no border, no bold - so the global black button fill no longer reaches the rail, and no fill or border makes either control the heaviest thing in the nav. The border .theme-toggle carried for WCAG 1.4.11 goes with the user decision of 2026-11-20: a control INSIDE THE RAIL is exempt from the boundary rule in every layout, exactly as the rail links are. The exemption follows rail MEMBERSHIP, not a drawn word, because the rail has three layouts and the same unchanged control draws its word in only one of them: the open desk rail draws glyph and word, the narrowed rail draws the glyph and reveals the word on hover or focus, and at phone width Sign out draws the glyph alone. A word-keyed exemption would give one control two different answers in two layouts and would owe a 3:1 boundary exactly where the rail is narrowest. The text of a rail control keeps the WCAG 1.4.3 obligation wherever it is DRAWN: against the rail surface in the open layout and against the flyout surface where the narrowed rail reveals it; a clipped word is not drawn and is not read for a ratio. The quiet presentation satisfies no boundary property by itself: the shipped canzan-theme-system walk skips a transparent fill, reads 1:1 for a fill equal to the surface, counts a zero-width border as none and reads no third property. That walk enumerates BUTTONS inside .app-shell, so it cannot read the exemption, and the acceptance designer migrates its scope to DOM containment in the rail element the sidebar partial renders. A control inside .app-shell but outside the rail keeps the 3:1 rule, whether or not it draws a word. The button role, aria-label, title and click behaviour of the theme control are untouched, and the shared .sidebar__user :focus-visible rule is untouched, so the selection ring still reads as a shape.
```

```text
One content-hash migration: the stylesheet is the only file whose name changes, and the rename travels to its four readers in the same commit - the link in templates/base.html, the row and sha256 in static/VENDOR.md, and the three cache-policy assertions in crates/foundry-app/src/lib.rs. check-arch already refuses a /static reference that does not resolve and a content-hashed filename that is not its own sha256 prefix, so a half-done rename is caught by the shipped gate.
```

```text
Claim-to-checker: the mark-and-offer algebra is a finite state model over the closed product of six page families and the four-member rights set, so Quint carries it and TLA+ is not needed - there is no temporal or fairness claim - and Agda is not needed because no proof over an open domain is claimed. The three geometry promises P3, P4 and P5 are real-browser layout facts with no state machine to model, so NO checker covers them: their evidence is the browser-measured acceptance reading, and that limit is recorded on their closure rows rather than hidden behind a model.
```

##### Constraints

```text
P4 bounds the phone bar at a fifth of the screen height at 480 px and below. The browser lane drives ONE emulated phone device, 390 x 844 CSS px, so the measured bound is a bar height at or under 168.8 px there. No authority bounds any other phone width, and no authority bounds the desk width either: the desk reading uses the harness fixed viewport. A reading at a third width is outside the recorded bound and would be a new product question.
```

```text
Who may open a page does not change: GET /workspace/invites and GET /admin/tokens keep the shipped workspace-admin gate and their non-enumerable uniform 404, and GET /teams keeps its session-only gate. This Request only changes what the nav OFFERS and MARKS.
```

```text
The shipped rail law holds unchanged: nothing in the rail may be invisible AND focusable. A destination removed for rights leaves the document entirely - no clip, no zero width, no opacity, no height cap with overflow - and a destination that stays is drawn, reachable and activatable in every geometry.
```

```text
Shipped promises this Request must not break: every rail member carries exactly one inline svg.sidebar__icon and exactly one span.sidebar__label holding its whole word and no aria-label (check-arch rail-member); every primary control is at least 44 px high at phone width (pwa-mobile-rendering); no colour literal outside the three stylesheet token regions and the three regions declare the identical token set (check-arch token seam); exactly one primary nav item is current on every authenticated page (navigation-bar-linear-ui); the sidebar shows a long workspace name on one line that ends in an ellipsis, on a phone as on a desk (instance-admin-workspace-rename). One shipped promise changes, and only in scope: by the user decision of 2026-11-20 the control-boundary rule of canzan-theme-system (3:1 fill or border for every visible button inside .app-shell) reads a rail control drawn as text plus an icon for text contrast instead.
```

```text
The theme control is built by static/js/theme.js and appended into .sidebar__user, so its quiet presentation has to come from the stylesheet alone. With scripting off there is no theme control at all, and P5 is then satisfied by the sign-out submit alone.
```

```text
P3 bounds nothing about the window height, and the design is narrower in one respect: a rail whose own content is taller than the window cannot have its whole box inside the viewport under any mechanism. The open geometry keeps every member reachable by scrolling inside the rail; the folded geometry cannot, because a scroll container there is exactly what clips its revealed word, which is a shipped promise. That bound is carried as risk Y5, never as a satisfied promise. The desk reading uses the harness fixed viewport, where the rail fits.
```

```text
The rail paint order is a band, not a number the rail may choose freely: above the positioned page content that carries no z-index (.column and .issue-card, both position: relative) and below every overlay the stylesheet already declares - the modal at 50, the keyboard-help overlay at 100 and the card drag ghost at 200. A rail above an overlay would cover a modal; a rail at z-index auto is covered by a board column, the flyout included, because position: sticky already makes the rail a stacking context.
```

##### Reuse

NavSection

crates\/foundry\-app\/src\/nav\.rs\:18

Extend

```text
The one carrier that already decides which primary item is marked, by equality on a sum. Three variants are added beside Home and Board; a parallel marker field or a per-item bool set would make two marks representable.
```

resolved

crates\/foundry\-app\/src\/nav\.rs\:136

Extend

```text
Already resolves the identity, the neutral fail-closed fallback and board_href for any section. The three new constructors are thin siblings over it, and the mark reconciliation goes INSIDE it, so the identity, the fallback and the member the rail may mark are decided in one place. A caller-side reconciliation would put that rule in four handlers and let one of them forget it.
```

NavContext\:\:home\_for

crates\/foundry\-app\/src\/nav\.rs\:92

Extend

```text
Three siblings teams_for, invites_for and tokens_for join it with the same signature; home_for stays for the pages that own no primary item (settings, project create, instance admin, unsubscribe).
```

is\_workspace\_admin

crates\/foundry\-app\/src\/nav\.rs\:42

Reuse

```text
The fact the two destinations gate on is already on the carrier from the single nav_identity read (ADR-IW-002) and already gates the Integrations item, so P2 needs no new lookup, no new field and no second reader of the same fact.
```

sidebar\_\_item

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:53

Extend

```text
The one owner of every rail member. The three moved destinations gain the mark conditions, two of them gain the rights condition; no member is renamed or moved and the glyph-and-word shape of each one is kept.
```

sidebar\_\_label

crates\/foundry\-app\/static\/css\/foundry\.2f9e0cb1\.css\:1153

Reuse

```text
The folded geometry clips the word instead of removing it, which is what the phone bar needs and what the workspace name needs. Reusing the mechanism keeps one answer to how a glyph-only member keeps its accessible name and how a clipped element stays readable for the promise that measures it.
```

teams\_for

crates\/foundry\-app\/src\/teams\.rs\:1205

Extend

```text
The single nav assembly for every /teams page and its descendants, so one edit marks Teams on all of them.
```

invites\_for

crates\/foundry\-app\/src\/member\_invites\.rs\:100

Extend

```text
Both invite renders assemble the rail here, the form at 100 and the sent fragment at 140, and both name Invite a member.
```

nav\_for

crates\/foundry\-app\/src\/admin\_tokens\.rs\:64

Extend

```text
One private helper assembles the rail for all four token renders, so the section this module marks is named once. It is the target of the prefactoring move.
```

NAV\_PROBE

crates\/foundry\-acceptance\/src\/support\/rail\_reading\.rs\:85

Extend

```text
The one live reading of the rail already returns each member box, its visibility, its accessible name and which member is marked; the sticky and phone-height readings are two more fields of the SAME probe, so a geometry fact and a mark fact cannot come from two moments. Acceptance support, owned by the acceptance designer.
```

foundry\.2f9e0cb1\.css

crates\/foundry\-app\/templates\/base\.html\:6

Extend

```text
The serving reader of the stylesheet name. The link carries the content hash, with no new head member and no second stylesheet.
```

foundry\.2f9e0cb1\.css

crates\/foundry\-app\/static\/VENDOR\.md\:33

Extend

```text
The auditor row for the hand-authored stylesheet. Its name and its sha256 are recomputed in place, and check-arch refuses a mismatch.
```

static\_cache\_control\_value

crates\/foundry\-app\/src\/lib\.rs\:344

Extend

```text
The three shipped cache-policy assertions name the stylesheet path literally (344, 361, 380); they are re-pointed, not replaced.
```

check\_sidebar\_rail\_members

xtask\/src\/check\_arch\.rs\:2140

Extend

```text
The shipped rail-member law already enumerates members from the partial; the new mark-comes-from-the-carrier obligation joins it as a sibling rule rather than a second gate over the same file.
```

lane\-menu\-trigger

crates\/foundry\-app\/templates\/partials\/board\_columns\.html\:44

Reuse

```text
The outside-rail control the boundary reading is anchored on: every column renders one, it sits outside the rail aside, and it carries its own 1 px --cz-muted border, so it is read for 3:1 and meets it. Consulted owner, read and not edited by this Request; the walk classifies it by DOM containment, and its ellipsis character is text rather than a glyph element.
```

##### Impact

rust\-module

crates\/foundry\-app\/src\/nav\.rs\:18

Changed

```text
Three NavSection variants, three predicates, three constructors, the total NavSection::offered_mark reconciliation called once inside resolved(), and the existing totality unit test extended over every variant and over every section-by-workspace-admin pair.
```

template\-partial

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:48

Changed

```text
Mark conditions on three members, the workspace-admin condition on two of them, and the sign-out button joins the item presentation.
```

stylesheet

crates\/foundry\-app\/static\/css\/foundry\.2f9e0cb1\.css\:1021

Migrated

```text
Sticky rail with the scroll container on the open geometry alone, overflow: visible and an explicit z-index on the rail, phone glyph geometry, the clipped workspace name and the quiet lower group change the bytes, so the content-hashed name changes with them and the old name is removed in the same change.
```

template\-head

crates\/foundry\-app\/templates\/base\.html\:6

Migrated

```text
The stylesheet link carries the new hash.
```

vendor\-manifest

crates\/foundry\-app\/static\/VENDOR\.md\:33

Migrated

```text
The row name and the sha256 of the hand-authored stylesheet are recomputed; check-arch refuses a mismatch.
```

cache\-policy\-assertions

crates\/foundry\-app\/src\/lib\.rs\:344

Migrated

```text
The three immutable-cache assertions name the stylesheet path literally at 344, 361 and 380.
```

rust\-handler

crates\/foundry\-app\/src\/teams\.rs\:1203

Changed

```text
Assembles the rail with Teams current for every teams page and its descendants.
```

rust\-handler

crates\/foundry\-app\/src\/member\_invites\.rs\:100

Changed

```text
Both invite renders (100 and 140) assemble the rail with Invite a member current.
```

rust\-handler

crates\/foundry\-app\/src\/admin\_tokens\.rs\:70

Changed

```text
All four token renders (70, 107, 170, 393) assemble the rail with Machine tokens current.
```

rust\-handler

crates\/foundry\-app\/src\/projects\.rs\:94

UnchangedCompatible

```text
Project create and the board family own no moved destination: Home and Board stay the sections they name (94, 143, 320, 437).
```

rust\-handler

crates\/foundry\-app\/src\/instance\_admin\.rs\:154

UnchangedCompatible

```text
Instance admin, account settings, unsubscribe and comments keep home_for or board_for; they own no primary item, so Home stays marked there exactly as today.
```

architecture\-gate

xtask\/src\/check\_arch\.rs\:2140

UnchangedCompatible

```text
The rail-member law ENUMERATES whatever members the partial holds and already covers the two conditionally rendered members (Integrations, Instance admin), so wrapping two more in a condition costs no edit; one new obligation is added beside it rather than inside it.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/nav\-quick\-actions\.feature\:37

Migrated

```text
This lane asserts the opposite of both promises: it offers Invite a member and Machine tokens to a member with no rights (two scenarios) and it asserts Home is marked on /teams (the final outline). Both laws are replaced by this Request, so the acceptance designer migrates the rows rather than leaving two authorities.
```

acceptance\-steps

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_quick\_actions\.rs\:1

Migrated

```text
Its step bodies carry the destination table and the Home-is-marked reading the migrated feature rows use.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature\:53

Migrated

```text
Its Background signs in Mei, a member with no rights, and EVERY_OPERATOR in the step module owes her Invite a member and Machine tokens; under P2 those two are no longer hers, so the owed set splits into the every-operator set and the workspace-admin set. Its phone scenario also reads the same bar this Request shortens.
```

acceptance\-steps

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_icon\_rail\.rs\:60

Migrated

```text
EVERY_OPERATOR is the named list the lane compares the live rail against; two of its seven members become rights-qualified.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/navigation\-bar\-linear\-ui\.feature\:75

UnchangedCompatible

```text
Its laws are exactly-one-current, the rail is present on every authed page, Board is current on a board and Home on the dashboard, plus CSRF and identity rows. All stay true: this Request only moves WHICH item is the one current member on three page families.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/pwa\-mobile\-rendering\.feature\:81

UnchangedCompatible

```text
No horizontal overflow, the 44 px thumb target, the rail not at full width on a phone and the css-hash consistency check all stay true under the shorter bar and the renamed stylesheet.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature\:159

Migrated

```text
Its two control-boundary rows demand 3:1 from every visible button inside .app-shell, so they refuse the quiet sign-out submit and the quiet theme control: the crafter measured 0.00:1 in the dark palette and the same in the light one. The user decision of 2026-11-20 exempts a control INSIDE THE RAIL from that boundary rule in every layout and keeps its text-contrast duty wherever its word is drawn, so the scope of this law changes and the lane owes a row for the narrowed rail and for the phone layout, where a rail control draws only a glyph. The same rows keep the outside-rail reading honest on a board page, where the lane-actions trigger every column renders is read for 3:1 and meets it. The acceptance designer owns these rows and migrates them; the lane is a support of this slice, never a production target. The focus-ring row, the body-text rows and the token-seam row stay true and are not edited.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/integrations\-wayfinding\.feature\:1

UnchangedCompatible

```text
It asserts the Integrations item for a workspace admin and its absence otherwise, on the same fact and the same condition this Request reuses; nothing it reads changes.
```

acceptance\-steps

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:2264

Migrated

```text
CONTROL_BOUNDARY_PROBE and its assertion hold the enumeration the migrated rows read: button, .button and submit input inside .app-shell. Two changes belong here. The exemption predicate becomes DOM containment in the rail element the sidebar partial renders - the inTheNav fact the probe already reports - instead of containment AND a glyph AND a non-empty word, and the text-contrast reading replaces the boundary reading wherever a rail control word is drawn, against the rail surface in the open layout and against the flyout surface in the narrowed one. The anti-vacuity floor then has to change with it: a count of glyph-only controls loses its last member once the rail is exempt as a whole, because no control outside the rail answers that shape, so the floor becomes at least one control OUTSIDE the rail read for the 3:1 boundary. Owner: acceptance designer.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/instance\-admin\-workspace\-rename\.feature\:234

UnchangedCompatible

```text
Its phone row reads the workspace name box and asks for one line that ends in an ellipsis. The phone bar clips the name out of the flow and keeps the element, its overflow and its text-overflow, so the row stays true. display:none measures a 0 box and makes the row red, which is why the mechanism is a clip. No row of this lane is edited.
```

##### Agreement

The workspace\-admin fact decides BOTH who may open \/workspace\/invites and \/admin\/tokens AND whether the left nav offers those two links\.

crates\/foundry\-app\/src\/member\_invites\.rs\:100

Producer

UnchangedCompatible

```text
The shipped gate on GET /workspace/invites: workspace admin, every other caller the uniform 404. It keeps deciding admission; this Request adds no caller and changes no gate.
```

The workspace\-admin fact decides BOTH who may open \/workspace\/invites and \/admin\/tokens AND whether the left nav offers those two links\.

crates\/foundry\-app\/src\/admin\_tokens\.rs\:294

Producer

UnchangedCompatible

```text
The shipped require_admin gate on /admin/tokens, fail-closed to the same non-enumerable 404; unchanged.
```

The workspace\-admin fact decides BOTH who may open \/workspace\/invites and \/admin\/tokens AND whether the left nav offers those two links\.

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:54

Consumer

Migrated

```text
The nav becomes a reader of the same fact for these two members instead of offering them unconditionally. It reads the value NavContext already carries from the single nav_identity read, so the two parties cannot read two different facts.
```

The workspace\-admin fact decides BOTH who may open \/workspace\/invites and \/admin\/tokens AND whether the left nav offers those two links\.

crates\/foundry\-app\/src\/nav\.rs\:42

Producer

UnchangedCompatible

```text
NavContext.is_workspace_admin is the one presentation projection of that fact, already shipped for the Integrations item and already fail-closed on lookup failure.
```

The served stylesheet filename carries the sha256 prefix of its own bytes\, and every reader names the same filename\.

crates\/foundry\-app\/static\/css\/foundry\.2f9e0cb1\.css\:1

Producer

Migrated

```text
The bytes change, so the name changes.
```

The served stylesheet filename carries the sha256 prefix of its own bytes\, and every reader names the same filename\.

crates\/foundry\-app\/templates\/base\.html\:6

Consumer

Migrated

```text
The document head is the serving reader; a stale hash serves a 404 stylesheet.
```

The served stylesheet filename carries the sha256 prefix of its own bytes\, and every reader names the same filename\.

crates\/foundry\-app\/src\/lib\.rs\:344

Consumer

Migrated

```text
Three cache-policy assertions name the path literally (344, 361, 380).
```

The served stylesheet filename carries the sha256 prefix of its own bytes\, and every reader names the same filename\.

crates\/foundry\-app\/static\/VENDOR\.md\:33

Consumer

Migrated

```text
The auditor row and its recomputed sha256; check-arch refuses a mismatch.
```

Every member of the left\-nav partial carries exactly one inline svg\.sidebar\_\_icon and exactly one span\.sidebar\_\_label holding its whole word and no aria\-label\.

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:53

Producer

UnchangedCompatible

```text
The three moved members keep their glyph and word; only conditions and classes around them change.
```

Every member of the left\-nav partial carries exactly one inline svg\.sidebar\_\_icon and exactly one span\.sidebar\_\_label holding its whole word and no aria\-label\.

xtask\/src\/check\_arch\.rs\:2140

Consumer

UnchangedCompatible

```text
The gate enumerates members from the file and already reads conditionally rendered ones, so it needs no edit.
```

##### Not applicable

contract_tests

```text
The two parties of the workspace-admin agreement - the route gates and the nav template - live in one deployable, are released together and change in one commit, and no persisted or wire data outlives a version. The primary oracle already runs the real producer against the real consumer: it opens every href the rail offered under the same session cookie and reads the status the shipped gate answers, which is stronger than a recorded pact.
```

##### Boundaries

```text
HTTP GET on the authenticated routes of the running application carrying a real session cookie (/, /team/{team}/project/{project}, /teams, /workspace/invites, /admin/tokens, /account/settings), and for the three geometry promises the same GET rendered in the real browser the @needs-browser lane drives, read at the harness desk viewport and at the emulated 390x844 phone device.
```

###### Driven ports

```text
Postgres through the shipped fail-closed nav_identity and is_instance_admin lookups (no new query, no new authorization).
```

```text
The served static stylesheet and static/js/theme.js, read by the browser as the application serves them.
```

```text
Inward: the template and the stylesheet read the NavContext projection; NavContext reads the already-resolved session identity; nothing in the rail reaches back into a store, a route table or an authorization decision.
```

###### Failures

####### B1

```text
The nav_identity lookup fails for this render, so the rail own read reports a neutral identity while the route gate has already admitted the acting operator.
```

Refusal

```text
The page still answers 200 with a neutral rail that offers neither admin link and marks Home, on /workspace/invites and /admin/tokens as on every other page, because resolved() reconciles a section the rail will not offer to Home. The error is logged, nothing is offered that would refuse her, and no render carries zero marks.
```

####### B2

```text
A plain member asks for /workspace/invites or /admin/tokens directly, by typed URL or a stale bookmark, where the nav no longer offers the link.
```

Refusal

```text
The shipped non-enumerable uniform 404, unchanged: the nav hiding the link neither adds nor removes an admission decision.
```

##### Observations

```text
The set of primary nav members the served document offers, each read by its own word and the href it opens.
```

Set\[\(word\, href\)\]

```text
Which identity fact caused an absence, and the order the stylesheet draws them in.
```

```text
Which nav members carry aria-current=page together with the sidebar__item--active class.
```

Set\[word\]

```text
The tint, the weight and anything else about how current is drawn.
```

```text
The HTTP status the application answers for each offered href under the SAME session cookie the rail was rendered for.
```

Map\[href\, status\]

```text
A 404 for a page that does not exist from a 404 for a page that refuses her - that non-enumerability is the shipped promise.
```

```text
The rail bounding box relative to the viewport at desk width after the page is scrolled to its bottom, with each member box and its visibility.
```

\(top\, bottom\, height\, Set\[\(word\, visible\)\]\)

```text
Whether the rail is held by sticky positioning, by a fixed box or by a scroll container.
```

```text
The rail height as a fraction of the viewport height at the emulated phone device, with every member visible and activatable inside the bar.
```

\(fraction\, Set\[\(word\, visible\, activatable\)\]\)

```text
How many rows the members wrapped into, and which glyph each one draws.
```

```text
The resolved fill, border width and text colour of the sign-out submit and of the theme control, each beside the same reading of a neighbour link in the lower group.
```

Map\[member\, \(fill\, borderWidth\, colour\, fontWeight\)\]

```text
Which token produced a colour, and any difference below the resolved value.
```

```text
That one press of the sign-out submit ends the session, and one press of the theme control still changes the palette.
```

\(signedOut\, paletteChanged\)

```text
How the palette travels, which the theme lane owns.
```

```text
Whether the revealed word of a folded rail member is visible beside the rail while that member holds the pointer or the keyboard, read by the centre hit test of the revealed box rather than by its layout box.
```

Map\[word\, \(revealed\, hitsItsOwnWord\)\]

```text
Which ancestor clipped it, and the box the word would have had if nothing clipped it.
```

##### Equality

the eight observations above

```text
Two rendered rails are the same rail when they offer the same set of word-and-href pairs, mark the same set, answer the same statuses for those hrefs, hold the same geometry predicates (whole box inside the viewport after the scroll at desk width, the revealed word of a focused folded member hit-testing back to itself, height at or under a fifth of the viewport at the phone device) and give the sign-out submit and the theme control the same resolved fill, border and text tier as their neighbour links. Two rails that differ only in row wrapping, tint, glyph, token name or source order are equal.
```

##### Constructors

NavSection\:\:Teams

NavSection

NavSection\:\:Invites

NavSection

NavSection\:\:Tokens

NavSection

NavContext\:\:teams\_for

async fn\(\&AppState\, Uuid\, Uuid\, bool\, String\) \-\> NavContext

NavContext\:\:invites\_for

async fn\(\&AppState\, Uuid\, Uuid\, bool\, String\) \-\> NavContext

NavContext\:\:tokens\_for

async fn\(\&AppState\, Uuid\, Uuid\, bool\, String\) \-\> NavContext

NavSection\:\:offered\_mark

fn\(NavSection\, bool\) \-\> NavSection

##### Laws

###### L1

```text
On every authenticated page a signed-in operator can open, exactly one primary nav member carries aria-current=page - never two and never zero - and it is the member that owns that page family whenever the rail offers her that member: Home on the dashboard and on every page no primary member owns, Board on the board family, Teams on /teams and its descendants, Invite a member on /workspace/invites, Machine tokens on /admin/tokens. When the rail will not offer the owning member, because the rail own identity read reported no workspace admin for this render, the marked member is Home, which the rail always offers.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/partials\/sidebar\.html

###### Treatment

Finite

###### Cases

```text
dashboard, any rights: Home
```

```text
board family, any rights: Board
```

```text
teams and a team page, any rights: Teams
```

```text
invites, workspace admin, identity read resolved: Invite a member
```

```text
tokens, workspace admin, identity read resolved: Machine tokens
```

```text
account settings, any rights: Home
```

```text
The domain is the closed product of six page families and the four-member rights set, with the rail own identity read RESOLVED; every reachable row is enumerated, so a generator would only re-draw the same table. The branch where that read fails is not a row here: no public surface and no declared seam can produce it, so it owes no acceptance row and is stated as its own NoTest law.
```

###### Evidence

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

###### L2

```text
The nav never marks a member it does not offer: the marked set is a subset of the offered set in every render, and it is never empty on a page she can open.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/partials\/sidebar\.html

###### Treatment

Finite

###### Cases

```text
invites page, workspace admin, identity read resolved: Invite a member is both offered and marked
```

```text
any page, plain member: the marked member is one of the three she is offered
```

```text
Same closed domain as the mark law, with the rail own identity read RESOLVED. The failed-read branch is stated as its own NoTest law, because no public surface can produce it.
```

###### Evidence

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

###### L3

```text
Every link the nav offers answers the SAME session cookie with a page, never the refusal 404: Invite a member and Machine tokens are present exactly when the identity the rail was rendered for is a workspace admin, and absent otherwise.
```

Invariant

Application

crates\/foundry\-app\/src\/nav\.rs

###### Treatment

Finite

###### Cases

```text
plain member: neither link offered
```

```text
workspace admin: both offered, both answer 200
```

```text
instance super-admin who administers no workspace: neither link offered, because /admin/tokens and /workspace/invites refuse her
```

```text
both admin kinds: both offered, both answer 200
```

```text
The rights domain is the closed four-member set the shipped lookups can report, so the four rows exhaust it.
```

###### Evidence

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

###### L4

```text
No operator loses a destination her rights already open: every primary destination that admits her is still offered to her.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/partials\/sidebar\.html

###### Treatment

Finite

###### Cases

```text
plain member: Home, Board and Teams all still offered
```

```text
workspace admin: all five offered
```

```text
Same closed rights set.
```

###### Evidence

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

###### L5

```text
At desk width, on a page taller than the window, the whole rail box lies inside the viewport after the page is scrolled to its bottom (top at or above 0, bottom at or below the viewport height) and every rail member stays visible and activatable - in the open geometry and in the folded one.
```

Invariant

Adapter

crates\/foundry\-app\/static\/css

###### Treatment

Finite

###### Cases

```text
long page, open rail, scrolled to the bottom
```

```text
long page, folded rail, scrolled to the bottom
```

```text
short page, open rail: the rail is unmoved from what it shipped
```

```text
short page, folded rail: unmoved
```

```text
Two geometries at one harness viewport on one page known to be taller than the window: four enumerated readings, no generated domain.
```

###### Evidence

None.

###### L6

```text
At the emulated 390x844 phone device the rail bar height is at most a fifth of the viewport height (168.8 px), and every destination the operator is offered plus every lower-group member is visible and activatable inside the bar, with no member below 44 px and no horizontal overflow of the document.
```

Invariant

Adapter

crates\/foundry\-app\/static\/css

###### Treatment

Finite

###### Cases

```text
plain member, bar as served
```

```text
workspace admin and instance admin, ten members
```

```text
folded choice stored, same device
```

```text
scripting off, same device
```

```text
One device, two rights rows (the plain member with eight members, the admin with ten) and the two geometries: four enumerated readings.
```

###### Evidence

None.

###### L7

```text
In the lower group the sign-out submit and the theme control resolve to the same fill as the rail surface, no border their neighbour links lack and the same text tier and weight as those links; and both still act - one press of Sign out ends the session, one press of the theme control changes the palette. NO CONTROL INSIDE THE RAIL OWES A 3:1 FILL OR BORDER BOUNDARY, in any layout: the exemption follows rail MEMBERSHIP, as it already does for the rail links, and not whether a word happens to be drawn, so it holds in the open desk layout, in the narrowed rail where both controls draw only a glyph, and at phone width where Sign out draws only a glyph. Wherever a rail control DRAWS its word, that word reaches 4.5:1 against the surface behind it in both palettes - the rail surface in the open layout, the flyout surface where the narrowed rail reveals it. EVERY control inside .app-shell but OUTSIDE the rail is read for the 3:1 boundary and meets it, whether or not it draws a word, and at least one such control is read on every board page the law runs on.
```

Invariant

Adapter

crates\/foundry\-app\/static\/css

###### Treatment

Finite

###### Cases

```text
light palette, open desk layout: sign-out fill equals the rail surface, border width 0, colour and weight equal a neighbour link
```

```text
dark palette, open desk layout: the same, through the re-bound tokens
```

```text
one press of Sign out still ends the session
```

```text
one press of the theme control still changes the palette
```

```text
scripting off: no theme control exists and the sign-out submit alone carries the law
```

```text
both palettes, open desk layout: the sign-out word and the theme-control word each reach 4.5:1 against the rail surface, and neither control is read for a fill or border boundary
```

```text
narrowed rail, both palettes: Sign out and the theme control draw only a glyph and are still read for no fill or border boundary; the word each one reveals reaches 4.5:1 against the flyout surface behind it
```

```text
phone device, both palettes: Sign out draws only a glyph and is still read for no fill or border boundary; its word is clipped and not drawn, so no text ratio is read for it there
```

```text
the lane-actions trigger on a board page, .lane-menu-trigger in partials/board_columns.html, is outside the rail, so it IS read for the 3:1 boundary in both palettes and meets it by its own 1 px --cz-muted border; a board page always renders at least one column, so the reading always has this member
```

```text
the floor: on each board page the law runs on, AT LEAST ONE control outside the rail was read for the 3:1 boundary; a reading in which every control has left the boundary reading is red, whatever the controls look like
```

```text
Two controls, two palettes, the with-scripting and without-scripting split, the three layouts the rail has - open desk, narrowed desk, phone - and the outside-rail reading with its floor: a closed enumeration of readings on one document. The exemption is read as DOM containment in the rail element the sidebar partial renders, the aside that holds #sidebar-nav, never as the control own class or fill and never as whether a word is drawn. Two consequences for the walk, both owned by the acceptance designer: the nav-collapse toggle is mounted into .sidebar__brand inside that aside, so it becomes exempt with the other rail controls; and the floor can no longer be a count of glyph-only controls, because after the exemption widens no control outside the rail answers that shape - the lane-actions trigger draws an ellipsis character, not a glyph element, so the walk reads its glyph flag as false and its word as non-empty. The floor is therefore a control OUTSIDE THE RAIL being read at all. An open modal gives a second such control, .modal-close in partials/edit_lane_modal.html and partials/delete_issue_modal.html, if a board page ever stops rendering a column.
```

###### Evidence

None.

###### L8

```text
In the folded desk geometry, while the rail is held in view on a page taller than the window, the revealed word of a rail member is VISIBLE beside the rail when that member holds the pointer or the keyboard: the centre of the revealed box hit-tests back to the word itself or to a node inside it, never to an ancestor and never to page content the word overlaps. No ancestor of a rail member clips that word out of view, and no positioned page content paints over it.
```

Invariant

Adapter

crates\/foundry\-app\/static\/css

###### Treatment

Finite

###### Cases

```text
folded rail, desk width, a board page with cards scrolled to the bottom, a primary nav member holds keyboard focus and its revealed word overlaps a column: the word hit-tests back to itself, not to the column and not to a card
```

```text
folded rail, desk width, the same board page, a lower-group member holds keyboard focus and its revealed word overlaps a column: the word hit-tests back to itself
```

```text
folded rail, desk width, an open modal or the keyboard-help overlay: the overlay covers the rail, which is the declared order, and no word is read for this law
```

```text
open rail, desk width: no flyout is drawn, so no word is read for this law and the rail may scroll inside itself
```

```text
Two revealed words in one geometry at one harness viewport, read on the board page because that is the surface whose own content is positioned beside the rail: enumerated readings, no generated domain. The LAYOUT box of the word is not the reading, because clip, clip-path, an ancestor overflow and a later positioned sibling all leave that box untouched; the reading is the clip-aware centre hit test crates/foundry-acceptance/src/support/rail_reading.rs already owns, which fails both when the word is clipped and when something paints over it. Owner of the row: acceptance designer.
```

###### Evidence

None.

###### L9

```text
When the rail own identity read fails for a render the route gate has already admitted, resolved() reconciles the section the page named to Home before the rail is built, so the served rail offers neither admin link and marks Home - one mark, never zero - on /workspace/invites and /admin/tokens as on every other page.
```

Invariant

Application

crates\/foundry\-app\/src\/nav\.rs

###### Treatment

NoTest

```text
No public surface and no declared seam can make that read fail: resolve_admin at crates/foundry-app/src/admin_tokens.rs:293 admits only a workspace admin, and the nav_identity read carries no public fault injection, so no acceptance row can drive this branch and none is owed. The DESIGN claim is closed by the Quint invariant markNeverEmpty, whose resolve=none mutant reproduces the zero-mark defect with a trace. The crafter totality unit test over every section-by-rights pair of NavSection::offered_mark is the craft evidence that the code matches the model; the runtime branch stays unobserved.
```

###### Evidence

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

##### Refuted

```text
Bounding the phone bar with max-height plus overflow satisfies P4.
```

```text
The carrier cannot express the promise: a height cap keeps every member in the layout and in the focus order while painting only the rows that fit, which is exactly the invisible-and-focusable state the shipped rail law forbids, and it also breaks every destination stays one press away.
```

```text
A 320 px wide phone with ten members: the last wrapped row falls outside the capped box, the member keeps its box and its tab stop, and the keyboard walk arrives at a destination the operator cannot see.
```

```text
The template can mark the current page by comparing the request path, so no new NavSection variant is needed.
```

```text
The rail carrier holds no request path and the template is given none; adding one would put a second, path-shaped authority beside NavContext.active and make two marks representable.
```

```text
A team page /team/general/members has a path no primary member equals, so a path comparison marks nothing there while the owner-family rule marks Teams.
```

```text
The quiet lower group satisfies the shipped canzan-theme-system control-boundary law unchanged, because a control drawn as text plus an icon needs no fill and no border to identify it.
```

```text
Law scope, not the stylesheet. The shipped walk enumerates every visible button inside .app-shell and asks for 3:1 from the better of the own opaque fill and the border. It skips a transparent fill, counts a zero-width border as none and reads no third property, so the text-plus-icon argument never reaches the checker. The theme control has no escape at all, because its fill is the very surface it is measured against. Only the scope of that law can change, and only the acceptance designer owns it: the scope becomes rail MEMBERSHIP, which gives the same answer in every layout, the narrowed rail and the phone bar included, where the word is not drawn at all.
```

```text
Dark palette: button.sidebar__item [Sign out] computes 0.00:1 against #0f1312, and the light twin computes the same against #f3f4f1. Rows red: every large-text and control-boundary pair reaches at least 3 to 1, and the same holds when the device preference is light.
```

```text
On a phone the workspace name can leave the layout with display:none, exactly as it does when the rail is folded.
```

```text
The carrier of the promise is the name box itself. A shipped row measures that box on a phone, so the element must stay in the document. The clip the design already blesses for a member word is the mechanism: position absolute, a 1 px box and clip-path inset(50%), with overflow and text-overflow untouched.
```

```text
instance-admin-workspace-rename, phone row: the sidebar shows the workspace name on one line ending in an ellipsis. With display:none the reading is scrollWidth 0 and clientWidth 0, so the row goes red. With the clip the same row passes and the bar still measures two rows.
```

```text
One overflow-y: auto declaration on the sticky rail keeps every member reachable in both geometries and breaks no shipped promise.
```

```text
The carrier, not a special case. A declared overflow-y of auto makes overflow-x compute to auto as well, so the rail clips its scrollable overflow on BOTH axes. The folded rail revealed word is an absolutely positioned descendant whose containing block lies inside that scroller, so no declaration on the word itself escapes the clip. The scroll container therefore moves to the open geometry, which draws no flyout, and the folded geometry keeps overflow: visible. No CSS condition reads content height, so the over-tall folded rail stays a recorded residue rather than a third mechanism.
```

```text
Folded rail, 72px strip, desk width: a member takes keyboard focus and its word is placed at left: 100% plus 8px, entirely outside the rail padding box, where it is clipped out of view with nothing to scroll it back. No shipped lane sees it: the flyout assertion compares the word LAYOUT box while focused against the same box while unfocused, and an ancestor overflow leaves a layout box untouched.
```

##### Claims

###### C1

```text
A rail that marks two primary members, or none, is unrepresentable in the server carrier.
```

```text
The operator cannot tell where she is, or believes she is somewhere she is not.
```

crates\/foundry\-app\/src\/nav\.rs

```text
One NavSection variant chosen by the handler for the page it renders, together with the workspace-admin fact the rail own identity read reported.
```

NavContext\:\:resolved

crates\/foundry\-app\/templates\/partials\/sidebar\.html

```text
The set of members carrying aria-current=page in the served document.
```

```text
Rust excludes two marks in the carrier and, through the total NavSection::offered_mark reconciliation inside resolved(), excludes zero marks as well - but it excludes neither in the template: a predicate applied to the wrong member, or a hard-coded aria-current in the partial, still emits a wrong or second mark. The new check-arch obligation and the per-family acceptance rows cover that, and the nav.rs totality test covers a future variant and every section-by-rights pair of offered_mark.
```

###### C2

```text
The nav offers a link only when the destination admits the identity the rail was rendered for.
```

```text
A link that answers her own session with a refusal 404 teaches her the app is broken, and it advertises a surface she may not use.
```

crates\/foundry\-app\/src\/nav\.rs

```text
The one is_workspace_admin value resolved with the identity for this render.
```

NavContext\:\:resolved

crates\/foundry\-app\/templates\/partials\/sidebar\.html

```text
The offered set, and the status each offered href answers under the same cookie.
```

```text
Rust cannot tie a template condition to a route gate: a future change to either gate, or a new admin-gated destination added with no condition, re-opens it. The acceptance law that walks every offered href and reads its status is the standing check, not the type.
```

##### States

Admitted

```text
Plain member on /teams: Teams marked, Invite a member and Machine tokens absent, three destinations offered and all three answer her.
```

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Admitted

```text
Workspace admin on /admin/tokens: Machine tokens marked, five destinations offered, all five answer her.
```

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Admitted

```text
Identity lookup failed for this render: the neutral identity reports no workspace admin, the two admin links are absent, the section the page named is reconciled to Home, and the page still renders 200 with Home marked - on /workspace/invites and /admin/tokens as on every other page.
```

crates\/foundry\-app\/src\/nav\.rs

Excluded

```text
A rail with zero or two primary members carrying aria-current=page.
```

crates\/foundry\-app\/src\/nav\.rs

Excluded

```text
A rail that offers Invite a member or Machine tokens to an identity those pages refuse.
```

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Excluded

```text
A rail member that is invisible and still focusable, by clip, by zero size, by opacity or by a height cap with overflow.
```

crates\/foundry\-app\/static\/css

Unknown

```text
The measured bar height at phone widths other than the lane device 390x844, for instance 320x568, where ten members at 44 px may exceed a fifth of a shorter screen.
```

product\-owner

Unknown

```text
Whether a future primary destination with its own gate is added without a nav condition; no type and no gate prevents it.
```

solution\-architect

##### Models

Quint

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

###### Properties

exactlyOneMarked

Invariant

```text
On a page she can open, the marked set has exactly one member, the failed-read branch included.
```

NavContext\.active

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:46

markedIsOwnPageOrHome

Invariant

```text
The marked member owns the page family, and Home both for a family no member owns and for a page whose own member the rail will not offer.
```

NavSection\:\:offered\_mark

crates\/foundry\-app\/src\/nav\.rs\:18

markedIsOffered

Invariant

```text
The marked set is a subset of the offered set.
```

NavContext\.is\_workspace\_admin

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:54

offeredImpliesAdmitted

Invariant

```text
Every offered member is admitted by its destination for the identity the ROUTE GATE reads, not for the identity the rail read.
```

NavContext\.is\_workspace\_admin

crates\/foundry\-app\/src\/nav\.rs\:42

admittedIsOffered

Invariant

```text
With the rail own read in agreement with the acting session, every member her rights admit is still offered.
```

NavContext\.is\_workspace\_admin

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:53

###### Atoms

State

page

the route family of the handler that renders the rail

crates\/foundry\-app\/src\/lib\.rs\:800

None.

State

gateRights

the workspace\-admin fact the route gate reads for this request

crates\/foundry\-app\/src\/admin\_tokens\.rs\:294

None.

State

navRights

NavContext\.is\_workspace\_admin\, from the rail own nav\_identity read

crates\/foundry\-app\/src\/nav\.rs\:42

None.

State

mark

NavContext\.active

crates\/foundry\-app\/src\/nav\.rs\:44

None.

Action

openAnotherPage

one authenticated GET that assembles a fresh NavContext

crates\/foundry\-app\/src\/nav\.rs\:136

```text
One render per request: the whole rail is assembled and served in one step, identity read included, so no interleaving is modelled.
```

Constant

admits

the shipped route gates\: workspace admin for \/workspace\/invites and \/admin\/tokens\, session only for the rest

crates\/foundry\-app\/src\/admin\_tokens\.rs\:294

None.

###### Assumptions

```text
The rights domain is the closed four-member set the shipped lookups can report.
```

```text
Six page families, five of which own a primary member.
```

```text
One render per request: the operator opens another page with the same acting identity and the rail re-reads its own identity on each render; no temporal or fairness claim.
```

```text
The shipped gates are the admit function and do not change in this Request.
```

```text
The rail own read reports either the acting identity or the neutral one; nothing else is representable.
```

###### Mutants

######## N3

```text
offer=all: the two admin links are offered to every signed-in operator (the shipped behaviour this Request replaces).
```

###### Kills

noRefusedLinkOffered

######## N4

```text
mark=home: Teams, Invite a member and Machine tokens mark nothing and Home stays marked there (the shipped behaviour this Request replaces).
```

###### Kills

ownPageIsMarked

######## N5

```text
resolve=none: a page keeps naming its own member even when the rail will not offer it, which is the unreconciled composition of P1 and P2 the first candidate shipped; on /admin/tokens with a failed rail read the rail then marks nothing at all.
```

###### Kills

markNeverEmpty

exactlyOneMarked

##### Closure

```text
the current-page mark (P1)
```

Safety

Pass

DesignedNotBuilt

```text
Quint checked exactlyOneMarked, markedIsOwnPageOrHome and markedIsOffered over the closed space of six page families, the four-member rights set and the two outcomes of the rail own identity read, symbolically to the model step bound and by randomized run. The checks tell the designs apart: the shipped mark=home variant is refuted with a trace, and so is the unreconciled variant that keeps naming a member the rail does not offer, whose trace marks NOTHING on /admin/tokens for an operator the route gate admitted. The runtime still marks Home on the three pages.
```

###### Evidence

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

```text
the current-page mark (P1)
```

Inhabitation

Pass

DesignedNotBuilt

```text
Each of the three new sections is built by its own constructor from the identity the handler already holds, and the model exhibits a marked state for every page family.
```

###### Evidence

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

```text
the current-page mark (P1)
```

PublicReachability

Pass

DesignedNotBuilt

```text
handler chooses NavSection (teams.rs:1203, member_invites.rs:100, admin_tokens.rs:70) to NavContext::teams_for/invites_for/tokens_for to partials/sidebar.html to aria-current in the body of an authenticated GET, which is the oracle stimulus.
```

###### Evidence

None.

```text
the rights-based visibility of the two links (P2)
```

Safety

Pass

DesignedNotBuilt

```text
Quint checked offeredImpliesAdmitted against the identity the ROUTE GATE reads, and admittedIsOffered over the four-member rights set bounded to a rail read that agreed with the acting session, which is the scope failure B1 declares. The unguarded form fails on the shipped offer=all variant with a plain member.
```

###### Evidence

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

```text
the rights-based visibility of the two links (P2)
```

Inhabitation

Pass

DesignedNotBuilt

```text
A workspace admin renders a rail with both links; the already-shipped Integrations item proves the same condition over the same fact builds.
```

###### Evidence

None.

```text
the rights-based visibility of the two links (P2)
```

PublicReachability

Pass

DesignedNotBuilt

```text
nav_identity read to NavContext.is_workspace_admin (nav.rs:42) to the template condition to the offered set of an authenticated GET, and each offered href is then opened through the same public port.
```

###### Evidence

None.

```text
the sticky rail, the short phone bar and the quiet lower group (P3, P4, P5)
```

Safety

Pass

DesignedNotBuilt

```text
No checker covers these: they are real-browser layout facts with no state machine. Each has one declared executable observation through the browser lane - the rail box after a scroll in both geometries, the revealed word of a focused folded member read by the clip-aware centre hit test, the bar height over the viewport height at the 390x844 device, and the resolved fill, border, colour and weight beside a neighbour link - and a falsifier the shipped stylesheet fails today. That is design evidence, not proof. A folded rail taller than the window is outside the declared bound and is carried as risk Y5.
```

###### Evidence

None.

```text
the sticky rail, the short phone bar and the quiet lower group (P3, P4, P5)
```

Inhabitation

Pass

DesignedNotBuilt

```text
Every mechanism is already in the stylesheet: the folded geometry clips the word and shows the glyph, and .sidebar__user a already carries the quiet presentation the two controls take.
```

###### Evidence

None.

```text
the sticky rail, the short phone bar and the quiet lower group (P3, P4, P5)
```

PublicReachability

Pass

DesignedNotBuilt

```text
stylesheet bytes to the content-hashed name to the link in base.html to the served document the browser lays out and the lane measures.
```

###### Evidence

None.

```text
the whole value
```

Liveness

Pass

DesignedNotBuilt

```text
Not applicable, with a concrete reason: one authenticated GET assembles and serves the whole rail in one synchronous render, there is no queue, retry, background step or eventual state, and no promise claims progress over time. There is therefore no temporal or fairness property to check and no TLC run is owed.
```

###### Evidence

None.

```text
the mark on a page whose own member the rail will not offer (P1, failure B1)
```

Safety

Pass

DesignedNotBuilt

```text
The Quint invariant markNeverEmpty holds over the closed space with the rail own read failing, and its resolve=none mutant refutes the unreconciled composition with a trace that marks NOTHING on /admin/tokens for an operator the route gate admitted. The crafter totality unit test over every section-by-rights pair of NavSection::offered_mark is the craft evidence that the code matches the model. No acceptance row is owed.
```

###### Evidence

docs\/feature\/ui\-nav\-polish\/design\/nav\_mark\_and\_offer\.qnt

```text
the mark on a page whose own member the rail will not offer (P1, failure B1)
```

PublicReachability

Blocked

Indeterminate

```text
No public surface and no declared seam reaches this branch: resolve_admin at crates/foundry-app/src/admin_tokens.rs:293 admits only a workspace admin and the nav_identity read carries no public fault injection, so no lane can drive it and none is asked to. The claim is therefore unchecked at the public port and stays BLOCKED with an indeterminate runtime, never PASS; the model and the unit test carry it instead.
```

###### Evidence

None.

##### Risks

###### Y1

Stressor

```text
Negate the phone device assumption: the operator holds a 320x568 phone instead of the 390x844 the lane drives.
```

```text
Every destination stays in the bar, visible, 44 px and activatable; the document never widens sideways.
```

```text
The bar takes more than a fifth of that shorter screen because the glyph rows wrap once more.
```

```text
A destination clipped out of the bar, a member below 44 px, or horizontal overflow.
```

```text
The wrapping glyph bar with 44 px targets, which degrades by growing rather than by hiding.
```

product\-owner

###### Y2

Stressor

```text
Negate the identity assumption: nav_identity fails for this render.
```

```text
The page renders 200 and the nav offers only links that open.
```

```text
A real workspace admin loses Invite a member and Machine tokens on that render.
```

```text
Offering a link that answers her own session with the refusal 404, or a 500 in place of the page.
```

```text
The shipped neutral fail-closed identity, which already reports no workspace admin.
```

solution\-architect

###### Y3

Stressor

```text
Negate the scripting assumption: scripting is off, so theme.js builds no control and nav-collapse.js stamps no mark.
```

```text
The sticky rail, the short phone bar and the quiet sign-out submit, all of which are stylesheet-only.
```

```text
No theme control exists at all, so P5 rests on the sign-out submit alone.
```

```text
A rail whose sign-out is unreachable, or a lower group where the submit is still the heaviest thing in the nav.
```

```text
The server-rendered rail: every mark, every condition and every geometry rule is in the document and the stylesheet.
```

crafter

###### Y4

Stressor

```text
Second, independent stressor set walked over the finished design. Negate the assumption that a workspace admin is the only identity with elevated rights: an instance super-admin who administers no workspace opens the rail.
```

```text
She is offered Home, Board and Teams, which admit her, and Instance admin in the lower group.
```

```text
She is not offered Invite a member or Machine tokens, because the shipped gates on both pages require workspace admin and would refuse her.
```

```text
Offering her either link because she is an admin of some kind, which would conflate two independent facts.
```

```text
The design reads only is_workspace_admin for these two members; the model row rights=instance admin shows the hidden case, so the walk found no gap.
```

solution\-architect

###### Y5

Stressor

```text
Second set, continued. Negate the assumption that the rail is shorter than the window at desk width: a short window or a large text size makes the rail taller than the viewport while the page is also long.
```

```text
In the open geometry every rail member stays reachable, because the open rail is the scroll container and scrolls inside itself.
```

```text
In the open geometry the operator scrolls within the rail to reach its last member. In the folded geometry the rail paints its last members below the window edge, where the sticky box does not scroll them back.
```

```text
A member clipped out of view with no way to reach it in the open geometry, and a revealed word clipped out of view in the folded geometry.
```

```text
overflow-y: auto on the OPEN rail alone. The folded rail keeps overflow: visible, because the scroll container is exactly what clips its revealed word; the folded rail is the shorter of the two, since the workspace name and the identity leave the layout and every target is a 40px square. No CSS condition reads content height, so no single declaration closes both the over-tall folded rail and the revealed word.
```

crafter

###### Y6

SinglePointOfFailure

```text
The content-hashed stylesheet name: one rename, four readers (base.html, VENDOR.md, three lib.rs assertions).
```

```text
Every /static reference resolves and every content-hashed filename is its own sha256 prefix.
```

```text
Nothing.
```

```text
A served document linking a stylesheet that 404s, or a VENDOR row whose sha256 does not recompute.
```

```text
The shipped check-arch static-reference and sha256 gates, plus the pwa css-hash scenario.
```

crafter

###### Y7

Bypass

```text
The template is outside the Rust carrier: a predicate applied to the wrong member, a hard-coded aria-current, or a future authed handler that passes NavSection::Home on a page that owns a member. Devtools or an extension can also add a second aria-current in the browser.
```

```text
The server carrier still holds exactly one section per render.
```

```text
A mark wrong in the document that no Rust type can refuse.
```

```text
The server emitting two marks or none.
```

```text
The new check-arch obligation that no rail member carries aria-current outside a nav.is_ condition, the per-family acceptance rows, and the nav.rs totality test over every variant.
```

crafter

###### Y8

Bypass

```text
The 1.4.11 exemption is a scope rule inside the acceptance walk, not a property of the stylesheet, and it now keys on rail MEMBERSHIP. Any control later placed inside the rail element inherits the exemption with no boundary of its own - the nav-collapse toggle, mounted into .sidebar__brand, already does - and a control moved out of the rail loses it silently. No Rust type and no gate refuses either move.
```

```text
Every control inside .app-shell but outside the rail is still read for the 3:1 fill or border rule and meets it, and every rail control word that is actually drawn keeps the WCAG 1.4.3 text ratio against the surface behind it.
```

```text
A new rail control may ship quiet, with no fill and no border, in every layout.
```

```text
An exemption read from the control own class, tag or fill, or from whether a word is drawn: the first skips every quiet button in the shell, and the second gives one unchanged control two different answers in two layouts.
```

```text
The floor inside the walk: at least one control OUTSIDE the rail is read for the 3:1 boundary on every board page the law runs on, anchored on the lane-actions trigger that every column renders. It is a reading, not a count of a shape, so it goes red the moment the exemption widens past the rail and the outside-rail controls leave the reading. Owner: acceptance designer.
```

acceptance\-designer

###### Y9

Stressor

```text
Negate the assumption that a word and a glyph are enough to show a control: the operator needs a boundary, for example in forced-colours mode or with a high-contrast need.
```

```text
Forced-colours mode draws its own boundaries from the user agent, and the shared .sidebar__user :focus-visible outline still reads as a shape for the keyboard.
```

```text
In the normal palette the two rail controls read as quiet items, like the rail links beside them.
```

```text
Removing the focus outline, or hiding the word or the glyph of either control.
```

```text
The user decision of 2026-11-20 accepts this residue for a rail control drawn as text plus an icon; the rail links already carry the same one.
```

product\-owner

###### Y10

Bypass

```text
The revealed word holds neither its clip nor its paint order by type. Any later overflow, clip-path, contain or filter on a rail ancestor clips it again, and any change to the rail own z-index, or any page content that takes a z-index above it, paints over it. position: sticky already makes the rail a stacking context, so the word z-index of 30 orders nothing outside the rail. No gate refuses either change, and the shipped flyout assertion compares the word LAYOUT box, which both a clip and a cover leave untouched, so the regression is silent in every shipped lane.
```

```text
The word keeps its box, its accessible name and its reveal rules, and the member itself stays visible and activatable.
```

```text
Nothing.
```

```text
A folded rail whose revealed word is clipped out of view, or covered by page content, while the member still reads as visible.
```

```text
The clip-aware law and its browser row are the standing check, read with the centre hit test rail_reading.rs already owns rather than with a layout box, and read on a board page where the word overlaps a positioned column. Owner of the row: acceptance designer.
```

acceptance\-designer

##### Architectural tests

###### T1

```text
In templates/partials/sidebar.html the current-page mark comes from the carrier and nowhere else: every aria-current and every sidebar__item--active occurrence sits inside an {% if nav.is_... %} condition over NavContext.active, none is written literally, and no two members share one condition. The shipped rail-member law keeps holding over every member, conditionally rendered ones included.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo xtask check-arch"
```

##### Contract tests

None.

## ADRs

- NAV\-POLISH\-001 Nav marks its own page and offers only what opens — Superseded by NAV-POLISH-003
- NAV\-POLISH\-002 Rail text\-plus\-glyph controls owe text contrast\, not a boundary — Superseded by NAV-POLISH-006
- NAV\-POLISH\-003 Nav marks an offered member and the rail keeps its flyout — Superseded by NAV-POLISH-004
- NAV\-POLISH\-004 The rail carries its own paint level above page content — Accepted
- NAV\-POLISH\-005 Unobservable rail\-identity failure is closed by model and unit test — Accepted
- NAV\-POLISH\-006 Rail controls are exempt from the boundary rule in every layout — Superseded by NAV-POLISH-007
- NAV\-POLISH\-007 The boundary floor is an outside\-rail reading\, not a glyph count — Accepted
