# Architecture brief: nav\-collapse\-toggle

## Paradigm

procedural

## Shared records

### Decisions

None.

### Constraints

#### K1

```text
The collapsed or expanded choice is kept per browser, the same way the theme choice is kept, not per account or across devices.
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

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

Create

```text
The whole mechanism: the guarded read, the boolean carrier, the valueless marker, the button and its accessible name. A straight-line guarded IIFE, like theme.js.
```

crates\/foundry\-app\/templates\/base\.html

Extend

```text
One render-blocking script tag beside theme.js, with no defer, async or type=module, and the stylesheet href moved to the renamed blob.
```

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css

Extend

```text
The collapsed rail rules, the hidden items, the theme-control word span, and the collapsed arm inside the existing 480px block. The file carries the sha256 prefix of its current bytes, so any further edit renames it again.
```

crates\/foundry\-app\/src\/lib\.rs

Extend

```text
The three static cache-control tests name the stylesheet hashed path; the path moves, the assertions do not.
```

crates\/foundry\-app\/static\/VENDOR\.md

Extend

```text
The provenance row path and sha256 move with the renamed blob, and the gate recomputes the digest.
```

xtask\/src\/check\_arch\.rs

Extend

```text
One layer for the pre-paint script law over the whole set of pre-paint scripts, with its fixtures in the style the file uses, and its sentence added to the gate PASSED line.
```

CHANGELOG\.md

Extend

```text
The repository records every stylesheet re-hash here; this change names its new hash.
```

crates\/foundry\-app\/templates\/partials\/sidebar\.html

Extend

```text
Exactly one id attribute on the existing nav.sidebar__nav element, so the toggle aria-controls has something to name. No element, class, link or form is added, renamed or removed.
```

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature

#### Verification

```text
scripts/test-in-lima.sh cargo xtask check-arch
scripts/test-in-lima.sh cargo xtask smoke
scripts/test-in-lima.sh env FOUNDRY_ACCEPTANCE_TAGS=nav-collapse-toggle cargo test -p foundry-acceptance --release
scripts/test-in-lima.sh env FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs

crates\/foundry\-acceptance\/src\/lib\.rs

crates\/foundry\-acceptance\/tests\/acceptance\.rs

crates\/foundry\-acceptance\/src\/support\/browser\_harness\.rs

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs

#### Public oracle

```text
An operator signed in to the app shell opens an authenticated page in a real browser at desktop width, activates the rail collapse control with the keyboard, then reloads that page and opens another app-shell page in the same browser; she then activates the control again and reloads once more. The same journey is driven at a real 390 by 844 phone viewport, and once in a session with scripting disabled.
```

```text
After the first activation the rail is a narrow control strip and the content box is wider by the width the rail gave up, with the rail and content widths still summing to the shell width. After the reload and after the other page, the rail is still the strip and the document already carries the collapse marker at the first moment a script can read it, from a head script tag that carries no defer, async or type=module. After the second activation the rail is back at its shipped 240 px and stays expanded across the next reload. Throughout, the control is reachable by keyboard and its accessible name and expanded state say which state the rail is in and what the next press does. At 390 by 844 the page has no horizontal overflow in either state. With scripting disabled the rail renders expanded and no control for it exists in the document.
```

```text
Any of these fails the slice: the content box width does not change when the rail does, or the two widths stop summing to the shell width; the rail is expanded again after a reload or on another page; the marker is absent at first script time, or its script tag carries defer, async or type=module; the control cannot be reached or activated from the keyboard, or its accessible name does not name the state; the page overflows horizontally at 390 by 844 in either state; a control for the nav is present in a document where scripting never ran; or a nav link stays focusable while the rail is collapsed.
```

##### Promises

P1

P2

P3

P4

P5

P6

#### Prefactoring

None.

#### Records

##### Decisions

```text
Follow the theme control pattern but not the theme asset: a NEW render-blocking head script crates/foundry-app/static/js/nav-collapse.js stamps the collapse marker before first paint and builds its own button, so no control exists when scripting is off. theme.js stays a byte-identical canzan-lift port and is not taught a second job, which base.html states as a standing constraint.
```

```text
The per-document collapse state is a JavaScript boolean held in the nav-collapse.js closure; the document marker is the VALUELESS attribute data-nav-collapsed on the document element; the stored form is the single sentinel "collapsed" under localStorage key foundry.nav, and the expanded state is the ABSENCE of that key. Carrier owner: static/js/nav-collapse.js, which also owns the effects (attribute write, storage write) and the one error (a refused storage origin). No per-process state exists outside the document.
```

```text
The toggle mounts into .sidebar__brand at the head of the rail, so it stays visible and focusable in the collapsed strip. It adds no focus rule: .sidebar :focus-visible already outlines focused descendants of the rail in both palettes.
```

```text
Collapsed geometry: the rail narrows to a control strip. .sidebar__nav, .sidebar__workspace and the .sidebar__user links, identity and sign-out form take display none, so no invisible element stays focusable; .sidebar__monogram, the collapse toggle and .theme-toggle stay. .theme-toggle__mode hides so the theme control fits the strip, which costs it no accessible name because theme.js puts that name in aria-label, not in the word span.
```

```text
No rule widens the content: .app-shell__content is already flex 1 1 auto with min-width 0, so it takes exactly the width the rail gives up. The affected layers: domain, where the rail is in one of two states; application, where a press and a document load are the only operations; adapter, where the stored sentinel and the document marker live; infrastructure, where a refused storage origin and a browser that never runs the script live.
```

```text
Phone layout: inside the existing max-width 480px media block the collapsed selector re-asserts the top-bar geometry (width auto, flex 0 0 auto), so the collapsed state only removes items from the bar and never applies a desktop rail width inside a row layout. The toggle joins that block existing 44px touch-target list beside .theme-toggle.
```

```text
The control is a real button element carrying aria-expanded, aria-controls naming the rail primary navigation, and an aria-label that names the SUBJECT as the nav, the current state and what the next press does: "Left nav: expanded. Collapse the left nav." and "Left nav: collapsed. Expand the left nav.". The wording is fixed to that shape, not left to the crafter: a label that named only the state would be a valid accessible name that no reader could tie to the nav. The id aria-controls points at is SERVER-RENDERED, exactly one id attribute added to the existing nav.sidebar__nav in partials/sidebar.html: an id is static, is harmless with scripting off, and keeps the asset to its two DOM effects instead of a third that would reach outside its own button. Keyboard activation needs no code: a button answers Enter and Space.
```

```text
No cross-tab synchronisation: a second already-open tab keeps its geometry until it reloads or navigates. The product scopes out syncing across browsers, devices and accounts, and within one browser promises only the reload and the next page. No storage event listener is added.
```

```text
The collapsed rules land in the one hand-authored stylesheet, whose filename is its own sha256 prefix under an immutable cache policy. The blob is renamed to foundry dot new-8-hex dot css, and base.html, the three static cache-control tests in crates/foundry-app/src/lib.rs and the static/VENDOR.md provenance row move with it in the same change. cargo xtask check-arch decides that rename; no new gate is needed for it.
```

```text
Checker mapping: Quint with the Apalache backend covers the cross-document state machine (three laws, two reachability witnesses) because two documents of one browser write one storage key, which is shared state with more than one actor. No TLA+: the feature states no fairness or temporal claim. No Agda: there is no proof over types to discharge. Paint ordering is outside every model and is carried as a separate observation.
```

```text
Every localStorage read and write is wrapped, as theme.js does. A refused origin costs the operator persistence and never the control: the press still moves the rail in the current document.
```

```text
Procedural paradigm, preserved from the surfaces this change touches: nav-collapse.js is a straight-line guarded IIFE like theme.js, and the new gate layer is a free function beside the other check_arch layers. Procedural has no smell catalogue, so the general classes applied are duplicated knowledge (one owner for the storage key and the sentinel), long function (separate read, decide, apply and build steps), and primitive obsession (the boolean carrier replaces a state string). No class or object is introduced to fit a label.
```

##### Constraints

```text
The stored value has NO bound: localStorage admits any string of any length under key foundry.nav, and the authority states no bound either. The design admits that whole domain and interprets every string that is not exactly "collapsed" as expanded, so no narrower validation is introduced and no input is refused.
```

```text
Phone bound from the promise: at 480 px and below the page has no horizontal scroll with the rail expanded or collapsed. The shipped mobile harness drives 390 by 844 with real mobile viewport semantics, which is inside that bound; 480 px itself is the boundary the media query switches at and is exercised as its own case.
```

```text
The expanded rail stays at its shipped 240 px. The collapsed strip width is a design value set once in the stylesheet and named in the ADR; it is not a product bound, and no promise fixes it.
```

```text
DEVIATION from the standing rule that every formal law gets a fast executable projection on the pure core in the default suite: the pure core is JavaScript, and the repository has no JS test runner, no package.json and no node step in cargo xtask ci. Each law projection is therefore a browser-lane acceptance observation through the real document, and no toolchain is added for an S-sized change. The gap is the risk owner item recorded against the host, not a silent omission.
```

```text
The browser lane is tagged needs-browser, excluded from the default lane, and runs only with FOUNDRY_XTASK_INCLUDE_DOCKER=1 in cargo xtask ci. A green gate on a host with no Docker observes none of this feature scenarios, so the close evidence for this slice must name the lane that actually ran.
```

```text
Deployment independence holds by construction: the feature adds no route, handler, migration, secret, service, URL, DNS name or FOUNDRY_ setting, so a standalone docker compose, plain Kubernetes or single-binary deploy needs nothing new.
```

```text
Tests and both gates run in the shared Lima VM local-dev through scripts/test-in-lima.sh, never on Docker on the Mac. The acceptance lane for this feature is selected with FOUNDRY_ACCEPTANCE_TAGS=nav-collapse-toggle.
```

```text
Default test budgets apply unchanged: no infrastructure suite, no derogation request and no new default-suite time. The browser scenarios already live outside the default lane under the shipped needs-browser arrangement.
```

##### Reuse

nav\-collapse\.js

crates\/foundry\-app\/static\/js\/nav\-collapse\.js\:1

CreateNew

```text
No shipped asset owns a navigation state. keyboard.js owns shortcuts, the three board scripts own the board, and theme.js is a frozen port. Extending any of them would put a second responsibility in a file whose own header forbids it, so the collapse state gets the one file that owns it.
```

theme\.js

crates\/foundry\-app\/templates\/base\.html\:48

Extend

```text
Before this feature the document shell carried exactly ONE render-blocking script, the theme one cited here, plus the hashed stylesheet link at line 6. The feature adds a second render-blocking tag immediately after it, with no defer, async or type=module, and moves the line 6 href to the renamed blob. The SEVEN deferred tags below do not move, and copying their shape is the regression the architectural obligation refuses. Going from one pre-paint script to two is what breaks the shipped flash scenario arm that counts them, which is why that step file is now a declared support.
```

check\_static\_asset\_integrity

xtask\/src\/check\_arch\.rs\:1615

Extend

```text
The static-asset gate family already decides hashed-filename honesty, VENDOR.md digests and static reference resolution, so the rename needs no new gate. One layer is added beside them for the pre-paint script law, which now governs two scripts and no longer fits a template comment.
```

static\_cache\_control\_value

crates\/foundry\-app\/src\/lib\.rs\:344

Extend

```text
Three shipped cache-control tests name the stylesheet by its hashed path. They are the consumers the rename must carry; the assertions themselves are unchanged.
```

VENDOR\.md

crates\/foundry\-app\/static\/VENDOR\.md\:33

Extend

```text
The provenance row records the stylesheet path and its sha256, and cargo xtask check-arch recomputes it. Both move with the renamed blob in the same change.
```

STORAGE\_KEY

crates\/foundry\-app\/static\/js\/theme\.js\:41

Reuse

```text
Consulted as the pattern owner and deliberately NOT edited: the pre-paint head tag, the JS-built control, the guarded storage access, the foundry-dot namespace for the key and the state-naming accessible name all come from here. base.html records that this file is a byte-identical canzan-lift port whose divergence is the thing the port exists to avoid, so the nav state gets its own asset and its own key beside this one.
```

sidebar\_\_brand

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:2

Extend

```text
Two uses of this file, one of them an edit. The brand row cited here is the mount point and is NOT edited: the control is built in JavaScript, so a server-rendered button would be a dead control with scripting off, and this row already survives the collapse so the toggle has somewhere visible to live. The edit is elsewhere in the same file and is exactly one id attribute on the nav.sidebar__nav element, which the toggle aria-controls names.
```

app\-shell\_\_content

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:953

Extend

```text
The one hand-authored stylesheet already owns .app-shell, this flexible content column, the whole .sidebar family, the theme control and the 480px phone block. The collapsed geometry belongs beside them and needs no rule of its own for the content, which is already flex 1 1 auto with min-width 0; a second stylesheet would split the rail rules across two cache-immutable blobs. The blob is cited by its CURRENT name: the design mandated re-hash has landed, from foundry.8d661015.css to foundry.4c73f62c.css.
```

new\_session

crates\/foundry\-acceptance\/src\/support\/browser\_harness\.rs\:408

Reuse

```text
Every browser state this oracle needs already ships beside this constructor: open_mobile_session for real 390x844 mobile semantics, new_session_without_scripting for the scripting-off promise, new_dark_session_refusing_site_storage for the refused-storage guard, plus unhandled_script_errors, viewport and sign_in_through_browser. No new harness capability is required.
```

sidebar\_\_nav

crates\/foundry\-acceptance\/src\/steps\/feature\_navigation\_bar\.rs\:33

Reuse

```text
Consulted as the render-contract consumer: its scraper asserts this class contract over the server HTML, including the rail, the two primary items, the user menu and the sign-out form. The feature renames none of those selectors and adds no server-rendered element, so those scenarios stay green untouched.
```

foundry\.8d661015\.css

CHANGELOG\.md\:897

Extend

```text
The repository records every stylesheet re-hash in this file, which is why the previous one is named at this line. This change adds its own entry for the new hash; nothing else in the file moves.
```

##### Impact

static script asset

crates\/foundry\-app\/static\/js\/nav\-collapse\.js\:1

Changed

```text
New file: the whole collapse mechanism, served from the app own origin and linked from the document shell.
```

document shell template

crates\/foundry\-app\/templates\/base\.html\:48

Changed

```text
Two sites in this file change. The new pre-paint script tag is inserted immediately after the theme tag cited here, so the two render-blocking scripts sit together above the seven deferred ones; and the stylesheet link at line 6 moves to the renamed blob. Nothing else in the head or the body changes. The head declares ten script tags in total, two of them pre-paint.
```

stylesheet blob and its content\-hash filename

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:984

Migrated

```text
Rules added, so the bytes changed and the filename became the new sha256 prefix: foundry.8d661015.css is now foundry.4c73f62c.css. The old URL is served immutable, so an edit without the rename would have pinned every holding browser stale; cargo xtask check-arch decides it. The cited line is the .sidebar rule the collapsed geometry overrides.
```

static cache\-control unit tests in the production crate

crates\/foundry\-app\/src\/lib\.rs\:344

Migrated

```text
Three assertions name the stylesheet hashed path. They move to the new name with their assertions unchanged.
```

asset provenance table

crates\/foundry\-app\/static\/VENDOR\.md\:33

Migrated

```text
The row path and recorded sha256 move with the renamed blob; the gate recomputes the digest from the committed bytes.
```

architecture gate layer

xtask\/src\/check\_arch\.rs\:1615

Changed

```text
One layer added for the pre-paint script law, with its two unit fixtures in the style the file already uses, and its sentence joined to the gate PASSED line.
```

release notes

CHANGELOG\.md\:897

Changed

```text
The repository records every stylesheet re-hash here; this change adds its own entry naming the new hash.
```

acceptance feature file

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\:1

Changed

```text
New oracle, owned by the acceptance designer: the slice scenarios and the lane tags.
```

acceptance step module and its registration

crates\/foundry\-acceptance\/src\/steps\/feature\_nav\_collapse\.rs\:1

Changed

```text
New step module, owned by the acceptance designer; it must also be declared in the steps module list and used from tests/acceptance.rs the way every other feature module is, or cucumber never collects it.
```

sidebar template

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:6

Changed

```text
Exactly one id attribute added to the nav element cited here, which carries class sidebar__nav and aria-label Primary. No selector is renamed, removed or newly required and no element is added, so every HTTP-lane scraper assertion over this markup keeps its meaning; an id is inert with scripting off.
```

theme control asset

crates\/foundry\-app\/static\/js\/theme\.js\:36

UnchangedCompatible

```text
Not edited. It keeps mounting into .sidebar__user and keeps its accessible name in aria-label, so hiding its word span in the collapsed strip costs it no name. The byte-identical port stays byte-identical.
```

navigation render\-contract scenarios

crates\/foundry\-acceptance\/src\/steps\/feature\_navigation\_bar\.rs\:1

UnchangedCompatible

```text
Its scraper reads the server HTML, where nothing this feature adds appears. Expanded is the markup default, so the existing rail, item, user-menu and sign-out assertions are unaffected.
```

phone\-layout scenarios

crates\/foundry\-acceptance\/src\/steps\/feature\_pwa\_mobile\.rs\:1

UnchangedCompatible

```text
Its no-horizontal-overflow and desktop-unchanged assertions still hold: the collapsed state is never the initial state of a session that has not stored a choice, and the phone block re-asserts the top-bar geometry for the collapsed selector.
```

theme control scenarios

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:1633

Changed

```text
REFUTED by a real run, not predicted: the us-cts-04 lane exits 101 because the shipped flash scenario asserts at this line that theme.js is the ONE head script that does not defer, and this design deliberately adds a second render-blocking pre-paint script, so the arm reads 1 of the 8 other head scripts not deferring. The control reachability, focus indicator and 24x24 / 44x44 target scenarios are still unaffected, for the reason they always were: they run with no stored nav choice, so the rail is expanded and the theme control geometry is the shipped one. The arm that breaks asserts a COUNT over the set of pre-paint scripts, which this feature changes from one to two, and its restatement belongs to the oracle owner.
```

##### Agreement

the content\-hashed stylesheet filename under an immutable cache policy

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1

Producer

Migrated

```text
The blob bytes changed, so its name became the new sha256 prefix: foundry.8d661015.css is now foundry.4c73f62c.css.
```

the content\-hashed stylesheet filename under an immutable cache policy

crates\/foundry\-app\/templates\/base\.html\:6

Consumer

Migrated

```text
The only link tag that serves the stylesheet to a browser; a stale href resolves to a deleted file and the gate static-reference layer refuses it.
```

the content\-hashed stylesheet filename under an immutable cache policy

crates\/foundry\-app\/src\/lib\.rs\:344

Consumer

Migrated

```text
Three cache-control assertions name the path literally and fail on the old name.
```

the content\-hashed stylesheet filename under an immutable cache policy

crates\/foundry\-app\/static\/VENDOR\.md\:33

Consumer

Migrated

```text
The provenance row path and digest are recomputed by the gate from the committed bytes.
```

the valueless data\-nav\-collapsed marker on the document element

crates\/foundry\-app\/static\/js\/nav\-collapse\.js\:1

Producer

Migrated

```text
Sole writer. It stamps presence or absence from a boolean and never writes a value.
```

the valueless data\-nav\-collapsed marker on the document element

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css\:1020

Consumer

Migrated

```text
The consuming rule now EXISTS at the cited line, where it was recorded as the insertion anchor to be created: it selects on attribute PRESENCE, so a stray value from any writer still selects one of exactly two geometries. The collapsed arm inside the existing 480px phone block is the second site.
```

the foundry\.nav browser storage key\, which outlives every version that wrote it

crates\/foundry\-app\/static\/js\/nav\-collapse\.js\:1

Producer

Migrated

```text
Writes the single sentinel "collapsed" or removes the key; it never writes a word for expanded, so the pre-feature default stays absence.
```

the foundry\.nav browser storage key\, which outlives every version that wrote it

crates\/foundry\-app\/static\/js\/nav\-collapse\.js\:1

Consumer

UnchangedCompatible

```text
Reads it through a total interpretation: any string that is not exactly the sentinel, including a value left by another tool or a later version, reads as expanded. No stored value can be rejected or crash the read.
```

the rail markup mount points \.sidebar\_\_brand and \.sidebar\_\_user

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:2

Producer

UnchangedCompatible

```text
Both already exist and neither is renamed or restructured.
```

the rail markup mount points \.sidebar\_\_brand and \.sidebar\_\_user

crates\/foundry\-app\/static\/js\/theme\.js\:112

Consumer

UnchangedCompatible

```text
The theme control keeps appending into .sidebar__user and keeps its accessible name in aria-label; the collapsed strip hides its word span only.
```

##### Not applicable

contract_tests

```text
No party in this change can change apart from the others. The one contract whose parties could drift, the content-hashed stylesheet filename against base.html, the three cache-control tests and the VENDOR.md row, is already decided by the shipped asset-hash, VENDOR-digest and static-reference layers of cargo xtask check-arch, which recompute from the committed bytes rather than from a hand-written record. Every other party, the script, the marker, the stylesheet rules and the template tag, ships in one deployable in the same commit and is covered by the primary oracle. No persisted or wire data outlives a producer version except the browser storage key, whose producer and consumer are the same file and whose foreign-value case is an admitted state with its own law case.
```

prefactoring

```text
Nothing existing is in the way. The change adds one asset, one head tag and additive stylesheet rules, and touches no structure a preserving move could improve first. The one rename it forces, the stylesheet content hash, cannot be done before the edit: the new name IS the hash of the new bytes, so it is a mechanical consequence of the change and not a move that could be verified green before it.
```

##### Boundaries

```text
HTTP GET on any authenticated app-shell page (/, /team/{team}/project/{project}, .../report, /admin/tokens, /workspace/invites) rendered in a real browser, plus the operator activating the rail collapse control in that document
```

###### Driven ports

```text
the browser origin localStorage key foundry.nav, read and written only by static/js/nav-collapse.js, every access guarded
```

```text
the valueless data-nav-collapsed attribute on the document element, the only channel from the script to the stylesheet
```

```text
the stylesheet depends on the marker, the marker depends on the stored sentinel, and nothing in Rust depends on either; no server-side code reads or writes the collapse choice, so no route, handler, migration or FOUNDRY_ setting enters the feature
```

###### Failures

####### B1

```text
Site storage is refused for the origin, so reading the stored choice throws.
```

Refusal

```text
The page renders the expanded rail, nothing is reported to the operator, and no unhandled script error appears. The choice does not survive the next load.
```

####### B2

```text
Site storage is refused for the origin, so writing the choice throws.
```

Refusal

```text
The press still moves the rail in this document; only persistence is lost. Not observable through a signed-in screen, because refusing site storage also blocks the session cookie.
```

####### B3

```text
The script never runs: scripting is disabled, or the asset fails to load.
```

Refusal

```text
The rail renders expanded, exactly as before this feature, and no control for it exists in the document.
```

####### B4

```text
The document carries no rail, which is every pre-auth and utility screen.
```

Refusal

```text
No control is built and no element is appended; the marker is still stamped on the document element, where no rule consumes it.
```

####### B5

```text
The key holds a value this feature never wrote.
```

Refusal

```text
The rail renders expanded. No error, no refusal copy, and no third geometry.
```

##### Observations

```text
the rendered width of the rail and of the content box on one authenticated page at a fixed viewport, read from the live browser
```

two CSS pixel lengths

```text
which stylesheet rule produced either width, and whether the state came from a press in this document or from the stored choice
```

```text
whether the document element carries the collapse marker at the first moment a script can read it, together with the shape of the tag that put it there
```

presence or absence\, plus the attribute set of that script tag

```text
the pixels of the first painted frame; this suite uses no CDP surface, so first-frame colour is not observable and the no-flash promise rests on the render-blocking tag shape plus paint timing
```

```text
the accessible name and the expanded state the toggle exposes, read through the browser
```

a string and a boolean

```text
the glyph a sighted operator sees, and whether a screen reader announces the name the way the operator expects
```

```text
whether the page overflows horizontally at a real 390 by 844 mobile viewport, in each rail state
```

document scroll width against window inner width

```text
overflow inside an element that scrolls itself, which the board columns do by design
```

```text
how many controls for the nav exist in the document, and whether anything was reported to the operator, on a session with scripting disabled or site storage refused
```

an element count and a list of unhandled script errors

```text
what the rail would have done had scripting been on in that same session
```

##### Equality

the five observations above

```text
Two documents are equal when they show the same rail and content widths, the same marker presence, the same accessible name and expanded state, the same overflow verdict and the same control count. The stored string is NOT observable through any of them, so a browser holding no key, a browser holding "expanded" and a browser holding "1" are the same document; only the exact sentinel "collapsed" is a different one.
```

##### Constructors

interpret

interpret\(storedValue\: string \| null\) \-\> boolean

sentinel

sentinel\(collapsed\: boolean\) \-\> \"collapsed\" \| absent\, written to or removed from localStorage key foundry\.nav

applyMarker

applyMarker\(collapsed\: boolean\) \-\> data\-nav\-collapsed present or absent on document\.documentElement

buildToggle

buildToggle\(brand\: Element\) \-\> HTMLButtonElement appended to \.sidebar\_\_brand\, or nothing when the rail is absent

##### Laws

###### L1

```text
The rail shows what the operator of this document last asked for, her own press or the choice the document loaded with, and nothing else moves it; a press in another tab of the same browser never moves this rail.
```

Invariant

Domain

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

###### Treatment

Finite

###### Cases

```text
expanded rail, one press: the rail is collapsed and the content box is wider by the width the rail gave up
```

```text
collapsed rail, one press: the rail is expanded again at its shipped 240 px
```

```text
collapsed rail, a press in a second tab of the same browser: this tab geometry does not change until it navigates
```

```text
The state domain is two values over two documents, a closed finite domain, so examples cover every transition and a generated property would repeat them.
```

###### Evidence

docs\/feature\/nav\-collapse\-toggle\/design\/nav\_collapse\_choice\.qnt

docs\/feature\/nav\-collapse\-toggle\/design\/verification\-receipt\.md

###### L2

```text
A document that has read the storage key more recently than any press shows the last choice made in any document of this browser, which is what staying collapsed across reloads and pages means.
```

Invariant

Application

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

###### Treatment

Finite

###### Cases

```text
collapse, then reload the same page: still collapsed
```

```text
collapse, then open another app-shell page: still collapsed
```

```text
collapse, expand again, then reload: expanded
```

```text
Closed finite domain: two states crossed with the two ways a document is re-read, reload and navigation.
```

###### Evidence

docs\/feature\/nav\-collapse\-toggle\/design\/nav\_collapse\_choice\.qnt

docs\/feature\/nav\-collapse\-toggle\/design\/verification\-receipt\.md

###### L3

```text
After a press the storage key holds exactly that press canonical form, and a document load never rewrites the key.
```

Invariant

Adapter

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

###### Treatment

Finite

###### Cases

```text
press to collapsed: the key holds the sentinel
```

```text
press back to expanded: the key is gone, not set to a word
```

```text
three page loads with no press: the key is byte-identical to what it was before them
```

```text
Closed finite domain over the two states and the one load operation; the load case is the one that matters, because a load that normalised storage would erase a real collapsed choice the first time storage happened to be unreadable.
```

###### Evidence

docs\/feature\/nav\-collapse\-toggle\/design\/nav\_collapse\_choice\.qnt

docs\/feature\/nav\-collapse\-toggle\/design\/verification\-receipt\.md

###### L4

```text
Activating the toggle twice returns the rail to the geometry it started from.
```

Algebraic

Domain

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

###### Treatment

Finite

###### Cases

```text
start expanded, two presses: expanded, and the key is absent
```

```text
start collapsed, two presses: collapsed, and the key holds the sentinel
```

```text
The carrier is a boolean and the operation is its negation, so the domain has exactly two members; an exhaustive pair of examples IS the whole domain.
```

###### Evidence

None.

###### L5

```text
Reading back what the feature wrote yields the same state, and every other stored string yields the expanded default rather than a state no stylesheet matches.
```

RoundTrip

Adapter

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

###### Treatment

Finite

###### Cases

```text
no key: expanded
```

```text
the sentinel "collapsed": collapsed
```

```text
the word "expanded": expanded
```

```text
the case variant "Collapsed": expanded
```

```text
the value "1": expanded
```

```text
the value "null": expanded
```

```text
The round trip over the feature own two forms is finite and exhaustive. The broad half, arbitrary strings from an outside writer, has no runner: the pure core is JavaScript and the repository has no JS test framework, so the six cases above are the observable projection of a law that is total by construction over all strings.
```

###### Evidence

None.

###### L6

```text
With site storage refused for the origin, the page still renders and nothing is reported to the operator.
```

Invariant

Infrastructure

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

###### Treatment

Finite

###### Cases

```text
storage-refused session on the sign-in screen: the page renders, the document carries no collapse marker, and no unhandled script error is reported
```

```text
The other half of the degradation, that a press still moves the rail with storage refused, is unreachable by construction and is NOT claimed as tested: refusing site storage also blocks the session cookie, so no signed-in screen is reachable, and the rail renders only on signed-in screens. The theme control recorded the same exclusion for the same reason.
```

###### Evidence

None.

###### L7

```text
No app-shell page overflows horizontally at a real phone viewport, with the rail expanded or collapsed.
```

Invariant

Adapter

crates\/foundry\-app\/static\/css\/foundry\.4c73f62c\.css

###### Treatment

Finite

###### Cases

```text
dashboard at 390 by 844, rail expanded: no horizontal overflow
```

```text
dashboard at 390 by 844, rail collapsed: no horizontal overflow
```

```text
board at 390 by 844, rail collapsed: no page overflow, and the columns strip still scrolls within its own container
```

```text
The authority bound is one breakpoint, so the domain is the two states crossed with the surfaces the shipped mobile harness already drives.
```

###### Evidence

None.

###### L8

```text
Every script tag in the document shell that must run before the browser is permitted to paint is render-blocking: none of them carries defer, async or type=module.
```

Invariant

Infrastructure

xtask\/src\/check\_arch\.rs

###### Treatment

Finite

###### Cases

```text
the shipped shell passes
```

```text
a pre-paint tag given defer, async or type=module is refused, naming the tag and the flash it would reintroduce
```

```text
The law governs a set, not a pair: it must still refuse a third pre-paint script added later, which is why it lives in the gate rather than in a scenario about one tag.
```

###### Evidence

None.

##### Refuted

```text
A document that has loaded and whose own control has not been pressed shows the last choice made in any document of this browser.
```

```text
Law scope, not the carrier: the guard named the wrong fact. Whether a document agrees with the key is an ORDERING fact, whether it read the key more recently than the last press that rewrote it, and nothing in the product promises that a press reaches a document that is already open. The guard was restated over that ordering and the two-tab divergence became an admitted state.
```

```text
Document A loads with no key under foundry.nav, then the operator presses the control in document B. The key holds the sentinel and A still shows the expanded rail, while A own control has never been pressed.
```

##### Claims

###### C1

```text
The document can never carry a collapse marker that selects a third rail geometry.
```

```text
A stray marker value would match no rule, leaving a rail that is neither collapsed nor expanded and a content box of unpredictable width, which no scenario would name and no operator could repair.
```

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

```text
any string of any length under localStorage key foundry.nav, or no key at all
```

applyMarker

the stylesheet rules that select on \[data\-nav\-collapsed\]

```text
the rail and content widths at a fixed viewport
```

```text
JavaScript has no static checker, so this is centralised construction rather than a proved predicate: the state is a boolean and the attribute is valueless, and the SELECTOR matches on presence, so even an outside script, an extension or devtools writing a value still selects one of exactly two geometries. The bypass that would reopen it is a later edit that rewrites the selector to match a VALUE; the pre-paint script law does not cover that, so it stays a review obligation on the stylesheet.
```

###### C2

```text
The feature never stores a word for the expanded state.
```

```text
Two stored forms for one state would make an operator who expanded indistinguishable from a browser that never touched the control, and the pre-feature default would stop being the absence of a key, which is exactly what the no-JavaScript promise rests on.
```

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

```text
the boolean the control holds
```

sentinel

interpret

```text
the rail on the next page load in the same browser
```

```text
An outside writer can still leave any value in the key. interpret is total over strings, so such a value reads as expanded; the design admits that state rather than refusing it, and the storage key agreement rows record it.
```

###### C3

```text
No control for the nav exists in a document where scripting never ran.
```

```text
A server-rendered toggle would be present but dead with scripting off, and a dead control is worse than no control: the operator presses it and the page does nothing.
```

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

```text
a document of the app shell, with or without scripting
```

buildToggle

the rendered document the operator reads

```text
the count of nav controls in the document on a scripting-disabled session, which must be zero while the rail renders expanded
```

```text
The template contains no button, so there is no server-side path to a dead control. A later change that moves the control into partials/sidebar.html would reopen it; no gate covers that, so it stays a review obligation on the template.
```

##### States

Admitted

```text
Expanded with no key: every operator who has never touched the control, and every browser with scripting off.
```

None.

Admitted

```text
Collapsed with the sentinel in the key: the rail is a control strip and the content box holds the width the rail gave up.
```

None.

Admitted

```text
A foreign value in the key, left by another tool, an extension or a later version: read as expanded, not refused.
```

solution architect

Admitted

```text
Site storage refused for the origin: the control still moves the rail in this document and the choice does not survive the next load.
```

solution architect

Admitted

```text
Two tabs of one browser disagreeing: the tab that was already open keeps its geometry until it reloads or navigates.
```

product owner

Excluded

```text
A third rail geometry: the marker is valueless and the selector matches on presence.
```

None.

Excluded

```text
A stored word for the expanded state: the expanded state removes the key.
```

None.

Excluded

```text
An invisible but focusable nav link in the collapsed rail: the hidden items take display none, so they leave the focus order and the accessibility tree together.
```

None.

Excluded

```text
A control present with scripting off: the button exists only in script.
```

None.

Excluded

```text
A collapsed desktop rail width applied inside the phone top-bar layout: the phone block re-asserts the bar geometry for the collapsed selector.
```

None.

Unknown

```text
Whether the first painted frame of a collapsed operator page is ever the expanded rail. No CDP surface is used, so the first frame pixels are not observable; the render-blocking tag shape and the paint-timing measurement are the available evidence.
```

solution architect

Unknown

```text
Whether an operator with two tabs open expects the second tab to follow the first immediately. Out of the promised scope today; a storage event listener would be the change.
```

product owner

Unknown

```text
Whether hiding the rail navigation links while collapsed, rather than showing an icon-only rail, matches what the operator wants. The shipped items are text-only anchors with no icons, so an icon rail is not available without new iconography.
```

product owner

##### Models

Quint

docs\/feature\/nav\-collapse\-toggle\/design\/nav\_collapse\_choice\.qnt

###### Properties

railShowsThisDocumentsOwnChoice

Invariant

```text
The rail of each document equals that document last asked-for state, unconditionally, so a press moves this rail whatever storage does and a press in the other document never moves it.
```

applyMarker

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

aSyncedDocumentShowsTheLastChoice

Invariant

```text
A document that read the key more recently than the last press shows the last choice made in either document, while the stored value is still the one this feature wrote.
```

interpret

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

storageHoldsTheLastPress

Invariant

```text
The key holds the canonical form of the last press, and no document load rewrites it.
```

sentinel

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

neverCollapsed

Invariant

```text
Refutation witness, expected to fail: the collapsed rail is reachable from the admitted initial state through the operator own press.
```

buildToggle

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

neverReexpanded

Invariant

```text
Refutation witness, expected to fail: collapsing and then expanding again returns the rail to its starting geometry, which is the expand-again promise.
```

buildToggle

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

###### Atoms

Constant

COLLAPSED

STORAGE\_VALUE

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

None.

State

stored

window\.localStorage item foundry\.nav

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

None.

State

collapsedA

the collapsed boolean of the script closure\, mirrored by data\-nav\-collapsed

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

None.

State

collapsedB

the same boolean in a second document of the same origin

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

None.

State

storageUsable

the guarded try block around every localStorage access

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

None.

Action

loadA

the head script top\-level run on a new document

crates\/foundry\-app\/templates\/base\.html

```text
One document load. The read, the decision and the attribute stamp all happen inside the render-blocking script, so a browser never observes the document between them.
```

Action

pressA

the toggle click handler

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

```text
One operator event. The flip, the attribute stamp, the storage write and the label repaint run in one task; a refused write does not undo the three others.
```

Action

refuseStorage

the catch arm of the guarded localStorage access

crates\/foundry\-app\/static\/js\/nav\-collapse\.js

None.

Action

outsideWrite

model\-only\: any writer of the key that is not this feature

no runtime symbol in this repository

None.

State

syncedA

model\-only bookkeeping\: whether this document read the key more recently than the last press

no runtime symbol in this repository

None.

State

trusted

model\-only bookkeeping\: whether the stored value is still the one this feature wrote

no runtime symbol in this repository

None.

###### Assumptions

```text
Two documents of one browser origin, which is the smallest shape that has two actors on one storage key.
```

```text
One shared storage key, with no cross-document notification between the two documents.
```

```text
Paint ordering is outside the model: no step of it says anything about when the browser is permitted to paint.
```

```text
A refused storage origin refuses reads and writes together, as measured for the theme control on chromedriver 151.
```

###### Mutants

######## N1

```text
A document load shows the expanded rail whatever the key holds, so the read side of persistence is gone.
```

###### Kills

aSyncedDocumentShowsTheLastChoice

######## N2

```text
A press is remembered but does not stamp the document, so the rail only moves on the next load.
```

###### Kills

railShowsThisDocumentsOwnChoice

######## N3

```text
A press moves the rail but never writes the key, so nothing is remembered.
```

###### Kills

storageHoldsTheLastPress

##### Closure

```text
the collapse choice as it travels between the storage key and the documents that read it
```

Safety

Pass

DesignedNotBuilt

```text
Three invariants verified exhaustively by Apalache through quint verify at six steps over two documents and six actions, each with its own mutant counterexample, plus two construction claims that make a third rail geometry and a stored word for expanded unrepresentable rather than checked. No production byte of the feature exists yet, so nothing is observed at runtime.
```

###### Evidence

docs\/feature\/nav\-collapse\-toggle\/design\/nav\_collapse\_choice\.qnt

docs\/feature\/nav\-collapse\-toggle\/design\/verification\-receipt\.md

```text
both rail states, and the return trip between them
```

Inhabitation

Pass

DesignedNotBuilt

```text
Two witnesses were refuted on the clean model, which is the evidence: neverCollapsed fell to Apalache at four steps, so the collapsed rail is reachable from the admitted initial state; neverReexpanded fell to both the simulator and Apalache, so collapsing and expanding again returns the rail to its start.
```

###### Evidence

docs\/feature\/nav\-collapse\-toggle\/design\/verification\-receipt\.md

```text
the path from the operator press to an observation at the public driving port
```

PublicReachability

Pass

DesignedNotBuilt

```text
HTTP GET on an authenticated app-shell page in a real browser reaches crates/foundry-app/templates/base.html, which loads the new crates/foundry-app/static/js/nav-collapse.js; buildToggle appends the button into the existing .sidebar__brand of crates/foundry-app/templates/partials/sidebar.html; the click handler calls applyMarker and sentinel; the collapsed rules in the renamed stylesheet consume the marker; and the rail and content widths are read from the live browser. Every site is a concrete repository path and an existing mount point. Acyclic and buildable; nothing is implemented yet.
```

###### Evidence

None.

```text
progress of the collapse choice
```

Liveness

Pass

DesignedNotBuilt

```text
No liveness claim: every step is the operator own synchronous DOM event or a document load, the toggle is enabled in both states, and no step waits on another process scheduling, a timer, a queue or a remote party. Bounded synchronous execution carries this axis, for liveness only; the safety, refinement and reachability axes above rest on executed checks, not on it.
```

###### Evidence

None.

##### Risks

###### Y1

Stressor

```text
Negates the assumption that site storage is available: the origin refuses localStorage, so both the read and the write throw.
```

```text
The page renders, the rail works in this document, nothing is reported to the operator.
```

```text
The choice does not survive the next load; every page opens expanded.
```

```text
An unguarded first read that kills the script at parse time, which would take the control down on every screen.
```

```text
The read guard is observable on the sign-in screen; the write guard is not observable at all, because refusing storage also blocks the session cookie and the rail renders only on signed-in screens.
```

solution architect

###### Y2

Stressor

```text
Negates the assumption that scripting runs: the operator has JavaScript off, or the asset fails to load.
```

```text
The rail renders expanded exactly as before this feature, with every link reachable.
```

```text
No collapse control exists and no choice can be made.
```

```text
A server-rendered control that is present but dead.
```

```text
None: the button exists only in script, and no template path can build one.
```

solution architect

###### Y3

Stressor

```text
Negates the assumption that the rail is on the page: the 15 pre-auth and utility screens have no rail.
```

```text
Those screens render unchanged; the marker is stamped where no rule consumes it.
```

```text
No control is built on them.
```

```text
A script error from reaching into an absent mount point.
```

```text
buildToggle must return early on a missing .sidebar__brand, the same way theme.js returns on a missing .sidebar__user.
```

solution architect

###### Y4

Stressor

```text
Negates the assumption of one document at a time: two tabs of the same browser are open and one of them presses the control.
```

```text
Each tab keeps a coherent rail, and the key holds the last press.
```

```text
The tab that was already open shows the other geometry until it reloads or navigates.
```

```text
A tab whose rail and marker disagree, or a press in one tab that resets the other mid-read.
```

```text
The divergence is an admitted product state with an open question against the product owner; a storage event listener is the change that would close it.
```

product owner

###### Y5

Stressor

```text
Negates the assumption that the stylesheet and the script ship together: a browser holds the previous stylesheet.
```

```text
The previous stylesheet lives at a DIFFERENT immutable URL, so a held copy can never serve the new rules; the renamed blob is fetched with the page.
```

```text
Nothing: the rename is what makes the two versions distinct resources.
```

```text
Editing the blob without renaming it, which pins every holding browser stale at an immutable URL.
```

```text
Decided by the shipped asset-hash layer of cargo xtask check-arch, which recomputes the digest from the committed bytes.
```

crafter

###### Y6

SinglePointOfFailure

```text
The render-blocking head script tags, now two rather than one. If either ever gains defer, async or type=module, every operator who chose that state sees the wrong state flash on every navigation, and every test that looks at the settled page still passes.
```

```text
The rail and the theme still reach the right state eventually, so no functional scenario fails.
```

```text
Nothing: the flash is the defect the whole pre-paint arrangement exists to prevent.
```

```text
A silent regression, which is what copying the seven deferred neighbours produces.
```

```text
Covered by the recorded architectural obligation over the whole set of pre-paint scripts, not by a comment and not by a scenario about one tag. The shipped flash scenario asserts a COUNT that assumes exactly one pre-paint script, so it has to be restated by its owner as this set grows; a count over that set is the brittle form of the law the obligation states properly.
```

crafter

###### Y7

Bypass

```text
An outside writer, another tool on the origin, a browser extension, devtools or a later version of this feature, writes the storage key or the document attribute directly.
```

```text
One of exactly two geometries: interpret is total over strings and the stylesheet selects on attribute presence, not on a value.
```

```text
The operator loses her own choice to whatever the other writer left.
```

```text
A rail that matches no rule, or a read that throws on an unexpected value.
```

```text
A later edit that rewrites the selector to match a VALUE would reopen it; no gate covers that, so it stays a review obligation on the stylesheet.
```

solution architect

###### Y8

Bypass

```text
The browser lane is skipped: cargo xtask ci without FOUNDRY_XTASK_INCLUDE_DOCKER=1 runs none of the needs-browser scenarios, so this whole slice can be green by absence.
```

```text
The architectural gate, the cache-control tests and the asset digests still run in the default lane.
```

```text
Nothing about this feature observable behaviour is checked in that configuration.
```

```text
Reading a green gate without Docker as evidence that the slice behaves.
```

```text
The close evidence must name the lane that actually ran; the repository already carries this hazard for every needs-browser feature.
```

host

###### Y9

Stressor

```text
Second stressor set, not used to derive the design, walked over the finished design: an operator using a screen reader collapses the rail.
```

```text
The toggle keeps naming the state and what the next press does, and stays in reading order inside the brand row.
```

```text
The navigation links leave the accessibility tree entirely while collapsed; she expands to reach them.
```

```text
Links that are invisible but still focusable and announced, which a width-only or opacity-only collapse would produce.
```

```text
The hidden items must take display none rather than a visual trick; whether hiding them beats an icon-only rail is an open product question, since the shipped items are text-only anchors with no icons.
```

product owner

###### Y10

Stressor

```text
Second stressor set: the operator collapses the rail on a window just above the phone breakpoint and then narrows or rotates to 390 px.
```

```text
No horizontal scroll in either state, which is the phone promise.
```

```text
The collapsed state stops changing the bar geometry below the breakpoint; it only removes items from the top bar.
```

```text
A narrow fixed rail width applied inside the row-direction top bar, which would widen the document.
```

```text
The phone block must re-assert width auto and flex 0 0 auto for the collapsed selector; a bare desktop rule would outrank the bar at that width.
```

crafter

###### Y11

Stressor

```text
Second stressor set: the workspace name is at the longest a workspace can carry, 24 characters, which the database caps with workspaces_name_max_24_chars, and the rail is collapsed. The oracle drives "Northwind Logistics Ltd.".
```

```text
The collapsed strip keeps its set width; the monogram never shrinks and the name is hidden.
```

```text
The name is unreachable while collapsed, like the rest of the brand text.
```

```text
The brand row widening the strip, which the shipped no-wrap name would do if it were merely clipped instead of hidden.
```

```text
The hidden name must take display none, not only overflow hidden, inside the collapsed rail. The cap means no longer name is creatable, so 24 characters is the whole stressor and not a sample of it.
```

crafter

##### Architectural tests

###### T1

```text
Every script tag in the document shell that must run before the browser is permitted to paint is render-blocking: none carries defer, async or type=module. The check governs the whole set of pre-paint scripts, so it must still refuse a third one added later, and it names in its refusal which tag it is and that the flash of the wrong state is what the attribute would reintroduce.
```

xtask\/src\/check\_arch\.rs

```text
cargo xtask check-arch
```

##### Contract tests

None.

## ADRs

- NAV\-COLLAPSE\-001 Collapse state as a boolean behind a valueless marker — Accepted
