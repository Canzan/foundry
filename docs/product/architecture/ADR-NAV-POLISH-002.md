# NAV\-POLISH\-002 Rail text\-plus\-glyph controls owe text contrast\, not a boundary
Status: Accepted

## Context

```text
P5 draws the sign-out submit and the theme control like their neighbour rail links: the rail surface fill, no border, the muted text tier. The shipped canzan-theme-system law walks every visible button inside .app-shell and asks for 3:1 from the better of the own opaque fill and the border (WCAG 1.4.11). It skips a transparent fill, counts a zero-width border as none and reads no third property. Two rows went red on the implemented design: every large-text and control-boundary pair reaches at least 3 to 1, and the same holds when the device preference is light. The measurement was 0.00:1 for button.sidebar__item [Sign out] against #0f1312 in the dark palette, and the same against #f3f4f1 in the light one. The theme control has no escape at all, because the fill it would need is the very surface it is measured against. ADR-NAV-POLISH-001 argued that the border may go because the control becomes text plus glyph; that argument is true of the design and does not reach the checker, because the shipped walk enumerates buttons.
```

## Decision

```text
User decision 2026-11-20: a rail control drawn as text plus an icon is exempt from the WCAG 1.4.11 boundary rule, exactly as the rail links are. Its text keeps the WCAG 1.4.3 rule and reaches 4.5:1 against the surface behind it. P5 ships as designed. The acceptance designer migrates the scope of the shipped control-boundary law in canzan-theme-system.feature and in its step module, and reads the exempt control for text contrast instead of a boundary. The exemption is keyed on the visible word the control draws, so a button inside .app-shell with no visible word keeps the boundary rule. The same revision records the P4 mechanism the crafter needed: on a phone the workspace name is clipped out of the flow (position absolute, a 1 px box, clip-path inset(50%)) instead of display:none, because the shipped instance-admin-workspace-rename row measures that name box and a removed box reads 0.
```

## Alternatives

```text
Keep an identifying fill or border on the two controls and ask the acceptance designer to loosen the P5 comparison (rejected: a border makes the control the heaviest thing in the nav again, which is what P5 removes, and the theme control cannot pass by fill because its fill is the measured surface). Take P5 back to product and drop it (rejected by the user decision of 2026-11-20). Exempt every button whose computed fill is transparent (rejected: that skips every quiet button in the shell, far outside the rail). Keep display:none for the workspace name on a phone (refuted: the shipped phone row measures the name box, and a removed box reads scrollWidth 0 and clientWidth 0).
```

## Consequences

```text
canzan-theme-system is a third migrated lane beside nav-quick-actions and nav-collapse-icon-rail, and it joins the slice supports with its step module; the acceptance designer owns both files. The exemption lives in the acceptance walk, not in a type, so nothing refuses a future rail control that drops its word: the L7 case that keeps the boundary rule for a button with no visible word is the standing check. P5 stays DESIGNED_NOT_BUILT until the migrated rows run green. An operator who needs a drawn boundary keeps the user-agent boundaries of forced-colours mode and the focus outline; that residue is the same one the rail links already carry. instance-admin-workspace-rename stays unchanged and is read, not edited, under the clip mechanism.
```
