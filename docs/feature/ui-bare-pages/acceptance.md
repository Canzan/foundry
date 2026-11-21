# Acceptance brief: ui\-bare\-pages

## Slices

None.

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

None.
