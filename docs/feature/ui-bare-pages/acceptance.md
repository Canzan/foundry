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

unmet

#### Observations

```text
Keyboard shortcuts page, reached as the operator reaches it. Signed in as Priya at 1440x800, a browser NAVIGATION to /keyboard-help returns a full styled document (Foundry stylesheet foundry.e770ce20.css, <h1>Keyboard shortcuts</h1>, main.page-card centred at x=496 w=448). Its rendered landmarks are only: main, contentinfo. "other links on the page: 0". The same browser at the same width on another Foundry page, /team/backend/project/identity-platform, shows: complementary (left rail) x=0 w=240 h=800, banner, navigation "Primary" with the links Home, Board, Teams, Invite a member, Machine tokens, plus the Theme and Sign out controls and 4 further links. The shortcuts page carries none of that frame and offers the operator no link at all, although she arrived from the "Keyboard shortcuts" item in that very rail. The signed-in and signed-out responses for /keyboard-help are the same 2044-byte document, so the rail is absent for a signed-in operator too.
```

```text
Shortcut rows, both response arms. A read that declares no destination, a read that declares Sec-Fetch-Dest: empty, and a navigation that declares Sec-Fetch-Dest: document all answer HTTP 200 with "vary: sec-fetch-dest". The two non-navigation arms return the bare fragment (section.keyboard-help, role=dialog, aria-label="Keyboard shortcuts", no html wrapper, 769 bytes); the navigation returns the full document. All three carry the identical <dl class="kb-shortcuts"> with the identical seven pairs in the identical order: c/Create issue, //Search, j/Next, k/Previous, Enter/Open selected, ?/Show this help, Esc/Close modal, followed by the same p.kb-selection-instruction.
```

product_violation

### E2

met

#### Observations

```text
Shortcut rows, both response arms. A read that declares no destination, a read that declares Sec-Fetch-Dest: empty, and a navigation that declares Sec-Fetch-Dest: document all answer HTTP 200 with "vary: sec-fetch-dest". The two non-navigation arms return the bare fragment (section.keyboard-help, role=dialog, aria-label="Keyboard shortcuts", no html wrapper, 769 bytes); the navigation returns the full document. All three carry the identical <dl class="kb-shortcuts"> with the identical seven pairs in the identical order: c/Create issue, //Search, j/Next, k/Previous, Enter/Open selected, ?/Show this help, Esc/Close modal, followed by the same p.kb-selection-instruction.
```

```text
Shortcut key/description pairing. The served rows are <dt>key</dt><dd>description</dd> pairs inside <dl class="kb-shortcuts">, and the stylesheet gives that list "display: grid; grid-template-columns: auto 1fr", so each dt sits in column 1 and its own dd in column 2 of the same grid row. The stylesheets only width-conditional blocks start at @media (max-width: 480px) further down and contain no .kb-shortcuts selector, so there is no narrow-width override. Rendered at 1440x800 and again at the emulated 390x844 phone: "horizontal scroll: no", "boxes that scroll sideways inside the page: none", "boxes that reach past the right edge (a clipped box is listed too): none"; the card is main x=496 w=448 at desk and main x=16 w=358 on the phone. Limitation: the rendered read reports landmarks and controls, not per-row boxes, so the one-line pairing is read from the grid definition plus the absence of any overflow or clipping, not from a measured row-by-row capture.
```

### E3

met

#### Observations

```text
Signed-out sign-in page at desk width. Served markup is one full document whose body holds <main class="page-card"> with <p class="page-card__brand">Foundry</p> above its own <h1>Sign in</h1> and the form inside the same card; no navigation rail and no theme control appear on it. The stylesheet gives .page-card "max-inline-size: 28rem; inline-size: calc(100% - 2rem); margin-inline: auto". Rendered at 1440x800: main x=496 w=448, so the card takes 448 of 1440 px with an equal 496 px margin on each side; horizontal scroll: no; the "Sign in" button is visible at x=525.
```

### E4

unmet

#### Observations

```text
The other signed-out form pages at desk width. /forgot-password (<h1>Forgot password</h1>), /reset-password with a live token (<h1>Choose a new password</h1>) and /invites/accept with a live id and signature (<h1>Set your password</h1>, "Set a password for your Canzan Labs workspace") each serve one full document with <main class="page-card"> and <p class="page-card__brand">Foundry</p> above their own single h1 and their form inside that card. Rendered at 1440x800 each shows main x=496 w=448 with no horizontal scroll and its submit button visible: "Send reset link", "Set password", "Set password and sign in".
```

```text
The first-admin claim page could not be reached. Every state the probe offers is an already-claimed install; starting a state with --before produced the same claimed install and no claim link; the mail double held only invite, reset and password-changed messages and never a /bootstrap link; and GET /bootstrap without a token answers HTTP 400 "Missing token" on a page that is not the card page. So no observation of "Claim your workspace" (the first-admin claim form), at desk or phone width, and none of what a password manager is offered there, was captured.
```

observation_gap

### E5

unmet

#### Observations

```text
The signed-out pages at phone width. At the emulated 390x844 phone, /sign-in (main x=16 w=358), /forgot-password (x=16 w=358), /reset-password with a live token (x=16 w=358) and /invites/accept with a live invite (x=16 w=358) each report "horizontal scroll: no", "boxes that scroll sideways inside the page: none", "boxes that reach past the right edge (a clipped box is listed too): none", and each submit control is listed visible. The card takes the screen inside a 16 px page padding on both sides.
```

```text
The first-admin claim page could not be reached. Every state the probe offers is an already-claimed install; starting a state with --before produced the same claimed install and no claim link; the mail double held only invite, reset and password-changed messages and never a /bootstrap link; and GET /bootstrap without a token answers HTTP 400 "Missing token" on a page that is not the card page. So no observation of "Claim your workspace" (the first-admin claim form), at desk or phone width, and none of what a password manager is offered there, was captured.
```

observation_gap

### E6

met

#### Observations

```text
An issue full page carries this issues stored values. Through the public port as a signed-in member, GET /team/backend/project/identity-platform/issues/2 for an issue whose values were written in advance shows <h1>AUTH-2</h1> (the key alone as the heading), <h2 class="issue-title">Audit the signing keys</h2>, and <p class="issue-description__text"> holding the stored text "First line of the description\nSecond line after a break <script>alert(1)</script> and a < & > in it" with the line break kept as a literal newline and every angle bracket escaped, so the script tag is read as text and not drawn as markup; the stylesheet gives that paragraph "white-space: pre-wrap". The status appears as <span class="state" data-state="backlog">backlog</span>. A second issue with nothing stored, AUTH-1, shows "Nothing written down yet." in the same one description place. No dialog and no return to the board is needed; the page is the ordinary app page with the left rail.
```

### E7

unmet

#### Observations

```text
The status on the issue page is not a chip and matches nothing else in Foundry. The value does follow the stored status: with the issue in Backlog the page serves <span class="state" data-state="backlog">backlog</span>, and after the issue is moved to In-Progress through its own public state endpoint the same page serves <span class="state" data-state="in_progress">in_progress</span>. But the shipped stylesheet foundry.e770ce20.css contains no rule for .state and no rule for [data-state] anywhere: every occurrence of the word "state" in it is either prose in a comment or the unrelated .empty-state. The span therefore paints as plain inline body text, with no chip shape, border, background or colour. And <span class="state"> appears on no other public page: not on the project board (its issue cards carry only key and title, the status being the lane), not on the project change report, and not in the boards issue dialog. The shipped CHANGELOG states "its status in the same chip the board uses"; the board uses no such chip.
```

product_violation

### E8

unmet

#### Observations

```text
The issue pages delete control is a Foundry control; its attachment control is the browser default. Delete is served as <a class="button secondary issue-delete" href=".../issues/2/delete">Delete</a>, and the stylesheet defines .button/.button.secondary fully (min-height 32px, 6px 12px padding, 1px border, radius, surface background, muted border), so it is drawn as one of Foundry own buttons; the Upload submit is a <button> covered by the shared button rule and is rendered visible at w=75. The attachment control itself is a bare <input type="file" name="file" required> whose only rules in the shipped stylesheet are "font: inherit; max-inline-size: 100%", above the stylesheets own comment: "The browser raw file control is the one thing on the issue page that does not belong to the application - it is drawn in the UA own face, beside her attachments." The rendered page lists it as button "Attach a file" w=313, the user agent widget. The surrounding section classes .attachments and .attachments-empty carry no rules at all.
```

product_violation

### E9

unmet

#### Observations

```text
What the served sign-in and password fields tell a browser. /sign-in serves <input type="email" name="email" autocomplete="username"> and <input type="password" name="password" autocomplete="current-password">. /reset-password with a live token serves <input type="password" name="password" autocomplete="new-password"> and <input type="password" name="confirm" autocomplete="new-password">. /invites/accept with a live invite serves the same pair of new-password fields. The email field on /forgot-password carries no autocomplete token. The first-admin claim page could not be reached, so its two tokens were not read.
```

```text
No password manager behaviour could be observed. The probe browser runs a fresh per-viewer profile with no stored credential for the site and no password-manager UI; its rendered read reports landmarks, buttons, links and focus only. No fill suggestion on /sign-in, no generated-password suggestion, no filling of both new-password fields with one value, and no save prompt after a successful sign-in or password set was captured on any page. The autocomplete tokens above are the intent the page declares, not an observed offer by a manager.
```

observation_gap

### E10

unmet

#### Observations

```text
The first-admin claim page could not be reached. Every state the probe offers is an already-claimed install; starting a state with --before produced the same claimed install and no claim link; the mail double held only invite, reset and password-changed messages and never a /bootstrap link; and GET /bootstrap without a token answers HTTP 400 "Missing token" on a page that is not the card page. So no observation of "Claim your workspace" (the first-admin claim form), at desk or phone width, and none of what a password manager is offered there, was captured.
```

```text
No password manager behaviour could be observed. The probe browser runs a fresh per-viewer profile with no stored credential for the site and no password-manager UI; its rendered read reports landmarks, buttons, links and focus only. No fill suggestion on /sign-in, no generated-password suggestion, no filling of both new-password fields with one value, and no save prompt after a successful sign-in or password set was captured on any page. The autocomplete tokens above are the intent the page declares, not an observed offer by a manager.
```

observation_gap

### E11

unmet

#### Observations

```text
What the served sign-in and password fields tell a browser. /sign-in serves <input type="email" name="email" autocomplete="username"> and <input type="password" name="password" autocomplete="current-password">. /reset-password with a live token serves <input type="password" name="password" autocomplete="new-password"> and <input type="password" name="confirm" autocomplete="new-password">. /invites/accept with a live invite serves the same pair of new-password fields. The email field on /forgot-password carries no autocomplete token. The first-admin claim page could not be reached, so its two tokens were not read.
```

```text
No password manager behaviour could be observed. The probe browser runs a fresh per-viewer profile with no stored credential for the site and no password-manager UI; its rendered read reports landmarks, buttons, links and focus only. No fill suggestion on /sign-in, no generated-password suggestion, no filling of both new-password fields with one value, and no save prompt after a successful sign-in or password set was captured on any page. The autocomplete tokens above are the intent the page declares, not an observed offer by a manager.
```

observation_gap
