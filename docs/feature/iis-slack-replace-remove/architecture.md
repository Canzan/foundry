# Architecture brief: iis\-slack\-replace\-remove

## Paradigm

object_oriented

## Shared records

### Decisions

#### D1

```text
Claim-to-checker mapping, each claim checked by the checker that decides it. Quint with the Apalache backend decides the pending-app lifecycle, whose safety is quantified over orderings of separate atomic transactions (submit, confirm, remove, the retention sweep, a secrets-key Move) and which exhaustive finite tables over single-transition pure functions cannot decide: the model is pending_app_provenance.qnt, checked exhaustively to length 12 with no error, with four behavioural mutants each refuted by the property it targets and two reachability witnesses violated as intended. TLA+/TLC decides the two temporal claims that need fairness: pending_sweep.tla for the eventual sweep of an expired pending row and replica_convergence.tla for a committed change reaching every replica, each run three times - with fairness, without fairness and with a progress mutant under fairness - where only the fairness run is free of a counterexample. Agda is not selected: no obligation here exceeds a decidable finite relation. Every local, pure, finite claim - the field shapes, the submission kind, the app in effect, the I-14 and I-15 count forms, the offer set, the side-effect-free GET - is decided by the repository own checker, rustc, over closed enums plus exhaustive tables. Every checker runs in a pinned container inside the shared local-dev Lima VM, with nothing installed on the Mac.
```

#### D2

```text
The submitted app awaiting confirmation lives in its own single-row table instance_slack_app_pending (client_id, sealed_client_secret, sealed_signing_secret, requested_by, expires_at), added by its own migration 0034, never in the session table (ADR-IIS-003 r4).
```

#### D3

```text
The current app a submission is compared against is the app in effect: the environment's when its trio is set, else the saved app whose secrets open. So the I-14 gate and I-14's old client ID read the same client ID the page names in I-6 or I-7.
```

#### D4

```text
A blank secret field keeps the saved value by re-sealing the secret the app layer already holds opened, so SlackAppSave keeps both sealed columns required and all sealing stays in slack_app.rs. Where the saved app does not open (I-13) a blank field has nothing to keep and answers I-11 for that field.
```

#### D5

```text
The pending row's two secrets join the three shipped sealed-value sets - the seal fence, reseal_sealed_values and the boot probe - as ADR-IIS-003 r4 requires, but they are not credentials in I-22's count, which stays connections plus the saved app (ADR-IIS-001).
```

#### D6

```text
Carriers and owners. The submission and its verdict are SlackSubmission, SlackChange and the shipped Refusal, owned by crates/foundry-app/src/integrations/slack_app.rs; the page's Replace and Remove controls are a closed SlackOffer set shaped like the shipped KeyOffer, owned by crates/foundry-app/src/integrations/handlers/instance.rs; effects are the fenced writes save_slack_app, hold_pending_slack_app, apply_pending_slack_app and remove_slack_app, owned by crates/foundry-store/src/integrations/instance.rs; errors are SlackSaveOutcome extended with the pending and remove outcomes plus StoreError, owned by that same module; per-process state stays the shipped Integrations held by IntegrationsHandle in crates/foundry-app/src/integrations/mod.rs, rebuilt by the shipped reload.
```

#### D7

```text
With the environment's app in effect the page offers neither Replace nor Remove, and both the GET and the POST of the remove door change nothing and answer the page (ADR-IIS-003 r5). The shipped save form stays where the environment's app is in effect (US-IIS-06 AC 4, I-17), and a POST through it with a client ID different from the environment's while workspaces are connected answers I-14 first (US-IIS-06 AC 6).
```

#### D8

```text
Remove deletes the instance_slack_app row, records slack_app_removed and bumps the config version in one locked transaction, and touches nothing else: no connection row, no lost_reason, no notice, no delivery. Afterwards workspaces read W-2 through the shipped NoConnectionCause, POST /integrations/slack/events answers 401 through the shipped verification, and pending deliveries keep failing transiently under the shipped 24-hour give-up.
```

#### D9

```text
activity_line gains its slack_app_replaced and slack_app_removed arms, so I-18 reads 'replaced the Slack app (client_id)' and 'removed the Slack app (client_id)' instead of the generic underscore fallback. The stored action names and their CHECK are already in migration 0033, so the activity table needs no migration.
```

#### D10

```text
The connected-workspace count is one read in crates/foundry-store/src/integrations/instance.rs over integration_connections with kind slack and a state other than disconnected, counting distinct workspace_id. The I-14 gate reads it inside the write's own locked transaction; the I-14 and I-15 pages read it per request, never from a rebuilt Integrations, because a workspace connection does not bump the config version.
```

#### D11

```text
POST of the confirm door applies the pending row only when its requester is the posting instance admin and its expiry is after the injected clock's now, and deletes it in that same transaction. A second confirm, another instance admin's confirm and an expired confirm therefore all find nothing and answer the page. A new submission overwrites the row.
```

#### D12

```text
GET of the confirm door and GET of the remove door read and render only: a prefetch of either changes no row, writes no activity line and bumps no version.
```

#### D13

```text
ADR-IIS-001, ADR-IIS-003, ADR-IIS-004 and ADR-IIS-005 carry this design unchanged, including their Proposed status; this Request adds no ADR and changes no ADR body.
```

#### D14

```text
What the replica model carries and what it does not: replica_convergence.tla proves that every committed version eventually reaches every replica under weak fairness on each replica's reload task. The 60-second figure of the promise is that eventual convergence under the configured poll interval, FOUNDRY_INTEGRATIONS_RELOAD_MS with its 5000 ms default; it is a consequence of that configuration, not a theorem of the model, and the two-replica acceptance scenario is what observes it.
```

#### D15

```text
I-14's count is a reading taken when the page is rendered, not a gate on the confirm: a workspace connecting or disconnecting between the submission and the confirm changes the number Priya read, never whether her confirm applies.
```

### Constraints

#### K1

```text
Field bounds are ADR-IIS-003 r3's, unchanged: a client ID is digits, a dot and digits; each secret is exactly 32 lowercase hex characters; leading and trailing whitespace is trimmed. No authority bounds a field's total length beyond that shape and this design adds none.
```

#### K2

```text
The pending app expires exactly 10 minutes after it is held, judged by the injected Clock.
```

#### K3

```text
I-15 has three locked forms by the connected-workspace count: the n = 0 form, the n = 1 form (Remove the Slack app from this Foundry? 1 workspace's Slack routes stop posting until it is set up again, and no workspace can connect Slack.) and the counted n >= 2 form. I-14 likewise has separate n = 1 and n >= 2 forms.
```

#### K4

```text
No authority bounds the connected-workspace count; the count is whatever the store holds and the counted forms carry it verbatim.
```

#### K5

```text
Copy I-11, I-14, I-15, I-16, I-17 and I-18 is locked verbatim. W-2, W-3, K-2 and K-19, the Slack app manifest and every workspace connect and disconnect flow are unchanged.
```

#### K6

```text
A committed change is in effect on every replica within 60 seconds without a restart, through the shipped version poll and reload whose default interval is 5000 ms (ADR-IIS-004 r3 and r4).
```

#### K7

```text
Removal leaves the shipped 24-hour delivery give-up untouched: pending deliveries meet no app and keep failing transiently.
```

#### K8

```text
Every write in this design takes the one instance-integrations advisory lock exclusively, fences on the sealing key id and bumps the config version in its own transaction; the pending hold takes that lock shared and fences the same way (ADR-IIS-004 r1, r2, r3).
```

#### K9

```text
The design runs on a standalone deploy: no setting, host, cluster or DNS name beyond the documented FOUNDRY_ settings, and no dependency on any particular environment.
```

#### K10

```text
Default test budgets apply and no feature derogation is requested; the acceptance lane runs in the isolated Lima VM through scripts/test-in-lima.sh.
```

#### K11

```text
No secret, key or key id reaches the confirm page, the remove page, the activity table or a log line.
```

#### K12

```text
Out of scope and unchanged by this design: US-IIS-07's Replace secrets key and Start over, and Deliverable A carried-forward items 32 and 35.
```

### Reuse

SlackAppForm

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:113

Extend

```text
It already reads and trims the three fields; the replace path needs the same reading plus which app the submission names, so the submission kind is added to this owner instead of a second form type.
```

check

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:156

Reuse

```text
The shapes and the trimming are unchanged by Replace and Remove, and its table and property tests already decide the whole field relation.
```

plan\_save

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:198

Extend

```text
It already seals both secrets into their slots and sets the fence; keeping a saved secret and holding a pending app are the same sealing with a different source and a different target row.
```

app\_in\_effect

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:59

Reuse

```text
The current app the gate compares against and the app the offers follow are exactly this function's answer, so no second rule is written.
```

page\_state

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:255

Extend

```text
It already decides every line and offer of the page from one PageFacts; the Replace and Remove controls are two more offers of the same pure decision.
```

KeyOffer

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:190

Reuse

```text
Its shape - a closed offer carrying its label, its door and when it applies - is the shape SlackOffer follows, so the page keeps one way of offering a control.
```

activity\_line

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:352

Extend

```text
It is the one place a stored action becomes I-18 words; the replaced and removed arms belong there rather than in a second renderer.
```

save\_slack\_app

crates\/foundry\-store\/src\/integrations\/instance\.rs\:406

Extend

```text
It already holds the lock, the fence, the upsert, the activity line and the version bump; a replace is that same write with the saved-secret source decided upstream.
```

reseal\_sealed\_values

crates\/foundry\-store\/src\/integrations\/instance\.rs\:715

Extend

```text
It is the one definition of the sealed-value set, so the pending secrets join it there rather than being re-sealed by a second path.
```

commit\_recorded

crates\/foundry\-store\/src\/integrations\/instance\.rs\:775

Reuse

```text
Every write in this design ends the same way - one activity line, one version bump, one commit - so none of them writes that ending again.
```

lock\_exclusive

crates\/foundry\-store\/src\/integrations\/instance\.rs\:272

Reuse

```text
The one instance-integrations lock is already taken here; the new writes take the same one.
```

stored\_key\_id

crates\/foundry\-store\/src\/integrations\/instance\.rs\:409

Reuse

```text
The fence comparison is already defined; the pending hold and the confirm use it unchanged.
```

Activity

crates\/foundry\-store\/src\/integrations\/instance\.rs\:232

Extend

```text
Its SlackAppReplaced variant already exists and SlackAppRemoved is the only missing action of ADR-IIS-005's closed list, so the enum is extended rather than replaced.
```

InstanceSnapshot

crates\/foundry\-store\/src\/integrations\/instance\.rs\:199

Extend

```text
The boot probe and the reload read the sealed-value set from this one snapshot, so the pending row is added to it rather than read by a second path.
```

sealed\_slot

crates\/foundry\-app\/src\/integrations\/mod\.rs\:129

Extend

```text
It is the one map from a stored slot to its sealing slot; the two pending slots are added there.
```

StoreCapabilities

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs\:203

Extend

```text
It is the only writer of the instance tables from the app crate, so the hold, the confirm, the remove, the count and the prune are methods on it.
```

words

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:202

Reuse

```text
W-2 after a removal is exactly its NotSetUp cause, delivered by Request 1; the removal writes no new words.
```

require\_instance\_admin

crates\/foundry\-app\/src\/instance\_admin\.rs\:1

Reuse

```text
Every new door is behind the same guard with the same uniform 404, so no second authorization rule exists.
```

ensure\_csrf\_cookie

crates\/foundry\-app\/src\/csrf\.rs\:1

Reuse

```text
Each new POST carries the same CSRF token as the shipped ones.
```

Retention

crates\/foundry\-app\/src\/integrations\/worker\.rs\:290

Extend

```text
The hourly sweep already prunes replica states and deliveries; deleting expired pending rows is one more prune on that tick rather than a new task.
```

SEAL\_FENCED\_WRITERS

xtask\/src\/check\_arch\.rs\:15043

Extend

```text
The list is the definition of a fenced writer, so the pending hold is added to it; a new rule would compete with it.
```

INSTANCE\_TABLES

xtask\/src\/check\_arch\.rs\:14764

Reuse

```text
It already names instance_slack_app_pending, so the one-module rule covers the new table unchanged.
```

page\.saved\_slack

crates\/foundry\-app\/templates\/integrations\/instance\.html\:45

Extend

```text
The saved-app section is where Replace and Remove belong; a second Slack section would compete with it.
```

0034\_instance\_slack\_app\_pending\.sql

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

CreateNew

```text
No shipped table can hold the pending app: instance_slack_app is the app in effect and the session table sits outside the fence and the re-seal set (ADR-IIS-003 r4). The evolution doc requires its own migration.
```

slack\_confirm\.html

crates\/foundry\-app\/templates\/integrations\/slack\_confirm\.html\:1

CreateNew

```text
No shipped template renders a one-question confirm, and I-14 must render with no secret and no form field, so it cannot reuse the save form's template.
```

slack\_remove\.html

crates\/foundry\-app\/templates\/integrations\/slack\_remove\.html\:1

CreateNew

```text
Same reason for I-15: a counted question with one control, which no shipped template offers.
```

connected\_slack\_workspaces

crates\/foundry\-store\/src\/integrations\/instance\.rs\:452

CreateNew

```text
No shipped read answers how many workspaces hold a live Slack connection: integration_census groups every kind and state for gauges, and the disconnect preview is scoped to one workspace.
```

### Impact

module\, the decision owner

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:113

Changed

```text
The submission kind, the replace-aware verdict and the keep-a-saved-secret plan live here.
```

public interface of the store capability

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs\:203

Migrated

```text
Five methods are added and every implementation follows; no shipped method's signature changes.
```

handler and its template

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:502

Changed

```text
The gate, the two new doors, the offers, I-14, I-15, I-16 and the two I-18 arms.
```

router\, the public door list

crates\/foundry\-app\/src\/lib\.rs\:858

Changed

```text
Four routes are added: GET and POST of the confirm door and of the remove door.
```

store module\, the only writer of the instance tables

crates\/foundry\-store\/src\/integrations\/instance\.rs\:406

Changed

```text
The pending hold, the confirm, the remove, the count, the prune and the pending row in the re-seal set.
```

public types of the store module

crates\/foundry\-store\/src\/integrations\/instance\.rs\:164

Migrated

```text
PendingSlackAppSave, PendingApplied and RemoveOutcome are added and SlackSaveOutcome gains no variant that a shipped caller must handle differently.
```

migration set

crates\/foundry\-store\/migrations\/0033\_instance\_integrations\.sql\:9

UnchangedCompatible

```text
0033 already admits the two activity actions and reserves the pending table for its own migration, so it is not edited.
```

worker tick

crates\/foundry\-app\/src\/integrations\/worker\.rs\:290

Changed

```text
The hourly sweep also deletes expired pending rows.
```

architecture guard

xtask\/src\/check\_arch\.rs\:15043

Changed

```text
The pending hold joins the fenced writers; the instance-tables list already names the table.
```

acceptance support\, the oracle\'s driver

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs\:838

Changed

```text
It gains the replace, confirm and remove requests and the connected-workspace setup the new scenarios drive.
```

acceptance steps

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_integration\_setup\.rs\:179

Changed

```text
It gains the step definitions those scenarios compose.
```

store integration test

crates\/foundry\-store\/tests\/iis\_instance\_store\.rs\:377

Changed

```text
The lock, fence, activity and version obligations of the new writes are asserted beside the shipped ones.
```

app reload test

crates\/foundry\-app\/tests\/iis\_reload\.rs\:58

UnchangedCompatible

```text
The snapshot gains a field it does not read; the shipped reload assertions still hold.
```

build and package metadata

crates\/foundry\-app\/Cargo\.toml\:1

UnchangedCompatible

```text
No dependency, feature or binary changes.
```

operator documentation

RELEASING\.md\:419

UnchangedCompatible

```text
The runbook already sends an instance admin to the integrations page; Replace and Remove add no operator step.
```

### Agreement

instance\_slack\_app\_pending row shape\: client\_id\, both sealed secrets\, requested\_by and expires\_at \(ADR\-IIS\-003 r4\)

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

Producer

Migrated

```text
The migration defines the columns, the single-row CHECK and the NOT NULLs the store relies on.
```

instance\_slack\_app\_pending row shape\: client\_id\, both sealed secrets\, requested\_by and expires\_at \(ADR\-IIS\-003 r4\)

crates\/foundry\-store\/src\/integrations\/instance\.rs\:406

Consumer

Migrated

```text
The only module that reads or writes the table, in the same commit as the migration.
```

instance\_integration\_audit\_events\.action wire names \(ADR\-IIS\-005 r1\)

crates\/foundry\-store\/src\/integrations\/instance\.rs\:232

Producer

Migrated

```text
It writes slack_app_replaced and adds slack_app_removed, both already admitted by the 0033 CHECK.
```

instance\_integration\_audit\_events\.action wire names \(ADR\-IIS\-005 r1\)

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:352

Consumer

Migrated

```text
It renders both actions as I-18 words them instead of falling back to the generic form; rows written by another release are read by this arm.
```

SealedSlot\, the set of sealed values the fence and the re\-seal agree on \(ADR\-IIS\-004 r2\)

crates\/foundry\-store\/src\/integrations\/instance\.rs\:136

Producer

Migrated

```text
Two variants are added for the pending secrets.
```

SealedSlot\, the set of sealed values the fence and the re\-seal agree on \(ADR\-IIS\-004 r2\)

crates\/foundry\-app\/src\/integrations\/mod\.rs\:129

Consumer

Migrated

```text
It maps each slot to its sealing slot and gains the two arms; rustc refuses a missing one.
```

SealedSlot\, the set of sealed values the fence and the re\-seal agree on \(ADR\-IIS\-004 r2\)

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:478

Consumer

UnchangedCompatible

```text
Generate and Move pass their re-seal closure through unchanged; the new slots reach it through the same map.
```

InstanceSnapshot\, what a reload and the boot probe rebuild from \(ADR\-IIS\-004 r4\)

crates\/foundry\-store\/src\/integrations\/instance\.rs\:199

Producer

Migrated

```text
It gains the pending row so the boot probe sees its sealed bytes.
```

InstanceSnapshot\, what a reload and the boot probe rebuild from \(ADR\-IIS\-004 r4\)

crates\/foundry\-app\/src\/integrations\/reload\.rs\:1

Consumer

UnchangedCompatible

```text
The reload rebuilds from the snapshot and ignores the new field.
```

InstanceSnapshot\, what a reload and the boot probe rebuild from \(ADR\-IIS\-004 r4\)

crates\/foundry\-app\/src\/integrations\/mod\.rs\:583

Consumer

Migrated

```text
The boot probe counts the pending secrets among the values a held key must open, and direct_seals keeps counting credentials only.
```

The instance store capability trait\, the only writer of the instance tables \(DDR\-26\)

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs\:203

Producer

Migrated

```text
Five methods are added to the trait.
```

The instance store capability trait\, the only writer of the instance tables \(DDR\-26\)

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:502

Consumer

Migrated

```text
The new doors call only those methods.
```

The instance store capability trait\, the only writer of the instance tables \(DDR\-26\)

crates\/foundry\-services\/src\/integrations\/store\_impls\.rs\:2

Consumer

UnchangedCompatible

```text
The services implementations hold no instance-write capability, so nothing there changes.
```

The acceptance harness contract\: the production router in process\, real Postgres and the Slack double driven through HTTP

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs\:838

Consumer

Migrated

```text
The driver gains the new requests; the harness itself is unchanged.
```

### Not applicable

None.

### Boundaries

```text
HTTP on the production axum router in crates/foundry-app/src/lib.rs: GET and POST /admin/instance/integrations/slack/confirm, GET and POST /admin/instance/integrations/slack/remove, POST /admin/instance/integrations/slack and GET /admin/instance/integrations, each behind require_instance_admin with the uniform 404 for everyone else and CSRF on every POST.
```

#### Driven ports

```text
The instance integrations store capability over Postgres (crates/foundry-app/src/integrations/store_capabilities.rs), the only writer of the instance tables.
```

```text
The injected clock, Arc<dyn Clock>, which decides the pending row's expiry.
```

```text
The credential box that seals and opens the Slack secrets (foundry_auth::secret_box).
```

```text
The shipped Slack adapter and the mail notifier, neither of which a removal calls.
```

```text
The crates/foundry-app handlers depend inward on the pure slack_app decision module and outward only through the store capability trait and the clock. crates/foundry-store names no handler, and SQL on the instance tables never leaves crates/foundry-store/src/integrations/instance.rs.
```

#### Failures

##### B1

```text
The condition that the store cannot be read while a confirm or remove page is being rendered
```

Indeterminate

```text
The door answers the instance page with the reason logged and no count rendered; an unread count is never rendered as zero, so I-15's n = 0 form cannot stand in for it.
```

##### B2

```text
A submission with a different client ID and a blank client secret or signing secret
```

Refusal

```text
I-11 naming that field, the typed client ID kept in the form and both secret fields empty; no row, activity line or version changes.
```

##### B3

```text
A confirm posted by another instance admin, posted twice, or posted after the pending app expired
```

Refusal

```text
The instance page as it is, with the saved app, the activity list and the config version unchanged.
```

##### B4

```text
A remove posted while the environment's app is in effect
```

Refusal

```text
The instance page as it is, offering neither Replace nor Remove, with the stored row, the activity list and the version unchanged.
```

##### B5

```text
A write whose sealing key is no longer the sealing key when its transaction checks the fence
```

Retry

```text
The handler reloads its integrations and tries once more; a second turn-back answers the page with nothing saved.
```

### Observations

```text
The rendered instance integrations page: its notice line, its Slack lines, whether a Replace or a Remove control is offered, and its Activity list.
```

HTTP status and HTML response body

```text
Which sealed bytes a row holds, which key sealed them, and in which order two serialized writes committed.
```

```text
The confirm page at GET /admin/instance/integrations/slack/confirm: the I-14 line with its count and the current client ID, and the Save the new app control.
```

HTTP status and HTML response body

```text
The pending row's secrets, its requester and its exact expiry instant.
```

```text
The remove page at GET /admin/instance/integrations/slack/remove: the I-15 line with its count.
```

HTTP status and HTML response body

```text
Whether the saved app's secrets open, and which workspaces the count stands for.
```

```text
The app in effect, read through POST /integrations/slack/events signed with a given signing secret and through Add to Slack's client ID on a workspace integrations page.
```

HTTP status and the redirect target\'s client\_id parameter

```text
Which replica answered and how recently it reloaded.
```

```text
A workspace's Slack connection line on GET /workspace/integrations and the mail double's recorded emails, after a removal.
```

HTML response body and the recorded email list

```text
A delivery's internal retry timing inside the 24-hour give-up.
```

```text
The instance activity list on GET /admin/instance/integrations.
```

an ordered list of rendered lines

```text
The actor's identity beyond the display name, and two lines committed in the same second.
```

### Equality

SlackSubmission

```text
Two submissions are equal when their trimmed client IDs are equal and each secret field is equally blank or equally the same text; the form's surrounding whitespace is not observable.
```

the app in effect

```text
Two servers hold the same app in effect when the same source wins and the winning app's client ID and both opened secrets are equal; the sealed bytes and the sealing key id are not observable through it.
```

the pending app

```text
Two pending states are equal when both are absent, or both hold the same client ID, the same requester and the same expiry instant; the sealed bytes are not observable.
```

the instance page

```text
Two pages are equal when their notice, their Slack lines, their offered controls and their activity lines are equal; the template's whitespace is not observable.
```

### Constructors

SlackSubmission\:\:from\_fields

fn from\_fields\(fields\: \&\[\(String\, String\)\]\) \-\> SlackSubmission

SlackSubmission\:\:judge

fn judge\(\&self\, current\: Option\<\&str\>\, saved\_secrets\: Option\<\&SavedSecrets\>\) \-\> Result\<SlackChange\, Refusal\>

plan\_replace

fn plan\_replace\(sealing\: \&SecretBox\, mode\: KeyMode\, change\: \&SlackChange\, by\: Uuid\, at\: OffsetDateTime\) \-\> Option\<SlackAppSave\>

plan\_hold\_pending

fn plan\_hold\_pending\(sealing\: \&SecretBox\, mode\: KeyMode\, change\: \&SlackChange\, by\: Uuid\, at\: OffsetDateTime\) \-\> Option\<PendingSlackAppSave\>

Store\:\:hold\_pending\_slack\_app

async fn hold\_pending\_slack\_app\(\&self\, app\: \&PendingSlackAppSave\) \-\> Result\<SlackSaveOutcome\, StoreError\>

Store\:\:apply\_pending\_slack\_app

async fn apply\_pending\_slack\_app\(\&self\, actor\: Uuid\, now\: OffsetDateTime\) \-\> Result\<PendingApplied\, StoreError\>

Store\:\:remove\_slack\_app

async fn remove\_slack\_app\(\&self\, actor\: Uuid\, now\: OffsetDateTime\) \-\> Result\<RemoveOutcome\, StoreError\>

Store\:\:connected\_slack\_workspaces

async fn connected\_slack\_workspaces\(\&self\) \-\> Result\<i64\, StoreError\>

Store\:\:prune\_pending\_slack\_app

async fn prune\_pending\_slack\_app\(\&self\, now\: OffsetDateTime\) \-\> Result\<\(\)\, StoreError\>

SlackOffer\:\:applies

fn applies\(self\, effect\: AppInEffect\, mode\: KeyMode\) \-\> bool

confirm\_lines

fn confirm\_lines\(current\_client\_id\: \&str\, connected\: u32\) \-\> String

remove\_lines

fn remove\_lines\(connected\: u32\) \-\> String

### Laws

#### L1

```text
A submission whose trimmed client ID equals the current app's is a Rotate, and a blank secret field in a Rotate keeps the saved value; a submission with a different client ID is a NewApp and refuses a blank secret field with I-11 naming that field.
```

Invariant

Domain

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs

##### Treatment

Finite

###### Cases

```text
same client ID, both secret fields blank: both saved secrets kept
```

```text
same client ID, only the signing secret given: the client secret kept, the signing secret replaced
```

```text
same client ID, both secrets given: both replaced
```

```text
different client ID, client secret blank: I-11 for the client secret, nothing stored
```

```text
different client ID, signing secret blank: I-11 for the signing secret, nothing stored
```

```text
different client ID, both secrets given: accepted as a NewApp
```

```text
same client ID over a saved app that does not open, a secret blank: I-11 for that field, because there is nothing to keep
```

```text
The form has three fields over a closed verdict, so the whole relation is a finite table rather than a sampled domain.
```

##### Evidence

None.

#### L2

```text
Field shapes and trimming are unchanged by the replace path: a value's verdict is the same with surrounding whitespace, any 32 lowercase hex value is a secret, and any character outside a field's alphabet is malformed rather than blank.
```

Invariant

Domain

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs

##### Treatment

Broad

###### Generator

```text
each of the three form fields, over the alphabet 0-9 a-f and the dot, plus arbitrary Unicode, each value wrapped in up to four whitespace characters, and the blank value
```

40 characters per field value and 4 whitespace characters on each side

the runner\'s default seed\, printed on failure so the case replays

###### Must include

```text
a blank field
```

```text
a 32 lowercase hex secret
```

```text
an uppercase hex secret
```

```text
a client ID with two dots and one with no dot
```

```text
a value holding one character outside its field's alphabet
```

one field per generated case

```text
the native generator's shrinking to the shortest refused value, with the case printed for replay
```

```text
a hand-written alphabet-and-length predicate over the trimmed value, written apart from the production check
```

```text
a value the predicate accepts that the field check refuses, or a value the predicate rejects that it accepts
```

##### Evidence

None.

#### L3

```text
The I-14 gate holds exactly when the submission is a NewApp and the connected-workspace count is above zero; a NewApp with no connected workspace saves at once, and a Rotate never reaches the confirm.
```

Invariant

Application

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

##### Treatment

Finite

###### Cases

```text
NewApp with 0 connected workspaces: saved at once, no pending row
```

```text
NewApp with 1 connected workspace: held pending, I-14's n = 1 form
```

```text
NewApp with 3 connected workspaces: held pending, I-14's n >= 2 form
```

```text
Rotate with 3 connected workspaces: saved at once, no pending row
```

```text
NewApp with 1 connected workspace while the environment's app is in effect: held pending, I-14 naming the environment's client ID
```

```text
The gate is a pure function of two closed facts, so every combination is tabled.
```

##### Evidence

None.

#### L4

```text
The pending row is applied only for the requester that holds it and only while the injected clock is before its expiry, and applying it deletes it, so a second apply, another admin's apply and an expired apply all change nothing.
```

Invariant

Application

crates\/foundry\-store\/src\/integrations\/instance\.rs

##### Treatment

Finite

###### Cases

```text
the holder confirms one minute in: applied, the row deleted, one activity line, one version bump
```

```text
the holder confirms at the expiry instant: nothing applied
```

```text
the holder confirms eleven minutes in: nothing applied
```

```text
another instance admin confirms one minute in: nothing applied, the row left as it is
```

```text
the holder confirms twice: the second confirm finds no row
```

```text
no pending row exists: the confirm changes nothing
```

```text
The decision reads three facts with a closed outcome, so the table is the whole relation.
```

##### Evidence

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_app\_provenance\.qnt

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_app\_provenance\.receipt\.txt

#### L5

```text
With the environment's app in effect neither Replace nor Remove is offered, and the GET and the POST of the remove door delete no row, write no activity line and bump no version.
```

Invariant

Application

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

##### Treatment

Finite

###### Cases

```text
the environment's app in effect with a saved row present: no Replace or Remove control; the POST changes nothing and answers the page
```

```text
the environment's app in effect with no saved row: no Replace or Remove control; the POST changes nothing
```

```text
the saved app in effect: both controls offered and the POST removes it
```

```text
no app in effect: neither control offered, and the POST changes nothing
```

```text
The offer set is a pure function of the app in effect and the key mode, both closed enums.
```

##### Evidence

None.

#### L6

```text
A GET of the confirm door or of the remove door leaves the stored rows, the activity table and the config version exactly as they were.
```

Invariant

Application

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

##### Treatment

Finite

###### Cases

```text
GET the confirm door twice with a pending row: the row, the activity list and the version unchanged
```

```text
GET the confirm door with no pending row: nothing changed, the page answered
```

```text
GET the remove door twice with a saved app: the row, the activity list and the version unchanged
```

```text
The two GET handlers reach only read methods, which the table and the capability set together decide.
```

##### Evidence

None.

#### L7

```text
Removing the saved app changes no connection row, writes no lost_reason, sends no notice and cancels no delivery; afterwards a workspace integrations page reads W-2 and POST /integrations/slack/events answers 401.
```

Invariant

Infrastructure

crates\/foundry\-store\/src\/integrations\/instance\.rs

##### Treatment

Finite

###### Cases

```text
one connected workspace: its connection state and lost_reason unchanged, no email recorded, its pending delivery still pending
```

```text
no connected workspace: nothing to preserve, the removal still answers I-16
```

```text
after removal: the workspace integrations page reads W-2 and an event signed with the removed signing secret answers 401
```

```text
The preserved set is the shipped connection and delivery rows; the cases are the presence or absence of one connection.
```

##### Evidence

None.

#### L8

```text
A Rotate that keeps a secret re-seals the secret the server already holds opened, so the app in effect opens afterwards to the same secrets it opened to before, except for the fields the Rotate replaced.
```

RoundTrip

Domain

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs

##### Treatment

Finite

###### Cases

```text
both fields blank: both opened secrets equal to before
```

```text
the signing secret replaced: the opened client secret equal to before, the signing secret the new one
```

```text
both replaced: both opened secrets the new ones
```

```text
Three cases cover the closed set of kept-or-replaced combinations for two fields, with the blank-blank case the identity.
```

##### Evidence

None.

#### L9

```text
A saved, replaced or removed app is in effect on every replica within 60 seconds of its commit, with no restart.
```

Temporal

Infrastructure

crates\/foundry\-app\/src\/integrations\/reload\.rs

##### Treatment

Finite

###### Cases

```text
two replicas on one database, the shared clock advanced past one reload interval, each replica asked directly after a replace
```

```text
the same after a removal: both replicas answer 401 to an event signed with the removed secret
```

```text
The shipped version poll and reload carry it; the finite observation is the two-replica scenario. The quantified-over-time form of this claim is the TLA+/TLC obligation recorded as INDETERMINATE.
```

##### Evidence

docs\/feature\/iis\-slack\-replace\-remove\/design\/replica\_convergence\.tla

docs\/feature\/iis\-slack\-replace\-remove\/design\/replica\_convergence\.receipt\.txt

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_sweep\.tla

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_sweep\.receipt\.txt

#### L10

```text
Every write in this design is refused when the key it sealed under is no longer the sealing key, and the pending row's secrets are re-sealed with every other sealed value when the secrets key moves.
```

Invariant

Infrastructure

crates\/foundry\-store\/src\/integrations\/instance\.rs

##### Treatment

Finite

###### Cases

```text
a save sealed under a superseded key: refused, nothing stored, the handler reloads and retries once
```

```text
a pending hold sealed under a superseded key: refused, no pending row
```

```text
a Move while a pending row exists: the pending secrets re-sealed with the rest, and still openable at the confirm
```

```text
a Move while a pending row exists: the I-22 credential count unchanged by the pending row
```

```text
The fence and the re-seal set are decided per write; the cases are the superseded-key write and the re-seal with and without a pending row.
```

##### Evidence

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_app\_provenance\.qnt

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_app\_provenance\.receipt\.txt

#### L11

```text
I-14 and I-15 each read the form the connected-workspace count selects: no workspace, exactly one, or more than one.
```

Invariant

Application

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

##### Treatment

Finite

###### Cases

```text
0 connected workspaces: I-15's n = 0 form; I-14 is not reached
```

```text
1 connected workspace: I-14's n = 1 form; I-15's n = 1 form
```

```text
2 connected workspaces: I-14's n >= 2 form; I-15's n >= 2 form reading 2
```

```text
3 connected workspaces: I-14's n >= 2 form; I-15's n >= 2 form reading 3
```

```text
The count partitions into the three locked forms at 0, at 1 and above 1, so four cases decide the whole mapping.
```

##### Evidence

None.

### Refuted

```text
A blank secret field can always keep the saved value, whatever client ID the form submitted.
```

```text
The carrier was one three-field form with no notion of which app it names. The saved secrets belong to the saved app, so keeping them under a different client ID would seal the old app's secrets to a new app's ID. The missing distinction is the submission's own kind, not a guard added over the unchanged form.
```

```text
Client ID 6123456789012.8888888888888 with both secret fields blank over the saved app 6123456789012.7234567890123: the save would store the new ID with the old app's secrets, and every event and every code exchange would then fail.
```

```text
The submitted app awaiting its confirm can be held in the session table.
```

```text
The session table sits outside the seal fence and outside the re-seal set, so its ciphertext is not re-sealed when the sealing key changes; the law that every sealed value is re-sealed does not hold over that carrier.
```

```text
A Move between the submission and the confirm re-seals every sealed value but not the session row, so the confirm opens nothing and the admin's submission is silently lost.
```

```text
The connected-workspace count can be read from this server's rebuilt integrations.
```

```text
A workspace connection does not bump the config version, so the rebuilt value is stale by construction; the count must be read from the store at the moment it is used, which is a different observation than the reload's snapshot.
```

```text
A workspace connects after the last version bump: the gate reads zero connected workspaces and saves a different app at once, with no I-14 at all.
```

### Claims

#### C1

```text
The saved Slack app only ever becomes an app that an instance admin submitted and, where that submission named a different client ID while workspaces were connected, that the same instance admin confirmed before its 10-minute expiry.
```

```text
A switched app that nobody confirmed silently breaks every connected workspace's posting and every event's verification, with no warning and no record of who chose it.
```

crates\/foundry\-store\/src\/integrations\/instance\.rs

```text
a POST body of three trimmed form fields from an authenticated instance admin, plus the stored pending row and the injected clock's now
```

Store\:\:apply\_pending\_slack\_app

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs confirm\_slack\_app\, and the page\, events and install paths through IntegrationsHandle\:\:current

```text
GET /admin/instance/integrations names the app in effect and its activity line names who replaced it; Add to Slack's client ID and the event verification follow the same app
```

```text
An outside writer of instance_slack_app or instance_slack_app_pending, a restore of the database from backup, or a previous-release replica during a roll that does not know the pending table.
```

#### C2

```text
No confirm page, remove page, activity line or log line can carry a Slack secret.
```

```text
A leaked client or signing secret lets anyone impersonate this Foundry to Slack and forge its events.
```

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

```text
the stored sealed secrets and the submitted form values
```

the typed page state structs\, which hold no secret field\, and the activity row\, which has only an action\, a client ID and a count

crates\/foundry\-app\/templates\/integrations\/instance\.html\, slack\_confirm\.html and slack\_remove\.html

```text
The rendered pages hold the client ID and never a secret, and the acceptance lane greps the page and the audit table for planted secrets
```

```text
A template interpolating a sealed value or a form field directly instead of the typed page state, a log line formatting the submission, or a database dump.
```

#### C3

```text
A GET of the confirm door or the remove door is side-effect free.
```

```text
A link prefetch or a crawler would otherwise switch or remove the Slack app for the whole instance without anyone choosing it.
```

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

```text
an authenticated instance admin's GET with no body
```

the read\-only path through the store capability\'s read methods

the confirm and remove templates

```text
After two GETs of either door, the instance page, its activity list and the config version read exactly as before
```

```text
A handler that calls a write method from a GET, which only rustc's capability split and the review can exclude.
```

### States

Admitted

```text
A pending row whose expiry has passed and whose retention sweep has not yet run: present, and applicable to nobody.
```

crates\/foundry\-store\/src\/integrations\/instance\.rs

Admitted

```text
A saved app in effect and a pending row for a different app at the same time: the saved app stays in effect until the confirm.
```

crates\/foundry\-store\/src\/integrations\/instance\.rs

Admitted

```text
The environment's app in effect with a saved app also stored: the environment wins, and Replace and Remove are withheld.
```

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

Admitted

```text
No app in effect after a removal, with Slack connections still holding their bot tokens.
```

crates\/foundry\-store\/src\/integrations\/instance\.rs

Excluded

```text
Two pending apps at once: excluded by the single-row primary key with CHECK (id = 1).
```

crates\/foundry\-store\/src\/integrations\/instance\.rs

Excluded

```text
A saved or pending app with only one of its two secrets written: excluded by the NOT NULL columns and the single upsert.
```

crates\/foundry\-store\/src\/integrations\/instance\.rs

Excluded

```text
A confirm that applies a row it did not read in the same transaction: excluded by the exclusive advisory lock held across the read, the write and the delete.
```

crates\/foundry\-store\/src\/integrations\/instance\.rs

Excluded

```text
A pending app sealed under a key that is not the sealing key: excluded by the fence on the hold.
```

crates\/foundry\-store\/src\/integrations\/instance\.rs

Unknown

```text
A pending row that no held key opens after an outside restore of the database: the confirm cannot open it. The confirm treats it as absent and the boot probe names it.
```

the instance operator

Unknown

```text
Events from a replaced app arriving after the switch: they fail verification, and uninstall detection for those workspaces falls back to refusals.
```

the instance admin

Unknown

```text
How long a workspace admin takes to reconnect Slack after a replace or a removal.
```

the workspace admin

### Models

Quint

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_app\_provenance\.qnt

##### Properties

provenanceInv

Invariant

```text
The saved app, whenever present, was either saved directly by an admin whose submission named the app in effect or found no connected workspace, or applied by the same admin who held the pending row, read before its expiry.
```

provenanceInv

Store\:\:apply\_pending\_slack\_app and save\_slack\_app in crates\/foundry\-store\/src\/integrations\/instance\.rs

pendingSealedInv

Invariant

```text
A held pending row is always sealed under the current sealing key, because a secrets-key Move re-seals it with every other sealed value.
```

pendingSealedInv

reseal\_sealed\_values in crates\/foundry\-store\/src\/integrations\/instance\.rs

noSaveReachable

Invariant

```text
A reachability witness, expected to be violated: its counterexample is the trace that constructs a saved app, so the safety claim is not vacuous.
```

noSaveReachable

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs save\_slack\_app

noConfirmRouteReachable

Invariant

```text
A reachability witness, expected to be violated: its counterexample is the trace that constructs a saved app through the confirm route.
```

noConfirmRouteReachable

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs confirm\_slack\_app

##### Atoms

State

saved

instance\_slack\_app\.client\_id

crates\/foundry\-store\/migrations\/0033\_instance\_integrations\.sql\:30

None.

State

envApp

Settings\.slack\.client\_id

crates\/foundry\-app\/src\/integrations\/config\.rs\:1

None.

State

pending

instance\_slack\_app\_pending\.client\_id

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

None.

State

pendingBy

instance\_slack\_app\_pending\.requested\_by

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

None.

State

pendingExpires

instance\_slack\_app\_pending\.expires\_at

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

None.

State

pendingSealedUnder

the sealing key id the pending secrets were sealed under

crates\/foundry\-store\/src\/integrations\/instance\.rs\:136

None.

State

sealingKey

integration\_secrets\_key\.key\_id\, as stored\_key\_id reads it

crates\/foundry\-store\/src\/integrations\/instance\.rs\:409

None.

State

now

Arc\<dyn Clock\>\:\:now

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:502

None.

State

connected

Store\:\:connected\_slack\_workspaces

crates\/foundry\-store\/src\/integrations\/instance\.rs\:452

None.

Action

submit

save\_slack\_app

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:502

```text
One transaction under the advisory lock: the gate, then either the fenced upsert with its activity line and version bump, or the fenced pending hold.
```

Action

confirm

Store\:\:apply\_pending\_slack\_app

crates\/foundry\-store\/src\/integrations\/instance\.rs\:406

```text
One transaction under the exclusive advisory lock: the read of the pending row, the requester and expiry decision, the upsert, the delete, the activity line and the version bump.
```

Action

remove

Store\:\:remove\_slack\_app

crates\/foundry\-store\/src\/integrations\/instance\.rs\:406

```text
One transaction under the exclusive advisory lock: the delete, the activity line and the version bump.
```

Action

sweep

Retention\:\:tick

crates\/foundry\-app\/src\/integrations\/worker\.rs\:290

```text
One statement deleting expired pending rows, outside any other write.
```

Action

move

Store\:\:store\_secrets\_key

crates\/foundry\-store\/src\/integrations\/instance\.rs\:293

```text
One transaction under the exclusive advisory lock, re-sealing every sealed value including the pending row.
```

Action

reconnect

upsert\_connection

crates\/foundry\-store\/src\/integrations\/connections\.rs\:247

```text
One transaction under the shared advisory lock; it bumps no config version.
```

Constant

Ttl

the 10\-minute pending expiry \(ADR\-IIS\-003 r4\)

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

None.

##### Assumptions

```text
Every instance write serialises on one Postgres advisory exclusive lock (ADR-IIS-004 r1), so one model action is one committed transaction; the model explores the orderings of those transactions, not interleavings inside one.
```

```text
The injected clock is monotone on a single server; skew between replicas is outside the model and is carried as a residual risk.
```

```text
Two apps and two instance admins, a clock horizon of 4 and an expiry of 2 ticks, with up to 2 connected workspaces, are enough to exhibit every ordering the claim quantifies over; the exhaustive check runs to computation length 12.
```

```text
The environment's app is fixed for a run, which is how a server reads it: its trio is read at boot and a change needs a restart.
```

##### Mutants

###### N1

```text
A confirm that applies the pending row for any instance admin, not only its requester: the requester check is removed from the applying branch.
```

###### Kills

provenanceInv

###### N2

```text
A confirm that applies the pending row whatever the clock says: the expiry check is removed from the applying branch.
```

###### Kills

provenanceInv

###### N3

```text
A secrets-key Move that re-seals every sealed value but forgets the pending row.
```

###### Kills

pendingSealedInv

###### N4

```text
A submit that saves a different client ID at once even while workspaces are connected, so the confirm is never reached.
```

###### Kills

provenanceInv

TlaPlus

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_sweep\.tla

##### Properties

SweptEventually

Temporal

```text
Whenever the pending row is expired, it eventually stops being expired: the sweep, a confirm or a fresh hold removes it.
```

SweptEventually

crates\/foundry\-app\/src\/integrations\/worker\.rs\:290

TypeOK

Invariant

```text
The pending row, its requester, the clock and the expiry stay inside their declared ranges.
```

TypeOK

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

##### Atoms

State

pending

instance\_slack\_app\_pending\.client\_id

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

None.

State

now

Arc\<dyn Clock\>\:\:now

crates\/foundry\-app\/src\/integrations\/worker\.rs\:267

None.

Action

Sweep

Retention\:\:tick

crates\/foundry\-app\/src\/integrations\/worker\.rs\:290

```text
One delete statement of its own.
```

Action

Hold

Store\:\:hold\_pending\_slack\_app

crates\/foundry\-store\/src\/integrations\/instance\.rs\:406

```text
One fenced transaction under the shared advisory lock.
```

Constant

Ttl

the 10\-minute pending expiry

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

None.

##### Assumptions

```text
Weak fairness on the retention sweep and on the clock: the worker runs and time advances. Without that fairness the claim is false, which the no-fairness run shows.
```

```text
Two admins, two apps, an expiry of 2 ticks and a clock horizon of 3 bound the state space to 44 distinct states, so the check is over the complete state space rather than a sample.
```

```text
A hold, a confirm and a sweep are each one committed transaction, as in the Quint model.
```

##### Mutants

###### N5

```text
The same specification with no fairness at all, so the sweep may stutter forever while a row is expired.
```

###### Kills

SweptEventually

###### N6

```text
A progress mutant under fairness: a sweep that only deletes rows held by one named admin, so another admin's expired row survives.
```

###### Kills

SweptEventually

TlaPlus

docs\/feature\/iis\-slack\-replace\-remove\/design\/replica\_convergence\.tla

##### Properties

Converges

Temporal

```text
Whenever a replica's applied version differs from the committed config version, that replica eventually applies the committed version, with no restart.
```

Converges

crates\/foundry\-app\/src\/integrations\/reload\.rs\:1

TypeOK

Invariant

```text
The committed version and every replica's applied version stay inside their declared range.
```

TypeOK

crates\/foundry\-store\/src\/integrations\/instance\.rs\:278

##### Atoms

State

version

integration\_config\_version\.version

crates\/foundry\-store\/src\/integrations\/instance\.rs\:278

None.

State

applied

integration\_replica\_states\.applied\_version

crates\/foundry\-store\/src\/integrations\/instance\.rs\:542

None.

Action

Commit

commit\_recorded

crates\/foundry\-store\/src\/integrations\/instance\.rs\:775

```text
One transaction that writes the change, its activity line and the version bump together.
```

Action

Reload

the reload task\'s rebuild from one snapshot

crates\/foundry\-app\/src\/integrations\/reload\.rs\:1

```text
One REPEATABLE READ snapshot read, with the version read first (ADR-IIS-004 r4).
```

##### Assumptions

```text
Weak fairness on each replica's reload task: every replica's reload runs. Without it the claim is false, which the no-fairness run shows.
```

```text
Two replicas and at most three committed versions bound the state space to 60 distinct states, so the check is over the complete state space.
```

```text
The model carries eventual convergence only. The 60-second figure of the promise follows from the configured poll interval, FOUNDRY_INTEGRATIONS_RELOAD_MS with its 5000 ms default, and is observed by the two-replica acceptance scenario, not proved here.
```

##### Mutants

###### N7

```text
The same specification with no fairness, so a replica may stutter forever and never apply the committed version.
```

###### Kills

Converges

###### N8

```text
A progress mutant under fairness: a replica that reloads only when the committed version is even, so an odd version never reaches it.
```

###### Kills

Converges

### Closure

```text
The pending-app provenance claim: the saved app only ever becomes a submitted and same-admin-confirmed app
```

Safety

Pass

DesignedNotBuilt

```text
Quint 0.33.1 with the Apalache 0.62.4 backend checked provenanceInv exhaustively over init and step to length 12 with the outcome NoError, and each of four behavioural mutants - a confirm ignoring the requester, a confirm ignoring the expiry, a Move forgetting to re-seal the pending row, and a submit saving while workspaces are connected - is refuted, by simulation and exhaustively. Two reachability witnesses are violated as intended, so the claim is not vacuous: both a save and a confirm-route save are constructed.
```

##### Evidence

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_app\_provenance\.qnt

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_app\_provenance\.receipt\.txt

```text
No secret on a page, in the activity table or in a log line
```

Safety

Pass

DesignedNotBuilt

```text
rustc over the typed page-state structs, which declare no secret field, plus the exhaustive page table; the one route no checker closes is a template writing a sealed value literally, recorded as a Bypass risk.
```

##### Evidence

None.

```text
A GET of either door changes nothing
```

Safety

Pass

DesignedNotBuilt

```text
rustc over the read-only capability the GET handlers hold, plus the finite table over two GETs of each door.
```

##### Evidence

None.

```text
The submission verdict is total over the three-field form
```

Safety

Pass

DesignedNotBuilt

```text
rustc rejects a non-exhaustive match over the closed SlackChange and Refusal enums, and the exhaustive table decides the whole relation.
```

##### Evidence

None.

```text
The offer set and the remove gate are total over the app in effect
```

Safety

Pass

DesignedNotBuilt

```text
rustc over the closed AppInEffect and KeyMode enums, with the finite table covering every pair.
```

##### Evidence

None.

```text
A replaced app exists: a submitted and confirmed app becomes the saved app
```

Inhabitation

Pass

DesignedNotBuilt

```text
Admitted input: a POST of client ID 6123456789012.8888888888888 with both secrets by an instance admin. Constructor: SlackSubmission::judge then plan_hold_pending then Store::hold_pending_slack_app then Store::apply_pending_slack_app. Consumer: the page, events and install through IntegrationsHandle::current. Acyclic from the POST body, with no bootstrap needed.
```

##### Evidence

None.

```text
A removed app exists: no app in effect with connections intact
```

Inhabitation

Pass

DesignedNotBuilt

```text
Admitted input: an instance admin's POST of /admin/instance/integrations/slack/remove with a saved app in effect. Constructor: Store::remove_slack_app. Consumer: NoConnectionCause::words on the workspace page and the event verification. Acyclic from the POST.
```

##### Evidence

None.

```text
The I-14 confirm reaches a public observation
```

PublicReachability

Pass

DesignedNotBuilt

```text
POST /admin/instance/integrations/slack reaches save_slack_app in handlers/instance.rs, which holds the pending row and answers the confirm page; GET /admin/instance/integrations/slack/confirm renders slack_confirm.html; POST of it reaches confirm_slack_app and the saved app is then observed on GET /admin/instance/integrations and through Add to Slack's client ID.
```

##### Evidence

None.

```text
The I-15 remove confirm and I-16 reach a public observation
```

PublicReachability

Pass

DesignedNotBuilt

```text
GET /admin/instance/integrations/slack/remove renders slack_remove.html with the count from Store::connected_slack_workspaces; POST of the same path reaches remove_slack_app and answers I-16, observed on the instance page, on GET /workspace/integrations as W-2 and as a 401 from POST /integrations/slack/events.
```

##### Evidence

None.

```text
An expired pending app is eventually deleted
```

Liveness

Pass

DesignedNotBuilt

```text
TLC 1.7.4 checked SweptEventually over the complete state space of pending_sweep.tla: no error under weak fairness on the sweep and the clock, a counterexample with no fairness at all, and a counterexample for a sweep that only deletes one admin's rows. The fairness assumption is load-bearing and named, and the single-row table bounds the residue to one row.
```

##### Evidence

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_sweep\.tla

docs\/feature\/iis\-slack\-replace\-remove\/design\/pending\_sweep\.receipt\.txt

```text
A committed change is in effect on every replica within 60 seconds
```

Liveness

Pass

DesignedNotBuilt

```text
TLC 1.7.4 checked Converges over the complete state space of replica_convergence.tla: no error under weak fairness on each replica's reload, a counterexample with no fairness, and a counterexample for a replica reloading only on an even version. The model carries the eventual convergence; the 60-second figure follows from the configured 5000 ms poll interval and is observed by the two-replica scenario.
```

##### Evidence

docs\/feature\/iis\-slack\-replace\-remove\/design\/replica\_convergence\.tla

docs\/feature\/iis\-slack\-replace\-remove\/design\/replica\_convergence\.receipt\.txt

### Risks

#### Y1

Stressor

```text
Assumption negated: every replica's clock agrees. A replica whose clock lags judges a pending row unexpired after the holder's ten minutes have passed.
```

```text
Only the requester can apply it, and it applies at most once.
```

```text
A confirm succeeding slightly after the ten minutes on a lagging replica.
```

```text
Another instance admin applying it, or an applied row surviving its own application.
```

```text
The expiry is only as sharp as the worst clock skew between replicas.
```

the instance operator

#### Y2

Stressor

```text
Assumption negated: a worker is running. With no worker, expired pending rows are never swept.
```

```text
No expired row can ever be applied, because the confirm judges the expiry itself.
```

```text
One stale row sitting in the single-row table.
```

```text
An expired row applying.
```

```text
None beyond one row of sealed bytes, which the next hold overwrites.
```

the design

#### Y3

Stressor

```text
Assumption negated: the store answers. The connected-workspace count read fails while I-14 or I-15 is being rendered.
```

```text
Nothing is written.
```

```text
The door answering the instance page with the reason logged and no count rendered.
```

```text
A failed read rendered as a count of zero, so I-15's n = 0 form stands in for an unknown count.
```

```text
The design must carry an unread count as its own case, not as zero.
```

the crafter

#### Y4

Stressor

```text
Assumption negated: only this Foundry writes its tables. An operator edits instance_slack_app_pending by hand or restores it from a backup.
```

```text
The fence refuses a row sealed under another key, and the confirm opens nothing it cannot open.
```

```text
The confirm finding nothing to apply and answering the page.
```

```text
Applying a row no held key opens.
```

```text
An unopenable pending row is named by the boot probe and must be cleared by the operator.
```

the instance operator

#### Y5

Stressor

```text
Assumption negated: the connected set is stable while a confirm is open. A workspace connects or disconnects between the I-14 count and the confirm.
```

```text
The confirm still applies for its requester.
```

```text
I-14 having named a count that has since changed.
```

```text
The count deciding whether the confirm applies.
```

```text
I-14's count is a reading taken at render time, by design.
```

the design

#### Y6

SinglePointOfFailure

```text
The one instance-integrations advisory lock serializes every instance write: Replace, the pending hold, the confirm, Remove, Generate and Move.
```

```text
Correctness: no two instance writes interleave.
```

```text
A door waiting for the lock.
```

```text
A write proceeding without the lock, or a row lock taken before it.
```

```text
A long transaction stalls every instance write for its duration.
```

the design

#### Y7

SinglePointOfFailure

```text
The single-row instance_slack_app row is the only saved app, so a removal leaves the instance with no app until one is saved again.
```

```text
Every connection and its bot token.
```

```text
Slack being unavailable instance-wide until an app is saved.
```

```text
Connections being marked lost, workspace admins being emailed, or deliveries being cancelled.
```

```text
The gap is the instance admin's own intent, which I-15 states before the removal.
```

the instance admin

#### Y8

Bypass

```text
A template interpolating a sealed value or a submitted form field directly instead of the typed page state.
```

```text
Nothing: the claim would be broken.
```

```text
Nothing.
```

```text
Any secret in a response body.
```

```text
No checker decides a template's literals; the acceptance grep for planted secrets and the implementation review carry it.
```

the implementation reviewer

#### Y9

Bypass

```text
SQL naming an instance table outside crates/foundry-store/src/integrations/instance.rs, or a sealed-column write outside the fenced writers.
```

```text
One writer per table and the fence on every sealed write.
```

```text
Nothing.
```

```text
An unfenced sealed write or a second writer of the pending table.
```

```text
check-arch decides it lexically over string literals, so a query assembled at runtime escapes it.
```

the crafter

#### Y10

Bypass

```text
Process failure between the pending hold and the confirm, or inside a write between the delete and the version bump.
```

```text
Each write is one transaction, so a failure leaves the previous state whole.
```

```text
A lost pending row, which the admin can submit again.
```

```text
A committed app with no version bump or no activity line.
```

```text
None within one transaction; across the two steps the admin repeats the submission.
```

the design

#### Y11

Stressor

```text
Second walk: a replica rolls back to a previous release while a pending row exists, so the older code does not know the pending table.
```

```text
The pending row and the saved app are untouched by the older code.
```

```text
The confirm door answering the uniform 404 on the old replica.
```

```text
An old replica saving the new app with no confirm.
```

```text
The confirm door must exist on every replica before an admin is sent to it; the roll order is the operator's.
```

the instance operator

#### Y12

Stressor

```text
Second walk: the same instance admin opens two tabs on the confirm page and posts both.
```

```text
Exactly one save.
```

```text
The second tab answering the page with nothing changed.
```

```text
Two saves or two activity lines for one submission.
```

```text
None: the row is deleted inside the applying transaction.
```

the design

#### Y13

Stressor

```text
Second walk: a workspace admin connects Slack through the old app between the submission and the confirm.
```

```text
The new connection keeps its own bot token and its state.
```

```text
I-14's count being one short of the truth at the moment of the confirm.
```

```text
The new connection being marked lost or dropped.
```

```text
The count is a reading, and the new workspace keeps posting through the old app like the others.
```

the design

#### Y14

Stressor

```text
Second walk: an instance admin is removed as an instance admin between the submission and the confirm.
```

```text
The pending row stays unapplied.
```

```text
The confirm answering the uniform 404 and the row expiring unapplied.
```

```text
A confirm applying for someone who is no longer an instance admin.
```

```text
None: the guard runs before the body on every door.
```

the design

### Architectural tests

#### T1

```text
SQL naming instance_slack_app_pending appears only in crates/foundry-store/src/integrations/instance.rs, and every write of a sealed column stands inside a listed fenced writer, the new pending hold included (ADR-IIS-001 Enforcement, ADR-IIS-004 r2). The check covers every crate's src, members added later included.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh cargo xtask check-arch
```

### Contract tests

#### T2

```text
A stored activity action written by the store renders as I-18 words it, so a persisted slack_app_replaced or slack_app_removed row written by one release reads correctly in another; the generic underscore fallback must not answer for either.
```

instance\_integration\_audit\_events\.action wire names \(ADR\-IIS\-005 r1\)

crates\/foundry\-store\/src\/integrations\/instance\.rs save\_slack\_app and remove\_slack\_app

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs activity\_line

```text
After the real store writes carry out a replace and a removal, GET /admin/instance/integrations Activity reads the two I-18 lines with their client IDs, not 'slack app replaced' or 'slack app removed'.
```

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

```text
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-05'
```

## Slices

### S1

#### Targets

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs

Extend

```text
The submission kind, the replace-aware verdict and the plan that re-seals a kept saved secret.
```

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

Extend

```text
The replace form's pre-filled client ID, the replace notice and the I-18 replaced arm.
```

crates\/foundry\-app\/templates\/integrations\/instance\.html

Extend

```text
The replace form over a saved app in effect: the client ID pre-filled, both secret fields empty.
```

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

#### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::slack_app
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::handlers::instance
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-05'
scripts/test-in-lima.sh cargo xtask check-arch
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/external\_integration\_plugins\.rs

#### Public oracle

```text
With the Slack app 6123456789012.7234567890123 saved and in effect and no workspace connected to Slack, Priya Raman POSTs /admin/instance/integrations/slack through the production router behind require_instance_admin, with her CSRF token, the same client ID, a blank client secret and the new signing secret b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05. In a second run she POSTs the client ID 6123456789012.8888888888888 with a blank client secret.
```

```text
The first response reads 'Saved. Workspace admins can now choose Add to Slack.', the page still reads app 6123456789012.7234567890123 with both secrets set, and the Activity list reads '21 Oct 2026 . Priya Raman replaced the Slack app (6123456789012.7234567890123)'. Within 60 seconds and with no restart, POST /integrations/slack/events signed with the new signing secret verifies on each replica asked directly, while Add to Slack still exchanges Slack's code with the kept client secret. The second response reads 'Enter the client secret from the Slack app's Basic Information.', keeps the typed client ID in the form with both secret fields empty, and the saved app, the activity list and the config version are unchanged.
```

```text
A blank client secret that clears or replaces the saved client secret, so Add to Slack's code exchange fails; a replace whose activity line reads 'saved the Slack app' or 'slack app replaced'; a different client ID with a blank secret that saves; or a new signing secret that verifies on the posting replica but not on the other within 60 seconds.
```

##### Promises

P1

P2

P6

#### Prefactoring

```text
L2: move the three per-field shape checks and the first-failing-field order out of SlackAppForm::refusal into a SlackFields value object that judges one named field at a time, with no verdict changed. It makes the follow-up change easier: the replace path asks the same object a different question - which fields this submission requires - instead of duplicating the checks.
```

```text
Every verdict of SlackField::check and SlackAppForm::refusal, including the trimming, the form order and the Refusal variants; the module's existing table test and its three property tests are unchanged in content.
```

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs

##### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::slack_app
```

#### Records

##### Decisions

None.

##### Constraints

None.

##### Reuse

refusal

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:186

Extend

```text
The verdict now depends on which app the submission names, so the one refusal rule is extended rather than copied into a replace-only rule.
```

refused

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:536

Extend

```text
It is the one map from a Refusal to its I-11 or I-12 words; the replace path refuses with the same words.
```

SavedSlackLines

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:163

Reuse

```text
I-6 and the two secret-set lines already describe the saved app, and the replace form sits beside them unchanged.
```

open\_saved

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:87

Reuse

```text
It already opens both saved secrets, which is exactly what a kept field needs; nothing new opens a stored secret.
```

##### Impact

template\, the Slack section

crates\/foundry\-app\/templates\/integrations\/instance\.html\:45

Changed

```text
The saved-app section gains the replace form with its pre-filled client ID.
```

pure decision module

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:186

Changed

```text
The verdict reads the current app and the saved secrets.
```

handler\, the save door

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:502

Changed

```text
The save door reads the current app before judging, and the notice and the activity arm follow.
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

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql

Create

```text
The single-row pending table with its 10-minute expiry column, in its own migration.
```

crates\/foundry\-store\/src\/integrations\/instance\.rs

Extend

```text
The fenced pending hold, the apply-and-delete, the connected-workspace count, the pending row in the re-seal set and in the snapshot.
```

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs

Extend

```text
The hold, confirm, count and prune methods on the one instance-write capability.
```

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs

Extend

```text
The two pending sealing slots and the sealed pending plan.
```

crates\/foundry\-app\/src\/integrations\/mod\.rs

Extend

```text
The sealed_slot arms for the two pending secrets and the boot probe's view of them.
```

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

Extend

```text
The I-14 gate on the save door, and the confirm door's GET and POST.
```

crates\/foundry\-app\/templates\/integrations\/slack\_confirm\.html

Create

```text
I-14 with its count and the current client ID, one Save the new app control and no secret.
```

crates\/foundry\-app\/src\/lib\.rs

Extend

```text
The GET and POST routes for the confirm door, behind the same guard and CSRF.
```

crates\/foundry\-app\/src\/integrations\/worker\.rs

Extend

```text
The hourly sweep also deletes expired pending rows.
```

xtask\/src\/check\_arch\.rs

Extend

```text
The pending hold joins SEAL_FENCED_WRITERS.
```

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

#### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-store --test iis_instance_store
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-05'
scripts/test-in-lima.sh cargo xtask check-arch
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/external\_integration\_plugins\.rs

#### Public oracle

```text
With Canzan Labs connected to Slack through the saved app 6123456789012.7234567890123, Priya Raman POSTs /admin/instance/integrations/slack with client ID 6123456789012.8888888888888 and both of its secrets, GETs /admin/instance/integrations/slack/confirm, then POSTs it. In further runs Tomas Rivera, also an instance admin, POSTs the confirm instead; Priya POSTs it eleven minutes later on the shared clock; and the whole run is repeated with all three SLACK_ settings in the environment so the environment's app is the current one.
```

```text
The first POST answers the confirm page reading '1 workspace is connected through the current Slack app (6123456789012.7234567890123). It keeps posting through it until a workspace admin reconnects Slack. Save the new app?' with a Save the new app control and no secret anywhere in it, while GET /admin/instance/integrations still names app 6123456789012.7234567890123 and the config version is unchanged. Priya's confirm then saves 6123456789012.8888888888888, the Activity reads 'Priya Raman replaced the Slack app (6123456789012.8888888888888)' and Add to Slack sends her to that client ID. Tomas's confirm changes nothing and answers the page; so does Priya's confirm eleven minutes later. With the environment's app current, the same POST answers the same I-14 line with the environment's client ID and the same confirm saves the new app.
```

```text
The first POST saving the new app or bumping the version; the confirm page carrying either secret; the count reading 0 or 3 for one connected workspace, or using the n >= 2 wording for it; another instance admin's confirm saving it; an expired pending app still applying; a second confirm saving a second time; or the environment-app run skipping the confirm.
```

##### Promises

P3

P6

#### Prefactoring

None.

#### Records

##### Decisions

None.

##### Constraints

None.

##### Reuse

save\_slack\_app

crates\/foundry\-store\/src\/integrations\/instance\.rs\:406

Extend

```text
The confirm's apply is the same fenced upsert with its source the pending row, so the write is not duplicated.
```

SlackSaveOutcome

crates\/foundry\-store\/src\/integrations\/instance\.rs\:175

Extend

```text
The hold and the apply report through the same outcome type the shipped save uses, so the handler keeps one retry rule.
```

retried

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:600

Reuse

```text
The stale-key retry is already defined once; the hold and the apply use it unchanged.
```

prune\_replica\_states

crates\/foundry\-store\/src\/integrations\/instance\.rs\:587

Reuse

```text
It is the shape a timed prune on the retention tick takes, so the pending prune follows it rather than inventing a second sweep shape.
```

client\_secret\_slot

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:20

Reuse

```text
The slot construction is already defined; the pending slots are two more of the same kind, bound to the pending row.
```

route

crates\/foundry\-app\/src\/lib\.rs\:858

Extend

```text
The router is the one public door list, behind the same session, CSRF and instance-admin layers as the shipped instance doors, so the confirm door's GET and POST is added there rather than in a second router or a nested route group.
```

##### Impact

database schema\, a new table

crates\/foundry\-store\/migrations\/0034\_instance\_slack\_app\_pending\.sql\:1

Changed

```text
A new single-row table; no shipped table is altered.
```

router\, the public door list

crates\/foundry\-app\/src\/lib\.rs\:858

Changed

```text
GET and POST of the confirm door are added.
```

worker tick

crates\/foundry\-app\/src\/integrations\/worker\.rs\:300

Changed

```text
The sweep gains the expired-pending prune.
```

architecture guard

xtask\/src\/check\_arch\.rs\:15043

Changed

```text
The pending hold is listed as a fenced writer.
```

snapshot type\, read by the reload and the boot probe

crates\/foundry\-store\/src\/integrations\/instance\.rs\:199

Migrated

```text
The pending row joins the snapshot; the reload ignores it and the boot probe counts its secrets among the values a held key must open.
```

module\, the sealed\-slot map

crates\/foundry\-app\/src\/integrations\/mod\.rs\:129

Changed

```text
sealed_slot gains the two pending arms, and the boot probe counts the pending secrets among the values a held key must open.
```

template\, the confirm page

crates\/foundry\-app\/templates\/integrations\/slack\_confirm\.html\:1

Changed

```text
A new template: I-14 with its count and the current client ID, one Save the new app control, and no secret or form field.
```

##### Agreement

None.

##### Not applicable

prefactoring

```text
S2 adds a table, a door and store functions beside the shipped ones and moves no shipped behaviour: the save door's existing body keeps its shape and gains a gate before it. The one preserving move this Request needs is S1's field-object extraction, declared there, and repeating it here would be a second claim over the same move.
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

crates\/foundry\-store\/src\/integrations\/instance\.rs

Extend

```text
The fenced remove: delete the row, record slack_app_removed, bump the version, in one transaction.
```

crates\/foundry\-app\/src\/integrations\/store\_capabilities\.rs

Extend

```text
The remove method on the one instance-write capability.
```

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

Extend

```text
The remove door's GET and POST, I-15, I-16, the Replace and Remove offers and the I-18 removed arm.
```

crates\/foundry\-app\/templates\/integrations\/slack\_remove\.html

Create

```text
I-15 with its count and one Remove control.
```

crates\/foundry\-app\/templates\/integrations\/instance\.html

Extend

```text
The Replace and Remove controls over a saved app in effect, and neither under the environment's app.
```

crates\/foundry\-app\/src\/lib\.rs

Extend

```text
The GET and POST routes for the remove door, behind the same guard and CSRF.
```

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

#### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-store --test iis_instance_store
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::handlers::instance
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-05'
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_lane instance-integration-setup'
scripts/test-in-lima.sh cargo xtask check-arch
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/external\_integration\_plugins\.rs

#### Public oracle

```text
With Canzan Labs connected to Slack through the saved app 6123456789012.7234567890123 and a pending delivery on one of its routes, Priya Raman GETs /admin/instance/integrations/slack/remove twice and then POSTs it, each through the production router with her CSRF token. In a second run all three SLACK_ settings are in the environment and she GETs and POSTs the same door. In a third run no workspace is connected.
```

```text
Each GET reads 'Remove the Slack app from this Foundry? 1 workspace's Slack routes stop posting until it is set up again, and no workspace can connect Slack.' and Slack is still set up after both of them, with the activity list and the config version unchanged. The POST answers 'Removed. Slack is not set up on this Foundry.', the Activity reads '21 Oct 2026 . Priya Raman removed the Slack app (6123456789012.7234567890123)', Canzan Labs' workspace integrations page reads 'Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once.' with its Slack connection not marked lost, the mail double has recorded no email, the pending delivery is still pending under the 24-hour give-up, and POST /integrations/slack/events answers 401 on each replica asked directly. With the environment's app in effect the instance page offers neither Replace nor Remove and both the GET and the POST change nothing and answer the page. With no workspace connected the GET reads 'Remove the Slack app from this Foundry? No workspace is connected to Slack.'
```

```text
A GET that removes the app or bumps the version; a removal that marks the connection lost, writes a lost_reason, records an email or cancels the pending delivery; a removal while the environment's app is in effect; the n = 0 wording for one connected workspace, the n = 1 wording for none, or the n >= 2 plural wording for one; an event still verifying after the removal; or an activity line reading 'slack app removed'.
```

##### Promises

P4

P5

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

applies

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:217

Reuse

```text
The shape of an offer that knows its label, its door and when it applies is already here; SlackOffer follows it so the page keeps one way of offering a control.
```

commit\_recorded

crates\/foundry\-store\/src\/integrations\/instance\.rs\:775

Reuse

```text
The remove ends like every other instance write: one activity line, one version bump, one commit.
```

NoConnectionCause

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:120

Reuse

```text
After the removal the workspace page's W-2 comes from the shipped cause set with no new words, as Request 1 left it.
```

stored\_notice

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:225

Reuse

```text
It is the shape a door's notice takes; I-16 is one more notice of that kind.
```

route

crates\/foundry\-app\/src\/lib\.rs\:858

Extend

```text
The router is the one public door list, behind the same session, CSRF and instance-admin layers as the shipped instance doors, so the remove door's GET and POST is added there rather than in a second router or a nested route group.
```

##### Impact

template\, the Slack section

crates\/foundry\-app\/templates\/integrations\/instance\.html\:45

Changed

```text
The saved-app section gains the Replace and Remove controls, withheld under the environment's app.
```

router\, the public door list

crates\/foundry\-app\/src\/lib\.rs\:858

Changed

```text
GET and POST of the remove door are added.
```

store module\, the only writer of the instance tables

crates\/foundry\-store\/src\/integrations\/instance\.rs\:406

Changed

```text
The fenced remove joins the writes beside the save.
```

workspace page\, the downstream reader

crates\/foundry\-app\/src\/integrations\/handlers\/workspace\.rs\:262

UnchangedCompatible

```text
It already reads W-2 from the shipped cause when no app is in effect, so a removal needs no change there.
```

template\, the remove page

crates\/foundry\-app\/templates\/integrations\/slack\_remove\.html\:1

Changed

```text
A new template: I-15 in the form the connected-workspace count selects, with one Remove control.
```

##### Agreement

None.

##### Not applicable

prefactoring

```text
S3 adds one store write, one door and two page offers beside the shipped ones; no shipped behaviour moves first. The page's offer decision follows the shipped KeyOffer shape already in place, so there is nothing to extract, and S1 declares this Request's one preserving move.
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

None.
