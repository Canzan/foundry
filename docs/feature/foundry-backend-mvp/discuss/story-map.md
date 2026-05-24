# Story Map: Foundry Backend MVP

## Personas
- **Operator** (Devansh Iyer, SRE) — installs, runs, scales, backs up, upgrades.
- **Admin / Member** (Devansh after install; Mei Chen, Hiroshi) — bootstraps workspace, signs in, files issues, comments, attaches files, navigates by keyboard.
- **Contributor** (Jamal Okafor, Rust dev) — clones, builds, contributes.

## End-to-End Goal
A self-hosted team (5-50 people) coordinates software work on infrastructure they control, with Linear-feel speed, in under an hour from `docker compose up` to filing the first issue.

---

## Backbone

| Install & Run            | Bootstrap & Invite      | Daily Use                          | Operate at Scale           | Contribute Back        |
|--------------------------|--------------------------|-------------------------------------|----------------------------|------------------------|
| Spin up containers       | Claim admin account      | Sign in                             | Run multiple replicas      | Clone repo             |
| Reach landing page       | Name workspace           | Create / view project               | Back up to a single file   | Build + test locally   |
| Get bootstrap URL        | Invite teammates         | File an issue (the hot path)        | Upgrade between versions   | Submit a PR            |
|                          |                          | See realtime updates                |                            |                        |
|                          |                          | Comment, attach files               |                            |                        |
|                          |                          | Navigate by keyboard                |                            |                        |

---

## Walking Skeleton (Slice 1)

The minimum end-to-end slice that lets one operator install Foundry, claim admin, invite themselves as a member, sign in, create a project, and file an issue. This is the thinnest possible "Foundry works" demo.

| Activity                    | Story | Why it's in the skeleton                                            |
|-----------------------------|-------|---------------------------------------------------------------------|
| Install & Run               | US-01 | Without `docker compose up` reaching healthy, nothing else exists.  |
| Bootstrap & Invite          | US-05 | Admin claim is the only way to create the first user + workspace.   |
| Daily Use — sign in         | US-06 | Returning users need to come back; not strictly needed for first session but trivially small and unblocks every other daily-use story. |
| Daily Use — create project  | US-07 | Issues live in projects; an empty workspace is not demonstrable.    |
| Daily Use — file an issue   | US-08 | The JTBD hot-path. Without this, the demo has no payoff.            |

End-to-end demonstrable value: **An operator installs Foundry, claims admin, signs in, creates a project, and files an issue — all from a fresh VM in under 30 minutes.** This is exactly the "hour to demo" promise from JTBD outcome #1.

---

## Release Slices

Every slice contains at least one user-visible value story (no slice is all `@infrastructure`). Slices are sliced by user outcome impact, not by technical layer.

### Slice 1 — Walking Skeleton: "Foundry works end-to-end"

- **Stories**: US-01, US-05, US-06, US-07, US-08
- **End-to-end demonstrable value**: A single operator can install Foundry on a fresh VM and file their first issue inside 30 minutes.
- **Target outcome KPIs**: jtbd-outcome-1 (install ≤ 1 hour), jtbd-outcome-4 (issue-create speed for the hot path).
- **Dependencies**: None (this is the entry slice).
- **Release theme**: "Hello, Foundry."

### Slice 2 — Linear-feel Hot Path: "Foundry feels like Linear"

- **Stories**: US-09 (realtime), US-10 (comments), US-12 (keyboard navigation)
- **End-to-end demonstrable value**: Two users on the same project board see each other's edits in real time, discuss in-issue with markdown comments, and never reach for the mouse.
- **Target outcome KPI**: jtbd-outcome-4 (Linear-feel speed: realtime ≤1s, ≥60% of sessions invoke ≥1 shortcut).
- **Dependencies**: Slice 1 (issues, projects, sign-in must exist).
- **Release theme**: "Same speed, different host."

### Slice 3 — Operator-grade: "Foundry runs in production"

- **Stories**: US-02 (multi-replica), US-03 (backup / restore), US-04 (in-place upgrade), US-11 (file attachments)
- **End-to-end demonstrable value**: A 3-replica Foundry deployment survives a rolling restart with no user-visible logouts; the operator takes a `pg_dump` that includes all attachments and restores it on a fresh VM; a minor-version upgrade applies cleanly with zero data loss.
- **Target outcome KPIs**: jtbd-outcome-2 (single-file backup = data sovereignty), jtbd-outcome-6 (multi-replica with no ops tax), jtbd-outcome-7 (upgrade without breakage).
- **Dependencies**: Slice 1 (Foundry exists to operate); Slice 2 (SSE realtime exists to test under replica restarts).
- **Release theme**: "Production-grade self-host."
- **Why US-11 (attachments) sits here, not in Slice 2**: Attachments are operator-facing in the sense that the bytea-in-Postgres decision is what gives Slice 3's pg_dump story its punch ("one dump = everything"). The user-facing attach action is incremental polish on top of an issue; the differentiator is what backup looks like with attachments included.

### Slice 4 — Contributor onboarding: "Foundry is a project you can join"

- **Stories**: US-13 (contributor clones, runs, ships a change)
- **End-to-end demonstrable value**: A new Rust contributor goes from `git clone` to a green local `cargo test` in 10 minutes, then makes a visible UI change locally.
- **Target outcome KPI**: jtbd-outcome-3 (contributor productive on day one).
- **Dependencies**: Slices 1-3 (the codebase must exist and be self-consistent before it can be onboarded into).
- **Release theme**: "Open-source community on rails."
- **Slice-level sanity check**: Single story, but it produces user-visible value (a green test run + a visible UI change), so it passes the slice-level Elevator Pitch check.

---

## Slice-Level Validation Checklist

Per the nw-product-owner-reviewer Dimension 0 slice-level rule: every slice must contain at least one user-visible value story (i.e., not entirely `@infrastructure`). This is a hard-blocking review check.

| Slice | User-visible stories | Infrastructure-only stories | Pass? |
|-------|----------------------|-----------------------------|-------|
| 1 — Walking Skeleton | US-01, US-05, US-06, US-07, US-08 (5) | 0 | YES |
| 2 — Linear-feel Hot Path | US-09, US-10, US-12 (3) | 0 | YES |
| 3 — Operator-grade | US-02, US-03, US-04, US-11 (4) | 0 | YES |
| 4 — Contributor onboarding | US-13 (1) | 0 | YES |

No slice is all-infrastructure. No re-slicing required.

---

## Priority Rationale

Order: Slice 1 → Slice 2 → Slice 3 → Slice 4.

1. **Slice 1 first (Walking Skeleton)** — Validates the riskiest assumption (does the Postgres-for-everything boring monolith actually deliver an under-an-hour install?). Without this, no other slice has anything to stand on.
2. **Slice 2 second (Linear-feel)** — JTBD outcome #4 (Linear-feel) is the *differentiator* against existing OSS trackers. Shipping Slice 1 alone produces a working but boring tracker; Slice 2 turns it into "the OSS Linear." This is where evaluators decide to demo to their team.
3. **Slice 3 third (Operator-grade)** — Production hardening only matters once the product is worth running in production. Shipped before Slice 2, it would build resilience for an underwhelming product. Shipped after, it earns trust for upgrading from "I'll try this in a dev VM" to "I'll run this for my 80-person team."
4. **Slice 4 last (Contributor onboarding)** — The codebase must be stable and self-consistent before inviting contributors. Earlier contributor onboarding risks PRs against an in-flux architecture. JTBD outcome #3 has high opportunity score (14) but its value compounds only after Slices 1-3 establish a worth-contributing-to project.

Tie-breaking applied: Walking Skeleton beats riskiest-assumption beats highest-value-outcome (per Maurya's prioritization in nw-user-story-mapping). Slice 1 is the skeleton; Slice 2 carries the riskiest "Linear-feel parity" assumption; Slice 3 carries the highest-value operator outcomes once the product earns the right to be operated.

---

## Notes on Story Granularity

- All 13 stories are individually right-sized (S, S-M, or M; 1-3 days, 3-7 scenarios).
- No story currently spans multiple slices. If a story did, it would be split.
- The slice boundaries assume a 2-Rust-developer team can ship one slice per 2-3 weeks, putting the full MVP at the 8-12 week target from the DIVERGE recommendation.
