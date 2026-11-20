# Product brief: nav\-quick\-actions

## Request

```text
Move the quick actions to the left nav and make the left nav collapsible.
```

## Work

M

```text
An operator reaches Teams, Invite a member and Machine tokens from the left nav on every app page, as she reaches Home and Board, instead of only from the Home page
```

### Rework

None.

## Promises

### P1

Outcome

```text
On every app page the left nav offers Teams, Invite a member and Machine tokens, placed after Board in the top part of the nav, so the operator opens each of them in one press from wherever she is.
```

### P2

Outcome

```text
The Home page no longer shows a Quick actions section; each of its links lives in the left nav only (Integrations, Keyboard shortcuts and Sign out are already there), and Home keeps its greeting and Your projects.
```

### P3

Outcome

```text
An operator sees each moved link in the nav exactly when she saw it in Home's Quick actions before: the move changes where a link is, never who may see it.
```

### P4

Outcome

```text
With the left nav folded into its icon rail, Teams, Invite a member and Machine tokens are each an icon that names its destination and opens it in one press, like every other rail item, and at 480 px and below no page scrolls sideways.
```

## Scope

```text
Teams, Invite a member and Machine tokens added to the left nav after Board, with rail icons; Home's Quick actions section removed.
```

```text
Making the left nav collapsible: delivered by nav-collapse-toggle and nav-collapse-icon-rail in v0.19.0. Any change to who may see a link. Any new destination page.
```

## Decisions

```text
User decision 2026-11-19: remove Home's Quick actions section once its links are in the nav.
```

#### Status

Confirmed


```text
User decision 2026-11-19: Teams, Invite a member and Machine tokens go in the top part of the nav, after Board; Settings, Keyboard shortcuts and Sign out stay in the bottom group.
```

#### Status

Confirmed


```text
User decision 2026-11-19: each moved link is visible to exactly whoever sees it in Quick actions today; no rights change.
```

#### Status

Confirmed


## Journey

None.

## Slices

### S1

```text
Signed in, the operator opens Teams, Invite a member and Machine tokens from the left nav on a board page, in the open nav and in the folded rail; Home shows no Quick actions; a member who could not see a link before still cannot.
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
