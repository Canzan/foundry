# Acceptance brief: ui\-bare\-pages

## Slices

### S1

##### Covers

P1

```text
The same shortcut address is asked for once as a page and once as a script fetching it, and the shortcuts each answer advertises are read out of both.
```

```text
Both answers advertise exactly the seven shipped shortcuts in the shipped order, and they advertise the identical rows as each other.
```

```text
The operator learns one set of keys, whichever way she reads it, and a key she is told about is a key she can press.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: The page and the overlay advertise the same shortcuts

```text
Both bodies keep advertising a key the client no longer binds, because what a key does in the browser is read by the shipped keyboard lane.
```

###### Obligations




##### Covers

P1

```text
The operator follows the nav to the keyboard shortcut list at desk width, and opens the same list on a 390 by 844 phone.
```

```text
Every shortcut key is painted on the same line as what it does, beside it rather than above it, and the instruction about choosing a card is drawn below the whole list.
```

```text
She reads each key and its meaning as one pair, on a desk and on a phone, instead of matching a key to a line below it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: Opening the shortcut list from the nav\, each key sits beside what it does\; crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: On her phone each shortcut key still sits beside what it does

```text
The pairs sit on one line in a type that is not the application's own, because the look an operator judges by eye is examined rather than measured.
```

###### Obligations

None.

##### Covers

P2

```text
Signed out, with no session at all, the operator looks at sign-in, the forgotten-password page, the check-your-email page she is sent to, the first-admin claim, the invitation page where she sets her password, the page where she chooses a new password, and the public shortcut list, each at desk width and each on a 390 by 844 phone.
```

```text
Each one is a whole document whose own stylesheet the browser resolved, drawing exactly one painted card around what she came for, with the product name above the page's single heading inside that card, centred with a margin she can see at desk width and filling the width inside the page padding on the phone, nothing running off the side of the screen, and no navigation rail and no palette control anywhere on it.
```

```text
Every page she can land on signed out reads as one family of Foundry pages that name the product, instead of a form stretched across the window.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: Signed out\, what she came for sits in one centred card under the product name

```text
The card is centred and named at the two widths read here while a third width between them draws it differently, because only these two are measured.
```

###### Obligations








##### Covers

P2

```text
The operator looks at the sign-in page on a screen that asks for the dark palette.
```

```text
The painted card and the product name above the heading are there as they are in the light palette, and the page still offers no navigation rail and no palette control.
```

```text
An operator whose screen is dark gets the same named card as everyone else, not a page whose frame disappeared.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: The sign\-in card holds on a screen that asks for the dark palette

```text
The card is drawn in a colour that is legible nowhere, because the palette's own contrast is read by the shipped theme lane.
```

###### Obligations


##### Covers

P4

```text
A visitor opens sign-in, the first-admin claim, the invitation page where she sets her password, the page where she chooses a new password, and the forgotten-password page, and the intent each named field declares is read off the served page.
```

```text
The sign-in email declares her username and its password her current password; the claim email declares her username and its password a password she is creating; both fields of the invitation page and both of the password-reset page declare a password she is creating; and the forgotten-password email declares nothing.
```

```text
Her password manager fills the sign-in form she returns to, and offers to generate and save a new password where she is creating one, instead of offering her an old secret or nothing at all.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: The browser is told what each sign\-in and password field holds

```text
A particular password manager ignores what the page declares, which no page can control and no reading here can see.
```

###### Obligations








##### Covers

P3

```text
A member opens the full page of her own issue, once for each place an issue can sit, the issue carrying a title and a description written over two lines.
```

```text
The page is headed by the issue key alone, shows the stored title in a heading of its own, shows the stored description whole with its line breaks and as text rather than markup, and shows exactly one status chip of the shipped shape whose marked state and whose word are both the stored state.
```

```text
She reads the issue on its own page: what it is called, what it says, and where it sits, without opening a dialog or going back to the board.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: An issue\'s own page shows its key\, its title\, what she wrote and where it sits

```text
The chip shows the stored state as the stored word rather than a human label, which is the decision of 2026-11-21 and not a defect.
```

###### Obligations




##### Covers

P3

```text
A member opens the page of an issue nobody has described yet, beside the page of one that has a description.
```

```text
Each page marks exactly one place as the description; the place on the described issue holds that text, the place on the undescribed issue carries copy rather than being blank or absent, and on both pages the element her words sit in carries a class the stylesheet can reach.
```

```text
She can tell an issue nobody has described from a page that failed to show her the description.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: An issue with nothing written down still has one place for it

```text
The empty copy is worded differently from the attachment and comment empty states, because the wording itself is not pinned.
```

###### Obligations



##### Covers

P3

```text
Descriptions of every shape are written as the operator would write them, from a recorded seed and always including nothing at all, something that looks like a script, two lines ended the way a browser ends them, and one unbroken word of four thousand letters; each issue's own page is then opened.
```

```text
Every page shows that description whole, with its line breaks kept, with every character of it drawn as text and nothing of it as markup, and with no live script of hers anywhere in the document.
```

```text
Whatever she writes in a description is what she reads back, however long it is and whatever it looks like, and nothing she writes can run in anyone's browser.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature\:\:Whatever she wrote in the description is shown as she wrote it

```text
A description wider than the page is wrapped rather than cut, because no authority bounds its length and no pixel bound is read here.
```

###### Obligations






##### Covers

P3

```text
A member follows Delete from her issue's own page.
```

```text
Exactly one Delete control is offered, it carries the address that is its whole mechanism, and following that address answers the page that asks her to confirm before anything is deleted.
```

```text
Styling the control does not cost her the way out of it: one press still asks her first, with scripting on or off.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: Deleting from an issue\'s own page still asks her to confirm

```text
The confirm page itself is restyled, because this reading is about reaching it.
```

###### Obligations


##### Covers

P3

```text
A member looks at her issue's own page in a real browser.
```

```text
Delete is painted as one of the application's own controls, with room around its word and a surface or an edge of its own rather than a bare underlined link, and the control for attaching a file is painted in the same type as the page around it.
```

```text
The controls beside her issue look like they belong to Foundry, not like the browser's own leftovers.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: The other controls on an issue\'s own page are drawn like Foundry\'s own

```text
The controls are styled in a way an operator would still call ugly, because looking right is examined by eye and not measured here.
```

###### Obligations


##### Covers

P1

```text
The shipped palette sweep walks the board, the dashboard, an issue, the shortcut list and the sign-in screen in both palettes.
```

```text
The overlay still draws its keycaps and its muted meanings over the scrim, and the sign-in screen still carries no rail and no palette-named control.
```

```text
The list she opens over her board looks exactly as it did, and the new card frame costs the sign-in screen nothing.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature

```text
The overlay keeps its colours while the page arm draws different ones, which the shortcut-page readings catch instead.
```

###### Obligations


##### Covers

P1

P2

P3

```text
The repository's own architectural gate is run over the application tree.
```

```text
Every static reference resolves on disk and every content-hashed filename is the sha256 prefix of its own bytes; no colour literal lives outside the three stylesheet token regions and those regions declare the identical token set; the pre-paint head scripts carry none of the deferring attributes and every other head script defers.
```

```text
The restyled pages reach her with a stylesheet that is actually there, in the palette tokens the rest of the application uses, and with no flash of the wrong palette on the way.
```

##### Disposition

Tested

cargo xtask check\-arch \:\: asset\-reference\, asset\-hash\, colour\-token and pre\-paint\-script layers

```text
A stylesheet rule is wrong in a way no gate parses, because the file has no linter and no CSS parser in the gate.
```

###### Obligations




##### Covers

P1

```text
The operator with a session of her own opens the keyboard shortcut list as a page, and the answer's own headers are read with it.
```

```text
She is given the whole of Foundry around the list: a full styled document carrying the rail and something to sign out with, exactly one place in the rail marked as where she is and that place her home, the shortcut section a reader with no scripting finds and nothing announcing the page as a dialog, an answer that names both what the client declared and who asked as what it depends on, and an answer no cache may keep a copy of.
```

```text
Opening the shortcut list leaves her inside Foundry, with the rail she came from and a way back, and no cache can show her page to whoever reads that screen next.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: Signed in\, she opens the shortcut list and keeps the rest of Foundry around it

```text
The rail offers a destination that answers an error, because this reading holds the marking and the way out rather than every link's answer.
```

###### Obligations




##### Covers

P1

```text
She opens the keyboard shortcut list as a page with a session of her own, and then presses the sign-out of that page.
```

```text
The page carries exactly one anti-forgery token in its sign-out form, that token is the one in the cookie the same answer minted, the press is accepted, and her old session opens her workspace no more.
```

```text
She can leave from the shortcut page itself, instead of finding that the one control on it is always refused.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: She can sign out from the shortcut page she opened signed in

```text
A second press of the same spent token is refused, because this reading follows one sign-out rather than the reuse of its token.
```

###### Obligations


##### Covers

P1

```text
Her session stays open while her place in the workspace is removed, and she then opens the keyboard shortcut list as a page.
```

```text
She is given the centred card: a full styled document with no rail and no workspace named anywhere on it, never a refusal and never a frame greeting a nameless identity, carrying the shortcut rows, and an answer that names both what was declared and who asked and that forbids any cache from keeping it.
```

```text
A reader whose account has gone can still read the shortcut list, and is never shown a workspace that is no longer hers.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: A reader whose account is gone is given the card\, not a refusal and not a nameless frame

```text
The card is reached through a redirect rather than answered straight away, because this reading holds the body she ends on.
```

###### Obligations


##### Covers

P1

```text
The same shortcut address is asked for as a script fetching it, as a frame loading it, and declaring nothing at all.
```

```text
Each is given the bare list a live page writes into itself, with its labelled dialog container and no document wrapper, each answer names both the declared destination and the asking reader as what it depends on, and none of them forbids a cache from keeping it.
```

```text
The list she opens over her board keeps working and stays quick, and no cache can hand it a whole page instead.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: A client that is not opening a page is still given the bare list

```text
The overlay's own paint changes, because this reading holds the body shape and the rows rather than the look.
```

###### Obligations






##### Covers

P4

```text
Every page this work touches is served: the four forms the promise names, the forgotten-password page, the public shortcut list and an issue's own page.
```

```text
Exactly eight of their fields are told what they hold, and no other field on any of those pages is told anything at all.
```

```text
Her password manager is offered the eight fields that hold her credentials, and is taught nothing about any other box she types in.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: No field beyond those eight claims to hold her credentials

```text
A page outside this work gains a token, because the count is taken over the pages this work touches.
```

###### Obligations


##### Covers

P3

```text
A member looks at her issue's own page in a real browser, once on a screen that asks for the light palette and once on a screen that asks for the dark one.
```

```text
The status is painted on the application's own resting surface, opaque rather than see-through, with a hairline edge and a rounded corner, and its word reaches the ordinary readable contrast against that surface; the button that chooses a file is painted on the application's own raised surface with its quiet boundary, its word reaches the same readable contrast, and its edge stays visible against what is behind it. Both hold in each palette.
```

```text
Where her work stands is a chip she can read at a glance, and attaching a file is a control that belongs to Foundry, on a light screen and on a dark one.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: Whichever palette her screen asks for\, the status and the attach control are readable

```text
The chip's corner is a different size from the keycap's, because this reading holds that it has one rather than which one.
```

###### Obligations




##### Covers

P1

```text
With no session at all she opens the keyboard shortcut list as a page, and the answer's own headers are read with it.
```

```text
She is given the centred card with no rail and no workspace named, carrying the shortcut rows and the shortcut section a reader with no scripting finds, with nothing announcing the page as a dialog, an answer naming both the declared destination and the asking reader as what it depends on, and no cache allowed to keep it.
```

```text
An operator stuck at the sign-in screen can still open the shortcut list and read it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature \:\: With no session at all the shortcut list is still a page she can read

```text
The card is drawn at a width this reading does not measure, because the painted frame is read by the browser scenarios instead.
```

###### Obligations




## Expectations

### Rows

#### E1

##### Promises

P1

```text
Opening the keyboard shortcuts page, the operator sees a page that looks like the rest of Foundry: the same page frame, heading, typography and colours as other Foundry pages, not an unstyled browser-default list.
```

```text
A user sees only the painted page. An oracle that asserts a stylesheet, class name or markup container cannot show that the rendered page actually matches the look of other Foundry pages, and a styled page can still differ in a way only the eye catches.
```

None.

None.

```text
The shortcuts page reached the way the operator reaches it, plus one other Foundry page captured at the same width for comparison.
```

#### E2

##### Promises

P1

```text
On the keyboard shortcuts page each shortcut key sits on the same line as the action it performs, so the operator reads key and meaning as one pair and never has to match a key to a description on another line.
```

```text
An oracle can confirm a one-row layout at a single window width. It does not see the pair break onto two lines at a narrower width, with a long action description, or at a large browser font, which is exactly when the operator loses the pairing.
```

None.

None.

```text
A capture of the full shortcut list at desk width and at phone width, showing several key/action pairs including the longest description.
```

#### E3

##### Promises

P2

```text
Signed out at desk width, the sign-in form appears inside one card that is centred in the window with the product name above it, and the form does not stretch across the full window width.
```

```text
The promise does not say how wide the card is or how the product name is shown, so 'centred' and 'not stretched' stay a matter of judgement. An oracle that checks a container element or a CSS class cannot show the painted width, the centring, or that the product name is visible above the card.
```

None.

None.

```text
A full-window capture of the signed-out sign-in page at desk width.
```

#### E4

##### Promises

P2

```text
The first-admin claim page, the invite accept page and the password reset page each show their own form in the same centred card under the product name as sign-in, so every signed-out page the operator can land on looks like one family.
```

```text
An oracle can test sign-in alone and assume the other three follow, because they share a template. It does not show the other three pages at all, and reaching them needs an unclaimed install, a live invite and a reset link that a capture may be unable to create.
```

None.

None.

```text
One capture per page: first-admin claim on an unclaimed install, invite accept from a pending invite, password reset from a reset link.
```

#### E5

##### Promises

P2

```text
At phone width every signed-out page still shows its form in one centred card under the product name, with all fields and the submit control fully on screen and no sideways scrolling.
```

```text
The promise names 'phone width' without a size. An oracle may assert at one emulated viewport and report pass while, at another common phone width, the card is clipped, the product name is cut off, or the page scrolls sideways.
```

None.

None.

```text
Captures of the signed-out pages at a narrow phone width, showing the whole card and whether the page scrolls sideways.
```

#### E6

##### Promises

P3

```text
Opening an issue's own full page, the operator reads the issue's key, its title, its description text and its status on that page, without opening a dialog or going back to the board.
```

```text
An oracle can confirm that four labelled places exist on the page. It does not show that each one holds this issue's stored value, so a page that shows the key and title but an empty or stale description, or a status that belongs to another issue, can still pass.
```

None.

None.

```text
A capture of the full page of a known issue whose key, title, description and status are known in advance, including one issue with a long multi-line description.
```

#### E7

##### Promises

P3

```text
The status on the issue full page is shown as the same status chip the operator already knows from the rest of Foundry, carrying this issue's stored status value.
```

```text
The user decision of 2026-11-21 keeps the stored status value rather than a human label, so a raw value such as 'in_progress' is expected here and an oracle cannot tell a correct raw value from a wrong-but-raw one without the issue's known status.
```

None.

None.

```text
A capture of the chip for an issue whose stored status is known, and a capture of the same chip elsewhere in Foundry for comparison.
```

#### E8

##### Promises

P3

```text
The other controls on the issue full page, its delete control and its attachments area, look like Foundry's own controls, not like unstyled browser defaults.
```

```text
'Styled like the rest of Foundry' is a judgement a user makes by eye, and the promise names no reference control. An oracle that checks class names or button markup shows nothing about appearance, and the promise covers only the look, not what delete or attachments do.
```

None.

None.

```text
A capture of the delete control and the attachments area on the issue full page, with a comparable Foundry control for reference.
```

#### E9

##### Promises

P4

```text
On the sign-in page the browser or password manager offers the operator her stored email and her existing password for that site, and after a successful sign-in it offers to save them.
```

```text
Whether a manager offers to fill or save depends on the browser, its profile and its stored entries, which no capture of the page controls. An oracle can read the fields' autocomplete intent but cannot show a real offer to fill or a real save prompt.
```

None.

None.

```text
A real browser with a password manager, one stored credential for the site, and a capture of the fill suggestion and of the save prompt after sign-in.
```

#### E10

##### Promises

P4

```text
On the first-admin claim page the manager treats the email and the password field as a brand-new credential: it offers a new generated password rather than suggesting an existing one, and offers to save the result.
```

```text
The difference between a new and an existing password is invisible on the page itself; the operator only learns it from what the manager suggests. An oracle that reads an attribute cannot show the manager's suggestion, and a wrong intent leaves the page looking identical.
```

None.

None.

```text
A real browser with a password manager on an unclaimed install, with a capture of what it suggests in the password field and of the save prompt.
```

#### E11

##### Promises

P4

```text
On the invite accept page and on the password reset page the manager treats the new password and its confirmation as one new credential: it can fill both fields with the same new password and offers to save that one password, not two.
```

```text
Fill-and-match across two fields and the single save offer live in the manager, not on the page, so a capture of the page shows neither. The operator sees nothing wrong until the manager fills the wrong value or saves the confirmation separately.
```

None.

None.

```text
A real browser with a password manager on a pending invite and on a reset link, with a capture of both fields after fill and of the save prompt.
```

## Charter

None.

## Examination

### E1

met

#### Observations

```text
Keyboard shortcuts page, signed in as Priya, browser navigation at 1280x800: the page arrives in the normal application frame. Landmarks complementary x=0 w=240 y=0 h=800, navigation "Primary" x=12 w=215 y=78 h=183 with the rail links Home (current=page, the only one marked), Board, Teams, Invite a member, Machine tokens, all painted and seen; buttons "Theme: following your device", "Left nav: expanded. Collapse the left nav." and "Sign out"; contentinfo footer x=0 w=1280 y=784. The same viewport on /team/backend/project/identity-platform and on the issue page /team/backend/project/identity-platform/issues/1 reports the identical frame (complementary x=0 w=240 h=800, the same Primary nav at x=12 w=215, the same footer), so the shortcuts page sits in the same frame as other Foundry pages and is not a bare default list. Its own text reads: heading "Keyboard shortcuts", then c Create issue, / Search, j Next, k Previous, Enter Open selected, ? Show this help, Esc Close modal, then the screen-reader instruction. No horizontal scroll and no box past the right edge.
```

```text
Keyboard shortcuts page, signed out (no session), browser navigation at 1280x800: landmarks main x=416 w=448 y=48 h=463 and contentinfo only; no navigation landmark, no rail link, no sign-out button, no theme control, no horizontal scroll. That is the same centred-card geometry (main x=416 w=448) the signed-out /sign-in page reports at the same viewport, so the signed-out arm of the shortcuts page belongs to the same signed-out card family.
```

```text
Keyboard shortcuts page, signed in as Priya, phone viewport 390x844: no horizontal scroll; the single box reaching past the right edge is the nav text "Machine tokens" (to x=404), and all five rail labels report words painted "not seen at its centre (clipped or covered)" in the 44px-wide collapsed rail. The project board page and the issue full page report exactly the same at the same viewport, so this is the application nav own phone behaviour on every signed-in page, not a trait of the shortcuts page.
```

### E2

not_examined

```text
Not provable from here, see O4 and O1. The only page reader this examination can run prints landmark, button and link boxes and painted words for nav links and the focused control; it prints no box or line geometry for non-interactive text, so it never shows whether a key and its action share one painted line, and shot writes images this examiner cannot read. The seven key and action pairs are present in the served text at desk and phone width with no sideways scroll, which does not decide the pairing. Owner: the evidence-capture owner, who must expose element-level geometry for the shortcut rows at desk width, at a narrow phone width, with the longest description and at a large browser font.
```

### E3

met

#### Observations

```text
All five signed-out pages, read as served and in the browser at 1280x800 and at the 390x844 phone, each from its own live precondition. /sign-in reads "Foundry" then "Sign in", Email, Password, Sign in, Forgot password?. /forgot-password reads "Foundry" then "Forgot password", Email, Send reset link. /bootstrap?token=cABfsVkp... on a fresh unclaimed install reads "Foundry" then "Claim your workspace", Email, Password, Display name, Workspace name, Claim. /invites/accept?id=01a2025f-19d7-7f61-93d2-1ab1f3697d65&sig=VH_z6deI... from an invite minted by POST /invites reads "Foundry" then "Set your password", "Set a password for your Canzan Labs workspace.", New password, Confirm password, Set password and sign in. /reset-password?token=VIw-fZlh... from the reset mail reads "Foundry" then "Choose a new password", New password, Confirm password, Set password. In the browser every one of the five reports exactly one main landmark at x=416 w=448 of a 1280px page at desk width, that is 416px of margin on each side and one third of the window, and x=16 w=358 of a 390px page on the phone; the footer sits below the card; no page has any navigation landmark or theme-named control; every page reports horizontal scroll: no, no inner box scrolling sideways and no box reaching past the right edge; and each submit button is visible inside the viewport (Sign in, Send reset link, Claim, Set password and sign in, Set password).
```

### E4

met

#### Observations

```text
All five signed-out pages, read as served and in the browser at 1280x800 and at the 390x844 phone, each from its own live precondition. /sign-in reads "Foundry" then "Sign in", Email, Password, Sign in, Forgot password?. /forgot-password reads "Foundry" then "Forgot password", Email, Send reset link. /bootstrap?token=cABfsVkp... on a fresh unclaimed install reads "Foundry" then "Claim your workspace", Email, Password, Display name, Workspace name, Claim. /invites/accept?id=01a2025f-19d7-7f61-93d2-1ab1f3697d65&sig=VH_z6deI... from an invite minted by POST /invites reads "Foundry" then "Set your password", "Set a password for your Canzan Labs workspace.", New password, Confirm password, Set password and sign in. /reset-password?token=VIw-fZlh... from the reset mail reads "Foundry" then "Choose a new password", New password, Confirm password, Set password. In the browser every one of the five reports exactly one main landmark at x=416 w=448 of a 1280px page at desk width, that is 416px of margin on each side and one third of the window, and x=16 w=358 of a 390px page on the phone; the footer sits below the card; no page has any navigation landmark or theme-named control; every page reports horizontal scroll: no, no inner box scrolling sideways and no box reaching past the right edge; and each submit button is visible inside the viewport (Sign in, Send reset link, Claim, Set password and sign in, Set password).
```

### E5

met

#### Observations

```text
All five signed-out pages, read as served and in the browser at 1280x800 and at the 390x844 phone, each from its own live precondition. /sign-in reads "Foundry" then "Sign in", Email, Password, Sign in, Forgot password?. /forgot-password reads "Foundry" then "Forgot password", Email, Send reset link. /bootstrap?token=cABfsVkp... on a fresh unclaimed install reads "Foundry" then "Claim your workspace", Email, Password, Display name, Workspace name, Claim. /invites/accept?id=01a2025f-19d7-7f61-93d2-1ab1f3697d65&sig=VH_z6deI... from an invite minted by POST /invites reads "Foundry" then "Set your password", "Set a password for your Canzan Labs workspace.", New password, Confirm password, Set password and sign in. /reset-password?token=VIw-fZlh... from the reset mail reads "Foundry" then "Choose a new password", New password, Confirm password, Set password. In the browser every one of the five reports exactly one main landmark at x=416 w=448 of a 1280px page at desk width, that is 416px of margin on each side and one third of the window, and x=16 w=358 of a 390px page on the phone; the footer sits below the card; no page has any navigation landmark or theme-named control; every page reports horizontal scroll: no, no inner box scrolling sideways and no box reaching past the right edge; and each submit button is visible inside the viewport (Sign in, Send reset link, Claim, Set password and sign in, Set password).
```

### E6

met

#### Observations

```text
Issue full page, signed in as Lena, /team/backend/project/identity-platform/issues/1, for an issue this examination created through the public New issue form with a known title and a known two-line description. The page reads: AUTH-1 (the key, as the heading), then "backlog Delete", then the stored title "Readable issue with a description", then the stored description "First line of the description. Second line with <script>alert(1)</script> inside it." - the angle brackets are painted as text and nothing is drawn as markup - then No attachments yet, Attach a file, Upload, No changes yet, No comments yet, Add a comment, Post. A Delete link points to .../issues/1/delete. The same page for an issue created with no description reads "Nothing written down yet." in that place. The board shows AUTH-1 in the Backlog lane, so the stored status is backlog and the page status text matches it. In the browser at 1280x800 the page carries the normal frame (complementary x=0 w=240 h=800, Primary nav, Sign out, footer) and buttons Upload x=272 w=75, Post x=272 w=57 and "Attach a file" x=359 w=313; at 390x844 the same controls are visible, no horizontal scroll and no box past the right edge. Selecting In-Progress in the board dialog Status combobox without saving left the page reading backlog.
```

### E7

not_examined

```text
Half provable only, see O6 and O7. The issue full page shows the stored status value for a known issue (backlog, matching the board lane), so the value part holds; whether it is drawn as the status chip the operator knows elsewhere is not decidable, because the reader gives no background, border, radius or contrast for that word and Foundry shows no second drawing of the chip to compare with: the board card carries no status text and the card dialog presents a Status combobox instead. Owner: the evidence-capture owner, who must expose computed style or contrast for the chip plus one reference chip elsewhere in Foundry.
```

### E8

not_examined

```text
Not provable from here, see O8. The reader reports the attachments area as a button named "Attach a file" at x=359 w=313 beside Upload, and Delete as a link to .../issues/1/delete, but prints no background, border, radius, colour or contrast for any of them and no reference Foundry control, so it shows nothing about how they are painted. The custom label hints at a styled control and is not a reading of its appearance. Owner: the evidence-capture owner, who must expose computed style or contrast for the delete control and the file-selector button together with a comparable Foundry control.
```

### E9

not_examined

```text
Not provable from here, see O9. What the promise names happens in the browser password manager, and the probe browser has no step to store a credential for the site, to open a profile carrying a saved entry, to type into a field, or to read a fill suggestion or a save prompt; the served-page reader prints the field input types (email, password) and no autocomplete token. The sign-in page itself is reached, served and painted as a card with its email and password fields. Owner: the evidence-capture owner, who must supply a real browser with a manager and one stored credential for the site and capture the fill suggestion and the save prompt.
```

### E10

not_examined

```text
Not provable from here, see O9 and O5. The first-admin claim page is reached on a fresh unclaimed install with a live token and shows its email, password, display name and workspace name fields in the centred card, but whether a manager offers a new generated password rather than an existing one is only visible in the manager, and no step of the probe browser stores a credential, opens a manager suggestion or reads a save prompt. Owner: the evidence-capture owner, who must supply a real browser with a manager on an unclaimed install and capture the password-field suggestion and the save prompt.
```

### E11

not_examined

```text
Not provable from here, see O9 and O5. Both pages are reached live - invite accept from an invite minted through POST /invites, and reset from a link in the mail double - and each shows a new password and a confirmation field in the centred card, but filling both fields with one value and a single save offer live in the manager; the probe browser cannot type, cannot carry a manager profile and reads no suggestion or save prompt. Owner: the evidence-capture owner, who must supply a real browser with a manager on a pending invite and on a reset link and capture both fields after fill and the save prompt.
```
