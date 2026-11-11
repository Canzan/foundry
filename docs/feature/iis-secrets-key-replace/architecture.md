# Architecture brief: iis\-secrets\-key\-replace

## Paradigm

object_oriented

## Shared records

### Decisions

#### D1

```text
Claim to checker: the Replace and Start over state machine - Q1 all-or-nothing, Q3 the live-replica gate inside the POST, Q4 reload, prefetch and double submit, Q5 no dead end, Q6 the ordering of replace and start over, and the safety half of Q2 - is a Quint state model (docs/feature/iis-secrets-key-replace/design/secrets_key_replace.qnt), exhaustively checked with Apalache over the compiled model. The temporal half of Q2 and P4 - the window where replicas hold different keys closes with no restart and nothing queued in it is lost - needs fairness, which Quint cannot carry on this host, so it is TLA+ with TLC (design/reseal_convergence.tla) in the shape Request 2 used.
```

#### D2

```text
One write use-case per apply, under the instance-integrations advisory lock that every instance write takes: Store::replace_secrets_key and Store::start_over_secrets_key stand beside store_secrets_key and rotate_instance_key in crates/foundry-store/src/integrations/instance.rs and reuse reseal_sealed_values, so the new wrapped key, every re-sealed value, the key check, the activity line and the version bump commit together or roll back together. A refusal from the re-seal closure rolls the transaction back, as Generate, Move and the rotation already do.
```

#### D3

```text
D4 resolved, no grace window: the integration_secrets_key row holds exactly one key. The replace seals every saved value under the new key in the same transaction, so after the commit no replica needs the old key, and the old wrapped key is overwritten, not kept. A replica that still holds the old secrets key in memory opens nothing it re-reads and answers SecretUnavailable, which the delivery backoff retries; its next reload adopts the version bump and re-derives the new key from the row wrapped under the instance key it holds. Nothing marks a connection lost and nothing emails on a credential this server cannot open (DD-2 is a per-connection judgement, not a loss), so the window costs silence and a retry, never a reconnect.
```

#### D4

```text
Replace is withheld wherever Generate and Move are withheld: under I-26 (the stored key does not open) and under I-4 (some saved value no held key opens). Under I-4 the all-or-nothing re-seal could never succeed, so offering Replace would be a control that always refuses; the page keeps the shipped I-4 restore words. KeyMode gains replaces(unopenable_values) beside generates and moves, so the four offers are decided by one closed type.
```

#### D5

```text
The live-replica gate reads integration_replica_states: a replica counts as still opening the stored secrets key while its row says opens_secrets_key and its beat_at is no older than the gate window, which is three reload intervals (FOUNDRY_INTEGRATIONS_RELOAD_MS, default 5000 ms, so 15 s by default) and never longer than the hour the retention prune keeps rows for. The gate is read again inside the POST own locked transaction, as D10 reads the connected-workspace count inside the write, so a replica that beats while the post is in flight cannot be started over from under.
```

#### D6

```text
Carriers and their owning modules. A new module crates/foundry-app/src/integrations/secrets_key_rotation.rs owns the domain decisions of this value: SecretsKeyOffer (Replace, StartOver) for what a page offers; UnopenableOffer (StartOver, RestoreOnlyLiveReplica, RestoreOnly) for the three I-26 states; SavedCredentials, a value object that counts by the I-22 rule (each connection one, the saved Slack app one) and selects the n >= 2, n = 1 and n = 0 copy; StartOverConfirmation, which admits only the trimmed text START OVER; ReplacePlan and StartOverPlan over a GeneratePlan. The store owns the outcome types ReplaceOutcome and StartOverOutcome (Replaced, StartedOver, Refused(E), KeyChanged). Per-process state is unchanged: Integrations::reboot rebuilds and installs the keyring as Generate and Move already do.
```

#### D7

```text
Post, redirect, get with a one-shot session notice, as D16 decided for the Slack app changes: SavedNotice gains Replaced(count) and StartedOver, the count carried in the session value and never in the address, so a reload posts nothing again, the page reads I-24 or I-21 once, and a session that cannot hold the notice leaves the committed change with no notice. Both confirm pages are ReadOnlyDoors over PageReads, so a prefetch and a link preview change nothing.
```

#### D8

```text
The GET and POST of each door answer only where the door applies; anywhere else they change nothing and answer the instance integrations page, as Generate and Move do. GET and POST .../secrets-key/replace under I-26 change nothing and answer the page; GET and POST .../secrets-key/start-over are answered only under I-26 with the live-replica gate.
```

#### D9

```text
Cross-layer algebra. Domain (keys.rs, secrets_key_rotation.rs): states KeyMode x the saved-credential count x the gate; operations plan a replacement or a start over and select copy; observations are the page offer, the chosen copy and the count; a plan is equal to another when it names the same key generation and the same slot decisions. Application/ports (handlers/instance.rs, store_capabilities.rs): operations are the two GETs and the two POSTs; outcomes are the page answered unchanged, a redirect with a notice, or a refusal that answers the confirm page again. Adapter (foundry-store instance.rs): operations are the two locked transactions; observations are the stored rows, the activity line and the config version; failures are the re-seal refusal, the compare-and-set loss and the database error, each rolling back. Infrastructure/recovery (reload.rs, worker.rs): operations are the reload and the delivery attempt; observations are applied_version, the key a replica holds and the delivery settlement; a credential no held key opens is SecretUnavailable settled by the backoff policy, never a loss and never an email.
```

### Constraints

#### K1

```text
The locked copy of instance-integration-setup DISCUSS, exact words, with no form invented: I-23 in its n >= 2, n = 1 and n = 0 forms; I-24 in its n >= 2, n = 1 and n = 0 forms; I-28 in its n >= 2, n = 1 and n = 0 forms; I-28a "Type START OVER to confirm that no Foundry server of an earlier release is still running."; the sentence appended to I-26 while another live server still opens the key, "Another Foundry server still opens the secrets key, so Start over is not offered."; I-21 as the answer of a successful start over; and the two Deliverable B I-18 lines "{date} - {name} replaced the secrets key ({n} credentials sealed again)" and "{date} - {name} started over with a new secrets key ({n} credentials discarded)".
```

#### K2

```text
The count n is the I-22 counting rule the store already applies: each connection credential one, the saved Slack app one however many of its secrets are sealed; the app held for its confirm is re-sealed with the rest and counted with the app. The authority states no upper bound on the number of credentials, and none is introduced: both writes walk every stored sealed row under FOR UPDATE.
```

#### K3

```text
The typed confirmation is admitted only as the exact text START OVER after surrounding whitespace is trimmed, case-sensitive; an empty, lower-case or otherwise different value answers I-28 again and changes nothing. The authority fixes the words and not the trimming, so the trimming is a design choice, narrower than a free-text field and wider than a byte-exact comparison.
```

#### K4

```text
Within 60 seconds of a replacement every replica opens every credential. The design carries this as the eventual convergence the TLA+ model checks, times the reload interval FOUNDRY_INTEGRATIONS_RELOAD_MS (default 5000 ms): the model proves the convergence, not the wall-clock figure, which follows from that configured interval. An operator who sets the interval above 20 s loses the 60-second figure; that bound is on the setting, not on the code.
```

#### K5

```text
No schema change: both writes use the 0033 tables as they are, and the two new activity actions are new text values of instance_integration_audit_events.action, which ADR-IIS-005 r1 already treats as wire names. No migration ships with this value.
```

### Reuse

None.

### Impact

None.

### Agreement

instance\_integration\_audit\_events\.action wire names \(ADR\-IIS\-005 r1\)\: persisted text read back by another release

crates\/foundry\-store\/src\/integrations\/instance\.rs\:341

Producer

Migrated

```text
The Activity enum gains SecretsKeyReplaced ("secrets_key_replaced") and SecretsKeyStartedOver ("secrets_key_started_over"); the six shipped names keep their text, so rows written by the shipped release still read.
```

instance\_integration\_audit\_events\.action wire names \(ADR\-IIS\-005 r1\)\: persisted text read back by another release

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:1

Consumer

Migrated

```text
The I-18 renderer gains the two Deliverable B lines with their counts; the generic underscore fallback must never answer for either, which the contract test observes.
```

integration\_replica\_states rows\: opens\_secrets\_key and beat\_at\, written by every replica reload and read by the live\-replica gate

crates\/foundry\-app\/src\/integrations\/reload\.rs\:190

Producer

UnchangedCompatible

```text
The beat already carries opens_secrets_key (mode == Sealed) and the caller clock beat_at; the gate needs no new column and no new write.
```

integration\_replica\_states rows\: opens\_secrets\_key and beat\_at\, written by every replica reload and read by the live\-replica gate

crates\/foundry\-store\/src\/integrations\/instance\.rs\:784

Consumer

UnchangedCompatible

```text
Store::replica_states already returns every row; the gate filters on opens_secrets_key and the freshness window and is added to the StoreCapabilities trait, with no change to the row shape. A replica of an earlier release beats the same columns, which is exactly why the gate can see it.
```

The one\-shot session notice under the key integrations\.slack\_notice\: a wire name stored in the admin session and read by the next page

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:382

Producer

Migrated

```text
Two new wire names carry a count ("replaced:{n}", "started-over"); an unrecognised stored value still reads as the page itself, so a session written by the shipped release is never misread.
```

The one\-shot session notice under the key integrations\.slack\_notice\: a wire name stored in the admin session and read by the next page

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:1

Consumer

Migrated

```text
show_instance parses the count out of the stored value; of_wire keeps answering None for anything it does not name.
```

### Not applicable

None.

### Boundaries

```text
HTTP, the production axum router behind require_instance_admin with csrf_middleware and session_layer: GET and POST /admin/instance/integrations/secrets-key/replace, GET and POST /admin/instance/integrations/secrets-key/start-over, GET /admin/instance/integrations; and for the observed outcome GET /workspace/integrations and the CLI foundry doctor rotate-secrets. Everyone without the right reads the uniform 404.
```

#### Driven ports

```text
Postgres through foundry-store Store, reached by the StoreCapabilities trait: the two new locked write use-cases, instance_snapshot, replica_states, connected_slack_workspaces and the activity read.
```

```text
The instance key source (InstanceKeySource over the key file and the FOUNDRY_INSTANCE_KEY setting), unchanged.
```

```text
The clock, injected: every beat_at, the gate freshness judgement and every activity line take the caller clock, never now().
```

```text
The chat vendors through the existing delivery channels; no new outbound call.
```

```text
Inward: the axum handlers depend on crates/foundry-app/src/integrations/secrets_key_rotation.rs and keys.rs, which depend on foundry-auth secret_box and on the foundry-store types; the domain modules call no handler and no store adapter, and the store never calls back into the app. The new re-seal closures stay inside the integrations module, as check-arch integrations-secret-seam requires.
```

#### Failures

##### B1

```text
The re-seal closure cannot open a stored sealed value, or cannot seal it again under the new key.
```

Refusal

```text
The transaction is rolled back, the old key stays in effect, every credential still opens, no activity line is written, no version bump, and the confirm page is answered again with no I-24 anywhere.
```

##### B2

```text
The stored secrets key is no longer the one the plan was made from (another admin replaced it first).
```

Refusal

```text
The compare-and-set matches no row, the transaction is rolled back and the page is answered as it is; the first replacement stands and is the only one recorded.
```

##### B3

```text
A replica with a fresh heartbeat still opens the stored secrets key when the start over reaches its transaction.
```

Refusal

```text
Nothing is discarded, no key is stored, and the start-over confirm page is answered again with I-28.
```

##### B4

```text
A wrong, empty or differently cased Confirmation field.
```

Refusal

```text
The start-over confirm page again with I-28; the stored rows, the activity list and the config version unchanged.
```

##### B5

```text
The acting admin session cannot hold the notice.
```

Refusal

```text
The change stands committed and the page she lands on reads no notice; a warning is logged, as D16 already decided for the Slack changes.
```

##### B6

```text
The database cannot be reached while a confirm page is being read.
```

Indeterminate

```text
The count is unread rather than zero: the page answers without claiming a count, because an unread count is not none saved; nothing is written.
```

##### B7

```text
A replica cannot open a credential because it has not yet reloaded after a replacement.
```

Retry

```text
The delivery settles as SecretUnavailable under the backoff policy and goes out after a later attempt; the connection is not marked lost, no workspace admin is emailed, and the workspace page reads the shipped not-readable words meanwhile.
```

##### B8

```text
The process fails between the commit and the acting replica rebuild.
```

Retry

```text
The commit stands; the replica that comes back reads the new key at boot, and the admin sees her notice or, if the session died with it, the page as it is. Nothing is half applied.
```

### Observations

```text
What GET /admin/instance/integrations offers and reads next to I-2 or I-26: the Replace secrets key control, the Start over link, the appended live-replica sentence, and the one-shot notice.
```

PageState

```text
Which key generation is stored, how many times the key was replaced before, and whether another replica has reloaded yet.
```

```text
The confirm copy on GET .../secrets-key/replace and GET .../secrets-key/start-over, selected by the saved-credential count.
```

String

```text
Which slots those credentials are (a connection against the saved Slack app), and whether the count was read before or after another admin saved a connection.
```

```text
What a POST answers: the redirect to GET /admin/instance/integrations with the notice waiting in the acting admin session, or the confirm page again with nothing changed.
```

Response plus the session notice

```text
Whether a refusal was the re-seal, the compare-and-set or the gate; the page reads the state, not the cause.
```

```text
The Activity lines on GET /admin/instance/integrations, newest first.
```

Vec\<InstanceActivityRow\> rendered as I\-18 words

```text
The key generations involved, and a replacement that rolled back, which records nothing.
```

```text
Whether a replica opens every stored credential, observed through GET /workspace/integrations (no W-2 or K-19 cause) and through a delivery that reaches its Slack channel and its Matrix room.
```

ConnectionWords and the delivery outcome

```text
Which key generation the credential is sealed under, and how many reloads it took to get there.
```

```text
Whether a workspace admin was emailed and whether a connection reads lost, observed through the mail double and the connection state.
```

Emails received and lost\_reason

```text
An email suppressed by a muted workspace from an email never composed; the acceptance oracle therefore observes the mail double with no suppression in force.
```

### Equality

the page and its copy

```text
Two states are equal when PageState and the confirm copy agree: the same offer, the same secrets-key line with the same appended sentences, the same count form and the same notice. A different stored key generation with the same offers and the same count is the same state under this equality, which is why the design never shows a key id.
```

a committed apply

```text
Two applies are equal when they leave the same stored rows up to the key generation: the same set of saved slots, every one sealed under the stored key, the same key check generation, one activity line of the same action with the same count, and the version bumped once. A replacement is never equal to a start over: the slot sets differ whenever this Foundry cannot open a credential.
```

### Constructors

SavedCredentials\:\:of

fn of\(connections\: usize\, saved\_app\: bool\) \-\> SavedCredentials

StartOverConfirmation\:\:typed

fn typed\(field\: \&str\) \-\> Option\<StartOverConfirmation\>

UnopenableOffer\:\:decide

fn decide\(mode\: KeyMode\, live\_opener\: bool\, key\_file\: KeyFile\) \-\> UnopenableOffer

KeyMode\:\:replaces

fn replaces\(self\, unopenable\_values\: bool\) \-\> bool

GeneratePlan\:\:reseal\_under

fn reseal\_under\(\&self\, opening\: \&SecretBox\, slot\: \&SecretSlot\, sealed\: \&\[u8\]\) \-\> Option\<Vec\<u8\>\>

Store\:\:replace\_secrets\_key

async fn replace\_secrets\_key\<E\>\(\&self\, from\_key\_id\: \&str\, key\: \&NewSecretsKey\, canary\: \&KeyCheck\, actor\: Uuid\, at\: OffsetDateTime\, reseal\: impl FnMut\(SealedSlot\, \&\[u8\]\) \-\> Result\<Option\<Vec\<u8\>\>\, E\>\) \-\> Result\<ReplaceOutcome\<E\>\, StoreError\>

Store\:\:start\_over\_secrets\_key

async fn start\_over\_secrets\_key\(\&self\, from\_key\_id\: \&str\, key\: \&NewSecretsKey\, canary\: \&KeyCheck\, actor\: Uuid\, at\: OffsetDateTime\, gate\: impl FnOnce\(\&\[ReplicaState\]\) \-\> bool\) \-\> Result\<StartOverOutcome\, StoreError\>

### Laws

#### L1

```text
A re-seal is all or nothing: in every reachable state every saved slot is sealed under the stored secrets key and the stored key check names that same key. No state has one credential under the new key and another under the old one.
```

Invariant

Adapter

Store\:\:replace\_secrets\_key and Store\:\:start\_over\_secrets\_key\, crates\/foundry\-store\/src\/integrations\/instance\.rs

##### Treatment

Broad

###### Generator

```text
Every interleaving of the two GETs, the two POSTs with each confirmation, the refusal of each POST, a reload or heartbeat of either replica, a delivery attempt on either replica, the loss and restore of the instance key, and the CLI instance-key rotation.
```

2 replicas\, 3 sealed slots\, 12 to 14 steps

fixed seed 0x5ec1e7\, recorded with each run

###### Must include

```text
a replacement followed by a start over
```

```text
a start over followed by a replacement
```

```text
a POST whose re-seal refuses
```

14

```text
Quint reports the first violating trace in full; the model is small enough that the trace is already minimal.
```

```text
The model observes the sealed-under generation of every slot and the key check generation, which no action may set apart from one another.
```

```text
Mutant m1: the replace stores the new key and its check without re-sealing the saved values in the same transaction.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

#### L2

```text
No connection is marked lost and no workspace admin is emailed because of the window in which replicas hold different keys: a credential no held key opens is SecretUnavailable, settled by the delivery backoff policy.
```

Invariant

Infrastructure

crates\/foundry\-app\/src\/integrations\/worker\.rs channel and the RetryPolicy settlement

##### Treatment

Broad

###### Generator

```text
The same interleavings, with a delivery attempt admitted on any replica whose held key is not the one the credential is sealed under.
```

2 replicas\, 3 sealed slots\, 12 to 14 steps

fixed seed 0x5ec1e7

###### Must include

```text
a delivery attempt on a replica that has not reloaded after a replacement
```

14

```text
the first violating trace, reported in full
```

```text
Two ghost observations the design forbids: a connection marked lost and an email sent. They are set by nothing in the design.
```

```text
Mutant m4: a replica that cannot open a credential marks the connection lost and emails its admins.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

#### L3

```text
No start over commits while a replica with a fresh heartbeat still opens the stored secrets key: the gate is read again inside the POST own locked transaction, not only at the GET.
```

Invariant

Application

Store\:\:start\_over\_secrets\_key gate closure\, read inside the locked transaction

##### Treatment

Broad

###### Generator

```text
Every interleaving in which the other replica beats fresh or goes stale between the GET and the POST.
```

2 replicas\, 14 steps

fixed seed 0x5ec1e7

###### Must include

```text
a heartbeat that becomes fresh again between the GET and the POST
```

14

```text
the first violating trace, reported in full
```

```text
A ghost flag set at the moment of the apply if any live replica then opened the stored key.
```

```text
Mutant m5: the POST checks only I-26 and not the gate again.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

#### L4

```text
Every durable write comes from a POST: a GET of either confirm page, a prefetch of it and a link preview of it change no row, write no activity line and bump no version.
```

Invariant

Application

ReadOnlyDoor over PageRead\, crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

##### Treatment

Broad

###### Generator

```text
Every interleaving of the two GETs and the two POSTs in every key state.
```

2 replicas\, 14 steps

fixed seed 0x5ec1e7

###### Must include

```text
a GET of each confirm page with no POST after it
```

14

```text
the first violating trace, reported in full
```

```text
A durable-write counter against a POST counter; a GET that writes makes writes exceed posts.
```

```text
Mutant m2: the GET confirm page applies the replacement.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

#### L5

```text
The notice is read at most once per committed change, so a reload of the page posts nothing again and claims no second replacement, and no address carries it.
```

Invariant

Application

SavedNotice\, taken out of the session as it is read

##### Treatment

Broad

###### Generator

```text
Every interleaving of commits and page reads, including two page reads after one commit and a page read with no commit.
```

14 steps

fixed seed 0x5ec1e7

###### Must include

```text
two page reads after one commit
```

14

```text
the first violating trace, reported in full
```

```text
A count of notices read against a count of commits.
```

```text
Mutant m3: the page reads the notice without taking it out of the session.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

#### L6

```text
No dead end: while a secrets key is stored, the page always offers a way on - Replace while it opens, Start over while it does not open and no live replica opens it, or the restore that the live replica still makes possible.
```

Invariant

Domain

KeyMode\:\:replaces and UnopenableOffer\:\:decide

##### Treatment

Broad

###### Generator

```text
Every interleaving including the loss of the instance key, the restore of it, replacements, start overs and heartbeats going stale and fresh.
```

2 replicas\, 14 steps

fixed seed 0x5ec1e7

###### Must include

```text
the instance key lost while the other replica still opens the key, then its heartbeat going stale
```

14

```text
the first violating trace, reported in full
```

```text
The three offers read off the state, with no state in which all three are absent.
```

```text
Mutant m6: Start over is never offered under I-26.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

#### L7

```text
Replace and Start over are never offered at the same time, in any order of replacements, start overs, key losses and restores.
```

Invariant

Domain

KeyMode\:\:replaces and UnopenableOffer\:\:decide

##### Treatment

Broad

###### Generator

```text
Every interleaving of the two applies and the key-state changes, in both orders.
```

2 replicas\, 14 steps

fixed seed 0x5ec1e7

###### Must include

```text
a replacement and a start over in the same trace, in both orders
```

14

```text
the first violating trace, reported in full
```

```text
The two offers read off the same state; the model also witnesses that both orders are reachable.
```

```text
Mutant m6 makes the offer set empty; a mutant that offers Replace under I-26 would make it two.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

#### L8

```text
The window closes with no restart: after the replacement bumps the config version, every replica eventually holds the new secrets key and opens every credential, and a delivery waiting through the window eventually goes out.
```

Temporal

Infrastructure

crates\/foundry\-app\/src\/integrations\/reload\.rs and the delivery worker

##### Treatment

Broad

###### Generator

```text
The finite TLA+ slice: two replicas, at most two key generations, one queued delivery; every interleaving of the replacement, the reloads and the delivery attempts.
```

2 replicas\, MaxGen 2

TLC breadth\-first over the complete state space\; no seed

###### Must include

```text
a replacement with the delivery queued before any replica has reloaded
```

the complete state space of the slice

```text
TLC reports the counter-example behaviour in full
```

```text
Two temporal properties: every replica eventually holds the stored generation, and a queued delivery eventually leaves the queue.
```

```text
The same check with no fairness, which admits the stuttering behaviour; and a progress mutant with fairness whose reload adopts the version but keeps the key it holds.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/reseal\_convergence\.receipt\.txt

#### L9

```text
A failed re-seal changes nothing: the old secrets key stays in effect, every saved credential still opens, no activity line is written, the config version is unchanged and the page reads no I-24.
```

Invariant

Adapter

Store\:\:replace\_secrets\_key\, the transaction rolled back on a refusal from the re\-seal closure

##### Treatment

Finite

###### Cases

```text
A re-seal refusal on the first connection credential: nothing stored, nothing recorded, no notice earned, the page answered as it is.
```

```text
A re-seal refusal on the saved Slack app secrets: the same.
```

```text
A compare-and-set loss, the stored key no longer the one the plan was made from: nothing stored, nothing recorded.
```

```text
A closed set of refusal points, each observable once through the public port; a generated domain would add no case.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

#### L10

```text
The count the copy carries is the I-22 counting rule: each stored connection credential one, the saved Slack app one, and the copy form follows the count - the n >= 2 form, the n = 1 form and the n = 0 form, for I-23, I-24 and I-28 alike.
```

Algebraic

Domain

SavedCredentials\, crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs

##### Treatment

Broad

###### Generator

```text
Connection counts 0 to 8 crossed with the saved Slack app present or absent, for each of I-23, I-24 and I-28.
```

0 to 8 connections

the repository proptest default\, as keys\.rs already uses

###### Must include

```text
zero connections with no saved app (n = 0)
```

```text
zero connections with a saved app (n = 1)
```

```text
one connection with no saved app (n = 1)
```

```text
one connection with a saved app (n = 2)
```

not applicable\, the value is one count

```text
proptest shrinks the counts
```

```text
The chosen sentence read against the locked copy table, with the number substituted independently of the selector.
```

```text
A selector that reads the n = 1 form for n = 0, or that counts each Slack app secret separately and so reads 2 where the locked copy reads 1.
```

##### Evidence

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs

#### L11

```text
After a replacement or a start over, rotating the instance key itself with foundry doctor rotate-secrets leaves every saved credential opening: the stored secrets key is re-wrapped under the new instance key and no credential is re-sealed, because every one is already under the secrets key.
```

Refinement

Infrastructure

crates\/foundry\-app\/src\/admin\_cli\.rs rotate\_secrets over Store\:\:rotate\_instance\_key

##### Treatment

Finite

###### Cases

```text
A rotation after a replacement: the key is re-wrapped, resealed is 0, every credential still opens on every replica after its reload.
```

```text
A rotation after a start over with credentials discarded: the same over the credentials that remain.
```

```text
Two finite states to observe through the public CLI and the public page; the rotation itself is shipped and unchanged, so the law is about this value not breaking it.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

### Refuted

```text
A grace window keeping the old secrets key readable until every replica has reloaded is needed for the window to cost nothing.
```

```text
Law scope, not carrier: the window costs nothing already, because the only thing a stale replica does is retry. A second live key would add an authority over what opens and would have to be discarded by a decision nothing can take safely - exactly the dead end Q5 forbids.
```

```text
In the model, a delivery attempt on a replica holding the old key while every value is sealed under the new one: the state is admitted, the delivery retries, no connection is marked lost and nobody is emailed. Mutant m4, which marks it lost, is the only way to break it, and that is a behaviour of the worker, not of the key.
```

```text
The live-replica gate read at the GET is enough, because the admin reads it and then confirms.
```

```text
Boundary, not carrier: the gate is a fact about other processes, which may change between the two requests. Only a read inside the transaction that discards the credentials can hold it.
```

```text
Mutant m5: a trace in which the other replica heartbeat goes stale, the admin opens Start over, the replica beats fresh again and the POST discards credentials a live server still opens.
```

### Claims

#### C1

```text
A start over cannot be applied without the exact typed confirmation START OVER.
```

```text
Credentials discarded by a misread click, costing every workspace a reconnect and the Slack app a re-entry, with no way back.
```

StartOverConfirmation\, crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs

```text
The Confirmation form field as posted, any UTF-8 text or absent.
```

StartOverConfirmation\:\:typed\(field\) \-\> Option\<StartOverConfirmation\>\, the only way to build the value\, which trims surrounding whitespace and admits only START OVER\.

The start\-over POST handler\, which cannot call the apply without a StartOverConfirmation value\.

```text
A wrong or empty Confirmation answers the confirm page with I-28 again and the stored rows are unchanged.
```

```text
rustc enforces that the apply takes the value, so no caller inside this crate can skip it; a future caller could construct it from its own parse if the constructor were made public with a looser input, and the store write itself does not re-check the words - the gate it re-checks inside the transaction is the live replica one, which the model covers.
```

#### C2

```text
No door of this value can write while answering a confirm page.
```

```text
A browser prefetch or a link preview replacing the secrets key or discarding credentials without anyone confirming.
```

ReadOnlyDoor over PageRead\, crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

```text
A GET of either confirm page from any instance admin, including a prefetch.
```

ReadOnlyDoor\:\:read\, which walks only the PageRead list of its variant\; PageRead has no variant that writes\.

The two confirm handlers\, which reach the store only through that call\.

```text
After any number of GETs of either confirm page the stored key, every credential, the activity list and the config version are unchanged.
```

```text
A future handler could call StoreCapabilities directly instead of through the door; the check-arch fenced-writer rule catches a write of a sealed column, not an arbitrary read-path write, so the unit test that lists each door reads stays the guard.
```

#### C3

```text
A replacement either seals every saved value under the new key or changes nothing.
```

```text
A half-replaced instance: some credentials opening and others lost for good, with no key that opens both.
```

Store\:\:replace\_secrets\_key\, crates\/foundry\-store\/src\/integrations\/instance\.rs

```text
The stored rows as they are at the post, and a re-seal closure that may refuse on any slot.
```

One sqlx transaction under pg\_advisory\_xact\_lock\, with the refusal of the closure rolling it back\.

Every replica on its next reload\, and the acting replica through Integrations\:\:reboot\.

```text
After a refused re-seal every credential still opens and no I-24 is read; after a committed one every credential opens under the new key within 60 seconds.
```

```text
A direct database write, a restore of a backup taken mid-transaction, and a process failure between the commit and the acting replica reboot - which costs that replica its own reload interval and nothing else, because the commit is already complete.
```

### States

Admitted

```text
A stored key that opens, with 0, 1 or many saved credentials: Replace offered, I-23 in its matching form.
```

None.

Admitted

```text
I-26 with no live opener: Replace withheld, Start over offered, I-28 in its matching form.
```

None.

Admitted

```text
I-26 with a live opener: restore-only with the added sentence, neither control offered.
```

None.

Admitted

```text
I-31, no instance key read at all: restore-only without the added sentence.
```

None.

Admitted

```text
A window after a commit in which replicas hold different keys: deliveries retry, pages read the connection as not readable, nothing is lost and nobody is emailed.
```

None.

Excluded

```text
One saved credential sealed under the new key and another under the old one: no reachable state of the model has it, because each apply is one locked transaction.
```

None.

Excluded

```text
A start over committed while a replica with a fresh heartbeat opens the stored key: the gate is inside the transaction.
```

None.

Excluded

```text
A start over applied without the typed confirmation: the apply takes a StartOverConfirmation value.
```

None.

Excluded

```text
Replace offered under I-26 or under I-4: KeyMode::replaces answers false in both.
```

None.

Excluded

```text
I-24 or I-21 read twice for one apply, or carried in an address: the notice is taken out of the session as it is read.
```

None.

Unknown

```text
A Foundry server of an earlier release that never beats a replica state row at all: it cannot be seen by the gate, which is exactly why I-28a asks the human to confirm that none is running. The residual risk is the human answer.
```

the instance admin\, with the product owner as the owner of the asked\-for confirmation

Unknown

```text
A replica whose clock is far from the writer clock, so its beat_at looks stale or fresh wrongly to the gate window.
```

the instance operator who deploys the replicas

### Models

Quint

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.qnt

##### Properties

allOrNothing

Invariant

```text
Every saved slot is sealed under the stored secrets key and the stored key check names that key.
```

Store\:\:replace\_secrets\_key\, Store\:\:start\_over\_secrets\_key

crates\/foundry\-store\/src\/integrations\/instance\.rs\:402

noLostNoEmail

Invariant

```text
No connection is marked lost and no admin is emailed because a replica cannot open a credential.
```

DeliveryOutcome ReasonCode\:\:SecretUnavailable settled by RetryPolicy\:\:BACKOFF

crates\/foundry\-app\/src\/integrations\/worker\.rs\:650

gateHeld

Invariant

```text
No start over commits while a replica with a fresh heartbeat opens the stored key.
```

the gate closure of Store\:\:start\_over\_secrets\_key\, read inside the locked transaction

crates\/foundry\-store\/src\/integrations\/instance\.rs\:402

writesOnlyFromPosts

Invariant

```text
Every durable write comes from a POST; a GET, a prefetch and a link preview write nothing.
```

ReadOnlyDoor over PageRead

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:511

noticeHonest

Invariant

```text
The notice is read at most once per committed change.
```

SavedNotice\:\:taken\_from

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:439

noDeadEnd

Invariant

```text
While a key is stored the page always offers Replace, Start over, or the restore a live opener still allows.
```

KeyMode\:\:replaces and UnopenableOffer\:\:decide

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs

offersExclusive

Invariant

```text
Replace and Start over are never offered together, in either order of applies.
```

page\_state

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:636

##### Atoms

Action

postReplaceApplies

Store\:\:replace\_secrets\_key

crates\/foundry\-store\/src\/integrations\/instance\.rs\:402

```text
One sqlx transaction that takes pg_advisory_xact_lock on the instance-integrations key before it reads anything, so the key row, every re-sealed value, the key check, the activity line and the version bump are one atomic step.
```

Action

postStartOverApplies

Store\:\:start\_over\_secrets\_key

crates\/foundry\-store\/src\/integrations\/instance\.rs\:402

```text
The same lock and transaction; the gate is read inside it, after the lock, so no replica state can change between the gate and the commit.
```

Action

getReplace

ReadOnlyDoor\:\:SecretsKeyReplace read

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:511

```text
Not atomic and not required to be: every call it makes is a PageRead, so an interleaving cannot make it write.
```

Action

reload

Reloader\:\:reload

crates\/foundry\-app\/src\/integrations\/reload\.rs\:160

```text
Reads the version, then the snapshot, then installs over the integrations it read; install_over is compare-and-set on the seen value, so a concurrent install is not lost.
```

Action

deliver

the worker channel over a sealed credential

crates\/foundry\-app\/src\/integrations\/worker\.rs\:663

```text
One claimed delivery attempt, fenced by its claim_id and settled once.
```

State

sealedUnder

integration\_connections\.sealed\_credential\, instance\_slack\_app and instance\_slack\_app\_pending sealed columns

crates\/foundry\-store\/src\/integrations\/instance\.rs\:1078

None.

State

storedKey

integration\_secrets\_key

crates\/foundry\-store\/src\/integrations\/instance\.rs\:95

None.

State

beating

integration\_replica\_states\.beat\_at judged against the gate window

crates\/foundry\-store\/src\/integrations\/instance\.rs\:784

None.

State

held

Keyring\:\:secrets\_key\, the Arc\<SecretBox\> a replica holds

crates\/foundry\-app\/src\/integrations\/keys\.rs\:267

None.

State

notice

the session value under SavedNotice\:\:SESSION\_KEY

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:382

None.

##### Assumptions

```text
Two replicas and three sealed slots (two connection credentials and the saved Slack app) reach every seam this value has: one acting server, one other that may or may not still open the key.
```

```text
The two write use-cases are atomic, because each is one transaction under the instance-integrations advisory lock; the model does not re-derive that atomicity, it assumes it and names its mechanism.
```

```text
Key generations are numbers, so the model carries which key a value is sealed under and never any key material.
```

```text
The simulation runs are bounded: 12 to 14 steps, 4000 to 8000 samples, seed 0x5ec1e7. They are counterexample search, not exhaustive verification; the exhaustive run is recorded separately.
```

##### Mutants

###### N1

```text
m1: the replace stores the new key and its check without re-sealing the saved values in the same transaction, so the re-seal is a second step.
```

###### Kills

allOrNothing

###### N2

```text
m2: the GET confirm page applies the replacement, so a browser prefetch replaces the key.
```

###### Kills

writesOnlyFromPosts

###### N3

```text
m3: the page reads the notice without taking it out of the session, so a reload claims a second replacement.
```

###### Kills

noticeHonest

###### N4

```text
m4: a replica that cannot open a credential marks its connection lost and emails the workspace admins.
```

###### Kills

noLostNoEmail

###### N5

```text
m5: the POST start over checks only I-26 and not the live-replica gate again.
```

###### Kills

gateHeld

###### N6

```text
m6: Start over is never offered under I-26, so a lost instance key with no backup is a dead end.
```

###### Kills

noDeadEnd

TlaPlus

docs\/feature\/iis\-secrets\-key\-replace\/design\/reseal\_convergence\.tla

##### Properties

EveryReplicaOpens

Temporal

```text
Every replica eventually holds the stored secrets key generation, with no restart.
```

Reloader\:\:reload and Keyring\:\:resolve\_holding

crates\/foundry\-app\/src\/integrations\/reload\.rs\:160

DeliveryArrives

Temporal

```text
A delivery queued through the window eventually goes out.
```

the delivery worker over RetryPolicy\:\:BACKOFF

crates\/foundry\-app\/src\/integrations\/worker\.rs\:650

##### Atoms

Action

Replace

Store\:\:replace\_secrets\_key

crates\/foundry\-store\/src\/integrations\/instance\.rs\:402

```text
One locked transaction, as model 1 records.
```

Action

Reload

Reloader\:\:reload

crates\/foundry\-app\/src\/integrations\/reload\.rs\:160

```text
One poll of the config version and one install_over.
```

Constant

Replicas

the replica names of integration\_replica\_states

crates\/foundry\-app\/src\/integrations\/reload\.rs\:190

None.

##### Assumptions

```text
A finite slice: two replicas, at most two key generations, one queued delivery. The state the temporal properties need and nothing else.
```

```text
Fairness assumed: each replica reload task runs and each replica delivery attempt runs (WF on Reload(r) and Deliver(r)). That is the obligation of the reload loop of the worker, not of this value.
```

```text
The model proves eventual convergence, not the 60-second figure: the wall clock follows from the reload interval times the bound the model converges within.
```

##### Mutants

###### N7

```text
A progress mutant with the same fairness: the reload adopts the config version but does not re-derive the secrets key it holds, so the window never closes and the queued delivery never goes out.
```

###### Kills

EveryReplicaOpens

DeliveryArrives

### Closure

```text
A replacement, a start over, both orders of them, and a start over leaving nothing saved are each constructible from admitted input
```

Inhabitation

Pass

DesignedNotBuilt

```text
Eight reachability witnesses, each taken as a counterexample to its own negation in the Quint model and recorded with its full trace: a replacement committed, a start over committed, a notice read, I-26 reached by the loss of the instance key, I-26 with a live opener, Start over offered, a replacement and a start over in one trace in both orders, and a start over that leaves nothing saved.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

```text
Each promised observation is reached through a public driving port: the GET and POST of the two doors, the instance integrations page, GET /workspace/integrations and the CLI rotation
```

PublicReachability

Pass

DesignedNotBuilt

```text
Each slice public oracle names its stimulus on the production router behind require_instance_admin with a CSRF token, or on the shipped CLI, and its observation on a public page, the mail double, a chat destination or a process exit. Every production target is a module of that path or a new module it calls; no chain needs a private symbol. The capability is designed and not yet built.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.qnt

```text
The window closes with no restart: every replica eventually opens every credential and a delivery queued through the window eventually goes out (Q2 temporal half, P4)
```

Liveness

Pass

DesignedNotBuilt

```text
TLC over the complete state space of the finite slice, three recorded runs: under the declared fairness (each replica reload task runs and each replica delivery attempt runs) no error; with no fairness both temporal properties are violated by the admitted stuttering behaviour; and a progress mutant whose reload adopts the config version without re-deriving the secrets key violates both under the same fairness. The model proves the eventual convergence, not the 60-second wall clock, which follows from the configured reload interval.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/reseal\_convergence\.receipt\.txt

```text
Replace: the re-seal is all-or-nothing, and no reachable state has one credential under the new key and another under the old one (Q1)
```

Safety

Pass

DesignedNotBuilt

```text
Exhaustive model checking of the conjunction of the seven named invariants over the Quint model with Apalache through quint verify, bound 8 steps, outcome NoError; and bounded simulation of each invariant on its own (12 steps, 4000 samples, seed 0x5ec1e7), with one behavioural mutant per invariant each refuted by a counterexample trace. The bound stops at 8 steps for the exhaustive run; the simulation reaches 14. The claim is about the design, not the runtime: no code is built yet.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

```text
Start over: no apply without the live-replica gate re-read inside the same transaction (Q3)
```

Safety

Pass

DesignedNotBuilt

```text
Exhaustive model checking of the conjunction of the seven named invariants over the Quint model with Apalache through quint verify, bound 8 steps, outcome NoError; and bounded simulation of each invariant on its own (12 steps, 4000 samples, seed 0x5ec1e7), with one behavioural mutant per invariant each refuted by a counterexample trace. The bound stops at 8 steps for the exhaustive run; the simulation reaches 14. The claim is about the design, not the runtime: no code is built yet.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

```text
Both confirm pages change nothing, and one committed change is read once (Q4)
```

Safety

Pass

DesignedNotBuilt

```text
Exhaustive model checking of the conjunction of the seven named invariants over the Quint model with Apalache through quint verify, bound 8 steps, outcome NoError; and bounded simulation of each invariant on its own (12 steps, 4000 samples, seed 0x5ec1e7), with one behavioural mutant per invariant each refuted by a counterexample trace. The bound stops at 8 steps for the exhaustive run; the simulation reaches 14. The claim is about the design, not the runtime: no code is built yet.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

```text
No connection marked lost and no workspace admin emailed because of the window where replicas hold different keys (Q2, safety half)
```

Safety

Pass

DesignedNotBuilt

```text
Exhaustive model checking of the conjunction of the seven named invariants over the Quint model with Apalache through quint verify, bound 8 steps, outcome NoError; and bounded simulation of each invariant on its own (12 steps, 4000 samples, seed 0x5ec1e7), with one behavioural mutant per invariant each refuted by a counterexample trace. The bound stops at 8 steps for the exhaustive run; the simulation reaches 14. The claim is about the design, not the runtime: no code is built yet.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

```text
No dead end, and never both Replace and Start over offered, in either order of applies (Q5, Q6)
```

Safety

Pass

DesignedNotBuilt

```text
Exhaustive model checking of the conjunction of the seven named invariants over the Quint model with Apalache through quint verify, bound 8 steps, outcome NoError; and bounded simulation of each invariant on its own (12 steps, 4000 samples, seed 0x5ec1e7), with one behavioural mutant per invariant each refuted by a counterexample trace. The bound stops at 8 steps for the exhaustive run; the simulation reaches 14. The claim is about the design, not the runtime: no code is built yet.
```

##### Evidence

docs\/feature\/iis\-secrets\-key\-replace\/design\/secrets\_key\_replace\.receipt\.txt

```text
A start over cannot be applied without the typed confirmation START OVER
```

Safety

Blocked

DesignedNotBuilt

```text
Not a model claim: the exclusion is the StartOverConfirmation constructor, which rustc enforces at the call of the apply. The model admits the refused POST as an action and so cannot falsify the type. The oracle observes the refusal through the public port, and the type claim is recorded with its bypasses; no checker covers it, so this axis stays Blocked.
```

##### Evidence

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs

### Risks

#### Y1

Stressor

```text
Negates "no Foundry server of an earlier release is running": a server that never writes a replica state row at all is running and holds the old secrets key.
```

```text
This Foundry own state stays coherent: every credential it keeps is sealed under the new key and opens.
```

```text
That other server keeps opening what it holds in memory until it is stopped, and may post with credentials this Foundry has discarded.
```

```text
Offering Start over as if the gate had seen every server: I-28a asks the human to confirm exactly this, in words.
```

```text
The gate can only see servers that beat; the typed confirmation is the design answer, and it is a human answer, not a check.
```

the instance admin\, with the asked\-for confirmation owned by the product owner

#### Y2

Stressor

```text
Negates "the reload interval is the default": an operator sets FOUNDRY_INTEGRATIONS_RELOAD_MS far above 20000.
```

```text
Convergence still happens with no restart, and no credential is lost.
```

```text
The window is longer than 60 seconds, so deliveries retry for longer and pages read connections as not readable for longer.
```

```text
A promise of 60 seconds that the setting cannot keep: the design carries the figure as the interval times the proved bound, never as a constant in code.
```

```text
The 60-second figure is a property of the setting; an operator who raises it loses it.
```

the instance operator

#### Y3

Stressor

```text
Negates "the clocks of the replicas agree": a replica clock is minutes off, so its beat_at looks stale or fresh wrongly.
```

```text
Every apply stays all-or-nothing and every page stays coherent.
```

```text
The gate may withhold Start over while nothing opens the key (the admin restores or waits), or may admit it while a skewed-clock replica still opens it.
```

```text
Reading the clock of the database or of now() inside the judgement instead of the caller clock, which would hide the skew.
```

```text
The gate window is three reload intervals, so skew beyond that is visible as a wrong offer; I-28a remains the human backstop.
```

the instance operator

#### Y4

Stressor

```text
Negates "one admin at a time": two instance admins post Replace from two confirm pages at once, or one double-submits.
```

```text
The advisory lock serialises them; each transaction is all-or-nothing, so every credential always opens under whichever key won.
```

```text
Two replacements in a row, each with its own activity line and its own notice to its own admin, or the second refused on the compare-and-set.
```

```text
A mixed state, or one notice claiming two replacements.
```

```text
Nothing: the model checks both interleavings and the notice count.
```

the solution architect

#### Y5

Stressor

```text
Negates "the instance key is held": the instance key is lost again right after a start over, before any replica has reloaded.
```

```text
I-26 is reached again with the same way out; no state offers nothing.
```

```text
The credentials saved since the start over become unopenable and are discarded by the next start over.
```

```text
A state in which neither Replace nor Start over nor a restore is offered - the no-dead-end invariant.
```

```text
Repeated loss costs repeated reconnects; that is the cost the copy states before she chooses.
```

the instance operator

#### Y6

SinglePointOfFailure

```text
The single integration_secrets_key row: one key, no grace window (D4 resolved).
```

```text
Because every value is re-sealed in the same transaction, the row alone is enough; no replica needs a key the row does not carry.
```

```text
A replica that has not reloaded holds a key that opens nothing it re-reads, and retries.
```

```text
Keeping the old wrapped key alive as a second authority over what opens, which would make two keys able to disagree.
```

```text
A backup restored from the middle of a replacement is outside the transaction, and is an operator restore, not a Foundry state.
```

the instance operator

#### Y7

SinglePointOfFailure

```text
The instance-integrations advisory lock: every instance write, both new ones included, serialises on it.
```

```text
All-or-nothing and the gate read inside the transaction both rest on it.
```

```text
A long re-seal over many credentials holds the lock and makes another instance write wait.
```

```text
Taking the lock after reading the rows the decision is made over, which would let the state move under the decision.
```

```text
The re-seal is one pass over the stored rows with no vendor call inside the transaction, so the hold is bounded by the database.
```

the solution architect

#### Y8

Bypass

```text
A direct database write, a restore of a backup, or any writer outside Foundry setting integration_secrets_key or a sealed column.
```

```text
Nothing: such a writer can make a mixed state no type prevents.
```

```text
Nothing; it is outside the design.
```

```text
Treating the check-arch fenced-writer rule as covering a writer that is not Rust code of this repository.
```

```text
The key check row is the detector: the boot probe reads a credential it cannot open as I-4 and withholds Replace rather than refusing forever.
```

the instance operator

#### Y9

Bypass

```text
Deserialization: the session value that carries the notice, and the Confirmation form field, both arrive as untrusted text.
```

```text
An unknown session wire name reads as the page itself, and a Confirmation that is not the exact text builds no value.
```

```text
A crafted session value naming a known notice shows a notice the admin did not earn, in her own session only.
```

```text
Parsing the count out of the session value without bounding it, or trusting the field to carry a decision.
```

```text
The notice is cosmetic and scoped to one session; the apply is decided by the stored rows, never by either input.
```

the solution architect

#### Y10

Bypass

```text
A future handler reaching StoreCapabilities directly instead of through a ReadOnlyDoor, so a read path writes.
```

```text
Today: every call of both confirm pages is a PageRead, and the unit test lists them.
```

```text
Nothing today.
```

```text
Relying on check-arch for this: the fenced-writer rule covers writes of sealed columns and the instance tables, not an arbitrary write on a read path.
```

```text
The listing unit test over ReadOnlyDoor::reads is the guard; it is a test, not a type.
```

the solution architect

### Architectural tests

#### T1

```text
Every write of a sealed column and of integration_secrets_key stands inside a listed fenced writer of crates/foundry-store/src/integrations/instance.rs, the two new writes included, and no module outside crates/foundry-app/src/integrations opens or seals a credential, so the new re-seal closures cannot leak the seam. The rule covers every crate src, members added later included.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh cargo xtask check-arch
```

### Contract tests

#### T2

```text
A stored activity action written by the two new store writes renders as its I-18 Deliverable B words, so a persisted secrets_key_replaced or secrets_key_started_over row written by one release reads correctly in another; the generic underscore fallback must not answer for either.
```

instance\_integration\_audit\_events\.action wire names \(ADR\-IIS\-005 r1\)

crates\/foundry\-store\/src\/integrations\/instance\.rs replace\_secrets\_key and start\_over\_secrets\_key

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs activity\_line

```text
After the real store writes carry out a replacement and a start over, GET /admin/instance/integrations Activity reads "{date} - Priya Raman replaced the secrets key (3 credentials sealed again)" and "{date} - Priya Raman started over with a new secrets key (3 credentials discarded)", not "secrets key replaced" or "secrets key started over".
```

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

```text
scripts/test-in-lima.sh bash -c "source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-07"
```

## Slices

### S1

#### Targets

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs

Create

None.

crates\/foundry\-app\/src\/integrations\/keys\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/handlers\/secrets\_key\.rs

Create

None.

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs

Extend

None.

crates\/foundry\-app\/src\/lib\.rs

Extend

None.

crates\/foundry\-app\/templates\/integrations\/instance\.html

Extend

None.

crates\/foundry\-app\/templates\/integrations\/secrets\_key\_replace\.html

Create

None.

crates\/foundry\-store\/src\/integrations\/instance\.rs

Extend

None.

crates\/foundry\-store\/src\/integrations\/mod\.rs

Extend

None.

xtask\/src\/check\_arch\.rs

Extend

None.

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

Extend

None.

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

#### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::secrets_key_rotation
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::handlers
scripts/test-in-lima.sh cargo test -p foundry-store --test iis_instance_store
scripts/test-in-lima.sh bash -c "source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-07"
scripts/test-in-lima.sh cargo xtask check-arch
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/external\_integration\_plugins\.rs

#### Public oracle

```text
This Foundry holds a stored secrets key that opens, 2 saved connection credentials for Canzan Labs (Slack and Matrix), a saved Slack app, and a second replica serving the same database. Priya Raman GETs /admin/instance/integrations, then GETs /admin/instance/integrations/secrets-key/replace twice (the second as a prefetch), then POSTs it with her CSRF token through the production router behind require_instance_admin, and reloads the page she lands on. In a second run the re-seal closure refuses on the first credential. In a third run nothing is saved at all.
```

```text
The page offers Replace secrets key next to I-2. The confirm page reads "Replace the secrets key? Foundry seals its 3 saved credentials again under a new key. Nobody needs to reconnect." and leaves the stored key id, the activity list and the config version unchanged after both GETs. The POST answers a redirect to GET /admin/instance/integrations, which reads "Replaced. 3 saved credentials are sealed under the new key." once; the reload of that same address reads the page with no notice and posts nothing. The Activity reads "{date} - Priya Raman replaced the secrets key (3 credentials sealed again)". Within 60 seconds and with no restart, each replica asked directly opens every credential: GET /workspace/integrations of Canzan Labs reads neither connection as lost or unreadable, the next new issue of Sam reaches its Slack channel and its Matrix room, and the mail double has received nothing. foundry doctor rotate-secrets then exits 0 and every credential still opens. In the second run the response is the confirm page again with nothing changed: the stored key id, every credential, the activity list and the config version are as they were, and no I-24 is read anywhere. In the third run the confirm page reads "Replace the secrets key? This Foundry has no saved credentials yet, so nothing needs to be sealed again." and the apply answers "Replaced. This Foundry has a new secrets key."
```

```text
A prefetch of the confirm page that bumps the config version or replaces the key; a state after the POST in which one credential opens under the new key and another does not; a reload of the landing page that reads I-24 a second time or claims a second replacement; a notice carried in the address; a credential that stops opening on the other replica and never starts again, or a connection marked lost or a workspace admin emailed in the window; a refused re-seal that leaves the new key stored, writes an activity line or bumps the version; an activity line reading "secrets key replaced"; the n = 1 or n >= 2 sentence read for n = 0.
```

##### Promises

P1

P2

P3

P4

P5

P10

P11

#### Prefactoring

```text
L2: move the secrets-key door family out of the 1970-line crates/foundry-app/src/integrations/handlers/instance.rs into a new crates/foundry-app/src/integrations/handlers/secrets_key.rs - KeyOffer, key_door, store_secrets_key, stored_notice and the I-21, I-22 and I-3 copy constants - with no verdict and no word changed, the page still composing them through the same calls.
```

```text
Every verdict of KeyOffer::applies and every notice stored_notice selects, the Generate and Move doors behaviour through the public port, and the I-21 and I-22 words; the same oracle, the same argv and the same runtime identity before and after the move.
```

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

##### Verification

```text
scripts/test-in-lima.sh bash -c "source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-02"
scripts/test-in-lima.sh bash -c "source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-06"
```

#### Records

##### Decisions

None.

##### Constraints

None.

##### Reuse

reseal\_sealed\_values

crates\/foundry\-store\/src\/integrations\/instance\.rs\:1078

Reuse

```text
It already walks every stored sealed value under FOR UPDATE - connections, the saved Slack app, the app held for its confirm - and rolls everything back on a refusal from the closure. The replacement needs exactly that walk, so it calls it rather than writing a second one; that is what makes the re-seal all-or-nothing.
```

store\_secrets\_key

crates\/foundry\-store\/src\/integrations\/instance\.rs\:402

Extend

```text
Generate and Move store a key only where none is stored, so neither can carry a replacement. The module gains replace_secrets_key beside it: the same lock, the same re-seal walk and the same commit ending, with a compare-and-set on the stored key id in place of the absence check.
```

GeneratePlan

crates\/foundry\-app\/src\/integrations\/keys\.rs\:131

Extend

```text
plan_generate already makes exactly what a replacement needs: a fresh secrets key wrapped under the held instance key, with its check value. Only its reseal opens under the instance keys; the replacement must open under the old secrets key, so the type gains reseal_under(opening, slot, sealed). Nothing else about the plan changes.
```

KeyOffer

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:271

Extend

```text
The closed offer type with its label, door and applies already decides Generate and Move. Replace is a third arm of the same decision, judged by KeyMode::replaces; a separate control type beside it would be a second authority over what the page offers.
```

SavedNotice

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:373

Extend

```text
It is the one-shot session notice D16 decided; I-24 waits in it exactly as I-10, I-16 and I-17 do. It gains the two arms and a count, because I-24 carries n, and keeps answering None for a wire name it does not know.
```

stored\_notice

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:573

Reuse

```text
The n = 0 answer of a start over is I-21, which this function already selects for a Generate with nothing re-sealed; S3 calls it rather than repeating the sentence.
```

reboot

crates\/foundry\-app\/src\/integrations\/mod\.rs\:346

Reuse

```text
Generate and Move already rebuild and install this server integrations through the shared boot probe before they answer, which is how the acting replica opens every credential at once; the replacement answers after the same reboot.
```

Activity

crates\/foundry\-store\/src\/integrations\/instance\.rs\:341

Extend

```text
The one closed set of instance activity actions with their wire names (ADR-IIS-005 r1). The two Deliverable B lines are two more arms, not a second audit writer.
```

Feature

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

of

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

secrets\_key

crates\/foundry\-app\/src\/integrations\/handlers\/secrets\_key\.rs\:1

CreateNew

```text
A new file: nothing in the repository owns this responsibility yet, which is why the design creates it rather than extending an owner; its callers and the owners it reuses are recorded on the modules it calls.
```

secrets\_key\_rotation

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:1

CreateNew

```text
A new file: nothing in the repository owns this responsibility yet, which is why the design creates it rather than extending an owner; its callers and the owners it reuses are recorded on the modules it calls.
```

dispatch

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

from\_ref

crates\/foundry\-app\/src\/lib\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

extends

crates\/foundry\-app\/templates\/integrations\/instance\.html\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

secrets\_key\_replace

crates\/foundry\-app\/templates\/integrations\/secrets\_key\_replace\.html\:1

CreateNew

```text
A new file: nothing in the repository owns this responsibility yet, which is why the design creates it rather than extending an owner; its callers and the owners it reuses are recorded on the modules it calls.
```

wire\_name

crates\/foundry\-store\/src\/integrations\/mod\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

exit\_code

xtask\/src\/check\_arch\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

##### Impact

write use\-case and its outcome type

crates\/foundry\-store\/src\/integrations\/instance\.rs\:402

Changed

```text
replace_secrets_key, ReplaceOutcome and two Activity arms are added; every shipped write keeps its signature and its behaviour.
```

crate public interface

crates\/foundry\-store\/src\/integrations\/mod\.rs\:80

Changed

```text
The new outcome type is exported beside SecretsKeyOutcome and RotateOutcome; nothing exported is removed or renamed.
```

port trait the app reaches the store through

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs\:33

Changed

```text
The trait gains replace_secrets_key, so every test double of it is extended too; the shipped methods are untouched.
```

router route table

crates\/foundry\-app\/src\/lib\.rs\:855

Changed

```text
The GET and POST of the replace door are registered beside Generate and Move, behind the same guard, carrying _csrf.
```

module exports

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs\:95

Changed

```text
The two new handlers and their path constants are exported as the shipped doors are.
```

architecture rule data

xtask\/src\/check\_arch\.rs\:14769

Changed

```text
The fenced-writer list gains the two new writes, so the seal fence keeps covering every write of a sealed column.
```

CLI use\-case over the same rows

crates\/foundry\-app\/src\/admin\_cli\.rs\:694

UnchangedCompatible

```text
foundry doctor rotate-secrets re-wraps whatever key is stored and re-seals only values not under the secrets key. A replacement leaves exactly that shape, so the rotation keeps working with no change; P10 is an observation of this, not a change to it.
```

database schema

crates\/foundry\-store\/migrations\/0033\_instance\_integrations\.sql\:1

UnchangedCompatible

```text
Both writes use the 0033 tables as they are; the new activity actions are new text values of a text column. No migration.
```

source file of the changed path

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/handlers\/secrets\_key\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/keys\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/templates\/integrations\/instance\.html\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/templates\/integrations\/secrets\_key\_replace\.html\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

##### Agreement

None.

##### Not applicable

None.

##### Boundaries

None.

##### Observations

None.

##### Equality

None.

##### Constructors

None.

##### Laws

None.

##### Refuted

None.

##### Claims

None.

##### States

None.

##### Models

None.

##### Closure

None.

##### Risks

None.

##### Architectural tests

None.

##### Contract tests

None.

### S2

#### Targets

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/handlers\/secrets\_key\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs

Extend

None.

crates\/foundry\-app\/src\/lib\.rs

Extend

None.

crates\/foundry\-app\/templates\/integrations\/instance\.html

Extend

None.

crates\/foundry\-app\/templates\/integrations\/secrets\_key\_start\_over\.html

Create

None.

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

Extend

None.

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

#### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::secrets_key_rotation
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::handlers
scripts/test-in-lima.sh bash -c "source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-07"
scripts/test-in-lima.sh cargo xtask check-arch
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs

#### Public oracle

```text
The instance key file of this server is new, so it cannot open the stored secrets key (I-26), and 3 saved credentials cannot be opened. Run A: a second replica is still running and its heartbeat says it opens the stored secrets key. Run B: no running replica opens it (the second replica has stopped beating). Run C: as B but under I-31, this server reading no instance key at all. In each run Priya Raman GETs /admin/instance/integrations, then GETs /admin/instance/integrations/secrets-key/start-over, then GETs and POSTs /admin/instance/integrations/secrets-key/replace, all with her CSRF token through the production router.
```

```text
In run A the page reads the shipped I-26 restore words followed by "Another Foundry server still opens the secrets key, so Start over is not offered.", offers no Replace secrets key and no Start over, and the start-over GET answers the instance integrations page having changed nothing. In run B the page reads I-26 with Start over appended and no Replace; the start-over confirm page reads "Start over with a new secrets key? The 3 saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again." with the Confirmation field "Type START OVER to confirm that no Foundry server of an earlier release is still running.", and leaves the stored key, every credential, the activity list and the config version untouched. In run C the page reads I-26 restore-only, without that sentence and without Start over. In every run the replace GET and POST change nothing and answer the instance integrations page.
```

```text
Replace offered or applying under I-26; Start over offered while a live replica still opens the key, or under I-31; the live-replica sentence shown under I-31 or while no replica beats; a start-over GET that writes a row, an activity line or a version bump; the n = 1 or n >= 2 sentence read for the wrong count, or the I-28a words paraphrased; the shipped I-26 restore words altered rather than appended to.
```

##### Promises

P6

P7

#### Prefactoring

None.

#### Records

##### Decisions

None.

##### Constraints

None.

##### Reuse

ReadOnlyDoor

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:511

Extend

```text
The type whose every variant is a PageRead, so a door built on it can reach no write. Both confirm pages are two more variants: the replace confirm reads the snapshot, the start-over confirm reads the snapshot and the replica states. A new read-only door type beside it would be a second way of saying the same thing.
```

PageRead

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:499

Extend

```text
It gains ReplicaStates, the read the live-replica gate needs. Keeping it inside this type is what keeps the no-write property mechanical rather than argued.
```

replica\_states

crates\/foundry\-store\/src\/integrations\/instance\.rs\:784

Reuse

```text
Every row with its opens_secrets_key and beat_at is already read for ADR-IIS-004 r8; the gate filters that list and needs no new query. It is added to the StoreCapabilities trait, which does not have it yet.
```

SECRETS\_KEY\_UNOPENABLE

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:139

Reuse

```text
The shipped I-26 restore words stay exactly as they are; this value appends to them and never rewrites them, which is what Deliverable A scope-out requires.
```

page\_state

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:636

Extend

```text
The one pure function from the key mode to what the page shows and offers. The three I-26 states are decided there, from the mode, the gate and the key file, so no template decides anything.
```

Feature

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

of

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

secrets\_key

crates\/foundry\-app\/src\/integrations\/handlers\/secrets\_key\.rs\:1

CreateNew

```text
A new file: nothing in the repository owns this responsibility yet, which is why the design creates it rather than extending an owner; its callers and the owners it reuses are recorded on the modules it calls.
```

secrets\_key\_rotation

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:1

CreateNew

```text
A new file: nothing in the repository owns this responsibility yet, which is why the design creates it rather than extending an owner; its callers and the owners it reuses are recorded on the modules it calls.
```

dispatch

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

from\_ref

crates\/foundry\-app\/src\/lib\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

extends

crates\/foundry\-app\/templates\/integrations\/instance\.html\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

secrets\_key\_start\_over

crates\/foundry\-app\/templates\/integrations\/secrets\_key\_start\_over\.html\:1

CreateNew

```text
A new file: nothing in the repository owns this responsibility yet, which is why the design creates it rather than extending an owner; its callers and the owners it reuses are recorded on the modules it calls.
```

##### Impact

page state value

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:581

Changed

```text
PageState gains the I-26 offer: the appended Start over link, or the live-replica sentence, or neither under I-31. Its shipped fields keep their meaning, so every existing page assertion still reads.
```

port trait the app reaches the store through

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs\:33

Changed

```text
The trait gains replica_states; every test double of it is extended.
```

router route table

crates\/foundry\-app\/src\/lib\.rs\:855

Changed

```text
The GET of the start-over door is registered behind the same guard; its POST lands with S3.
```

page template

crates\/foundry\-app\/templates\/integrations\/instance\.html\:1

Changed

```text
The secrets-key block renders the appended sentence and the Start over link from PageState; it decides nothing itself.
```

source file of the changed path

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/handlers\/secrets\_key\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/templates\/integrations\/secrets\_key\_start\_over\.html\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

##### Agreement

None.

##### Not applicable

prefactoring

```text
The one preserving move this design needs is the S1 move of the secrets-key door family out of handlers/instance.rs; S2 adds its doors to the module that move created and needs no further move of its own.
```

##### Boundaries

None.

##### Observations

None.

##### Equality

None.

##### Constructors

None.

##### Laws

None.

##### Refuted

None.

##### Claims

None.

##### States

None.

##### Models

None.

##### Closure

None.

##### Risks

None.

##### Architectural tests

None.

##### Contract tests

None.

### S3

#### Targets

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/handlers\/secrets\_key\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs

Extend

None.

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs

Extend

None.

crates\/foundry\-app\/src\/lib\.rs

Extend

None.

crates\/foundry\-store\/src\/integrations\/instance\.rs

Extend

None.

crates\/foundry\-store\/src\/integrations\/mod\.rs

Extend

None.

xtask\/src\/check\_arch\.rs

Extend

None.

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

Extend

None.

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

#### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::secrets_key_rotation
scripts/test-in-lima.sh cargo test -p foundry-store --test iis_instance_store
scripts/test-in-lima.sh bash -c "source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-07"
scripts/test-in-lima.sh cargo xtask check-arch
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/external\_integration\_plugins\.rs

#### Public oracle

```text
As S2 run B: this Foundry cannot open its stored secrets key, 3 saved credentials of Canzan Labs and the Slack app cannot be opened, and no running replica opens the key. Priya Raman POSTs /admin/instance/integrations/secrets-key/start-over with her CSRF token and the Confirmation START OVER. Three more runs: the Confirmation left empty; the Confirmation typed as "start over"; and a POST with START OVER while the second replica has beaten fresh again saying it opens the key.
```

```text
The first POST answers a redirect to GET /admin/instance/integrations, which reads "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it." once; a reload of that address reads the page with no notice. The page reads the stored secrets key as set, Matrix as ready, asks for the Slack app again (I-5), and the Activity reads "{date} - Priya Raman started over with a new secrets key (3 credentials discarded)". GET /workspace/integrations of Canzan Labs reads no discarded connection as connected, offers to connect Matrix and tells its workspace admin to reconnect Slack, in the shipped W-2 and K-2 words. foundry doctor rotate-secrets then exits 0 and the new key still opens. Each of the other three runs answers the start-over confirm page with I-28 again and changes nothing at all: the stored key, the remaining credentials, the activity list and the config version are as they were, and no I-21 is read.
```

```text
A start over that commits while a replica beating fresh still opens the key, or that checks the gate only at the GET; a wrong or empty Confirmation that applies anything; a credential this Foundry can open being discarded, or one it cannot open surviving; an activity line reading "secrets key started over" or carrying the wrong count; a workspace page that still reads a discarded Slack connection as connected; I-21 read twice, or carried in the address; a state after the apply in which the stored key check names a different key than the stored key.
```

##### Promises

P8

P9

P5

P10

P12

#### Prefactoring

None.

#### Records

##### Decisions

None.

##### Constraints

None.

##### Reuse

store\_secrets\_key

crates\/foundry\-store\/src\/integrations\/instance\.rs\:402

Extend

```text
start_over_secrets_key is its third sibling: the same lock and the same commit ending, with the gate read inside the transaction, a compare-and-set on the stored key id, and the credentials this Foundry cannot open deleted rather than re-sealed.
```

plan\_generate

crates\/foundry\-app\/src\/integrations\/keys\.rs\:154

Reuse

```text
A start over needs exactly a fresh secrets key wrapped under the held instance key with its check value, and nothing re-sealed; that is plan_generate unchanged.
```

lost\_words

crates\/foundry\-app\/src\/integrations\/handlers\/workspace\.rs\:1

Reuse

```text
What a workspace admin reads about a connection that must be set up again is the shipped W-2 and K-2 wording, which the scope keeps unchanged; the discarded credential leaves the connection in exactly the state those words already describe, so the workspace page needs no new sentence.
```

Activity

crates\/foundry\-store\/src\/integrations\/instance\.rs\:341

Reuse

```text
The started-over arm S1 added carries this apply activity line; S3 writes it and adds nothing to the type.
```

Feature

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

of

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

secrets\_key

crates\/foundry\-app\/src\/integrations\/handlers\/secrets\_key\.rs\:1

CreateNew

```text
A new file: nothing in the repository owns this responsibility yet, which is why the design creates it rather than extending an owner; its callers and the owners it reuses are recorded on the modules it calls.
```

secrets\_key\_rotation

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:1

CreateNew

```text
A new file: nothing in the repository owns this responsibility yet, which is why the design creates it rather than extending an owner; its callers and the owners it reuses are recorded on the modules it calls.
```

dispatch

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

from\_ref

crates\/foundry\-app\/src\/lib\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

wire\_name

crates\/foundry\-store\/src\/integrations\/mod\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

exit\_code

xtask\/src\/check\_arch\.rs\:1

Extend

```text
This file already owns its part of the shipped path, so the design extends it in place rather than adding a parallel owner; what it gains is named in the slice targets and in the shared decisions.
```

##### Impact

write use\-case and its outcome type

crates\/foundry\-store\/src\/integrations\/instance\.rs\:402

Changed

```text
start_over_secrets_key and StartOverOutcome are added; the gate closure is read inside the locked transaction, so a replica that beats while the post is in flight refuses the apply.
```

stored connection credential rows

crates\/foundry\-store\/src\/integrations\/connections\.rs\:1

Changed

```text
The sealed_credential of each connection this Foundry cannot open is cleared in the same transaction, which is what leaves its workspace told to reconnect; the connection row, its routes and its history are kept.
```

saved Slack app row

crates\/foundry\-store\/src\/integrations\/instance\.rs\:1105

Changed

```text
The saved app this Foundry cannot open is deleted with the rest, which is what makes the page ask for the Slack app again (I-5).
```

router route table

crates\/foundry\-app\/src\/lib\.rs\:855

Changed

```text
The POST of the start-over door joins its GET on the same route, behind the same guard, carrying _csrf and the Confirmation field.
```

source file of the changed path

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/handlers\/secrets\_key\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

crates\/foundry\-store\/src\/integrations\/mod\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

source file of the changed path

xtask\/src\/check\_arch\.rs\:1

Changed

```text
This file changes in this slice; every shipped item in it keeps its meaning, and the slice targets name what is added.
```

##### Agreement

None.

##### Not applicable

prefactoring

```text
S3 extends the store module and the door module its two predecessors shaped; no preserving move is needed before its behaviour, and the S1 move is already recorded with its own recipe.
```

##### Boundaries

None.

##### Observations

None.

##### Equality

None.

##### Constructors

None.

##### Laws

None.

##### Refuted

None.

##### Claims

None.

##### States

None.

##### Models

None.

##### Closure

None.

##### Risks

None.

##### Architectural tests

None.

##### Contract tests

None.

## ADRs

- IIS\-SKR\-001 Replace or start over the secrets key — Accepted
