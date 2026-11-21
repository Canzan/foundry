# Product brief: ui\-bare\-pages

## Request

```text
clean up the ui (visual pass, group A bare pages plus D3). Agreed with the user: A1 style the keyboard shortcuts page; A2 give sign-in and the other signed-out pages a centred card with the product name; A3 show the issue's title, description and status on its full page; D3 give the six sign-in and password fields the right autocomplete.
```

## Work

M

```text
Every page the operator lands on looks like Foundry and tells her what it is for
```

### Rework

None.

## Promises

### P1

Outcome

```text
The keyboard shortcuts page has the same look as the rest of Foundry, and each key sits on the same line as what it does.
```

### P2

Outcome

```text
Sign in and every other signed-out page (first-admin claim, invite accept, password reset) show their form in one centred card under the product name, not stretched across the window, at desk and phone width.
```

### P3

Outcome

```text
An issue's full page shows its key, title, description and status, so the operator can read the issue there, and its other controls (delete, attachments) are styled like the rest of Foundry.
```

### P4

Outcome

```text
The browser and the password manager can fill and save the sign-in and password fields: the email and current password on sign-in; the email and new password on first-admin claim; the new password and its confirmation on invite accept and password reset.
```

## Scope

```text
Keyboard shortcuts page, signed-out pages, the issue full page, and autocomplete on the six sign-in and password fields.
```

```text
The page frame, form sizing and section headings, the issue dialog's Delete and the board header: the next Request (ui-page-frame). Any change to who may open a page or what a form does.
```

## Decisions

```text
User decision 2026-11-20: the UI clean-up covers groups A to D, delivered as three Requests; this one is group A plus D3.
```

#### Status

Confirmed


```text
User decision 2026-11-20: A1 style the shortcuts page, A2 centred card for signed-out pages, A3 the issue full page shows title, description and status, D3 autocomplete on the six password fields.
```

#### Status

Confirmed


```text
User decision 2026-11-21: the first-admin claim password field is a new password (autocomplete new-password), not a current one.
```

#### Status

Confirmed


```text
User decision 2026-11-21: the issue full page shows the shipped status chip with the stored status value; human status labels are a later change.
```

#### Status

Confirmed


## Journey

None.

## Slices

### S1

```text
The operator opens the keyboard shortcuts page and reads each key beside what it does; signed out, she sees sign-in as a centred card her password manager fills; she opens an issue's full page and reads its title, description and status.
```

#### Promises

P1

P2

P3

P4

#### Depends on

None.

#### Slice work

None.
