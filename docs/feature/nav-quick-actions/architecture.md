# Architecture brief: nav\-quick\-actions

## Paradigm

procedural

## Shared records

### Decisions

None.

### Constraints

#### K1

```text
User decision 2026-11-19: remove Home's Quick actions section once its links are in the nav.
```
Governed slices: S1

#### K2

```text
User decision 2026-11-19: Teams, Invite a member and Machine tokens go in the top part of the nav, after Board; Settings, Keyboard shortcuts and Sign out stay in the bottom group.
```
Governed slices: S1

#### K3

```text
User decision 2026-11-19: each moved link is visible to exactly whoever sees it in Quick actions today; no rights change.
```
Governed slices: S1

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

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Extend

```text
Three anchors after Board inside nav#sidebar-nav, each with one inline Lucide svg.sidebar__icon (users, user-plus, key-round) and one span.sidebar__label reading Teams, Invite a member, Machine tokens; no condition, no aria-label, no title element. The partial comment keeps its rule and gains the corrected member count.
```

crates\/foundry\-app\/templates\/dashboard\_root\.html

Extend

```text
Delete the Quick actions section (heading plus ul.actions). Leave the greeting, the workspace line and the Your projects section untouched.
```

crates\/foundry\-app\/src\/views\.rs

Extend

```text
Remove the now readerless DashboardRoot fields is_instance_admin, is_workspace_admin and csrf with their doc comments.
```

crates\/foundry\-app\/src\/signin\.rs

Extend

```text
Drop the same three initialisers from the DashboardRoot literal in the handler and from the in-crate test helper. Keep both admin lookups and ensure_csrf_cookie: NavContext::for_page consumes them.
```

crates\/foundry\-app\/src\/nav\.rs

Extend

```text
Correct the NavSection doc comment: the rail has five primary destinations, of which two are marked sections.
```

crates\/foundry\-app\/static\/VENDOR\.md

Extend

```text
Add users, user-plus and key-round to the icons-used list with their rail members and move the verification instruction from seven to ten.
```

xtask\/src\/check\_arch\.rs

Extend

```text
Add the rail-member layer that enumerates the members of templates/partials/sidebar.html and refuses one without exactly one svg.sidebar__icon and one span.sidebar__label, or with an aria-label, naming the offending member and the fix.
```

crates\/foundry\-acceptance\/tests\/features\/nav\-quick\-actions\.feature

#### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-acceptance --test acceptance
scripts/test-in-lima.sh cargo xtask check-arch
scripts/test-in-lima.sh cargo test -p foundry-app
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=nav-collapse-icon-rail FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo test -p foundry-acceptance --test acceptance
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_quick\_actions\.rs

crates\/foundry\-acceptance\/src\/lib\.rs

crates\/foundry\-acceptance\/tests\/features\/navigation\-bar\-linear\-ui\.feature

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs

#### Public oracle

```text
Signed in as each of the four rights rows, GET /team/general/project/sandbox and GET / over real HTTP against the running binary; then, in the browser lane, narrow the left nav to its icon rail on the dashboard at desk width and at 320, 390 and 480 px.
```

```text
Each board page nav offers Home, Board, Teams, Invite a member and Machine tokens in that order, linking to /, the first-project board, /teams, /workspace/invites and /admin/tokens, each with one inline icon and its own word, with Home the only member marked current; Integrations and Instance admin appear only for the rights that already gated them. Home carries no Quick actions heading and no ul.actions, and still greets the operator and lists her projects. In the folded rail every member, the three new ones included, is in the layout, visible, focusable and named by its own word, and no page slides sideways.
```

```text
A plain member loses any of the three (a condition was authored where none existed); any of the three is placed before Board or outside nav#sidebar-nav; an anchor renders a glyph with no span.sidebar__label, or an aria-label beside it; Home still shows Quick actions, or lost its greeting or project list; two primary members are marked current; the folded rail leaves a new member out of the layout or unnamed; the 320 px page slides sideways.
```

##### Promises

P1

P2

P3

P4

#### Prefactoring

None.

#### Records

##### Decisions

```text
CONFIRMED, all three FIXED owner decisions unchanged: Teams, Invite a member and Machine tokens become three unconditional anchors inside nav#sidebar-nav.sidebar__nav, placed after the Board anchor; Home loses its whole Quick actions section; no visibility gate is added or removed. templates/partials/sidebar.html stays the single owner of the rail member list and NavContext (crates/foundry-app/src/nav.rs) gains no field, because each of the three destinations is a fixed href with no per-identity decision and no active-section marking to carry.
```

```text
No model for the nav membership, visibility and Home-removal claims: there is no shared state, no multi-step protocol, no concurrency and no crash window - each claim is one synchronous render of one template from one already-resolved NavContext, and the rights domain is the closed four-member set (plain member, workspace admin, instance admin, both) that the shipped icon-rail outline already enumerates. The claims are therefore finite and exhausted by enumerated cases, not by a state model. Quint is installed on this host, so this is a reasoned modelling choice and not a tooling gap; Agda is absent and no claim here needs a proof over types.
```

```text
The three new items carry no aria-current and NavSection keeps its two variants (Home, Board). Home therefore stays the single marked section on /teams, /workspace/invites and /admin/tokens exactly as it already is on /account/settings and /keyboard-help, so "exactly one primary navigation item is marked as the current page" (navigation-bar-linear-ui, notification-preferences-ui) keeps holding with five primary members instead of two.
```

```text
The stylesheet is not touched. The three items reuse .sidebar__item, .sidebar__icon and .sidebar__label, and both the desk collapsed rules and the max-width 480px block already make .sidebar__nav and .sidebar__user wrapping rows bounded by the bar (flex-wrap: wrap with min-width: 0), which is quantified over members and not over a member count. foundry.fb3aa227.css is content-hash named, so any byte change migrates templates/base.html, static/VENDOR.md and three hashed-name assertions in crates/foundry-app/src/lib.rs; spending that for rules that need no change is refused. The .actions rules become unreferenced and stay in place until the next stylesheet change re-fingerprints the file.
```

```text
Subtraction the removal makes available: DashboardRoot drops is_instance_admin, is_workspace_admin and csrf. Once the Quick actions list is gone those three fields have no reader, and the same three facts already reach the page through the nav field. The handler keeps computing all three, unchanged and fail-closed, because NavContext::for_page consumes them.
```

```text
Glyphs come from the release static/VENDOR.md already pins, lucide-static 1.75.0 (ISC): users for Teams, user-plus for Invite a member, key-round for Machine tokens. The shape children of package/icons/<name>.svg are copied verbatim in upstream order; the svg wrapper is authored like the seven shipped ones (class sidebar__icon, aria-hidden true, focusable false, no title element, stroke currentColor, no fill or colour literal). VENDOR.md gains the three names and its "all seven" count becomes ten.
```

```text
Placement inside nav#sidebar-nav is load-bearing, not cosmetic: the collapse control reaches the nav by aria-controls="sidebar-nav", the collapsed rules select [data-nav-collapsed] .sidebar__nav, and the browser rail reading enumerates ".sidebar a.sidebar__item" plus the sign-out button. An item added inside that element is therefore folded, clipped, revealed and read with no change to static/js/nav-collapse.js and no change to crates/foundry-acceptance/src/support/rail_reading.rs.
```

```text
P4 is inherited, not re-oracled. The shipped nav-collapse-icon-rail scenarios quantify over the members they read from the live document, so the enlarged set is covered by RE-RUNNING that lane rather than by a new browser scenario. The slice therefore takes its RED from fast served-markup scenarios, and the close must name the browser lane that actually ran: on a host without Docker that lane observes nothing and P4 stays unverified behind a green gate.
```

##### Constraints

```text
No stored or validated input is added: the three destinations are fixed href literals /teams, /workspace/invites and /admin/tokens with no path or query parameter, so no length, range or character bound applies to this change and none is invented. The only authority bound carried is P4 width bound, 480 px and below.
```

```text
Visibility bound, read from the authority code and not inferred: the three moved links carry NO condition in dashboard_root.html today (lines 35-37 are plain list items), so every authenticated identity sees them, and the nav items carry no condition either. Integrations keeps nav.is_workspace_admin and Instance admin keeps nav.is_instance_admin, both already in the nav and both untouched. Signed out there is no nav at all: / and the three destinations redirect to /sign-in.
```

```text
No route, handler or authorization changes. /teams, /workspace/invites and /admin/tokens keep their own server-side gates, which this change neither reads, copies nor weakens: the nav offers a link, the destination still decides. No new destination page is added.
```

##### Reuse

sidebar\_\_nav

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:36

Extend

```text
The single owner of the rail member list, already carrying the glyph-plus-label rule for every destination. Three anchors are appended after the Board anchor inside nav#sidebar-nav; no second nav list is created anywhere.
```

dash\_\_section

crates\/foundry\-app\/templates\/dashboard\_root\.html\:32

Extend

```text
The section the owner decided to remove is the second dash__section (heading plus ul.actions). Only it is deleted; the greeting, the workspace line and the Your projects section with its four states stay byte-unchanged.
```

DashboardRoot

crates\/foundry\-app\/src\/views\.rs\:1188

Extend

```text
Three fields lose their only reader with the section. Dropping them keeps one carrier per fact on the page: nav already holds is_instance_admin, is_workspace_admin and csrf for the sidebar.
```

DashboardRoot

crates\/foundry\-app\/src\/signin\.rs\:491

Extend

```text
The one construction site of DashboardRoot (plus its in-crate test helper at signin.rs:750) drops the same three initialisers. The three lookups above it stay: NavContext::for_page consumes them, so no query and no fail-closed default is removed.
```

NavSection

crates\/foundry\-app\/src\/nav\.rs\:16

Extend

```text
The active-section carrier is REUSED unchanged, two variants, because the three new destinations mark no section. The one line that changes is its doc comment claim that the rail has exactly two primary destinations, which this change makes false.
```

lucide\-static

crates\/foundry\-app\/static\/VENDOR\.md\:92

Extend

```text
The icons-used list and the all-seven verification instruction are the provenance record for inline Lucide path data. Three names join the list and the count becomes ten; the pinned release, the tarball hash and the copy procedure stay unchanged.
```

check\_arch

xtask\/src\/check\_arch\.rs\:176

Extend

```text
The repository architecture gate already reads templates (base.html, api_docs.html) and already hosts a nav-collapse layer, so the rail-member law gets a free function beside the existing layers rather than a new tool, a new gate command or a new crate.
```

##### Impact

template

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:36

Changed

```text
Three anchors added after Board, each with its inline Lucide glyph and its span.sidebar__label; the partial comment block that tells the next author to give a new destination both gains the corrected member count.
```

template

crates\/foundry\-app\/templates\/dashboard\_root\.html\:32

Changed

```text
The Quick actions section is deleted. Its three moved links move to the nav; Integrations, Keyboard shortcuts, Instance admin and Sign out were already in the nav with identical gates, so nothing becomes unreachable.
```

view\-struct

crates\/foundry\-app\/src\/views\.rs\:1188

Changed

```text
is_instance_admin, is_workspace_admin and csrf removed with their doc comments, which all describe quick-action links that no longer exist.
```

handler

crates\/foundry\-app\/src\/signin\.rs\:491

Changed

```text
Both DashboardRoot literals drop the same three fields. Askama fails to compile if any template still reads them, so the removal cannot be partial.
```

module\-doc

crates\/foundry\-app\/src\/nav\.rs\:16

Changed

```text
The NavSection doc comment states the rail has exactly two primary destinations. It becomes five, and the comment is the only line that changes in this file.
```

provenance\-doc

crates\/foundry\-app\/static\/VENDOR\.md\:92

Changed

```text
Three icon names added to the icons-used list and the verification instruction count moved from seven to ten, so the auditor procedure still names every inline glyph.
```

architecture\-gate

xtask\/src\/check\_arch\.rs\:176

Changed

```text
A new layer falsifies the rail-member law over whatever members sidebar.html holds, including members added after this change.
```

stylesheet

crates\/foundry\-app\/static\/css\/foundry\.fb3aa227\.css\:1488

UnchangedCompatible

```text
Every rule the new items need already exists and is quantified over members, not over a count: .sidebar__item, .sidebar__icon, .sidebar__label at desk width, the clip-and-reveal rules under [data-nav-collapsed], and the wrapping rows inside the bar below 480px. Changing a byte would rename the hashed file and migrate base.html, VENDOR.md and three cache-policy assertions for no behavioural gain.
```

client\-script

crates\/foundry\-app\/static\/js\/nav\-collapse\.js\:62

UnchangedCompatible

```text
It owns the id on nav.sidebar__nav and the pre-paint mark only; it enumerates no member and injects no glyph, so three more server-rendered anchors inside that element reach it with no edit.
```

acceptance\-support

crates\/foundry\-acceptance\/src\/support\/rail\_reading\.rs\:74

UnchangedCompatible

```text
It reads members from the live document by selector (".sidebar a.sidebar__item" plus the sign-out button) and takes each name from the member own .sidebar__label, so the enlarged set is read with no change.
```

acceptance\-support

crates\/foundry\-acceptance\/src\/support\/team\_membership\.rs\:1641

UnchangedCompatible

```text
when_follows_teams opens / and resolves link_to(dash, "Teams") over every a[href] in the body. The sidebar anchor on that same page answers with text "Teams" and href /teams, so the step keeps resolving after the Quick actions link is gone.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/navigation\-bar\-linear\-ui\.feature\:194

Migrated

```text
The Slice-09 scoping guard asserts the sidebar does NOT contain an Invite a member item and does NOT contain a Machine tokens item, which is the exact inverse of P1. The acceptance designer retires or inverts it before craft; leaving it is a red suite, and silently deleting it without recording the superseded scope is a lost guard.
```

acceptance\-oracle

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:790

Migrated

```text
Six dark-palette assertions resolve colours on ".actions a" and ".actions button", selectors that exist ONLY in Home Quick actions list. With the section gone the dashboard palette scenario reads nothing there, so those rows go and the scenario clause that promises "the action controls" goes with them.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature\:203

Migrated

```text
The Then clause names the action controls among the surfaces measured in the dark palette. The project cards and the section labels stay; the action controls clause is removed with its step rows.
```

##### Agreement

rendered sidebar member markup\: inside nav\#sidebar\-nav each member is one a\.sidebar\_\_item carrying exactly one inline svg\.sidebar\_\_icon and exactly one span\.sidebar\_\_label holding its whole word\, with no aria\-label

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:36

Producer

Migrated

```text
Gains three members that satisfy the same shape; it is the sole producer of the markup.
```

rendered sidebar member markup\: inside nav\#sidebar\-nav each member is one a\.sidebar\_\_item carrying exactly one inline svg\.sidebar\_\_icon and exactly one span\.sidebar\_\_label holding its whole word\, with no aria\-label

crates\/foundry\-app\/static\/css\/foundry\.fb3aa227\.css\:1196

Consumer

UnchangedCompatible

```text
Selects on the classes, never on a position or a count, at both geometries and both breakpoints.
```

rendered sidebar member markup\: inside nav\#sidebar\-nav each member is one a\.sidebar\_\_item carrying exactly one inline svg\.sidebar\_\_icon and exactly one span\.sidebar\_\_label holding its whole word\, with no aria\-label

crates\/foundry\-app\/static\/js\/nav\-collapse\.js\:62

Consumer

UnchangedCompatible

```text
Needs only the id on the nav element and the brand mount; it enumerates no member.
```

rendered sidebar member markup\: inside nav\#sidebar\-nav each member is one a\.sidebar\_\_item carrying exactly one inline svg\.sidebar\_\_icon and exactly one span\.sidebar\_\_label holding its whole word\, with no aria\-label

crates\/foundry\-acceptance\/src\/support\/rail\_reading\.rs\:74

Consumer

UnchangedCompatible

```text
Enumerates members by selector and reads each name from its own label, so its laws quantify over the enlarged set automatically.
```

rendered sidebar member markup\: inside nav\#sidebar\-nav each member is one a\.sidebar\_\_item carrying exactly one inline svg\.sidebar\_\_icon and exactly one span\.sidebar\_\_label holding its whole word\, with no aria\-label

crates\/foundry\-acceptance\/src\/steps\/feature\_navigation\_bar\.rs\:222

Consumer

UnchangedCompatible

```text
primary_items reads label, aria-current and href per item; "shows primary items Home and Board" is a containment check and the single-current law stays true because the new members mark nothing.
```

Home Quick actions markup\: section\.dash\_\_section holding the heading \"Quick actions\" and ul\.actions with one link per action plus the sign\-out form

crates\/foundry\-app\/templates\/dashboard\_root\.html\:32

Producer

Migrated

```text
The producer is deleted, so the contract ends; every link it carried already exists in the nav with the same gate.
```

Home Quick actions markup\: section\.dash\_\_section holding the heading \"Quick actions\" and ul\.actions with one link per action plus the sign\-out form

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:790

Consumer

Migrated

```text
Its six .actions colour rows have no element left to resolve; they are removed with the scenario clause that names them.
```

Home Quick actions markup\: section\.dash\_\_section holding the heading \"Quick actions\" and ul\.actions with one link per action plus the sign\-out form

crates\/foundry\-acceptance\/tests\/features\/navigation\-bar\-linear\-ui\.feature\:194

Consumer

Incompatible

```text
It asserts the two links STAY out of the sidebar, which P1 reverses. It cannot stay as written and is the one oracle this change contradicts rather than extends.
```

Home Quick actions markup\: section\.dash\_\_section holding the heading \"Quick actions\" and ul\.actions with one link per action plus the sign\-out form

crates\/foundry\-app\/static\/css\/foundry\.fb3aa227\.css\:793

Consumer

UnchangedCompatible

```text
The .actions rules simply stop matching. Unreferenced rules in a hash-named stylesheet change no behaviour, and removing them would rename the file.
```

DashboardRoot template fields consumed by dashboard\_root\.html

crates\/foundry\-app\/src\/views\.rs\:1188

Producer

Migrated

```text
Three fields removed; Askama binds template reads to struct fields at compile time, so no consumer can keep reading them.
```

DashboardRoot template fields consumed by dashboard\_root\.html

crates\/foundry\-app\/src\/signin\.rs\:750

Consumer

Migrated

```text
The in-crate test helper constructs the struct and drops the same three initialisers; it keeps the nav field it already builds.
```

##### Not applicable

contract_tests

```text
No party can change apart from another. The template, the handler, the view struct, the stylesheet and nav-collapse.js ship inside one binary and one commit, and the change persists nothing and crosses no wire: the only durable artefact involved is the per-browser fold choice, which this change does not touch. The slice primary oracle covers every caller in the deployable.
```

prefactoring

```text
No preserving move is available or needed. The nav member list is already isolated in one partial and the stylesheet already carries every rule the new members use, so nothing has to be moved to make the change easy. The one restructuring the change does perform, dropping the three readerless DashboardRoot fields, is NOT a preserving move: those fields lose their reader only because the Quick actions section goes, so it is part of the behaviour change and would have no GREEN oracle to observe before it.
```

##### Boundaries

```text
HTTP GET on any authenticated page of the running foundry binary (/, /team/{team}/project/{project}, /teams, /account/settings), served with the real session cookie; the same origin driven in a real browser for the folded-rail geometry
```

###### Driven ports

```text
Postgres through Store::is_instance_admin, Store::is_workspace_admin and nav_identity (read-only, fail-closed, unchanged)
```

```text
the browser document and its accessibility tree, read once per scenario by the shipped rail reading
```

```text
Template reads NavContext; NavContext is a presentation projection assembled by the handler from already-resolved session identity. Nothing in the template reaches the store, and no store or domain module learns about a nav member.
```

###### Failures

####### B1

```text
Store::is_workspace_admin or Store::is_instance_admin errors while assembling the nav.
```

Refusal

```text
The gated member is absent from the nav (fail-closed, unchanged), while Home, Board and the three moved members are all offered.
```

####### B2

```text
The browser runs no page scripts, so no fold mark and no collapse control exist.
```

Refusal

```text
The folded geometry is unavailable and the open nav offers all five primary members with their words, as the shipped degraded-arm scenarios already assert.
```

##### Observations

```text
The ordered list of (word, href) the primary nav offers on a served page, read from nav#sidebar-nav
```

ordered list of label and href pairs

```text
which template branch produced a member, the glyph geometry, the stylesheet that draws it, and whether the destination will authorize the operator once she arrives
```

```text
Whether a served Home page carries a Quick actions heading or a ul.actions list, and whether it still carries the greeting, the workspace line and the Your projects section
```

four booleans per page

```text
how the section was removed, and whether unreferenced .actions rules remain in the stylesheet
```

```text
Per member of the folded rail: in the layout, visible, focusable, and the name the browser computes for it
```

list of member readings

```text
what an assistive technology actually announces, and whether a long word stays legible in its revealed box
```

```text
Whether the document slides sideways at a given viewport width with the nav folded
```

boolean

```text
how many rows the bar took and how much vertical space that left the content
```

##### Equality

the nav observations

```text
Two renders of the nav are equal when they offer the same ordered list of (word, href) and mark the same member as the current page. Whitespace, attribute order, glyph path data, the stylesheet hash and the identity strings in the brand and footer are not distinguished.
```

##### Constructors

sidebar\-member

word plus href plus inline Lucide shape children \-\> one a\.sidebar\_\_item inside nav\#sidebar\-nav\, authored in templates\/partials\/sidebar\.html

NavContext\:\:for\_page

workspace\_name\, display\_name\, is\_instance\_admin\, is\_workspace\_admin\, csrf\, NavSection\, board\_href \-\> NavContext \(crates\/foundry\-app\/src\/nav\.rs\, unchanged\)

##### Laws

###### L1

```text
On every authenticated page the primary nav offers exactly Home, Board, Teams, Invite a member and Machine tokens, in that order, linking to /, nav.board_href, /teams, /workspace/invites and /admin/tokens.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/partials\/sidebar\.html

###### Treatment

Finite

###### Cases

```text
a plain member on a board page
```

```text
a workspace admin on a board page
```

```text
an instance admin on a board page
```

```text
a member on / with no projects, where Board falls back to /
```

```text
The rights domain is the closed four-member set and the page families are / and the board route; the order is a single authored sequence, so enumerated cases exhaust it and a generator would only repeat them.
```

###### Evidence

None.

###### L2

```text
Each of Teams, Invite a member and Machine tokens is offered to exactly the identities that saw it in Home Quick actions, that is every authenticated identity, while Integrations stays workspace-admin only and Instance admin stays instance-admin only.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/partials\/sidebar\.html

###### Treatment

Finite

###### Cases

```text
a plain member sees the three and neither gated item
```

```text
a workspace admin sees the three and Integrations
```

```text
an instance admin sees the three and Instance admin
```

```text
a workspace-and-instance admin sees all five
```

```text
Visibility is a function of the two boolean rights facts only, so four cases are the whole domain; the moved three carry no condition at all, in either place.
```

###### Evidence

None.

###### L3

```text
A served Home page carries no Quick actions heading and no ul.actions, and still carries the greeting, the workspace line and the Your projects section in each of its states.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/dashboard\_root\.html

###### Treatment

Finite

###### Cases

```text
Listed, with at least one project card
```

```text
CreateOn, the create-first-project empty state
```

```text
CreateUnlinked
```

```text
NoTeam
```

```text
DashboardProjectsSection is a closed four-variant enum and the removal is one section, so the four variants are the whole domain.
```

###### Evidence

None.

###### L4

```text
Every member of the shipped sidebar partial carries exactly one inline svg.sidebar__icon and exactly one span.sidebar__label holding its whole word, and no member carries an aria-label.
```

Invariant

Adapter

xtask\/src\/check\_arch\.rs

###### Treatment

Finite

###### Cases

```text
every a.sidebar__item and the sign-out button enumerated from templates/partials/sidebar.html, whatever their number
```

```text
The population is the file own member list, which the check enumerates rather than assumes, so the law is exhaustive over present and future members without a generator.
```

###### Evidence

None.

###### L5

```text
With the nav folded, every member including the three new ones is in the layout, visible, focusable and named by its own word, and the document does not slide sideways at phone width.
```

Invariant

Adapter

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature

###### Treatment

Finite

###### Cases

```text
the four rights rows at desk width, folded
```

```text
320 px folded
```

```text
390 px folded
```

```text
480 px folded
```

```text
Inherited from the shipped lane, which quantifies over the members it reads from the live document. P4 promises every width at or below 480 px while the lane drives the boundary and two widths under it; the stylesheet declares no further breakpoint below 480 px, so the untested widths are recorded as residue rather than claimed.
```

###### Evidence

None.

###### L6

```text
Exactly one primary nav item carries aria-current="page" on every authenticated page; none of the three new members ever carries it.
```

Invariant

Application

crates\/foundry\-app\/src\/nav\.rs

###### Treatment

Finite

###### Cases

```text
Home marked on /
```

```text
Board marked on a board page
```

```text
Home marked on /teams
```

```text
Home marked on /account/settings
```

```text
NavSection is a closed two-variant enum chosen once per page, so the marked member is a function with two values and the page families enumerate it.
```

###### Evidence

None.

##### Refuted

```text
Removing Home Quick actions section touches no oracle outside Home own scenarios.
```

```text
ul.actions is a shared markup contract, not a Home-local section: another feature measures its palette through it, and an older guard asserts the inverse of P1.
```

```text
crates/foundry-acceptance/src/steps/feature_canzan_theme.rs:790-795 resolves six dark-palette values on ".actions a" and ".actions button"; crates/foundry-acceptance/tests/features/navigation-bar-linear-ui.feature:194 asserts the sidebar does NOT contain an Invite a member item or a Machine tokens item.
```

```text
Adding rail members needs a new stylesheet rule to keep the phone bar inside the screen.
```

```text
The wrap rule is quantified over members, not over a member count, so the member total is not a parameter of the layout.
```

```text
[data-nav-collapsed] .sidebar__nav and .sidebar__user already declare flex-wrap: wrap with min-width: 0 and max-width: 100% inside the max-width 480px block, so ten members take rows inside the same bar exactly as seven do.
```

```text
Invite a member is the longest word the rail has to draw, so the revealed label box needs a new bound.
```

```text
The worst case was already shipped; the new words are shorter than it.
```

```text
Keyboard shortcuts, eighteen characters, is already a rail member; Invite a member is fifteen.
```

##### Claims

###### C1

```text
A reintroduced quick-action link cannot silently read a rights fact from the dashboard view struct.
```

```text
A second carrier of the same rights fact on one page drifts from the nav and shows an affordance the nav denies.
```

crates\/foundry\-app\/src\/views\.rs

```text
the resolved session identity the handler already holds
```

crates\/foundry\-app\/src\/views\.rs DashboardRoot literal

crates\/foundry\-app\/templates\/dashboard\_root\.html

```text
the served Home page offers no gated link of its own
```

```text
none through the type system: Askama binds template field reads at compile time, so a template reading a removed field fails the build. The remaining bypass is adding the field back, which is a visible diff, and the nav field itself, which is the intended single carrier.
```

###### C2

```text
A nav member cannot reach an operator as a glyph with no word, or with a second name channel beside its word.
```

```text
In the folded rail a member with no word is a silent icon for a screen-reader operator and a blank sliver for a sighted one; a second name channel drifts from the text the oracle reads.
```

xtask\/src\/check\_arch\.rs

```text
the bytes of templates/partials/sidebar.html
```

the sidebar\-member markup authored in that partial

the served page\, the stylesheet clip\-and\-reveal rules and the rail reading

```text
the browser-computed name of every folded rail member is non-empty and equals its own word
```

```text
Rust types exclude nothing here: the template is text and a missing span compiles. The exclusion is a build-time gate plus the browser lane, so the open bypasses are a commit that skips cargo xtask smoke, an htmx fragment response that does not include the partial, and a host with no Docker where the browser lane observes nothing.
```

##### States

Admitted

```text
The four rights rows (plain member, workspace admin, instance admin, both) crossed with the open nav and the folded rail, at desk width and at 320, 390 and 480 px
```

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Admitted

```text
The four Home project-section states, each with no Quick actions section
```

crates\/foundry\-app\/templates\/dashboard\_root\.html

Admitted

```text
Signed out: every one of the five nav destinations redirects to /sign-in and no nav is rendered
```

crates\/foundry\-app\/src\/signin\.rs

Excluded

```text
A nav member with a glyph and no word, or with an aria-label beside its word
```

xtask\/src\/check\_arch\.rs

Excluded

```text
A rights condition on Teams, Invite a member or Machine tokens in either place
```

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Excluded

```text
A second nav member list in any other template, or a Quick actions section on Home
```

crates\/foundry\-app\/templates\/dashboard\_root\.html

Unknown

```text
What an assistive technology actually announces for the three new icons (inherited limit of the icon-rail lane, which reads one engine accessibility tree)
```

product

Unknown

```text
Desk viewports shorter than the ten-member rail, and phone widths below 320 px
```

architect

##### Models

None.

##### Closure

```text
Teams, Invite a member and Machine tokens as nav destinations
```

Safety

Pass

DesignedNotBuilt

```text
The excluded states are enumerated over the whole rights domain: the three items are authored with no condition, so there is no guard to widen or narrow, and the gated members keep their own conditions. The glyph-and-word exclusion is falsified by the new check-arch layer over the file own member list. Nothing is built yet.
```

###### Evidence

None.

```text
Teams, Invite a member and Machine tokens as nav destinations
```

Inhabitation

Pass

DesignedNotBuilt

```text
Each member is constructed from admitted input already present on every authenticated page: NavContext::for_page is unchanged and the three hrefs are literals, so a valid nav holding all five primary members exists for every one of the four rights rows with no new input and no new lookup.
```

###### Evidence

None.

```text
Teams, Invite a member and Machine tokens as nav destinations
```

PublicReachability

Pass

DesignedNotBuilt

```text
session identity -> signin.rs:491 dashboard handler and the per-page handlers -> NavContext::for_page (crates/foundry-app/src/nav.rs) -> templates/partials/sidebar.html via app_shell.html -> HTTP GET on / and on /team/{team}/project/{project} -> the anchor the operator presses -> GET /teams, /workspace/invites, /admin/tokens. Every symbol on that path exists today; only the three anchors are new.
```

###### Evidence

None.

```text
Teams, Invite a member and Machine tokens as nav destinations
```

Liveness

Pass

DesignedNotBuilt

```text
No liveness claim: one synchronous response renders the whole nav, with no queue, retry, background step or eventual state to reach. The fold geometry is a pre-paint client mark with no progress obligation of its own.
```

###### Evidence

None.

##### Risks

###### Y1

Stressor

```text
Negate "the stylesheet keeps every member inside the bar at 480 px and below": ten members instead of seven on a 320 px phone.
```

```text
Every destination stays inside the top bar and the page does not slide sideways; both groups are wrapping rows bounded by the bar.
```

```text
The bar takes more rows and the document scrolls vertically.
```

```text
A destination dropped below the breakpoint, or any sideways slide.
```

```text
Widths below 320 px are driven by no lane, and how much vertical space a three-row bar leaves the content is unobserved.
```

architect

###### Y2

Stressor

```text
Negate "the desk rail is shorter than the viewport": ten members plus brand, identity, theme control and collapse control on a short desk window.
```

```text
Every member stays reachable: .app-shell is min-height 100vh with no overflow clip, so the document grows and scrolls vertically instead of clipping the rail.
```

```text
The operator scrolls the page to reach the bottom group.
```

```text
A member clipped with no way to reach it, or a horizontal scrollbar at desk width.
```

```text
.sidebar declares no overflow-y of its own and no lane drives a short desk viewport; adding one would re-fingerprint the stylesheet.
```

architect

###### Y3

Stressor

```text
Negate "the pinned lucide-static 1.75.0 tarball is reachable during craft".
```

```text
No hand-drawn glyph enters the template and VENDOR.md keeps claiming only what was copied.
```

```text
Craft stops on the three glyphs and reports the missing prerequisite.
```

```text
Invented or recalled path data recorded as Lucide provenance.
```

```text
Icon provenance stays INDETERMINATE until the tarball is extracted and the shape children compared, per the procedure VENDOR.md already states.
```

crafter

###### Y4

Stressor

```text
Negate "the browser lane runs": cargo xtask ci on a host with no Docker.
```

```text
The served-markup scenarios still fail on a missing or misplaced member, so P1, P2 and P3 keep a falsifier in the fast lane.
```

```text
The folded-rail and phone observations stay unobserved for the enlarged set.
```

```text
Close evidence that names a green gate without naming the lane that actually ran.
```

```text
P4 runtime stays INDETERMINATE until the icon-rail lane is reported with Docker included.
```

host

###### Y5

SinglePointOfFailure

```text
templates/partials/sidebar.html is the sole owner of the member list for every authenticated page.
```

```text
One edit changes the nav everywhere, and no page can disagree with another about what the nav offers.
```

```text
Nothing: the partial is included by app_shell.html for every full page.
```

```text
A second nav member list authored in another template.
```

```text
A future page that renders its own nav would diverge silently; the check-arch layer reads this one file only.
```

architect

###### Y6

Bypass

```text
An htmx fragment response re-renders part of a page without including partials/sidebar.html.
```

```text
The nav laws are asserted on full-page responses, which is where the nav is produced.
```

```text
A fragment carries no nav and is not measured for one.
```

```text
Treating a fragment response as evidence that the nav law holds.
```

```text
The membership law is unobservable on fragment responses by construction, not by omission.
```

architect

##### Architectural tests

###### T1

```text
Every member of templates/partials/sidebar.html carries exactly one inline svg.sidebar__icon and exactly one span.sidebar__label holding its whole word, and no member carries an aria-label. The layer ENUMERATES the members from the file rather than counting an expected number, so it governs members added after this change.
```

xtask\/src\/check\_arch\.rs

```text
cargo xtask check-arch
```

##### Contract tests

None.

## ADRs

- NQA\-001 Nav owns the three moved destinations — Accepted
