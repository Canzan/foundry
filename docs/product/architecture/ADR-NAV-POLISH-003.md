# NAV\-POLISH\-003 Nav marks an offered member and the rail keeps its flyout
Status: Superseded by NAV-POLISH-004

## Context

```text
The nav and the route gate read the workspace-admin fact separately, so one render can hold a section the rail will not offer: on /workspace/invites and /admin/tokens a route gate that admitted a workspace admin could meet a failed nav identity read, the owning member was then not rendered at all, and the rail carried no mark. A scroll container clips its scrollable overflow on BOTH axes, because a declared overflow-y of auto makes overflow-x compute to auto as well, so a rail held in view by one overflow declaration clips the revealed word of every folded member - the word the shipped folded-rail promise requires to be visible beside the 72px strip. Three destinations moved into the left nav in nav-quick-actions also carried no section, both admin links were offered to every signed-in operator, the rail scrolled away with a long page, the phone bar took about 320 of 844 px, and the sign-out submit was a filled black slab beside a bordered theme button.
```

## Decision

```text
NavSection gains Teams, Invites and Tokens, so the mark stays derived by equality from the single NavContext.active field, and resolved() maps the named section through the total NavSection::offered_mark before the rail is built: a page whose own member the rail will not offer marks Home, which the rail always offers, so neither two marks nor zero marks is representable in the carrier, and the three page families name their own section through teams_for, invites_for and tokens_for over that same resolved(). The two admin links are wrapped in the same {% if nav.is_workspace_admin %} condition the Integrations item already uses, reading the one fact the destinations gate on, with no new lookup and no new authorization decision. The stylesheet makes the rail sticky at 100vh with align-self: flex-start, keeps the scroll container on the OPEN geometry alone and sets overflow: visible on the folded geometry, draws the phone bar in the shipped folded glyph geometry so the words are clipped rather than removed, and gives the sign-out submit and the theme control the presentation of their neighbour links. Quint carries the mark-and-offer laws over both identities; the three geometry promises have no checker and rest on the browser-measured acceptance reading, with the revealed word read by a clip-aware centre hit test rather than by its layout box.
```

## Alternatives

```text
Compare the request path in the template (refuted: the carrier holds no path, a second path-shaped authority makes two marks representable, and a team page matches no primary href). A per-item boolean set instead of one section (rejected: it makes two marks representable). A separate rights lookup for the nav (rejected: a second reader of the same fact can disagree with the gate). Reconcile the mark in each handler instead of inside resolved() (rejected: the rule would then live in four handlers and one of them can forget it). Bound the declared mark to renders whose identity resolved, leaving an admin page unmarked on the failure branch (rejected: it weakens the shipped exactly-one-current promise and buys nothing). One overflow-y: auto declaration for both geometries (refuted: it clips the folded revealed word on the horizontal axis and no shipped lane reads that). A fixed-position flyout layer outside the clip (rejected: its vertical anchor is a layout-time static position that drifts when the rail scrolls under an open word). Cap the phone bar with max-height plus overflow (refuted: it leaves a destination invisible and still focusable, which the shipped rail law forbids). Keep the theme border for WCAG 1.4.11 (rejected: the control becomes text-plus-glyph like its neighbour links, which have never needed one, and the shared focus rule is untouched).
```

## Consequences

```text
Every new primary destination from now on owes a NavSection variant, a nav condition on the same fact its gate reads when the page is gated, and an offered_mark row when that condition can hide its member - none of the three is enforced by a Rust type, so the check-arch obligation, the nav.rs totality test over every section-by-rights pair and the per-family acceptance rows are the standing checks. The revealed word escapes no clip by type: any later overflow, clip-path, contain or filter on a rail ancestor clips it again, and only the clip-aware browser row refuses that. A folded rail taller than the window leaves its last members outside the window, which no CSS condition closes, and that bound is recorded with the crafter rather than claimed as a satisfied promise. Two shipped lanes change with this Request: nav-quick-actions asserts the opposite of both promises, and nav-collapse-icon-rail owes a plain member the two links. The stylesheet rename travels to four readers. The phone bound is measured only at the lane device 390x844; a 320 px phone is recorded as unknown with the product owner.
```
