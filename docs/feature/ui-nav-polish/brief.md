# Product brief: ui\-nav\-polish

## Request

```text
clean up the ui (visual pass, group C: nav). Agreed with the user: C1 mark Teams, Invite a member and Machine tokens as current on their own pages; C2 show Invite a member and Machine tokens only to operators whose page lets them in; C3 keep the nav in view on long pages; C4 shrink the phone top bar; C5 quieten Sign out and the theme button in the nav's lower group.
```

## Work

M

```text
The left nav always tells the operator where she is, offers only places she can open, and stays out of her way
```

### Rework

None.

## Promises

### P1

Outcome

```text
On Teams, Invite a member and Machine tokens, and on every page under them, the nav marks that item as the page she is on, and no other item.
```

### P2

Outcome

```text
The nav offers Invite a member and Machine tokens only to an operator whom those pages let in; an operator they would refuse does not see them, and every link the nav offers her opens.
```

### P3

Outcome

```text
On a page taller than the window, the whole nav stays in view while she scrolls the page, at desk width, open or folded.
```

### P4

Outcome

```text
On a phone 390 px wide or wider (and at most 480 px), the nav bar above the content takes at most a fifth of the screen height; on any phone at 480 px and below every destination stays one press away, and on a narrower phone the bar may grow taller rather than hide one.
```

### P5

Outcome

```text
In the nav's lower group, Sign out and the theme control read as quiet items like their neighbours, not as the heaviest things in the nav, and still work as before.
```

## Scope

```text
Left nav only: current-page marks, rights-based visibility of the two links, sticky nav, phone bar height, lower-group styling.
```

```text
Bare pages, page frame, forms and details: the next two Requests (ui-bare-pages, ui-page-frame). Any change to who may open a page.
```

## Decisions

```text
User decision 2026-11-20: fix the current-page mark for the three moved items in the UI clean-up.
```

#### Status

Confirmed


```text
User decision 2026-11-20: hide Invite a member and Machine tokens from operators those pages refuse.
```

#### Status

Confirmed


```text
User decision 2026-11-20: the UI clean-up covers groups A to D, delivered as three Requests; this one is group C.
```

#### Status

Confirmed


```text
User decision 2026-11-20: the phone bar's one-fifth bound holds for phones 390 px wide and up; on smaller phones the bar may be taller but shows every destination.
```

#### Status

Confirmed


## Journey

None.

## Slices

### S1

```text
Signed in, the operator opens Teams, Invite a member and Machine tokens and sees each marked; a plain member does not see the two links her rights refuse; on a long page the nav stays put while she scrolls; on a phone the bar is short; Sign out and the theme control look like their neighbours.
```

#### Promises

P1

P2

P3

P4

P5

#### Depends on

None.

#### Slice work

None.
