# DELIVER upstream issues: external-integration-plugins

This file collects the decisions, deviations, follow-ups and open gates that DELIVER found. They
come from the orchestrator's notes (2026-10-22 to 2026-10-27). "FD" means
`../feature-delta.md`, followed by the wave and the section. Commit shas point at the change that
settled an item, where there is one. Nothing here is released: no push, no tag.

## 1. User decisions

| # | Date | Decision | Relates to |
|---|---|---|---|
| UD-1 | 2026-10-22 | **The H-3 fixture gate moves to release time.** DELIVER builds slice 01 onward against the Slack double's hand-written answers, behind `FOUNDRY_EIP_HANDWRITTEN_SLACK=1`, which prints a loud notice. Without the setting the double refuses to start. No tag until the recorded fixtures replace the hand-written answers (see §5). | FD DEVOPS [REF] Hand-off tasks (H-3); FD DISTILL Revision cycle 3 (B7); roadmap `open_items`; `0faf718` |
| UD-2 | 2026-10-25 | **Matrix disconnect is built like Slack's.** It has a confirm page, forgets the token and pauses routes ("Paused: Matrix was disconnected"). No admin email is sent. A same-bot reconnect resumes without sending the window. The copy is ratified: "Disconnect Matrix? 1 route in 1 project stops posting. Foundry forgets the bot's access token." and "… disconnected Matrix (matrix.canzan.dev)". | FD DISCUSS K-9, D-8; FD DISTILL User ratification R-9; DIST-25; `6bb8cd5`, `63bf28d` |
| UD-3 | 2026-10-25 | **The sidebar's "Notifications" link is renamed "Settings"** (it still opens `/account/settings`). This un-pends scenario 99. | FD DISTILL Upstream Issues DIST-U9; `cf2c8bd` |
| UD-4 | 2026-10-25 | **R-12 is ratified as written:** "The test message was not delivered: {reason}." | FD DISTILL [REF] User ratification (R-12); review B4 |
| UD-5 | 2026-10-25 | **Fix before release, part 1, the safety set:** drop the unused 0030 columns `endpoint` and `not_before`; trim previous keys; fail an email that has no stop link; keep the notifier timeout under the lease; make a Slack client build failure refuse boot. | FD DESIGN DDR-9, DDR-16, ADR-EIP-007; `fbe9bdc`, `a61a399` (R2) |
| UD-6 | 2026-10-25 | **Fix before release, part 2:** poll performance (0032 pending indexes, no `::text` cast), the kind as a `ChannelKind` type, and the big-file splits (store `integrations.rs`, the integration handlers). | FD DESIGN [REF] Persistence Model; `a61a399` (R2), `cc5a9ce` (R3) |
| UD-7 | 2026-10-25 | **Email burst coalescing is a follow-up feature**, not this one. | FD DISCUSS D-11, OD-8 |
| UD-8 | 2026-10-25 | **Accepted as known behaviour:** "mine" emails ("issues I created or commented on") for a deleted issue never send. | FD DISCUSS US-EIP-10 AC; FD DESIGN DDR-6 |
| UD-9 | 2026-10-21 | **Deliverable C** (signed webhooks, OAuth apps, routes by API; slices 06-08) is split off as the feature `integration-webhooks-and-oauth-apps`. | FD DISCUSS [REF] User Decisions (OD-1); FD DESIGN [REF] What Deliverable C Inherits |
| UD-10 | 2026-10-25 | **The release gate:** no tag until (a) the H-3 recorded Slack fixtures exist and the Slack scenarios pass with `FOUNDRY_EIP_HANDWRITTEN_SLACK` unset, (b) G1-G4 hold, and (c) a full `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` is green. | FD DEVOPS [REF] CI/CD pipeline outline (G1-G6); §5 |

Earlier user decisions (OD-1..OD-15 "accept all", 2026-10-21; R-1..R-11, 2026-10-22) are recorded
in FD DISCUSS [REF] User Decisions and FD DISTILL [REF] User ratification.

## 2. Deviations from DESIGN (accepted by the orchestrator)

| # | Deviation | Relates to |
|---|---|---|
| DV-1 | `SealedSecret` lives in `foundry-auth/src/secret_box.rs`, not as a `foundry-core` newtype. | FD DESIGN brief "For Software Crafter"; DDR-9 |
| DV-2 | The expiry sweep moved from 01-02 to 03-01, and check-arch rule 4 landed in 01-04. | roadmap |
| DV-3 | Roadmap step 01-12 was added: the integration doors redirect a signed-out visitor to `/sign-in`, and project pages get a History link. That makes 27 steps. | `3377fde`, `b54a3b4` |
| DV-4 | Slack signature freshness (±300 s) is checked against the wall clock, not the injected `Clock`. | FD DESIGN DDR-11; 03-02 |
| DV-5 | Disconnect calls Slack `apps.uninstall`, not `auth.revoke`. | FD DESIGN DDR-11, ADR-EIP-005 |
| DV-6 | `resume_route` moves `receives_from` only when the route was paused (03-04 changed the 03-03 semantics). An edit keeps `receives_from`, so events queued before an edit still send. | FD DESIGN DDR-27, DD-8 |
| DV-7 | History times: `finished_at`, then the first attempt, then `occurred`. The 24-hour give-up is derived at read time (`QueuedDelivery::is_expired_at`, shared with `expiry_set`). | FD DESIGN [REF] Route and Delivery Status |
| DV-8 | The kind is decoded closed at the store boundary (a private `StoredKind`), but the row structs still carry `String` (see PA-2). The census, backlog and tally aggregates and the audit rows are not converted, by design. | FD DESIGN DDR-1; check-arch rule 6 (`deny.toml`) |
| DV-9 | 0030 was edited in place in R2 (columns dropped). It was never pushed, but local dev databases that applied the earlier 0030 need a reset. | FD DESIGN DDR-17 |

## 3. Follow-ups and known behaviour

### 3a. Review items the user did not choose for this release (Phase 4, non-blocking)

| # | Item | Relates to |
|---|---|---|
| FU-1 | The services forwarder layer is a middle man (thin forwarders over the use-cases). | FD DESIGN DDR-2 |
| FU-2 | Slack `plain_text` escapes "&" as "&amp;", so a title with "&" reads "&amp;" in Slack. | FD DESIGN DDR-12; FD DISCUSS D-3 |
| FU-3 | The address judge's range table ("judge ranges") could be tightened or documented further. | FD DESIGN DDR-14, ADR-EIP-006 |
| FU-4 | A Matrix message body has no size bound. | FD DESIGN DDR-13 |
| FU-5 | `StopToken` derives `Debug`. | FD DESIGN SC-3 (secrets never in debug) |
| FU-6 | A race in connection upsert. | FD DESIGN DDR-4 |
| FU-7 | A replica sends its deliveries one after another, across routes. | FD DESIGN DDR-5 |
| FU-8 | Slack Events API replays are not restricted beyond the ±300 s signature window. | FD DESIGN DDR-11 |

### 3b. Known behaviour (recorded, no change planned)

| # | Behaviour | Relates to |
|---|---|---|
| KB-1 | Reconnecting Matrix with a **different** bot user on the same homeserver is accepted. | FD DISCUSS US-EIP-08 |
| KB-2 | A disconnected Slack connection may reconnect to **another** Slack team. Only a *lost* connection refuses another team. | FD DISCUSS US-EIP-06 scenario 4 |
| KB-3 | A refused other-team install is not revoked at Slack, so an orphan install remains in that team. | FD DESIGN ADR-EIP-005 |
| KB-4 | Pause, disconnect and delete do not count the deliveries they drop in `foundry_integration_deliveries_total` (the services have no metrics). The CHANGELOG says so. | FD DESIGN DDR-22 |
| KB-5 | Pacing is set at claim time, so the gap is about 1 s plus preparation time and may stretch under load. | FD DESIGN DDR-5 |
| KB-6 | Saving email choices and stopping emails write no audit row. | FD DISCUSS D-16 |
| KB-7 | `FOUNDRY_SECRETS_KEY_PREVIOUS` is ignored when no current key is set. | FD DESIGN ADR-EIP-004 |
| KB-8 | A non-UUID `project_id` in a hand-written outbox payload now fails the dispatch tick. Before R2 it matched nothing. Only hand-made rows can carry one. | FD DESIGN DDR-3; `a61a399` |
| KB-9 | The service's "route with no event" check cannot be reached at acceptance level, because the DB CHECK refuses first (it is a mutation survivor by construction). | FD DISTILL DIST-U3 |
| KB-10 | A relay that accepts and then bounces an email is reported Delivered. | FD DESIGN [REF] Earned Trust |
| KB-11 | Under at-least-once delivery, a send that times out after Slack accepted it is posted twice, with the same `event_id`. | FD DISCUSS OD-7; FD DESIGN ADR-EIP-002 |
| KB-12 | An event whose transaction started before a route's create and committed after it is not delivered to that route. | FD DESIGN DDR-27 (accepted edge) |
| KB-13 | Saving a route to a lane that has gone still answers the uniform 404 (review B1, not in the design). | FD DESIGN DDR-20 |
| KB-14 | "Mine" emails for a deleted issue never send (UD-8). | — |

### 3c. Follow-ups (code, tests, docs)

| # | Item | Relates to |
|---|---|---|
| FU-9 | Email burst coalescing, as a feature (UD-7). | FD DISCUSS D-11 |
| FU-10 | Burst copy for New issue and New comment bursts is unratified. It is unreachable today (no single action creates more than five issues or comments). | FD DESIGN UI-3; FD DISTILL R-10 |
| FU-11 | DD-2 says degraded mode admits no connection or route write on an unavailable connection. 01-08 built the page side (Add to Slack hidden, K-19 shown). No later step recorded refusing route writes on an unavailable connection. Verify, and add a scenario if it is missing. | FD DESIGN DD-2 |
| FU-12 | Mutation survivors left after `12cdbbe`: the worker's `Retention::tick`, `expiry_of` and `head_of` (they need a worker harness); `route_form` `matrix_homeserver` and the channels Matrix arm; `shared_tick`; Slack `answer_head` (Retry-After); services `room_still_routed` (needs a Postgres-backed services test); app lane rows not measured. | FD DEVOPS [REF] Mutation testing strategy |
| FU-13 | No scenario follows a route's History link from the project page, and no scenario covers the three POST doors that redirect signed-out visitors (route create, route test, Slack return). | `b54a3b4` |
| FU-14 | Rename the store test `previous_release_migrator_accepts_0028_to_0031`: it now migrates through 0032. | FD DEVOPS C-9 |
| FU-15 | The check_arch unit-test fixtures `ROUTE_STORE` and `HANDLERS_FILE` still name the paths from before the R3 split (fixtures only, so the rule is unaffected). | `cc5a9ce` |
| FU-16 | The R2-c "Slack client fails boot" check is a source check, because no seam can make `reqwest` `build()` fail. | `fbe9bdc` |
| FU-17 | The feature files use "he/his" for Sam. They should use they/them. | FD DISCUSS [REF] Personas |
| FU-18 | The `eip_pre` preflight refuses while foreign listeners hold ports in the 5xxxx range (another app's helper, a leftover `http.server`). Use `EIP_PRE_IGNORE`, or narrow the check. | `scripts/eip-lanes.sh` |
| FU-19 | A product performance note, not this feature's: lane move latency grows with the destination column's size (18 ms empty to 116 ms at about 500 cards; the T-1 bench). | `docs/product/kpi-contracts.yaml` EIP-KPI-4 |
| FU-20 | Process: 05-01's RED evidence was recorded after implementing. | Evolution doc, Lessons |

## 4. Pending approvals (the user's)

| # | Item | Relates to |
|---|---|---|
| PA-1 | **Two proposed scenarios (DIST-30), not ratified:** "Deleting one of two routes to a room keeps the bot in it" (the behaviour exists in the DESIGN adapter contract and in a store test only) and "A member gets no email about events or projects they did not choose". New Gherkin needs the user's approval. | FD DISTILL [REF] DISTILL decisions (DIST-30) |
| PA-2 | **Move `ChannelKind` to `foundry-core`** so the store row structs can carry a typed kind. Deny rule 4 bans store → integrations today (DV-8). | FD DESIGN [REF] check-arch and Dependency Rules |
| PA-3 | **Accept ADR-EIP-001..007.** They are still *Proposed*. DEVOPS took DD-1..DD-11 as accepted at their recommendations; the user has not confirmed them. Each ADR now carries an "Implemented" note. | FD DESIGN [REF] Open Design Decisions; `docs/product/architecture/adr-eip-00*.md` |
| PA-4 | **Phase C session cleanup:** remove `deliver/.develop-progress.json` and `.nwave/des/deliver-session.json`. | nw-finalize Phase C |
| PA-5 | **Push and tag**, only on the user's go and only after §5. | Release gate |

## 5. Release gate (open; no release has been made)

| Gate | State at `8c681b2` | What closes it |
|---|---|---|
| **H-3 recorded Slack fixtures** | Open. `crates/foundry-acceptance/tests/fixtures/slack/` does not exist, and the double serves hand-written shapes under `FOUNDRY_EIP_HANDWRITTEN_SLACK=1`. | A Slack test workspace and app. Record `oauth.v2.access`, `auth.test`, `conversations.list`, `conversations.info`, `chat.postMessage`, `apps.uninstall` and the Events envelopes, then run every Slack scenario with the variable unset. |
| **G1** full gate | Open. No fully green Docker `xtask ci` on the final code. The last attempt (05-02) ran 1997 of 2003 acceptance rows; the 6 failures passed alone; load average was about 65. | One green `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` on the tagged commit, on a quiet machine, with non-zero counts for every EIP tag, including `needs-matrix` and `needs-smtp`. |
| **G2** nothing pending | Met at `4c43d8c`. | Recheck `@pending` on the tagged commit. |
| **G3** KPI-4 guardrail | Open. Only the arm-A baseline exists (`verdict: BASELINE`, `573faf7`). | Run the bench in guardrail mode (arms A and B) on the tagged commit. `verdict` must be PASS, read with the pinned `yq` container. |
| **G4** prod only with deliverable A complete | Satisfied in code (03-06 and 01-11 done). | Prod promotion stays the user's approval. |
| G5 / G6 | H-1 (secrets key) and H-2 (Slack app) per cluster are the user's. The CHANGELOG and RELEASING edits are landed. | Done before integrations are offered on a cluster. Without them the release runs with integrations not offered. |
| Before the tag | Two items are open. (a) The migration deadlock flake: two slice-8 `@migration-histogram` scenarios migrate fresh schemas concurrently and get "deadlock detected". It also happens on the pre-feature base, and is likely 0028/0029 against a concurrent migrate. (b) The manual Continuwuity checklist from 04-02: directory and room, encryption 404, invite-only 403/404, `joined_members`, `m.mentions`, `whoami` 401 errcode. | Investigate (a); work through (b) against the dev Continuwuity. |
| After deploy | — | DoD 3 dogfood on dev, then H-5/H-6 alerts and panels and the H-7 Cloudflare check. Verify dev and, after promotion, prod. |
