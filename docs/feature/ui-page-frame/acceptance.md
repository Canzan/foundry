# Acceptance brief: ui\-page\-frame

## Slices

### S1

##### Covers

P1

```text
She opens every app page on a desk and on a phone, including the signed page that sits outside the frame, then opens the same pages in a wider window. The destinations the rail offers her are compared with the pages she opened.
```

```text
Every framed page starts its content at one shared edge and one shared height, inside exactly one padded box, with nothing running off the side of the window. The first heading on every framed page is painted on that same top edge; a heading that keeps a margin of its own sits lower, and the reading names the page. The page outside the frame keeps the padding of its own. Widening the window leaves every start where it was, and the board's lanes grow with the window. Every destination the rail offers, apart from the shortcut list and the integrations pages this request leaves alone, is one of the pages she opened.
```

```text
As she moves between Home, the board, Teams, invites, tokens and settings, the content never jumps sideways or down, and the board still uses the width of the window.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-page\-frame\.feature\: Every page she opens starts its content at the one frame edge

```text
A fragment painted later inside a page she already opened can carry padding of its own, and a destination the rail does not offer yet is unseen. A notice above the heading, or a title that wraps, on a landing this session does not make, is unseen. The painted top of the first heading is part of this reading, so a heading that sits lower on one page does not pass.
```

###### Obligations










##### Covers

P1

P2

```text
The architecture check reads the shipped stylesheet name, the colour tokens in that stylesheet, and the scripts in the document head.
```

```text
The stylesheet file name matches its own bytes, colour values stay in the token seam, and a head script that touches the browser store before paint stays blocking while every other head script of that shell stays deferred.
```

```text
After this change a refresh still loads the colours and the scripts the way the running app already does.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs\, run by cargo xtask check\-arch\: the stylesheet hash\, the colour\-token seam and the head\-script rule

```text
The check accepts a frame whose left edge or field width is wrong, because it never reads the painted layout. Those readings are the page scenarios.
```

###### Obligations





##### Covers

P2

```text
She stores names of every shape, including an unbroken project name of 256 letters, a workspace name of 24 wide letters, a one-letter name, a team name of 64 letters and an email of 254 letters. She opens the forms and both dialogs in each palette, and she opens the lane-delete dialog and the sign-in card.
```

```text
Each field inside the frame stays within the cap for its own size, and no page scrolls sideways. In both dialogs the title is as wide as the description under it and as wide as the dialog. The password field keeps the width of the card. Lists, number fields, file controls and the password are drawn with the surface, the hairline and the type around them, and a list keeps its arrow. A page that overflows names the seed and the shortest run that still overflows.
```

```text
Fields are sized to what they hold, the dialogs stay level, and every list, number and file control looks like the rest of Foundry in either palette.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-page\-frame\.feature\: Whatever she has stored\, a field stays as wide as its own size and no page scrolls sideways

```text
The room field and the channel list of a Matrix route are absent from the pages this runtime opens, so a wrong drawing of those two is unseen here. Delete's word contrast is skipped when its ink or fill resolves no colour.
```

###### Obligations












##### Covers

P2

```text
She opens a new Matrix route and looks for the room field and the channel list.
```

```text
The room field and the channel list are drawn as the same family as every other text field and list.
```

```text
The room field and the channel list match the other fields.
```

##### Disposition

NotTested

```text
The public route hides the room field and the channel list until a Matrix connection exists. This design's verification runtime does not provide one, and adding a connection would invent a fixture the closed design did not declare.
```

solution\-architect

##### Covers

P3

```text
She opens Teams, the instance dashboard and her account settings, then renames a project and a workspace, once with a name each accepts and once with a name each refuses.
```

```text
Each form sits in a section that holds that form alone, with one heading and its own action. A project rename form that shares the Workspaces heading with another form does not. The rename rows carry no bullet and no indent of the browser's own, and a project row can still be told from its workspace by a hairline or a different ground. The grouped choices are drawn as Foundry's own groups, and the content edge stays where the frame put it. An accepted name replaces that row, and a refused name is answered inside the row that refused it.
```

```text
She can tell where one form ends and the next begins, because each form has a heading of its own, and a rename either shows the new name in place or the reason it was refused, without the page jumping.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-page\-frame\.feature\: Each page of stacked forms reads as sections\, each under its own heading

```text
The heading's words can be the same phrase on every form. The reading checks that the section holds that form alone and one heading, and it does not pin the heading's copy.
```

###### Obligations




##### Covers

P4

```text
She opens the issue dialog in each palette and presses Delete.
```

```text
Delete is painted with the surface and the muted hairline, Save with the primary fill, and both stay readable against the dialog. Pressing Delete asks her to confirm before the issue is deleted.
```

```text
Delete reads as the secondary control, and pressing it still asks her to confirm.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-page\-frame\.feature\: In the issue dialog Delete is the quiet control and Save alone is the main action

```text
The confirmation is recognised by the word delete somewhere in the dialog, so a sentence that mentions delete without asking her to confirm would still pass. A control in that dialog other than Delete and Save can stay unstyled.
```

###### Obligations



##### Covers

P5

```text
She opens the board and its change report, opens the board as a teammate who may not manage it, deletes the issue on her open board from somewhere else, and presses slash.
```

```text
Both headers show the key as a labelled value. Change report and Integrations are separate quiet controls with a gap and no underline, and the report address is the board's own report. A reader sees the key and only Change report. The card that was deleted elsewhere leaves her open board, and the column that held it stays that column. Slash opens the search panel in its own block beside New issue.
```

```text
The board header reads as one tidy row. A card deleted somewhere else leaves the board she already has open, and slash still opens the search panel beside New issue.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-page\-frame\.feature\: The board header and the change report each name the project key as a labelled value

```text
The key label is accepted when its words appear anywhere in the header text, so a label repeated in another sentence of that header would still pass. The card leaving is accepted when that card is gone and the column that held it is still the column she had open, so some other script that removes only that card would still pass. A new card appearing is not read.
```

###### Obligations






##### Covers

P2

```text
She stores names drawn from a recorded seed, always including the named extremes, and every page that shows those names is opened.
```

```text
For any stored value within its own bound, the field stays within the cap that applies where it sits and the page gains no horizontal overflow. A failing case prints the seed and shrinks toward the shortest overflowing run.
```

```text
A name of any shape she is allowed to store stays inside its field, and the page does not scroll sideways.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/src\/steps\/feature\_ui\_page\_frame\.rs\:\:generated\_names

```text
The sample is finite, so a name shape outside the seed and the named extremes can still overflow. The seed and the shrink are printed when a sampled case fails.
```

###### Obligations


##### Covers

P1

```text
She opens her board on a desk and reads every lane against the frame she can see.
```

```text
At desk width every lane lies inside the frame, and the board strip is no wider than what she can see, so she does not have to scroll the board sideways.
```

```text
At her desk the board stays on the screen, and she can read every lane without scrolling sideways.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-page\-frame\.feature\: Her board stays on the screen with no sideways scroll

```text
A board with more lanes than the project this session holds can still overflow at desk width. This reading checks the shipped board at desk width.
```

###### Obligations


##### Covers

P1

```text
Home holds two projects, Sandbox and Ledger. She opens it on a desk and on a phone and reads each project card against the frame.
```

```text
Each project card is as wide as the frame and starts at the frame's own left edge. A card that sits in a narrow track of a wider grid is narrower than the frame, and the reading names it.
```

```text
The project cards on Home span the frame instead of sitting in a narrow column.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-page\-frame\.feature\: Each project card on Home spans the frame

```text
Two cards that share a row only when Home holds a third project, or only on a desk wider than the two screens this reading opens, still each match the frame here. This reading is these two projects, at those two widths.
```

###### Obligations


##### Covers

P1

```text
She opens her board on her phone and reads the page width and the board lane strip.
```

```text
On a phone the board page itself does not run off the side of her screen, while the board strip keeps its own horizontal scroll so lanes scroll inside the board instead of collapsing.
```

```text
On her phone the board page stays within the screen width and she can scroll the lanes sideways inside the board strip.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-page\-frame\.feature\: On her phone her board scrolls its lanes inside the board without widening the page

```text
A mobile board with a single lane would not overflow the strip; a phone width other than 390px or a landscape orientation is not read here.
```

###### Obligations


## Expectations

### Rows

#### E1

##### Promises

P1

```text
Moving between the signed-in pages (Home, the board, Teams, invites, tokens, settings), the user sees each page's content begin at the same left edge beside the nav, so the content never jumps sideways from page to page.
```

```text
A user sees only the pages she visits: a signed-in page outside the sampled set, or a page in an empty, error or loading state, can still start at a different edge, and she cannot see whether a page added later keeps the edge.
```

None.

None.

None.

#### E2

##### Promises

P1

```text
On every signed-in page the gap between the top of the page frame and the first line of content is the same, so the first heading sits at the same height on each page.
```

```text
The user judges the top gap on a page as she first lands on it; she cannot see whether the gap holds while the page is scrolled, with a long or wrapped page title, or when a banner or flash message is present above the content.
```

None.

None.

None.

#### E3

##### Promises

P1

```text
At desk width and on a phone the content keeps the same frame: it fills the same width as the other pages, is not clipped, and needs no sideways scrolling to read.
```

```text
Only the two widths she tries are visible to her: tablet and in-between widths, landscape on a phone, browser zoom and a larger system font are unobserved, and 'same width' is not something she can measure by eye.
```

None.

None.

None.

#### E4

##### Promises

P1

```text
Home no longer shows a narrow centred column: its content starts at the same left edge as the other pages and its cards span the frame width (confirmed decision 2026-11-22).
```

```text
She sees the card row width on her own window only; she cannot see whether a wider or much narrower window keeps the cards inside the frame, nor whether a page with a single card still reads as intended.
```

None.

None.

None.

#### E5

##### Promises

P2

```text
A short text field on an app form (for example a project key, a name or a number) is drawn narrow, about as wide as the value it holds, instead of stretching across the whole page beside its label.
```

```text
'As wide as what it holds' has no stated measure she can check: she cannot see whether the field still fits the longest value the form accepts, and a field that is too narrow for real content looks the same to her as one that is correctly sized.
```

None.

None.

None.

#### E6

##### Promises

P2

```text
Every list (drop-down), number and file control on the app's forms is drawn in the same style as Foundry's other controls, so a form reads as one set of Foundry controls and not a mix of Foundry fields and plain browser widgets.
```

```text
She sees one browser on one operating system: the native look of these controls differs elsewhere, and the open drop-down list, the file picker, the number spinner and the focused, disabled and error states can still fall back to the browser default without her noticing on the closed, resting form.
```

None.

None.

None.

#### E7

##### Promises

P3

```text
On a page that holds several forms, each form appears as its own visible section under its own heading, so she can tell where one form ends and the next begins.
```

```text
She can see that headings exist and that sections are separated, but not whether each heading names the right form, nor the heading order a screen reader announces.
```

None.

None.

None.

#### E8

##### Promises

P3

```text
Each section's action button sits with that section's own fields, so no button is left over at the end of the page away from the fields it belongs to; Teams shows no orphaned Save.
```

```text
Position does not show binding: she cannot see which form a button actually submits, and she learns a wrong pairing only by pressing it and watching the result.
```

None.

None.

None.

#### E9

##### Promises

P3

```text
Instance admin shows its rename forms as sections rather than nested bullet lists, and settings shows labelled sections rather than raw browser fieldsets with their default boxes and legends.
```

```text
Only the three named pages are promised: she cannot see whether any other page with several forms still shows bullet lists, raw fieldsets or an orphaned button.
```

None.

None.

None.

#### E10

##### Promises

P4

```text
In the issue dialog Delete reads as a quiet, secondary control: the dialog's main action is clearly the more prominent one, and Delete is not what her eye or a stray Enter lands on first.
```

```text
'Quiet' is a judgement, not a value she can read off the screen: prominence depends on her own perception, and the keyboard default and focus order of the dialog are not visible from how the buttons look.
```

None.

None.

None.

#### E11

##### Promises

P4

```text
Pressing Delete in the issue dialog still asks her to confirm before anything is deleted, and declining the confirmation leaves the issue in place.
```

```text
Seeing the confirmation appear does not show her the outcome: whether declining truly keeps the issue and confirming truly deletes it is only visible after she returns to the board, and dismissing by Escape or by clicking outside is a separate path she may never try.
```

None.

None.

None.

#### E12

##### Promises

P5

```text
The board header shows the project key as a labelled value: the label 'Key prefix' with the key beside it, read as label and value rather than one run of plain text.
```

```text
She can see the label and the key, but not whether the pairing is announced as a label and value to a screen reader, nor how the header reads with an unusually long key.
```

None.

None.

None.

#### E13

##### Promises

P5

```text
In the board header Change report and Integrations appear as two separate, clearly drawn controls with visible separation, not two underlined links running together into one word.
```

```text
A resting header does not show her the hover, focus and pressed states or the size of each control's clickable area, so two visually separate labels can still be hard to hit apart, and a narrow phone header may wrap them back together.
```

None.

None.

None.

## Charter

None.

## Examination

### E1

met

#### Observations

```text
Desk 1280x800 (slack-connected, viewer Priya): Home /, Board /team/backend/project/identity-platform, Change report .../report, Integrations .../integrations, Teams /teams, Invites /workspace/invites, Tokens /admin/tokens, Settings /account/settings, Instance dashboard /admin/instance/workspaces, and Instance integrations /admin/instance/integrations all place h1 and main/banner at x=272 y=24 with 976px frame width (961px with 15px scrollbar) and horizontal scroll: no. Phone 390x844: Home, Board, Teams, Invites, Tokens, Settings, and Instance dashboard all place h1 and main/banner at x=16 y=141 (16px below 125px top nav) with 358px frame width and horizontal scroll: no; Board .board keeps internal horizontal lane scroll (358px box, 692px content) without document overflow.
```

### E2

met

#### Observations

```text
Desk 1280x800 (slack-connected, viewer Priya): Home /, Board /team/backend/project/identity-platform, Change report .../report, Integrations .../integrations, Teams /teams, Invites /workspace/invites, Tokens /admin/tokens, Settings /account/settings, Instance dashboard /admin/instance/workspaces, and Instance integrations /admin/instance/integrations all place h1 and main/banner at x=272 y=24 with 976px frame width (961px with 15px scrollbar) and horizontal scroll: no. Phone 390x844: Home, Board, Teams, Invites, Tokens, Settings, and Instance dashboard all place h1 and main/banner at x=16 y=141 (16px below 125px top nav) with 358px frame width and horizontal scroll: no; Board .board keeps internal horizontal lane scroll (358px box, 692px content) without document overflow.
```

### E3

met

#### Observations

```text
Desk 1280x800 (slack-connected, viewer Priya): Home /, Board /team/backend/project/identity-platform, Change report .../report, Integrations .../integrations, Teams /teams, Invites /workspace/invites, Tokens /admin/tokens, Settings /account/settings, Instance dashboard /admin/instance/workspaces, and Instance integrations /admin/instance/integrations all place h1 and main/banner at x=272 y=24 with 976px frame width (961px with 15px scrollbar) and horizontal scroll: no. Phone 390x844: Home, Board, Teams, Invites, Tokens, Settings, and Instance dashboard all place h1 and main/banner at x=16 y=141 (16px below 125px top nav) with 358px frame width and horizontal scroll: no; Board .board keeps internal horizontal lane scroll (358px box, 692px content) without document overflow.
```

### E4

met

#### Observations

```text
Home / at 1280x800: main.dash, h1, ul.card-grid, and both project card links (AUTH Identity Platform, GEN Sandbox) sit at x=272 w=976. At 1600x800: main, h1, ul.card-grid, and both cards remain at x=272 and expand to w=1296 (left edge stays fixed; no narrow centered column).
```

### E5

met

#### Observations

```text
Form field widths at 1280x800 inside 976px/961px frame: Teams input.field--name w=384; Invites input.field--long (email) w=576; Tokens input.field--name w=384 and input.field--short (number ttl_days=90) w=128; Instance dashboard 4x input.field--name w=384; Instance integrations 3 visible inputs w=384. Outside frame / in dialogs: signed-out /sign-in card form w=390 with email and password inputs w=390; issue edit dialog Title, Description, Status all w=430.
```

### E6

not_examined

```text
Select and number controls verified in light and dark palettes (O4), but file control (<input type=file>) was not observed within the 3-probe budget because selector "dialog input[type=file]" matched 0 elements.
```

### E7

met

#### Observations

```text
Teams /teams has 2 section.page-section blocks (Create a team y=149..266 with input y=189 and Create team button y=229; Default team for new members y=306..419 with select y=345 and Save button y=382; no orphaned Save). Instance dashboard /admin/instance/workspaces has 6 section.page-section blocks (Workspaces, 3 rename sections at y=166/327/497, Provision a workspace y=697, Grant super-admin y=894), each with its own heading and submit button under its fields; ul padding=0px at x=272 and li.instance-row padding=10.4px 0px border=1px solid rgb(227, 229, 224) (no UA indent). Account settings /account/settings has 2 section.page-section blocks (Workspaces with Mute, Project emails with Save) and 2 fieldset.choice-group elements with padding=0px border=0px none radius=0px (no raw UA fieldset box).
```

### E8

met

#### Observations

```text
Teams /teams has 2 section.page-section blocks (Create a team y=149..266 with input y=189 and Create team button y=229; Default team for new members y=306..419 with select y=345 and Save button y=382; no orphaned Save). Instance dashboard /admin/instance/workspaces has 6 section.page-section blocks (Workspaces, 3 rename sections at y=166/327/497, Provision a workspace y=697, Grant super-admin y=894), each with its own heading and submit button under its fields; ul padding=0px at x=272 and li.instance-row padding=10.4px 0px border=1px solid rgb(227, 229, 224) (no UA indent). Account settings /account/settings has 2 section.page-section blocks (Workspaces with Mute, Project emails with Save) and 2 fieldset.choice-group elements with padding=0px border=0px none radius=0px (no raw UA fieldset box).
```

### E9

met

#### Observations

```text
Teams /teams has 2 section.page-section blocks (Create a team y=149..266 with input y=189 and Create team button y=229; Default team for new members y=306..419 with select y=345 and Save button y=382; no orphaned Save). Instance dashboard /admin/instance/workspaces has 6 section.page-section blocks (Workspaces, 3 rename sections at y=166/327/497, Provision a workspace y=697, Grant super-admin y=894), each with its own heading and submit button under its fields; ul padding=0px at x=272 and li.instance-row padding=10.4px 0px border=1px solid rgb(227, 229, 224) (no UA indent). Account settings /account/settings has 2 section.page-section blocks (Workspaces with Mute, Project emails with Save) and 2 fieldset.choice-group elements with padding=0px border=0px none radius=0px (no raw UA fieldset box).
```

### E10

not_examined

```text
Tab order verified skipping Delete (O6), but Delete vs Save visual styling (computed fill/border/ink) was not captured within the 3-probe budget because selector "dialog button" matched 0 elements.
```

### E11

not_examined

```text
Pressing Delete was observed to show confirmation controls "Delete AUTH-1 permanently" and "Close" (O6), but declining via "Close" and verifying AUTH-1 remains after reload was not completed within the 3-probe budget (probe attempted "Cancel").
```

### E12

met

#### Observations

```text
Board header.project-header (x=272 y=24 w=976 h=45 display=flex) shows h1 "Identity Platform" and p.project-key (x=539 y=31 display=flex) with separate span.project-key__label "Key prefix" (x=539 w=71, muted rgb(92, 100, 95)) and span[data-key-prefix] "AUTH" (x=618 w=36, JetBrains Mono, 8px gap); Change report header also renders span[data-key-prefix] "AUTH" in JetBrains Mono. Board nav renders a.report-link.button.secondary "Change report" (x=272 y=81 w=126 h=37) and a.integrations-link.button.secondary "Integrations" (x=406 y=81 w=110 h=37) with identical secondary button styling (border=1px solid rgb(92, 100, 95), radius=6px, fill=rgb(255, 255, 255), padding=6px 12px) and 8px gap; on phone 390x844 both render at h=44 (x=16 w=126 and x=150 w=110, 8px gap).
```

### E13

met

#### Observations

```text
Board header.project-header (x=272 y=24 w=976 h=45 display=flex) shows h1 "Identity Platform" and p.project-key (x=539 y=31 display=flex) with separate span.project-key__label "Key prefix" (x=539 w=71, muted rgb(92, 100, 95)) and span[data-key-prefix] "AUTH" (x=618 w=36, JetBrains Mono, 8px gap); Change report header also renders span[data-key-prefix] "AUTH" in JetBrains Mono. Board nav renders a.report-link.button.secondary "Change report" (x=272 y=81 w=126 h=37) and a.integrations-link.button.secondary "Integrations" (x=406 y=81 w=110 h=37) with identical secondary button styling (border=1px solid rgb(92, 100, 95), radius=6px, fill=rgb(255, 255, 255), padding=6px 12px) and 8px gap; on phone 390x844 both render at h=44 (x=16 w=126 and x=150 w=110, 8px gap).
```
