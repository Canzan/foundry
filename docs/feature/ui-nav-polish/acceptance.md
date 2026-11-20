# Acceptance brief: ui\-nav\-polish

## Slices

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
Standing on Teams, on Invite a member, and on Machine tokens, the operator sees the nav mark that same item as the page she is on, and sees no second item marked at the same time.
```

```text
A user cannot see why an item is marked; an oracle that only confirms the expected item is marked does not show that every other nav item is unmarked, so a nav that marks two items at once still looks met.
```

None.

None.

```text
The promise does not say what 'marks' looks like to the operator, so the observation must name the visible difference (for example highlight or label) rather than an internal attribute.
```

#### E2

##### Promises

P1

```text
On a page reached below Teams, Invite a member, or Machine tokens (a detail or sub page, not the section landing page), the operator still sees the nav mark the item of the section she is in, and only that item.
```

```text
The oracle sees only the pages it chose to visit; it cannot show that an unvisited page under those three sections, or a page whose address carries extra segments or query values, still marks the right item.
```

None.

None.

```text
The promise says 'every page under them' but does not list those pages, so the observation must say which deeper pages were visited and admit the rest are unobserved.
```

#### E3

##### Promises

P2

```text
An operator whom the Invite a member and Machine tokens pages admit sees both items in the nav, and pressing each one opens that page for her instead of a refusal or an error.
```

```text
Opening a page once does not show that every link the nav offers her opens; the oracle also cannot see the pages' own admission rule, so it can agree with the nav while the nav and the pages disagree for some other operator.
```

None.

None.

```text
The promise names 'an operator whom those pages let in' without stating the admission rule, so the observation must name the operator used and how she was admitted.
```

#### E4

##### Promises

P2

```text
An operator whom the Invite a member or Machine tokens page would refuse does not see that item anywhere in the nav, at desk width and at phone width, open or folded.
```

```text
The oracle sees only the nav states it opened; a hidden item that appears in a fold, a sub menu, or at another width it did not open stays unobserved, and absence in one capture cannot prove absence for every refused operator.
```

None.

None.

```text
The promise does not say whether the item must be absent or only inactive; the observation must show absence from the operator's view, not merely a disabled look.
```

#### E5

##### Promises

P3

```text
At desk width, on a page taller than the window, the operator scrolls the page to the bottom and the whole nav, from its first item to its last, is still in view; this holds with the nav open and with the nav folded.
```

```text
A capture of the nav after one scroll on one page height cannot show that the nav stays whole on a much taller page or a shorter window, and a check of a declared style value instead of the nav's seen position after a real scroll would pass a nav that still scrolls away.
```

None.

None.

```text
The promise does not fix a desk width or a page height, so the observation must name the window size and the page used.
```

#### E6

##### Promises

P4

```text
At 480 px wide and below, the nav bar above the content takes no more than a fifth of the screen height, measured while the operator looks at a normal page.
```

```text
A measurement at one phone size and one page cannot show the share of the screen at a smaller or shorter phone, and it does not see the bar grow when the item labels wrap or a long account name widens a row.
```

None.

None.

```text
The promise does not say whether 'screen height' is the whole screen or the browser view, so the observation must state what it measured against.
```

#### E7

##### Promises

P4

```text
At 480 px wide and below, the operator reaches each nav destination with one press: every destination is visible or directly reachable in the bar without first pressing a menu, a fold, or a 'more' control.
```

```text
Counting the links in the bar does not show that the operator can press them; items pushed out of sight by horizontal overflow, or covered by the content, stay unobserved unless each one is really pressed and the page it opens is seen.
```

None.

None.

```text
The promise does not list the destinations, so the observation must name the full set it pressed.
```

#### E8

##### Promises

P5

```text
In the nav's lower group, Sign out and the theme control look as quiet as the other items beside them: neither one stands out as the heaviest thing in the nav, for example as a filled or coloured button among plain items.
```

```text
Quietness is a matter of how the nav looks to the operator; a check of style names or classes does not show the seen weight, and a single screenshot cannot show the look in the other theme, on hover, or at phone width.
```

None.

None.

```text
The promise gives no measurable test for 'quiet' or 'heaviest', so the observation must be a visible comparison with the neighbouring items, and the examiner must say where the judgment is a matter of degree.
```

#### E9

##### Promises

P5

```text
Sign out and the theme control still do their work after the change: pressing Sign out ends the operator's session, and using the theme control changes the look of the page she is on.
```

```text
The oracle sees the single press it made; it does not see whether the theme choice holds after she loads another page or comes back later, and a sign out that appears to work can still leave the old session usable.
```

None.

yes

None.

## Charter

None.

## Examination

None.
