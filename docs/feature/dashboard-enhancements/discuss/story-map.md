# Story Map — dashboard-enhancements

## Backbone (user activity: "orient and act from the dashboard")

```
  ORIENT ─────────────► NAVIGATE ─────────────► ACT ──────────► (maintain)
  see who/where I am    reach my work +          end my         surface stays
                        role-appropriate tools   session        trustworthy
```

| Activity | Stories |
|----------|---------|
| Orient   | US-01 (greeting: name + workspace) |
| Navigate | US-03 (instance-admin link, super-admin only); projects + quick actions already shipped |
| Act      | US-02 (sign out) |
| Maintain | US-04 (styles → stylesheet), US-05 (test coverage) |

## Walking skeleton

Already shipped in `51ba981` (dashboard lists projects + quick actions). This feature layers thin slices
on that skeleton; no new skeleton needed (brownfield).

## Elephant-carpaccio slices (each ≤1 day, end-to-end, test-first in DELIVER)

| # | Slice | Stories | Learning hypothesis (fails if…) | Value |
|---|-------|---------|--------------------------------|-------|
| 01 | `slice-01-greeting` | US-01 (+ its store-query test) | Disproves "a single session-scoped query cleanly yields name+workspace" if it needs >1 round-trip or a join we lack. | User sees name + workspace |
| 02 | `slice-02-instance-admin-link` | US-03 (+ tests) | Disproves "role-conditional nav needs nothing but the shipped `is_instance_admin`" if the predicate is insufficient/mis-scoped. | Super-admin reaches instance admin; others don't see it |
| 03 | `slice-03-sign-out` | US-02 (+ tests) | Disproves "CSRF plumbing on `/` is a straight copy of `admin_tokens::show_index`" if the response-type change ripples further than expected. | User can sign out |
| 04 | `slice-04-coverage-and-styles` | US-05 (retroactive base coverage) + US-04 (styles promote) | Disproves "the base dashboard is faithfully coverable + styles promote with no visual drift" if an acceptance scenario surfaces a latent bug or the hash bump breaks caching. | Dashboard protected by tests; styles canonical |

### Carpaccio taste tests
- No slice ships 4+ new components ✓ (each touches 1 query + 1 template + 1 handler edit).
- No slice depends on a NEW abstraction ✓ (all reuse shipped seams).
- Each slice disproves a real pre-commitment ✓ (hypotheses above).
- Production data, not synthetic ✓ (runs against the real store; the live dev workspace has `GEN/Sandbox`).
- No two slices are identical-except-scale ✓.
- Slice composition: slice 04 pairs the `@refactor` (US-04) + test-debt (US-05) with a user-observable
  acceptance scenario (the dashboard end-to-end) — not an infrastructure-only slice ✓.

## Prioritization (execution order + rationale)

1. **slice-01-greeting** — highest learning leverage on the new-query pattern; smallest; sets the
   handler-loads-identity shape the later slices extend.
2. **slice-02-instance-admin-link** — pure add, reuses `is_instance_admin`; independent of 01.
3. **slice-03-sign-out** — deferred behind 01/02 because it changes `dashboard_root`'s response type
   (`Html` → `(headers, Html)` for the CSRF cookie); doing it after the additive slices isolates that
   ripple.
4. **slice-04-coverage-and-styles** — last: the acceptance scenario now exercises the fully-assembled
   dashboard (greeting + admin link + sign-out), and the style promotion is a clean visual-equivalence
   refactor over the final markup.

Dogfood moment: after each slice, refresh `http://localhost:3000/` in the watch-mode dev instance.
