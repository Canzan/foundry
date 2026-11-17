# Architecture brief: nav\-collapse\-icon\-rail

## Paradigm

procedural

## Shared records

### Decisions

#### D1

```text
CONFIRMED: every destination the expanded nav shows the operator becomes an icon in the collapsed rail - Home, Board, Settings, Keyboard shortcuts, Integrations when she is a workspace admin, Instance admin when she is an instance admin - and Sign out. The rail renders the same destination set the expanded nav renders, from the same Askama nav view model; this feature adds no second source of destinations.
```

#### D2

```text
CONFIRMED with a precision: the icons are inline SVG in templates/partials/sidebar.html, with path data copied from the Lucide icon set (ISC). They carry stroke=currentColor and no colour literal, so each icon inherits the rail text token in both palettes and the token-seam gate stays silent. PROVENANCE SHAPE: an inline path in a template is not a served blob, so it gets no hashed VENDOR.md row, which would be a sha256 claim about a file that does not exist; it gets a VENDOR.md note naming the Lucide release tag the paths were copied from, the icon names, and the ISC notice. The crafter records the exact release tag it copied from; this design does not invent one.
```

#### D3

```text
CONFIRMED: each rail item keeps the accessible name its text link has today. MECHANISM: the destination word moves into a span.sidebar__label that stays in the document in both geometries; in the collapsed geometry it is clipped to one pixel rather than given display:none, so it stays in the accessibility tree and the accessible name is still computed from the item OWN TEXT. No aria-label is added anywhere: a second name channel beside the text is a drift risk, and the text node is the one owner of the name.
```

#### D4

```text
DECISION: the pointer operator reads the destination name from that same label span, revealed as a flyout on :hover and on :focus-visible in the collapsed geometry only. No title attribute is added to any rail item. A title would fire in the EXPANDED nav too, which the human fixed as unchanged, and an SVG title element would add a second text node to the item and make its text content read Home Home, which the navigation-bar oracle reads. The flyout serves the pointer and the keyboard from one declaration.
```

#### D5

```text
DECISION: at 480 px and below the flyout is placed under the icon inside the bar, not to its right, and is bounded by the bar. The collapsed bar there is a wrapping row that reaches the full width, so a right-hand flyout on the last icon of a row would reach past the right edge; the phone promise forbids the document widening.
```

#### D6

```text
CONFIRMED: Sign out in the rail stays one press, with no confirmation step, exactly as in the expanded nav. The rail renders the SAME post form that is there today; only its button gains an icon and a label span.
```

#### D7

```text
CONFIRMED: the collapsed rail marks the page the operator is on with the same channel the expanded nav uses - the server-rendered aria-current=page attribute and the sidebar__item--active class. The collapsed geometry adds no second marking channel; it only keeps the active tint legible on an icon-sized target.
```

#### D8

```text
CONFIRMED: this feature extends the walking skeleton the delivered nav-collapse-toggle feature established - the collapse control, the stored choice and the pre-paint stamp, end to end - and adds no second skeleton. Neither slice introduces a new end-to-end path; S1 extends an existing gate layer and an existing oracle arm, S2 extends the rail the skeleton already renders.
```

#### D9

```text
CONFIRMED: signing out does not clear the stored collapse choice. This needs no code: the rail Sign out is a post navigation and nothing in it touches the foundry.nav key, so the choice survives as the theme choice does. The accepted per-browser consequence is that another person signing in on the same browser inherits the collapsed rail, exactly as she inherits the theme.
```

#### D10

```text
CONFIRMED: for a head script that does not stamp stored state, DEFERS means it carries the defer attribute. async and type=module in the head are refused for it too and the contributor is told to add defer. This is the only total reading of the confirmed finding; it changes no verdict for any script in the shipped head, where every neighbour already carries defer; and a contributor who later wants an async or module head script changes this decision deliberately instead of meeting a silent gap.
```

#### D11

```text
CONFIRMED and NOT repaired here: the nav shows the rights of the last page load in both geometries, and activating a destination whose right was revoked is refused by the app door that owns it. The condition is pre-existing, it is not caused by collapsing, and this feature neither repairs nor worsens it. Recorded as an admitted state with its risk owner.
```

#### D12

```text
CONFIRMED: scenario exploration was relevant to both values and its two models are adopted as this design models - the guards value because its behaviour turns on execution order around the first paint and on which head scripts count as pre-paint, the icon-rail value because its sad path is a trapping state over the collapsed, scripting and permission combinations. Second-tab sync is out of scope, so no race is modelled.
```

#### D13

```text
DECISION: the collapsed geometry stops hiding the destinations. Today the stylesheet gives display:none to sidebar__nav, sidebar__workspace and every sidebar__user child but the theme control. From this feature only sidebar__workspace and sidebar__identity keep display:none; sidebar__nav, the user-section links and the Sign out form stay in the layout as icons. The workspace name and the display name are the two members the human kept out of the strip, and they are the two that carry no destination.
```

#### D14

```text
DECISION (mine, with its reason; NOT a user confirmation, and it narrows nothing the user fixed). The user fixed that all five review findings are met. Finding 3 names THREE head-script parses: feature_nav_collapse.rs, feature_canzan_theme.rs and xtask. It is met like this. The two ACCEPTANCE copies get ONE owner, crates/foundry-acceptance/src/support/served_markup.rs, which reads each head script src paired with the subset of defer, async and type=module it carries; both step modules call it and neither keeps a copy. The GATE copy, head_script_tags in xtask/src/check_arch.rs, stays a SECOND implementation for a technical reason: xtask depends on anyhow, sha2, serde_json and tempfile only, so a dependency on foundry-acceptance would make cargo xtask check-arch build the whole scraper, cucumber, fantoccini and testcontainers stack for a gate that must run on a host with no Docker, and it would make the gate depend on the very crate it has to be able to contradict. Duplication is therefore reduced from three to two, and the remaining two are not left to agree by luck: a contract test PAIRS them on the shipped head, each asserting the same independently written expected table rather than asserting against the other output. That is the whole of finding 3, and no reduction of it is claimed.
```

#### D15

```text
DECISION: no gate is added to forbid a fourth private head-script parse in the acceptance crate. The producer fix is the shared owner itself, which removes the reason to write one; a gate keyed on the scraper selector spelling would be the very narrow-spelling defect this feature repairs in check_arch. A new copy is a review matter, not a gate.
```

#### D16

```text
DECISION: the gate recognises a head script that stamps the document from stored state by what the asset NAMES - localStorage or sessionStorage, and documentElement - in its comment-stripped source, instead of by the three narrow idiom substrings it uses today. The property is what the asset reaches for; the old test decided a spelling, so a bracket access, a local alias or an element reference escaped the set and was accepted with defer. Every shipped deferred neighbour still falls outside the set: none of them names localStorage or sessionStorage at all.
```

#### D17

```text
DECISION: the gate layer becomes TOTAL over the head. It keeps refusing a stamper that carries any of defer, async or type=module, and it now also refuses a head script that is not a stamper and does not carry defer, naming that script. The two directions are one rule with two arms, so no head-script shape is left without a verdict and the direction the weakened oracle arm stopped observing is observed by the gate as well.
```

#### D18

```text
DECISION: the stale counts in prose are REMOVED rather than pinned by a test. base.html says the ONE script that must not defer and the deferred five while the head declares two pre-paint scripts and seven deferred neighbours; the live panic text in feature_canzan_theme.rs says the same. The repair names the pre-paint FAMILY and the deferred NEIGHBOURS and carries no number, so there is no count left to go stale. A test that pinned those sentences would be a wording pin on a prose target; promise P10 is therefore verified by review and by EXAMINE, and it is deliberately absent from the S1 public-oracle promise list.
```

#### D19

```text
DECISION: static/js/nav-collapse.js is not changed. The icons are server-rendered, the label flyout is CSS, and the marker, the storage key, the sentinel and the button are untouched. Server-rendering the icons is required rather than convenient: the marker lands before the first paint, so an icon injected at DOMContentLoaded would paint an empty strip first - the same flash, one element down.
```

#### D20

```text
DECISION on carriers. The collapse decision is a boolean carried by the valueless data-nav-collapsed attribute on the document element, owned by static/js/nav-collapse.js; its stored form is the single sentinel under foundry.nav, read by a total interpret. The rail geometry is carried by the attribute-presence selectors in the one hand-authored stylesheet. A destination is carried by one a.sidebar__item or the Sign out post form in templates/partials/sidebar.html, with its name in span.sidebar__label and its glyph in svg.sidebar__icon. The gate verdict is carried by the violation string list of xtask/src/check_arch.rs, one entry per refused script, with the empty list as accept; there is no third verdict value. Per-process state: none is added - the gate is a pure read of the tree, and the oracle holds its reading in the step world.
```

#### D21

```text
DECISION: the stylesheet bytes change, so its content-hashed name changes with them. foundry.4c73f62c.css becomes foundry.<new sha256 prefix>.css in the same change as base.html, the three cache-control assertions in crates/foundry-app/src/lib.rs, the VENDOR.md row and its note, and the CHANGELOG entry. The old URL is served immutable, so an edit without the rename pins every holding browser stale; cargo xtask check-arch decides it.
```

#### D22

```text
DECISION: paradigm stays procedural, the repository evidenced style for this surface. The three touched languages are an Askama template, a hand-authored stylesheet and straight-line Rust rule functions over a staged tree; none of them is an object model and none of them is a functional core. No class, no new module layer and no FP relabelling is introduced to fit a carrier.
```

#### D23

```text
DECISION, closing the dependency-direction gap: the rail READING moves to a shared owner too. NAV_PROBE and read_nav are a private const and a private function in feature_nav_collapse.rs, so a second step module could reach them only by a step-to-step import or a visibility change, which contradicts the boundary that no step module depends on another. They move to crates foundry-acceptance/src/support/rail_reading.rs, beside served_markup.rs and for the same reason, and both step modules call the one owner. The new observations this feature needs - the accessible name of each rail item, the invisible-focusable set and the visible label of the focused icon - are added to that ONE reading, so a geometry claim and an accessibility claim still cannot drift into two moments. No step-to-step exception is taken and none is recorded, because none is needed.
```

### Constraints

#### K1

```text
The stored collapse choice is read from one browser key, foundry.nav, with NO length or character bound on the value: the reader is total over every string and over the absence of the key, and exactly one spelling, collapsed, means collapsed. This feature keeps that total reader and widens no bound. The finite stored-value domain the delivered oracle already drives - nothing, collapsed, expanded, Collapsed, 1, null - stays the whole domain.
```

#### K2

```text
The phone boundary is 480 px and below, as the human fixed it. The delivered oracle drives 320, 390 and 480; this design adds no width and narrows none. Above 480 px the desk geometry is unchanged.
```

#### K3

```text
The destination set has no upper bound in the authority: it is Home, Board, Settings, Keyboard shortcuts, plus Integrations for a workspace admin and Instance admin for an instance admin, plus Sign out - seven at most today, four at least. The collapsed bar must lay out the largest set at the narrowest supported width, so the bar wraps rather than scrolls sideways. A destination added later is covered by the standing review obligation that every rail item carries an icon.
```

#### K4

```text
The workspace name bound is the app own refusal limit, 24 characters; the longest name an operator can create must still not widen the strip. The name keeps display:none in the collapsed geometry, so this constraint is met the way it is met today and the delivered scenario that drives it keeps its meaning.
```

#### K5

```text
INHERITED, not restated: the laws of the delivered nav-collapse-toggle feature stay in force and are not re-declared here - two geometries only, selected on the presence of the valueless marker; collapse then expand returns the rail to its shipped 240 px and the stored key to absent; a reload or a navigation never rewrites the stored choice; with scripting off there is no control and the nav is open. See ADR-NAV-COLLAPSE-001 and the nav-collapse-toggle architecture record.
```

#### K6

```text
The collapsed geometry exists only where scripting ran: nothing but static/js/nav-collapse.js stamps the marker. So every promise of the collapsed rail is a promise about a scripting-on document, and the scripting-off document is the expanded text nav it is today. This is a bound on the promise, not a deviation from it.
```

#### K7

```text
Test-time: both slices work inside the project default budgets, so no derogation is requested and none is granted. S1 lands in the default suite through cargo xtask smoke, where the gate rule and its staged-tree fixtures are straight-line file reads. S2 scenarios are @needs-browser and therefore excluded from the default fast lane; they run in cargo xtask ci only with FOUNDRY_XTASK_INCLUDE_DOCKER=1, which every verification command for S2 names. A Docker-less gate observes NOTHING of S2.
```

#### K8

```text
Checker environment: Quint 0.32.0 with the Apalache backend in the shared Lima VM local-dev, reached through scripts/test-in-lima.sh and run from a scratch copy outside the repository. Stopping bound 6 steps under Apalache on both models; the two static tables are exhaustive over their twelve and twenty-four combinations at every bound. No run timed out.
```

#### K9

```text
EVIDENCE LIMIT of the models: they are DESIGN evidence and observe no runtime. Not modelled - pixel layout including the 480 px bar and the icon wrapping; screen-reader output beyond what is visible and focusable; a second tab; how the gate finds the names inside an asset whose identifiers are built at runtime; inline scripts and scripts added at runtime. The icon wrapping, the label flyout and the accessible names need the real browser observation, which belongs to the acceptance oracle.
```

#### K10

```text
Sign out stays reachable in the collapsed strip. The user decided this in chat on 2026-11-17.
```

#### K11

```text
The collapsed left nav shows its links as icons instead of hiding them. The user decided this in chat on 2026-11-17.
```

#### K12

```text
All five non-blocking findings of the nav-collapse-toggle whole-candidate review are fixed in this feature. The user decided this in chat on 2026-11-17.
```

#### K13

```text
Carrying the collapse choice to a second tab stays out of scope. The user decided this in chat on 2026-11-17.
```

#### K14

```text
At phone width (480 px and below) the collapsed bar shows the same icons and lets them wrap inside the bar; no destination is dropped below the breakpoint. The human chose this in chat on 2026-11-17, over a sideways-scrolled row and over keeping today's behaviour.
```

#### K15

```text
The expanded left nav stays text only; no icon is added beside its labels. The human chose this in chat on 2026-11-17.
```

### Reuse

None.

### Impact

None.

### Agreement

The rail render\-class contract\: \.sidebar\, \.sidebar\_\_nav \.sidebar\_\_item\, \.sidebar\_\_user\, aria\-current\=page\, plus the new members svg\.sidebar\_\_icon and span\.sidebar\_\_label

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Producer

Migrated

```text
Each destination gains an icon and moves its word into a label span; no class is renamed or removed and no link or form is added or dropped.
```

The rail render\-class contract\: \.sidebar\, \.sidebar\_\_nav \.sidebar\_\_item\, \.sidebar\_\_user\, aria\-current\=page\, plus the new members svg\.sidebar\_\_icon and span\.sidebar\_\_label

crates\/foundry\-acceptance\/src\/steps\/feature\_navigation\_bar\.rs\:77

Consumer

UnchangedCompatible

```text
It reads each item label with el.text() and matches with contains, and its own comment already anticipates an icon plus label item. The icon adds no text node, because it carries no SVG title element, so the collected text stays exactly the destination word.
```

The rail render\-class contract\: \.sidebar\, \.sidebar\_\_nav \.sidebar\_\_item\, \.sidebar\_\_user\, aria\-current\=page\, plus the new members svg\.sidebar\_\_icon and span\.sidebar\_\_label

crates\/foundry\-acceptance\/src\/steps\/feature\_pwa\_mobile\.rs\:552

Consumer

UnchangedCompatible

```text
It asserts that the first two items of .sidebar__nav share a row within one item height. The phone bar stays a wrapping row in both geometries, so the assertion holds; the collapsed arm only changes what each item draws.
```

The rail render\-class contract\: \.sidebar\, \.sidebar\_\_nav \.sidebar\_\_item\, \.sidebar\_\_user\, aria\-current\=page\, plus the new members svg\.sidebar\_\_icon and span\.sidebar\_\_label

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:406

Consumer

UnchangedCompatible

```text
Its contrast sweep reads the computed colour of .sidebar__item and .sidebar__item--active. Icons inherit that colour through currentColor and add no literal, so the sweep reads the same two surfaces.
```

The rail render\-class contract\: \.sidebar\, \.sidebar\_\_nav \.sidebar\_\_item\, \.sidebar\_\_user\, aria\-current\=page\, plus the new members svg\.sidebar\_\_icon and span\.sidebar\_\_label

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1341

Consumer

Migrated

```text
This is the consumer the change INVERTS. assert_folded and then_no_hidden_item_takes_focus assert navDisplay == none while the rail is folded, and the inFoldedRegion walk treats every focus stop inside .sidebar__nav or .sidebar__user as a trespass. Under the icon rail the destinations stay in the layout and ARE focusable, so both expectations must be restated by their owner: the folded rail keeps its items in the layout and focusable, the workspace name and the identity stay out, and the trespass set becomes the invisible-focusable set, which must be empty.
```

The head\-script shape contract\: which head scripts of the document shell must run before paint and which must defer

crates\/foundry\-app\/templates\/base\.html\:48

Producer

UnchangedCompatible

```text
No tag shape changes: the two pre-paint tags stay plain and the seven neighbours keep defer. Only the comment prose above them changes, and it loses its stale counts.
```

The head\-script shape contract\: which head scripts of the document shell must run before paint and which must defer

xtask\/src\/check\_arch\.rs\:1844

Consumer

Migrated

```text
The layer gains the non-stamper arm and the widened membership test, so it answers every external head script exactly once.
```

The head\-script shape contract\: which head scripts of the document shell must run before paint and which must defer

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:1629

Consumer

Migrated

```text
The neighbour arm is restated over the whole non-pre-paint set instead of over one member, and the panic text and the doc comment above it lose the claims that theme.js is the ONE pre-paint script and that there are five neighbours.
```

The head\-script shape contract\: which head scripts of the document shell must run before paint and which must defer

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1847

Consumer

Migrated

```text
Its private head-script parse is replaced by the call to the shared owner; the assertion it feeds is unchanged.
```

The served\-head reader\: each head script src paired with the subset of defer\, async and type\=module it carries

crates\/foundry\-acceptance\/src\/support\/served\_markup\.rs

Producer

Migrated

```text
New shared owner for the acceptance crate, placed beside the other shared oracle machinery so neither feature oracle depends on the other.
```

The served\-head reader\: each head script src paired with the subset of defer\, async and type\=module it carries

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:2757

Consumer

Migrated

```text
Its own copy is deleted and the shared owner is called instead.
```

The content\-hashed stylesheet URL\, served immutable

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1

Producer

Migrated

```text
The bytes change, so the name becomes the new sha256 prefix.
```

The content\-hashed stylesheet URL\, served immutable

crates\/foundry\-app\/templates\/base\.html\:6

Consumer

Migrated

```text
The stylesheet href carries the new name.
```

The content\-hashed stylesheet URL\, served immutable

crates\/foundry\-app\/src\/lib\.rs\:344

Consumer

Migrated

```text
The three cache-control assertions name the stylesheet by path and must carry the new name.
```

The content\-hashed stylesheet URL\, served immutable

crates\/foundry\-app\/static\/VENDOR\.md\:33

Consumer

Migrated

```text
The provenance row and its two notes carry the new filename and the recomputed sha256, which the gate re-checks.
```

The stored collapse choice\: the foundry\.nav key\, the single sentinel collapsed\, and a reader total over every other string

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

Producer

UnchangedCompatible

```text
Untouched: this feature writes no storage code, and signing out deliberately leaves the key alone.
```

The stored collapse choice\: the foundry\.nav key\, the single sentinel collapsed\, and a reader total over every other string

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\:160

Consumer

UnchangedCompatible

```text
The finite stored-value outline keeps its six rows and its meaning; a browser holding the sentinel from an earlier version still reads as collapsed.
```

The live rail reading\: one probe returning the rail geometry\, the mark\, the item set with each accessible name\, the invisible\-focusable set\, the focused label box and the controls

crates\/foundry\-acceptance\/src\/support\/rail\_reading\.rs

Producer

Migrated

```text
New shared owner, moved out of feature_nav_collapse.rs so neither step module depends on the other. VERIFIED DIRECTION: support depends on fantoccini and serde_json only; it names no step module and no feature.
```

The live rail reading\: one probe returning the rail geometry\, the mark\, the item set with each accessible name\, the invisible\-focusable set\, the focused label box and the controls

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1246

Consumer

Migrated

```text
Its private const and reader are deleted and it calls the shared owner; the delivered assertions keep their inputs.
```

The live rail reading\: one probe returning the rail geometry\, the mark\, the item set with each accessible name\, the invisible\-focusable set\, the focused label box and the controls

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_icon\_rail\.rs

Consumer

Migrated

```text
The new lane calls the same owner rather than declaring a second probe, so the two lanes cannot read the rail differently. VERIFIED DIRECTION: it imports from support, never from another step module.
```

### Not applicable

None.

### Boundaries

```text
Two public ports, one per value. The contributor port is the project gate CLI, cargo xtask check-arch, whose observable surface is the exit status plus one named entry per refused script and, on acceptance, the PASSED line that names the pre-paint-script layer. The operator port is the running app shell in a real browser: her press of the collapse control, her Tab presses, her activation of a rail icon, her one-press submit of the rail Sign out form, her sign-in through the real form, and the viewport width.
```

#### Driven ports

```text
The filesystem the gate reads: the head region of crates/foundry-app/templates/base.html and the comment-stripped bytes of each asset under crates/foundry-app/static that the head names.
```

```text
The browser store holding the foundry.nav sentinel, written and read only by static/js/nav-collapse.js.
```

```text
ServeDir over crates/foundry-app/static, which serves the content-hashed stylesheet immutable.
```

```text
The gate depends on the templates and the static assets and on nothing else - not on foundry-app, not on the acceptance crate - so it can contradict either. The template and the stylesheet are leaves the browser consumes. The acceptance crate depends only on the served output of the app and on its own shared support; no step module depends on another step module, which is why the shared head reader lives in support rather than in either feature module. The server-rendered icons depend on no script, which is what keeps them on the page before the first paint.
```

#### Failures

##### B1

```text
A head script of the document shell NAMES localStorage or sessionStorage and NAMES documentElement in its comment-stripped source, and its tag carries defer, async or type=module.
```

Refusal

```text
cargo xtask check-arch exits non-zero with one entry naming that src, saying that the asset NAMES stored state and the document element - which is what the gate can decide from source, and the reason it is treated as a pre-paint script that must run before the browser is permitted to paint - and telling the contributor to remove the attribute. The wording claims naming, never that the asset was observed to read and stamp: that is what the names test establishes and no more.
```

##### B2

```text
A head script of the document shell is not a member of that set and does not carry the defer attribute - it is plain, async or type=module.
```

Refusal

```text
cargo xtask check-arch exits non-zero with one entry naming that src and telling the contributor to add defer. No shape in this branch is left unanswered.
```

##### B3

```text
The head declares script tags and the derived pre-paint set comes out EMPTY, which is what a parse that stopped seeing the family looks like.
```

Indeterminate

```text
The existing fail-closed entry: the layer reports the head script count and says an empty pre-paint set means the rule is looking at the wrong thing and would hold over nothing, so a PASSED line must not be trusted.
```

##### B4

```text
The document shell declares no head region at all, so its render-blocking tags cannot be found.
```

Indeterminate

```text
The existing no-head entry, naming the shell by path and saying that a head which cannot be read is a head whose tags are unguarded.
```

##### B5

```text
An asset that does stamp the document from stored state builds its identifiers at runtime, so the classifier does not recognise it.
```

Refusal

```text
The gate treats it as a non-stamper and refuses it unless it carries defer, naming it. The verdict is wrong for that asset and it is LOUD: the contributor must widen the classifier or change this decision deliberately, where the narrow classifier accepted it in silence.
```

##### B6

```text
The browser refuses this site access to stored state, so the collapse choice cannot be read or written.
```

Indeterminate

```text
The rail renders expanded, the control still works for the life of the page, and no script error reaches the operator; the choice simply does not survive a reload. Observed on the sign-in screen, because refusing site data also refuses the session cookie.
```

##### B7

```text
A right the rail rendered has been revoked since the last page load and the operator activates that destination from the collapsed rail.
```

Refusal

```text
The app door that owns the destination refuses, exactly as it refuses the same activation from the expanded nav. Pre-existing and unchanged by this feature.
```

##### B8

```text
The stylesheet bytes change and its content-hashed name, its provenance row or one of its references does not move with them.
```

Refusal

```text
cargo xtask check-arch exits non-zero: the content-hashed filename is not its own sha256 prefix, or a /static reference does not resolve, or the VENDOR.md sha256 does not recompute.
```

##### B9

```text
Scripting never ran in the document, so nothing stamped the marker.
```

Indeterminate

```text
The rail is the expanded text nav it is today and no collapse control exists; the collapsed icon rail is simply not reachable, which is a bound on the promise rather than a degraded rail.
```

##### B10

```text
THE FALSE POSITIVE of a names test: a head script that NAMES localStorage or sessionStorage and NAMES documentElement but touches neither before the first paint, because it acts on the settled DOM. Three shipped neighbours - board-dnd.js, form-errors.js and keyboard.js - already name documentElement, so one added storage name reaches this shape.
```

Refusal

```text
The gate refuses it while it carries defer and names it, saying that the gate decided BY NAME: the asset names stored state and the document element, so it is treated as a pre-paint script. The refusal names the two legitimate resolutions and warns against the third. RESOLVE by splitting the stored-state read into its own deferred asset, so the settled-DOM asset no longer names both; or by recording a deliberate exception as a design decision with its reason. DO NOT answer the refusal by dropping defer: that satisfies the gate and puts a non-pre-paint script on the render-blocking path, which the gate then accepts and nothing observes.
```

### Observations

```text
The set of destinations the operator can activate in the rail right now, read from the live document: each a.sidebar__item and the Sign out submit that is in the layout, visible and focusable.
```

Set of destination names

```text
How wide the strip is, which glyph an icon draws, and in which order the members are laid out.
```

```text
The accessible name computed for each rail item in the live document.
```

Destination name per item

```text
Whether the name comes from the clipped label span, from content elsewhere in the item or from an attribute - only that it equals the destination name.
```

```text
The set of rail links that can take the keyboard focus while they are invisible.
```

Set of destination names\, empty when the design holds

```text
Why a member is invisible - clipped, zero-sized or out of the layout.
```

```text
Whether the destination the operator is on is marked in the rail: aria-current=page and the active tint on exactly one item.
```

Marked destination\, or none

```text
The colour of the tint, which the theme lane owns.
```

```text
Whether the label of a focused or hovered icon is visible: a box with a non-zero width that shows the destination word.
```

Visible or not\, plus the shown text

```text
Where the flyout sits and how it is styled.
```

```text
Whether the document scrolls sideways at a given viewport width with the collapsed bar showing its icons, and whether any box reaches past the right edge.
```

Overflow verdict plus the offending boxes

```text
By how many pixels a non-overflowing layout stays inside the edge.
```

```text
The gate verdict for the document shell head: the exit status of cargo xtask check-arch and, for each refused script, its src and the sentence that names what failed, why and how to fix it.
```

Accept\, or one refusal entry per refused script

```text
The order of the refusal entries, and any prose in the head comments - the gate reads comment-stripped markup and source.
```

```text
For one head-script shape - storage named, document element named, loading mode - the verdict the gate gives it.
```

Exactly one of accept or refuse

```text
Which asset carries the shape; the verdict is a property of the shape, not of the file name.
```

### Equality

the rail observations

```text
Two rail renderings are equal when they offer the same activatable destination set, give each item the same accessible name, mark the same current destination, and leave the same - empty - set of invisible focusable links. The strip width, the glyph shapes, the item order and the flyout styling are not observed, so two strips of different widths are equal under this set. The only geometric facts that are observed are that the collapsed rail is strictly narrower than the expanded one and that the document does not scroll sideways.
```

the gate observations

```text
Two heads are equal when the gate gives the same verdict for each of their scripts and names the same scripts in its refusals. Comment prose, tag order and attribute order are not observed: the gate strips comments and reads each tag as a src plus the subset of defer, async and type=module it carries.
```

### Constructors

railItem

destination href \+ name \+ icon \+ active flag \-\> one a\.sidebar\_\_item carrying svg\.sidebar\_\_icon and span\.sidebar\_\_label \(templates\/partials\/sidebar\.html\)

railSignOut

csrf token \+ icon \-\> the existing post form to \/sign\-out whose button carries svg\.sidebar\_\_icon and span\.sidebar\_\_label \(templates\/partials\/sidebar\.html\)

collapsedGeometry

presence of data\-nav\-collapsed on the document element \-\> the narrow icon strip\, selected on presence alone \(the one hand\-authored stylesheet\)

headScriptShape

one head script tag \+ its asset source \-\> \(names stored state\, names the document element\, loading mode\) \(xtask\/src\/check\_arch\.rs\)

verdict

headScriptShape \-\> accept \(no entry\) \| refuse \(one violation entry naming what\, why and how\) \(xtask\/src\/check\_arch\.rs\)

### Laws

#### L1

```text
With the icon rail, for every combination of stored choice, scripting and admin rights, every destination the rendered rights give the operator, and Sign out, is in the layout, visible and focusable in the rail.
```

Invariant

Domain

the collapsed\-geometry region of the one hand\-authored stylesheet\, over the items templates\/partials\/sidebar\.html renders

##### Treatment

Finite

###### Cases

```text
collapsed, scripting on, no admin rights
```

```text
collapsed, scripting on, workspace admin
```

```text
collapsed, scripting on, instance admin
```

```text
expanded, scripting on, each of the three rights
```

```text
scripting off, each of the three rights
```

```text
The state space is the twelve combinations of stored choice, scripting and the three rights; a closed finite domain gets an exhaustive table, not a generated property. The model table matrix enumerates all twelve and is exhaustive at every bound.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/icon\_rail\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

#### L2

```text
No rail link can take the keyboard focus while it is invisible, in either geometry: the invisible-focusable set is empty.
```

Invariant

Domain

the collapsed\-geometry region of the one hand\-authored stylesheet

##### Treatment

Finite

###### Cases

```text
collapsed with the icon rail, each of the three rights
```

```text
expanded, each of the three rights
```

```text
Same closed twelve-combination domain as the reachability law; the comparison variants that break it (links hidden with display none, links hidden visually) are the recorded mutants.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/icon\_rail\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

#### L3

```text
Signing out from the rail leaves the stored collapse choice alone, so the next sign-in in the same browser with scripting on starts collapsed.
```

Invariant

Application

static\/js\/nav\-collapse\.js\, which is the only writer of foundry\.nav\, and the \/sign\-out door\, which does not touch it

##### Treatment

Finite

###### Cases

```text
collapsed, sign out from the rail, sign in again, scripting on
```

```text
collapsed, sign out from the rail, sign in again, scripting off
```

```text
expanded, sign out, sign in again
```

```text
Three closed cases over one boolean and the scripting switch; the mutant that clears the key on sign out is recorded.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/icon\_rail\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

#### L4

```text
The gate gives every head-script shape exactly one verdict, accept or refuse; no shape is left without one.
```

Invariant

Application

xtask\/src\/check\_arch\.rs

##### Treatment

Finite

###### Cases

```text
each of the twenty-four shapes: storage in localStorage, sessionStorage, neither; document element named or not; loading plain, defer, async, type=module
```

```text
The shape domain is closed and small - three storages by two document-element values by four loading modes - so the obligation is an exhaustive table over all twenty-four members, not a generated property. The as-written reading, which leaves eight shapes with no verdict, is the recorded mutant.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/head\_guard\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

#### L5

```text
A head the gate ACCEPTS is a head in which every script that stamps the document from stored state has run before the first paint.
```

Invariant

Application

xtask\/src\/check\_arch\.rs\, over the head of crates\/foundry\-app\/templates\/base\.html

##### Treatment

Finite

###### Cases

```text
the shipped head: two plain stampers and seven deferred neighbours
```

```text
a head whose stamper carries defer
```

```text
a head whose stamper carries async
```

```text
a head carrying a third, aliased stamper that carries defer
```

```text
The four heads are the closed set the paint-order model admits; each is a distinct arrangement rather than a point in a broad domain.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/head\_guard\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

#### L6

```text
The membership test is a NAMES test and is sound in the direction that matters: it recognises every asset that names localStorage or sessionStorage and names documentElement, in whatever idiom its comment-stripped source spells them, so no true pre-paint stamp is missed; it recognises no asset that names neither; and it DELIBERATELY OVER-APPROXIMATES, recognising an asset that names both and touches them only after the settled DOM, which is refused loudly rather than accepted silently.
```

Invariant

Adapter

xtask\/src\/check\_arch\.rs

##### Treatment

Broad

###### Generator

```text
Asset sources composed from a closed grammar of segment kinds: a direct getItem read; a bracket read, window.localStorage of a key; a local alias, var s = window.localStorage then s.getItem; a destructured read; a direct documentElement.setAttribute stamp; an element-reference stamp, var root = document.documentElement then root.setAttribute; a dataset stamp; a settled-DOM line that names documentElement inside a DOMContentLoaded handler; a mention of either name inside a line comment only; a mention inside a block comment only; and a neutral line that names neither.
```

sequences of one to three segments\, enumerated exhaustively over the segment kinds\, in the shape of the existing exhaustive property in this module

none\: the enumeration is total over the bound and therefore deterministic and replayable without a seed

###### Must include

```text
the local-alias read paired with the element-reference stamp, which is the idiom the shipped narrow classifier misses and which MUST be recognised
```

```text
a settled-DOM segment naming documentElement paired with any storage read, which MUST be recognised even though it is not a true pre-paint stamp - the over-approximation, asserted rather than tolerated
```

```text
a comment-only mention of both names, which must NOT be recognised
```

3

```text
not needed and not claimed: the enumeration visits one-segment documents before two and three, so the first failing case is already minimal. xtask carries no property-test library, so there is no shrinker to run; this limitation is stated in the test file.
```

```text
the expected classification is computed from the segment kinds the generator chose, never by calling the classifier
```

```text
an asset built from the local-alias read plus the element-reference stamp is classified as a non-stamper, so defer on its tag is accepted; or an asset built from a settled-DOM segment plus a storage read is classified as a non-stamper, so the over-approximation this law asserts is absent and the law has become a claim about intent that no source test can keep
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/head\_guard\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

#### L7

```text
Each rail item carries the name of its destination as its accessible name in both geometries, and the name is the item own text.
```

Invariant

Domain

templates\/partials\/sidebar\.html for the label span\, the stylesheet for clipping it rather than removing it

##### Treatment

Finite

###### Cases

```text
each of the seven destinations: Home, Board, Settings, Keyboard shortcuts, Integrations, Instance admin, Sign out
```

```text
each of those in the expanded geometry
```

```text
The destination set is closed and at most seven; the obligation is an exhaustive sweep over the rendered items rather than a generated property.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

#### L8

```text
At 480 px and below the collapsed bar lays every icon out inside the bar and the document does not scroll sideways.
```

Invariant

Infrastructure

the 480 px region of the one hand\-authored stylesheet

##### Treatment

Finite

###### Cases

```text
320 px collapsed
```

```text
320 px expanded
```

```text
390 px collapsed
```

```text
390 px expanded
```

```text
480 px collapsed
```

```text
480 px expanded
```

```text
The three widths the delivered oracle already drives, times the two geometries. Pixel layout is not modelled by either checker, so this law is observed only in the real browser and carries no model evidence.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

### Refuted

```text
Every head script that is not a pre-paint script defers - taken as written.
```

```text
Law scope, not a carrier defect: the predicate defers is undefined for async and for type=module, so the rule as written answers nothing for those shapes and a gate must never answer no verdict. Repaired by the confirmed defer-attribute reading, which is total.
```

```text
A non-stamping head script loaded type=module, or loaded async: eight of the twenty-four shapes get no verdict at all. quint run head_guard.qnt --init=init_as_written_wide --step=step --invariant=noThirdVerdict returned a violation.
```

```text
A pre-paint script is one whose source contains localStorage.getItem or sessionStorage.getItem AND one of documentElement.setAttribute, documentElement.removeAttribute, documentElement.dataset.
```

```text
Carrier defect in the classifier: the property is what the asset NAMES, and the test decided a SPELLING. Repaired by widening the membership test to the names themselves.
```

```text
An asset spelling var s = window.localStorage; var root = document.documentElement; root.setAttribute(...) names both and stamps the document, yet is not recognised; given defer, the gate ACCEPTS the head while the stamp lands after the first paint. quint run head_guard.qnt --init=init_defer_attr_narrow --step=step --invariant=gateMatchesPaint returned a violation, and the witness w_aliasAcceptedWithLateStamp returned the accepting trace.
```

```text
The collapsed rail may hide its destinations, because the monogram, the collapse control and the theme control are enough to work with.
```

```text
The carrier was the wrong one: hiding members of the rail cannot preserve reachability, whichever way it hides them. Repaired by rendering the destinations as icons instead of removing them.
```

```text
With display none the activatable set after a collapse is empty and Sign out cannot be activated: quint run icon_rail.qnt --init=init_hide_none --step=step --invariant=matrixNoTrap returned a violation. Hiding them visually instead leaves Board, Home, Keyboard shortcuts, Settings and Sign out able to take the focus while invisible: --init=init_hide_clip --invariant=matrixNoInvisibleFocusable returned a violation.
```

### Claims

#### C1

```text
The gate has no third verdict: a head script shape is accepted or refused, and no shape can be left unanswered.
```

```text
A contributor who reads no verdict concludes the gate has no opinion, ships a deferred stamper, and the flash of the wrong theme or the wrong rail returns on every navigation while every settled-page assertion stays green.
```

xtask\/src\/check\_arch\.rs

```text
One head script tag of the document shell - a src plus the subset of defer, async and type=module it carries - and the comment-stripped bytes of the asset it names.
```

the verdict arms of check\_pre\_paint\_scripts\_are\_render\_blocking

the violation list source\_violations aggregates into the check\-arch exit status and PASSED line

```text
the exit status of cargo xtask check-arch and the refusal entry naming the script
```

```text
Rust gives centralised construction, not a proved predicate: the two arms are exhaustive by inspection, not by the type checker. A script the head adds inline, or adds at runtime, is not in the src set at all and gets no verdict - stated as a limit of the layer rather than claimed as coverage. An asset whose identifiers are built at runtime is classified as a non-stamper.
```

#### C2

```text
A rail item cannot lose its accessible name by losing its word on screen, because the name is the item own text and the collapsed geometry clips that text rather than removing it.
```

```text
An unnamed icon is not a destination a screen-reader operator can choose: the rail is her only route while collapsed, so an icon with no name makes every destination unreachable for her.
```

templates\/partials\/sidebar\.html for the label span\, the collapsed\-geometry stylesheet region for clipping it

```text
The destination name the template renders for that item.
```

railItem and railSignOut

the browser accessible\-name computation\, read by the acceptance oracle

```text
the computed accessible name of each rail item in the live document
```

```text
A clipped span is in the accessibility tree by the standard visually-hidden contract, not by a checker: an assistive technology that drops a one-pixel clipped node would still lose the word. Nothing stops an outside script from writing aria-label on an item and overriding the text. No gate covers either; both are review and oracle obligations.
```

#### C3

```text
INHERITED claim C1, still in force: a third rail geometry is unrepresentable, because every collapsed selector matches on the PRESENCE of data-nav-collapsed and never on a value.
```

```text
An outside script, an extension or devtools writing a value would otherwise select a width that is neither collapsed nor expanded and that no operator could repair.
```

the collapsed\-geometry region of the one hand\-authored stylesheet

```text
Any attribute value, including none, that any writer puts on the document element.
```

collapsedGeometry

the rail geometry the browser lays out

```text
the rail width with a marker carrying a value of its own, which the delivered oracle already drives
```

```text
No gate covers it: rewriting a selector to match a value reopens the state, which is a standing review obligation on the stylesheet. This feature adds selectors to that same region and therefore inherits the obligation for each one it adds.
```

### States

Admitted

```text
The collapsed icon rail exists only in a document where scripting ran; with scripting off the nav is the expanded text nav and there is no control.
```

architect

Admitted

```text
The nav shows the rights of the last page load in both geometries; a right granted or revoked since then is reflected only at the next load, and activating a revoked destination is refused by its own app door.
```

product owner

Admitted

```text
A second tab keeps the geometry it loaded with until it is reloaded; the collapse choice is per browser, so another person signing in on the same browser inherits the collapsed rail as she inherits the theme.
```

product owner

Admitted

```text
With site data refused the rail opens and the choice does not survive a reload; the control still works for the life of the page.
```

architect

Excluded

```text
A rail link that is focusable while invisible.
```

architect

Excluded

```text
A collapsed rail from which a destination the operator has, or Sign out, cannot be reached.
```

architect

Excluded

```text
A head-script shape the gate answers with no verdict.
```

architect

Excluded

```text
A third rail geometry, neither the strip nor the shipped 240 px.
```

architect

Excluded

```text
A head the gate accepts in which a script that stamps the document from stored state runs after the first paint.
```

architect

Unknown

```text
The pixels of the first painted frame: the suite has no paint-capture surface, so whether a collapsed operator ever sees the expanded rail flash stays unobserved and rests on the render-blocking tag shape plus the paint timing.
```

architect

Unknown

```text
Screen-reader output beyond what is visible and focusable: the lane reads the computed accessible name in one browser engine, not what an assistive technology announces.
```

architect

Unknown

```text
Whether the label flyout is legible at every supported width with the longest destination name, and whether it is placed inside the bar at 320 px.
```

acceptance designer

Unknown

```text
Whether an asset whose identifiers are built at runtime, or an inline or runtime-added head script, can reach the document before the first paint without the gate seeing it.
```

architect

Admitted

```text
A names test cannot decide WHEN an asset touches what it names, so a settled-DOM neighbour that names storage and documentElement is classed as a pre-paint script and refused. The over-approximation is deliberate and errs toward refusing.
```

architect

Unknown

```text
Whether a contributor answers the names-test refusal by dropping defer rather than by splitting the stored-state read: the gate accepts the plain result, so nothing observes a non-pre-paint script placed on the render-blocking path.
```

architect

### Models

Quint

docs\/feature\/nav\-collapse\-icon\-rail\/design\/icon\_rail\.qnt

##### Properties

noTrap

Invariant

```text
Every destination the rendered rights give the operator, and Sign out, is activatable in the current state.
```

the a\.sidebar\_\_item set and the \/sign\-out submit rendered by the rail

crates\/foundry\-app\/templates\/partials\/sidebar\.html

matrixNoTrap

Invariant

```text
The same reachability, exhaustively over the twelve combinations of stored choice, scripting and rights.
```

the collapsed\-geometry selectors that decide which rail members stay in the layout

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1030

noInvisibleFocusable

Invariant

```text
No rail link is focusable while it is invisible.
```

the collapsed\-geometry display and clip declarations for the label span and the rail members

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1030

focusNeverInvisible

Invariant

```text
The element holding the focus is never an invisible rail link.
```

the keyboard order of the rendered rail

crates\/foundry\-app\/templates\/partials\/sidebar\.html

matrixNoInvisibleFocusable

Invariant

```text
The invisible-focusable set is empty in every one of the twelve combinations.
```

the collapsed\-geometry selectors

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1030

nextSignInStartsCollapsed

Invariant

```text
After a sign-out from the collapsed rail, the next sign-in with scripting on starts collapsed.
```

the stored sentinel read by interpret and stamped by applyMarker

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

w\_collapseThenActivateIcon

Invariant

```text
Witness, stated as a negated goal: a trace exists in which the operator collapses the rail and then activates a destination from the icon rail.
```

the collapse control press followed by an a\.sidebar\_\_item activation

crates\/foundry\-app\/templates\/partials\/sidebar\.html

expandedShowsCurrentRights

Invariant

```text
Deliberately refutable: the nav shows the rights the operator has right now. It does not - it shows the rights of the last load, in both geometries.
```

the Askama nav view model built per request

crates\/foundry\-app\/templates\/partials\/sidebar\.html

##### Atoms

State

stored

the foundry\.nav key in the browser store

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

```text
One read or one write per press; the key is read once before the first paint and written once per press.
```

State

scripting

whether static\/js\/nav\-collapse\.js ran at all

crates\/foundry\-app\/templates\/base\.html\:65

None.

State

loadedRights

the nav view model fields is\_workspace\_admin and is\_instance\_admin as rendered

crates\/foundry\-app\/templates\/partials\/sidebar\.html

None.

State

focus

document\.activeElement inside the rail

crates\/foundry\-app\/templates\/partials\/sidebar\.html

None.

Action

doCollapse

a press of the nav\-collapse button\, which stamps the marker and writes the sentinel

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

```text
One click handler: marker, storage and repaint happen together; no intermediate state is observable.
```

Action

doActivate

activating one a\.sidebar\_\_item \- a real navigation

crates\/foundry\-app\/templates\/partials\/sidebar\.html

None.

Action

doSignOut

submitting the rail post form to \/sign\-out

crates\/foundry\-app\/templates\/partials\/sidebar\.html

None.

Constant

ALL\_LINKS

the seven destinations the rail can render

crates\/foundry\-app\/templates\/partials\/sidebar\.html

None.

##### Assumptions

```text
Instance admin does not include workspace admin.
```

```text
Admin rights reach the nav only at a page load.
```

```text
A reload can switch scripting on or off.
```

```text
With scripting off the stored choice is kept and ignored.
```

```text
The monogram, the collapse control and the theme control are always visible in the rail.
```

```text
Activating a destination from the rail is a navigation.
```

```text
Icons wrap without clipping anything; pixel layout is not modelled.
```

##### Mutants

###### N1

```text
init_hide_none: the collapsed rail hides its destinations with display none, which is the delivered design this feature replaces.
```

###### Kills

matrixNoTrap

noTrap

###### N2

```text
init_hide_clip: the collapsed rail hides its destinations visually and leaves them focusable.
```

###### Kills

matrixNoInvisibleFocusable

focusNeverInvisible

###### N3

```text
init_icons_clear: Sign out clears the stored collapse choice.
```

###### Kills

nextSignInStartsCollapsed

Quint

docs\/feature\/nav\-collapse\-icon\-rail\/design\/head\_guard\.qnt

##### Properties

totalVerdicts

Invariant

```text
The verdict table holds all twenty-four distinct head-script shapes, once each.
```

the verdict arms of check\_pre\_paint\_scripts\_are\_render\_blocking

xtask\/src\/check\_arch\.rs\:1844

noThirdVerdict

Invariant

```text
Every shape verdict is accept or refuse; none is unanswered.
```

the verdict arms of check\_pre\_paint\_scripts\_are\_render\_blocking

xtask\/src\/check\_arch\.rs\:1844

classifierSeesEveryStamper

Invariant

```text
Every script in the head that really names stored state and the document element is recognised by the gate, whatever idiom it uses.
```

paints\_stored\_state\_onto\_the\_document

xtask\/src\/check\_arch\.rs\:2014

gateMatchesPaint

Invariant

```text
A head the gate accepts is one in which every stamp has landed by the first paint.
```

check\_pre\_paint\_scripts\_are\_render\_blocking over the head of the document shell

crates\/foundry\-app\/templates\/base\.html\:27

stampsBeforePaint\_acceptedHead

Invariant

```text
In the shipped head both stamps have landed at the first paint.
```

the two plain head script tags of the document shell

crates\/foundry\-app\/templates\/base\.html\:48

w\_acceptedHeadDone

Invariant

```text
Witness, stated as a negated goal: the accepted head reaches the done phase, so the paint-order model terminates.
```

the document shell head as served

crates\/foundry\-app\/templates\/base\.html\:3

w\_aliasAcceptedWithLateStamp

Invariant

```text
Witness, stated as a negated goal: with the narrow classifier the gate ACCEPTS a head whose aliased stamp lands after the first paint.
```

paints\_stored\_state\_onto\_the\_document as it is shipped today

xtask\/src\/check\_arch\.rs\:2014

overApproximationNeverSilent

Invariant

```text
A script the gate recognises by name but that is not a true pre-paint stamp is REFUSED while it carries a deferring attribute - the over-approximation is loud, never a silent accept.
```

the stamper arm of check\_pre\_paint\_scripts\_are\_render\_blocking\, reached through the names\-based membership test

xtask\/src\/check\_arch\.rs\:1844

settledOnlyNeverRenderBlocking

Invariant

```text
Deliberately refutable, and the hazard the names test leaves behind: no settled-DOM asset that names stored state and the document element is loaded plain. The gate ACCEPTS it once it is plain, so a contributor who answers the refusal by dropping defer puts a non-pre-paint script on the render-blocking path with the gate green.
```

the head of the document shell\, which no gate arm can protect from this answer

crates\/foundry\-app\/templates\/base\.html\:48

##### Atoms

State

hd

the head region of the document shell as served

crates\/foundry\-app\/templates\/base\.html\:3

None.

State

rec

the asset membership test the gate uses

xtask\/src\/check\_arch\.rs\:2014

None.

State

verdicts

the violation list the layer returns

xtask\/src\/check\_arch\.rs\:1844

None.

Action

runEarly

the browser running a plain head script while parsing is blocked

crates\/foundry\-app\/templates\/base\.html\:48

```text
One script runs to completion before the next; a stamp is visible to the next reader the moment it returns.
```

Action

paint

the browser first contentful paint

crates\/foundry\-app\/templates\/base\.html\:3

```text
Not atomic in the browser; modelled as one point after every render-blocking script has run.
```

Constant

SHAPES

the twenty\-four head\-script shapes the gate can meet

xtask\/src\/check\_arch\.rs\:1935

None.

##### Assumptions

```text
Every head script is external, carrying a src.
```

```text
An async script can run at any time after the head is parsed, before or after the first paint.
```

```text
A type=module script runs with the deferred scripts.
```

```text
The gate reads what an asset NAMES; how it finds the names in source text is not modelled.
```

```text
Inline scripts and scripts added at runtime are outside the model.
```

##### Mutants

###### N6

```text
init_as_written_wide: every other head script defers, read as written, with no reading for async or type=module.
```

###### Kills

noThirdVerdict

###### N7

```text
init_defer_attr_narrow: the classifier recognises only the direct getItem and documentElement idioms, so an aliased stamper escapes the pre-paint set.
```

###### Kills

classifierSeesEveryStamper

gateMatchesPaint

### Closure

```text
The collapsed rail reaches every destination the operator has, and Sign out
```

Safety

Pass

DesignedNotBuilt

```text
The excluded states - a rail link focusable while invisible, and a collapsed rail from which a destination or Sign out cannot be reached - were checked exhaustively under Apalache at bound 6 on icon_rail.qnt for noTrap, matrixNoTrap, noInvisibleFocusable, focusNeverInvisible and matrixNoInvisibleFocusable, each NoError, and each is killed by its own recorded mutant. Runtime is DesignedNotBuilt: no command observed the rendered rail, because the icon members do not exist yet.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/icon\_rail\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

```text
The collapsed rail reaches every destination the operator has, and Sign out
```

Inhabitation

Pass

DesignedNotBuilt

```text
An intended valid value is constructible from admitted input: with foundry.nav holding the sentinel, scripting on and instance-admin rights, the rail renders seven activatable destinations including Sign out. The witness w_collapseThenActivateIcon returned the trace in which the operator collapses the rail and then activates a destination from it.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/icon\_rail\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

```text
The collapsed rail reaches every destination the operator has, and Sign out
```

PublicReachability

Pass

DesignedNotBuilt

```text
The chain is acyclic and buildable: the foundry.nav sentinel in the browser store, read by interpret and stamped by applyMarker in crates/foundry-app/static/js/nav-collapse.js, selects the collapsed-geometry rules at crates/foundry-app/static/css/foundry.4c73f62c.css:1020-1046, which lay out the items templates/partials/sidebar.html renders, observed on the served authenticated page in a real browser at .sidebar .sidebar__item and the /sign-out submit. The two members that do not exist yet are svg.sidebar__icon and span.sidebar__label inside each item of crates/foundry-app/templates/partials/sidebar.html; everything else on the chain is shipped.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/icon\_rail\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

```text
The collapsed rail reaches every destination the operator has, and Sign out
```

Liveness

Pass

DesignedNotBuilt

```text
Every promised observation is reached in ONE bounded user action - a press, a Tab, an activation, a form submit - so nothing waits on an unbounded schedule and no fairness assumption is needed. Progress is nevertheless evidenced rather than asserted: the witnesses w_collapseThenActivateIcon and w_railSignOutThenSignIn each returned a reaching trace on the clean model.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/icon\_rail\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

```text
The head-script gate is total and a head it accepts has every stamp landed before the first paint
```

Safety

Pass

DesignedNotBuilt

```text
Checked exhaustively under Apalache at bound 6 on head_guard.qnt: totalVerdicts, noThirdVerdict, classifierSeesEveryStamper, gateMatchesPaint and overApproximationNeverSilent, each NoError. classifierSeesEveryStamper is stated over the TRUE pre-paint stampers, not over what the source names, so it says the one thing that matters for the flash promise: no real stamp is missed. overApproximationNeverSilent says the other direction is loud: a script recognised by name that is not a true stamper is refused while deferred, never silently accepted. The as-written reading kills noThirdVerdict and the narrow classifier kills both classifierSeesEveryStamper and gateMatchesPaint, so each law has a matching mutant. One hazard is carried rather than closed: settledOnlyNeverRenderBlocking is deliberately refutable and exhibits the trace in which a settled-DOM asset made plain to satisfy the refusal is accepted on the render-blocking path; no gate arm can cover it, so it lives in the refusal wording and in a recorded risk. Runtime is DesignedNotBuilt: the layer shipped today refuses only the deferring direction and decides membership by three narrow substrings, so neither the totality arm nor the widened classifier is built.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/head\_guard\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

```text
The head-script gate is total and a head it accepts has every stamp landed before the first paint
```

Inhabitation

Pass

DesignedNotBuilt

```text
An accepted head is constructible from admitted input: the shipped head - two plain stampers above seven deferred neighbours - is accepted under the chosen reading and classifier, and stampsBeforePaint_acceptedHead holds on it. Each refusal is likewise inhabited: a stamper carrying any of the three attributes, and a non-stamper carrying none.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/head\_guard\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

```text
The head-script gate is total and a head it accepts has every stamp landed before the first paint
```

PublicReachability

Pass

DesignedNotBuilt

```text
The chain is shipped end to end except its widened arms: the head region of crates/foundry-app/templates/base.html and the asset bytes under crates/foundry-app/static/, read by head_script_tags at xtask/src/check_arch.rs:1935 and classified by paints_stored_state_onto_the_document at xtask/src/check_arch.rs:2014, aggregated by source_violations at xtask/src/check_arch.rs:117 into the exit status and PASSED line of cargo xtask check-arch, which is the contributor public port. The DNB sites are the widened membership test at :2014 and the new non-stamper arm inside check_pre_paint_scripts_are_render_blocking at :1844.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/head\_guard\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

```text
The head-script gate is total and a head it accepts has every stamp landed before the first paint
```

Liveness

Pass

DesignedNotBuilt

```text
The gate is one bounded read of the tree and terminates by construction; the paint-order model likewise terminates, which is evidenced rather than assumed: the witness w_acceptedHeadDone returned a trace reaching the done phase on the accepted head. No fairness assumption is needed, so none is declared.
```

##### Evidence

docs\/feature\/nav\-collapse\-icon\-rail\/design\/head\_guard\.qnt

docs\/feature\/nav\-collapse\-icon\-rail\/design\/verification\-receipt\.md

### Risks

#### Y1

Stressor

```text
NEGATE the assumption that the collapsed geometry only exists where nav-collapse.js ran: a content blocker, an extension or devtools leaves the marker on the document element while the asset itself never runs.
```

```text
Two geometries only, and every destination the operator has, plus Sign out, still reachable as an icon.
```

```text
No collapse control exists, so she cannot expand the rail from the page.
```

```text
A destination or Sign out unreachable while the marker is stuck.
```

```text
An operator stuck in the strip with no toggle. This is strictly BETTER than the delivered design, where the same stuck marker left her with no destinations at all: the icon rail turns a trapping state into a cosmetic one.
```

architect

#### Y2

Stressor

```text
NEGATE the assumption that a one-pixel clipped span stays in the accessibility tree: an assistive technology drops it.
```

```text
The icon link is in the layout, focusable and activatable, and its destination is still reachable.
```

```text
The operator hears no destination name for that item.
```

```text
A silent loss with no observation: the lane must read the computed accessible name rather than infer it from the markup.
```

```text
The lane reads one browser engine, so agreement across engines and across assistive technologies is unobserved.
```

acceptance designer

#### Y3

Stressor

```text
NEGATE the assumption that the label flyout fits the viewport: at 320 px the last icon of a wrapped row is at the right edge.
```

```text
No sideways document scroll in either geometry at 320, 390 and 480 px.
```

```text
The flyout is placed under the icon and may be clipped by the bar.
```

```text
The flyout widening the document, which is the overflow the phone promise forbids.
```

```text
A flyout partly cut at very narrow widths; the destination is still reachable and still named to a screen reader.
```

crafter

#### Y4

Stressor

```text
NEGATE the assumption that the repository has ONE document shell: a second template grows a head of its own.
```

```text
The head of crates/foundry-app/templates/base.html stays governed, and the fail-closed arm still reports a head whose pre-paint set came out empty.
```

```text
Another template head is ungoverned.
```

```text
A PASSED line that implies coverage the layer does not have.
```

```text
The layer names one shell by path. Nothing detects a second shell; the layer doc comment must say so instead of claiming the repository.
```

architect

#### Y5

Stressor

```text
NEGATE the assumption that an asset names its storage and its document access literally: it builds the name at runtime, or reaches them through an imported helper.
```

```text
Every asset that NAMES either storage and the document element in its comment-stripped source is recognised, in any idiom.
```

```text
An asset whose identifiers are assembled at runtime is classified as a non-stamper and may carry defer.
```

```text
Claiming the layer governs every asset a contributor could write.
```

```text
The layer claimed strength is: it recognises what the source NAMES. That limit belongs in the layer doc comment, replacing the current claim that a third pre-paint script is governed the moment the head references it.
```

architect

#### Y6

Stressor

```text
NEGATE the assumption that every rail item gets an icon: a destination added by a later feature carries a label and no icon.
```

```text
The item is still a link, still focusable, still named.
```

```text
In the collapsed strip it draws nothing visible where the icon would be.
```

```text
An item whose word is clipped AND whose icon is absent, which looks like an empty target.
```

```text
No gate covers this. It is a standing review obligation on templates/partials/sidebar.html, of the same kind as the inherited obligations on the stylesheet and on the control mount: every rail item carries an icon, or the collapsed geometry must keep its word.
```

architect

#### Y7

SinglePointOfFailure

```text
The one hand-authored stylesheet carries both geometries, the icon and label rules and the phone block; a syntax error in the collapsed region silently disables that whole region.
```

```text
The destinations stay text links in the shipped rail look and stay reachable.
```

```text
The collapsed geometry does not apply, so the rail stays full width after a press.
```

```text
An unreachable destination.
```

```text
An operator who collapsed sees a full-width rail - degraded, never trapped. The acceptance lane observes the strip width in the same probe as the state, so the failure is loud.
```

architect

#### Y8

Bypass

```text
An outside writer: a browser extension or an injected script writing aria-label, hidden or inert on a rail item, or writing data-nav-collapsed with a value of its own.
```

```text
Presence-only selectors keep exactly two geometries, which is inherited claim C1 and is observed by the delivered oracle.
```

```text
An outside aria-label overrides the item own text as the accessible name.
```

```text
A selector rewritten to match the marker VALUE, which would reopen a third geometry.
```

```text
No gate covers either direction; both are standing review obligations, and this feature adds selectors to the same region and inherits them.
```

architect

#### Y9

Bypass

```text
The gate reads only head script tags that carry a src. An inline head script, or a script the page appends at runtime, is outside the set entirely and receives no verdict.
```

```text
Every external head script of the shell gets exactly one verdict.
```

```text
An inline or runtime-added script stamps the document without a verdict.
```

```text
Describing the layer as total over the head without naming this set.
```

```text
The totality claim is total over EXTERNAL head scripts. The layer doc comment and the refusal text must say so.
```

architect

#### Y10

Stressor

```text
SECOND STRESSOR SET, walked over the finished design and not used to derive it. The operator rights change between the collapsed render and her press.
```

```text
The rail she sees is the rail the last load rendered, and activating a destination whose right was revoked is refused by its own app door.
```

```text
The rail shows a destination she no longer has until the next load.
```

```text
A silent success on a revoked destination.
```

```text
Pre-existing, found by the model as expandedShowsCurrentRights and carried as an admitted state; neither repaired nor worsened here.
```

product owner

#### Y11

Stressor

```text
SECOND STRESSOR SET. The gate runs on a host with no Docker, so cargo xtask ci silently skips the browser lane and observes none of S2.
```

```text
S1 is fully observed: its rule and fixtures run in the default suite through cargo xtask smoke.
```

```text
A green ci run that observed no scenario of S2.
```

```text
Closing S2 on evidence from a run whose lane never executed.
```

```text
Every S2 verification command names FOUNDRY_XTASK_INCLUDE_DOCKER=1 and the lane tag, and any close evidence for S2 must name the lane that actually ran.
```

host

#### Y12

Stressor

```text
SECOND STRESSOR SET. The stylesheet is edited without being renamed, or renamed in base.html only.
```

```text
Nothing: cargo xtask check-arch refuses, because the content-hashed filename must be its own sha256 prefix, every VENDOR.md sha256 must recompute and every /static reference must resolve.
```

```text
A red gate until the rename lands in base.html, in the three cache-control assertions of crates/foundry-app/src/lib.rs and in the VENDOR.md row together.
```

```text
Shipping a stale immutable URL, which pins every holding browser.
```

```text
None: the gate decides it, and the delivered feature already walked this path once.
```

crafter

#### Y13

Stressor

```text
NEGATE the assumption that naming stored state and the document element means stamping from stored state before the paint: a settled-DOM neighbour names both for work it does after the DOM is settled.
```

```text
No true stamper is ever missed - the over-approximation errs toward refusing, never toward accepting, and classifierSeesEveryStamper holds over the TRUE stampers. The refusal is loud and names the script: overApproximationNeverSilent was verified at bound 6.
```

```text
A legitimately deferred asset is refused and its author has to split the stored-state read into its own deferred asset, or record a deliberate exception as a design decision.
```

```text
Answering that refusal by dropping defer. The gate ACCEPTS the asset once it is plain, so a non-pre-paint script would then sit on the render-blocking path with the gate green - the hazard the deliberately refutable settledOnlyNeverRenderBlocking exhibits as a trace.
```

```text
No gate arm covers the plain-made case, because no source test can tell a real pre-paint stamp from a settled-DOM one. The remaining obligation is the refusal WORDING, which must name the two legitimate resolutions and warn against the third, plus the standing review obligation on the head: a new plain head script needs a recorded reason.
```

architect

### Architectural tests

#### T1

```text
T1, extended from the delivered T1: the pre-paint script law becomes TOTAL over the external head scripts of the document shell. A head script whose asset names localStorage or sessionStorage AND names documentElement, in any idiom, must carry none of defer, async or type=module; every other head script must carry defer; and every script gets exactly one of those two verdicts, named with what failed, why and how to fix it. The law governs the whole set, including a script added later, at the strength of what the source NAMES. The fail-closed arm stays: a head that declares scripts and yields an empty pre-paint set is reported.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo xtask check-arch
```

#### T2

```text
T2, an existing layer this change can violate: no colour literal outside the three stylesheet token regions, and the three regions declare the identical colour-token set. Every rule this feature adds - the icon, the label, the clip and the flyout - names only --cz-* tokens, so the icons and the flyout follow the palette the operator chose.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo xtask check-arch
```

#### T3

```text
T3, an existing layer this change can violate: every /static reference resolves, every content-hashed filename is its own sha256 prefix, and every VENDOR.md sha256 recomputes. The stylesheet bytes change in S2, so its name, its VENDOR.md row and every reference to it must move together.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo xtask check-arch
```

### Contract tests

None.

## Slices

### S1

#### Targets

xtask\/src\/check\_arch\.rs

Extend

```text
The pre-paint layer gains its second arm, its widened membership test and its de-numbered texts. The crafter owns the rule functions; the staged-tree test module pre_paint_script_tests in the same file is the oracle and is owned by the acceptance designer.
```

crates\/foundry\-app\/templates\/base\.html

Extend

```text
The two pre-paint comment blocks lose their stale counts and name the pre-paint family and the deferred neighbours instead. No tag, attribute or script order changes.
```

CHANGELOG\.md

Extend

```text
One entry for the total gate and the corrected head texts.
```

xtask\/src\/check\_arch\.rs

#### Verification

```text
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo test -p xtask check_arch::
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo xtask check-arch
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo xtask smoke
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo build --release -p foundry-app
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=canzan-theme-system CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=nav-collapse-toggle CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
```

#### Supports

crates\/foundry\-acceptance\/src\/support\/served\_markup\.rs

crates\/foundry\-acceptance\/src\/support\/mod\.rs

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature

#### Public oracle

```text
A contributor runs the project gate, cargo xtask check-arch, over the repository as it stands, and over staged trees that differ only in the head of the document shell and in the bytes of the asset that head names: (a) the shipped head, two plain stampers above seven deferred neighbours; (b) a head whose stamping asset reads the stored choice through a local alias and stamps through an element reference, with defer on its tag; (c) the same asset with async, and again with type=module; (d) a head one of whose deferred neighbours has lost its defer; (e) a head whose non-stamping script carries async, and one whose non-stamping script carries type=module; (f) a head whose DEFERRED neighbour names localStorage and documentElement but touches both only inside a DOMContentLoaded handler - the false positive of a names test. She also runs the gate over each of the twenty-four head-script shapes - three storage values by document element named or not by the four loading modes. Independently, the gate reader and the shared acceptance reader are each asserted against the same expected table of the shipped head nine (src, deferring) pairs.
```

```text
(a) exit 0, with the PASSED line still naming the pre-paint-script layer. (b) and (c) non-zero, each with one entry naming that src, saying that the asset NAMES stored state and the document element and so is treated as a script that must run before the browser is permitted to paint, and telling her to remove the attribute - whichever idiom the asset spells localStorage, sessionStorage and documentElement in. (d) non-zero, with one entry naming the neighbour that lost its defer and telling her to add defer - not a pass earned because the other six neighbours still defer. (e) non-zero for each, naming the script and telling her to add defer, rather than answering nothing. (f) non-zero, with one entry naming that src and saying the gate decided BY NAME, offering the two legitimate resolutions - split the stored-state read into its own deferred asset, or record a deliberate exception - and warning that dropping defer would put a non-pre-paint script on the render-blocking path. Over the twenty-four shapes, each shape receives exactly one verdict, accept or refuse, and every refusal names what failed, why it matters and how to fix it. The two readers yield the identical nine pairs for the shipped head. The operator sees no change in the app: no served byte of the head changes.
```

```text
Any of these fails the slice: the aliased stamper carrying defer, async or type=module is accepted; a deferred neighbour that silently lost its defer is accepted; a non-stamping head script carrying async or type=module receives no entry; the deferred neighbour that names both is accepted, or is refused with wording that claims the asset was observed to READ and STAMP rather than to name, or with wording whose only remedy is to drop defer; a shape appears twice or not at all in the sweep; a refusal names no script, or omits what failed, why, or how to fix it; the shipped head is refused; the fail-closed arm stops reporting a head whose pre-paint set came out empty; either reader yields a different set of pairs from the expected table for the shipped head; or a served byte of the document shell head other than a comment changes.
```

##### Promises

P8

P9

P12

#### Prefactoring

```text
L3, extract to a shared owner, BEFORE any behaviour changes: move the head-script parse that feature_nav_collapse.rs:1847 and feature_canzan_theme.rs:2757 each declare privately into crates/foundry-acceptance/src/support/served_markup.rs, and have both modules call it with no change to either caller verdicts. This makes the follow-up change easier: the neighbour arm is then restated ONCE, over a reading both modules share, instead of in two copies that can disagree, and a fourth deferring attribute is added in one place rather than two.
```

```text
Both head-script observations keep their current verdicts on the shipped head: the theme scenario load-bearing assertion (a), and the nav-collapse no-flash scenario first Then.
```

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature

##### Verification

```text
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo build --release -p foundry-app
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=canzan-theme-system CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=nav-collapse-toggle CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
```

#### Records

##### Decisions

None.

##### Constraints

None.

##### Reuse

check\_pre\_paint\_scripts\_are\_render\_blocking

xtask\/src\/check\_arch\.rs\:1844

Extend

```text
The pre-paint layer already owns this law, is already wired into source_violations, the rule-marker count and the PASSED line, and already fails closed on an empty pre-paint set. It gains the non-stamper arm and loses its stale prose; a second layer would split one law across two owners.
```

paints\_stored\_state\_onto\_the\_document

xtask\/src\/check\_arch\.rs\:2014

Extend

```text
This is the membership test the review refuted. It keeps its place and its comment-stripping, and its body widens from three idiom substrings to the names themselves. Replacing it with a new predicate would leave two membership tests for one set.
```

head\_script\_tags

xtask\/src\/check\_arch\.rs\:1935

Reuse

```text
The gate own head parse already pairs each src with the subset of defer, async and type=module it carries, which is exactly the shape both new arms need. It stays the gate deliberately independent second implementation and is not merged with the acceptance crate reader.
```

head\_scripts

crates\/foundry\-acceptance\/src\/support\/served\_markup\.rs

CreateNew

```text
Three copies of one parse exist today: feature_nav_collapse.rs:1847, feature_canzan_theme.rs:2757 and the gate own. CANDIDATES CONSIDERED. support/html_assertions.rs is the closest existing owner, and it does NOT safely own this: it is a thin generic wrapper over scraper for the markdown-rendering scenarios, its parse uses parse_fragment where a head selection needs parse_document, and its exported shape is select-and-dump rather than one semantic reading of a served head. Reaching into either step module private helper would couple two feature oracles. support/ is where this crate already keeps shared oracle machinery - browser_harness.rs, harness.rs - so a new file there leaves neither feature depending on the other. The gate copy in xtask stays separate on purpose.
```

html\_assertions

crates\/foundry\-acceptance\/src\/support\/mod\.rs\:22

Extend

```text
The shared support module list is the one declaration point for this crate helpers; the new served-head reader is declared beside html_assertions, which is the closest existing neighbour and the candidate the reuse analysis rejected as an owner.
```

then\_the\_theme\_is\_settled\_before\_the\_browser\_is\_permitted

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:1629

Extend

```text
The weakened arm lives here and its owner restates it over the whole non-pre-paint set. A new scenario would leave the weakened arm in place.
```

head\_scripts

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1847

Replace

```text
Its private copy is deleted in favour of the call to the shared owner; the assertion it feeds does not change.
```

nav\-collapse\.js

crates\/foundry\-app\/templates\/base\.html\:53

Extend

```text
The standing instruction to the next engineer who touches the head lives in the two comment blocks above the two pre-paint tags. They are corrected in place and de-numbered; a separate document would leave the wrong sentences beside the tags.
```

Changelog

CHANGELOG\.md\:1

Extend

```text
The project keeps one release-notes file and every change adds its entry at the top of it; there is no second place for it.
```

##### Impact

gate rule

xtask\/src\/check\_arch\.rs\:1844

Changed

```text
Two arms instead of one, a widened membership test, and a de-numbered doc comment and refusal text.
```

gate aggregate

xtask\/src\/check\_arch\.rs\:117

UnchangedCompatible

```text
The layer is already wired into source_violations and the PASSED line; its sentence there may need the second arm named, which is prose inside the same change.
```

document shell prose

crates\/foundry\-app\/templates\/base\.html\:27

Changed

```text
The counts go: the pre-paint FAMILY and the deferred NEIGHBOURS replace the ONE script and the five.
```

oracle support\, new

crates\/foundry\-acceptance\/src\/support\/served\_markup\.rs

Migrated

```text
One owner for the served-head reading, called by both step modules.
```

crate support interface

crates\/foundry\-acceptance\/src\/support\/mod\.rs\:18

Changed

```text
One module declaration added.
```

oracle step module

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:1586

Migrated

```text
The arm, its panic text and its doc comment are restated by their owner, and the private parse is replaced by the shared call.
```

oracle step module

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1847

Migrated

```text
The private parse is replaced by the shared call.
```

provenance document

crates\/foundry\-app\/static\/VENDOR\.md\:100

UnchangedCompatible

```text
Its note already names both pre-paint assets and makes no count claim, so it stays true after the de-numbering.
```

release notes

CHANGELOG\.md\:1

Changed

```text
One entry for the totality of the gate and the corrected head texts.
```

build and manifest metadata

xtask\/Cargo\.toml

UnchangedCompatible

```text
No dependency is added: the widened test is string work over bytes already read, and the exhaustive sweeps are hand-written enumerations in the style this module already uses. xtask carries no property-test library and gains none.
```

runtime

crates\/foundry\-app\/src\/lib\.rs

UnchangedCompatible

```text
S1 changes no served byte: the head tags keep their shapes and only comments change, so the operator sees no change in the app.
```

##### Agreement

None.

##### Not applicable

None.

##### Boundaries

```text
The project gate CLI: cargo xtask check-arch, run over a repository tree. Its observable surface is the exit status plus, on refusal, one entry per refused script naming what failed, why it matters and how to fix it; on acceptance, the PASSED line that names the pre-paint-script layer. The staged-tree fixtures in the layer own test module drive the same rule over trees the contributor cannot produce in the live repository.
```

###### Driven ports

```text
The filesystem: the head region of crates/foundry-app/templates/base.html and the comment-stripped bytes of each asset under crates/foundry-app/static/ that the head names.
```

```text
The served document head, read by the acceptance crate through one shared owner, for the second independent observation of the same contract.
```

```text
The gate depends on the templates and the static assets and on NOTHING else - not on foundry-app, not on the acceptance crate, so it can contradict either. The acceptance crate depends on the served output of the app and on its own shared support; no step module depends on another step module. The shared head reader depends on scraper alone and knows no feature.
```

###### Failures

None.

##### Observations

None.

##### Equality

None.

##### Constructors

None.

##### Laws

None.

##### Refuted

None.

##### Claims

None.

##### States

None.

##### Models

None.

##### Closure

None.

##### Risks

None.

##### Architectural tests

None.

##### Contract tests

###### T4

```text
The gate reader and the acceptance reader must agree about the shipped head, so the claim that they are independent implementations is observed rather than asserted. TWO HALVES, ONE EXPECTED TABLE. The xtask half asserts that head_script_tags over crates/foundry-app/templates/base.html yields exactly the nine expected pairs - the two plain pre-paint tags, and the seven neighbours each carrying defer - written out independently in the test, not read from the other reader. The acceptance half asserts that the shared reader over the SERVED page yields exactly the same nine pairs; it runs under scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=canzan-theme-system CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release. A reader that drifts - a fourth deferring attribute, a different quoting form, a missed tag - fails on exactly the side that drifted. STATED LIMIT: a drift that changes both readers identically is not caught, and the two inputs differ by construction (template source against served markup), which is what makes the pairing worth having rather than a tautology.
```

The head\-script reading\: each head script src of the document shell paired with the subset of defer\, async and type\=module it carries

xtask\/src\/check\_arch\.rs\:1935

crates\/foundry\-acceptance\/src\/support\/served\_markup\.rs

```text
Both readers yield the identical set of nine (src, deferring attributes) pairs for the shipped head, each compared with the same independently written expected table.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo test -p xtask check_arch::
```

### S2

#### Targets

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Extend

```text
Each destination and the Sign out button gain an inline aria-hidden SVG icon with focusable=false and no title element, and their word moves into span.sidebar__label. Classes, hrefs, the form and the active marking stay as they are.
```

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css

Extend

```text
The collapsed geometry keeps only the workspace name and the identity out of the layout; the destinations stay and show their icon with the label clipped, revealed as a flyout on hover and focus-visible; the 480 px arm lets the icons wrap inside the bar and places the flyout under its icon. Every added rule names only --cz-* tokens. THE BYTES CHANGE, SO THE NAME CHANGES: this blob becomes foundry.<new sha256 prefix>.css in the same change, and the old name is left nowhere.
```

crates\/foundry\-app\/templates\/base\.html

Extend

```text
The stylesheet href carries the new content-hashed name. Nothing else in the head changes.
```

crates\/foundry\-app\/src\/lib\.rs

Extend

```text
The three cache-control assertions that name the stylesheet by path carry the new name.
```

crates\/foundry\-app\/static\/VENDOR\.md

Extend

```text
The stylesheet provenance row and its notes carry the new filename and the recomputed sha256, and a note records the Lucide release tag the inline icon paths were copied from, the icon names and the ISC notice. No hashed row is added for the icons: an inline path in a template is not a served blob, so a sha256 row would be a claim about a file that does not exist.
```

CHANGELOG\.md

Extend

```text
One entry for the collapsed icon rail and the stylesheet re-hash.
```

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature

#### Verification

```text
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo xtask check-arch
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo xtask smoke
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo build --release -p foundry-app
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=nav-collapse-icon-rail CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=nav-collapse-toggle CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=navigation-bar CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=pwa-mobile-rendering CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=canzan-theme-system CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
scripts/test-in-lima.sh env FOUNDRY_XTASK_INCLUDE_DOCKER=1 CARGO_BUILD_JOBS=4 cargo xtask ci
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_icon\_rail\.rs

crates\/foundry\-acceptance\/src\/support\/rail\_reading\.rs

crates\/foundry\-acceptance\/src\/support\/mod\.rs

crates\/foundry\-acceptance\/src\/lib\.rs

crates\/foundry\-acceptance\/tests\/acceptance\.rs

crates\/foundry\-acceptance\/src\/support\/browser\_harness\.rs

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature

#### Public oracle

```text
In a real browser against the running app, an operator who is both a workspace admin and an instance admin signs in through the real form and opens an authenticated app-shell page at desk width. She presses the collapse control. She walks the whole page with the Tab key, stopping on each rail icon. She activates the Board icon. She submits Sign out from the narrow rail, in one press, and signs in again in the same browser. The same journey is then driven at 480 px and at 390 px viewports.
```

```text
While the rail is collapsed it is a strip strictly narrower than the shipped 240 px, and every destination her rights give her - Home, Board, Settings, Keyboard shortcuts, Integrations, Instance admin - and Sign out are in the layout, visible and focusable in it, each with the accessible name of the destination it opens. The workspace name and her display name are out of the layout. No rail link can take the focus while it is invisible. The icon that holds the focus shows its destination word in a visible box. Activating Board opens the board with the rail still a strip and the Board icon alone carrying aria-current=page and the active tint. The one press of Sign out ends her session with no confirmation step, and her next sign-in in the same browser starts with the rail collapsed and with the icons her current rights give her. At 480 px and at 390 px the bar shows every one of those icons, wrapping inside the bar, and the document does not scroll sideways in either geometry.
```

```text
Any of these fails the slice: a destination her rights give her, or Sign out, is absent, out of the layout, invisible or unfocusable while the rail is collapsed; any rail link can take the focus while invisible; an icon accessible name is empty or is not its destination; the rail does not narrow, or narrows to nothing; the focused icon shows no visible label; the Board icon is not marked, or more than one item is; Sign out needs a second press or a confirmation; the next sign-in starts expanded; the document scrolls sideways at 480 or 390 px with the icons shown; the workspace name or her display name appears in the strip; or the expanded rail gains an icon beside its words.
```

##### Promises

P1

P2

P3

P4

P5

P6

P7

P11

#### Prefactoring

```text
L3, extract to a shared owner, BEFORE any behaviour changes: move NAV_PROBE and read_nav out of crates/foundry-acceptance/src/steps/feature_nav_collapse.rs into crates/foundry-acceptance/src/support/rail_reading.rs and have that step module call the owner, with every assertion it feeds unchanged and no new observation added yet. This makes the follow-up change easier and legal: the new lane then extends ONE reading with the accessible-name, invisible-focusable and focused-label observations, instead of either importing a private const across step modules - which the boundaries forbid - or declaring a second probe that could drift from the first.
```

```text
Every delivered nav-collapse observation keeps its verdict: the geometry and content widths, the mark and its value, the nav and workspace display, the item count, shown and focusable counts, and the control and theme-control readings.
```

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature

##### Verification

```text
scripts/test-in-lima.sh env CARGO_BUILD_JOBS=4 cargo build --release -p foundry-app
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=nav-collapse-toggle CARGO_BUILD_JOBS=4 cargo test -p foundry-acceptance --release
```

#### Records

##### Decisions

None.

##### Constraints

None.

##### Reuse

sidebar\_\_item

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:11

Extend

```text
This partial already owns every destination, the active marking, the user section and the Sign out form, and it is the one place the rail is rendered. Each destination gains an icon and a label span in place. CANDIDATE CONSIDERED AND REJECTED: extracting an Askama macro for the item row. Seven literal rows in one file, each with its own glyph, would gain an indirection and a per-icon branch without removing a decision; no follow-up change is made easier by the move.
```

data\-nav\-collapsed

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1020

Extend

```text
The one hand-authored stylesheet already owns the whole .sidebar family, the collapsed geometry, the theme and collapse controls and the 480 px block. The icon, label, clip and flyout rules belong beside the geometry they modify; a second stylesheet would split the rail across two cache-immutable blobs.
```

max\-width\: 480px

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1332

Extend

```text
The phone block already re-asserts the bar geometry for the collapsed selector, which is exactly where the icons must be told to wrap inside the bar and the flyout to sit under its icon.
```

sidebar\_\_item\-\-active

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1130

Reuse

```text
The current-page marking already exists as the raised surface tint plus weight, and the server already renders aria-current=page. The collapsed rail reuses both; no second marking channel is added.
```

applyMarker

crates\/foundry\-app\/static\/js\/nav\-collapse\.js\:98

Reuse

```text
The marker, the storage key, the sentinel, the total reader and the button are reused unchanged. Nothing in the icon rail needs a script: that is why the icons are server-rendered.
```

static\_cache\_control\_value

crates\/foundry\-app\/src\/lib\.rs\:344

Extend

```text
The three assertions that name the stylesheet by path must carry its new content-hashed name; they are the existing owner of that claim.
```

assert\_folded

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1347

Extend

```text
The delivered folded-state assertion is the consumer this change inverts: it asserts navDisplay == none. Its owner restates it so the folded rail keeps its destinations in the layout and focusable while the workspace name and the identity stay out. Replacing the helper would leave the old expectation in the scenarios that share it.
```

NAV\_PROBE

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1246

Replace

```text
The one probe that reads the geometry and the state together is the right reading to extend, and it is PRIVATE to this step module, so the new lane cannot call it without a step-to-step import. It is replaced by a move: the const and its read_nav wrapper go to support/rail_reading.rs and this module calls that owner instead. Extending it in place and widening its visibility was rejected: it would make one step module a dependency of another, which the boundaries forbid and which D14 already refused for the head reader.
```

browser\_harness

crates\/foundry\-acceptance\/src\/support\/mod\.rs\:16

Reuse

```text
The real-browser session, the Selenium node and the session-slot contract are shared machinery; the new lane opens its sessions through them and adds no harness of its own.
```

foundry\.4c73f62c\.css

crates\/foundry\-app\/templates\/base\.html\:6

Extend

```text
The document shell already owns the one stylesheet reference; it carries the new content-hashed name. There is no second reference to reuse or create.
```

css\/foundry\.4c73f62c\.css

crates\/foundry\-app\/static\/VENDOR\.md\:33

Extend

```text
The provenance table already owns the stylesheet row and the authored-in-tree audit procedure for it; the row carries the new filename and the recomputed sha256. The icon provenance is added as a note in the same file rather than as a hashed row, because an inline path in a template is not a served blob.
```

Changelog

CHANGELOG\.md\:1

Extend

```text
The project keeps one release-notes file and every change adds its entry at the top of it; there is no second place for it.
```

read\_nav

crates\/foundry\-acceptance\/src\/support\/rail\_reading\.rs

CreateNew

```text
CANDIDATES CONSIDERED. support/browser_harness.rs owns the session and the Selenium node contract, not what a page shows, so a rail reading there would mix two responsibilities. support/html_assertions.rs is a scraper wrapper over served markup and cannot execute script in a live browser. Leaving the probe in feature_nav_collapse.rs is what the boundary forbids. A new support module beside served_markup.rs leaves neither feature oracle depending on the other, which is the same shape D14 chose for the head reader.
```

##### Impact

rail template

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:1

Changed

```text
Each destination and the Sign out button gain an inline aria-hidden SVG icon and a label span; no class is renamed or removed, no link or form is added or dropped.
```

stylesheet\, content\-hashed blob

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1020

Migrated

```text
The collapsed geometry stops hiding the destinations, gains the icon, label, clip and flyout rules and the phone arm; the bytes change, so the filename becomes the new sha256 prefix.
```

document shell stylesheet reference

crates\/foundry\-app\/templates\/base\.html\:6

Changed

```text
The href carries the new content-hashed name.
```

cache\-policy assertions

crates\/foundry\-app\/src\/lib\.rs\:344

Changed

```text
Three assertions name the stylesheet path and must move with the rename.
```

provenance document

crates\/foundry\-app\/static\/VENDOR\.md\:33

Changed

```text
The stylesheet row and its two notes carry the new name and the recomputed sha256, and a note records the Lucide release the inline icon paths were copied from with its ISC notice.
```

oracle\, new lane

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature

Migrated

```text
The new feature file carries this slice scenarios under its own tag.
```

oracle step module\, new

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_icon\_rail\.rs

Migrated

```text
The steps of the new lane; it reuses the nav probe reading rather than declaring a second one.
```

crate step interface

crates\/foundry\-acceptance\/src\/lib\.rs\:12

Changed

```text
One module declaration added to the steps list.
```

oracle step module

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1347

Migrated

```text
The folded-state expectations invert: the destinations stay in the layout and focusable, and the trespass set becomes the invisible-focusable set, which must be empty.
```

oracle feature file

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\:216

Migrated

```text
The scenario about a folded rail leaving no hidden link in the keyboard order keeps its promise and changes its premise: nothing in the rail is hidden any more, so its Then is restated by its owner.
```

oracle consumer

crates\/foundry\-acceptance\/src\/steps\/feature\_navigation\_bar\.rs\:77

UnchangedCompatible

```text
It reads item labels with el.text() and matches with contains; the icon adds no text node, so the collected text stays the destination word.
```

oracle consumer

crates\/foundry\-acceptance\/src\/steps\/feature\_pwa\_mobile\.rs\:552

UnchangedCompatible

```text
It asserts the first two nav items share a row within one item height; the phone bar stays a wrapping row in both geometries.
```

oracle consumer

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:2384

UnchangedCompatible

```text
Its contrast sweep reads the computed colour of the item and the active item; icons inherit it through currentColor and add no literal.
```

pre\-paint asset

crates\/foundry\-app\/static\/js\/nav\-collapse\.js\:1

UnchangedCompatible

```text
Not touched: the icons are server-rendered and the flyout is CSS, so the marker, the key, the sentinel, the reader and the button all stay as delivered.
```

build and manifest metadata

crates\/foundry\-acceptance\/Cargo\.toml

UnchangedCompatible

```text
No dependency is added: the new lane uses the shared browser harness and scraper, which the crate already carries. No new static asset is served, so no manifest or lock entry changes.
```

release notes

CHANGELOG\.md\:1

Changed

```text
One entry for the collapsed icon rail and the re-hashed stylesheet.
```

oracle support\, new

crates\/foundry\-acceptance\/src\/support\/rail\_reading\.rs

Migrated

```text
One owner for the live rail reading, called by both step modules; it gains the accessible-name, invisible-focusable and focused-label observations.
```

crate support interface

crates\/foundry\-acceptance\/src\/support\/mod\.rs\:16

Changed

```text
One module declaration added for the rail reading, beside browser_harness and served_markup.
```

oracle step module

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1246

Migrated

```text
The private probe const and its reader move out to the shared owner; this module calls it and keeps every assertion it feeds.
```

##### Agreement

None.

##### Not applicable

None.

##### Boundaries

```text
The running app shell in a real browser: the operator press of the collapse control, her Tab presses, her activation of a rail icon, her submit of the rail Sign out form, her sign-in through the real form, and the viewport width. Observations are layout and accessibility facts read from the live document through ONE shared reading - which rail members are in the layout and focusable, the computed accessible name of each, which item carries aria-current=page, whether a focused icon label is visibly shown, the strip and content widths, and the horizontal-overflow verdict - plus the choice the origin holds.
```

###### Driven ports

```text
The browser store, holding the foundry.nav sentinel, written and read only by static/js/nav-collapse.js.
```

```text
ServeDir over crates/foundry-app/static, which serves the content-hashed stylesheet immutable.
```

```text
The template and the stylesheet are leaves: the browser consumes them and they know nothing of the acceptance crate. NO STEP MODULE DEPENDS ON ANOTHER STEP MODULE, and this slice keeps that direction by MOVING the rail reading into support/rail_reading.rs rather than by importing a private const across modules or widening its visibility; both step modules depend on support, support depends on neither. The oracle depends only on the served output and on that shared support. Nothing in the app depends on the oracle. static/js/nav-collapse.js is not a dependency of the icons: they are server-rendered precisely so the rail does not wait for a script.
```

###### Failures

None.

##### Observations

None.

##### Equality

None.

##### Constructors

None.

##### Laws

None.

##### Refuted

None.

##### Claims

None.

##### States

None.

##### Models

None.

##### Closure

None.

##### Risks

None.

##### Architectural tests

None.

##### Contract tests

None.

## ADRs

- NAV\-ICON\-RAIL\-001 Collapsed left nav is an icon rail\, not a hidden one — Accepted
- NAV\-ICON\-RAIL\-002 The head\-script gate answers every shape\, by what the asset names — Superseded by NAV-ICON-RAIL-003
- NAV\-ICON\-RAIL\-003 A names test over\-approximates\, and its refusal says so — Accepted
