# Architecture brief: project\-key\-length

## Paradigm

object_oriented

## Shared records

None.

## Slices

### S1

#### Targets

crates\/foundry\-core\/src\/lib\.rs

Extend

```text
The bound becomes named constants, the length arm reads them and all three refusal arms render one sentence stating the whole rule.
```

crates\/foundry\-app\/src\/projects\.rs

Extend

```text
The restated operator sentence goes away; the handler renders the refusal Display.
```

crates\/foundry\-store\/migrations\/0035\_project\_key\_length\.sql

Create

```text
The new migration replaces the key_prefix check with the widened one, finding the existing constraint by its definition rather than by its name.
```

xtask\/src\/check\_arch\.rs

Extend

```text
A project-key clause joins the existing one-source rules so a second copy of the sentence fails the gate.
```

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature

#### Verification

```text
cargo xtask check-arch
cargo test -p foundry-core
cargo test -p foundry-store --test project_key_db_rule
cargo test -p foundry-acceptance --test acceptance
cargo xtask smoke
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/us\_07\_project\_create\.rs

crates\/foundry\-acceptance\/src\/steps\/us\_08\_file\_issue\.rs

crates\/foundry\-acceptance\/src\/support\/harness\.rs

crates\/foundry\-acceptance\/tests\/features\/project\-key\-length\-one\-source\.feature

crates\/foundry\-acceptance\/src\/steps\/feature\_project\_key\_length\.rs

crates\/foundry\-acceptance\/tests\/acceptance\.rs

crates\/foundry\-core\/tests\/project\_key\.rs

crates\/foundry\-core\/tests\/project\_name\.rs

crates\/foundry\-store\/tests\/project\_key\_db\_rule\.rs

#### Public oracle

```text
Signed in as a member of the Backend team, the operator posts the project create form with the name Platform Operations and the key PLATFORMOP, then files an issue in the project she lands on; next she posts the same form with the key PLATFORMOPSX; and she opens the board of a project created earlier with the key AUTH.
```

```text
The first post creates the project and lands on its board, which shows PLATFORMOP, and the filed issue carries the key PLATFORMOP-1. The second post creates nothing and answers a refusal whose message states that a project key must be 2 to 10 capital letters A to Z. The earlier AUTH project still opens, still shows AUTH and its issue keys are unchanged.
```

```text
A ten-letter key refused, an eleven-letter key accepted or stored, a refusal whose message still states a 2 to 6 rule, an issue key whose prefix is truncated or case-folded, or an earlier two to six letter project that no longer opens.
```

##### Promises

P1

P2

P3

#### Prefactoring

```text
Give the key rule one owner with no behaviour change: move the 2 and the 6 into named constants in foundry-core, make the refusal Display the operator sentence byte for byte, and have the web handler render that Display instead of its own copy; this makes the later widening one edit in one place.
```

```text
Every operator-visible answer of the create path stays byte-identical: the same status and the same sentence for each refusal class, the same redirect and board for an accepted key, with the bound still 2 to 6.
```

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature

##### Verification

```text
cargo test -p foundry-acceptance --test acceptance
```

#### Records

##### Decisions

```text
One bound, one owner: the key length is stated three times today, as a length check, a refusal sentence and a database CHECK, and three independent numbers cannot be kept equal by review, so the bound becomes named constants in foundry-core that the refusal sentence interpolates.
```

```text
Refusal copy keeps one home: the web handler restates the operator sentence, so it renders the ProjectKeyError Display instead and a new check-arch clause fails any second copy, mirroring the project-name rule this repository already enforces.
```

```text
Every refusal states the whole rule: the product asks every refused key to get a message stating the 2 to 10 capital letter rule, so all three refusal arms render one sentence naming 2 to 10 capital letters A-Z, while the arms stay as domain diagnosis.
```

```text
Widen the database rule by migration: migration 0001 is shipped and must never be edited, so a new migration drops the key_prefix CHECK found by its definition rather than by its name and adds a named CHECK for 2 to 10 letters.
```

```text
Keep byte length then character check: the length arm counts bytes before the A-Z arm runs, which only reorders refusals for non-ASCII input, and one shared refusal sentence makes that order invisible to the operator.
```

```text
Carriers stay with their owners: ProjectKey owns the rule and its text, ProjectKeyError owns the refusal, CreateProjectError owns the create outcome, the projects CHECK owns the stored rule, and this change adds no per-process state.
```

```text
Each layer states one rule: the domain answers a Result over a raw string, the application turns a refused key into a typed refusal and writes nothing, the HTTP adapter answers 422 and re-renders the form with the sentence, and the database admits the same set on every write including writers outside the application.
```

```text
No formal model here: no shared state, no multi-step protocol, no concurrency and no crash window is introduced, the widening is monotone so no replica interleaving can offer the database a key it refuses, and the two independent statements of the rule are compared by executed parity against a real database, which a third transcription cannot do.
```

```text
Tests belong to the oracle owner: the craft targets of this slice are the domain source, the web handler, the new migration and the gate code, and every file that observes a law is an acceptance support the acceptance designer creates before craft, so no obligation waits on a production file that does not exist yet.
```

```text
The gate is observed through its own CLI: the one-source law lives in gate code the crafter owns, so the acceptance designer observes it from outside by running cargo xtask check-arch --root over a throwaway copy of the tree carrying a planted second copy of the sentence, the form the gate already documents and the programmatic steps already use; before the clause exists that planted copy is accepted, which is the law red.
```

```text
Preservation is seeded before the widening: the migration preservation observation runs the migration set up to the one before the widening, seeds a two-letter and a six-letter project row, then applies the widening and compares those rows, so it distinguishes a key left alone from a key re-minted; seeding after the widening cannot tell the two apart.
```

##### Constraints

```text
A project key is 2 to 10 characters inclusive and every character is a capital letter A to Z; the 2-character minimum is unchanged and out of scope.
```

```text
Out of scope: renaming the key of an existing project, lower-case or digit keys, and any change to the 2-character minimum.
```

```text
crates/foundry-store/migrations/0001_init.sql is shipped and must not be edited; the database rule changes only through a new migration file.
```

```text
Every project that exists before the change keeps its key, its issue keys and its board; no row is rewritten and no key is re-minted.
```

```text
The widening is monotone: every key the 2 to 6 rule accepted the 2 to 10 rule accepts, so an old replica serving next to a new one can never offer the database a key the database refuses.
```

```text
A replica applies migrations at start-up under the existing advisory lock before it serves, so the serving code never enforces a rule wider than the database rule.
```

```text
The dev deploy at foundry.unintelligent-design.us updates from main and applies the new migration on start-up; the production deploy at foundry.jeffbailey.us is held for approval by design.
```

```text
Default test budgets apply and no derogation is requested: the real-database parity test joins the existing foundry-store integration tests, which already start one Postgres container per test file.
```

```text
Tests and gates run in the shared Lima VM local-dev through scripts/test-in-lima.sh, never on Docker on the Mac.
```

##### Reuse

ProjectKey

crates\/foundry\-core\/src\/lib\.rs\:83

Extend

```text
The value object that owns the key rule and is the single construction point; the bound moves into named constants beside it.
```

ProjectKeyError

crates\/foundry\-core\/src\/lib\.rs\:71

Extend

```text
The owner of the refusal; its Display becomes the one operator sentence stating the 2 to 10 capital letter rule.
```

key\_error\_message

crates\/foundry\-app\/src\/projects\.rs\:644

Replace

```text
A second statement of the operator sentence in the web handler; it is replaced by rendering the error Display, so no copy can drift again.
```

OneSourceRule

xtask\/src\/check\_arch\.rs\:5727

Reuse

```text
The existing one-source scanner already guards the workspace, project and team name copy; the key copy becomes one more clause, with no new scanner.
```

PROJECT\_NAME\_MAX\_CHARS

crates\/foundry\-core\/src\/project\_name\.rs\:20

Reuse

```text
The naming and shape precedent for a named cap constant in this crate; the key constants follow it.
```

create\_project

crates\/foundry\-services\/src\/projects\.rs\:293

Reuse

```text
Already parses the raw key through the one construction point and maps a refusal to a typed outcome; no change is needed.
```

insert\_project

crates\/foundry\-store\/src\/lib\.rs\:1799

Reuse

```text
The only writer of a project row; it binds the accepted value and needs no change.
```

project\_key\_db\_rule

crates\/foundry\-store\/tests\/project\_key\_db\_rule\.rs\:1

CreateNew

```text
The domain-against-database parity probe needs a real database and its own seeded parents; the existing name-rule suite keeps its probe helpers private and answers a different question.
```

project\_key\_properties

crates\/foundry\-core\/tests\/project\_key\.rs\:1

CreateNew

```text
The broad key laws need generated input; the in-file unit tests hold only fixed examples and the neighbouring tests file owns the name rule.
```

key\_prefix

crates\/foundry\-store\/migrations\/0001\_init\.sql\:57

Extend

```text
The stored rule lives in this shipped line; it is widened by a new migration because a shipped migration is never edited.
```

key\_prefix

crates\/foundry\-store\/migrations\/0035\_project\_key\_length\.sql\:1

CreateNew

```text
A new migration file is the only way to change the stored rule; no existing migration can carry it.
```

a\_fallback\_address\_is\_the\_lowest\_free\_candidate

crates\/foundry\-core\/tests\/project\_name\.rs\:379

Extend

```text
This existing property owns the key-fallback address law; its key generator widens here rather than a second property restating the law.
```

##### Impact

domain\-value\-object

crates\/foundry\-core\/src\/lib\.rs\:93

Changed

```text
The length arm moves from a literal 2 to 6 to the named constants.
```

refusal\-copy

crates\/foundry\-core\/src\/lib\.rs\:71

Changed

```text
All three arms render one sentence that states the whole rule and interpolates the constants.
```

web\-handler

crates\/foundry\-app\/src\/projects\.rs\:644

Changed

```text
The restated sentence is removed; the handler renders the error Display.
```

migration

crates\/foundry\-store\/migrations\/0035\_project\_key\_length\.sql\:1

Changed

```text
A new migration replaces the key_prefix CHECK with the widened one; no existing migration file is touched.
```

architecture\-rule

xtask\/src\/check\_arch\.rs\:6015

Changed

```text
A project-key clause joins the one-source rules so a second copy of the key sentence fails the gate.
```

domain\-unit\-test

crates\/foundry\-core\/src\/lib\.rs\:232

Changed

```text
A nine-character key with a digit now fails on the character arm instead of the length arm, and the accepted row gains a ten-letter key.
```

acceptance\-support

crates\/foundry\-core\/tests\/project\_name\.rs\:380

Changed

```text
The acceptance designer owns this test file; its fallback-address generator draws keys from the widened set so the mint is exercised with a ten-letter key.
```

acceptance\-oracle

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature\:10

Changed

```text
Its header and key-shape outline state the 2 to 6 rule; the acceptance designer owns that file and extends it for this slice.
```

acceptance\-support

crates\/foundry\-acceptance\/src\/steps\/us\_07\_project\_create\.rs\:479

Changed

```text
The outcome step observes a redirect or a refusal; the acceptance designer owns whether it also reads the refusal sentence.
```

store\-test

crates\/foundry\-store\/tests\/insert\_issue\_with\_outbox\.rs\:280

UnchangedCompatible

```text
Its per-case prefix generator stays inside the widened set, so every existing case still inserts.
```

app\-property\-test

crates\/foundry\-app\/src\/teams\.rs\:1477

UnchangedCompatible

```text
It generates two to six letter keys, which the widened rule still accepts.
```

read\-path

crates\/foundry\-services\/src\/lib\.rs\:1026

UnchangedCompatible

```text
It re-parses a stored key, and the widened rule accepts every key any earlier rule allowed to be stored.
```

read\-path

crates\/foundry\-app\/src\/keyboard\.rs\:250

UnchangedCompatible

```text
Same re-parse of a stored key; no stored key becomes unparseable.
```

form\-template

crates\/foundry\-app\/templates\/project\_create\.html\:9

UnchangedCompatible

```text
The key field declares no maxlength, so the server rule is the only bound and the longer key reaches the handler.
```

api\-description

crates\/foundry\-api\/openapi\.json\:1

UnchangedCompatible

```text
The project key appears in no described request body, parameter or schema, so the API description does not state the bound.
```

unique\-index

crates\/foundry\-store\/migrations\/0001\_init\.sql\:61

UnchangedCompatible

```text
The key stays unique per workspace; widening the shape does not change the index or its refusal.
```

lifecycle\-audit

crates\/foundry\-store\/migrations\/0025\_project\_lifecycle\_events\.sql\:35

UnchangedCompatible

```text
The audit row stores the key as plain text with no shape check, so a longer key records unchanged.
```

acceptance\-support

crates\/foundry\-core\/tests\/project\_key\.rs\:1

Changed

```text
A new file the acceptance designer creates and owns, holding the generated key laws on the pure core: the accepted set, the monotone widening and the issue-key rendering.
```

acceptance\-support

crates\/foundry\-store\/tests\/project\_key\_db\_rule\.rs\:1

Changed

```text
A new file the acceptance designer creates and owns, holding the domain-against-database parity probe and the migration preservation observation on a real Postgres.
```

acceptance\-support

crates\/foundry\-acceptance\/tests\/features\/project\-key\-length\-one\-source\.feature\:1

Changed

```text
A new feature file the acceptance designer creates, holding the contributor-port scenarios that drive the gate over a planted copy of the tree; it keeps the contributor port out of the operator story file.
```

acceptance\-support

crates\/foundry\-acceptance\/src\/steps\/feature\_project\_key\_length\.rs\:1

Changed

```text
A new step module the acceptance designer creates for the gate scenarios, following the staged-root form that crates/foundry-acceptance/src/steps/feature_a_programmatic.rs:1484 already uses.
```

acceptance\-runner

crates\/foundry\-acceptance\/tests\/acceptance\.rs\:35

Changed

```text
The runner force-links each step module, so the acceptance designer adds the new module there or its steps are stripped from the test binary and its scenarios would report undefined.
```

##### Agreement

The project key shape\: 2 to 10 capital letters A\-Z

crates\/foundry\-core\/src\/lib\.rs\:93

Producer

Migrated

```text
The one construction point states the rule and is widened in this change.
```

The project key shape\: 2 to 10 capital letters A\-Z

crates\/foundry\-store\/migrations\/0035\_project\_key\_length\.sql\:1

Consumer

Migrated

```text
The stored rule must admit exactly what the construction point admits; the new migration widens it in the same change.
```

The project key shape\: 2 to 10 capital letters A\-Z

crates\/foundry\-store\/src\/lib\.rs\:1799

Producer

UnchangedCompatible

```text
The only writer binds the text of an already accepted value, so it states no bound of its own.
```

The project key shape\: 2 to 10 capital letters A\-Z

crates\/foundry\-services\/src\/lib\.rs\:1026

Consumer

UnchangedCompatible

```text
It re-parses a stored key through the same construction point, and the widened rule accepts every stored key.
```

The project key shape\: 2 to 10 capital letters A\-Z

crates\/foundry\-app\/src\/keyboard\.rs\:250

Consumer

UnchangedCompatible

```text
Same re-parse of a stored key at the keyboard path.
```

The project key shape\: 2 to 10 capital letters A\-Z

crates\/foundry\-store\/migrations\/0001\_init\.sql\:57

Consumer

UnchangedCompatible

```text
Every key already stored under the 2 to 6 rule satisfies the widened rule, so no stored row is rewritten or refused.
```

The operator sentence that refuses a key

crates\/foundry\-core\/src\/lib\.rs\:71

Producer

Migrated

```text
The Display of the refusal becomes the one operator sentence and states the whole rule.
```

The operator sentence that refuses a key

crates\/foundry\-app\/src\/projects\.rs\:644

Consumer

Migrated

```text
The handler stops restating the sentence and renders the producer text, so the two cannot drift.
```

The operator sentence that refuses a key

crates\/foundry\-app\/src\/projects\.rs\:739

Consumer

UnchangedCompatible

```text
The already-in-use sentence answers a different refusal, uniqueness rather than shape, and stays with the handler.
```

##### Not applicable

None.

##### Boundaries

```text
The project create form: HTTP POST /team/{team_slug}/projects, with the key read from the key_prefix field
```

###### Driven ports

```text
The projects table through foundry-store insert_project
```

```text
The project board page and the issue key prefix, where the stored key is read back
```

```text
foundry-app depends on foundry-services, which depends on foundry-core; foundry-store holds the only writer of a project row; foundry-core depends on no other crate, and the database rule is a second statement of the domain rule, never a source for it.
```

###### Failures

####### B1

```text
The key is shorter than 2 characters, longer than 10, or carries a character outside A to Z
```

Refusal

```text
The form answers 422 and re-renders with the sentence stating the 2 to 10 capital letter rule, and no project row exists
```

####### B2

```text
The key is well shaped but already used by another project of the workspace
```

Refusal

```text
The form answers 409 with the already-in-use sentence, which is a different refusal and unchanged by this feature
```

####### B3

```text
The write fails the database CHECK because the migration has not been applied to this database
```

Indeterminate

```text
The create answers a server error and writes nothing; start-up ordering is what prevents this state, not the create path
```

##### Observations

```text
The answer the operator gets from the project create form
```

An HTTP status with a rendered page or an error fragment

```text
Which refusal arm fired, because every refused key renders the same sentence
```

```text
The key a created project carries, on its board header and as the prefix of its first issue key
```

The key text as rendered

```text
How the key was stored, and the order in which two projects were created
```

```text
The domain verdict on a raw key
```

Result of an accepted key or a typed refusal

```text
Where in the string the offending character sits
```

```text
The database verdict on a key_prefix write
```

The write is accepted\, or it fails the check

```text
Which writer attempted the write
```

##### Equality

ProjectKey

```text
Two keys are equal when their text is equal; an accepted key is the input text unchanged, so the value, its Display and the stored column agree.
```

A refused key

```text
Two refused keys are equal for the operator when they render the same sentence, and all refusals now do; in the domain they stay distinguishable by their refusal arm.
```

##### Constructors

ProjectKey\:\:try\_new

fn try\_new\(raw\: \&str\) \-\> Result\<ProjectKey\, ProjectKeyError\>

IssueKey\:\:try\_new

fn try\_new\(project\_key\: \&ProjectKey\, number\: u32\) \-\> Result\<IssueKey\, IssueKeyError\>

##### Laws

###### L1

```text
A raw key is accepted exactly when it is 2 to 10 characters long and every character is a capital letter A to Z.
```

Invariant

Domain

crates\/foundry\-core\/src\/lib\.rs

###### Treatment

Broad

###### Generator

```text
Strings assembled from capital letters, lower-case letters, digits, punctuation, space and non-ASCII scalars, of length 0 to 14
```

14 characters\, 256 cases

The repository proptest runner with its default seeding\; a failure prints the seed that replays it

###### Must include

```text
AU
```

```text
AUTHWS
```

```text
PLATFORMOP
```

```text
PLATFORMOPS
```

```text
A
```

```text
the empty string
```

```text
auth
```

```text
AUTH-X
```

```text
AU1
```

```text
a four-character non-ASCII string longer than ten bytes
```

Flat strings\, no nesting

```text
proptest shrinking to the shortest failing string, replayed from the printed seed
```

```text
The expected verdict is computed in the test from the stated rule, by counting characters and classifying each one, never by calling the constructor under test
```

```text
An eleven-letter capital key is accepted, or a ten-letter capital key is refused
```

###### Evidence

None.

###### L2

```text
Every key the earlier 2 to 6 rule accepted the new rule still accepts.
```

Invariant

Domain

crates\/foundry\-core\/src\/lib\.rs

###### Treatment

Broad

###### Generator

```text
Strings of capital letters A to Z of length 2 to 6
```

6 characters\, 128 cases

The repository proptest runner with its default seeding

###### Must include

```text
AU
```

```text
AUTHWS
```

Flat strings\, no nesting

```text
proptest shrinking to the shortest failing string
```

```text
The earlier rule is restated in the test as length 2 to 6 of capital letters, independent of the constructor
```

```text
A two to six letter capital key the new rule refuses
```

###### Evidence

None.

###### L3

```text
The database admits a key_prefix write exactly when the domain rule accepts the same text.
```

Invariant

Infrastructure

crates\/foundry\-store\/migrations\/0035\_project\_key\_length\.sql

###### Treatment

Broad

###### Generator

```text
The same strings as the domain law, excluding the null character which a text column cannot hold
```

14 characters\, 64 cases\, one database container for the file

The repository proptest runner with its default seeding\, cases drawn once and replayed from the printed seed

###### Must include

```text
AU
```

```text
PLATFORMOP
```

```text
PLATFORMOPS
```

```text
auth
```

```text
AU1
```

```text
the empty string
```

Flat strings\, no nesting

```text
proptest shrinking to the shortest disagreeing string
```

```text
The domain verdict and the real insert outcome are read separately for the same text and then compared; neither is derived from the other
```

```text
A key the domain accepts that the write refuses, or a key the domain refuses that the write stores
```

###### Evidence

None.

###### L4

```text
An issue key renders the project key verbatim, then a hyphen, then the issue number.
```

Algebraic

Domain

crates\/foundry\-core\/src\/lib\.rs

###### Treatment

Broad

###### Generator

```text
Accepted keys of 2 to 10 capital letters paired with issue numbers 1 to 1000
```

10 characters and 1000\, 128 cases

The repository proptest runner with its default seeding

###### Must include

```text
PLATFORMOP with number 1
```

One key and one number per case

```text
proptest shrinking to the shortest key and smallest number
```

```text
The expected string is assembled in the test from the key text and the number, not from the rendering under test
```

```text
A ten-letter prefix truncated, padded or case-folded in the rendered issue key
```

###### Evidence

None.

###### L5

```text
A refused key leaves no project created and answers one sentence that states the 2 to 10 capital letter rule.
```

Invariant

Adapter

crates\/foundry\-app\/src\/projects\.rs

###### Treatment

Finite

###### Cases

```text
An eleven-letter capital key
```

```text
A one-letter key
```

```text
An empty key
```

```text
A lower-case key
```

```text
A key carrying a digit
```

```text
Five named refusal classes meet the create form; each is one example at the HTTP boundary, and the class set is closed by the rule itself.
```

###### Evidence

None.

###### L6

```text
Applying the new migration leaves every existing project row unchanged and its key still accepted.
```

Invariant

Infrastructure

crates\/foundry\-store\/migrations\/0035\_project\_key\_length\.sql

###### Treatment

Finite

###### Cases

```text
A row with a two-letter key, the shortest the earlier rule allowed
```

```text
A row with a six-letter key, the longest the earlier rule allowed
```

```text
The pre-change set is bounded by its two length ends; a row at each end witnesses that the widening rewrites nothing.
```

###### Evidence

None.

###### L7

```text
The operator sentence that refuses a key has exactly one home in the workspace.
```

Invariant

Application

xtask\/src\/check\_arch\.rs

###### Treatment

Finite

###### Cases

```text
A second copy of the sentence placed in the web crate source
```

```text
A second copy placed in the services crate source
```

```text
A second copy placed in the store crate source
```

```text
The gate decides over every file of the four closed crates, so the law is witnessed by one injected copy per crate that may not hold it.
```

###### Evidence

None.

##### Refuted

```text
Three independent statements of the same bound can be kept equal by review.
```

```text
No owner holds the bound, so each statement is edited on its own; the operator sentence has already drifted from the domain sentence in capitalisation.
```

```text
crates/foundry-app/src/projects.rs:647 states Project key must be 2-6 characters while crates/foundry-core/src/lib.rs:74 states project key must be 2-6 characters.
```

```text
A refused key is best answered by a sentence naming only its own arm.
```

```text
The product asks every refused key to be answered with the whole rule, and an arm-specific sentence leaves two of the three arms without the length rule.
```

```text
The key auth is refused today with Project key must contain only uppercase A-Z characters, which never states the length rule.
```

##### Claims

###### C1

```text
A ProjectKey value can hold only 2 to 10 capital letters A to Z.
```

```text
An out-of-shape key reaches the write and either fails it with a server error or gives every issue of that project a prefix no operator can read.
```

crates\/foundry\-core\/src\/lib\.rs

```text
Any string from the create form key field after the handler trims it
```

ProjectKey\:\:try\_new

foundry\_services\:\:projects\:\:create\_project\, then foundry\_store insert\_project

```text
The key on the project board header and the prefix of each issue key
```

```text
The inner field is private and the crate exposes no raw constructor, setter or deserializer, so Rust blocks the invalid value on typed paths; foundry-store reads key_prefix back as plain text, so a row written by a database client or restored from an older backup is re-parsed at the read path and the widened CHECK is the only guard on that write.
```

##### States

Admitted

```text
A stored key of 2 to 10 capital letters A-Z
```

crates\/foundry\-core\/src\/lib\.rs

Admitted

```text
A stored key of 2 to 6 capital letters, created before this change, with its issue keys unchanged
```

crates\/foundry\-store\/src\/lib\.rs

Excluded

```text
A new key of 11 or more characters, of one character, empty, or carrying any character outside A-Z
```

crates\/foundry\-core\/src\/lib\.rs

Excluded

```text
A project created while the serving code enforces a rule wider than the database rule
```

crates\/foundry\-store\/src\/lib\.rs

Unknown

```text
A key_prefix written straight into the database by an operator with a database client, or restored from a backup taken before this change
```

crates\/foundry\-store\/src\/lib\.rs

##### Models

None.

##### Closure

```text
A project key of 2 to 10 capital letters
```

Safety

Pass

DesignedNotBuilt

```text
The constructor is the only public way to build the value, the inner field is private and the crate exposes no raw constructor or deserializer, so Rust blocks an out-of-shape key on typed paths; the widened CHECK refuses the same set on every write, including writers that never pass the typed path. No byte of this change exists yet, so nothing is observed.
```

###### Evidence

None.

```text
A project key of 2 to 10 capital letters
```

Inhabitation

Pass

DesignedNotBuilt

```text
PLATFORMOP is admitted input of the constructor, which yields the value, which create_project hands to insert_project, and the widened CHECK admits it. No byte of this change exists yet.
```

###### Evidence

None.

```text
A project key of 2 to 10 capital letters
```

PublicReachability

Pass

DesignedNotBuilt

```text
The path is the key field of the create form, then POST /team/{team_slug}/projects, then create_project, then ProjectKey::try_new, then insert_project, then the board header and the prefix of the first issue key. Every step names an existing symbol; only the widened bound is not built.
```

###### Evidence

None.

```text
A project key of 2 to 10 capital letters
```

Liveness

Pass

DesignedNotBuilt

```text
No liveness claim: the create is one synchronous request, its refusal is immediate, and no step waits on another process, a timer, a queue or a remote party. Bounded synchronous execution carries this axis, for liveness only; the other three axes rest on the construction argument above, not on it.
```

###### Evidence

None.

##### Risks

###### Y1

Stressor

```text
Negate the assumption that the application rule and the database rule change together: a build ships the widened domain rule while the new migration has not been applied to that database.
```

```text
Every 2 to 6 letter key still creates a project, and every existing project still opens.
```

```text
A 7 to 10 letter create answers a server error from the write instead of the form sentence.
```

```text
A stored key that the database rule refuses.
```

```text
The create path and every read path keep working for the keys the earlier rule allowed.
```

crates\/foundry\-store\/src\/lib\.rs run\_migrations at start\-up

###### Y2

Stressor

```text
Negate the assumption that all replicas run one version: during a rolling upgrade an old replica enforcing 2 to 6 serves next to a new one enforcing 2 to 10.
```

```text
Existing projects, and every create of a 2 to 6 letter key, on either replica.
```

```text
A 7 to 10 letter create is refused with the rule sentence by whichever old replica answers, until the rollout finishes.
```

```text
A stored key that either rule refuses.
```

```text
The widening is monotone, so no interleaving offers the database a key it refuses.
```

the deploy owner

###### Y3

SinglePointOfFailure

```text
The widened CHECK is the only guard for a writer that does not pass the domain constructor.
```

```text
The stored rule on every write path, including a database client and a restore.
```

```text
A direct writer is refused by the check and sees a check violation.
```

```text
An out-of-shape key stored without any refusal.
```

```text
The parity test reads the database verdict for itself rather than trusting the domain.
```

crates\/foundry\-store

###### Y4

Bypass

```text
foundry-store reads key_prefix back as plain text, so a row written by a database client or restored from an older backup is re-parsed at the read path.
```

```text
The read path refuses to build a key it does not accept, and no unparsed key reaches a template.
```

```text
The affected project reports an error on its keyboard and issue-key paths rather than rendering a bad key.
```

```text
An unchecked key text rendered as a key.
```

```text
The widened rule accepts every key the earlier rule accepted, so no older backup becomes newly unreadable.
```

crates\/foundry\-app\/src\/keyboard\.rs and crates\/foundry\-services\/src\/lib\.rs

###### Y5

Stressor

```text
Second set, walked over the finished design: negate the assumption that the constraint swap is instant, because a long transaction holds the projects table while the migration waits for its lock.
```

```text
The earlier rule stays in force until the swap completes.
```

```text
Start-up blocks on the lock and the replica does not serve while it waits.
```

```text
Serving while the constraint swap is half applied.
```

```text
The swap is one drop and one add in one transaction, so an interrupted migration leaves the earlier constraint in place.
```

the deploy owner

###### Y6

Stressor

```text
Second set: negate the assumption that no two operators want the same longer key, as two creates race on one new ten-letter key.
```

```text
The unique key per workspace, which refuses the second create.
```

```text
The second create answers 409 with the already-in-use sentence.
```

```text
Two projects of one workspace holding one key.
```

```text
The unique index is untouched by this feature and decides the race.
```

crates\/foundry\-store

###### Y7

Stressor

```text
Second set: negate the assumption that a longer key still reads well, as a ten-letter key renders in the board header, the dashboard card and every issue key.
```

```text
The key renders verbatim everywhere it appears.
```

```text
A dashboard card key may wrap or clip where the stylesheet gives it a narrow box.
```

```text
A truncated, padded or case-folded key in an issue key.
```

```text
No template states a key length and none truncates, so only the card box is a layout question.
```

crates\/foundry\-app\/templates

##### Architectural tests

###### T1

```text
The operator sentence that refuses a key appears in no source file of the web, services, api and store crates, over every file those crates hold now or gain later; its one home is the refusal Display in foundry-core. The acceptance designer observes it through the real gate CLI over a throwaway copy of the tree, the form the gate already documents: cargo xtask check-arch --root COPY exits non-zero and names the file and line of a planted second copy of the sentence, while the same gate over the unplanted copy exits zero. Before the gate clause is built, the planted copy is accepted, which is the red this obligation starts from.
```

crates\/foundry\-acceptance\/tests\/features\/project\-key\-length\-one\-source\.feature

```text
cargo test -p foundry-acceptance --test acceptance
```

##### Contract tests

###### T2

```text
The database admits exactly the keys the domain rule accepts, arm for arm, so the stored rule is never stricter and never looser than the rule the application enforces. The acceptance designer owns the probe file; it needs no production code that does not exist, because it reads the domain verdict from the existing construction point and the database verdict from a real insert.
```

The project key shape\: 2 to 10 capital letters A\-Z

crates\/foundry\-core\/src\/lib\.rs\:93

crates\/foundry\-store\/migrations\/0035\_project\_key\_length\.sql\:1

```text
For each probe key the domain verdict and the outcome of a real insert into a real Postgres agree
```

crates\/foundry\-store\/tests\/project\_key\_db\_rule\.rs

```text
cargo test -p foundry-store --test project_key_db_rule
```

## ADRs

- 001 The project key bound and its sentence have one owner — Accepted
