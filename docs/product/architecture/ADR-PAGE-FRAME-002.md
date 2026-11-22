# PAGE\-FRAME\-002 Three field sizes\, never full width
Status: Superseded by PAGE-FRAME-004

## Context

```text
The shipped rule gave every text, email and password input and every textarea width: 100 percent, so a team name field stretched about 1100px beside a tiny unstyled number field, and the list, number and file controls resolved no Foundry rule at all and were drawn by the platform.
```

## Decision

```text
A field inline size comes from a closed set of three size classes and nothing else: short for a count or an expiry, name for a team, project or workspace name, long for an email, a URL or a Matrix room id. An unclassed field takes the name size, so a field added later is sized and not stretched. The cap is presentation only and never refuses: stored bounds stay where they are and a longer value scrolls inside its own field. select and input type=number join the one text-input rule group and take the same border, fill, radius, face and minimum height, following the file-selector-button precedent. appearance: none is not used on a list: drawing a chevron would need an asset or a colour literal below the token seam, which check-arch S1 refuses, so the list keeps the platform arrow inside the Foundry box.
```

## Alternatives

```text
Keep width: 100 percent and cap the page instead: refused, because it makes the width of a field a property of the page rather than of what it holds, and it binds the board. A size per field, written at each site: refused, because the next field invents a fourth width. A fully drawn custom list control: refused for the token-seam and asset cost against a platform arrow that already works.
```

## Consequences

```text
One description of a field box for the whole application, and three names a reviewer can check by eye. The platform arrow and the number spinner stay visible, so the controls are not pixel-identical to a drawn design. A template may still name no class, so the default size is load-bearing.
```
