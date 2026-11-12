# Product brief: iis\-secrets\-key\-replace

## Request

```text
Deliverable B Request 3: US-IIS-07 (docs/feature/instance-integration-setup/discuss/user-stories.md). An instance admin replaces the secrets key from the instance integrations page (GET then POST /admin/instance/integrations/secrets-key/replace, I-23 then I-24): every stored credential, the saved Slack app included, is re-sealed under a new key and nobody reconnects; a failed re-seal changes nothing. While the stored secrets key cannot be opened (I-26), Replace is withheld and the page offers Start over (GET then POST /admin/instance/integrations/secrets-key/start-over) with its cost (I-28): credentials it cannot open are discarded, workspaces reconnect, the Slack app is entered again.
```

## Work

M

```text
Priya can rotate the secrets key the moment she doubts it without making any workspace reconnect, and when the instance key is lost with no backup she has a confirmed way forward whose cost she reads before choosing.
```

### Rework

None.

## Promises

### P1

Outcome

```text
With a stored secrets key that opens, GET /admin/instance/integrations offers Replace secrets key next to I-2, and GET /admin/instance/integrations/secrets-key/replace shows the Replace secrets key button with I-23 for the count of saved credentials: "Replace the secrets key? Foundry seals its {n} saved credentials again under a new key. Nobody needs to reconnect." for n >= 2, "Replace the secrets key? Foundry seals its 1 saved credential again under a new key. Nobody needs to reconnect." for n = 1, and "Replace the secrets key? This Foundry has no saved credentials yet, so nothing needs to be sealed again." for n = 0. The page changes nothing, including when prefetched.
```

### P2

Outcome

```text
POST /admin/instance/integrations/secrets-key/replace generates a new secrets key, stores it encrypted under the instance key, re-seals every saved credential and the key check all-or-nothing, and redirects to GET /admin/instance/integrations, which reads I-24 once: "Replaced. {n} saved credentials are sealed under the new key." for n >= 2, "Replaced. 1 saved credential is sealed under the new key." for n = 1, and "Replaced. This Foundry has a new secrets key." for n = 0. A reload posts nothing again and claims no second replacement.
```

### P3

Outcome

```text
A failed re-seal changes nothing: the old secrets key stays in effect, every saved credential still opens, the Activity records no replacement, and the page does not read I-24.
```

### P4

Outcome

```text
Observed on GET /workspace/integrations and the route history, within 60 seconds of a replacement every replica opens every credential; in between no connection reads lost, the mail double receives no email, and the next new issue of a team member still reaches its Slack channel and its Matrix room.
```

### P5

Outcome

```text
GET /admin/instance/integrations Activity shows the I-18 lines "{date} - {name} replaced the secrets key ({n} credentials sealed again)" and "{date} - {name} started over with a new secrets key ({n} credentials discarded)".
```

### P6

Outcome

```text
While the stored secrets key cannot be opened (I-26) and no running replica opens it, GET /admin/instance/integrations appends Start over to I-26 and offers no Replace secrets key. While another live server still opens the key, I-26 stays restore-only with no Start over and adds "Another Foundry server still opens the secrets key, so Start over is not offered."; under I-31 GET /admin/instance/integrations reads I-31 in the instance key line's place, reads no I-26 words, and offers no Replace secrets key, no Start over and no live-server sentence. GET and POST /admin/instance/integrations/secrets-key/replace under I-26 change nothing and answer the page.
```

### P7

Outcome

```text
GET /admin/instance/integrations/secrets-key/start-over is answered only under I-26 with that same gate; it shows the I-28a Confirmation field "Type START OVER to confirm that no Foundry server of an earlier release is still running." and I-28 for the count of credentials this Foundry cannot open: "Start over with a new secrets key? The {n} saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again." for n >= 2, the same sentence with "The 1 saved credential this Foundry cannot open is discarded" for n = 1, and "Start over with a new secrets key? This Foundry has no saved credentials to discard." for n = 0. It changes nothing.
```

### P8

Outcome

```text
POST /admin/instance/integrations/secrets-key/start-over with the Confirmation START OVER, and the gate checked again inside the POST, generates a new secrets key, discards the credentials this Foundry cannot open, records the I-18 started over line, and redirects to GET /admin/instance/integrations, which reads I-21 "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it." once. A wrong or empty Confirmation, or a running replica that opens the key, answers I-28 again and changes nothing.
```

### P9

Outcome

```text
After a start-over, Matrix is ready, and GET /workspace/integrations shows no discarded connection as connected and tells its workspace admin to reconnect Slack and to re-enter the Matrix bot token; the instance integrations page asks for the Slack app again (I-5).
```

### P10

Outcome

```text
After a replacement or a start-over, rotating the instance key itself with the CLI foundry doctor rotate-secrets leaves every saved credential opening.
```

### P11

Scenario

```text
Replacing the key re-seals everything and nobody reconnects
```

```text
This Foundry holds 3 saved credentials for Canzan Labs and the Slack app, and the stored secrets key opens.
```

```text
Priya confirms Replace secrets key.
```

```text
Priya reads "Replaced. 3 saved credentials are sealed under the new key.", the next new issue of Sam in Identity Platform reaches #auth-dev and #auth:canzan.dev, no connection is marked lost, and no workspace admin is emailed.
```

### P12

Scenario

```text
Starting over states its cost first, then leaves workspaces told to reconnect
```

```text
The stored secrets key cannot be opened with the instance key of this server, 3 saved credentials cannot be opened, and no running replica opens the key.
```

```text
Priya chooses Start over, reads the cost, types START OVER and confirms.
```

```text
Before she confirms, Priya reads "Start over with a new secrets key? The 3 saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again." and nothing has changed; after she confirms, she reads "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it." once, and the workspace integrations page of Canzan Labs offers to connect Matrix and does not read Slack as connected.
```

## Scope

```text
US-IIS-07 AC 1-6 of docs/feature/instance-integration-setup/discuss/user-stories.md: the Replace confirm and its apply (I-23, I-24) with an all-or-nothing re-seal and every replica opening every credential within 60 seconds, the withheld Replace and offered Start over under I-26 with its live-replica gate, the Start over confirm with its cost and typed confirmation (I-28, I-28a), the Start over apply with discarded credentials and the reconnect state of the workspace pages, the I-18 activity lines, and the CLI instance-key rotation that leaves every credential opening; KPI-IIS-4 and KPI-IIS-G1.
```

```text
Out: generating or moving the secrets key and the operator-side restore of the instance key, as Deliverable A shipped them (I-2, I-2e, I-2e0, I-3, I-21, I-22, I-25, the I-26 restore text, I-27, I-30, I-31); Replace and Remove of the Slack app (Request 2, delivered) and the setup links of US-IIS-04 (Request 1, delivered); no change to the Slack app manifest, to the workspace connect and disconnect flows, or to the W-2, W-3, K-2 and K-19 words.
```

```text
Out: how the new key is stored and any grace window for the old one. D4 (the old secrets key is discarded once all replicas use the new one, approved by the user 2026-11-05) leaves the window to DESIGN; this value promises only the observable outcome.
```

## Decisions

```text
The copy I-23, I-24, the Start over appended to I-26, I-28, I-28a and the Deliverable B I-18 lines are the locked copy of instance-integration-setup DISCUSS (section "Copy (locked once approved)"); I-28a (typing START OVER) and D4 (the old secrets key is discarded once all replicas use the new one) were approved by the user 2026-11-05. This value adds no new product decision to that authority.
```

#### Status

Confirmed


```text
Replace and Start over answer post, redirect, get, and the notice waits once in the session of the acting instance admin, as D16 of iis-slack-replace-remove already decided for the Slack app changes: I-24 is never a query value, an address alone claims no change that was not made, and a session that cannot hold the notice leaves the committed change with no notice.
```

#### Status

Confirmed


```text
The n = 1 forms, following I-22 and the I-15 n = 1 form: I-23 reads "Replace the secrets key? Foundry seals its 1 saved credential again under a new key. Nobody needs to reconnect."; I-24 reads "Replaced. 1 saved credential is sealed under the new key."; I-28 reads "Start over with a new secrets key? The 1 saved credential this Foundry cannot open is discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again." The n >= 2 forms stay as locked. The user confirmed these exact words on 2026-11-11.
```

#### Status

Confirmed


```text
The n = 0 forms, following I-2e0 and the I-15 n = 0 form: Replace and Start over stay offered while nothing is saved, as I-2 does. With a stored secrets key and nothing saved yet, I-23 reads "Replace the secrets key? This Foundry has no saved credentials yet, so nothing needs to be sealed again." and I-24 reads "Replaced. This Foundry has a new secrets key."; under I-26 with no credential that cannot be opened, I-28 reads "Start over with a new secrets key? This Foundry has no saved credentials to discard." The user confirmed these exact words on 2026-11-11.
```

#### Status

Confirmed


```text
After a successful start over, GET /admin/instance/integrations reads I-21 "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it." once, as I-2e0 has a move with n = 0 answer I-21. The locked copy names no other notice for this apply. The user confirmed on 2026-11-11.
```

#### Status

Confirmed


```text
While another live server still opens the stored secrets key, I-26 stays restore-only and the page adds one sentence, so that Priya can tell this state from the plain restore case. Its locked exact words are "Another Foundry server still opens the secrets key, so Start over is not offered." Under I-31 the page reads I-31 in the instance key line's place, reads no I-26 words, and offers no Replace secrets key, no Start over and no live-server sentence. The user confirmed these exact words on 2026-11-11.
```

#### Status

Confirmed


```text
Quint relevance of this one value: a bounded Quint exploration is not a product prerequisite here, because the Request and the locked US-IIS-07 copy already state every outcome recorded in these promises (nw-nwdes guidance). The six open interleaving and safety questions - Q1 an all-or-nothing re-seal, Q2 no lost marking and no email in the window where replicas hold different keys, Q3 the live-replica gate between the GET and the POST, Q4 reload, prefetch and double submit, Q5 no dead-end state, and Q6 replace-then-start-over ordering - pass to DESIGN as obligations the solution architect must verify formally with a model and receipts, as the design of Request 2 did with Quint and TLA+. The user decided this on 2026-11-11. Jev: typesafe unavailable (TYPESAFE_API_KEY absent), so the copy and slice judgments here stay local.
```

#### Status

Confirmed


## Journey

```text
Priya doubts the secrets key of this Foundry and opens the instance integrations page, where Replace secrets key is offered next to the stored key.
```

```text
Not supplied; the authority records no account of emotion.
```

```text
Priya reads that the 3 saved credentials of Canzan Labs and the Slack app are sealed again under a new key and that nobody needs to reconnect, confirms, and reads that they are sealed under the new key; the next issue of Sam still reaches #auth-dev and #auth:canzan.dev.
```

```text
Not supplied; the authority records no account of emotion.
```

```text
A docker compose volume is recreated and the instance key file is new, so the page reads I-26 and offers no Replace; Priya looks for a backup of that file and would restore it and restart.
```

```text
Not supplied; the authority records no account of emotion.
```

```text
No backup exists, so Priya opens Start over, reads that the 3 credentials this Foundry cannot open are discarded, types START OVER and confirms; Matrix is ready, the workspace admins are told to reconnect Slack and re-enter the Matrix bot token, and Priya enters the Slack app again.
```

```text
Not supplied; the authority records no account of emotion.
```

## Slices

### S1

```text
An instance admin replaces the secrets key from the instance integrations page: she reads how many saved credentials are sealed again and that nobody needs to reconnect, confirms, and reads that they are sealed under the new key; within 60 seconds every replica opens every credential, no connection reads lost and no workspace admin is emailed, and a failed re-seal leaves the old key in effect with every credential still opening.
```

#### Promises

P1

P2

P3

P4

P5

P10

P11

#### Depends on

None.

#### Slice work

M

```text
Rotate the secrets key the moment she doubts it, without making any workspace reconnect.
```

##### Rework

None.

### S2

```text
While this Foundry cannot open its stored secrets key and no running replica opens it, the instance admin reads on the instance integrations page that Replace is withheld and Start over is offered, opens Start over, and reads its exact cost and the confirmation it asks for; nothing has changed yet, and while another live server still opens the key the page stays restore-only.
```

#### Promises

P6

P7

#### Depends on

S1

#### Slice work

S

```text
See a confirmed way forward and its cost before she chooses between restoring the instance key and starting over.
```

##### Rework

None.

### S3

```text
The instance admin confirms Start over with the typed confirmation: a new secrets key is made, the credentials this Foundry cannot open are discarded, the Activity records it, Matrix is ready, and every workspace integrations page shows no discarded connection as connected and tells its admin to reconnect Slack and re-enter the Matrix bot token; a wrong or empty confirmation changes nothing.
```

#### Promises

P8

P9

P5

P10

P12

#### Depends on

S2

#### Slice work

M

```text
Get out of a lost instance key with no backup, at a cost she read first.
```

##### Rework

None.
