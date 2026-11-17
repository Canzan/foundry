# NAV\-ICON\-RAIL\-001 Collapsed left nav is an icon rail\, not a hidden one
Status: Accepted

## Context

```text
The delivered nav-collapse-toggle feature gave the collapsed rail a 72 px control strip and took the destinations, the workspace name and the user section out of the layout with display none, so they left the focus order and the accessibility tree together. That answered the flash and the tab-trap, and it left the operator with no destination at all while collapsed: she had to expand the rail to go anywhere, and Sign out was out of reach. The human decided on 2026-11-17 that the collapsed rail shows its links as icons and keeps Sign out. ADR-NAV-COLLAPSE-001 is unchanged and kept: the carrier is still a boolean behind the valueless marker, and every collapsed selector still matches on its presence.
```

## Decision

```text
The collapsed rail renders an icon for every destination the expanded nav shows the operator, and one for Sign out. The icons are inline aria-hidden SVG in the rail partial, server-rendered because the marker lands before the first paint and an icon injected later would paint an empty strip first. Each destination word moves into a label span that stays in the document in both geometries; the collapsed geometry CLIPS that span rather than removing it, so the accessible name is still computed from the item own text, and reveals it as a flyout on hover and on focus-visible so a pointer operator reads it too. Only the workspace name and the operator display name keep display none in the strip. The expanded nav is untouched: no icon beside its words, no tooltip it did not have.
```

## Alternatives

```text
Keep hiding the destinations and accept that the strip is a dead end - refuted by the model: the activatable set after a collapse is empty and Sign out cannot be activated. Hide them visually and leave them focusable - refuted by the model: five links can then take the focus while invisible, which is the tab-trap the delivered feature exists to avoid. Carry the destination name in aria-label instead of the clipped span - rejected: a second name channel beside the text can drift, and the text is the one owner. Carry it in a title attribute - rejected: the tooltip would fire in the expanded nav too, which the human fixed as unchanged. Carry it in an SVG title element - rejected: it adds a second text node, so the item text content reads the destination twice and the navigation-bar oracle reads that text. Vendor the icons as an external sprite - rejected: an external use reference is fetched after the document, so the strip paints without its icons.
```

## Consequences

```text
The operator reaches every destination she has, and Sign out, from the strip at every supported width; a stuck marker now degrades to a rail with no toggle instead of a rail with no destinations. The rail partial, the stylesheet and three of its consumers change together, and the stylesheet bytes change, so its content-hashed name, its provenance row and every reference to it move in one change. The delivered oracle folded-state expectation inverts and is restated by its owner. Two obligations no gate covers: every rail item must carry an icon, or the collapsed geometry must keep its word; and a one-pixel clipped span stays in the accessibility tree by contract, not by a checker, so the lane must read the computed accessible name rather than infer it.
```
