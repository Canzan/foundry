# Acceptance brief: nav\-collapse\-icon\-rail

## Slices

### S1

##### Covers

P8

```text
A contributor adds a head script to the document shell whose asset reads the stored choice through a local alias and stamps the document through an element reference, lets that tag carry defer, then async, then type=module, and runs the project gate over the tree.
```

```text
The gate refuses each time with exactly one entry for that script: it names the script and the attributes the tag carries, says the verdict came from the names the asset uses - a stored state name and the document element name - and that the source cannot show when it touches them, gives both remedies, remove the attributes or split the stored-state read into its own deferred asset or record a deliberate exception, and warns against dropping the attributes to satisfy the refusal. It never claims the asset was watched reading and stamping.
```

```text
The operator keeps a first paint that already carries her remembered choice: no newly added script that paints stored state can reach the head deferred, whatever idiom it is written in.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:a\_script\_that\_names\_stored\_state\_and\_the\_document\_element\_is\_refused\_with\_one\_text

```text
An asset that names the pair only inside a comment could still be refused; that direction is observed by the names enumeration row, not here.
```

###### Obligations





##### Covers

P8

```text
A contributor runs the gate over two trees that differ only in the body of one head asset: in the first the asset really stamps the document from the stored choice, in the second it touches the same two names only after the document is settled. The tag, its attribute and the script name are the same in both.
```

```text
Both trees are refused with the same entry, word for word. The gate cannot read which of the two the contributor wrote, so it states what it decided from and leaves the choice of remedy to her.
```

```text
A contributor is never sent down the wrong repair by a gate guessing at an intent it cannot see, so the repair she makes is the one that keeps the first paint correct.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:the\_same\_refusal\_text\_serves\_a\_true\_stamper\_and\_a\_settled\_dom\_neighbour

```text
Both entries could be the same text and still omit a required clause; the clause content is asserted by the refusal row above.
```

###### Obligations



##### Covers

P8

```text
A contributor runs the gate over a head that declares, each with defer, every asset that can be built from one, two or three lines drawn from eleven kinds: four ways of naming a store, four ways of naming the document element including one that touches it only once the document is settled, two mentions that appear in a comment alone, and a line that names neither.
```

```text
Every asset whose code names a store and names the document element is refused, in whichever idiom it spells them, the alias and element-reference pair and the settled-document pair included; every asset that names only one of them, or names them only in a comment, is accepted with defer.
```

```text
The promise of a correct first paint does not depend on how a contributor happens to spell a stored-state read: a real stamp cannot be hidden from the gate by a rename or an alias.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:membership\_follows\_what\_the\_asset\_names\_in\_every\_idiom\_and\_ignores\_its\_comments

```text
An asset that builds its identifier names at run time still reads as naming neither and is asked to defer; that limit is a recorded consequence, not closed here.
```

###### Obligations




##### Covers

P8

```text
A contributor takes the asset that touches the stored state only after the document is settled, removes the deferring attribute instead of splitting the read, and runs the gate.
```

```text
The gate accepts the plain tag. From source that asset has the same shape as the two legitimate scripts that must run before paint, so refusing it would refuse them too.
```

```text
Nothing the operator sees changes; the delay a needless render-blocking script adds stays guarded by the warning in the refusal and by review, and this row records that the gate does not catch it.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:the\_settled\_dom\_neighbour\_made\_plain\_is\_accepted

```text
It would also pass if the gate accepted every plain head script whatever it names; the shape sweep asserts that a plain script naming no stored state is refused.
```

###### Obligations


##### Covers

P9

```text
A contributor edits the head so that one of the deferred neighbours quietly loses its defer while the rest keep theirs, and runs the gate.
```

```text
The gate refuses with one entry that names that one script and tells her to add defer, and it names none of the neighbours that still defer. The others still deferring earn it no pass.
```

```text
The operator keeps the paint she waits for: a script with nothing to do before paint cannot slip onto the render-blocking path behind compliant neighbours.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:a\_neighbour\_that\_quietly\_lost\_its\_defer\_is\_refused\_by\_name

```text
The entry could name the script and the remedy while saying nothing about the slower paint it costs the operator; how well it reads is judged by review.
```

###### Obligations


##### Covers

P12

```text
A contributor runs the gate over a head declaring every one of the twenty-four head-script shapes: a store named in one of two ways or not at all, the document element named or not, each loaded plain, with defer, with async and as a module.
```

```text
Every shape comes back with exactly one verdict and exactly one message. A script naming both must be plain; every other script must carry defer, so plain, async and module are each refused for it and told to add defer. No shape is answered with silence.
```

```text
Whatever shape of head script a contributor writes, she is told that it is accepted or what to change, instead of reading no answer and taking it for permission.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:every\_head\_script\_shape\_gets\_exactly\_one\_verdict\_and\_one\_message

```text
Both refusal texts could be right in content and poor in order or phrasing; readability is judged by review.
```

###### Obligations


##### Covers

P12

```text
A contributor adds a head script that paints nothing from stored state and loads it async, and again as a module, then runs the gate.
```

```text
Each is refused with one entry that names the script and tells her to add defer, rather than being answered nothing at all.
```

```text
A contributor who reaches for async or a module in the head learns it is a deliberate change to make, not a silent gap to walk through.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:a\_non\_stamping\_head\_script\_loaded\_async\_or\_as\_a\_module\_is\_told\_to\_add\_defer

```text
It would still pass if the gate also wrongly refused a correctly deferred neighbour; the shipped-head row asserts the deferred neighbours stay accepted.
```

###### Obligations



##### Covers

P8

```text
A contributor runs the gate over the repository as it stands.
```

```text
The gate passes, and its passing line still names the pre-paint-script layer. The head it ships declares two scripts that paint stored state, both plain, above seven neighbours that all defer, and several of those neighbours already name the document element without naming any store.
```

```text
The operator sees no change in the app, and a contributor can run the gate at all: it does not refuse the head that ships today.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:the\_head\_this\_repository\_ships\_is\_accepted\, with the project gate command over the repository

```text
It would still pass if the rule had stopped recognising the two scripts that paint stored state; the blind-head row asserts that case is reported.
```

###### Obligations



##### Covers

P8

```text
A contributor runs the gate over a head that declares scripts, every one of them correctly deferred, and holds no script that paints stored state at all.
```

```text
The gate reports that head once, attributing the report to the pre-paint-script layer, because a rule that has stopped seeing the family it governs would otherwise hold over nothing with the gate green.
```

```text
The promise of a correct first paint cannot quietly stop being checked: a gate that has gone blind says so instead of passing.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:a\_head\_that\_yields\_no\_pre\_paint\_script\_at\_all\_is\_reported

```text
The report could name no remedy at all and still satisfy this row; its wording is judged by review.
```

###### Obligations


##### Covers

P9

```text
The operator who chose dark navigates to a foundry screen, and the suite reads the head of the page foundry served her.
```

```text
The script that settles the theme carries none of the deferring attributes; every head script outside the small family that must run before paint carries defer, and the suite names any that does not instead of passing because one neighbour still defers; and the served head is exactly the nine scripts the promise names, in order, each with the loading it is promised, compared with a table written out independently of the reader.
```

```text
The operator never sees a light frame before her dark screen, and a second pair of eyes on the same head stops a drifted reading from agreeing with a wrong head.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature\, scenario A chosen dark screen never flashes light\: steps the theme is settled before the browser is permitted to paint and the served head declares exactly the scripts the pre\-paint promise names

```text
It would still pass if the project gate had gone blind, since this side reads only the served page; the gate-side reading of the same head is the next row.
```

###### Obligations


##### Covers

P9

```text
The gate reads the head of the document shell with its own parse, independently of the suite.
```

```text
It yields exactly the same nine scripts with the same loading attributes, in order, each compared with the same independently written table, so a reader that drifts fails on the side that drifted.
```

```text
The claim that two independent readings watch this head is observed rather than asserted; a drift that changes both readings identically is the stated limit.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs pre\_paint\_script\_tests\:\:the\_gate\_reads\_the\_shipped\_head\_as\_the\_pre\_paint\_family\_above\_its\_deferred\_neighbours

```text
It would still pass if both readings drifted identically; that limit is stated and not closed.
```

###### Obligations


##### Covers

P10

```text
A contributor opens the head of the document shell, the refusal she is shown by the gate, and the documentation of the layer that produced it, to learn which head scripts must run before paint and which must defer.
```

```text
All three agree with the head the suite asserts: they name the family that must run before paint and the neighbours that must defer, and state no count that a later script can make wrong.
```

```text
A contributor who reads before she edits is told the truth about this head, so she does not reintroduce the flash by trusting a stale sentence.
```

##### Disposition

NotTested

```text
The texts are prose. The correction removes the measurable claim - the counts - rather than replacing it with a new one, so there is nothing a test can pin that would not freeze wording a contributor should be free to improve; pinning prose is also refused by the design. Their agreement with the head is judged by the oracle review and by the final user-view examination, against the nine scripts both readers assert.
```

the design owner\, through oracle review and the final examination

### S2

##### Covers

P1

P3

P6

```text
An operator signs in at desk width with the left nav open and presses the control that narrows it, once - with no admin rights, with workspace-admin rights, with instance-admin rights, and with both.
```

```text
Every destination her rights give her - Home, Board, Settings, Keyboard shortcuts, and Integrations or Instance admin where she has them - and Sign out is in the narrow rail: drawing a glyph she can see and aim at, in the layout, visible, able to take the keyboard focus, and announced with the name of the place it opens. Nothing in the rail can take the keyboard focus while it is invisible. The strip is strictly narrower than the width the open nav ships with. The open nav she started from still shows its words with no glyph laid out beside them.
```

```text
She narrows the left nav to give the page the width and keeps every place she could go, each one drawn as an icon and still named.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature \:\: Every destination the operator had stays one press away in the narrow rail

```text
An icon draws the wrong picture for its destination. That each member draws a glyph of a size she can aim at is observed, and so is its name; WHICH picture is drawn is not, so a rail of correct names over wrong pictures still passes - the promise is the name, not the drawing.
```

###### Obligations









##### Covers

P1

```text
The same operator opens the dashboard in a browser that runs no page scripts at all - with no admin rights, with workspace-admin rights, and with instance-admin rights.
```

```text
There is no narrow rail and nothing stamped the page: the left nav is the full-width nav it has always been, and every destination her rights give her, with Sign out, is in it - visible, able to take the keyboard focus, and announced with the name of the place it opens.
```

```text
In a browser that runs no scripts she gets the whole nav, and nothing she could reach before is missing.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature \:\: With no scripting the operator gets every destination in the open nav

```text
The destinations are laid out in a different order in the open nav. The order the members are laid out in is not observed, because no promise fixes it.
```

###### Obligations


##### Covers

P4

P5

```text
With the left nav narrowed on her home page, the operator presses the Board icon in the narrow rail.
```

```text
Her project board opens, the rail is still a strip, and the rail says where she now is: exactly one member carries the current-page marking and it is Board, painted in a fill that sets it apart from every icon she is not on.
```

```text
She goes to her board from the strip and can see, in the strip, that the board is where she is.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature \:\: The operator opens her board from the narrow rail and sees it marked there

```text
The marked icon is tinted in a colour the palette never intended. That the marked member is set apart from the others is observed; the colour it is set apart with belongs to the theme and is observed there.
```

###### Obligations

None.

##### Covers

P3

```text
With the left nav narrowed, the operator walks the whole page with the Tab key, stopping on each icon in the rail.
```

```text
The walk stops on every destination her rights give her, Sign out included, and the word each icon she stops on carries is REVEALED while it holds the focus: that word's box is materially wider than the sliver the same word is while the icon does not hold the focus, and it is the name of the place that icon opens.
```

```text
She can read where an icon goes before she presses it, instead of pressing a glyph to find out.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature \:\: Walking the narrow rail from the keyboard reveals each destination\'s name

```text
The revealed word is placed where it overlaps a neighbour, or sits outside the bar at the narrowest phone width. That the word grows from a sliver into something readable is observed; where the revealed box sits and how it is styled is not, and stays an open question of this slice.
```

###### Obligations


##### Covers

P2

```text
With the left nav narrowed, the operator presses Sign out in the strip once.
```

```text
Her session is over on that one press: the sign-in screen renders, nothing was put in front of her to confirm, and the app renders no page of hers afterwards until she signs in again.
```

```text
She leaves from the strip in one press, exactly as she leaves from the open nav.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature \:\: One press of Sign out in the narrow rail ends the operator\'s session

```text
Sign out in the strip can be pressed with a pointer but cannot be reached with the keyboard. This row presses it; that it can also take the keyboard focus is observed by the reachability row.
```

###### Obligations

None.

##### Covers

P11

```text
The operator leaves the left nav narrow, or leaves it open, signs out from it in the same browser, and signs in again - once with the rights she had, and once with her instance-wide rights taken away in between.
```

```text
Signing out leaves the remembered choice exactly as she left it, so the nav she comes back to is the nav she left; and the destinations in it are the ones the rights she holds NOW give her, with the one she lost absent.
```

```text
She finds the nav as she left it at her next sign-in, showing the places she can go today.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature \:\: The nav the operator left is the nav she comes back to

```text
The choice is remembered for the browser rather than for the operator, so another person signing in on the same browser inherits her narrow rail. That is the admitted per-browser consequence, and these rows cannot tell the two apart.
```

###### Obligations



##### Covers

P7

```text
The operator opens the dashboard with the left nav narrowed on a real emulated phone 320, 390 and 480 pixels wide.
```

```text
The bar holds every destination her rights give her, with Sign out, each one named, visible and reachable, and every one of their boxes lies inside the bar; the bar is no wider than her screen and the page does not slide sideways at any of the three widths.
```

```text
On a phone she sees all of her destinations in the bar and never has to push the page sideways to reach one.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature \:\: On a phone the narrow bar shows every destination inside itself

```text
Every icon happens to fit on a single row at all three widths, so the wrapping the bar is told to do is never exercised - what is observed is that no icon leaves the bar and the page never slides, which is the promise.
```

###### Obligations




##### Covers

P11

```text
An earlier visit left the narrow nav remembered in her browser; now her browser runs no page scripts, she signs out of the left nav she gets there, and signs in again.
```

```text
There is no narrow rail and nothing stamped the page, because no script ran to stamp it; the open nav carries every destination her rights give her with Sign out; and what the browser remembers is byte-for-byte the choice she left.
```

```text
A visit in a browser that runs no scripts gives her the open nav and throws away nothing she chose earlier.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature \:\: Signing out with no scripting leaves the choice she made earlier untouched

```text
Signing out clears the choice only in a browser that does run scripts. This row observes the no-script visit; the scripting-on arm of the same law is observed by the row above it.
```

###### Obligations


##### Covers

P8

```text
A contributor writes a head script that reaches for stored state and for the document element in any idiom - a direct read, a bracket read, a local alias, a destructured read, an element reference, a dataset stamp - or that only mentions those names inside a comment, and runs the project gate.
```

```text
The gate recognises what the asset NAMES in every one of those idioms and refuses it when it defers; it does not recognise an asset that merely mentions the names in a comment; and it recognises an asset that names both and touches them only once the page has settled, refusing that one loudly rather than accepting it in silence.
```

```text
No way of writing a script that stamps the page from stored state slips past the gate, so the flash of the wrong rail does not come back.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs\:\:membership\_follows\_what\_the\_asset\_names\_in\_every\_idiom\_and\_ignores\_its\_comments

```text
An asset that builds its identifiers while the page runs is classed as a non-stamper. The enumeration covers written idioms; a name assembled at runtime is outside what any test of the source can see, and the layer records that limit rather than claiming it.
```

###### Obligations


##### Covers

P1

P3

P7

```text
A contributor changes the rail's styling - the icon, the clipped word, the revealed label, the phone arm - and runs the project gate over the repository.
```

```text
Every added rule names only the palette's own tokens and no colour of its own, and the stylesheet the shell asks for still resolves: its content-hashed name is its own fingerprint, its provenance row recomputes, and every reference to it moves with its bytes.
```

```text
Her icons and the label that names them follow the palette she chose, and her page keeps its styling after the rail is restyled instead of arriving unstyled.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs \:\: the colour\-token seam and the static\-asset layers\, run by cargo xtask check\-arch

```text
A rule names a token that exists in every palette but is the wrong one for the rail, so the icon is legible and looks out of place. The layers observe that no colour is hard-coded and that the asset resolves; which token suits the rail is a review matter.
```

###### Obligations



##### Covers

P7

```text
The operator opens the dashboard with the left nav OPEN on a phone 320, 390 and 480 pixels wide.
```

```text
The nav lays out as the top bar at every one of those widths and the page does not slide sideways, exactly as it did before the rail learned to draw icons.
```

```text
On a phone her page never slides sideways, whichever way she has the nav set.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature \:\: A phone screen never scrolls sideways\, whichever way the left nav is set

```text
The bar is taller than it needs to be at the narrowest width. The promise is about the page not sliding sideways; how much height the bar spends is not observed.
```

###### Obligations




## Expectations

### Rows

#### E1

##### Promises

P1

```text
With the left nav collapsed, the operator opens Home, Board, Settings, keyboard shortcuts and every admin entry her rights grant by activating an icon in the narrow rail; no destination the expanded nav offers her is absent from the rail.
```

```text
A capture that each icon exists and opens its page does not show the operator can tell the icons apart, nor that the rail keeps the order and grouping of the expanded nav; it cannot see destinations that appear only for a rights combination the capture never signs in with.
```

yes

None.

```text
Public probe of the collapsed rail for at least one plain operator and one operator with workspace or instance admin rights, listing the reachable destinations in both nav states.
```

#### E2

##### Promises

P5

```text
From the collapsed rail on the home page, activating the Board icon opens the board, the left nav is still collapsed afterwards, and the Board icon is the one marked as the page the operator is on.
```

```text
One Home-to-Board step does not show the other icons behave the same way, that the collapsed state survives a full page reload rather than only an in-page navigation, or that the Home icon loses its current mark.
```

yes

None.

None.

#### E3

##### Promises

P2

```text
With the nav collapsed, one press on the rail sign-out icon signs the operator out and leaves her where sign-out from the expanded nav leaves her; she needs no second press, menu, confirmation or expand step.
```

```text
Observing that the press ends the session does not show the operator recognises that icon as sign-out rather than a destination, and it does not cover a rail rendered for other rights where sign-out may sit elsewhere.
```

yes

None.

None.

#### E4

##### Promises

P3

```text
Every icon in the collapsed rail carries the name of its destination: a screen-reader operator hears that name, and a pointer operator sees the same name when she hovers or focuses the icon.
```

```text
An attribute-level capture cannot show the visible name appears inside the viewport and within reading time, that the heard name matches the wording of the expanded nav, or that the name is announced once instead of twice.
```

yes

None.

```text
Captured accessible names for every rail icon plus an observation of the visible name on hover or keyboard focus.
```

#### E5

##### Promises

P4

```text
In the collapsed rail the icon for the page the operator is on is marked as current in the same way the expanded nav marks it, and no other icon is marked.
```

```text
A current-marker capture does not show the mark is perceivable without colour alone, and it does not cover sub-pages or unknown routes where no top-level destination is the current one.
```

yes

None.

None.

#### E6

##### Promises

P6

```text
Collapsing the left nav takes nothing away: Home, Board, Settings, keyboard shortcuts, the admin destinations and Sign out are all still reachable as rail icons, and no link the collapse hides can still be reached with Tab or announced by a screen reader.
```

```text
A before-and-after capture for one operator at one width cannot establish that absence for every rights set and width, nor that nothing stays reachable while the collapse is still animating; markup-level checks do not show how a real assistive technology behaves.
```

yes

None.

```text
Focus-order and accessible-tree captures of the collapsed nav, not only element visibility.
```

#### E7

##### Promises

P7

```text
At 480 px wide and narrower, the page with the collapsed icon bar shows no horizontal scrollbar and the operator needs no sideways scrolling to reach anything on it.
```

```text
A no-overflow capture at one phone width does not show the icons stay legible and large enough to tap, and it does not cover other narrow widths, a long workspace name, or an enlarged browser font.
```

yes

None.

None.

#### E8

##### Promises

P8

```text
A contributor who adds or changes a head script that stamps the document from stored state and lets it defer is refused by the project gate, and the refusal names the script; the refusal comes whichever stored-state spelling the script uses, localStorage or sessionStorage.
```

```text
Refusals for the spellings the gate enumerates cannot show it refuses a stored-state access it was never shown, such as an aliased or computed accessor; and a gate refusal is no evidence that the flash itself is gone in a real browser.
```

yes

None.

```text
Observed gate runs over head scripts the examiner supplies, one per stored-state spelling, each showing the refusal text.
```

#### E9

##### Promises

P9

```text
When any head script that is not one of the declared pre-paint scripts loses its defer, the acceptance suite fails and names that script; the suite does not pass because another neighbour still defers.
```

```text
Mutating one neighbour shows a failure for that neighbour only; it does not show the suite names each of the other neighbours, nor that it also fails when a pre-paint script wrongly gains defer.
```

yes

None.

```text
A negative control: the same suite run against a head where one non-pre-paint script has lost its defer, with the failure text captured.
```

#### E10

##### Promises

P10

```text
The head comments, the refusal text of the gate and the project documentation tell the contributor the same, correct answer about which head scripts must run before paint and which must defer; none of the three states a count or a name that the head the suite asserts contradicts.
```

```text
Agreement captured at one moment does not show the three texts stay in step after the next head change, and matching counts do not show the prose names the right scripts for the right reason or that a contributor reads it correctly.
```

yes

None.

None.

#### E11

##### Promises

P11

```text
An operator who collapses the left nav, signs out from the rail and signs in again in the same browser finds the nav still collapsed, with rail icons for the destinations her current rights give her.
```

```text
One sign-out and sign-in cycle in one browser cannot show the choice survives a rights change between sessions, another browser or profile, cleared storage, or a second account in the same browser; it also does not show the collapsed state is in place before first paint instead of after a visible jump.
```

yes

None.

```text
A probe that signs out from the collapsed rail and signs in again in the same browser session, capturing the nav state and the icon list after sign-in.
```

#### E12

##### Promises

P12

```text
A head script that does not stamp stored state and is loaded async or as type=module gets one definite answer from the gate: it is refused and told to add defer, never left without a verdict.
```

```text
A verdict for the async and type=module shapes does not show every other loading shape gets one, and the advice to add defer is not shown to be right for a script for which defer is wrong.
```

yes

None.

```text
Observed gate runs over an async head script and a type=module head script that the examiner supplies, each with the printed verdict.
```

## Charter

None.

## Examination

### E1

met

#### Observations

```text
Collapsed rail, same session, after one click on 'Left nav: expanded. Collapse the left nav.': the sidebar narrows from 240 px to 'complementary x=0 w=72 y=0 h=900 visible' and the control's name becomes 'Left nav: collapsed. Expand the left nav.' expanded=false. 'tab-order 40' walks the whole page and the rail stops are, in order: Tab 1 link "Home" current=page, Tab 2 link "Board", Tab 3 link "Settings", Tab 4 link "Keyboard shortcuts", Tab 5 link "Integrations", Tab 6 link "Instance admin", Tab 7 button "Sign out", Tab 8 the theme button - each reported 'at x=16 w=40 visible' (the theme button x=20 w=30) with a focus ring. Tabs 9-17 are the main-content links of the dashboard; Tab 18 is 'on the page itself' and Tab 19 starts the cycle again. Every one of the 40 stops is reported visible: no stop is an invisible or off-layout link. The accessible name of each rail stop is the destination word itself ('Home', 'Board', 'Settings', 'Keyboard shortcuts', 'Integrations', 'Instance admin', 'Sign out'); none is empty and none is a glyph.
```

```text
Rights variation, viewer Lena (member of Canzan Labs, lead of team Backend, neither kind of admin), fresh browser profile, 1440x900. Expanded: 'complementary x=0 w=240', 'other links on the page: 7'. After one click on the collapse control: 'complementary x=0 w=72 y=0 h=900 visible'. 'tab-order 40' rail stops: Tab 1 link "Home" current=page, Tab 2 link "Board", Tab 3 link "Settings", Tab 4 link "Keyboard shortcuts", Tab 5 button "Sign out", Tab 6 the theme button - all 'at x=16 w=40 visible' with their labels revealed on focus (54x27, 54x27, 69x27, 136x27, 68x27). Integrations and Instance admin are absent from her rail, which matches the rights the state gives her, and no stop in her 40 is invisible.
```

```text
Navigating from the collapsed rail (same session). 'click "Board"' reports 'clicked link "Board" at 36,174 / the page changed: /team/backend/project/identity-platform (status 200)'. The look on the board page reports 'complementary x=0 w=72 ... visible' (still the strip), '[Primary] link "Board" current=page' and '[Primary] link "Home"' with no current mark - exactly one rail item carries current=page, and it is the page she is on. 'reload' then re-fetches /team/backend/project/identity-platform (status 200) and the look after it is identical: rail still w=72, Board still current=page, 'horizontal scroll: no'.
```

```text
Expanded baseline, real browser 1440x900, state slack-connected, viewer Priya (instance admin + Canzan Labs workspace admin). After signing in through the real form the page is / and the look reports: 'complementary x=0 w=240 y=0 h=900 visible', 'navigation "Primary" x=12 w=215 y=78 h=72 visible', 'button "Left nav: expanded. Collapse the left nav." expanded=true focusable=true', sidebar 'button "Sign out" x=20 w=76 visible', nav links 'Home' current=page and 'Board' with their words painted and seen, 'other links on the page: 12', 'horizontal scroll: no'.
```

### E2

met

#### Observations

```text
Navigating from the collapsed rail (same session). 'click "Board"' reports 'clicked link "Board" at 36,174 / the page changed: /team/backend/project/identity-platform (status 200)'. The look on the board page reports 'complementary x=0 w=72 ... visible' (still the strip), '[Primary] link "Board" current=page' and '[Primary] link "Home"' with no current mark - exactly one rail item carries current=page, and it is the page she is on. 'reload' then re-fetches /team/backend/project/identity-platform (status 200) and the look after it is identical: rail still w=72, Board still current=page, 'horizontal scroll: no'.
```

```text
Collapsed rail, same session, after one click on 'Left nav: expanded. Collapse the left nav.': the sidebar narrows from 240 px to 'complementary x=0 w=72 y=0 h=900 visible' and the control's name becomes 'Left nav: collapsed. Expand the left nav.' expanded=false. 'tab-order 40' walks the whole page and the rail stops are, in order: Tab 1 link "Home" current=page, Tab 2 link "Board", Tab 3 link "Settings", Tab 4 link "Keyboard shortcuts", Tab 5 link "Integrations", Tab 6 link "Instance admin", Tab 7 button "Sign out", Tab 8 the theme button - each reported 'at x=16 w=40 visible' (the theme button x=20 w=30) with a focus ring. Tabs 9-17 are the main-content links of the dashboard; Tab 18 is 'on the page itself' and Tab 19 starts the cycle again. Every one of the 40 stops is reported visible: no stop is an invisible or off-layout link. The accessible name of each rail stop is the destination word itself ('Home', 'Board', 'Settings', 'Keyboard shortcuts', 'Integrations', 'Instance admin', 'Sign out'); none is empty and none is a glyph.
```

### E3

met

#### Observations

```text
Sign out from the collapsed rail and the next sign-in (same session). 'click "Sign out"' on the rail button reports 'clicked button "Sign out" at 36,820 / the page changed: /sign-in (status 200)' - one press, no confirmation step and no second press, and the /sign-in look has no app shell, only the 'Sign in' button. 'sign-in' in the same browser then returns to / and the look reports 'complementary x=0 w=72 y=0 h=900 visible', the control named 'Left nav: collapsed. Expand the left nav.' expanded=false, the rail 'button "Sign out" x=16 w=40 visible' and '[Primary] link "Home" current=page' - the nav is collapsed, as she left it, with the same rail structure as before sign-out.
```

```text
Collapsed rail, same session, after one click on 'Left nav: expanded. Collapse the left nav.': the sidebar narrows from 240 px to 'complementary x=0 w=72 y=0 h=900 visible' and the control's name becomes 'Left nav: collapsed. Expand the left nav.' expanded=false. 'tab-order 40' walks the whole page and the rail stops are, in order: Tab 1 link "Home" current=page, Tab 2 link "Board", Tab 3 link "Settings", Tab 4 link "Keyboard shortcuts", Tab 5 link "Integrations", Tab 6 link "Instance admin", Tab 7 button "Sign out", Tab 8 the theme button - each reported 'at x=16 w=40 visible' (the theme button x=20 w=30) with a focus ring. Tabs 9-17 are the main-content links of the dashboard; Tab 18 is 'on the page itself' and Tab 19 starts the cycle again. Every one of the 40 stops is reported visible: no stop is an invisible or off-layout link. The accessible name of each rail stop is the destination word itself ('Home', 'Board', 'Settings', 'Keyboard shortcuts', 'Integrations', 'Instance admin', 'Sign out'); none is empty and none is a glyph.
```

### E4

met

#### Observations

```text
Collapsed rail, same session, after one click on 'Left nav: expanded. Collapse the left nav.': the sidebar narrows from 240 px to 'complementary x=0 w=72 y=0 h=900 visible' and the control's name becomes 'Left nav: collapsed. Expand the left nav.' expanded=false. 'tab-order 40' walks the whole page and the rail stops are, in order: Tab 1 link "Home" current=page, Tab 2 link "Board", Tab 3 link "Settings", Tab 4 link "Keyboard shortcuts", Tab 5 link "Integrations", Tab 6 link "Instance admin", Tab 7 button "Sign out", Tab 8 the theme button - each reported 'at x=16 w=40 visible' (the theme button x=20 w=30) with a focus ring. Tabs 9-17 are the main-content links of the dashboard; Tab 18 is 'on the page itself' and Tab 19 starts the cycle again. Every one of the 40 stops is reported visible: no stop is an invisible or off-layout link. The accessible name of each rail stop is the destination word itself ('Home', 'Board', 'Settings', 'Keyboard shortcuts', 'Integrations', 'Instance admin', 'Sign out'); none is empty and none is a glyph.
```

```text
Label reveal in the collapsed rail (same session). With nothing focused or hovered, each rail label's painted box is 1 px: 'link "Home" ... words painted: "Home" x=64 w=1 h=1 not seen at its centre (clipped or covered)', same for 'Board'. On keyboard focus the same label becomes 'words painted: "Home" x=64 w=54 h=27 seen' (Board 54x27, Settings 69x27, Keyboard shortcuts 136x27, Integrations 91x27, Instance admin 110x27, Sign out 68x27). On 'hover Board' the pointer gets the same reveal: 'pointer on link "Board" at 36,174 / words painted: "Board" x=64 w=54 h=27 seen', and the look taken while hovering shows Board's label seen while Home's is still 1x1. The revealed boxes are materially larger than the 1x1 unfocused box, and each reveal stays inside the 1440 px page with 'horizontal scroll: no'.
```

```text
Rights variation, viewer Lena (member of Canzan Labs, lead of team Backend, neither kind of admin), fresh browser profile, 1440x900. Expanded: 'complementary x=0 w=240', 'other links on the page: 7'. After one click on the collapse control: 'complementary x=0 w=72 y=0 h=900 visible'. 'tab-order 40' rail stops: Tab 1 link "Home" current=page, Tab 2 link "Board", Tab 3 link "Settings", Tab 4 link "Keyboard shortcuts", Tab 5 button "Sign out", Tab 6 the theme button - all 'at x=16 w=40 visible' with their labels revealed on focus (54x27, 54x27, 69x27, 136x27, 68x27). Integrations and Instance admin are absent from her rail, which matches the rights the state gives her, and no stop in her 40 is invisible.
```

### E5

met

#### Observations

```text
Navigating from the collapsed rail (same session). 'click "Board"' reports 'clicked link "Board" at 36,174 / the page changed: /team/backend/project/identity-platform (status 200)'. The look on the board page reports 'complementary x=0 w=72 ... visible' (still the strip), '[Primary] link "Board" current=page' and '[Primary] link "Home"' with no current mark - exactly one rail item carries current=page, and it is the page she is on. 'reload' then re-fetches /team/backend/project/identity-platform (status 200) and the look after it is identical: rail still w=72, Board still current=page, 'horizontal scroll: no'.
```

```text
Collapsed rail, same session, after one click on 'Left nav: expanded. Collapse the left nav.': the sidebar narrows from 240 px to 'complementary x=0 w=72 y=0 h=900 visible' and the control's name becomes 'Left nav: collapsed. Expand the left nav.' expanded=false. 'tab-order 40' walks the whole page and the rail stops are, in order: Tab 1 link "Home" current=page, Tab 2 link "Board", Tab 3 link "Settings", Tab 4 link "Keyboard shortcuts", Tab 5 link "Integrations", Tab 6 link "Instance admin", Tab 7 button "Sign out", Tab 8 the theme button - each reported 'at x=16 w=40 visible' (the theme button x=20 w=30) with a focus ring. Tabs 9-17 are the main-content links of the dashboard; Tab 18 is 'on the page itself' and Tab 19 starts the cycle again. Every one of the 40 stops is reported visible: no stop is an invisible or off-layout link. The accessible name of each rail stop is the destination word itself ('Home', 'Board', 'Settings', 'Keyboard shortcuts', 'Integrations', 'Instance admin', 'Sign out'); none is empty and none is a glyph.
```

### E6

met

#### Observations

```text
Expanded baseline, real browser 1440x900, state slack-connected, viewer Priya (instance admin + Canzan Labs workspace admin). After signing in through the real form the page is / and the look reports: 'complementary x=0 w=240 y=0 h=900 visible', 'navigation "Primary" x=12 w=215 y=78 h=72 visible', 'button "Left nav: expanded. Collapse the left nav." expanded=true focusable=true', sidebar 'button "Sign out" x=20 w=76 visible', nav links 'Home' current=page and 'Board' with their words painted and seen, 'other links on the page: 12', 'horizontal scroll: no'.
```

```text
Collapsed rail, same session, after one click on 'Left nav: expanded. Collapse the left nav.': the sidebar narrows from 240 px to 'complementary x=0 w=72 y=0 h=900 visible' and the control's name becomes 'Left nav: collapsed. Expand the left nav.' expanded=false. 'tab-order 40' walks the whole page and the rail stops are, in order: Tab 1 link "Home" current=page, Tab 2 link "Board", Tab 3 link "Settings", Tab 4 link "Keyboard shortcuts", Tab 5 link "Integrations", Tab 6 link "Instance admin", Tab 7 button "Sign out", Tab 8 the theme button - each reported 'at x=16 w=40 visible' (the theme button x=20 w=30) with a focus ring. Tabs 9-17 are the main-content links of the dashboard; Tab 18 is 'on the page itself' and Tab 19 starts the cycle again. Every one of the 40 stops is reported visible: no stop is an invisible or off-layout link. The accessible name of each rail stop is the destination word itself ('Home', 'Board', 'Settings', 'Keyboard shortcuts', 'Integrations', 'Instance admin', 'Sign out'); none is empty and none is a glyph.
```

```text
Rights variation, viewer Lena (member of Canzan Labs, lead of team Backend, neither kind of admin), fresh browser profile, 1440x900. Expanded: 'complementary x=0 w=240', 'other links on the page: 7'. After one click on the collapse control: 'complementary x=0 w=72 y=0 h=900 visible'. 'tab-order 40' rail stops: Tab 1 link "Home" current=page, Tab 2 link "Board", Tab 3 link "Settings", Tab 4 link "Keyboard shortcuts", Tab 5 button "Sign out", Tab 6 the theme button - all 'at x=16 w=40 visible' with their labels revealed on focus (54x27, 54x27, 69x27, 136x27, 68x27). Integrations and Instance admin are absent from her rail, which matches the rights the state gives her, and no stop in her 40 is invisible.
```

### E7

met

#### Observations

```text
Phone widths with the icons shown (same session, rail collapsed). 'width 480' then look: 'viewport 480x900; page 480 px wide; horizontal scroll: no', 'boxes that scroll sideways inside the page: none', 'boxes that reach past the right edge (a clipped box is listed too): none'; the sidebar has become a top bar, 'complementary x=0 w=480 y=0 h=73 visible', and the nav items sit side by side in it (Home x=104 w=40, Board x=146 w=40, rail Sign out x=370 w=40, theme x=414 w=30). 'width 390' then look: 'viewport 390x900; page 390 px wide; horizontal scroll: no', same two 'none' lines, and the bar is now 'complementary x=0 w=390 y=0 h=125 visible' - taller because the icons wrap inside it rather than overflow.
```

### E8

met

#### Observations

```text
E8 sessionStorage spelling. I staged the shipped head unchanged and replaced the bytes of the deferred neighbour crates/foundry-app/static/js/form-errors.js with my own asset that reads window.sessionStorage.getItem('foundry.nav') and calls document.documentElement.setAttribute('data-nav-collapsed',''). Gate output: 'check-arch: boundary guard FAILED - 1 violation(s)' and one entry 'pre-paint-script: crates/foundry-app/templates/base.html loads `/static/js/form-errors.js` with defer, and the gate decided that BY NAME: the comment-stripped source of that asset NAMES a browser store (`localStorage` or `sessionStorage`) and NAMES `documentElement`.' The entry then gives CASE ONE (remove defer, plain external script in head is render-blocking by specification), CASE TWO (split the stored-state read into its own deferred asset or record a deliberate exception) and the warning 'In CASE TWO, do not drop defer to satisfy this refusal'. It never claims the asset was observed to read or stamp. '== check-arch exit 1'.
```

```text
E8 localStorage spelling through a local alias and an element reference. I replaced the bytes of the deferred neighbour crates/foundry-app/static/js/keyboard.js with an asset whose body is 'var store = window.localStorage, root = document.documentElement; var v = store.getItem("foundry.nav"); if (v === "collapsed") { root.setAttribute("data-nav-collapsed", ""); }'. Gate output: one violation, 'pre-paint-script: ... loads `/static/js/keyboard.js` with defer, and the gate decided that BY NAME ...', exit 1. The refusal text is word-for-word the text of the sessionStorage case apart from the src and the attribute it names, so the two spellings and the aliased idiom are one input class with one message.
```

```text
Gate baseline, repository as it stands: 'nwdes examine probe -- examine-foundry gate' printed 'check-arch: boundary guard PASSED (...)' and '== check-arch exit 0'. The PASSED line names the pre-paint layer in these words: 'every head script of the document shell whose asset names a browser store and names documentElement is render-blocking - none carries defer, async or type=module - and every other head script of that shell carries defer, so none of them sits on the render-blocking path for nothing (pre-paint-script)'.
```

```text
E8 limitation I probed deliberately: an asset that builds the identifiers at run time, 'var k = "local" + "Storage", e = "document" + "Element"; window[k].getItem("foundry.nav"); document[e].setAttribute("data-nav-collapsed", "")', staged as the deferred neighbour crates/foundry-app/static/js/csrf-upload.js, was ACCEPTED: 'boundary guard PASSED', '== check-arch exit 0'. The gate's own refusal text states this limit - 'the NAMES in the asset's source - identifiers built at run time are invisible to it' - so it is a declared blind spot, not a silent one.
```

### E9

not_examined

```text
P9 is about the acceptance SUITE and 'examine-foundry suite' takes a tag. Over the feature's own tag, a head with the neighbour /static/js/board-live.js made plain passed 17/17 and a head with defer added to /static/js/theme.js also passed 17/17 - expected, because none of those 17 scenarios is about the document head. The tag whose feature holds the head-asserting scenario, canzan-theme-system ('A chosen dark screen never flashes light', canzan-theme-system.feature:352), collapsed on Selenium session timeouts ('Unable to execute request for an existing session: java.util.concurrent.TimeoutException', then 'Cannot find session with id') and printed no [Summary], so I observed neither a failure naming board-live.js nor a pass. Evidence-capture owner: the examiner's probe environment; prerequisite is one 'examine-foundry suite canzan-theme-system --file crates/foundry-app/templates/base.html <head with one neighbour plain>' run on a browser grid that holds its sessions, with no other probe queued on the examine lock. See observations O19, O20, O21; the project gate does refuse this same mutation (O8), which is not evidence about the suite.
```

### E10

met

#### Observations

```text
Read of crates/foundry-app/templates/base.html (public projection): the head carries exactly two plain external scripts, /static/js/theme.js and /static/js/nav-collapse.js, above seven deferred tags (htmx.min.js, board-dnd.js, board-lane-dnd.js, board-live.js, csrf-upload.js, keyboard.js, form-errors.js), i.e. nine (src, deferring) pairs. The comment above theme.js states it is 'A MEMBER OF THE PRE-PAINT FAMILY OF THIS HEAD: IT MUST NOT DEFER (the other member today is the nav-collapse tag below it, added 2026-11-16)' and that the deferred neighbours 'are correct as they are'. The comment above nav-collapse.js states the gate 'refuses any of those three attributes on EVERY head script whose asset NAMES a browser store and NAMES documentElement ... and that refuses any OTHER head script here which does not carry defer'. Neither comment states a numeric count of scripts.
```

```text
E8 sessionStorage spelling. I staged the shipped head unchanged and replaced the bytes of the deferred neighbour crates/foundry-app/static/js/form-errors.js with my own asset that reads window.sessionStorage.getItem('foundry.nav') and calls document.documentElement.setAttribute('data-nav-collapsed',''). Gate output: 'check-arch: boundary guard FAILED - 1 violation(s)' and one entry 'pre-paint-script: crates/foundry-app/templates/base.html loads `/static/js/form-errors.js` with defer, and the gate decided that BY NAME: the comment-stripped source of that asset NAMES a browser store (`localStorage` or `sessionStorage`) and NAMES `documentElement`.' The entry then gives CASE ONE (remove defer, plain external script in head is render-blocking by specification), CASE TWO (split the stored-state read into its own deferred asset or record a deliberate exception) and the warning 'In CASE TWO, do not drop defer to satisfy this refusal'. It never claims the asset was observed to read or stamp. '== check-arch exit 1'.
```

```text
E12 async: I staged a head in which the non-stamping neighbour /static/js/keyboard.js carries 'async' instead of 'defer' (its bytes unchanged). Gate: one violation, 'pre-paint-script: ... loads `/static/js/keyboard.js` with async, and the comment-stripped source of that asset does not name BOTH a browser store ... Add `defer` to this tag ...', exit 1. One entry, one message, a definite verdict.
```

```text
Read of CHANGELOG.md (public projection), Unreleased/Fixed: 'The head-script guard now answers every shape, by what an asset names. ... The guard now reads what the asset NAMES - a browser store and documentElement - in whatever idiom it spells them, and it refuses every other head script that does not carry defer instead of answering nothing at all for async and type=module. A refusal says which attributes the tag carries, that the verdict came from the names and not from watching the asset run, and both repairs that keep the first paint correct. The comments above the two render-blocking tags now name the pre-paint family and the deferred neighbours instead of a count that went stale.' Every clause of this matches the gate output and the head I observed.
```

```text
Read of DEVELOPER.md (public projection): its contributor sections mention 'cargo xtask check-arch' three times (the gates table, 'Crate boundaries', and 'If you're unsure whether a dependency edge is allowed'), always about crate/dependency layering. DEVELOPER.md nowhere describes the pre-paint-script layer, which head scripts must run before paint, or which must defer. It states nothing that contradicts the head; it is simply silent on the rule. The public projection carries no other developer documentation of the rule; the ADR identifiers the refusal cites (ADR-NAV-COLLAPSE-001, ADR-NAV-ICON-RAIL-002, ADR-NAV-ICON-RAIL-004) are not in the public projection.
```

### E11

met

#### Observations

```text
Sign out from the collapsed rail and the next sign-in (same session). 'click "Sign out"' on the rail button reports 'clicked button "Sign out" at 36,820 / the page changed: /sign-in (status 200)' - one press, no confirmation step and no second press, and the /sign-in look has no app shell, only the 'Sign in' button. 'sign-in' in the same browser then returns to / and the look reports 'complementary x=0 w=72 y=0 h=900 visible', the control named 'Left nav: collapsed. Expand the left nav.' expanded=false, the rail 'button "Sign out" x=16 w=40 visible' and '[Primary] link "Home" current=page' - the nav is collapsed, as she left it, with the same rail structure as before sign-out.
```

```text
Second, separate browser launch on the same viewer profile (no --fresh): after 'sign-in' the first look already reports 'complementary x=0 w=72 y=0 h=900 visible' and 'Left nav: collapsed. Expand the left nav.', so the collapsed choice survived both the sign-out and the end of the previous browser session. Stepping Tab through that session reproduced the reveal: Tab 1 the collapse control, Tab 2 'link "Home" current=page ... words painted "Home" x=64 w=54 h=27 seen', Tab 3 Board 54x27 seen, Tab 4 Settings 69x27 seen.
```

### E12

met

#### Observations

```text
E12 async: I staged a head in which the non-stamping neighbour /static/js/keyboard.js carries 'async' instead of 'defer' (its bytes unchanged). Gate: one violation, 'pre-paint-script: ... loads `/static/js/keyboard.js` with async, and the comment-stripped source of that asset does not name BOTH a browser store ... Add `defer` to this tag ...', exit 1. One entry, one message, a definite verdict.
```

```text
E12 type=module: the same head with 'type="module"' on /static/js/keyboard.js. Gate: one violation, 'loads `/static/js/keyboard.js` with type="module" ... Add `defer` to this tag ... `type="module"` is deferred but needs the module machinery to say so', exit 1. Same single verdict and remedy as the async shape.
```

```text
Gate baseline, repository as it stands: 'nwdes examine probe -- examine-foundry gate' printed 'check-arch: boundary guard PASSED (...)' and '== check-arch exit 0'. The PASSED line names the pre-paint layer in these words: 'every head script of the document shell whose asset names a browser store and names documentElement is render-blocking - none carries defer, async or type=module - and every other head script of that shell carries defer, so none of them sits on the render-blocking path for nothing (pre-paint-script)'.
```
