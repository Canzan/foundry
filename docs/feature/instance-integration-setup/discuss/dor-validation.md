# Definition of Ready: instance-integration-setup

This is the second self-check, after peer review iteration 1 (verdict
needs_revision). Every item below quotes evidence from `user-stories.md`. Where
the evidence is partial, the item says so.

## Per item

### 1. Problem stated in domain terms: PASS

- US-IIS-01: "nothing in Foundry says what that means or where to fix it".
- US-IIS-03: "A typo shows up only as a server that refuses to start, or a
  failed Add to Slack".
- US-IIS-06: "must not open a window where credentials cannot be opened".
- US-IIS-08: "Today that deploy simply has no integrations until the operator
  learns about the key".
- Every story has a Problem section. None uses implementation language.

### 2. User and persona identified: PASS

- The cast names Priya Raman (persona-instance-operator), Tomás Rivera
  (workspace admin, not an instance admin, inline), Lena Park (lead of
  Backend), Sam Okafor ("member of Backend, not a lead", from SSOT journey line
  329) and Marco (persona-team-member-foil, "on no Canzan Labs team").
- Marco has no surname. Prior features (external-integration-plugins,
  integrations-wayfinding) never gave him one, and
  `persona-instance-operator.yaml` says inventing details for him "would be
  fabrication". The review asked for a surname; I declined on that ground.

### 3. ≥ 3 domain examples with real data: PASS

Examples per story: 01 has 5, 02 has 4, 03 has 4, 04 has 4, 05 has 4, 06 has
4, 07 has 4, 08 has 5. Data used:

- client ID `6123456789012.7234567890123`;
- `https://foundry.example.org`;
- `/var/lib/foundry/instance.key`;
- "a 20-character value";
- Canzan Labs, Ops Guild and Reading Club;
- #auth-dev and #auth:canzan.dev;
- "Rotate LLDAP bind password".

### 4. UAT in Given/When/Then, 3 to 7 per story: PASS

Scenario counts: 01 has 7, 02 has 7, 03 has 6, 04 has 4, 05 has 5, 06 has 4,
07 has 5, 08 has 7. Three stories sit at the cap of 7.

### 5. AC derived from UAT, each naming its driving port: PASS, with gaps noted

- Ports: every AC now opens with its route, `Process start`, `CLI`, or
  "Observed through a direct database read" (US-02 AC 3, US-03 AC 3, US-08 AC
  2). The direct read is used where the dump itself is the attack surface.
- Gaps: these ACs have no scenario of their own. They come from the
  constraints and domain examples, and DISTILL should add scenarios.
  - US-01 AC 5: I-4 and I-13.
  - US-02 AC 6: the restart half.
  - US-06 AC 5: the partial SLACK_* trio.
  - US-08 AC 6: an operator key with no file.
  - US-05 AC 6.

### 6. Right-sized (1 to 3 days, ≤ 7 scenarios): PASS

- Estimates: 01 1 d, 02 2 d, 03 2 d, 04 1 d (re-estimated from 0.5 d), 05
  1.5 d, 06 2 d, 07 2 d, 08 2 d (raised from 1.5 d for the added scope).
- Deliverable A is 9 d and Deliverable B is 4.5 d (D17).

### 7. Technical notes and constraints: PASS

- System Constraints include:
  - "Operator instance key format. 32 random bytes, base64url … A malformed
    value refuses start";
  - the test seam ("feature-gated test-support read under check-arch
    `integrations-test-seam`");
  - "Slack's addresses need `FOUNDRY_PUBLIC_URL`";
  - Deployment independence.
- Each story has Technical Notes.

### 8. Dependencies resolved or tracked: PASS

- Each story lists its dependencies. Story order and the A/B split are in
  story-map.md.
- External, all tracked:
  - ADR-EIP-004 and ADR-EIP-005 amendments.
  - A home for the instance-level audit.
  - `Integrations::slack_events_url` (new).
  - `FOUNDRY_PUBLIC_URL` (US-01 dependency line).
- The spike candidate for cross-replica reload is in D10.

### 9. Outcome KPIs defined: PASS

- outcome-kpis.md defines KPI-IIS-1 to 6, G1 to G3 and M.
- KPI-IIS-2 lists all 16 states.
- KPI-IIS-3 is "I-19 present … and following it answers 200".
- G1 names the test seam.

### 0. Elevator Pitch: PASS

Every story has Before, After (naming a route such as
`POST /admin/instance/integrations/secrets-key`) and Decision enabled. None is
`@infrastructure`.

### J. `job_id`: PASS

All 8 stories are `job-route-project-events-to-chat` (D16, approved by the user
2026-11-05).

## Review items: how each was addressed

1. **A/B split.**
   - US-01 AC 5: I-26 is "restore-only in Deliverable A".
   - US-06 AC 6 (I-14) is marked "(Deliverable B, with US-IIS-05 AC 2)".
   - I-26 in A has no Start over. B appends it (copy table).
2. **Ports.** The listed ACs now start with routes or an explicit observation
   point.
3. **Instance-key lifecycle.**
   - US-08 gains a scenario and AC 5 for an operator key replacing the file
     (I-30).
   - I-4 is reworded to name the previous instance key and scoped to "saved
     credentials".
   - US-07 AC 4 and a scenario withhold Replace under I-26.
   - US-02 AC 1 and its scenario cover concurrent generate. US-08 AC 1 and its
     scenario cover concurrent file creation.
4. **Links.** US-04 AC 1 keeps "How to set up Slack" and adds I-19 after it.
5. **Cast.** Sam was added. Marco's surname was declined (see item 2).
6. **Estimates.** US-04 is 1 d, and the totals are updated.

## DoR Status: PASSED (self-check)

- Decisions D2 to D17 were approved by the user 2026-11-05. The refinements
  from peer review are noted under each decision.
- The item 5 gaps above are for DISTILL to cover. They do not block DESIGN.
- Peer review iteration 2 is pending.

## Risks

- No DISCOVER or DIVERGE wave ran.
- A Foundry-made key file on multi-replica deploys without a shared volume
  (mitigated by I-26).
- Cross-replica live reload is unproven (spike candidate).
- 13.5 days in total; split approved.
