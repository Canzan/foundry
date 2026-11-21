# Acceptance brief: ui\-nav\-polish

## Slices

### S1

##### Covers

P1

```text
A signed-in operator opens, in turn, the dashboard, her board, the teams page, a team's own page, the invite page and her machine tokens page, and then a page no nav item owns; each one as a real authenticated request of the running application.
```

```text
On every one of those pages the left nav marks exactly one item as the page she is on, and it is the item that owns that page - Home on the dashboard and on a page no item owns, Board on her board, Teams on the teams page and a team's own page, Invite a member on the invite page, Machine tokens on her tokens page - and the item it marks is one it also offers her.
```

```text
Wherever she is, the operator reads her own place off the left nav instead of being told she is on the landing page while she reads her machine tokens.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-nav\-polish\.feature\: The nav marks the page the operator is on and no other

```text
The nav keeps marking the owner item but draws the mark in some other tint or order; the reading is of which item carries the mark, never of how it is drawn.
```

###### Obligations









##### Covers

P2

```text
Four operators in turn - a member with no admin rights, a workspace admin, an instance super-admin who administers no workspace, and one who administers both - open the same board page, and every address the left nav offered each of them is then asked for again with her own session.
```

```text
The member with no admin rights and the instance-only super-admin are offered Home, Board and Teams and neither admin link; both workspace admins are offered those three plus Invite a member and Machine tokens; and every address any of them was offered answers her own session with her page, never a refusal.
```

```text
The operator sees only the places she can go, and everything the nav shows her opens when she presses it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-nav\-polish\.feature\: An operator the two admin pages refuse is offered neither of them\, and keeps everything else\; An operator those pages let in is offered all five places\, and every one opens

```text
The two links are hidden from an operator they would refuse by a different means than her rights, for instance by the page she happens to be on; the four rights rows are driven on one and the same page, so only a rights-shaped cause satisfies all four.
```

###### Obligations







##### Covers

P2

```text
A member with no admin rights types the address of the invite page herself, and then the address of the machine tokens page.
```

```text
Each one answers her nowhere - the same not-found the application answers for an address that does not exist - so no part of the surface can be mapped by reading refusals.
```

```text
What the nav no longer shows her is also still closed to her, and the application tells her nothing about what exists behind it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-nav\-polish\.feature\: The two admin pages still refuse an operator the nav no longer offers them to

```text
The two pages start answering a different refusal that is still not a page; the reading is of the one uniform not-found both pages ship with.
```

###### Obligations


##### Covers

P3

```text
In a real browser at desk width the operator opens a board holding more work than fits her window, once with the nav open and once with it narrowed to icons by her own press of its control, and scrolls that page to its bottom; then the same two geometries on a page with nothing to scroll.
```

```text
With the page scrolled down, the whole nav still lies inside what she can see, from its top edge to its bottom edge, and every place in it can still be seen and pressed - in both geometries. On a page with nothing to scroll the whole nav lies inside what she can see as well: the open nav used to run about ten pixels past the bottom of the window there, because the rail took the height of the whole page frame.
```

```text
At the bottom of a long board the operator goes somewhere else straight away instead of scrolling back to the top to find the nav, and on a page with nothing to scroll no edge of the nav is cut off below her window.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-nav\-polish\.feature\: The whole nav is still in view at the bottom of a long page\; On a page with nothing to scroll the whole nav fits her window

```text
The nav is held in view by lifting it out of the page rather than by pinning it in place; what the row refuses is the part of it she cannot see or press, so a different mechanism that keeps every place reachable passes.
```

###### Obligations





##### Covers

P4

```text
On an emulated phone 390 px across and 844 px tall, the operator opens the dashboard: as a member with no admin rights, as an operator who administers both the workspace and the instance, and after narrowing the nav earlier; and in a fourth reading in a browser that runs no page scripts, held at the same phone width.
```

```text
In every one of those four readings the bar above the page is at most a fifth of the screen it was measured on - 168.8 px on the emulated device - every place in it can be seen and pressed, none is under a thumb's width high, and the page does not slide sideways.
```

```text
On her phone the operator reads her work instead of a letterbox under a bar of links, and she still reaches every place from it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-nav\-polish\.feature\: On her phone the bar above the page is short and keeps every place

```text
The bar is shortened by some arrangement other than drawing each place as its glyph; what the row refuses is a bar bought by hiding or shrinking a place, which the same reading catches.
```

###### Obligations





##### Covers

P5

```text
In a real browser the operator looks at the foot of the nav, once in the light palette and once in the dark one, and in a third reading with no page scripts running at all.
```

```text
Sign out and the theme control are painted in the nav's own surface, carry no border the links beside them lack, and take the same text colour and weight as those links, in both palettes. With no page scripts there is no theme control at all and the sign-out submit alone carries the law.
```

```text
The operator's eye goes to where she is and where she is going, not to the way out and the theme switch.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-nav\-polish\.feature\: Sign out and the theme control read as quietly as the links beside them\; With no page scripts there is no theme control and Sign out alone is quiet

```text
Both controls are quietened to some other shared presentation than the one the links carry today; the reading compares each control with a neighbour link in the same document, so the two move together.
```

###### Obligations




##### Covers

P5

```text
One press of Sign out at the foot of the nav, and in a second reading one press of the theme control.
```

```text
The press of Sign out ends her session and the application asks her to sign in again before it opens any page of hers; the press of the theme control leaves the application in a different palette than it was in.
```

```text
The quieter controls still do exactly what they did: she leaves in one press, and the palette still changes when she asks.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-nav\-polish\.feature\: One press of Sign out still ends the operator\'s session\; One press of the theme control still changes the palette

```text
The palette changes to a third choice rather than the other one; the reading is that the application is in a different palette after the press, which is what she asked for.
```

###### Obligations



##### Covers

P2

P1

```text
The rights lookup fails part way through rendering the invite page or the machine tokens page, so the nav is built from the neutral identity the application falls back to for an operator the page itself let in.
```

```text
The page still answers her with her page, neither admin link is offered - so nothing is shown that would refuse her - and the nav marks Home, the one place the neutral nav still offers her, rather than the place she is on or nothing at all.
```

```text
An operator whose rights could not be read still gets her page, still reads one place off the nav, and is offered only places that open.
```

##### Disposition

NotTested

```text
No public surface and no declared seam makes that lookup fail: the two pages admit a workspace admin only, so every operator they admit is offered her own place there and every operator they refuse is answered nowhere, and the nav rights read carries no public fault injection. The browser and HTTP supports named for this oracle cannot inject a store failure, and inventing one would be a fixture fact no authority declares. The design carries this branch as its own law, closed by the model invariant that no render marks nothing together with a unit test of the nav carrier, so no acceptance row is owed for it. The publicly reachable consequence - an identity that is not an admin of this workspace is offered neither link - stays covered by the rights rows above.
```

solution\-architect

##### Covers

P1

```text
The repository's own architecture check is run over the left-nav template.
```

```text
Every place where the template says this item is the page she is on sits inside a condition over the one carrier that decides it, none is written into the markup literally, and no two places share one condition - and every member of the nav still carries exactly one glyph and one whole word.
```

```text
A later change cannot quietly give the operator a second mark, or a mark that no page chose, in a nav the carrier can no longer refuse.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs\, run by cargo xtask check\-arch\: the mark\-comes\-from\-the\-carrier obligation beside the shipped rail\-member law

```text
A condition is applied to the wrong item - the check reads where the mark comes from, not which item it lands on, which is what the per-page rows above read.
```

###### Obligations


##### Covers

P5

```text
In a real browser the operator looks at a board with the nav beside it, once in the light palette and once in the dark one, and every control she can see on that page is measured as it is drawn for her.
```

```text
The word Sign out and the word of the theme control each stand out from the surface behind them at the ratio body text owes, in both palettes, and neither control is asked for a fill or an outline of its own, because both sit in the nav. Every control outside the nav keeps that reading and meets it - the lane-actions trigger each lane of the board draws is one, and at least one such control outside the nav is read on every board page this law runs on.
```

```text
She reads the two quiet controls as easily as the links beside them, and she can still pick out the controls that sit outside the nav.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature\: Board and rail text stays legible in both palettes

```text
The two controls of the nav keep some fill or outline of their own as well; the row reads how legible their words are and whether anything outside the nav is still read, so a control that is both legible and outlined passes. What it refuses is a quiet control of the nav whose word cannot be read, and a reading in which nothing outside the nav is read at all.
```

###### Obligations




##### Covers

P3

```text
In a real browser at desk width the operator narrows the nav to icons on a board holding more work than fits her window, scrolls it to its bottom, and walks the keyboard onto one place in the nav's main group and then one in the group at its foot; over that same board she opens the list of keyboard shortcuts; and in a last reading she takes the same walk with the nav open.
```

```text
The word each narrowed place shows her is drawn across a lane of her board and is what is actually painted there: the middle of its box answers that word, never the lane, a card of work or whatever encloses it. The list of keyboard shortcuts she opens is drawn over the nav. With the nav open the word of the place holding the keyboard stays inside the nav, with nothing of it over her board.
```

```text
On her board the operator reads the name of each glyph she walks onto, instead of a word clipped out of view or painted under her work - and what she opens over the page still covers the nav.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-nav\-polish\.feature\: The word the narrowed nav reveals can be read over her board\; The shortcut list she opens covers the narrowed nav\; With the nav open no word hangs over her board

```text
The word is kept readable over the board by some other means than the two the design names; the reading is of what is painted where the word is drawn, so any means that leaves it readable there passes. It also still passes if the word moves a few pixels further from the strip, which no promise fixes.
```

###### Obligations





##### Covers

P5

```text
In a real browser on the same board, the operator narrows the nav to icons with its own control and stops on Sign out with the keyboard, in the dark palette and again in the light one; then she opens that board on her phone, in the dark palette she chose and again in the light one. Every control she can see is measured as it is drawn for her in each of those four readings.
```

```text
In the narrowed nav neither Sign out nor the theme control draws a word at rest and neither is asked for a fill or an outline of its own, and the word Sign out shows her on the keyboard stands out from the box it is drawn in at the ratio body text owes. On her phone Sign out is a glyph with its word clipped out of the bar, so no ratio is read for it and it is asked for no fill or outline either. In all four readings a control outside the nav is still read for its own fill or outline and meets it, and a control of the nav quieter than that reading would allow is what makes the exemption load-bearing.
```

```text
Whichever shape the nav takes - a column of words, a strip of icons, a bar across the top of her phone - the operator reads its two controls as quietly as the links beside them, reads any word they show her, and can still pick out the controls on the page outside the nav.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature\: With the nav narrowed its two controls owe no outline\, and the word it shows her stays legible\; On her phone the nav\'s one control owes no outline\, and shows no word to read

```text
A control of the nav gains an outline in one of these layouts while the rest stay quiet; the reading asks whether anything in the nav is quieter than the outline rule allows, so one outlined control among several still passes - the nav lane is where a control drawn as the heaviest thing in the nav is refused.
```

###### Obligations



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

### E1

met

#### Observations

```text
Browser at the desk viewport 1280x800, signed in as a workspace admin: on /teams the nav marks only "Teams" (current=page), on /workspace/invites only "Invite a member", on /admin/tokens only "Machine tokens". In each of the three captures the other four primary nav links carry no current marker, so exactly one item is marked and never none and never two.

```

```text
What the mark looks like, read off the captured screen pixels of /teams at 1280x800: the marked item's box is painted a lighter fill than the rail surface (255,255,255 on a 243,244,241 rail) and its word in the strongest ink in the rail (36,38,37, contrast 15.2:1), while the four unmarked items keep the rail surface as their fill and a 5.5:1 word ink. In the dark palette the marked box is (19,24,23) on a (15,19,18) rail with a 15.7:1 word ink against 6.1:1 for the others. The mark is therefore a difference the operator can see in both palettes, carried mostly by the stronger word ink in dark, where the fill differs by only 4 to 5 levels.

```

### E2

met

#### Observations

```text
Pages below the three sections, same desk viewport and session: /team/backend/members marks only "Teams"; /team/backend/project/identity-platform, the page the nav's own "Board" link opens, marks only "Board"; /teams?sort=name, an address carrying an extra query value, marks only "Teams". The two admin sections offer no deeper page of their own in the observed build: /workspace/invites carries one invite form and /admin/tokens one issue-token form and a token list, with no link below either. Deeper pages under other teams and other unvisited addresses were not opened and stay unobserved.

```

```text
Browser at the desk viewport 1280x800, signed in as a workspace admin: on /teams the nav marks only "Teams" (current=page), on /workspace/invites only "Invite a member", on /admin/tokens only "Machine tokens". In each of the three captures the other four primary nav links carry no current marker, so exactly one item is marked and never none and never two.

```

### E3

met

#### Observations

```text
Two operators the admin pages admit: Priya (workspace admin and instance admin) and Tomas (workspace admin, not an instance admin). Each is offered "Invite a member" and "Machine tokens" in the nav. Pressing "Invite a member" opens /workspace/invites with status 200 and pressing "Machine tokens" opens /admin/tokens with status 200, for both of them, and each arrives with that same item marked. Every one of the five links the nav offered them opened with the same signed-in session: Home (/), Board (/team/backend/project/identity-platform), Teams (/teams) and the two admin pages, all status 200, no refusal and no error page. Both were admitted by signing in through the public sign-in form with their own credentials.

```

### E4

met

#### Observations

```text
The operator the two pages refuse, Lena, a plain workspace member: asking for /workspace/invites and /admin/tokens directly answers her 404 in both cases, so those pages do refuse her. Her nav offers only Home, Board and Teams. The two admin items are absent from her nav, not greyed or disabled: no such link is listed in her rail at the desk viewport 1280x800 with the rail expanded, at 1280x800 with the rail folded to a 72 px strip, at the 390x844 phone with the bar expanded, and at the 390x844 phone with the bar folded. Each of the three links she is offered opened 200 with her own session. Other refused operators and other widths were not opened.

```

### E5

not_examined

```text
Not examined for want of a reachable stimulus, see observation O6. The promise needs a desk page taller than its window, scrolled to the bottom. Through the public harness no such desk page exists: in every capture the application shell fits the window, with the footer landmark sitting at the viewport bottom (y=784 of 800, y=404 of 420), and the harness carries no step that scrolls a page. Prerequisite: a desk subject whose document exceeds its window, plus a page-scroll step. Owner: the acceptance designer who owns the public harness. Remedy: add that step or that subject and re-capture the rail in both geometries. What I did capture supports only the weaker claim that the rail box spans the viewport with all five items visible before scrolling, and O6 retains the negative that in a 1280x420 window the rail's own content scrolled and the first item's word went off-screen.

```

### E6

met

#### Observations

```text
Height of the nav bar above the content at phone widths, measured against the browser view height that the same capture reports: 125 px of 844 (14.8 per cent) at 390x844, 125 px of 800 (15.6 per cent) at 480x800, and 125 px of 640 (19.5 per cent) at 360x640. All three are at or below a fifth, with only about 3 px of margin left at 360x640. A smaller or shorter phone, a wrapped item label and a long account name were not observed.

```

### E7

met

#### Observations

```text
One press to each destination at phone widths. At 390x844 all five destinations stand in the bar itself, each 44 px wide, with no menu, fold or "more" control in front of them; pressing each in turn opened its own page: Teams /teams 200, Board /team/backend/project/identity-platform 200, Invite a member /workspace/invites 200, Machine tokens /admin/tokens 200, each arriving with that item marked. At 480x800 and at 360x640 the same five items are present and visible, 44 and 40 px wide. Two readings qualify this: at these widths the item labels are reduced to a 1x1 clip, so the operator sees the glyph and not the word, and at 390 and 360 the clipped "Machine tokens" text reaches past the right edge without giving the page a sideways scroll.

```

### E8

met

#### Observations

```text
How quiet the lower group looks, read off the captured screen pixels at 1280x800. Scanning the full height of the rail for any box painted unlike the rail surface across the item width finds exactly one band in the whole rail: the current-page item. Sign out's box is the rail surface, with 3.6 per cent of its pixels unlike that surface, and the theme control's 4.0 per cent, against 5.6 to 8.3 per cent for the plain links beside them, so neither one owes a fill or a border in either palette, and neither is the heaviest thing in the nav. Sign out's word is painted in exactly the same ink as the plain links (92,100,95 light and 141,149,143 dark). MATTER OF DEGREE: the theme control's word is painted in the strongest ink tier instead (18,22,20, 16.5:1 in light; 232,235,232, 15.6:1 in dark) against its neighbours' 5.5:1 and 6.1:1, the same tier as the marked page item, so it reads as more prominent than the items beside it while still being a plain, unfilled, unbordered item. The look on hover and at phone width was not captured.

```

### E9

met

#### Observations

```text
Both controls still do their work. Pressing the theme control advanced it from "Theme: following your device" to light and then to dark, and the captured screens show the rail surface changing from (243,244,241) to (15,19,18), so the look of the page she is on did change; the dark choice was still in force after reloading that same page. Pressing Sign out took the operator to /sign-in with status 200, and a following request for /teams in the same browser answered 404, so her session had ended. Not observed: whether the theme choice holds on a different page or on a later visit, and whether the old session token is still usable anywhere else.

```
