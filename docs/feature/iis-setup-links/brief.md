# Product brief: iis\-setup\-links

## Request

```text
User request (verbatim): "Let's continue delivering any nWave 3 feature in nWave 4 from @docs/feature/"

Selected scope (user-approved split, 2026-11-08): instance-integration-setup Deliverable B, Request 1 of 3 — US-IIS-04 "Workspace pages send an instance admin to the setup", as written in docs/feature/instance-integration-setup/discuss/user-stories.md (lines 611-715, Deliverable B), with the locked copy I-19 and I-20 from that file's "Copy (locked once approved)" table. Deliverable A (US-IIS-01, 02, 03, 06, 08) is shipped (docs/evolution/2026-11-07-instance-integration-setup.md). Requests 2 (US-IIS-05 + US-IIS-06 AC 6) and 3 (US-IIS-07) follow separately.
```

## Work

S

```text
An instance admin who reads W-2 or W-3 on the workspace integrations page or a new-route form can follow 'Set up integrations for this Foundry' to fix it in the browser, and the Slack setup guide no longer says to restart
```

### Rework

None.

## Promises

### P1

Outcome

```text
On GET /workspace/integrations, an instance admin who reads W-2 (Slack or Matrix) or W-3 sees it followed by any existing link ("How to set up Slack" for Slack W-2), then I-19 "Set up integrations for this Foundry", a link to /admin/instance/integrations. US-IW-02 AC 1 and KPI-IW-3 stay valid.
```

### P2

Outcome

```text
GET /team/{team}/project/{project}/integrations/routes/new and its posts .../routes/preview and .../routes apply the same link rule per integration kind, with the same words for the same state as the workspace integrations page.
```

### P3

Outcome

```text
Each viewer is offered only links they can open: a workspace admin who is not an instance admin is offered "How to set up Slack" and not I-19; a lead who is not an admin is offered no link; every offered link (GET /admin/instance/integrations, GET /workspace/integrations/slack/setup), followed, answers 200 to that viewer.
```

### P4

Outcome

```text
GET /workspace/integrations/slack/setup shows I-20: step 3 reads "An instance admin enters the app's Client ID, Client Secret and Signing Secret under Instance admin › Integrations.", step 4 reads "No restart is needed. A workspace admin can then choose Add to Slack on the workspace integrations page.", with the footnote about SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET taking precedence; no step says "Restart Foundry", and the manifest is unchanged.
```

### P5

Outcome

```text
On GET /workspace/integrations and the new-route form, the words of W-2, W-3, K-2 and K-19 are unchanged.
```

### P6

Outcome

```text
RELEASING.md R-1 and R-2 and deploy/k8s/README.md describe setting up Slack and Matrix from Instance admin › Integrations in the browser, with the environment settings as the optional operator alternative.
```

## Scope

```text
US-IIS-04 (AC 1-5): I-19 on the workspace integrations page and the new-route form (GET and its preview and save posts), the per-viewer link rule, I-20 on the Slack setup guide, and the RELEASING.md R-1/R-2 and deploy/k8s/README.md rewording.
```

```text
US-IIS-05 and US-IIS-06 AC 6 (Slack Replace/Remove, I-14 confirm) are Request 2; US-IIS-07 (Replace secrets key, Start over) is Request 3. No change to the instance integrations page itself, to W-2/W-3/K-2/K-19 words, or to the Slack app manifest.
```

## Decisions

```text
Copy I-19 and I-20 are the locked copy approved in instance-integration-setup DISCUSS (discuss/user-stories.md "Copy (locked once approved)"); W-2, W-3, K-2 and K-19 keep their shipped words.
```

#### Status

Confirmed


## Journey

None.

## Slices

### S1

```text
An instance admin who reads W-2 or W-3 on the workspace integrations page or a new-route form is offered "Set up integrations for this Foundry" after any existing link and lands on the instance integrations page; every other viewer sees only the links they can open, and the W-2, W-3, K-2 and K-19 words are unchanged.
```

#### Promises

P1

P2

P3

P5

#### Depends on

None.

#### Slice work

S

```text
An instance admin fixes a missing Slack or Matrix setup from the page where she noticed it
```

##### Rework

None.

### S2

```text
The Slack setup guide shows I-20 and asks for no restart, and the operator docs describe browser setup with the environment as the alternative.
```

#### Promises

P4

P6

#### Depends on

S1

#### Slice work

S

```text
Nobody is told to restart Foundry to set up Slack
```

##### Rework

None.
