# Product brief: nav\-collapse\-toggle

## Request

```text
Add a left nav toggle - The left nav should be collapsible and will stay collapsed until expanded again by the user.
```

## Work

S

```text
An operator can collapse the left nav to give the page more room, and it stays collapsed across pages and reloads until the operator expands it again.
```

### Rework

None.

## Promises

### P1

Outcome

```text
An operator can collapse the left nav with a toggle control in the app shell, and the page content then uses the width the nav gave up.
```

### P2

Scenario

```text
Collapsed stays collapsed
```

```text
the operator collapsed the left nav
```

```text
the operator reloads the page or opens another page in the app shell in the same browser
```

```text
the left nav is still collapsed, with no flash of the expanded nav, until the operator expands it with the toggle
```

### P3

Scenario

```text
Expand again
```

```text
the left nav is collapsed
```

```text
the operator activates the toggle
```

```text
the left nav is expanded again, and it stays expanded across reloads and pages
```

### P4

Outcome

```text
The toggle works with the keyboard and states in its accessible name or state whether the nav is expanded or collapsed.
```

### P5

Outcome

```text
With JavaScript disabled, the left nav shows expanded and no toggle shows, as before this feature.
```

### P6

Outcome

```text
At phone width (480 px and below) the page has no horizontal scroll with the nav expanded or collapsed.
```

## Scope

None.

```text
Syncing the nav choice across browsers, devices or accounts.
```

## Decisions

```text
The collapsed or expanded choice is kept per browser, the same way the theme choice is kept, not per account or across devices.
```

#### Status

Confirmed


## Journey

None.

## Slices

### S1

```text
In the app shell, the operator collapses the left nav with a toggle; it stays collapsed across reloads and pages until the operator expands it again.
```

#### Promises

P1

P2

P3

P4

P5

P6

#### Depends on

None.

#### Slice work

None.
