# Evolution: external-integration-plugins (project events go to Slack channels and Matrix rooms over durable, never-pinging delivery, and members choose which project events Foundry emails them, with one-click stop links)

**Finalized**: 2026-10-27
**Commits**:
- DISCUSS: `0ab0ed3` (16 stories, OD-1..OD-15, user answer "accept all" on 2026-10-21).
- DESIGN: `2888450` (DDR-1..27, DD-1..11, ADR-EIP-001..007). DEVOPS: `07c6455`.
- DISTILL: `7ceb851` (scaffolded-RED acceptance suite; ratification R-1..R-11 of 2026-10-22).
- DELIVER roadmap: `45a81d3` (26 steps; 01-12 added in `3377fde`, so 27).
- DELIVER steps: `f5b152a` (01-01) to `4c43d8c` (05-02b). One feature commit per step, plus
  DISTILL harness fixes between steps (table below).
- Refactor (L1-L4): `bfd5daa` (core, auth, store, services, xtask) and `0bb5953` (foundry-app).
- Phase 4 review: RED tests `aa222f7`; R1 fixes `717c3a5`; R2 RED tests `fbe9bdc`; R2 fixes
  `a61a399`; R3 splits `cc5a9ce`.
- Mutation gaps closed: `12cdbbe`. Integrity count fix: `57ddd86`. Dead code removed: `8c681b2`.

`4576521` (tokio 1.54.1 for RUSTSEC-2026-0386) sits inside the same range. It is a dependency
advisory fix, carried onto `main` after `cargo deny` started failing during 05-02, and not part of
this feature's own work.

Gates:
- DES integrity: "All 27 steps have complete DES traces", exit 0.
- Phase 4 adversarial review: two parts (core and app), both NEEDS_REVISION. Every blocking item
  (A1-A5, B1-B7) was fixed in R1 or by a harness fix. The user chose which non-blocking items to fix
  before release (R2, R3); the rest are in `upstream-issues.md`.
- Mutation (per-feature, gate 80%): about 92.9% overall (1108 of 1193), foundry-app about 83.6%
  (368 of 440), after `12cdbbe`.
- G2 (nothing `@pending` in a released slice): met at `4c43d8c`.
- Full Docker `xtask ci`: **not yet run green on the final code** (release gate).

Feature dir PRESERVED. **Not pushed, not released, no tag.** The release gate is still open
(see Release).

Sources:
- `docs/feature/external-integration-plugins/feature-delta.md` (DISCUSS, DESIGN, DEVOPS, DISTILL;
  the single wave document, with no `design/` directory)
- `docs/feature/external-integration-plugins/deliver/roadmap.json` and `execution-log.json`
- `docs/feature/external-integration-plugins/deliver/upstream-issues.md` (written at this finalize)
- `docs/feature/external-integration-plugins/distill/red-classification.md`
- `docs/feature/external-integration-plugins/slices/slice-0{1..5}-*.md` (06-08 belong to the
  follow-on feature)
- The DELIVER orchestrator's tracker and upstream notes

**Scope**: deliverables A (slices 01-03, Slack) and B (04-05, Matrix and member email).
- A workspace admin connects the workspace's Slack in one approval, or Foundry's Matrix bot with a
  homeserver address and token.
- A team lead routes chosen project events to a channel or room. They choose:
  - the events: New issue, Issue moved, Issue deleted, New comment;
  - the lanes a move must enter;
  - how much each message reveals.
- Delivery is after commit, retried for 24 hours, paced and ordered per route, and visible in a
  route history.
- A broken connection pauses its routes and emails the workspace admins once. Reconnecting delivers
  what waited.
- A member chooses project emails per project and event. Each email carries a signed stop link and
  RFC 8058 one-click headers.
- The work adds one new workspace crate (`foundry-integrations`), five migrations (0028-0032) and
  no new third-party crate.
- Deliverable C (signed webhooks, OAuth apps, routes by API; slices 06-08) moved to its own feature,
  `integration-webhooks-and-oauth-apps`. It inherits the seams fixed here (DESIGN DDR-24).

## Summary

Before this feature a project's events stayed on the board. Nobody heard about AUTH-12 unless
they opened it. The outbox was a NOTIFY trigger with no consumer, and Foundry stored no outside
credential. Now:
- **Connections.** `/workspace/integrations` connects Slack (per-instance app, session-bound install
  `state`, signed events endpoint for uninstalls) and Matrix (bot token checked with `whoami`).
  Credentials are sealed with AES-256-GCM under `FOUNDRY_SECRETS_KEY`. Admins can disconnect, and
  every change is on the activity list.
- **Routes.** `/team/{t}/project/{p}/integrations` offers New route, Preview, Send test, Save,
  Pause, Resume, Edit, Delete and History. The status of each route is derived, never stored.
- **Delivery.** The outbox is now an event log. A single active dispatcher expands whole source
  transactions into `integration_deliveries` exactly once. Fenced, leased `SKIP LOCKED` senders on
  every replica deliver at least once:
  - per-route FIFO;
  - one post a second per destination;
  - backoff from 10 s to 15 min, honouring retry-after;
  - a stored `give_up_at` for every delivery.
- **Content.** One pure composer applies the three content levels and the never-sent list. Pure
  per-kind encoders make text from people unable to ping. Property tests prove both.
- **Project emails.** Members choose them in Settings, and they are sent through a header-capable
  `EmailTransport`. The stop links never expire, and `/email/stop/one-click` is CSRF-exempt and
  signature-authenticated.
- **Operations.** The work adds:
  - delivery metrics and stall gauges on closed label sets;
  - a degraded start when the secrets key cannot open a stored credential;
  - `foundry doctor rotate-secrets`;
  - an outbound address policy (`FOUNDRY_OUTBOUND_ALLOW`) for every user-supplied host;
  - an image-build guard that refuses a binary carrying test seams.

## Business context

The jobs were "hear about my project where my team already talks", "get the email I chose and
nothing else" and, for deliverable C, "connect a custom tool safely". A team lead (Lena) could not
send AUTH work to `#auth-dev`, and a member (Sam) could not ask for comment emails about their own
issues without getting every account email. The outcome KPIs are:
- EIP-KPI-1, the north star: an active route on AUTH and OPS for 30 days;
- KPI-2: Add to Slack to first message within 5 minutes;
- KPI-3: 99% within 60 s, 100% delivered or shown failed within 24 h;
- KPI-4, a guardrail: board p95 within 10% with every destination down;
- KPI-5: zero leaks or pings;
- KPI-6: connection loss flagged at once;
- KPI-7: stop in three actions or fewer.

At homelab scale each is a store query, an acceptance assertion or the user's report. Every dev
and prod half is owed, because nothing is deployed.

## Key decisions

**DISCUSS (user, 2026-10-21, "accept all").**
- OD-1: three deliverables, with C split off.
- OD-3: an operator-created Slack app per instance, with env credentials.
- OD-4: the admin enters the Matrix homeserver and token.
- OD-5: encrypted rooms are refused.
- OD-6: Summary is the default level.
- OD-7: at-least-once delivery, queued after commit, retried for 24 h. This changes ADR-007 for
  routes and project emails only.
- OD-8: one email per event.
- OD-9: plain members cannot see routes.
- OD-10: no email choices on by default.
- OD-11: nobody is emailed about their own actions.
- OD-12: a dump alone yields no usable credential.
- OD-13: loopback, link-local and metadata addresses are always refused; private ranges only
  through an allow-list.
- OD-14: no-mint is unchanged.
- OD-15: three OAuth scopes, for C.

**DESIGN DD-1..DD-11** (taken as accepted at their recommendations by DEVOPS; ADR-EIP-001..007
remain *Proposed* until the user accepts them, see `upstream-issues.md`):
- **DD-1** A signed `POST /integrations/slack/events`, for `app_uninstalled` and `tokens_revoked`
  only. Refusals remain the authority.
- **DD-2** A missing or non-opening secrets key starts the instance degraded. A malformed key
  refuses start.
- **DD-3** The Slack scope `chat:write.public` is requested, so public channels work without an
  invite.
- **DD-4** Per-route head-of-line ordering.
- **DD-5** Integration tables are not in the per-workspace export in v1.
- **DD-6** Copy for "cannot store connections yet". It was ratified as its own lines: K-20 for
  Slack (R-3) and K-18 for Matrix.
- **DD-7** Stop links never expire (R-11).
- **DD-8** Pausing a route cancels its pending deliveries.
- **DD-9** The Slack manifest is an admin page rendered with this instance's URLs
  (`/workspace/integrations/slack/setup`).
- **DD-10** Retention: finished deliveries older than 30 days beyond the newest 100 per route, and
  dispatched outbox rows older than 30 days, are pruned hourly.
- **DD-11** A 1 s poll, with no LISTEN coupling.

The mechanism decisions DDR-1..27 are summarised in the architecture brief.

**Ratified copy and rules (user, 2026-10-22; R-12 on 2026-10-25).**
- R-1: "Foundry will not send to this address. An instance operator can allow it."
- R-2/K-19: "{Kind} is unavailable: this Foundry cannot read its saved connection. An instance
  operator needs to check the secrets key."
- R-3/K-20: "Slack cannot be connected on this Foundry yet: an instance operator needs to set the
  secrets key."
- R-4: one pattern for every activity line, `{date} · {actor} connected|disconnected {kind}
  ({subject})` and `… created|edited|paused|resumed|deleted the route to {destination}`.
- R-5: "Too many test messages. Try again in a minute." (six a minute per person).
- R-6: 2 s is the acceptance bound for filing an issue while Slack is down. The KPI-4 bench (G3) is
  the guardrail.
- R-7: Matrix homeserver addresses go through the outbound address policy before any contact.
- R-8: "Needs attention: {room} is now end-to-end encrypted. Foundry's bot cannot post to encrypted
  rooms yet."
- R-9: a deliberate disconnect is a stop. Nothing queued or filed while disconnected is sent after
  a reconnect.
- R-10: "{n} issues moved to {lane} in {project} by {actor}" and "Foundry lost its {Kind}
  connection for {workspace}".
- R-11: stop links never expire. US-EIP-11 AC 4 reads "A tampered link".
- R-12: "The test message was not delivered: {reason}."

**DELIVER user decisions.**
- **2026-10-22: the H-3 fixture gate moves to release time.** DELIVER built 01-05 onward against
  the Slack double's hand-written answers, behind the loud opt-in `FOUNDRY_EIP_HANDWRITTEN_SLACK=1`.
- **2026-10-25: Matrix disconnect is built like Slack's.** It has a confirm page, forgets the token
  and pauses routes ("Paused: Matrix was disconnected"). No admin email is sent, and a same-bot
  reconnect resumes without sending the window. The confirm copy and the audit line were ratified.
- **2026-10-25: the sidebar's "Notifications" link is renamed "Settings".** This un-pended
  scenario 99.
- **2026-10-25, review round.** The user:
  - ratified R-12 as written;
  - ordered the pre-release safety set (R2): drop the unused 0030 columns `endpoint` and
    `not_before`; trim previous keys; fail an email that has no stop link; keep the notifier timeout
    under the lease; make a Slack client build failure refuse boot;
  - ordered poll performance (0032 indexes, no `::text` cast), a typed kind, and the big-file splits
    (R3);
  - made email burst coalescing a follow-up feature;
  - accepted that "mine" emails for a deleted issue never send, as known behaviour.

**DESIGN deviations recorded in DELIVER.**
- `SealedSecret` lives in `foundry-auth`, not `foundry-core`.
- The expiry sweep moved from 01-02 to 03-01.
- `apps.uninstall` is used, not `auth.revoke`.
- Slack signature freshness uses the wall clock, not the injected `Clock`.
- The kind is validated closed at the store boundary (`StoredKind`), but the row structs still
  carry a `String`. A typed field needs `ChannelKind` moved to `foundry-core`, because deny rule 4
  bans store → integrations. That is a pending user decision.
- `resume_route` moves `receives_from` only when the route was paused (03-04). An edit keeps it.

## Work completed

27 of 27 steps, from `execution-log.json` (RED, GREEN and COMMIT each; 01-10, 02-01, 02-03 and
05-02 ran twice). Each "DISTILL fix" commit is a harness or scenario correction by the acceptance
designer. None of them changed product code.

| Step | Commit | What |
|---|---|---|
| 01-01 | `f5b152a` | The KPI-4 bench and the T-1 baseline (arm A on v0.17.0 code) |
| 01-02 | `5032fe3` | Migrations 0028-0030; store dispatch, fenced claims and record |
| 01-03 | `ee07fec` | `SecretBox`: key ids, row-bound AAD, canary |
| 01-04 | `1d7cca5` | The pure core crate and the check-arch rules 1, 1a, 2 and 4, with the `deny.toml` wrapper |
| 01-05 | `2c60e6f` | Integration settings, harness `rewire`, Slack connect, return and setup page |
| 01-06 | `7ef6ac4` | Project routes, Send test and its limit, the worker, the Slack walking skeleton |
| 01-07 | `1c23d1c` (+ `7b749f3`, `fd1369e`) | Delivery isolation, the 24-hour give-up, stall gauges |
| 01-08 | `f5bf8da` (+ `c00a8fc`) | Restart durability, degraded start (K-19), `doctor rotate-secrets` |
| 01-09 | `2b80a1f` (+ `fe1fcba`) | The route history page, in Foundry's words |
| 01-12 | `b54a3b4` (roadmap `3377fde`) | Signed-out integration doors redirect to `/sign-in`; History link |
| 01-10 | `b37ad0f` (+ `55a88aa`, `7c72746`, `79a6bfa`) | Two-replica delivery, proven against seeded worker faults |
| 01-11 | `577e4d0` (+ `573faf7`, `8f47631`) | Single-replica failover seam and `integrations-test-seam`; scenario 132 (live claim never taken over) |
| 02-01 | `6471bbb`, re-run `83c1946` (+ `6170eac`) | All four chat events, payload enrichment (`from_state`, `author_id`, `title`), bursts by `source_txid` |
| 02-02 | `2724c66` (+ `087c488`) | Lane filters that follow renames, and a gone lane flagged |
| 02-03 | `beb0092`, re-run `9ce2f84` (+ `4e552fb`, `81d301c`) | Content levels, exact preview, shared-channel confirmation |
| 03-01 | `d8bf885` (+ `8805294`) | Backoff and retry-after, Slack answer classes, retention |
| 03-02 | `81056f8` (+ `76921c0`) | Connection lost, the signed events endpoint, one admin email |
| 03-03 | `a0010c9` | Pause, resume, edit and delete a route |
| 03-04 | `426d721` | Reconnect delivers what waited; wrong workspace refused; per-channel problems |
| 03-05 | `ee9dc7c` (+ `7628948`) | Disconnect Slack, project delete and restore, audit, authority |
| 03-06 | `803b24e` | Delivery metrics; per-destination pacing across replicas (advisory lock at claim) |
| 04-01 | `25a5d38` | `OutboundHttp` with the judging resolver, `FOUNDRY_OUTBOUND_ALLOW`, outbound test seam |
| 04-02 | `8815ab0` (+ `b083e83`) | Matrix connect, room routes, the Matrix walking skeleton |
| 04-03 | `3b15328` (+ `e33621c`) | Matrix refusals, connection health, Matrix content levels |
| 04-04 | `63bf28d` (+ `6bb8cd5`, `d97673e`) | Matrix history, edit, pause, delete (bot leaves an unrouted room), retries, disconnect |
| 05-01 | `5b65501` (+ `c1f0504`) | Project email choices, `EmailTransport`, the email walking skeleton |
| 05-02 | `cf2c8bd`, 05-02b `4c43d8c` (+ `b075e8a`, `7c8cc67`) | Stop links, one-click unsubscribe, the SMTP lane, sidebar Settings; UTF-8 for SMTP |

**Refactor (Phase 3).** There were two passes from base `45a81d3` (62 production files,
+15.9k lines; migrations excluded because they are applied and immutable).
- Pass A (`bfd5daa`) changed 12 files. The check-arch lexical write guard kept the
  `lock_live_project` openings and the `record_delivery` vendor branch as they were.
- Pass B (`0bb5953`) changed 10 files, with no test changes.
- Together the passes raised four suspects for the review: (a) `problem_cleared` used the Slack
  connection for Matrix routes; (b) the `delete_route` fallback skipped the room leave; (c)
  `proves_key` treated an empty check set as proven; (d) the Slack test failure copy said
  "the channel".

**Phase 4 review (two parts, both NEEDS_REVISION).**

Blocking items:
- **A1** (suspect a, confirmed) Resuming a Matrix route checked the Slack connection.
  `DestinationCatalogs` is now kind-aware.
- **A2** A delivery past `give_up_at` stayed claimable. The head choice, the claim SQL and the send
  now refuse it. The core property that pinned the defect was corrected.
- **A3** The D-3 leak property was vacuous. **A4** `stop_token_never_expires` was tautological.
  **A5** The OD-11 actor exclusion was untested. All three were fixed with fault proofs in
  `aa222f7`.
- **B1** A failed Matrix save or enrolment withdrew the bot from a shared room.
- **B2** (suspect b, confirmed) The `delete_route` fallback skipped the room leave.
- **B3** (security) Send test posted to any `form.destination`, with no postable check.
- **B4** (copy) The literal "the channel" (suspect d); the missing Matrix K-19 branch; K-18 never
  rendered; the unratified failure wrapper, which became R-12.
- **B5, B6** (harness) A hard-coded oracle and a discarded scanner response. Fixed in `aa222f7`.
- **B7** The contract-shape docstring mandate. The tag form was upheld, and docstrings were added to
  the new tests only.

Non-blocking items went to the user (decisions above).

**Revisions.**
- **R1** `717c3a5`: A1, A2, B1, B2, B3 and B4 green. `eip_review_findings` 10 of 10 with no
  ignores; 9 lanes green; mutation 28 of 33. One store test (`sweeps_apply_exactly_the_plan_ids`)
  had depended on the A2 defect and was corrected first.
- **R2** `fbe9bdc` (RED) and `a61a399` (fixes):
  - a project email whose stop link cannot be built is recorded failed;
  - the email transport gives up within the lease;
  - a Slack client build failure refuses start;
  - stored kinds are decoded closed;
  - previous keys are trimmed;
  - the 0030 columns are dropped;
  - 0032 adds the pending partial indexes, dispatch compares project ids as UUIDs, and an idle tick
    skips the read.

  10 lanes green; mutation 18 of 18.
- **R3** `cc5a9ce`: `foundry-store/src/integrations.rs` became `integrations/` (12 files, the
  largest 545 lines) and the integration handlers became `handlers/` (6 files, the largest 702
  lines). The SQL literals are byte-identical. There were no test edits and no check-arch rule
  change. `git log --follow` does not link the files; use `git blame -C -C`.

**Phase 5 and 6.**
- Mutation was measured at `cc5a9ce`: own tests 73.9%, with consumer suites 84.8%, foundry-app
  71.1%. The security survivors were `require_workspace_admin -> Ok(())` and the
  `finish_slack_install` state guard.
- `12cdbbe` killed 95 of 181 survivors, 55 of them in the app.
- `57ddd86` set the roadmap's `total_steps` to 27, and `des-verify-integrity` then passed.
- `8c681b2` removed the uncalled `IntegrationsWorker::tick`.

## Migrations

| Migration | Adds | Rollback |
|---|---|---|
| `0028_outbox_dispatch` | nullable `outbox.event_id`, `source_txid`, `dispatched_at` with metadata-only defaults; `integration_dispatch_start`. Takes a brief lock under a 2 s `lock_timeout` | Previous release runs; its outbox writes still get an event id; events it writes are delivered late unless marked dispatched (RELEASING R-6) |
| `0029_outbox_dispatch_index` | `idx_outbox_undispatched`, `CREATE INDEX CONCURRENTLY`, no transaction | Index unused |
| `0030_integrations` | connections, routes, route lanes, destination pacing, deliveries, audit, key checks, worker heartbeats | Tables unused |
| `0031_integration_email_choices` | members' project email choices | Table unused |
| `0032_integration_pending_indexes` | pending partial indexes on `integration_deliveries` (recipient, give-up) | Indexes unused |

All five are additive and forward-only, and the previous release boots on them. The store test
`previous_release_migrator_accepts_0028_to_0031` proves the previous release's migrator accepts
them (C-9). It now migrates through 0032, so its name is stale. `Store::probe` requires the
0028-0031 objects; 0032 adds only indexes. 0030 was **edited in place** in R2. It was never pushed, but a local dev database that
applied the earlier 0030 needs a reset.

## Guards (check-arch and deny)

- `integrations-pure-core` is an allow-list of the core crate's dependencies.
- `integrations-store-capability`: only `store_capabilities.rs` and `store_impls.rs` name `Store`.
- `integrations-secret-seam`: `SecretBox::open` appears only in the adapters, the connect handlers
  and `rotate_secrets`.
- `integrations-outbound-client-seam`: no stray `reqwest` client; no redirects; no proxy.
- `integrations-test-seam` checks that seam items are `cfg(feature = "test-support")`, that the
  Dockerfile builds exactly `-p foundry-app --bin foundry`, and that the marker string is unique.
  The Dockerfile also greps the built binary for the marker.
- `project-lifecycle-write-guard` now also covers route and email-choice writers.
- A `csrf.rs` pin keeps the exempt set at exactly `/bootstrap`, `/integrations/slack/events` and
  `/email/stop/one-click`.
- `deny.toml` bans `foundry-integrations` outside its wrappers.

## Quality results

**Lanes on the final code** (rows):

| Lane | Rows |
|---|---|
| `eip-01` (Slack connection, routes, delivery) | 44 |
| `eip-02` (events and content) | 38 |
| `eip-03` (trust and repair) | 61 |
| `eip-05` (member email) | 27 |
| `eip-replica` | 8 |
| `eip-restart` | 1 |
| `eip-metrics` | 8 |
| `needs-matrix` | 37 |
| `needs-smtp` | 1 |
| `notification-preferences-ui` (neighbour) | 12 |

The EIP lanes total 225 rows, the full DISTILL inventory (136 scenarios, 225 rows, 78 gold stubs),
with no `@pending` left. Every GREEN_ALREADY row was proven against a seeded fault.

**Mutation (per-feature, gate 80%).**
- Overall about 92.9% (about 1108 of 1193).
- foundry-app about 83.6% (368 of 440). **Gate met.**
- Remaining survivors:
  - the worker's `Retention::tick`, `expiry_of` and `head_of` (these need a worker harness);
  - `route_form` `matrix_homeserver` and the channels Matrix arm;
  - `shared_tick`;
  - Slack `answer_head` (Retry-After);
  - app lane rows the measurement did not reach.
- Log-only: `report_backlog`, the stalled warning.
- Equivalent: outbound `v4_of`, `ipv4_literal`, `number`; Jitter `>>`; one check_arch mutant.

**Integrity.** 27 of 27 steps traced: "All 27 steps have complete DES traces", exit 0.

**Not yet measured**:
- one fully green `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` on the final code (G1);
- the KPI-4 guardrail verdict (G3): only the arm-A baseline exists (`verdict: BASELINE`,
  `573faf7`);
- every dogfood KPI.

The last ci attempt (at 05-02) ran 1997 of 2003 acceptance rows green. The 6 us-03 shm failures
passed alone, and the run stopped twice on PortNotExposed and a connection reset at load average
about 65.

## Lessons

1. **Most red runs were harness defects, not product defects.** Over 15 DISTILL fix commits
   corrected the driver. Examples:
   - races in `given_filed`;
   - a Background that never called `rewire`;
   - a recorder that dropped headers;
   - an `/account/settings` 404 mistaken for a redirect;
   - replicas wired with integrations off;
   - a frozen clock that blocked pacing.

   Each fix was proven by reverting a seeded fault. Expect this ratio when a suite is scaffolded
   ahead of the code, and budget DISTILL time inside DELIVER.
2. **A seeded fault that kills nothing is a gap, not a pass.** Fault A (drop the live-lease fence)
   stayed green through 01-10 and 01-11. Only scenario 132 ("never sends a message a running
   replica is still sending") made it red. Keep a fault table per step, and refuse to close a step
   while one survives.
3. **Properties can pin a defect.** The core property at `core_properties` 249-279 asserted that a
   delivery past `give_up_at` was claimable (A2). A3 and A4 were vacuous or tautological. Review
   and mutation of the tests themselves found what the step mutations did not.
4. **Wall time against a frozen clock.** Per-destination pacing held the second message forever on
   the frozen `MockClock`. The fix was a `WallTickingClock` for the worker only, with the board
   kept frozen. Decide early which components read which clock in acceptance.
5. **Measure mutation through the consumer suites.** Own-crate tests scored 73.9% (services 13%).
   With the acceptance and app suites it was 84.8%, and 92.9% after the gap tests. Per-step
   in-diff figures (33%, 40%, 62.5% at times) were noise until the owning scenarios ran.
6. **RED evidence after the fact is not RED.** 05-01's RED was recorded after implementing.
   Record RED before writing product code, or mark the step's RED as not proven.
7. **The machine is shared.** Several things got in the way:
   - foreign listeners on 5xxxx ports (other projects' test servers, an editor helper) failed the
     preflight;
   - load averages of 16-65 skewed the bench;
   - leftover containers belonged to other sessions;
   - Docker Desktop and colima were swapped during 04-03.

   Check `ps`, `lsof` and the Docker context before blaming the product.
8. **Editing an applied migration has a cost** even before a push. Changing 0030's checksum
   forces a reset of every local database that applied it. Prefer a new migration unless the
   column has never left the machine (it had not here).

## Issues encountered

- 136 scenarios over 8 lanes, with 6 steps that each needed a second pass (01-10 blocked on the
  replica harness; 02-01, 02-03 and 05-02 partial; 03-01 and 03-05 re-pended). All were closed by
  DISTILL fixes and re-runs.
- **Product defects found by the suite:**
  - the New issue line read the lane at send time (02-01; fixed with the payload snapshot);
  - SMTP emails had no `Content-Type`, so "·" arrived as "Â·" (`4c43d8c`);
  - the probe fixture lacked 0031 (`7c8cc67`);
  - the A1/A2 and B1-B3 review findings.
- **Pre-existing flake to investigate before release:** two slice-8 `@migration-histogram`
  scenarios migrate fresh schemas concurrently and hit "deadlock detected". This happens on the
  pre-feature base too, and is likely 0028/0029 against a concurrent migrate.
- **Known flakes seen again:** sqlx `'\0'` under concurrency (an eip-01 row; passed alone);
  PortNotExposed and connection resets under load.
- **Advisory:** RUSTSEC-2026-0386 (tokio), bumped to 1.54.1 in `4576521`.
- Everything else is in `deliver/upstream-issues.md`: the review items the user did not choose, the
  known behaviour, the pending approvals and the release gate.

## Permanent artifacts

Already permanent; nothing copied at finalize. The layout is a single `feature-delta.md`, with no
`design/` or `discuss/` directory, and `distill/` holds no `walking-skeleton.md`. The ADRs live in
the house location.
- `docs/product/architecture/adr-eip-001-delivery-channel-contract-and-crate-placement.md`
- `docs/product/architecture/adr-eip-002-durable-after-commit-delivery.md`
- `docs/product/architecture/adr-eip-003-event-catalogue-v1-and-payload-enrichment.md`
- `docs/product/architecture/adr-eip-004-secrets-at-rest.md`
- `docs/product/architecture/adr-eip-005-slack-app-per-instance.md`
- `docs/product/architecture/adr-eip-006-matrix-bot-and-outbound-address-policy.md`
- `docs/product/architecture/adr-eip-007-member-project-email.md`

  Each now carries an "Implemented" status note. They stay Proposed pending the user's acceptance.
- `docs/product/architecture/brief.md` § "Project events leave the board: integrations, routes and
  durable delivery". It is marked shipped at this finalize, and its designed inventory is now the
  shipped one.
- `docs/product/journeys/journey-external-integration-plugins.yaml`, `docs/product/jobs.yaml` (three
  jobs), `docs/product/kpi-contracts.yaml` (`external-integration-plugins`, EIP-KPI-1..7; T-1
  baseline recorded)
- `RELEASING.md` "Integrations: secrets key and Slack"; `deploy/k8s/foundry-secret.example.yaml`,
  `foundry-configmap.yaml` and `README.md` (the new keys)
- `CHANGELOG.md` `[Unreleased]`: Added, Changed, Fixed, Security, Operators, Upgrading, Migration
  notes (0028-0032)
- `docs/feature/external-integration-plugins/deliver/upstream-issues.md`
- `docs/feature/external-integration-plugins/`: the full wave history

## Release (not done)

**No release has been made: nothing is pushed, and there is no tag.** Tag only on the user's go.
The gate:
1. **H-3, recorded Slack fixtures.** A Slack test workspace and its app. Record `oauth.v2.access`,
   `auth.test`, `conversations.list`, `conversations.info`, `chat.postMessage`, `apps.uninstall` and
   the Events envelopes under `crates/foundry-acceptance/tests/fixtures/slack/`. Then run every
   Slack scenario with `FOUNDRY_EIP_HANDWRITTEN_SLACK` **unset** (user decision 2026-10-22). Today
   the directory does not exist and the double serves hand-written shapes.
2. **G1:** one fully green `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` on the tagged commit, on a
   quiet machine. The acceptance step must be present, with non-zero counts for every EIP tag,
   including `needs-matrix` and `needs-smtp`.
3. **G2:** no `@pending` in `external-integration-plugins-0*.feature`. Met at `4c43d8c`; recheck on
   the tagged commit.
4. **G3:** the KPI-4 bench in guardrail mode (arms A and B) for the tagged commit, with `verdict`
   PASS read by the pinned `yq` container. Only the baseline exists.
5. **G4:** prod only with deliverable A complete. 03-06 and 01-11 are done. Prod promotion stays
   held for the user's approval by design.
6. G5/G6: the operator's H-1 (secrets key) and H-2 (Slack app) per cluster before integrations are
   offered there; the CHANGELOG and RELEASING edits are landed.
7. Before the tag: investigate the migration deadlock flake (Issues), and work through the manual
   Continuwuity checklist from 04-02.
8. After deploy: DoD 3, the dogfood on dev:
   - the operator sets up the Slack app;
   - Priya connects Slack and Matrix;
   - Lena routes AUTH;
   - Sam sets emails;
   - a forced uninstall and reconnect.

   Then H-5/H-6 alert rules and panels, and H-7 Cloudflare checks. Verify dev
   (`foundry.unintelligent-design.us`) and, after promotion, prod (`foundry.jeffbailey.us`).

## Follow-ups / open items

See `deliver/upstream-issues.md` for the full list, with the feature-delta sections each item
relates to. The main items:
- Email burst coalescing, a follow-up feature (user decision).
- Deliverable C, `integration-webhooks-and-oauth-apps`, which inherits DDR-24.
- Decisions owed to the user:
  - accept the two DIST-30 proposed scenarios, "Deleting one of two routes to a room keeps the bot
    in it" and "A member gets no email about events or projects they did not choose";
  - move `ChannelKind` to `foundry-core` for a typed row field;
  - accept ADR-EIP-001..007.
- From DISCUSS, still out of scope: F-1 (title and description edits), F-2 (membership events), F-3
  (encrypted Matrix rooms), F-4 (digest), F-5 (members see routes), F-7 (inbound Slack and Matrix).
- A product performance note, not this feature's: lane move latency grows with the column's size
  (18 ms empty to 116 ms at about 500 cards, measured by the bench).
