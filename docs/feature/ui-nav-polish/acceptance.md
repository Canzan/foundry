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

```text
The rights lookup fails part way through rendering a page, so the nav is built from the neutral identity the application falls back to.
```

```text
The page still answers her with her page, the nav marks the page she is on, and neither admin link is offered - so nothing is shown that would refuse her.
```

```text
An operator whose rights could not be read still gets her page and is offered only places that open.
```

##### Disposition

NotTested

```text
No public surface of the running application makes that lookup fail, and the design declares no seam for it at the acceptance boundary. The browser and HTTP supports named for this oracle cannot inject a store failure, and inventing one would be a fixture fact no authority declares. The observable consequence - an identity that is not a workspace admin is offered neither link - is covered by the rights rows above; what is not observed is the failing lookup as the cause.
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
The word Sign out and the word of the theme control each stand out from the surface behind them at the ratio body text owes, in both palettes, and neither control is asked for a fill or an outline of its own on top of that word; every control that draws no word at all, such as the glyph that narrows the nav, is still identified by its own fill or its outline strongly enough to be found by its shape.
```

```text
She reads the two quiet controls as easily as the links beside them, and she can still pick out the controls that carry no word.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature\: Board and rail text stays legible in both palettes

```text
The two quiet controls keep some fill or outline of their own as well; the row reads how legible their words are, so a control that is both legible and outlined passes. What it refuses is a quiet control whose word cannot be read, and a wordless control with nothing to find it by.
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

None.
