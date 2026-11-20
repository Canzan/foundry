# NQA\-001 Nav owns the three moved destinations
Status: Accepted

## Context

```text
Teams, Invite a member and Machine tokens lived only in Home Quick actions, so an operator on a board had to go back to Home to reach them. The left nav already collapses into an icon rail (v0.19.0), whose shipped oracle quantifies over the members it reads from the live document, and whose stylesheet wraps each group inside the bar at every phone width. The three links carry no rights condition in Quick actions today, while Integrations and Instance admin carry conditions identical to the ones already in the nav.
```

## Decision

```text
templates/partials/sidebar.html becomes the single owner of these three destinations: three unconditional anchors after Board inside nav#sidebar-nav, each with one inline Lucide glyph from the pinned lucide-static 1.75.0 (users, user-plus, key-round) and one span.sidebar__label, and no aria-current. Home Quick actions section is deleted and DashboardRoot drops the three fields it alone read. The stylesheet, nav-collapse.js and the rail reading are untouched. A new check-arch layer enumerates the partial members and refuses one without exactly one icon and one label.
```

## Alternatives

```text
Derive the member list from NavContext fields: refused, it adds a carrier with one reader and no decision to make, since all three hrefs are literals. Keep Quick actions as a duplicate of the nav: refused by the owner and it leaves two carriers of the same rights facts on one page. Add rail CSS for the enlarged set: refused, the wrap rules are quantified over members and a byte change would rename the hash-named stylesheet and migrate base.html, VENDOR.md and three cache-policy assertions. Write new browser scenarios for P4: refused, the shipped icon-rail lane already quantifies over whatever members the document holds, so it is re-run instead.
```

## Consequences

```text
Every authenticated page gains three destinations in one press, open or folded, with no rights change. The phone bar and a short desk window take more rows and more vertical scroll; nothing slides sideways. One shipped guard is contradicted and must be retired by its owner: navigation-bar-linear-ui Slice 09 asserts these two links stay OUT of the sidebar. The dashboard palette scenario loses its .actions rows. P4 is only observed when the browser lane runs with Docker, so close evidence must name the lane that ran.
```
