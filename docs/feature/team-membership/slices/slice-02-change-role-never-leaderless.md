# Slice 02: A workspace admin changes who leads a team, and the last lead cannot be demoted

Story: US-TM-03 | Estimate: 1 day | job_id: `job-team-membership`

## Goal

On a team's members page, a workspace admin promotes a member to Lead or demotes
a lead to Member. A demotion that would leave the team with no lead is refused
with a stated reason, including when two demotions race.

## IN

- `POST /team/{team_slug}/members/{user_id}/role`: "Make lead" and "Make member" controls, shown to a workspace admin (D11). The D15 copy is used. A same-role submit writes nothing (D13).
- The last-lead refusal: "{team} needs at least one lead. Make someone else a lead first." Nothing changes (D6).
- The never-leaderless guarantee under concurrency: INV-1, INV-2 and INV-4 (self-demotion). DESIGN picks the mechanism, and INV-8 asks where it lives.
- Uniform 404 for unauthorized callers (D5).

## OUT

- Remove (slice 03). Team-lead powers (slice 04). Repairing teams that already have no lead (allowed to be promoted into; not detected).

## Learning Hypothesis

- **Disproves if it fails**: that "never leaderless" can be guaranteed under concurrent demotes without a schema change or a global lock. If the race scenario ever ends with zero leads over 100 runs, the mechanism is wrong. Slices 03 and 04 must not start until it is fixed.
- **Confirms if it succeeds**: one invariant seam, which slice 03's remove and slice 04's lead actions reuse unchanged.

## Acceptance Criteria

- [ ] Backend (lead Lena Park, member Sam Okafor): Make lead on Sam → "Sam Okafor (sam.okafor@canzan.dev) is now a lead of Backend."
- [ ] Make member on Lena → "Lena Park (lena.park@canzan.dev) is now a member of Backend." Sam is the only lead.
- [ ] Make member on Sam (the only lead) → the last-lead copy, and Sam is still Lead.
- [ ] Lena and Sam are the only leads and both are demoted concurrently: exactly one succeeds, over at least 100 repeated races on real Postgres.
- [ ] Marco → uniform 404, and no role changes.
- [ ] Dogfood on dev: promote `jeff@unintelligent-design.us` to Lead of General, then demote it again. Demoting the bootstrap account while it is the sole lead is refused.

## Dependencies

- Slice 01 (members page, authz surface). OQ-2 (actors), whose recommended answer is assumed.
- High uncertainty: consider a 1-2 hour DESIGN spike on the serialization shape before DISTILL.
