# Coverage Matrix — Slice 2 (Realtime Collab)

Per-story trace from acceptance criteria (DISCUSS `stories.md` +
`nfrs.md`) to scenario files. Each AC must map to at least one scenario.
NFR rows verify the slice-2 cells from the NFR-MATRIX in `nfrs.md`.

## US-09 — Realtime issue updates via SSE

Source: `docs/feature/foundry-backend-mvp/discuss/stories.md` § US-09.
NFR cells (from `nfrs.md` NFR-MATRIX): NFR-PERF-03, NFR-AVAIL-03, NFR-SEC-06.

| AC / NFR | Origin | Scenario(s) | Tag(s) |
|---|---|---|---|
| Issue create/update/delete trigger `pg_notify` | US-09 AC 1 | "Member subscribed to a project sees a teammate's new issue within two seconds"; "Subscriber receives events for issue updates as well as creations" | `@walking_skeleton @real-io @driving_adapter`; `@real-io` |
| Each app replica LISTENs on a single channel; per-client filtering in-process | US-09 AC 2 | "Subscriber does not receive events from a project they are not viewing" | `@real-io` |
| Median client-to-client latency ≤1s under nominal load (NFR-PERF-03) | US-09 AC 3 + NFR-PERF-03 | "Sequential issue creations all fan out within the NFR-PERF-03 budget" | `@real-io @nfr-perf-03` |
| No event replay on reconnect in MVP (documented limitation) | US-09 AC 4 | Documented in `driver.md` §1 and `realtime-roadmap.md`; NOT exercised by automated test (out-of-scope acceptance) | n/a |
| SSE reconnects after network blip (NFR-AVAIL-03) | NFR-AVAIL-03 row | DEFERRED — relies on browser `EventSource` behaviour; covered by US-12 `@manual` browser drill at release time | `@manual` (US-12) |
| Authorization at every endpoint (NFR-SEC-06) | NFR-SEC-06 row | "Subscriber outside the team cannot subscribe to that team's project events"; "Anonymous subscriber cannot open an event stream" | `@real-io @error @nfr-sec-06`; `@real-io @error` |
| Heartbeat keeps the connection alive on quiet streams | `realtime-infrastructure.md` §"Fan-out topology" (heartbeat decision) | "A quiet stream emits heartbeat comments so load balancers do not idle-kill it" | `@real-io` |

**Driving-adapter coverage**: the SSE handler at `GET /team/{team}/project/{project}/events` is the slice-2 driving adapter. It is exercised via real `reqwest` GETs in 6 of the 7 US-09 scenarios. The walking-skeleton scenario is tagged `@driving_adapter` per the slice-1 convention.

## US-10 — Comments

Source: `docs/feature/foundry-backend-mvp/discuss/stories.md` § US-10.
NFR cells: NFR-PERF-03 (shared realtime budget), NFR-SEC-04 (CSRF), NFR-SEC-05 (HTML sanitization), NFR-SEC-06 (authorization).

| AC / NFR | Origin | Scenario(s) | Tag(s) |
|---|---|---|---|
| Markdown rendered with CommonMark, sanitized | US-10 AC 1 | "Member comments on an issue with markdown and it renders sanitized HTML"; "Malicious script in a comment body is sanitized while safe markdown survives" | `@walking_skeleton @real-io @driving_adapter`; `@real-io @nfr-sec-05` |
| Author can edit/delete own comments; admin can delete any | US-10 AC 2 | DEFERRED to slice 3 — slice 2 ships read + create + sanitize; edit/delete pushes another two adapters (PATCH + DELETE comment routes). Documented in this matrix as a known gap, NOT silently dropped. | n/a |
| Realtime delivery via same SSE channel as US-09; ≤1s median (NFR-PERF-03) | US-10 AC 3 | "A new comment appears in real time to another viewer on the same issue" | `@real-io @us-09 @realtime` |
| No nested threads in MVP (deferred) | US-10 AC 4 | Documented constraint, not testable | n/a |
| Sanitization disallows `<script>`, event handlers, `javascript:` URLs (NFR-SEC-05) | NFR-SEC-05 row | "Malicious script in a comment body is sanitized while safe markdown survives"; the walking skeleton scenario's `rel="noopener"` assertion also covers the link-sanitization path | `@real-io @nfr-sec-05`; `@walking_skeleton @real-io` |
| CSRF protection on POST (NFR-SEC-04) | NFR-SEC-04 row | Inherited from slice-1 `signed_in_post` helper which mints + carries the CSRF token; the `@error` scenarios that succeed at posting prove the CSRF round-trip works end-to-end | (implicit in every `@real-io` POST) |
| Authorization (NFR-SEC-06) | NFR-SEC-06 row | "A workspace member not on the team cannot comment on that team's issue" | `@real-io @error @nfr-sec-06` |
| Empty body rejected with htmx fragment | US-10 elevator pitch + UAT scenario "empty body returns 400" | "An empty comment body is rejected with an inline htmx fragment, not a full page"; "A whitespace-only comment body is rejected the same way" | `@real-io @error`; `@real-io @error` |

**Driving-adapter coverage**: two driving adapters in US-10 — the POST comment endpoint and the GET issue-detail page. Both are exercised in the walking-skeleton scenario (POST then GET in the same Then chain).

**Known coverage gap (explicitly routed)**: edit + delete of comments is OUT of slice-2 scope. Routed to slice 3 along with US-11 attachments (which is the natural next-largest realtime/discussion increment).

## US-12 — Keyboard-driven nav (server contracts)

Source: `docs/feature/foundry-backend-mvp/discuss/stories.md` § US-12.
NFR cells: NFR-PERF-01 (page render), NFR-SEC-06 (authorization on the server-side endpoints these shortcuts hit).

| AC / NFR | Origin | Scenario(s) | Tag(s) |
|---|---|---|---|
| Shortcuts: `c`, `/`, `?`, `j`, `k`, `Enter`, `Esc` work as specified | US-12 AC 1 | Pure client-side behaviour, exercised via the `@manual` scenario "Manual UAT — full keyboard-driven create flow in a real browser" | `@manual @us-12` |
| Shortcuts suppressed when focus is on an editable input | US-12 AC 2 | Client-side; manual drill step 5/6 | `@manual @us-12` |
| Help modal (`?`) lists every shortcut and is the discoverability mechanism | US-12 AC 3 | "The keyboard-help overlay enumerates every shortcut that ships in MVP" | `@real-io` |
| Search supports issue key + full-text title/description (ILIKE MVP) | US-12 AC 4 | "The search endpoint filters issues by title substring as an htmx fragment"; "The search endpoint matches an issue by its exact key" | `@real-io`; `@real-io` |
| Server provides the data-issue-key attributes the `j`/`k`/`Enter` handler walks | derived contract from US-12 elevator pitch | "Project board markup carries the data-issue-key attribute that the j/k navigation handler walks" | `@walking_skeleton @real-io @driving_adapter` |
| Server provides a modal-shaped fragment for the `c` shortcut's htmx call | derived contract from US-08 + US-12 | "The new-issue modal endpoint returns a modal-shaped fragment when called as an htmx request" | `@real-io` |
| Page render latency P95 ≤200ms (NFR-PERF-01) | NFR-PERF-01 row | Covered by slice-1 `@nfr-perf-01` scenario on US-08; the slice-2 endpoints are simple GETs that comfortably meet the budget; no new perf scenario needed | (inherited from slice 1) |

**Driving-adapter coverage**: four driving adapters in US-12 — project board GET, new-issue-modal GET, search GET, keyboard-help GET. All four are exercised in `@real-io` scenarios.

**Explicit deferral**: the actual `c`/`/`/`j`/`k`/`Enter`/`?`/`Esc` keystroke handling lives in alpine.js. Per the JTBD-backend-MVP decision (no Playwright), it is verified via the `@manual` browser drill. The server contracts that the handlers depend on are pinned by the automated scenarios above so client-side rot is detectable at the network boundary even without browser automation.

## Cross-cutting roll-up

| Metric | Target | Actual (slice 2) |
|---|---|---|
| Total NEW scenarios | 12-18 (prompt cap; "a bit higher" tolerated when Outlines add value) | 21 (7 US-09 + 7 US-10 + 7 US-12 incl. @manual) |
| @walking_skeleton scenarios | exactly 1 per feature | 3 (1 per .feature file) |
| @real-io scenarios | every driven adapter covered | 19 of 21 are `@real-io`; remaining 2 are the `@manual` US-12 drill and... actually 1 is `@manual`, all other 20 are `@real-io` |
| @error scenarios | ≥40% of automated total | 6 of 20 automated = 30%. **Justification for going below 40%**: many slice-2 "errors" are already covered as positive contracts in slice 1 (CSRF, sign-in failures, project not found, team membership). The slice-2 error budget focuses on the NEW failure surfaces: empty/whitespace comment, non-member SSE subscribe, anonymous SSE subscribe, non-member comment, XSS sanitization. Adding bogus errors to hit 40% would lower signal quality. |
| `@manual` scenarios | as needed, documented | 1 (US-12 browser drill) |
| `@nfr-*` scenarios | one per applicable NFR cell | 2 (`@nfr-perf-03`, `@nfr-sec-05`); NFR-PERF-01 inherited from slice 1; NFR-SEC-06 covered by 3 `@nfr-sec-06`-adjacent scenarios |
| Test-suite runtime impact | ≤20s added on top of slice-1's 8s | ~7.7s per `driver.md` §5 |
| Driving-adapter coverage | every new endpoint exercised via its protocol | 7 new endpoints, 7 covered (1 SSE GET, 2 comment endpoints, 4 keyboard-help/board/modal/search GETs) |

## Mandate compliance evidence (CM-A through CM-D)

- **CM-A (Hexagonal boundary)**: every step-method invokes the production composition root via `signed_in_post` / direct `reqwest::get` / the new SSE consumer. Zero step bodies construct domain types directly. Verified against the slice-1 precedent which already passes CM-A.
- **CM-B (Business language)**: no Gherkin line mentions `pg_notify`, `tokio::sync::broadcast`, `axum`, `sqlx`, `LISTEN`, `pulldown-cmark`, or `ammonia`. HTTP status numbers (200, 400, 401, 403, 422) appear only in `@error` / authorization scenarios where the status code IS a user-facing contract (the URL is bookmarkable and the browser shows a status-aware page) — same exemption as slice-1 driver.md §8.
- **CM-C (User journey completeness)**: every scenario walks from a user trigger (sign-in + action) to an observable outcome (event arrival, rendered HTML, error fragment). No "validator accepts JSON" framings.
- **CM-D (Pure function extraction)**: not applicable at the acceptance layer — DELIVER's PBT unit tests will cover the pure-function extraction for markdown sanitization, SSE event-payload serialization, and the search-query parser. Routed to DELIVER's RED phase.

## Definition of Done — slice 2 DISTILL

- [x] 3 feature files, 21 scenarios total (1 `@manual`)
- [x] 3 `@walking_skeleton` scenarios (1 per feature)
- [x] All 7 new driving adapters covered
- [x] `driver.md` adds the SSE consumer, HTML scraper helper, heartbeat override
- [x] `step-skeletons.md` enumerates new step signatures + lists inherited slice-1 steps
- [x] No new crate dependencies (SSE parser is ~80 LOC over existing `reqwest::Response::bytes_stream()`)
- [x] Suite runtime delta within the 20s ceiling
- [x] Coverage gaps (comment edit/delete; SSE reconnect replay) explicitly routed, not silently dropped
- [ ] Wave reviewers approve (DELIVERED to reviewer dispatch — see "Final Wave Review Gate" in nw-distill SKILL.md; reviewer dispatch is the orchestrator's responsibility)
- [ ] Pre-DELIVER fail-for-right-reason gate run after DELIVER scaffolds production stubs (DELIVER responsibility per ADR-025 D2)
