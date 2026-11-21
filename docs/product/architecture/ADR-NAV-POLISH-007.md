# NAV\-POLISH\-007 The boundary floor is an outside\-rail reading\, not a glyph count
Status: Accepted

## Context

```text
NAV-POLISH-006 moved the WCAG 1.4.11 exemption from a drawn word to rail membership, and left the anti-vacuity floor of the shipped walk unexamined. Two facts break it. The nav-collapse toggle is mounted into .sidebar__brand inside the rail aside, so rail membership makes it exempt along with Sign out and the theme control. The floor then asks for at least one control drawn as a glyph with no word of its own that was read for the 3:1 boundary, and no control outside the rail answers that shape on a board page: the lane-actions trigger every column renders draws an ellipsis CHARACTER, so the walk reads its glyph flag as false and its word as non-empty. The floor would therefore have no member left, and a floor with no member cannot fail - exactly the vacuity it exists to catch.
```

## Decision

```text
The floor is redefined as a READING, not as a shape: on every board page the law runs on, at least one control OUTSIDE the rail must have been read for the 3:1 boundary. It is anchored on .lane-menu-trigger in partials/board_columns.html, which every column renders, which sits outside the rail aside, and which carries its own 1 px --cz-muted border and so is read and MEETS 3:1 - a true negative for the exemption rather than a control the law has to forgive. The walk classifies it by DOM containment alone: its inTheNav fact is false, its glyph flag is false and its word is the ellipsis character, and none of those three decides the reading any more. If a board page ever stops rendering a column, .modal-close in partials/edit_lane_modal.html and partials/delete_issue_modal.html gives the same anchor behind an open modal. So defined, the floor goes red the moment the exemption widens past the rail and the outside-rail controls leave the reading, which is the failure the old count could no longer see. Everything else of NAV-POLISH-006 stands: rail membership in every layout, text contrast wherever a rail control word is drawn, and the P4 workspace-name clip mechanism.
```

## Alternatives

```text
Keep the glyph-only count and exclude the nav-collapse toggle from the rail for this law alone (rejected: it reintroduces a per-control exception inside a rule whose whole point is one answer by containment, and the toggle is a rail control by every other reading). Keep the glyph-only count and give an outside-rail control a glyph element so the count finds a member (rejected: it changes production markup to satisfy a test shape, which is the test owning the design). Anchor the floor on a count of all controls read (rejected: the rail controls alone would satisfy it, so it would pass while every outside-rail control left the reading). Drop the floor (rejected: the walk then cannot tell a scoped exemption from a vacuous one, and the exemption is a scope rule with no type behind it).
```

## Consequences

```text
The boundary reading now depends on the board page rendering at least one column, which it always does, and the acceptance designer owns both the anchor and the floor. A later change that moves the lane-actions trigger inside the rail, or removes the column menu, owes this law a new anchor, and the modal close control is the recorded fallback. The exemption still lives in the walk and not in a type, so nothing refuses a control being placed inside the rail to escape the boundary; the floor is what makes that visible, because an exemption that widened past the rail leaves the outside-rail reading empty and red.
```
