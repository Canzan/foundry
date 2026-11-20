# Acceptance brief: nav\-quick\-actions

## Slices

### S1

##### Covers

P1

P3

```text
Signed in on her project board, each of the four kinds of operator in turn - one with no rights beyond her workspace, a workspace admin, an instance super-admin who administers no workspace, and one who administers both - asks for the board page
```

```text
The left nav offers Home, Board, Teams, Invite a member and Machine tokens, in that order, opening the landing page, her board, her teams, the invite page and her machine tokens; the workspace-admin entry and the instance-admin entry each appear only for the operator those rights belong to
```

```text
From her board she opens her teams, the invite page or her machine tokens in one press, and nobody gained or lost a destination by the move
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-quick\-actions\.feature

```text
the three destinations were authored before Board instead of after it, if the ordered reading were relaxed to a containment check
```

###### Obligations








##### Covers

P1

```text
Before she has created any project, the operator asks for the landing page, and then asks for her teams page and her account settings
```

```text
The left nav still offers the three new destinations with Board falling back to the landing page; on the teams page and in account settings exactly one destination is marked as the page she is on, and it is Home
```

```text
A brand-new operator reaches the same three destinations, and no destination ever claims to be the page she is on when it is not
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-quick\-actions\.feature

```text
a new destination carried the current-page marker on a page that marks nothing else, which only the exactly-one reading refuses
```

###### Obligations




##### Covers

P1

P4

```text
Reading every destination the left nav offers on a served page
```

```text
Each destination carries exactly one icon and exactly one word, the word is its whole text, no icon carries a title, and no destination carries a second name beside its word
```

```text
Folded to its icons the nav still tells her, and tells her screen reader, what each icon opens
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-quick\-actions\.feature

```text
a destination were added to another template that renders its own nav, which this reading never visits
```

###### Obligations


##### Covers

P2

```text
The operator asks for the landing page, once with a project of her own, once before she has created any, and once as someone on no team at all
```

```text
No section is headed Quick actions and no quick-actions list is present, while the page still greets her by name, still names her workspace, still shows the Your projects section, and still lists her project card where she has one
```

```text
The landing page no longer repeats the nav, and she loses nothing else from it
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-quick\-actions\.feature

```text
the quick-actions markup were merely hidden by a stylesheet rule rather than removed, since the reading is of the served page, not of what is painted
```

###### Obligations




##### Covers

P4

```text
With the left nav folded to its icon rail, the operator opens the landing page at desk width and at 320, 390 and 480 pixels wide, as each of the four kinds of operator
```

```text
Every destination the rail holds, the three new ones included, is in the layout, visible, reachable by keyboard and named by its own word, and the page does not slide sideways
```

```text
Folded, she still finds and opens her teams, the invite page and her machine tokens, and no narrow page slides sideways
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-icon\-rail\.feature

```text
a destination were unreadable at a width narrower than 320 pixels, which no lane drives
```

###### Obligations





##### Covers

P1

```text
The operator asks for the landing page, and then for her project board
```

```text
The landing page marks Home as the page she is on and the board page marks Board, and in each case nothing else is marked
```

```text
The nav still tells her where she is, and the three new destinations never take that marking from Home or Board
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/navigation\-bar\-linear\-ui\.feature

```text
a new destination were marked on a page where the marking is already taken, which only the exactly-one reading refuses
```

###### Obligations



##### Covers

P1

P4

```text
Reading the one place the nav destinations are authored, every member it holds, however many that becomes
```

```text
Each member carries exactly one inline icon and exactly one word holding its whole text, and no member carries a second name beside its word; a member that does not is named with the correction
```

```text
A destination added later cannot reach her as a silent icon, because the repository refuses it before it ships
```

##### Disposition

Tested

cargo xtask check\-arch \(xtask\/src\/check\_arch\.rs rail\-member layer\)

```text
a second template authored its own nav, which this reading never visits
```

###### Obligations


## Expectations

### Rows

#### E1

##### Promises

P1

```text
From any app page she is already on (Home, a project page, a settings page), the operator sees Teams, Invite a member and Machine tokens in the left nav, and one press on each opens that destination directly, with no intervening menu, page or confirm step.
```

```text
An oracle that visits a fixed sample of pages cannot see pages outside the sample (later-added pages, error and empty states), and a press it drives programmatically does not show whether the operator first had to scroll the nav or dismiss something to reach the entry.
```

None.

None.

None.

#### E2

##### Promises

P1

```text
The three entries sit in the top part of the nav immediately after Board, in the order Teams, Invite a member, Machine tokens, and read as part of that same top group rather than as the bottom utility group.
```

```text
A check of markup order does not see what the eye sees: styling, a separator or a scroll boundary can place or group the entries elsewhere visually, and the contract fixes no order among the three, so a different order could pass an order-free oracle.
```

None.

None.

None.

#### E3

##### Promises

P2

```text
The Home page shows no Quick actions section: no such heading, and no second copy of those links anywhere on Home, so each of them is offered to the operator in the left nav only.
```

```text
An oracle that looks for the words Quick actions cannot see the same block renamed or restyled, nor the same links kept on Home somewhere else (a card, a footer, a greeting line).
```

None.

None.

None.

#### E4

##### Promises

P2

```text
Every link the operator used to reach from Home Quick actions is still offered in the left nav and still opens the same destination as before: Teams, Invite a member, Machine tokens, Integrations, Keyboard shortcuts and Sign out.
```

```text
After the change the old Quick actions inventory is no longer visible to the operator, so an oracle written against the new nav cannot see a link silently dropped in the move; matching a label also does not show that the destination behind it is the same page and the same effect (Sign out truly signs out).
```

None.

None.

```text
A record of the pre-change Home Quick actions inventory, or a build of the previous release to compare against.
```

#### E5

##### Promises

P2

```text
Home still greets the operator and still shows Your projects with the same projects and the same empty state as before the change.
```

```text
Checking that a greeting and a Your projects heading are present does not see loss inside the list: missing or reordered projects, a lost link per project, or a changed empty state.
```

None.

None.

None.

#### E6

##### Promises

P3

```text
An operator who could see a moved link in Home Quick actions still sees that entry in the left nav; an operator who could not see it there still does not see it in the nav.
```

```text
Exercising one or two operators cannot see the remaining roles and permission combinations, and nav visibility alone does not show whether the destination still refuses an operator who reaches its address directly.
```

None.

None.

```text
At least two operator accounts whose Quick actions sets differed before the change, plus one with no team or token permission.
```

#### E7

##### Promises

P3

```text
No operator gains an entry she could not see before: the move adds no link to anyone and takes none away, so her set of offered destinations is unchanged in content and changed only in place.
```

```text
This is an absence claim over all operators: a finite set of sampled accounts cannot show that no role anywhere gained or lost an entry, and the contract names no roles to enumerate.
```

None.

None.

None.

#### E8

##### Promises

P4

```text
With the left nav folded to its icon rail, Teams, Invite a member and Machine tokens are each shown as an icon, the operator can learn each destination name without unfolding the nav (as she can for every other rail item), and one press on the icon opens that destination.
```

```text
Asserting that a label or title attribute exists does not show that the name actually reaches the operator on hover or keyboard focus, nor that the three icons are distinguishable from each other and from the existing rail icons.
```

None.

None.

None.

#### E9

##### Promises

P4

```text
At a viewport 480 px wide and narrower, no app page scrolls sideways, with the nav folded and unfolded.
```

```text
An observation taken at exactly 480 px on sampled pages does not see narrower widths (the contract sets no lower bound), other pages, wide content such as long project, team or token names, or sideways scroll that appears only after an interaction such as opening the nav or a menu.
```

None.

yes

```text
Viewport width control at 480 px and below on each observed page.
```

## Charter

None.

## Examination

None.
