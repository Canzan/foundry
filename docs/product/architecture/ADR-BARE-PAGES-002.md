# BARE\-PAGES\-002 The shortcut frame follows the session\, and every answer says so
Status: Accepted

## Context

```text
The first examination refused the shortcut page an operator reaches signed in: a centred card with no application frame, no nav and no way back. The user ruled on 2026-11-21 that the frame follows the session. That ruling AMENDS two accepted records of this Request without replacing either: ADR-002, which decided one route and two bodies from the declared destination, and BARE-PAGES-001, which decided that both arms declare the request header their answer depends on. It also narrows ADR-003, the card frame, to the signed-out pages. The ruling raises a cache question those records could not have: the same URL now differs by the SESSION as well as by the destination header, and a signed-in frame names the operator's workspace in the rail.
```

## Decision

```text
The page arm reads the session and renders the normal application frame with the nav when a session resolves, the centred card when none does; the route stays PUBLIC, because no session is the card arm and never a refusal, and the nav comes from the shipped neutral home-for constructor so no nav rule is added. A template cannot choose its parent at run time, so the two frames are two templates over the one rows partial and the one row builder, both keeping the keyboard-help class with no role and no aria-label. Every answer from every arm carries Vary naming both Sec-Fetch-Dest and Cookie, and each page arm additionally carries Cache-Control: no-store, because a private cache that replayed a signed-in frame would show another reader, or the same reader after sign-out, that workspace identity, and Vary on Cookie alone does not prevent a replay once the cookie is cleared. The fragment arm stays cacheable under the two Vary names: its body is identical for every client of that arm and carries no identity.
```

## Alternatives

```text
Keep the card for a signed-in operator (rejected by the user on 2026-11-21: it is a page with no nav and no way back, which is what the examination refused). Gate the route behind a session so it can always render the frame (rejected: the help must stay readable from the sign-in page, where a stuck operator asks for it). One template with a conditional parent (rejected: the engine resolves the parent at compile time). no-store on both arms (rejected: the fragment is fetched on every overlay press and carries no identity, so forbidding its reuse costs round trips for nothing). Vary on the destination alone (rejected: it was enough only while the answer ignored the session, which this ruling ends). Re-key the two numeric ADRs of this Request under the feature prefix (not possible through the recorder: it refuses to drop an accepted key without an accepted supersedes declaration, and the user asked for an amendment rather than a supersession, so the keys stay and this record carries the amendment).
```

## Consequences

```text
AMENDS ADR-002 and BARE-PAGES-001 and narrows ADR-003; none of the three is superseded and none of their bodies changes. The shortcut route now depends on the session, so it joins the pages where exactly one primary nav member is marked. The page arm is not cached at all, which costs one round trip on a page a reader opens rarely. Any later route that decides an answer from a request header or a cookie owes the same declaration; today this is the only one. A response missing either header is a defect even with the right body.
```
