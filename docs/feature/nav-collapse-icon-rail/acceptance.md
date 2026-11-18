# Acceptance brief: nav\-collapse\-icon\-rail

## Slices

None.

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
