# NAV\-COLLAPSE\-001 Collapse state as a boolean behind a valueless marker
Status: Accepted

## Context

```text
The rail must collapse, stay collapsed across reloads and pages, show no flash of the expanded rail, work from the keyboard, and vanish entirely with scripting off. The app already solves the same shape once, for the theme choice: a render-blocking head script stamps the document before first paint, builds its own control, and keeps the choice in localStorage with every access guarded. That asset is a byte-identical port of another app and its own header forbids teaching it a second job. This declaration replaces the same decision recorded under a bare numeric key, which would collide with the first ADR of the next feature; the repository keys its ADRs per feature.
```

## Decision

```text
A new render-blocking head script owns the collapse state. The per-document state is a JavaScript boolean; the document marker is the VALUELESS attribute data-nav-collapsed and the stylesheet selects on its presence; the stored form is the single sentinel "collapsed" under localStorage key foundry.nav, with the expanded state being the ABSENCE of the key. The collapsed rail is a narrow control strip: the navigation items, the workspace name and the user links take display none, while the monogram, the collapse toggle and the theme control stay reachable. The content box needs no rule, because it is already a flexible column.
```

## Alternatives

```text
A server-rendered toggle with the choice in a cookie was rejected: the control would be present but dead with scripting off, and a dead control is worse than none. Teaching theme.js the second state was rejected: it is a frozen port whose divergence is the thing the port exists to avoid. A state-carrying attribute with values light, dark style was rejected: a boolean plus a valueless attribute makes a third rail geometry unrepresentable instead of merely unlikely. A zero-width rail with a floating toggle over the content was rejected: it puts the control outside the rail and leaves the operator with no visible place to get the nav back. Hiding the whole user block when collapsed was rejected: it would make the shipped theme control unreachable.
```

## Consequences

```text
Three defect classes cannot be built rather than being tested for: a third geometry, a stored word for expanded, and a control with no script behind it. The pre-paint tag becomes the single point of failure, so the no-defer law moves from a template comment into the architecture gate, where it governs every pre-paint script and not only the two shipped today. The stylesheet gains rules, so its content-hash filename and its three references move in the same change. Two open tabs can disagree until each navigates, which the product scopes as acceptable today. The pure core is JavaScript and the repository has no JS test runner, so each law executable projection is a browser-lane observation rather than a default-suite unit test; that lane only runs when Docker is included.
```
