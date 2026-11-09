# Architecture brief: iis\-setup\-links

## Paradigm

object_oriented

## Shared records

### Decisions

#### D1

```text
Carrier replacement: ConnectionWords.fix (Option<FixLink>) becomes ConnectionWords.fixes (Vec<FixLink>), an ordered offer list owned by crates/foundry-app/src/integrations/no_connection.rs. One Option cannot represent the existing link followed by I-19, so the carrier changes rather than a second link field or a template-side precedence.
```

#### D2

```text
New value object Viewer { is_workspace_admin, is_instance_admin } in no_connection.rs replaces the positional bool on NoConnectionCause::words, SharedConnection::line and project_lines. foundry-app builds it from the services fact (RouteTarget.viewer_is_workspace_admin, ProjectIntegrations.viewer_is_workspace_admin) and the app fact (NavContext.is_instance_admin); foundry-services never learns instance-admin, so ADR-IW-001's rejected alternative stands.
```

#### D3

```text
NoConnectionCause::words(Viewer) is the single owner of the W-2, W-3, W-4, K-4 and K-19 words and of the links offered with them. The workspace integrations page stops deciding its W-2 Slack, W-3 and W-2 Matrix paragraph in workspace.html from booleans plus a hardcoded setup link and renders the same ConnectionWords; its K-19 and Add-to-Slack arms keep their page-specific actions and stay outside this change.
```

#### D4

```text
I-19 is appended only for the causes NotSetUp(Slack) (W-2 Slack), NotSetUp(Matrix) (W-2 Matrix) and NoSecretsKey(Slack) (W-3), and only when the viewer is an instance admin. Unreadable (K-19), NotConnected (W-4, K-4) and the project page's W-6 and W-7 lines gain no link. Its href is the existing INSTANCE_PATH constant, a relative path, so a standalone deploy needs no setting.
```

#### D5

```text
Layers and carriers. Domain: NoConnectionCause (choice type) with states NotSetUp, NoSecretsKey, Unreadable, NotConnected per ChannelKind; operation words(Viewer); observation ConnectionWords (words plus ordered fixes); no decision, effect, error or per-process state carrier is added. Application: the three doors (workspace page, route form GET, routes preview and save posts) read the instance snapshot through state.integrations.current() and the viewer from NavContext, and render ConnectionWords; the outcome is a rendered page, not a refusal type. Adapter: Askama templates workspace.html, route_form.html, project.html render fixes in order; GET /admin/instance/integrations is guarded by require_instance_admin, which answers the uniform not-found page otherwise. Infrastructure: unchanged; nothing is stored, queued or recovered by this change.
```

#### D6

```text
Liveness is not applicable: one synchronous render answers one request, the design adds no queue, retry, lease, replica handoff or progress obligation, and no promise names eventual progress. The only temporal fact in reach, that every replica picks up a saved Slack app without a restart, is Deliverable A's shipped reload seam and is unchanged here.
```

#### D7

```text
P6 needs no edit: RELEASING.md R-1 and R-2 (lines 404-425) and deploy/k8s/README.md (lines 97-135) already describe an instance admin generating the secrets key and entering the Slack app at /admin/instance/integrations with no restart, and name the SLACK_* trio as the optional operator alternative. Deliverable A reworded them; this Request confirms them unchanged rather than rewriting shipped prose.
```

#### D8

```text
Claim-to-checker mapping. Every claim in this design is local, pure and finite, and each is covered by the checker that actually decides it. Totality and the exclusion of an unopenable or misordered offer: the repository's own checker, rustc, over the closed NoConnectionCause, SlackOffer and MatrixOffer enums, with words the only writer of fixes, so no other arm can add an href. The whole relation: exhaustive enumeration of all 112 (cause, connected, Viewer) rows in the module's finite table, which is complete rather than sampled. The link-to-guard agreement: the public oracle follows each offered link off the rendered page and observes its status, which is the real guarantor; a model of that agreement could only restate the assumption about require_instance_admin. No state-transition, temporal, fairness, concurrency or recovery claim exists, so no state model is selected: Quint, TLA+ and Agda would add a second encoding of the same finite table and no checking power. Observed on this host for the record, not as the reason: quint, agda and apalache-mc are absent and the java launcher is misconfigured (jenv points at an uninstalled 26 while Corretto 27 is installed); node and npm are present. The one gap no checker closes is a template writing an href literally instead of rendering fixes, recorded as a Bypass risk with its owner.
```

### Constraints

#### K1

```text
Locked copy, verbatim. I-19 label: Set up integrations for this Foundry, href /admin/instance/integrations, shown after any existing link. I-20 step 3: An instance admin enters the app's Client ID, Client Secret and Signing Secret under Instance admin > Integrations. I-20 step 4: No restart is needed. A workspace admin can then choose Add to Slack on the workspace integrations page. I-20 footnote: An instance operator may instead set SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET in Foundry's environment and restart; the environment takes precedence.
```

#### K2

```text
Unchanged bytes. The shipped constants SLACK_NOT_SET_UP, MATRIX_NOT_SET_UP, SLACK_NO_SECRETS_KEY, HOW_TO_SET_UP_SLACK, NO_KIND_CONNECTION, CONNECT_KIND, NO_CHAT_CONNECTION, the K-19 words from unavailable_words, and the Slack app manifest keep their current text. No step of the Slack setup guide may say Restart Foundry.
```

#### K3

```text
Closed finite input domain, no stored input. The whole domain is SlackOffer (NotSetUp, Unavailable, NoSecretsKey, Ready) or MatrixOffer (Unavailable, NoSecretsKey, Ready), times a connected flag, times Viewer's two booleans: 28 reachable (cause, connected) rows times 4 viewers. Authority states no numeric or length bound because the change stores, validates and accepts no user input; it adds no field, query, parameter or persisted value. Any design that widened this domain would be a deviation.
```

#### K4

```text
No added query: the instance-admin fact is read from the NavContext every one of these pages already builds (nav.rs:35, resolved by resolve_is_instance_admin and failing closed to false on a store error).
```

#### K5

```text
Observation bound for the oracle: the offered-links observation must read every page link on the rendered body (sidebar excluded), including hrefs under /admin/instance/integrations. The shipped helper workspace_links in crates/foundry-acceptance/src/support/integrations_wayfinding.rs:202 keeps only hrefs starting with /workspace/integrations, so the existing no-way-to-fix-it-here assertion cannot falsify an I-19 shown to Lena or Tomas. Widening that observation is a precondition of P3 being falsifiable.
```

### Reuse

NoConnectionCause

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:75

Extend

```text
It already owns the closed cause set and its words; the amendment to ADR-IW-001 widens its words function's input rather than adding a second owner. The cause set and its precedence are unchanged.
```

ConnectionWords

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:51

Extend

```text
It is the one observation every door renders; its fix field is replaced by the ordered fixes list. A second struct beside it would give the same page two places to look.
```

FixLink

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:58

Reuse

```text
A label plus a static href is exactly what I-19 is; it needs no new field, and FixLink::new already builds it.
```

Viewer

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:113

CreateNew

```text
No existing type carries both viewer facts: RouteTarget and ProjectIntegrations carry only viewer_is_workspace_admin (foundry-services, which cannot know instance-admin) and NavContext carries is_instance_admin plus nine unrelated page fields. Passing two bare bools to words, line and project_lines would be an agreement on argument position between five call sites, so the two facts get one named owner in the module that decides on them.
```

INSTANCE\_PATH

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:50

Reuse

```text
The target of I-19 is the shipped constant for /admin/instance/integrations; writing the path again in no_connection or a template would be a second source for the same address.
```

require\_instance\_admin

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs\:409

Reuse

```text
It is what makes the offered link answer 200 to the viewer it was offered to and the uniform not-found page to everyone else; the design relies on it rather than re-checking rights at the link.
```

project\_lines

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:190

Extend

```text
It returns ConnectionWords, so the carrier change reaches it; its viewer parameter becomes the Viewer value object for one viewer representation in the module. Its lines gain no I-19.
```

WorkspacePage

crates\/foundry\-app\/src\/integrations\/handlers\/workspace\.rs\:25

Extend

```text
Its four W-2 and W-3 string fields and two booleans are replaced by the rendered ConnectionWords for the Slack and Matrix sections, so the page stops deciding words and links itself. A new page struct would duplicate the eleven unrelated fields it already carries.
```

unavailable\_words

crates\/foundry\-app\/src\/integrations\/handlers\/mod\.rs\:277

Reuse

```text
K-19 keeps its single source for every door; the design does not touch it.
```

is\_instance\_admin

crates\/foundry\-app\/src\/nav\.rs\:35

Reuse

```text
The fact is already resolved on every one of these pages, fail-closed, so the link choice adds no query. No new resolver is needed.
```

### Impact

module\, the decision owner

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:51

Changed

```text
Carrier replacement (fixes), the new Viewer type, the widened words, line and project_lines signatures, and the finite table test extended with the instance-admin dimension.
```

public interface of the crate module

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:113

Migrated

```text
words(bool) becomes words(Viewer) and ConnectionWords.fix becomes fixes; both are pub within foundry-app and have three in-crate call sites, all migrated in this change. CODING_STANDARDS forbids a compatibility shim, so the old shape is deleted.
```

handler and its template\, consumer

crates\/foundry\-app\/src\/integrations\/handlers\/route\_form\.rs\:431

Changed

```text
Builds the Viewer from target.viewer_is_workspace_admin and nav.is_instance_admin, which the page function already holds, and the template loops over fixes. This covers the form GET and the preview and save posts, which all render through this one page builder.
```

handler and its template\, consumer

crates\/foundry\-app\/src\/integrations\/handlers\/routes\.rs\:733

Changed

```text
project_lines takes the Viewer, built where nav is already created at routes.rs:729, and project.html loops over fixes. The project page's words and links are unchanged.
```

handler\, consumer of the words only

crates\/foundry\-app\/src\/integrations\/handlers\/matrix\_setup\.rs\:285

Changed

```text
The Send-test refusal reads only ConnectionWords.words, so it must pass a Viewer. It takes the same two facts the refused page already resolves; its words do not depend on the instance-admin fact, and it offers no link.
```

handler and its template\, the workspace page

crates\/foundry\-app\/src\/integrations\/handlers\/workspace\.rs\:262

Changed

```text
The W-2 Slack, W-3 and W-2 Matrix arms render ConnectionWords from the same cause function; the hardcoded setup anchor in workspace.html:14 is removed in favour of the offer list.
```

upstream crate\, unchanged

crates\/foundry\-services\/src\/integrations\/mod\.rs\:299

UnchangedCompatible

```text
RouteTarget and ProjectIntegrations keep viewer_is_workspace_admin and gain nothing: the instance-admin fact stays in foundry-app, so no services signature, query or row changes.
```

test and oracle support\, consumer

crates\/foundry\-acceptance\/src\/support\/integrations\_wayfinding\.rs\:202

Changed

```text
then_fix_offered and workspace_links must observe an ordered list of page links over every href, not one link under the /workspace/integrations prefix, or P3 cannot fail when I-19 leaks. Owned by the acceptance designer.
```

build and package metadata

crates\/foundry\-app\/Cargo\.toml\:1

UnchangedCompatible

```text
No dependency, feature, binary or template-path change: the templates already exist and Askama discovers them by path.
```

operator documentation

RELEASING\.md\:404

UnchangedCompatible

```text
R-1 and R-2 already describe an instance admin generating the secrets key and entering the Slack app at /admin/instance/integrations with no restart, and name the SLACK_* trio as the optional alternative that wins while set. P6 is already satisfied by the shipped text.
```

operator documentation

deploy\/k8s\/README\.md\:97

UnchangedCompatible

```text
Lines 97-135 already send the operator to RELEASING.md R-1 and R-2 for browser setup and describe the SLACK_* trio as optional settings; nothing in it tells anyone to restart to set up Slack.
```

### Agreement

ConnectionWords\, the shape the no\-connection and connection\-line rendering agree on \(words plus the ordered fixes list\)

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:51

Producer

Migrated

```text
The producer replaces the single optional fix with the ordered list; it is the only writer of the shape.
```

ConnectionWords\, the shape the no\-connection and connection\-line rendering agree on \(words plus the ordered fixes list\)

crates\/foundry\-app\/templates\/integrations\/route\_form\.html\:20

Consumer

Migrated

```text
Its if-let over one fix becomes a loop over fixes in order; same words, same single link for every shipped row.
```

ConnectionWords\, the shape the no\-connection and connection\-line rendering agree on \(words plus the ordered fixes list\)

crates\/foundry\-app\/templates\/integrations\/project\.html\:6

Consumer

Migrated

```text
Same loop change; the project page's lines and links are unchanged by it.
```

ConnectionWords\, the shape the no\-connection and connection\-line rendering agree on \(words plus the ordered fixes list\)

crates\/foundry\-app\/templates\/integrations\/workspace\.html\:13

Consumer

Migrated

```text
A new consumer: its W-2 and W-3 arms stop reading four separate word fields plus a literal anchor and render the words and the fixes loop instead.
```

ConnectionWords\, the shape the no\-connection and connection\-line rendering agree on \(words plus the ordered fixes list\)

crates\/foundry\-app\/src\/integrations\/handlers\/matrix\_setup\.rs\:285

Consumer

Migrated

```text
Consumes the words only; migrated because the producer now needs a Viewer, and it offers no link.
```

The viewer facts a page decides link offers from\: workspace admin from foundry\-services\, instance admin from foundry\-app authorization

crates\/foundry\-services\/src\/integrations\/mod\.rs\:312

Producer

UnchangedCompatible

```text
Keeps producing viewer_is_workspace_admin only; it is not asked for the instance-admin fact, so the crate boundary holds.
```

The viewer facts a page decides link offers from\: workspace admin from foundry\-services\, instance admin from foundry\-app authorization

crates\/foundry\-app\/src\/nav\.rs\:236

Producer

UnchangedCompatible

```text
resolve_is_instance_admin already answers this fact for every page, fail-closed; it gains no caller it did not have, because NavContext is built on all three doors already.
```

The viewer facts a page decides link offers from\: workspace admin from foundry\-services\, instance admin from foundry\-app authorization

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs\:113

Consumer

Migrated

```text
The single consumer of both facts: it combines them into Viewer and is the only place that decides what the combination offers.
```

### Not applicable

contract_tests

```text
No party can change apart from another. Every producer and consumer of ConnectionWords and of the viewer facts lives in the foundry-app crate of one deployable and changes in this one commit; the shape is built per request and never persisted, serialized or sent on a wire, so no data outlives its producer's version. The primary oracle drives the real producer through the real HTTP ports and covers the agreement.
```

### Boundaries

```text
HTTP, the production axum router in process: GET /workspace/integrations, GET /team/{team}/project/{project}/integrations/routes/new, POST /team/{team}/project/{project}/integrations/routes/preview, POST /team/{team}/project/{project}/integrations/routes, GET /workspace/integrations/slack/setup, and GET /admin/instance/integrations as the followed link's target
```

#### Driven ports

```text
Integrations snapshot read through state.integrations.current() (slack_offer, matrix_offer, opens, sealing_box)
```

```text
foundry-services workspace and project integrations reads, which supply viewer_is_workspace_admin and the stored connections
```

```text
the instance-admin authorization read behind NavContext (resolve_is_instance_admin) and require_instance_admin
```

```text
Inward only: templates depend on no_connection, handlers depend on no_connection and on foundry-services and the Integrations snapshot, and no_connection depends on neither handlers' request state nor foundry-services. The instance-admin fact enters at the foundry-app handler and is never pushed down into foundry-services or foundry-store.
```

#### Failures

##### B1

```text
The viewer's instance-admin fact cannot be resolved (store error in resolve_is_instance_admin)
```

Indeterminate

```text
The page renders with W-2 or W-3, the existing link if any, and no I-19; an error line is logged. The viewer cannot tell a store outage from not being an instance admin.
```

##### B2

```text
Someone who is not an instance admin requests /admin/instance/integrations directly, by hand or from a stale page
```

Refusal

```text
The uniform not-found page, from require_instance_admin; no hint that the resource exists.
```

### Observations

```text
The rendered page's own links, sidebar excluded, in document order, as (text, href) pairs
```

Vec\<\(String\, String\)\>

```text
Which paragraph or section a link sits in, its styling, and whether one code path or another produced it; sidebar navigation is deliberately outside it
```

```text
The visible text of the page, used to read a W-2, W-3, K-4, K-19 or I-20 line and to assert an absent phrase such as Restart Foundry
```

String

```text
Word order inside one sentence is distinguished, but not which constant or template arm produced the sentence, nor a duplicated identical line
```

```text
The answer to following an offered link: its status and the path it lands on
```

\(StatusCode\, String\)

```text
Why a uniform not-found page was returned: a missing route, a failed guard and a withheld resource are one observation
```

### Equality

ConnectionWords

```text
Two ConnectionWords are equal when their words strings are equal and their fixes lists are equal element-wise and in order, each element compared on (label, href). Order is significant: the existing link before I-19 is a different value from I-19 before it.
```

Viewer

```text
Two Viewers are equal when both booleans agree. The four viewer values are: workspace admin and instance admin, workspace admin only, instance admin only, neither. The third is unreachable on GET /workspace/integrations, which requires workspace admin, and reachable on the route form.
```

### Constructors

Viewer\:\:new

fn new\(is\_workspace\_admin\: bool\, is\_instance\_admin\: bool\) \-\> Viewer

NoConnectionCause\:\:words

fn words\(self\, viewer\: Viewer\) \-\> ConnectionWords

project\_lines

fn project\_lines\(connections\: \&\[\(ChannelKind\, SharedConnection\)\]\, viewer\: Viewer\) \-\> Vec\<ConnectionWords\>

### Laws

#### L1

```text
For every (cause, Viewer), every link in ConnectionWords.fixes is openable by that viewer: an href of /admin/instance/integrations appears only when viewer.is_instance_admin, and an href of /workspace/integrations or /workspace/integrations/slack/setup only when viewer.is_workspace_admin.
```

Invariant

Domain

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

##### Treatment

Finite

###### Cases

```text
All 28 (cause, connected) rows of the shipped offer table, each against the four Viewer values, asserting the exact ordered (label, href) list
```

```text
Viewer instance-admin only on a route form: W-2 Slack yields I-19 alone, with no How to set up Slack
```

```text
Viewer neither: W-2 Slack yields an empty fixes list
```

```text
The domain is closed and finite (28 x 4 = 112 rows); the shipped table test already enumerates it exhaustively, so generated sampling would add no coverage and no shrinking value.
```

##### Evidence

None.

#### L2

```text
When I-19 is offered it is the last element of fixes, and every link that ships today keeps its position before it.
```

Invariant

Domain

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

##### Treatment

Finite

###### Cases

```text
W-2 Slack, workspace admin and instance admin: fixes is exactly [How to set up Slack -> /workspace/integrations/slack/setup, Set up integrations for this Foundry -> /admin/instance/integrations]
```

```text
W-2 Matrix and W-3, instance admin: fixes is exactly [Set up integrations for this Foundry]
```

```text
Three ordered cases exhaust every row where two links can coexist; no other cause offers more than one link.
```

##### Evidence

None.

#### L3

```text
Preservation under the carrier replacement: for every (cause, connected) row and every Viewer whose is_instance_admin is false, fixes equals the shipped fix as a list of zero or one element, and words is byte-identical to the shipped constant.
```

Refinement

Domain

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

##### Treatment

Finite

###### Cases

```text
The 28 shipped offer-table rows times the two non-instance-admin viewers, compared against the shipped expectations unchanged
```

```text
The 15 shipped project_lines rows (W-6, W-7, K-4, K-10, K-19) times the two non-instance-admin viewers
```

```text
This is the refutation of the old carrier re-run on the new one: the same finite table, same expected values, so the preservation claim is decided by enumeration rather than sampled.
```

##### Evidence

None.

#### L4

```text
The route form GET, the preview post and the save post of one kind render the same ConnectionWords for the same (offer, connected, Viewer); the workspace integrations page renders the same words and the same links for the same state and viewer.
```

Invariant

Application

crates\/foundry\-app\/src\/integrations\/handlers\/route\_form\.rs

##### Treatment

Finite

###### Cases

```text
Priya, no Slack app: GET routes/new?kind=slack, POST routes/preview and POST routes each read W-2 Slack with How to set up Slack then I-19
```

```text
Priya, no secrets key: the same three doors for kind=matrix each read W-2 Matrix with I-19
```

```text
Priya on GET /workspace/integrations with no Slack app reads the same W-2 Slack line and the same two links in the same order as her route form
```

```text
Three doors times two kinds is finite and each is a distinct public stimulus; the agreement is asserted by driving each door, not by sampling inputs.
```

##### Evidence

None.

#### L5

```text
I-19 is offered for exactly the causes NotSetUp(Slack), NotSetUp(Matrix) and NoSecretsKey(Slack), and never for Unreadable, NotConnected or any project-page connection line.
```

Invariant

Domain

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

##### Treatment

Finite

###### Cases

```text
Unreadable(Slack) and Unreadable(Matrix) for an instance admin: fixes is empty, K-19 words unchanged
```

```text
NotConnected(Slack) and NotConnected(Matrix) for an instance admin who is a workspace admin: fixes is exactly [Connect <kind> in workspace integrations]
```

```text
project_lines W-6, W-7 and K-4 for an instance admin: the shipped lines and links, with no I-19
```

```text
The cause set is a closed four-variant enum over two kinds; every variant is listed, so enumeration is complete.
```

##### Evidence

None.

#### L6

```text
GET /admin/instance/integrations answers 200 to an instance admin and the uniform not-found page to every other viewer, so following I-19 never lands on not-found for the viewer it was offered to.
```

Invariant

Adapter

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs

##### Treatment

Finite

###### Cases

```text
Priya (instance admin) follows the I-19 href read off the page and gets 200 at /admin/instance/integrations
```

```text
Tomas and Lena request the same path directly and get the uniform not-found page
```

```text
Two outcomes of one shipped guard; the finite check follows the real link off the rendered page rather than asserting the guard in isolation.
```

##### Evidence

None.

### Refuted

```text
One Option<FixLink> is enough for every no-connection refusal: each cause offers at most one way out.
```

```text
Carrier: Option cannot hold an ordered pair, so the law held only while a single viewer dimension existed. Adding the instance-admin dimension makes two simultaneous offers reachable, and no law or guard over the unchanged carrier can express their order.
```

```text
W-2 Slack read by a viewer who is both a workspace admin and this Foundry's instance admin: the page owes How to set up Slack and then Set up integrations for this Foundry, in that order.
```

```text
The instance-admin fact can ride along with the workspace-admin fact on RouteTarget, so one services-supplied struct carries the whole viewer.
```

```text
Boundary: foundry-services sees stored connections and workspace access only; whether a user is this Foundry's instance admin is foundry-app authorization state, and ADR-IW-001 already rejected moving that knowledge down. Adding it to RouteTarget would add a forbidden dependency edge.
```

```text
A route form opened by Priya: RouteTarget.viewer_is_workspace_admin is true from services, but is_instance_admin can only be answered by resolve_is_instance_admin in foundry-app, which the services layer cannot call.
```

### Claims

#### C1

```text
A viewer is never offered a link they cannot open.
```

```text
A workspace admin who is not an instance admin, or a team lead, follows Set up integrations for this Foundry into the uniform not-found page: the page lies about what she can fix, contradicting P3 and KPI-IW-G1, and the dead end teaches her to distrust every other offer on the page.
```

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

```text
A NoConnectionCause built by of_slack or of_matrix from the instance snapshot, and a Viewer built from the services workspace-admin fact and NavContext.is_instance_admin
```

NoConnectionCause\:\:words\(Viewer\)

crates\/foundry\-app\/templates\/integrations\/workspace\.html\, route\_form\.html and project\.html\, each looping over ConnectionWords\.fixes

```text
Reading every page link off the rendered body and following each one: it answers 200, never the uniform not-found page
```

```text
An Askama template that hardcodes an anchor instead of rendering fixes, as workspace.html does today for How to set up Slack: templates are not type-checked against the offer rule. No deserialization, cast, reflection or outside writer reaches ConnectionWords; it is built per request and never stored or sent on a wire.
```

### States

Admitted

```text
The 28 reachable (cause, connected) rows times the four Viewer values; on GET /workspace/integrations only the two workspace-admin viewers are reachable, because the page is behind require_workspace_admin.
```

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

Excluded

```text
fixes containing the /admin/instance/integrations href when viewer.is_instance_admin is false: no arm of words constructs it, and the href is appended only inside that branch.
```

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

Excluded

```text
I-19 before an existing link: fixes is built by pushing the existing link first and I-19 last, so the inverted order is not constructible through words.
```

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

Unknown

```text
Whether every current and future template renders all of fixes rather than its first element: Askama rendering is outside the type that owns the rule, so a partial render would silently drop I-19.
```

crafter and implementation reviewer

Unknown

```text
Whether the instance integrations page stays at INSTANCE_PATH with the same guard while Requests 2 and 3 change that page: a moved route or a tightened guard would turn an offered I-19 into a dead end.
```

architect for Requests 2 and 3

### Models

None.

### Closure

```text
Every offered link is openable by the viewer it is offered to
```

Safety

Pass

DesignedNotBuilt

```text
Constructive exclusion: words is the only writer of fixes, the /admin/instance/integrations href is appended only inside the is_instance_admin branch and the setup href only inside the workspace-admin branch, and rustc rejects a non-exhaustive match over the closed cause and offer enums. The exhaustive 112-row finite table decides the whole relation, and the public oracle follows each offered link and observes 200. Not covered by any checker: a template that writes an href literally instead of rendering fixes, recorded as a Bypass risk owned by the implementation reviewer.
```

##### Evidence

None.

```text
I-19 is unconstructible for a non-instance-admin viewer and never precedes an existing link
```

Safety

Pass

DesignedNotBuilt

```text
Constructive exclusion in words: the href is appended only inside the is_instance_admin branch and only after the existing link is pushed, and rustc rejects a non-exhaustive match over the closed NoConnectionCause and offer enums.
```

##### Evidence

None.

```text
An instance admin reading W-2 Slack is offered both links in order
```

Inhabitation

Pass

DesignedNotBuilt

```text
Admitted input SlackOffer::NotSetUp with connected false and Viewer::new(true, true) -> NoConnectionCause::of_slack -> words -> ConnectionWords with words SLACK_NOT_SET_UP and fixes [HOW_TO_SET_UP_SLACK -> SETUP_PATH, I-19 -> INSTANCE_PATH]; acyclic from the instance snapshot, with no bootstrap needed.
```

##### Evidence

None.

```text
The offer reaches a public port on all three doors
```

PublicReachability

Pass

DesignedNotBuilt

```text
workspace.rs:262-285 renders workspace.html on GET /workspace/integrations; route_form.rs:431 renders route_form.html on GET routes/new and on the preview and save posts; the followed href reaches show_instance at instance.rs:404 on GET /admin/instance/integrations. Every site exists today; only the offer list and the viewer parameter are new.
```

##### Evidence

None.

### Risks

#### Y1

Stressor

```text
Negated assumption: every page that shows W-2 or W-3 renders all of ConnectionWords.fixes. A template keeps the shipped if-let shape and renders only the first element.
```

```text
The W-2, W-3 and K-19 words and the first offered link
```

```text
I-19 silently missing on that page
```

```text
I-19 rendered for a viewer who is not an instance admin, or rendered before the existing link
```

```text
The rule is enforced in Rust but its rendering is not type-checked; every consuming template must loop over fixes, and the acceptance oracle must read the links of each of the three doors, not just one.
```

crafter and implementation reviewer

#### Y2

Stressor

```text
Negated assumption: NavContext.is_instance_admin is accurate. resolve_is_instance_admin hits a store error and fails closed to false.
```

```text
The W-2 or W-3 words and the existing link
```

```text
I-19 withheld from a genuine instance admin on that render
```

```text
I-19 offered after a failed resolve, which would offer a link the request cannot prove openable
```

```text
Fail-closed is the shipped nav policy and this design inherits it; a genuine instance admin may read a page with no I-19 during a store outage and reaches the setup through the sidebar instead.
```

crates\/foundry\-app\/src\/nav\.rs

#### Y3

SinglePointOfFailure

```text
NoConnectionCause::words is the only owner of these words and links for the workspace page, all three route-form doors and the project page; a wrong arm misleads every page at once.
```

```text
One finite table over the whole domain is the single falsifier, and the three doors cannot drift apart
```

```text
A single wrong arm reaching every page in the same release
```

```text
A second precedence for the same decision in a template or handler
```

```text
workspace.html keeps its own K-19 and Add-to-Slack arms, which this Request does not unify; those two arms remain a place where the page could drift from the route form.
```

architect

#### Y4

Bypass

```text
A template that writes an anchor directly instead of rendering fixes. workspace.html does exactly that today for How to set up Slack, and nothing in the type system prevents a new page from doing it again. No deserialization, cast, reflection, foreign process or outside writer reaches ConnectionWords, and no process failure can leave it half-built, because it is constructed per request and never stored.
```

```text
Nothing: a hardcoded link is outside the rule
```

```text
Nothing
```

```text
Any href to /admin/instance/integrations written in a template instead of coming from fixes
```

```text
Review must check that the W-2 and W-3 arms of workspace.html render fixes and hold no literal href; this is the one bypass the design cannot close in types.
```

implementation reviewer

#### Y5

Stressor

```text
Second stressor set, walked over the finished design. The instance integrations page is moved or its guard tightened by Request 2 or 3, while I-19 keeps pointing at INSTANCE_PATH.
```

```text
The W-2 and W-3 words and the existing link
```

```text
Nothing
```

```text
An offered I-19 that answers the uniform not-found page
```

```text
The oracle follows the I-19 href read off the page and asserts 200, so a moved route or a tightened guard fails the lane rather than shipping a dead end; the walk found no missing design element, only this oracle obligation.
```

acceptance designer

#### Y6

Stressor

```text
Second stressor set. The offered-links observation cannot see the new href: the shipped workspace_links helper keeps only hrefs under /workspace/integrations, so the no-way-to-fix-it-here assertion stays green while I-19 leaks to a lead.
```

```text
Nothing
```

```text
Nothing
```

```text
A P3 scenario that cannot fail when I-19 leaks
```

```text
The walk found a real hole in the existing oracle, not in the product: the support helper must read every page link before P3 is falsifiable. Recorded as a shared constraint and sent to the acceptance designer.
```

acceptance designer

#### Y7

Stressor

```text
Second stressor set. A viewer who is an instance admin but not a workspace admin opens a route form, a viewer state unreachable on the workspace page.
```

```text
The W-2 or W-3 words, and K-4 where that is what a non-admin reads
```

```text
I-19 offered with no How to set up Slack before it
```

```text
How to set up Slack offered to a viewer who is not a workspace admin, since that page is behind the workspace-admin guard
```

```text
The viewer value object makes this state explicit and the finite table covers it; the walk confirmed the two facts are genuinely independent and must not be collapsed into one admin flag.
```

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

### Architectural tests

#### T1

```text
The instance-admin fact does not cross down into foundry-services or foundry-store: no new dependency edge from a lower crate on foundry-app's authorization, over the whole crate graph the boundary law governs. This is ADR-IW-001's rejected alternative, restated as a check the change can actually violate.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh cargo xtask check-arch
```

### Contract tests

None.

## Slices

### S1

#### Targets

crates\/foundry\-app\/src\/integrations\/no\_connection\.rs

Extend

```text
The Viewer value object, the ordered fixes carrier, the widened words, line and project_lines, and the finite table extended with the instance-admin dimension.
```

crates\/foundry\-app\/src\/integrations\/handlers\/workspace\.rs

Extend

```text
Build the Viewer from the page's workspace-admin guard and nav.is_instance_admin and render ConnectionWords for the W-2 Slack, W-3 and W-2 Matrix arms instead of four word fields and two booleans.
```

crates\/foundry\-app\/templates\/integrations\/workspace\.html

Extend

```text
Render the words and loop over fixes in those arms; remove the literal setup anchor at line 14.
```

crates\/foundry\-app\/src\/integrations\/handlers\/route\_form\.rs

Extend

```text
Pass the Viewer into words where the page builder already holds nav; this one builder serves the form GET and the preview and save posts.
```

crates\/foundry\-app\/templates\/integrations\/route\_form\.html

Extend

```text
Loop over fixes in order instead of the single optional fix.
```

crates\/foundry\-app\/src\/integrations\/handlers\/routes\.rs

Extend

```text
Pass the Viewer into project_lines where nav is already built; the project page's lines and links are unchanged.
```

crates\/foundry\-app\/templates\/integrations\/project\.html

Extend

```text
Loop over fixes in order instead of the single optional fix.
```

crates\/foundry\-app\/src\/integrations\/handlers\/matrix\_setup\.rs

Extend

```text
Pass the Viewer into the Send-test refusal, which reads the words only and offers no link.
```

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

#### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::no_connection
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-04'
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_lane integrations-wayfinding'
scripts/test-in-lima.sh cargo xtask check-arch
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/integrations\_wayfinding\.rs

crates\/foundry\-acceptance\/src\/support\/external\_integration\_plugins\.rs

#### Public oracle

```text
With this Foundry holding a secrets key and no Slack app, and Priya Raman set as this Foundry's instance admin: GET /workspace/integrations as Priya (workspace admin and instance admin) and as Tomas Rivera (workspace admin, not instance admin); then with no secrets key, GET the new-route form, POST its preview and POST its save for a Matrix route and for a Slack route on Identity Platform, as Priya and as Lena Park (team lead, not a workspace admin); then follow each offered link read off the rendered page.
```

```text
Priya's Slack section reads the shipped W-2 Slack sentence followed by two page links in order, How to set up Slack then Set up integrations for this Foundry, and following the second answers 200 at /admin/instance/integrations. Priya's Matrix route form, its preview and its save each read the shipped W-2 Matrix sentence followed by Set up integrations for this Foundry alone, and her Slack W-3 state reads the shipped W-3 sentence followed by that same single link. Tomas reads the same W-2 Slack sentence with How to set up Slack and no second link. Lena's route form reads the shipped W-2 Slack sentence with no page link at all. Every W-2, W-3, K-2 and K-19 sentence is byte-identical to what ships today, and the three doors of one kind agree word for word and link for link.
```

```text
Any /admin/instance/integrations link among Tomas's or Lena's page links, read over every href on the body with the sidebar excluded; Set up integrations for this Foundry before How to set up Slack in Priya's list; a changed W-2, W-3, K-2 or K-19 word; an I-19 link that answers the uniform not-found page when followed; or a route-form preview or save that shows different words or links from the form GET for the same state and viewer.
```

##### Promises

P1

P2

P3

P5

#### Prefactoring

```text
L2 move the carrier: replace ConnectionWords.fix (Option<FixLink>) with the ordered fixes (Vec<FixLink>) and change route_form.html, project.html, route_form.rs, routes.rs and matrix_setup.rs to the list shape, with no viewer parameter, no new words and no new link, so every list is still zero or one element long. This makes the follow-up change, appending I-19 for an instance admin, a single branch in one arm of words instead of a second link field plus template precedence.
```

```text
Every shipped W-2, W-3, W-4, W-6, W-7, K-4, K-10 and K-19 sentence and its single fix link, on the route form GET, the preview and save posts, the Send-test refusal and the project page.
```

crates\/foundry\-acceptance\/tests\/features\/integrations\-wayfinding\.feature

##### Verification

```text
scripts/test-in-lima.sh cargo test -p foundry-app --lib integrations::no_connection
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_warm; eip_lane integrations-wayfinding'
```

#### Records

##### Decisions

None.

##### Constraints

None.

##### Reuse

how\_to\_set\_up\_slack

crates\/foundry\-app\/templates\/integrations\/workspace\.html\:14

Extend

```text
This arm already renders W-2 Slack and its setup link; it keeps the words and stops holding the link decision, rendering the offer list instead. The literal anchor on this line goes away, so the page can no longer disagree with the route form.
```

no\_connection

crates\/foundry\-app\/src\/integrations\/handlers\/route\_form\.rs\:46

Extend

```text
The page struct already carries the rendered ConnectionWords for all three doors of a kind; it keeps that one field and gains only the Viewer the page builder passes into words. A separate field for I-19 would put the ordering decision in the template.
```

refusal

crates\/foundry\-app\/templates\/integrations\/route\_form\.html\:20

Extend

```text
Its single optional fix becomes a loop over the ordered fixes; the words and, for every shipped row, the single link are unchanged.
```

connection\_lines

crates\/foundry\-app\/src\/integrations\/handlers\/routes\.rs\:43

Extend

```text
The project page already renders a list of ConnectionWords; project_lines takes the Viewer, built where nav already exists, and the page's lines and links stay as they ship.
```

connection\_lines

crates\/foundry\-app\/templates\/integrations\/project\.html\:6

Extend

```text
Same loop change as the route form, so one rendering shape serves every ConnectionWords consumer; no W-6, W-7 or K-4 line gains a link.
```

cause

crates\/foundry\-app\/src\/integrations\/handlers\/matrix\_setup\.rs\:281

Extend

```text
The Send-test refusal already reads its words from the same cause function; it keeps doing so through the Viewer and offers no link, so K-19 and K-4 stay word for word.
```

##### Impact

template\, consumer of ConnectionWords

crates\/foundry\-app\/templates\/integrations\/workspace\.html\:14

Changed

```text
The W-2 Slack, W-3 and W-2 Matrix arms render the words and loop over fixes; the literal setup anchor on this line is removed. The K-19 and Add-to-Slack arms are untouched.
```

template\, consumer of ConnectionWords

crates\/foundry\-app\/templates\/integrations\/route\_form\.html\:20

Changed

```text
The single optional fix becomes a loop over the ordered fixes; this one template serves the form GET and the preview and save posts.
```

template\, consumer of ConnectionWords

crates\/foundry\-app\/templates\/integrations\/project\.html\:6

Changed

```text
The same loop change; the project page's words and links are unchanged by it.
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

crates\/foundry\-app\/templates\/integrations\/slack\_setup\.html

Extend

```text
Steps 3 and 4 become the locked I-20 sentences and the footnote about the SLACK_* trio taking precedence is added; the manifest block is unchanged.
```

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature

#### Verification

```text
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_warm; eip_lane us-iis-04'
scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; eip_lane instance-integration-setup'
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/instance\_integration\_setup\.rs

crates\/foundry\-acceptance\/src\/support\/external\_integration\_plugins\.rs

#### Public oracle

```text
GET /workspace/integrations/slack/setup as Tomas Rivera, a workspace admin, on a Foundry with no Slack app; and read the manifest shown there against the one the instance integrations page renders for Priya.
```

```text
Step 3 reads: An instance admin enters the app's Client ID, Client Secret and Signing Secret under Instance admin > Integrations. Step 4 reads: No restart is needed. A workspace admin can then choose Add to Slack on the workspace integrations page. A footnote reads: An instance operator may instead set SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET in Foundry's environment and restart; the environment takes precedence. No step of the page says Restart Foundry, and the manifest the page shows is identical to the one the instance integrations page renders.
```

```text
The phrase Restart Foundry anywhere in the page's steps, a step 3 that still names setting SLACK_CLIENT_ID on this Foundry, a missing or reworded footnote, or a manifest that differs from the instance integrations page's.
```

##### Promises

P4

P6

#### Prefactoring

None.

#### Records

##### Decisions

None.

##### Constraints

None.

##### Reuse

SLACK\_CLIENT\_ID

crates\/foundry\-app\/templates\/integrations\/slack\_setup\.html\:9

Extend

```text
The shipped guide is the page W-2 Slack already links to; step 3 stops naming the settings on this Foundry and names the browser form, step 4 drops the restart, and a footnote keeps the environment alternative. A second guide would split the audience of one locked copy row; the heading, intro, manifest block and back link stay as they are.
```

manifest

crates\/foundry\-app\/src\/integrations\/handlers\/slack\_setup\.rs\:29

Reuse

```text
I-20 changes prose only; the manifest field and the handler that fills it are untouched, which is what AC 4 requires and what the shipped manifest-equality scenario already observes.
```

##### Impact

template\, the rendered operator guide

crates\/foundry\-app\/templates\/integrations\/slack\_setup\.html\:9

Changed

```text
Step 3 names the browser form under Instance admin > Integrations instead of the SLACK_* settings, step 4 replaces Restart Foundry with the no-restart sentence, and the footnote keeps the environment alternative with its precedence. No handler field, no struct and no other template is touched.
```

##### Agreement

None.

##### Not applicable

prefactoring

```text
The only code target is one template's step text and a new footnote: there is no production structure to move, no signature or carrier to change first, and no follow-up change a preserving move would make easier. Its GREEN baseline is the shipped instance-integration-setup lane, which this slice does not restructure.
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
