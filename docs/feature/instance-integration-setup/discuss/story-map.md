# Story map: instance-integration-setup

User: Priya Raman, instance admin (and workspace admin of Canzan Labs).
Goal: make Slack and Matrix available on this Foundry from the browser, with
no restart. Keys stay encrypted, and the deploy stays standalone.

## Backbone

| Notice it isn't available | Find where to set it up | See what this Foundry has and lacks | Turn on the secrets key | Enter the Slack app | Change it later |
|---|---|---|---|---|---|
| W-2 / W-3 (shipped, v0.18.2) | **US-IIS-04** I-19 link for instance admins, beside "How to set up Slack"; setup guide reworded | **US-IIS-01** instance page: instance key, secrets key, Slack, Matrix; manifest and URLs | **US-IIS-02** generate the secrets key (encrypted under the instance key); Matrix ready | **US-IIS-03** save, sealed, shape check, live on every replica | **US-IIS-05** replace / remove the Slack app |
| | I-1 dashboard link (in US-IIS-01) | | **US-IIS-08** standalone: Foundry makes its own instance key | US-IIS-06 save while the environment wins | **US-IIS-07** replace the secrets key; start over when it cannot be opened |
| | | | **US-IIS-06** upgrade: keep opening, then move under a secrets key | | |

## Walking skeleton

Decision: **yes, a thin skeleton.** The feature is brownfield, but it adds a
new page, keys and an app that Foundry now stores (encrypted), and a live
reconfiguration every replica must see. Those are new integration points.

The skeleton is US-IIS-01, then US-IIS-02, then US-IIS-03, with an
operator-set instance key:

1. The instance page (reached through I-1).
2. Generate the secrets key. It is stored encrypted under the instance key, the
   key in effect changes on every replica, and Matrix is offered.
3. Save the Slack app: Add to Slack is offered.

DISTILL tags US-IIS-02 scenario 1 `@walking_skeleton`.

## Release slices

### Release 1 (walking skeleton): an instance admin turns on Matrix and Slack without a restart

- US-IIS-01, US-IIS-02, US-IIS-03.
- Targets KPI-IIS-1 and KPI-IIS-2, with guardrails G1 and G2.
- A deploy with no instance key sees I-27, and integrations stay off, never in
  plaintext.

### Release 2: safe on existing and standalone deploys

- US-IIS-06, US-IIS-08.
- Targets KPI-IIS-5 and KPI-IIS-6.
- US-IIS-06 AC 1 (credentials sealed with an existing FOUNDRY_SECRETS_KEY keep
  opening) is a release blocker for Release 1 on any upgraded instance. DESIGN
  must keep it true from Release 1 on.

### Release 3: keys and the Slack app can change safely, and "not set up" leads to the fix

- US-IIS-07, US-IIS-04, US-IIS-05.
- Targets KPI-IIS-4 and KPI-IIS-3.

### Split (D17, approved by the user 2026-11-05)

- **Deliverable A:** Releases 1 and 2. US-IIS-01 (1 d), 02 (2 d), 03 (2 d),
  06 (2 d) and 08 (2 d): 9 days.
  - A is self-contained: I-26 is restore-only, and US-IIS-06's I-14 check
    (its AC 6) is in B.
- **Deliverable B:** Release 3. US-IIS-07 (2 d), 04 (1 d) and 05 (1.5 d): 4.5
  days.
  - B adds Start over, Replace secrets key, I-19, I-14 and I-15.
- Total: 13.5 days. Both stay in this feature directory.

## Priority Rationale

Priority score is Value × Urgency / Effort, on a 1 to 5 scale.

| Order | Story | V | U | E | Score | Why here |
|---|---|---|---|---|---|---|
| 1 | US-IIS-01 | 4 | 5 | 2 | 10 | Skeleton. It is the page everything else sits on, and it derisks the access rule and the shared state read. |
| 2 | US-IIS-02 | 5 | 5 | 4 | 6.3 | Skeleton, and the riskiest assumption: a key stored encrypted and live on every replica without a restart. |
| 3 | US-IIS-03 | 5 | 5 | 4 | 6.3 | Skeleton. It completes the Slack outcome on 02's live-change mechanism. |
| 4 | US-IIS-06 | 4 | 4 | 4 | 4 | Upgraded instances must keep opening what they sealed. Its first AC rides with Release 1. |
| 5 | US-IIS-08 | 4 | 3 | 4 | 3 | Deployment independence: a standalone deploy needs no key setting, and switching to an operator key loses nothing. It comes after 06 because existing instances break without 06. |
| 6 | US-IIS-07 | 3 | 3 | 4 | 2.3 | Rotation, plus the explicit way out of a lost instance key, which 08's file makes more likely. |
| 7 | US-IIS-04 | 3 | 2 | 2 | 3 | Low urgency: I-1 and the sidebar already reach the page. It keeps "How to set up Slack" beside the new link. |
| 8 | US-IIS-05 | 3 | 2 | 3 | 2 | Needed before anyone rotates a Slack secret. Until then Remove then Save works. |

Tie-break: walking skeleton first, then the riskiest assumption (US-IIS-02 and
upgrade safety), then value.
