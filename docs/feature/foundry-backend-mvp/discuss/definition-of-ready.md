# Foundry Backend MVP — Definition of Ready

Validates that each story passes all 9 DoR items before handoff to DESIGN.

Items checked (per the LeanUX skill):

1. Problem statement clear, in domain language.
2. User/persona with specific characteristics.
3. ≥3 domain examples with real data.
4. UAT scenarios in Given/When/Then (3-7 per story).
5. AC derived from UAT.
6. Right-sized (S/M/L, 1-3 days, 3-7 scenarios).
7. Technical notes capture constraints/dependencies.
8. Dependencies resolved or tracked.
9. Outcome KPIs defined with measurable targets.

Plus: every non-`@infrastructure` story carries an **Elevator Pitch** (per nw-product-owner-reviewer Dimension 0) AND a `job_id` referencing an entry in `jobs.yaml` (per JTBD-traceability hard-gate, Decision 1, 2026-04-28).

## Gate Blockers from Prior Review — Resolution Status

| Blocker | Severity | Resolution | Evidence |
|---------|----------|------------|----------|
| Missing `job_id` on all 13 stories | CRITICAL | RESOLVED | `jobs.yaml` created as single source of truth; every story US-01..US-13 carries a `**job_id**:` line under its title; mapping mirrored in `jobs.yaml :: story_job_map`. No `infrastructure-only` stories — all 13 trace to a real JTBD outcome. |
| Missing slice composition artifacts (`story-map.md`, `slices/*.md`) | CRITICAL | RESOLVED | `story-map.md` created with backbone, walking skeleton, 4 release slices, slice-level validation table, and priority rationale. No separate `slices/*.md` files needed — the consolidated map is sufficient. |
| US-06 AC-1 leaks argon2id implementation details | HIGH | RESOLVED | AC-1 rewritten to outcome-focused: "Password hashing uses parameters at or above current OWASP-recommended levels; parameters are reviewed annually and updated if OWASP recommendations change." Specific `m=64MiB, t=3, p=1` parameters moved to Technical Notes with cross-reference to NFR-SEC-01. |

## Confirmed (no change needed)

- **NFR-PERF-01**: Already states "P95 server-render latency ... ≤200 ms measured at the application boundary" as a ceiling (the 50ms aspirational figure is annotated as a stretch goal, not a release blocker). Aligned with the locked decision.
- **NFR-PERF-02**: Already states "Default = 10 MB. Recommended max = 50 MB." for `FILE_UPLOAD_MAX_MB`. Aligned with the locked decision.
- **US-05 invite flow**: UAT scenarios show invite-link as the default mechanism, with the email-invite scenario explicitly gated on `Given SMTP env vars are set in .env`. Aligned with the locked decision.

---

## Per-Story DoR Validation

### US-01: Operator installs Foundry in under an hour

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Devansh has 60 min, prior OSS trackers cost 2-4 hours, gives up. |
| 2. Persona | PASS | Devansh Iyer, SRE, 12-person Series-A, Docker comfortable, Rust unfamiliar. |
| 3. Examples (≥3) | PASS | Fresh VM happy path; port conflict edge case; proxy-blocked pull error. |
| 4. UAT (3-7) | PASS | 5 scenarios. |
| 5. AC from UAT | PASS | 5 ACs trace to scenarios. |
| 6. Right-sized | PASS | M, 3 days. |
| 7. Tech notes | PASS | Migration runner, bootstrap token mechanism, .env keys called out. |
| 8. Dependencies | PASS | None (walking-skeleton entry). |
| 9. Outcome KPI | PASS | P80 setup time ≤10 min; measurable via opt-in telemetry. |
| Elevator Pitch | PASS | Before/After/Decision triplet present. |
| **Verdict** | **READY** | |

### US-02: Operator scales to multi-replica

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | OSS trackers' sticky-session requirement makes self-host a SPOF. |
| 2. Persona | PASS | Devansh; 80-person org; production posture required. |
| 3. Examples | PASS | Rolling restart; mid-SSE replica death; DB outage. |
| 4. UAT | PASS | 4 scenarios. |
| 5. AC | PASS | 4 ACs from scenarios. |
| 6. Sized | PASS | M, 3 days. |
| 7. Tech notes | PASS | tokio::sync::broadcast for fanout; LISTEN/NOTIFY per replica. |
| 8. Deps | PASS | US-01, US-09 (tracked). |
| 9. KPI | PASS | 95% rolling restarts with no auth re-prompt. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-03: Backup and restore

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Data sovereignty (JTBD outcome #2); SaaS exports degraded. |
| 2. Persona | PASS | Devansh, post-install, DR-minded. |
| 3. Examples | PASS | Nightly dump + test restore; partial restore mid-write; PG-version mismatch. |
| 4. UAT | PASS | 3 scenarios — at lower bound but each is substantial; meets the 3 minimum. |
| 5. AC | PASS | 3 ACs. |
| 6. Sized | PASS | S, 1-2 days. |
| 7. Tech notes | PASS | bytea storage; backup-verify subcommand. |
| 8. Deps | PASS | US-01, US-11 (tracked). |
| 9. KPI | PASS | 100% restore success following docs (qualitative; survey-measured). |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-04: Upgrade in place

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Upgrading OSS in production = high-risk; brittle stories cause pinning forever. |
| 2. Persona | PASS | Devansh, 3 months in, applying v0.2.1 → v0.3.0. |
| 3. Examples | PASS | Minor bump happy; concurrent migration race; failed migration rollback. |
| 4. UAT | PASS | 3 scenarios. |
| 5. AC | PASS | 4 ACs from scenarios. |
| 6. Sized | PASS | M, 2-3 days. |
| 7. Tech notes | PASS | sqlx-cli, advisory lock ID, CREATE INDEX CONCURRENTLY note. |
| 8. Deps | PASS | US-01. |
| 9. KPI | PASS | 95% minor upgrades with zero user-visible disruption. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-05: Admin bootstrap + invite

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Multi-page wizards = friction; indie wants one form. |
| 2. Persona | PASS | Devansh, fresh-install admin, lunch hour. |
| 3. Examples | PASS | Invite link happy path; expired invite; duplicate workspace 409. |
| 4. UAT | PASS | 5 scenarios. |
| 5. AC | PASS | 5 ACs. |
| 6. Sized | PASS | M, 3 days. |
| 7. Tech notes | PASS | argon2id params, HMAC invite token, lettre crate for SMTP. |
| 8. Deps | PASS | US-01, US-06. |
| 9. KPI | PASS | 70% of bootstrapped instances see member #2 within 24h. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-06: Sign in with email/password

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | OIDC deferred; password auth must still be competent. |
| 2. Persona | PASS | Mei Chen, returning user, possibly stale cookie. |
| 3. Examples | PASS | Sign-in happy; password reset email; wrong password + 5x rate-limit. |
| 4. UAT | PASS | 6 scenarios. |
| 5. AC | PASS | 5 ACs from scenarios. |
| 6. Sized | PASS | M, 3 days. |
| 7. Tech notes | PASS | argon2 crate, secrecy crate, session schema, reset-token schema. |
| 8. Deps | PASS | US-05. |
| 9. KPI | PASS | 95% first-attempt sign-in success (excluding forgotten-password). |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-07: Create + view project

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Hierarchy must match Linear's: team→project→issue. |
| 2. Persona | PASS | Mei, member of Backend team, wants new project. |
| 3. Examples | PASS | Create happy; duplicate-name conflict; non-team-member 403. |
| 4. UAT | PASS | 5 scenarios. |
| 5. AC | PASS | 4 ACs. |
| 6. Sized | PASS | M, 2-3 days. |
| 7. Tech notes | PASS | Issue state list fixed in MVP; routes spelled out. |
| 8. Deps | PASS | US-05, US-06. |
| 9. KPI | PASS | 80% of workspaces have ≥2 projects within 7 days. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-08: File an issue

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | JTBD-critical hot path; "thought → captured" must be fast. |
| 2. Persona | PASS | Mei, on board, captures Safari refresh bug. |
| 3. Examples | PASS | Title-only quick create; full-detail issue; empty-title error. |
| 4. UAT | PASS | 6 scenarios. |
| 5. AC | PASS | 5 ACs. |
| 6. Sized | PASS | M, 3 days. |
| 7. Tech notes | PASS | pulldown-cmark, ammonia, Postgres sequence for keys. |
| 8. Deps | PASS | US-07. |
| 9. KPI | PASS | Median modal-to-submit ≤8s for title-only issues. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-09: Realtime updates

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Linear-feel realtime; without it, users ping each other in Slack. |
| 2. Persona | PASS | Mei on board, Hiroshi editing. |
| 3. Examples | PASS | State-change visible; new-issue appears live; SSE drop+reconnect. |
| 4. UAT | PASS | 5 scenarios. |
| 5. AC | PASS | 4 ACs. |
| 6. Sized | PASS | M, 3 days. |
| 7. Tech notes | PASS | pg_notify payload format, per-project filter, htmx/EventSource client choice deferred to DESIGN. |
| 8. Deps | PASS | US-08. |
| 9. KPI | PASS | 99% events propagate within 2s. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-10: Comment on an issue

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Linear's discussion-at-issue model co-locates context. |
| 2. Persona | PASS | Mei, comments on AUTH-3 with 2 existing comments. |
| 3. Examples | PASS | Comment posted + realtime; author edits; non-author 403. |
| 4. UAT | PASS | 6 scenarios. |
| 5. AC | PASS | 4 ACs. |
| 6. Sized | PASS | S-M, 2 days. |
| 7. Tech notes | PASS | Comment schema; soft-delete; reuses SSE channel. |
| 8. Deps | PASS | US-08, US-09. |
| 9. KPI | PASS | 50% of multi-participant issues have ≥1 comment. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-11: Attach file

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | bytea decision locked in DIVERGE; backup completeness depends on it. |
| 2. Persona | PASS | Mei attaches screenshot to UI bug. |
| 3. Examples | PASS | Small PNG; large PDF near cap; oversized file 413. |
| 4. UAT | PASS | 5 scenarios. |
| 5. AC | PASS | 5 ACs. |
| 6. Sized | PASS | M, 3 days. |
| 7. Tech notes | PASS | Streaming bytea read; chunked HTTP; CASCADE delete. |
| 8. Deps | PASS | US-08. |
| 9. KPI | PASS | 30% of issues have ≥1 attachment by month 1. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-12: Keyboard navigation

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Linear-feel difference vs JIRA/Trello. |
| 2. Persona | PASS | Mei, keyboard-heavy power user. |
| 3. Examples | PASS | List nav; suppressed-while-typing; unknown shortcut. |
| 4. UAT | PASS | 6 scenarios. |
| 5. AC | PASS | 4 ACs. |
| 6. Sized | PASS | M, 2-3 days. |
| 7. Tech notes | PASS | Alpine `x-on:keydown.window` + input-focus suppression; ILIKE search MVP. |
| 8. Deps | PASS | US-08. |
| 9. KPI | PASS | 60% of sessions invoke ≥1 shortcut. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

### US-13: Contributor onboarding

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | JTBD outcome #3; Rust dev productive in a day. |
| 2. Persona | PASS | Jamal Okafor; Rust dev; AGPLv3-attracted. |
| 3. Examples | PASS | Mac happy path; Linux+podman alternative; outdated toolchain error. |
| 4. UAT | PASS | 5 scenarios. |
| 5. AC | PASS | 5 ACs. |
| 6. Sized | PASS | S, 1-2 days. |
| 7. Tech notes | PASS | sqlx-cli, test isolation via transactions. |
| 8. Deps | PASS | US-01 + general compilability. |
| 9. KPI | PASS | 50% of first-clones reach green tests in 30 min. |
| Elevator Pitch | PASS | Present. |
| **Verdict** | **READY** | |

---

## Aggregate DoR Status

| Story | Status |
|-------|--------|
| US-01 | READY |
| US-02 | READY |
| US-03 | READY |
| US-04 | READY |
| US-05 | READY |
| US-06 | READY |
| US-07 | READY |
| US-08 | READY |
| US-09 | READY |
| US-10 | READY |
| US-11 | READY |
| US-12 | READY |
| US-13 | READY |

**13 of 13 stories pass all 9 DoR items, plus the Elevator Pitch invariant, plus the `job_id` JTBD-traceability check.**

| Story | `job_id` |
|-------|----------|
| US-01 | jtbd-outcome-1 |
| US-02 | jtbd-outcome-6 |
| US-03 | jtbd-outcome-2 |
| US-04 | jtbd-outcome-7 |
| US-05 | jtbd-outcome-1 |
| US-06 | jtbd-outcome-4 |
| US-07 | jtbd-outcome-4 |
| US-08 | jtbd-outcome-4 |
| US-09 | jtbd-outcome-4 |
| US-10 | jtbd-outcome-4 |
| US-11 | jtbd-outcome-2 |
| US-12 | jtbd-outcome-4 |
| US-13 | jtbd-outcome-3 |

**Aggregate verdict: READY for DESIGN handoff.** All 3 gate blockers from the prior review are resolved.

---

## Open Questions for the User (top 3, surfaced for resolution before DESIGN starts)

These are not DoR blockers — they are choices that, if reversed later, would create rework in DESIGN:

1. **P95 server-render budget**: stories.md and nfrs.md reflect the DIVERGE recommendation's 50 ms aspiration as a 200 ms measurable ceiling. If the operator-installing-foundry experience is intended to defend 50 ms as a release-blocker, DESIGN needs to know now (it changes the template-rendering and SQL strategy substantially).
2. **Default file-upload cap**: stories.md sets `FILE_UPLOAD_MAX_MB=10` as a default with 50 MB recommended max. If the indie segment files larger screencasts, the default may need to be 25 MB. Confirm or override before DESIGN sizes the upload pipeline.
3. **Email invites without SMTP**: US-05 falls back to "link only" when SMTP is unconfigured. If we instead want a fully-out-of-the-box experience with a default `sendmail`-mode or an embedded test-mail server, that's a separate decision worth surfacing now.

---

## Risks Surfaced (for DESIGN's risk register)

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|-----------|
| LISTEN/NOTIFY doesn't scale to N=10 replicas | Medium | High | Tested at MVP at N=3; load test at N=5 in NFR-PERF-03 CI |
| bytea attachments hit Postgres TOAST limits | Low (at 10MB default) | Medium | Hard cap 100 MB, defer S3 backend; document escape hatch |
| argon2id parameters become CPU bottleneck on small VMs | Low | Low | env-tunable; 2 vCPU reference hardware tested |
| sqlx compile-time-checked queries slow down build | Medium | Low (DevEx only) | Cache `target/` in CI; document `cargo sqlx prepare --offline` |
| Migration race against advisory lock under network partition | Low | Medium | Health check ensures replicas don't serve until migration complete |
| htmx 2 → 4 migration churn | High (over 12 months) | Medium | Keep hx-* surface minimal; vanilla EventSource for SSE |
