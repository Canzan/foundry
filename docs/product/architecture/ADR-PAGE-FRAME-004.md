# PAGE\-FRAME\-004 Field cap belongs to the frame
Status: Accepted

## Context

```text
ADR-PAGE-FRAME-002 said an unclassed field takes the name size, and said it without a scope. The rule it governs reaches 22 templates, and three of those places are not framed forms: the signed-out card pages of ui-bare-pages, where a 28rem .page-card less its 1.75rem padding leaves about 24.5rem of content, so a 24rem cap would visibly narrow every one of them, and the dialogs, where .modal-dialog is width: min(30rem, 92vw) with 1.5rem padding, so a capped title field would stop short while its description textarea ran the full dialog width. The design review found that gap before any byte was written.
```

## Decision

```text
The three size classes work WHEREVER THEY ARE NAMED. The UNCLASSED DEFAULT CAP APPLIES ONLY INSIDE .app-shell__content: inside a framed app form an unclassed field is sized at the name cap and is never stretched, and OUTSIDE the frame a field keeps inline-size: 100 percent with NO cap. That is the rule for the signed-out card pages, which must not change, and for a dialog field, whose title input must stay level with the description textarea beside it. textarea keeps the full-width box everywhere. The cap is still presentation only and never refuses. The control family rules, select and input type=number taking the Foundry border, fill, radius, face and minimum height, stay GLOBAL on purpose, so a list in a dialog is drawn like a list in a form: that reaches the issue edit status list and the lane-delete destination list, deliberately, without changing a name, a value or an option order. Everything else ADR-PAGE-FRAME-002 decided is carried unchanged: the three sizes, the presentation-only cap, and the platform arrow kept rather than a drawn chevron below the token seam.
```

## Alternatives

```text
Leave ADR-PAGE-FRAME-002 as it stood and hold the scope in the design record alone: refused, because the durable rule is what a later reader follows, and an unscoped sentence invites exactly the two defects the review named. One global default cap: refused, it narrows every signed-out card page and stops a dialog title field short of its own textarea. Scope the control family to the frame as well: refused, because a platform list is drawn by the platform wherever it sits, and a list in a dialog would then stay the odd one out.
```

## Consequences

```text
The cap is a property of the FRAME, not of the input, and that is now written where a later reader will look. Moving the default cap out of .app-shell__content reintroduces two defects at once, a narrowed card page and a dialog field that no longer matches its textarea, so the oracle reads a signed-out card-page field and a dialog title field against its textarea for exactly that reason. A template inside the frame may still name no class, so the default size stays load-bearing there.
```
