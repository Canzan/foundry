# Acceptance brief: nav\-collapse\-toggle

## Slices

### S1

##### Covers

P1

```text
An operator signed in at desk width on her dashboard presses the control at the head of the left nav.
```

```text
The nav becomes a narrow control strip, the page content grows by exactly the width the nav released, the strip and the content still fill the shell between them, and the browser now holds the one folded-away choice.
```

```text
She gets a wider page with no empty band where the nav was, and the app has taken note of her choice.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: Folding the left nav away gives its width to the page content

```text
The strip were a different width than the designers chose: no promise fixes the strip width, so the row fails only for a nav that does not narrow, or a page that does not take the released room.
```

###### Obligations



##### Covers

P2

```text
Having folded the nav away, the operator reloads that page, and separately opens the project board in the same browser.
```

```text
The narrow strip is there on the reloaded page and on the other page of the app shell.
```

```text
Her choice survives a reload and a move to another page; she does not have to fold the nav again on every screen.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: A folded\-away left nav is still folded away when the operator comes back

```text
The choice were kept under another name or in another shape: this row reads the nav she sees, not the spelling of what was stored, which its own row covers.
```

###### Obligations



##### Covers

P2

```text
Having folded the nav away, the operator opens another page of the app shell and the served document is read as the app sends it.
```

```text
The page already carries the folded-away mark, the script that stamps it is declared in the document head with nothing that lets the browser defer it past the first paint, and it finished loading before the browser's own first paint.
```

```text
Moving between pages she never sees the wide nav blink open before it folds away again.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: The folded\-away nav is settled before the browser is allowed to paint

```text
A very slow machine still painted one frame of the open nav: the pixels of the first frame are not observable here, so this row rests on the tag the app sends and on the paint timing the browser reports.
```

###### Obligations


##### Covers

P3

```text
With the nav folded away, the operator presses the control again.
```

```text
The nav is back at the full width it has always had, every item is in the layout, in sight and selectable, and the browser holds no nav choice at all.
```

```text
She gets her nav back exactly as it was, and the app is back to its untouched state rather than remembering a second choice.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: Opening the left nav again brings its items back

```text
The items were restyled or reordered: the row observes that each one is laid out, visible and selectable, not how it looks.
```

###### Obligations



##### Covers

P3

```text
Having folded the nav away and opened it again, the operator reloads the page.
```

```text
The nav is still at its full width after the load.
```

```text
Once she has her nav back it stays back, and no later load folds it away under her.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: A left nav the operator opened again stays open on the next load

```text
The nav were open because nothing was ever stored rather than because the stored choice was removed: the removal is pinned by the row above.
```

###### Obligations


##### Covers

P1

P3

```text
From the open nav, and again from the folded-away nav, the operator presses the control twice.
```

```text
The nav is back in the state it started from, and the browser holds the choice that state calls for: nothing at all for the open nav, the single folded-away choice for the folded one.
```

```text
Two presses leave her exactly where she was, on screen and in what the app remembers.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: Two presses leave the left nav and the remembered choice as they were

```text
Each press took an unobservably long time: the row observes the state after the pair, not how quickly either press lands.
```

###### Obligations



##### Covers

P2

```text
With her dashboard open in two tabs of one browser, the operator folds the nav away in the second tab, then reloads the first.
```

```text
The tab she was reading still shows the open nav, and shows the narrow strip only after she reloads it.
```

```text
A press in another tab never moves the page she is reading; the page she reloads picks up her latest choice.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: A press in a second tab leaves the page the operator is reading alone

```text
The two tabs agreed immediately by some other route: this row would then fail, which is the point; it fixes today's promise, and following the other tab at once is a change the product owner owns.
```

###### Obligations


##### Covers

P2

```text
A browser that already holds the folded-away choice opens the dashboard, the project board and the project change report, with no press anywhere.
```

```text
Every one of those pages shows the narrow strip, and the choice at the origin is byte-for-byte the one the operator left.
```

```text
Reading around the app keeps her nav folded away and never quietly rewrites or loses the choice she made.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: Reading three pages without touching the control never rewrites the choice

```text
A page rewrote the choice to the identical bytes: a rewrite that changes nothing is indistinguishable here, and it costs the operator nothing.
```

###### Obligations



##### Covers

P1

P2

```text
The browser holds, in turn, nothing at all, this app's own folded-away choice, and four strings it never wrote: expanded, Collapsed, 1 and null. The operator opens her dashboard.
```

```text
Only this app's own choice folds the nav away; every other string, and no string at all, leaves the nav open with its control present and reporting the open state. No string produces a third nav.
```

```text
A value left by another tool, an extension or an older version costs her nothing worse than an open nav; she never meets a nav that matches no rule.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: The left nav follows the remembered choice\, and only this app\'s own choice folds it

```text
Another unlisted string also read as open: that is the behaviour this row asks for, and the table walks the whole domain the design admits.
```

###### Obligations







##### Covers

P1

```text
On the open dashboard, something other than this app stamps the page with a folded-away mark that carries a value of its own.
```

```text
The nav is exactly as narrow as the strip the app's own valueless mark gives, and the strip and the content still fill the shell.
```

```text
Whatever writes that mark, she gets one of the two navs she knows, never a half-folded page she cannot repair.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: A mark another tool left with a value of its own still gives one of the two navs

```text
The outside writer's mark were refused outright rather than read as folded: the row compares the two marks' geometry, and either way there are only two navs.
```

###### Obligations


##### Covers

P4

```text
The operator walks the keyboard focus to the nav control from a cleared focus, then presses Enter; and in the second case Space.
```

```text
The control is reached in the page's own reading order, each key folds the nav away, and the control keeps the focus with a visible indicator.
```

```text
She can fold the nav away without a mouse and can see where she is while she does it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: The operator folds the left nav away from the keyboard alone

```text
The control sat late in the reading order: the row observes that the keyboard reaches it within the page, not where in the order it sits.
```

###### Obligations

None.

##### Covers

P4

```text
The operator reads what the control reports about the open nav, then folds the nav away and reads it again.
```

```text
The control is a real button whose published state names the open nav and then the folded one, whose target is the rail's own primary navigation, and whose name is never empty, names what it works, and is different after the press.
```

```text
An operator who cannot see the page is told which way the nav is set and what her next press will do, every time.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: The control says which way the left nav is set and what the next press does

```text
The wording were phrased differently: the exact words are the implementation's to choose, so the row fixes the published state, a non-empty name that names the nav, and that the name changes with the press.
```

###### Obligations

None.

##### Covers

P1

P4

```text
With the nav folded away, the operator walks the whole page with the Tab key.
```

```text
No item of the folded-away part of the rail ever takes the focus and those items are out of the layout, while the monogram, the nav control and the theme control are all still there and the theme control still carries its name.
```

```text
She never tabs into a nav she cannot see, and folding the nav away does not cost her the controls that stay in the strip.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: A folded\-away left nav leaves no hidden link in the keyboard order

```text
A screen reader announced the strip awkwardly: the row observes the focus order and the layout, not how any one assistive technology reads it aloud.
```

###### Obligations

None.

##### Covers

P5

```text
A browser with scripting switched off at the browser opens the dashboard.
```

```text
The nav renders at its full width with every item usable, and no control over the nav exists in the document or can be reached from the keyboard.
```

```text
With scripting off she gets the app exactly as it was before this feature: a usable nav and no control that looks live and does nothing.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: With scripting off the left nav is open and there is no control at all

```text
The page differed from the pre-feature page in some way this row does not read: no record of the earlier page is kept, so the row fixes the open nav, usable items and the absence of a control.
```

###### Obligations

None.

##### Covers

P2

P5

```text
A browser that refuses this site access to stored state opens the sign-in screen, where the refusal is first probed against the real origin.
```

```text
The screen renders its form, the document carries no folded-away mark, the fold asset was fetched on that load, and the browser's own record of unhandled script errors is empty while a deliberate error proves that record works.
```

```text
A browser that blocks stored state costs her only the memory of her choice: the page still works and nothing is reported at her.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: With stored state refused the page still renders and nothing is reported

```text
Some other browser refused stored state in a different way: the row drives the one refusal this suite can produce against a real origin.
```

###### Obligations


##### Covers

P5

```text
A browser that holds the folded-away choice opens the sign-in screen, which has no nav.
```

```text
No control is built there, the document still carries the mark that no rule on that screen consumes, and no script error is reported.
```

```text
The fifteen screens with no nav are untouched by this feature and report nothing at her.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: A screen with no left nav builds no control and reports nothing

```text
A screen with no nav rendered differently in some way this row does not read: it observes the absent control, the stamped mark and the quiet error record.
```

###### Obligations

None.

##### Covers

P6

```text
A real phone viewport 390, then 480, then 320 CSS pixels wide opens the dashboard, once holding no nav choice and once holding the folded-away choice.
```

```text
At every width and in both states the page does not scroll sideways, the nav still lays out as the top bar across the screen, and the state under test is really reached: the control is present and the mark agrees with what the origin holds.
```

```text
On a phone, in either nav state and at the widths the promise covers, nothing slides sideways under her thumb.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: A phone screen never scrolls sideways\, whichever way the left nav is set

```text
A width between the three driven ones overflowed: the promise covers an open range and three widths are driven, including the breakpoint itself and a width well under it.
```

###### Obligations



##### Covers

P6

```text
A real 390-pixel phone viewport holding the folded-away choice opens the project board.
```

```text
The page does not scroll sideways while the board's own columns strip still carries the sideways scroll inside itself.
```

```text
The board stays usable on her phone with the nav folded away: the columns move, the page does not.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: On a phone the folded\-away nav leaves the board columns scrolling inside their own strip

```text
The strip scrolled further or less far than before: the row observes that the strip, and not the page, is what scrolls.
```

###### Obligations


##### Covers

P1

P6

```text
The workspace is renamed to the longest name the app admits, 24 characters, and the operator folds the nav away on her dashboard.
```

```text
The strip is still narrower than the open nav, the workspace name is out of the layout rather than merely clipped, and the page does not scroll sideways.
```

```text
The longest workspace name she can create cannot stretch her folded-away nav or push the page sideways.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/nav\-collapse\-toggle\.feature\: The longest workspace name the app allows never widens the folded\-away nav

```text
A name longer than the app admits behaved differently: the app refuses such a name in the store, so no operator can create one, and a name out of the layout cannot contribute width at any length.
```

###### Obligations

None.

##### Covers

P2

```text
A page loads in a browser whose first painted frame could show the open nav before the folded one.
```

```text
No frame shows the wrong nav.
```

```text
The operator never sees the nav blink open during a load.
```

##### Disposition

NotTested

```text
The pixels of the first painted frame are not observable in this suite: no paint-capture surface is used, which the design records as an open question. The available evidence is the render-blocking shape of the tag and the paint timing, both observed by the oracle.
```

solution architect

##### Covers

P1

```text
A browser refuses this site access to stored state and the operator presses the nav control on a signed-in page.
```

```text
The press still folds the nav in that document and only the memory of the choice is lost.
```

```text
Even where her choice cannot be remembered, the control still works for the page she is on.
```

##### Disposition

NotTested

```text
Unreachable by construction, as the design records: refusing site storage also refuses the session cookie, so no signed-in screen exists under that condition, and the nav renders only on signed-in screens. The read half of the same guard is observed on the sign-in screen.
```

solution architect

##### Covers

P2

```text
A script in the document shell that must run before the browser paints is given defer, async or type=module, and a third such script is added later; the shipped shell is checked as it stands.
```

```text
The architecture gate passes on the shipped shell and refuses the altered one, naming which tag it is and that the flash of the wrong nav is what the attribute would bring back.
```

```text
The operator keeps the promise of no flash as the shell grows, instead of losing it silently to an attribute copied from a neighbouring tag.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs\, run by cargo xtask check\-arch\: the pre\-paint script layer and its two fixtures

```text
A pre-paint script were moved out of the head entirely rather than deferred: that is a different law, and the oracle observes the shipped shell's own tag from the served document. The layer at this locator is authored where the design assigns it, in a file this role does not write; the public oracle covers the shipped-shell arm of the same law.
```

###### Obligations



## Expectations

### Rows

#### E1

##### Promises

P1

```text
On a page of the app shell, the operator sees a toggle control for the left nav, and activates it to collapse the expanded nav: the nav becomes narrow or goes away.
```

```text
The promise does not say where the control is, what it looks like, or that the operator can find it without help. An oracle can find the control by a known identifier and never show that an operator sees or recognizes it.
```

yes

None.

None.

#### E2

##### Promises

P1

```text
After the nav collapses, the page content uses the released width: the text and the controls reflow wider, and no empty band stays where the nav was.
```

```text
The words "the width the nav gave up" give no quantity. An oracle can show that the content box became wider while a part of the gutter stays empty, or while the content is overlapped or cut off.
```

yes

None.

None.

#### E3

##### Promises

P2

```text
The operator collapses the nav, then reloads the same page in the same browser: the nav is collapsed when the page is ready to use.
```

```text
An observation that is made after the load is complete cannot show what the operator saw during the load. A collapsed marker in the page is not proof that the nav looks collapsed.
```

yes

None.

None.

#### E4

##### Promises

P2

```text
During that reload the operator never sees the expanded nav: no moment of the load shows the wide nav before the collapsed nav.
```

```text
The promise gives no time limit for a flash and needs frame-level capture. A fast local machine can hide a flash that a slow or throttled machine shows, and an oracle that reads only the page structure cannot see paint at all.
```

yes

None.

```text
Capture of the painted frames or of the first paint during the load, not only the settled page.
```

#### E5

##### Promises

P2

```text
The operator opens a different page of the app shell in the same browser and finds the nav collapsed, and the nav stays collapsed until the operator activates the toggle: no reload, page change or other use expands it again.
```

```text
The promise does not define "the same browser" (same tab, a second tab, a new window, a private window, a second device). A finite set of page visits cannot show that no other action expands the nav.
```

yes

None.

None.

#### E6

##### Promises

P3

```text
With the nav collapsed, the operator activates the toggle and the nav is expanded again: the nav items are legible and the operator can select them.
```

```text
An oracle can read a state marker or a width and never show that the nav items are legible and selectable again.
```

yes

None.

None.

#### E7

##### Promises

P3

```text
After that expansion, the nav is still expanded when the operator reloads the page and when the operator opens a different page of the app shell.
```

```text
Expanded is also the start condition. An observation of an expanded nav cannot tell a kept choice from a stored choice that was lost, cleared or never written.
```

yes

None.

None.

#### E8

##### Promises

P4

```text
With the keyboard only, the operator moves the focus to the toggle, sees which control has the focus, and operates it to collapse and to expand the nav.
```

```text
A scripted pointer click, or a key event that is sent directly to the control, can pass without proof that the control is in the keyboard order and that the focus is visible.
```

yes

None.

None.

#### E9

##### Promises

P4

```text
The toggle tells the operator who uses assistive technology if the nav is now expanded or collapsed, in its accessible name or in its state, and that wording or state changes when the operator operates the toggle.
```

```text
The promise accepts a name or a state, thus an oracle can read one attribute in one condition only. An attribute in the page is not proof that a screen reader announces the change.
```

yes

None.

None.

#### E10

##### Promises

P5

```text
The operator disables JavaScript in the browser, opens a page of the app shell, and the left nav shows expanded and usable.
```

```text
JavaScript that is disabled in one test engine does not stand for the settings of the operator browser. The observation cannot show that a stored collapsed choice never changes the page without JavaScript.
```

yes

yes

```text
A browser session with JavaScript disabled.
```

#### E11

##### Promises

P5

```text
With JavaScript disabled, the page gives no nav toggle: the operator sees no toggle and cannot reach one with the keyboard, and the page looks as it did before this feature.
```

```text
A control that is absent from the page source is not the same as a control that is present, hidden and still reachable by keyboard. The words "as before this feature" need a record of the earlier page, which a present observation cannot give.
```

yes

yes

```text
A browser session with JavaScript disabled; a record of the no-JavaScript page before this feature for the comparison.
```

#### E12

##### Promises

P6

```text
At a viewport width of 480 px with the nav expanded, the whole page fits the width: no horizontal scroll bar, no sideways scroll, and no content cut off at the right edge.
```

```text
A check of the page width or of the scroll bar does not see a part of the page that scrolls sideways inside it, and one width says nothing about the smaller widths that the promise also covers.
```

yes

None.

```text
Control of the viewport width.
```

#### E13

##### Promises

P6

```text
At a viewport width of 480 px with the nav collapsed, the page also fits the width, with no horizontal scroll.
```

```text
The collapsed condition can be observed on one page and at one width only. A wider page body on a different page of the app shell stays unseen.
```

yes

None.

```text
Control of the viewport width.
```

#### E14

##### Promises

P6

```text
At a phone width below 480 px, for example 320 px, the page has no horizontal scroll with the nav expanded and with the nav collapsed.
```

```text
The range "480 px and below" is open. Any count of selected widths leaves other widths unseen, and a no-scroll observation does not show that the nav and the content stay usable at that width.
```

yes

None.

```text
Control of the viewport width.
```

## Charter

None.

## Examination

None.
