# NAV\-POLISH\-001 Nav marks its own page and offers only what opens
Status: Accepted

## Context

```text
Three destinations moved into the left nav in nav-quick-actions carried no section, so Teams, /workspace/invites and /admin/tokens all marked Home, and both admin-gated links were offered to every signed-in operator and answered a plain member with the non-enumerable 404. The rail also scrolled away with a long page, took about 320 of 844 px on a phone, and drew its sign-out submit as a filled black slab beside a bordered theme button.
```

## Decision

```text
NavSection gains Teams, Invites and Tokens, so the mark stays derived by equality from the single NavContext.active field and the three page families name their own section through teams_for, invites_for and tokens_for over the existing resolved(). The two admin links are wrapped in the same {% if nav.is_workspace_admin %} condition the Integrations item already uses, reading the one fact the destinations gate on, with no new lookup and no new authorization decision. The stylesheet makes the rail sticky at 100vh with its own overflow, draws the phone bar in the shipped folded glyph geometry so the words are clipped rather than removed, and gives the sign-out submit and the theme control the presentation of their neighbour links. Quint carries the mark-and-offer laws; the three geometry promises have no checker and rest on the browser-measured acceptance reading.
```

## Alternatives

```text
Compare the request path in the template (refuted: the carrier holds no path and a second path-shaped authority makes two marks representable, and a team page matches no primary href). A per-item boolean set instead of one section (rejected: it makes two marks representable). A separate rights lookup for the nav (rejected: a second reader of the same fact can disagree with the gate). Cap the phone bar with max-height plus overflow (refuted: it leaves a destination invisible and still focusable, which the shipped rail law forbids). Keep the theme border for WCAG 1.4.11 (rejected: the control becomes text-plus-glyph like its neighbour links, which have never needed one, and the shared focus rule is untouched).
```

## Consequences

```text
Every new primary destination from now on owes a NavSection variant and, if its page is gated, a nav condition on the same fact its gate reads - neither is enforced by a Rust type, so the new check-arch obligation and the per-family acceptance rows are the standing checks. Two shipped lanes change with this Request: nav-quick-actions asserts the opposite of both promises, and nav-collapse-icon-rail owes a plain member the two links. The stylesheet rename travels to four readers. The phone bound is measured only at the lane device 390x844; a 320 px phone is recorded as unknown with the product owner.
```
