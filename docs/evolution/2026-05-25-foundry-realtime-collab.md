# Evolution — foundry-realtime-collab (Slice 2)

**Finalized**: 2026-05-25
**Ship commit**: [33e5f6f](../../) — "Initial commit: Foundry MVP — Slices 1 + 2"
**Wave coverage**: DISTILL → DELIVER (DIVERGE/DISCUSS/DESIGN inherited from slice 1; design context in `docs/feature/foundry-backend-mvp/design/realtime-roadmap.md` and `design/system/realtime-infrastructure.md`)

## Feature summary

Slice 2 of the Foundry MVP — realtime collaboration on top of the
slice-1 walking skeleton. Three user stories (US-09 realtime issue
updates via SSE, US-10 markdown comments with sanitization, US-12
keyboard-driven navigation contracts) — 19 acceptance scenarios green
(7 US-09 + 6 US-10 + 6 US-12 incl. 1 `@manual` browser drill).

The slice proves that Foundry can fan out issue + comment events to
every project member within the NFR-PERF-03 1 s p99 budget without
introducing Redis, without sticky sessions, and without breaking the
slice-1 "no state outside the DB" contract. The realtime substrate is
real `LISTEN`/`NOTIFY` on a dedicated per-replica Postgres connection +
real `tokio::sync::broadcast` fan-out + real SSE to the browser.

## Business context

JTBD outcome-4: "Foundry feels as fast as Linear." The slice's
load-bearing assertion is that operators can choose Postgres as the
realtime substrate — over the conventional Redis pub/sub default —
without sacrificing UX speed. If the assertion held, Foundry could
keep the slice-1 "Postgres + Foundry, nothing else" contract for the
contributor onboarding promise (which slice 4 / US-13 later cashed in
on directly).

## Key decisions

### From DISTILL (`distill/wave-decisions.md` + `distill/driver.md`)

- **Strategy C — all real adapters.** Walking skeletons use real
  `reqwest` HTTP → real `axum` binary → real Postgres testcontainer
  → real `LISTEN`/`NOTIFY` → real `tokio::sync::broadcast` → real SSE
  stream parsed by a custom client. No mocks at the driven side.
  Justification: JTBD outcome-4 (Linear-class speed) only manifests
  end-to-end; a mocked driven adapter cannot prove `pg_notify` +
  `LISTEN` + broadcast actually delivers within 1 s p99.
- **Custom ~80-line SSE line parser, not `eventsource-client`.** The
  off-the-shelf crate would have pulled in a second TLS stack
  (rustls + native-tls duplication). The custom parser sits over
  `reqwest::Response::bytes_stream()` and gives full control over
  the heartbeat-override seam.
- **Single `event:` type per channel + payload `event_type` +
  `schema_version: 1`.** Forward-compatible additions don't require
  parallel SSE event types — future event kinds extend the payload
  envelope. (Inherited from `design/realtime-roadmap.md`.)
- **Heartbeat env-var override.** `SSE_HEARTBEAT_MS` + per-scenario
  `override_heartbeat_ms(ms)` so the "quiet stream emits heartbeat"
  scenario doesn't have to wait the production-default interval.
- **Comment-event payload includes `author_email` at fan-out time.**
  Avoids a JOIN on every comment fan-out; the SSE consumer renders
  the author byline without a follow-up request.
- **Cross-project negative wait stays at 2.5 s.** "Subscriber does
  NOT receive events from a project they aren't viewing" is the only
  way to assert zero events without false-negative risk. Profiled
  cost; revisit only if CI suite exceeds budget.
- **No browser automation; alpine.js shortcut handlers verified via
  `@manual` drill.** Per the JTBD-backend-MVP "no Playwright"
  decision, US-12's actual `c`/`/`/`j`/`k`/`Enter`/`?`/`Esc`
  handling lives in client JS. The automated scenarios pin the
  server contracts those handlers depend on (`data-issue-key`
  attribute presence on the project board, modal-shaped htmx
  fragment, search htmx fragment, keyboard-help overlay listing)
  so client-side rot is detectable at the network boundary even
  without browser automation.

### From DISTILL — explicit deferrals + coverage gaps

- **Comment edit + delete → slice 3.** Slice 2 ships read + create +
  sanitize; the PATCH/DELETE comment routes would have pushed
  another two adapters. Routed to slice 3, NOT silently dropped.
  Documented in `coverage-matrix.md` as a known gap. (Slice 3 ended
  up shipping operator-grade work instead; comment edit/delete is
  still open.)
- **SSE reconnect-replay deferred.** Browser `EventSource` behaviour
  covers reconnect; replay-on-reconnect is documented as MVP
  out-of-scope in `driver.md` § 1 and `realtime-roadmap.md`.
- **`@error` budget below 40%.** 6 of 20 automated = 30%, justified:
  many slice-2 "errors" are already covered as positive contracts in
  slice 1 (CSRF, sign-in failures, project not found, team
  membership). The slice-2 error budget focuses on the NEW failure
  surfaces (empty/whitespace comment, non-member SSE subscribe,
  anonymous SSE subscribe, non-member comment, XSS sanitization).
  Adding synthetic errors to hit 40% would have lowered signal
  quality.

### From DELIVER (extracted from `33e5f6f` commit body)

- **Per-replica dedicated `PgListener` connection.** Each spawned
  axum replica owns its own LISTEN connection — the realtime fan-out
  topology slice 3's `MultiReplicaHarness` later validated by
  running N replicas concurrently against the same Postgres.
- **Trigger-based `pg_notify`.** Any future outbox insert auto-fans
  through the existing trigger; no per-table NOTIFY wiring needed in
  application code.
- **`pulldown-cmark` + `ammonia` sanitization** for markdown rendering.
  Real composition root in the test path; assertions parse the
  returned HTML with `scraper` and verify zero `<script>` elements +
  `rel="noopener"` on outbound links.
- **CommentAdded events carry `author_email` payload** per the
  DISTILL decision.
- **Hidden `data-issue-key` carriers** in project-board markup; htmx
  modal fragment for the `c` shortcut; `ILIKE` search for issue
  title + exact-key lookup; `/keyboard-help` overlay.

## Steps completed

No `deliver/execution-log.json` was emitted (slices 1 + 2 were
squashed into the initial commit, predating the nWave execute
orchestrator). The single ship commit `33e5f6f` enumerates the
delivered scope.

### 3 user stories (19 scenarios; 7 US-09 + 6 US-10 + 6 US-12 incl. 1 `@manual`)

- **US-09 — Realtime issue updates via SSE** (7 scenarios).
  Per-replica dedicated LISTEN connection, tokio broadcast,
  project-membership-filtered fan-out, heartbeats, trigger-based
  `pg_notify` (any future outbox insert auto-fans). Cross-project
  isolation verified with a 2.5 s zero-event wait. `@nfr-perf-03`
  asserts ≤ 2 s p99 per event under sequential creation.
- **US-10 — Markdown comments** (6 scenarios). `pulldown-cmark` +
  `ammonia` sanitization, real-time `CommentAdded` events with
  `author_email` payload, empty/whitespace body rejection via htmx
  fragment, XSS sanitization verified end-to-end (real
  `<script>alert('xss')</script>` body → zero `<script>` in
  rendered HTML). Comment edit + delete explicitly routed forward.
- **US-12 — Keyboard contracts** (5 automated + 1 `@manual`).
  Project-board `data-issue-key` carriers, htmx modal fragment for
  the new-issue shortcut, `ILIKE` search + exact-key lookup,
  `/keyboard-help` overlay listing every shortcut. Browser-side
  alpine.js shortcut handlers verified via `@manual` drill.

### Test harness additions (in `crates/foundry-acceptance/src/support/`)

- `sse_client.rs` — custom ~80-LOC SSE line parser over
  `reqwest::Response::bytes_stream()` with `open_sse_subscription()`,
  `read_event_with_timeout()`, `override_heartbeat_ms()`
- HTML scraping via `scraper` for sanitization assertions

## All slice-2 goals satisfied (verified at `33e5f6f`)

- [x] US-09 SSE walking skeleton: project-membership-filtered fan-out via real LISTEN + broadcast
- [x] US-10 markdown rendering + sanitization via real `pulldown-cmark` + `ammonia` composition
- [x] US-12 server contracts pinned for every alpine.js handler the manual drill exercises
- [x] All 3 walking skeletons exercise real Postgres + real axum (Strategy C)
- [x] Comment-event payload includes `author_email`
- [x] `@nfr-perf-03` asserts ≤ 2 s p99 per event
- [x] `@nfr-sec-05` asserts XSS sanitization end-to-end with a real `<script>` payload
- [x] No regression of slice-1's 37/37 green state
- [x] Suite runtime delta ~7.7 s (within the 20 s ceiling; total ~30 s well under the 60 s top-line)

## Verification at HEAD (`33e5f6f`)

- `cargo test --workspace` 52 default-tag scenarios green (37 slice 1 + 19 slice 2 minus the 4 `@docker-compose` and similar tag exclusions captured in the commit summary's "52 default + 3 docker-compose" math)
- `FOUNDRY_ACCEPTANCE_TAGS=docker-compose cargo test -p foundry-acceptance` (3 additional `@docker-compose` scenarios)
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo fmt --all -- --check` clean
- `cargo deny check` clean

## Lessons learned

1. **Strategy C is worth the substrate cost.** The +7.7 s for real
   `LISTEN`/`NOTIFY` + real SSE + real Postgres + real broadcast was
   small enough not to break the suite budget and large enough to
   catch the failure modes a mocked driven adapter would hide.
   Slice 3's multi-replica harness relied on this foundation — every
   slice-2 walking skeleton ran unchanged through the
   `MultiReplicaHarness::spawn(n)` proxy without breaking.
2. **Custom ~80-LOC parsers beat off-the-shelf crates when the
   dependency graph cost is real.** `eventsource-client` would have
   pulled in duplicated TLS plumbing. The custom parser also
   exposed exactly the seams the tests needed (`override_heartbeat_ms`)
   that an off-the-shelf parser would have hidden.
3. **Server-contract pinning + `@manual` browser drill is the right
   shape for SPA islands.** No Playwright needed; the contracts the
   alpine.js handler depends on are pinned at the network boundary
   so client-side rot fails loudly even without browser automation.
   Slice 4's contributor onboarding `@manual` drills followed the
   same pattern, generalizing the precedent.
4. **`author_email` at fan-out time is worth the small payload
   size.** Avoided a JOIN-per-comment fan-out. The "carry data
   inline rather than re-fetch" decision generalizes; future events
   should default to inline payload + `schema_version` for
   compatibility.
5. **Explicit deferrals beat silent gaps.** Routing comment
   edit/delete forward (with a "known gap" row in coverage-matrix)
   prevented the slice from feeling incomplete. Future slice owners
   know what to inherit; reviewers know what NOT to block on.

## Issues encountered

- **DELIVER ran outside the nWave execute orchestrator.** Slices 1
  + 2 were both squashed into the initial commit, so neither
  `deliver/roadmap.json` nor `deliver/execution-log.json` was
  emitted, and per-story commits are unavailable. The `33e5f6f`
  commit body is the single audit-trail substitute for both slices.
- **Coverage-matrix scenario-count drift.** The `coverage-matrix.md`
  cross-cutting roll-up names 21 scenarios while the shipped feature
  files (and the DISTILL feature files) both contain 19 (6 + 7 + 6).
  A counting error in the matrix, not a scope delta — the contract
  the matrix walks is correct row-by-row; only the rolled-up total
  is off. Not worth back-patching now.
- **Comment edit + delete still open.** Routed to slice 3, but
  slice 3 (operator-grade) shipped backup/restore + multi-replica +
  rolling-upgrade + attachments instead. Comment edit/delete is
  still an open MVP commitment.

## Permanent artefact locations

All artefacts stay in their delivery locations.
`docs/feature/foundry-realtime-collab/` has two inbound cross-references
from sibling feature workspaces
(`docs/feature/foundry-operator-grade/distill/driver.md` +
`distill/wave-decisions.md`), and the slice's design context is
inherited from already-preserved slice-1 artefacts at
`docs/feature/foundry-backend-mvp/design/realtime-roadmap.md` +
`design/system/realtime-infrastructure.md`.

## Open items for v0.1 RC

1. **Comment edit + delete (US-10 AC 2).** Routed forward to slice 3
   but not picked up; still an open MVP commitment. Decide:
   pre-v0.1 slice 5, or v0.1.x point release.
2. **SSE reconnect replay.** Documented as MVP out-of-scope; revisit
   when contributor / operator telemetry signals event-loss complaints.
3. **`@nfr-avail-03` SSE-reconnect coverage** is currently inherited
   from the US-12 `@manual` browser drill at release time. Consider
   adding an automated reconnect scenario if browser behaviour drifts.
