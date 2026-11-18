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

None.
