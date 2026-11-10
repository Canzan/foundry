# Product brief: iis\-slack\-replace\-remove

## Request

```text
User request (verbatim): "Let's continue delivering any nWave 3 feature in nWave 4 from @docs/feature/"

Selected scope (user-approved split, 2026-11-08): instance-integration-setup Deliverable B, Request 2 of 3 — US-IIS-05 "Replace or remove the Slack app, knowing what it affects" and US-IIS-06 AC 6, as written in docs/feature/instance-integration-setup/discuss/user-stories.md (US-IIS-05 lines 716-828; US-IIS-06 AC 6), with the locked copy I-11, I-14, I-15, I-16, I-17 and I-18 from that file's "Copy (locked once approved)" table. Request 1 (US-IIS-04, feature iis-setup-links) is delivered at 3659b9c. Request 3 (US-IIS-07) follows separately. Carried forward from Deliverable A (docs/evolution/2026-11-07-instance-integration-setup.md): item 10 (Replace activity wording) is in scope; items 27, 32 and 35 stay out unless the design needs them.
```

## Work

M

```text
An instance admin rotates a Slack secret, switches to another Slack app, or removes Slack from the browser, seeing how many connected workspaces each change affects before anything changes
```

### Rework

None.

## Promises

### P1

Outcome

```text
POST /admin/instance/integrations/slack (replace) with the saved app's client ID: the form shows the client ID pre-filled; a blank secret field keeps the saved value; there is no confirm; the change applies as in US-IIS-03 AC 1 (Slack events verify with the new signing secret on every replica within 60 seconds); Priya reads "Saved. Workspace admins can now choose Add to Slack."
```

### P2

Outcome

```text
POST /admin/instance/integrations/slack with a client ID different from the current app's requires both secrets; a blank one answers I-11 ("Enter the {field} from the Slack app's Basic Information.") and changes nothing.
```

### P3

Outcome

```text
POST /admin/instance/integrations/slack with a different client ID while Slack connections exist redirects to GET /admin/instance/integrations/slack/confirm, which shows I-14 with the connected-workspace count and the current client ID (the n = 1 form for one workspace) and changes nothing until "Save the new app"; the submitted app is held sealed for 10 minutes so the confirm page holds no secret; only the same instance admin's POST /admin/instance/integrations/slack/confirm saves it. This applies equally when the current app is the environment's (US-IIS-06 AC 6).
```

### P4

Outcome

```text
GET /admin/instance/integrations/slack/remove shows I-15 (with the connected-workspace count; the n = 0 form when none is connected) and changes nothing, including when prefetched; POST of the same path removes the saved app and answers I-16 "Removed. Slack is not set up on this Foundry."
```

### P5

Outcome

```text
After removal: GET /workspace/integrations reads W-2 for Slack; POST /integrations/slack/events answers 401; existing Slack connection states on GET /workspace/integrations are not marked lost and no workspace admin is emailed; pending deliveries on the route history wait under the 24-hour give-up.
```

### P6

Outcome

```text
GET /admin/instance/integrations Activity shows I-18 lines "{date} · {name} replaced the Slack app ({client_id})" and "{date} · {name} removed the Slack app ({client_id})".
```

### P7

Outcome

```text
With the environment's Slack app in effect, GET /admin/instance/integrations offers neither Replace nor Remove, and POST /admin/instance/integrations/slack/remove changes nothing and answers the page.
```

## Scope

```text
US-IIS-05 AC 1-6 and US-IIS-06 AC 6: Replace (same app, other app), the I-14 confirm with the sealed pending app (instance_slack_app_pending in its own migration, per the evolution doc), Remove with the I-15 confirm and I-16, after-removal effects, I-18 replaced/removed activity, and the environment-app rules; KPI-IIS-4.
```

```text
US-IIS-07 (Replace secrets key, Start over) is Request 3. No change to the Slack app manifest, to workspace connect/disconnect flows, or to W-2/W-3/K-2/K-19 words; Deliverable A items 27, 32 and 35 unless the design needs them.
```

## Decisions

```text
Copy I-11, I-14, I-15, I-16, I-17 and I-18 are the locked copy approved in instance-integration-setup DISCUSS (discuss/user-stories.md "Copy (locked once approved)"); D7 narrowed (different client ID requires both secrets) was approved by the user 2026-11-05.
```

#### Status

Confirmed


```text
I-15 gains an n = 1 form, approved by the user 2026-11-09: "Remove the Slack app from this Foundry? 1 workspace's Slack routes stop posting until it is set up again, and no workspace can connect Slack." (n ≥ 2 and n = 0 forms unchanged).
```

#### Status

Confirmed


## Journey

None.

## Slices

### S1

```text
An instance admin rotates a secret of the saved Slack app, or saves another app with both secrets, from the instance integrations page, and the change takes effect on every replica without a restart.
```

#### Promises

P1

P2

P6

#### Depends on

None.

#### Slice work

S

```text
Rotate a leaked or regenerated Slack secret in the browser
```

##### Rework

None.

### S2

```text
Saving a different Slack app while workspaces are connected first shows how many are connected through the current app and saves only when the same instance admin confirms, also when the current app comes from the environment.
```

#### Promises

P3

P6

#### Depends on

S1

#### Slice work

M

```text
Switch Slack apps knowing which workspaces keep the old one
```

##### Rework

None.

### S3

```text
An instance admin removes the saved Slack app after reading what it stops; afterwards workspaces read that Slack is not set up, events are refused, and nobody is told a connection was lost.
```

#### Promises

P4

P5

P6

P7

#### Depends on

S1

#### Slice work

M

```text
Retire Slack knowing what stops
```

##### Rework

None.
