# Product brief: ui\-page\-frame

## Request

```text
clean up the ui (visual pass, group B page frame and forms plus D1 and D2). Agreed with the user: B1 one content width and spacing on every app page (today the left edge is 424 px on Home, 272 px on the board and invites, 292 px on Teams, tokens and settings); B2 inputs sized to their content and styled lists, number and file inputs (fields stretch about 1100 px wide beside tiny unstyled controls on Teams, tokens, invites, Matrix and instance admin); B3 section headings and spacing on stacked forms (the orphaned Teams Save button, the nested bullet lists of rename forms on instance admin, raw fieldsets on settings); D1 a quiet, not primary, Delete in the issue dialog; D2 a tidy board header (Key prefix: AUTH is plain text, Change report and Integrations are run-together underlined links).
```

## Work

L

```text
Every app page has the same frame, and its forms read as one ordered set of sections
```

### Rework

None.

## Promises

### P1

Outcome

```text
Every signed-in app page starts its content at the same left edge and the same top spacing beside the nav, at the same width, at desk width and on a phone.
```

### P2

Outcome

```text
Each text field on the app's forms is only as wide as what it holds, and every list, number and file control is drawn like Foundry's own controls, not the browser's defaults.
```

### P3

Outcome

```text
A page with several forms shows them as separate sections, each under its own heading and with its own action beside its fields: Teams, instance admin and settings no longer show orphaned buttons, nested bullet lists of forms or raw fieldsets.
```

### P4

Outcome

```text
In the issue dialog, Delete is a quiet control and not the main action, and it still asks her to confirm.
```

### P5

Outcome

```text
The board header shows the project key as a labelled value and Change report and Integrations as separate, clearly drawn controls, not run-together underlined links.
```

## Scope

```text
The app page frame (content left edge, top spacing, width), form field sizing and control styling, sectioned forms on Teams, instance admin and settings, the issue dialog Delete, the board header.
```

```text
The left nav, the signed-out pages, the shortcuts page and the issue full page (already delivered). The error pages without a card (spent invite, spent reset, bootstrap with no token). Any change to who may open a page or what a form does.
```

## Decisions

```text
User decision 2026-11-20: the UI clean-up covers groups A to D, delivered as three Requests; this one is group B plus D1 and D2.
```

#### Status

Confirmed


```text
User decision 2026-11-22: Home loses its centred 880 px column; its content starts at the common left edge and its cards span the frame width.
```

#### Status

Confirmed


```text
User decision 2026-11-22: the frame sets no maximum content width; every page shares the frame's content box, and each field is capped by its own size.
```

#### Status

Confirmed


```text
User decision 2026-11-22: the board header keeps the label 'Key prefix', with the key as a labelled value beside it.
```

#### Status

Confirmed


## Journey

None.

## Slices

### S1

```text
The operator moves between Home, the board, Teams, invites, tokens and settings and the content never jumps sideways; she reads each form section by its heading, with fields sized to their content; the issue dialog's Delete reads as secondary; the board header reads as one tidy row.
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

L

```text
EXAMINE repair for unmet E2 E3 E4 E7 on the page frame
```

##### Rework

```text
1
```

```text
EXAMINE unmet: E2 top-gap mismatch; E3 board sideways scroll at 390px; E4 Home cards do not span frame; E7 project rename rows lack own headings
```
