# NAV\-POLISH\-006 Rail controls are exempt from the boundary rule in every layout
Status: Superseded by NAV-POLISH-007

## Context

```text
NAV-POLISH-002 keyed the WCAG 1.4.11 exemption on the visible word a rail control draws, so a button inside .app-shell with no visible word kept the boundary rule. The rail has three layouts and the same unchanged control draws its word in only one of them: the open desk rail draws glyph and word; the narrowed rail draws the glyph and reveals the word only on hover or focus; at phone width Sign out draws the glyph alone. Under a word-keyed exemption Sign out and the theme control therefore fall on the wordless side in the narrowed rail, and Sign out falls there at phone width, and each would owe a 3:1 fill or border exactly where the rail is narrowest - a boundary P5 removed, and one the theme control cannot pass by fill, because its fill is the very surface it is measured against. Both oracle reviews retained this as an open design question. The user decided it in chat on 2026-11-20, answering: exempt in every layout.
```

## Decision

```text
A control inside the left rail is exempt from the WCAG 1.4.11 boundary rule in EVERY layout, exactly as the rail links are. The exemption follows rail MEMBERSHIP, read as DOM containment in the rail element the sidebar partial renders - the aside that holds #sidebar-nav - and never from the control own class, tag or fill, and never from whether a word happens to be drawn. A control inside .app-shell but outside the rail keeps the 3:1 rule, whether or not it draws a word. The WCAG 1.4.3 text rule still applies wherever a rail control word is DRAWN, at 4.5:1 against the surface behind it: the rail surface in the open layout and the flyout surface where the narrowed rail reveals the word. A clipped word is not drawn and is not read for a ratio. The oracle therefore owes rows for the two icon-only layouts, and the obligation for the acceptance designer is: in the narrowed rail and at the phone device, in both palettes, each rail control resolves to no fill and no border boundary and is NOT held to 3:1, while a wordless button inside .app-shell but outside the rail still is; and the word the narrowed rail reveals is read for 4.5:1 against the flyout surface rather than against the rail surface. The P4 workspace-name clip mechanism NAV-POLISH-002 also recorded stands unchanged: on a phone the name is clipped out of the flow, never display:none, because the shipped instance-admin-workspace-rename row measures that box.
```

## Alternatives

```text
Keep the word-keyed exemption and add a fill or border to the two controls in the narrowed and phone layouts only (rejected: it makes the control the heaviest thing in the nav exactly where the nav is narrowest, which is what P5 removes, and it gives one unchanged control two presentations). Keep the word-keyed exemption and read the REVEALED word of the narrowed rail as the drawn word (rejected: the word is drawn only while the control has the pointer or the keyboard, so the law answer would depend on focus state, and at phone width Sign out reveals nothing at all). Exempt every button whose computed fill is transparent (rejected again: it skips every quiet button in the shell, far outside the rail). Exempt by the control own class (rejected: a class is a designation of rail membership, not the property, and it dangles the moment the partial renames a class). Take P5 back to product (rejected by the user decision).
```

## Consequences

```text
The exemption now has one answer for one control in all three layouts, so the migrated canzan-theme-system rows read the same property everywhere and the lane owes two more rows, for the narrowed rail and for the phone device. The exemption lives in the acceptance walk, not in a type, and it keys on containment, so any control later placed inside the rail inherits it with no boundary of its own and a control moved out of the rail loses it silently; nothing refuses either move, and the L7 case that keeps the boundary rule for a wordless button outside the rail is the standing check. An operator who needs a drawn boundary keeps the user-agent boundaries of forced-colours mode and the focus outline, the same residue the rail links already carry. P5 stays DESIGNED_NOT_BUILT until the migrated rows run green.
```
