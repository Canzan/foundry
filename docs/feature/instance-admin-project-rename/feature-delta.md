<!-- markdownlint-disable MD024 -->
# Feature Delta: instance-admin-project-rename

Instance super-admins can see every project in the instance from the instance
dashboard and correct a project's display name in place — without opening a
production `psql` session.

## Wave: DISCUSS

### [REF] Prior Wave Consultation

| Artifact | Status | Note |
|---|---|---|
| `docs/product/jobs.yaml` | ✓ | Read. One prior job (`job-sso-signin`); new job appended by this wave. |
| `docs/product/vision.md` | ⊘ | Does not exist (`docs/product/architecture/brief.md` exists but is DESIGN-scoped). |
| `docs/product/journeys/` | ⊘ | Directory does not exist. |
| `docs/project-brief.md` | ⊘ | Does not exist. |
| `docs/stakeholders.yaml` | ⊘ | Does not exist. |
| DISCOVER artifacts for this feature | ⊘ | None. Noted as risk: job grounded in code + operator context, not interviews. |
| DIVERGE artifacts for this feature | ⊘ | None. JTBD run inside this wave (below). |

### [REF] Persona

**Priya Raman — instance super-admin / self-hosting operator.** Runs foundry on
her own cluster alongside Grafana/Portainer/ArgoCD. She provisioned the
workspaces herself (`/admin/instance/workspaces`), holds an `instance_admins`
row, and is comfortable with a terminal — which is exactly why she resents that
the *only* way to fix a stale project name today is `UPDATE projects SET name…`
against the production database. She is a browser human on this surface: the
shipped dashboard already gates her via `require_instance_admin` and the
double-submit `_csrf` contract.

- Persona ID: `persona-instance-operator` (same operator persona as the
  provisioning surface; no separate personas file exists in this repo).

### [REF] JTBD

**job_id: `job-instance-project-rename`** (appended to `docs/product/jobs.yaml`)

One-liner: *When a project's purpose has drifted from the name it was created
with, I want to see every project in the instance and correct its display name
in place, so boards and reports stay legible without database surgery.*

All three stories below trace N:1 to this job.

### [REF] Locked Decisions

| ID | Decision | Rationale / source |
|---|---|---|
| D1 | Rename changes the **display name only**. The stored `projects.slug`, all board/report URLs, `key_prefix`, and issue keys (`AUTH-7`) are unchanged. | `projects` has distinct `name` and `slug` columns (`0001_init.sql:51-62`); slug is the URL identity (`find_project_by_slug`). Brief default confirmed viable. |
| D2 | **Premise correction (code contradicts brief assumption):** the board render path re-derives `project_slug`/`team_slug` from *names* at render time (`projects.rs::build_board_page` → `slugify(&project.name)`), so a name-only rename would break every issue-card edit/state URL on the board. The requirement pins the observable behavior — *board interactions keep working after a rename* (US-02 AC) — and the slug-derivation correction is a DESIGN-owned prerequisite of US-02. | Found while grounding stories in `crates/foundry-app/src/projects.rs:861-908`. |
| D3 | Listing is **instance-wide, grouped by workspace** on the existing dashboard (`GET /admin/instance/workspaces`), reusing the workspace list already rendered there. No pagination or search — homelab-scale instance (handful of workspaces, tens of projects). | Dashboard already lists workspaces; `list_projects_for_workspace` read exists (`foundry-store/src/lib.rs:708`) but lacks the project id the rename write needs — a listing read that includes ids is a driving-port delta. |
| D4 | Validation: trimmed name required non-empty; max **256 characters** (mirrors the `issues.title` CHECK precedent — `projects.name` has no DB CHECK today); rename is rejected when, within the same team, the trimmed new name case-insensitively equals another project's name **or** `slugify(new name)` equals another project's stored slug; renaming a project to its own current name is a no-op success. | Create path enforces name-uniqueness-within-team via slug (`projects.rs:178-194`); a name-only rename must preserve that invariant by direct check. |
| D5 | Authorization mirrors the shipped surface exactly: a signed-out caller or signed-in non-super-admin gets the **uniform non-enumerable 404** (`resource_not_found_page`) on both the listing and the rename POST. The rename form is an htmx-driven mutating trigger and **must carry the `_csrf` body field** (this repo has been burned twice by missing CSRF on htmx actions). | `instance_admin.rs::require_instance_admin`, ADR-002 / G5. |
| D6 | Validation failures return **422 + a bare error fragment** routed into the row's `[data-error-slot]` by `form-errors.js` (the established 400–499 client contract); the form stays mounted so Priya corrects and resubmits without a reload. | `static/js/form-errors.js`, `projects.rs` 422 idiom. |
| D7 | Residual gap accepted: after a rename, the *create* path could still create a second project whose name equals the renamed project's new name (create checks slug collision only). Changing the create path is OUT of scope; recorded so DESIGN does not silently "fix" it. | `projects.rs:178-194`. |

### [REF] Journey (lightweight, happy path)

Emotional arc: **Problem Relief** — irritated (stale name, only fix is SQL) →
focused (finds the project on the dashboard) → relieved (name corrected, board
intact).

```text
[Trigger]                    [Step 1]                       [Step 2]                        [Goal]
"Auth v2" shipped; the  →   Priya opens                →   Clicks Rename on the       →   Row swaps to "Identity
name is now wrong           /admin/instance/workspaces     "Auth v2" row, types           Platform (AUTH)";
                            and sees each workspace's      "Identity Platform",           board at /team/backend/
Feels: irritated            projects listed under it       submits (htmx + _csrf)         project/auth-v2 still
("psql again?")             Feels: focused                 Feels: momentary tension       works, retitled.
                            Sees: data-project-row         ("will boards break?")         Feels: relieved
                            per project                                                   Sees: updated row, no reload
```

Error path (US-03): invalid name → 422 fragment appears inline in the row's
`[data-error-slot]`; old name stays everywhere; Priya fixes and resubmits.

### [REF] Scope Assessment: PASS — 3 stories, 1 bounded context, estimated 2.5 days

Signals checked: 3 stories (≤10) | single module cluster (`foundry-app`
instance-admin surface + one store read/write) | no walking skeleton needed
(isolated extension of a shipped surface) | ~2.5 days total | one user outcome.
No oversized signal fired.

### [REF] Shared Artifacts

| Artifact | Source of truth | Consumers | Risk |
|---|---|---|---|
| Project display name | `projects.name` column | instance-dashboard project row (new), board heading (`BoardPage.project_name`), report heading (`ReportPage.project_name`), create-path uniqueness check | HIGH — rename must propagate to all read surfaces on next render |
| Project slug | `projects.slug` column | board/report URLs, `find_project_by_slug`, CSV export filename — **and today, incorrectly, re-derived from name in `build_board_page`** (D2) | HIGH — must stay stable across rename |
| CSRF token | `foundry_csrf` cookie + hidden `_csrf` field | every mutating form on the dashboard incl. the new rename form | HIGH — missing `_csrf` on the htmx POST = silent 403 |
| Uniform 404 page | `resource_not_found_page` | listing GET + rename POST for non-admins | MEDIUM — any divergent error shape becomes an enumeration oracle |

### [REF] User Stories

#### US-IAPR-01: See every project in the instance at a glance

##### Elevator Pitch

- **Before:** Priya must run `psql` SELECTs to know which projects exist across her instance's workspaces.
- **After:** opening `/admin/instance/workspaces` shows, under each workspace entry, that workspace's projects — e.g. "Auth v2 (AUTH) — team Backend" — each as a `data-project-row` element.
- **Decision enabled:** which stale-named project (and in which workspace) to rename — chosen from the page, not from a database session.

##### Problem

Priya is the instance super-admin of a foundry instance with three workspaces.
When she wants to know what projects exist — or find the one whose name has
gone stale — she finds it tedious and risky to open a production `psql` session
just to `SELECT name, slug FROM projects`.

##### Domain Examples

1. **Happy path** — Instance has workspaces "Canzan Labs" and "Bailey Family". "Canzan Labs" contains projects "Auth v2" (AUTH, team Backend) and "Sandbox" (SBX, team Backend); "Bailey Family" contains "Chores" (CHR, team Home). Priya opens the dashboard and sees both workspaces with their projects nested beneath, ordered by name.
2. **Edge** — Workspace "Bailey Family" has no projects yet: its section shows an explicit "No projects yet." empty state (mirrors the existing `data-workspace-empty` idiom), not a blank gap.
3. **Boundary/authz** — Marco, a signed-in workspace member who is *not* an instance admin, requests `/admin/instance/workspaces` and receives the byte-identical uniform 404 — the project list leaks to no one.

##### UAT Scenarios (BDD)

###### Scenario: Every project in the instance is visible under its workspace

- Given Priya is signed in and holds an `instance_admins` row
- And workspace "Canzan Labs" contains projects "Auth v2" (AUTH) and "Sandbox" (SBX)
- And workspace "Bailey Family" contains project "Chores" (CHR)
- When Priya opens the instance dashboard
- Then she sees "Auth v2" and "Sandbox" listed under "Canzan Labs" and "Chores" under "Bailey Family"
- And each project row shows the display name, key prefix, and owning team name

###### Scenario: A workspace with no projects says so

- Given workspace "Bailey Family" contains no projects
- When Priya opens the instance dashboard
- Then the "Bailey Family" section shows a "No projects yet." message

###### Scenario: The project list is invisible to non-admins

- Given Marco is signed in but is not an instance admin
- When Marco requests the instance dashboard URL
- Then he receives the same uniform 404 page a never-existed path returns

##### Acceptance Criteria

- [ ] The instance dashboard lists every project in the instance, grouped under its workspace, ordered by name within each workspace, each row carrying the display name, key prefix, and team name.
- [ ] A workspace with zero projects renders an explicit empty state.
- [ ] Signed-out and non-admin requests receive the uniform non-enumerable 404 (unchanged from today).

##### Size

0.5 day | 3 scenarios | job_id: `job-instance-project-rename`

#### US-IAPR-02: Correct a project's display name without breaking its board

##### Elevator Pitch

- **Before:** fixing a stale project name means typing `UPDATE projects SET name = …` into production psql, outside every authz and CSRF rail the app has.
- **After:** clicking Rename on the "Auth v2" row, typing "Identity Platform", and submitting swaps the row (htmx, no reload) to show "Identity Platform (AUTH)" — and the board at `/team/backend/project/auth-v2` still loads, now titled "Identity Platform", with every issue card still clickable.
- **Decision enabled:** whether the rename has fully taken effect (dashboard row + board title) or needs another correction — judged from the page, not from SQL output.

##### Problem

Priya named a project "Auth v2" during a migration; the project shipped and
became the team's permanent identity platform. Every board and report now leads
with a name that confuses new team members. Her only current fix is direct SQL
against production — no validation, no authz, and (per D2) she cannot even tell
from the schema that the board's issue-card URLs are re-derived from the name.

##### Domain Examples

1. **Happy path** — Priya renames "Auth v2" (team Backend, workspace Canzan Labs) to "Identity Platform". The dashboard row updates in place; the board URL `/team/backend/project/auth-v2` still serves the board, now headed "Identity Platform"; issue AUTH-7 keeps its key and its edit dialog still opens.
2. **Edge (no-op)** — Priya submits the unchanged name "Sandbox" for project "Sandbox": the row confirms success with no visible change and no error.
3. **Error/authz** — Marco (signed-in, not an instance admin) forges the rename POST for "Auth v2": he receives the uniform 404 and the name is untouched.

##### UAT Scenarios (BDD)

###### Scenario: A stale project name is corrected from the dashboard

- Given Priya is on the instance dashboard and project "Auth v2" (AUTH, team Backend, workspace Canzan Labs) exists
- When she submits the rename form on that row with the name "Identity Platform"
- Then the row shows "Identity Platform" without a full page reload
- And reloading the dashboard still shows "Identity Platform"

###### Scenario: Boards, URLs, and issue keys survive a rename

- Given issue AUTH-7 "Refresh token rotation" exists on the board at `/team/backend/project/auth-v2`
- When Priya renames the project "Auth v2" to "Identity Platform"
- Then `/team/backend/project/auth-v2` still serves the board, now titled "Identity Platform"
- And issue AUTH-7 keeps its key and its edit and state actions still work
- And the change report at `/team/backend/project/auth-v2/report` shows the new name

###### Scenario: Renaming to the current name is a quiet success

- Given project "Sandbox" (SBX) exists
- When Priya submits the rename form with the unchanged name "Sandbox"
- Then the row confirms success showing "Sandbox" and no error appears

###### Scenario: Only instance admins can rename

- Given Marco is signed in but is not an instance admin
- When Marco sends the rename request for "Auth v2" directly
- Then he receives the same uniform 404 page a never-existed path returns
- And the project is still named "Auth v2"

##### Acceptance Criteria

- [ ] Each project row on the instance dashboard carries a rename form (htmx POST with the hidden `_csrf` field) that updates the display name and swaps the row in place on success.
- [ ] After a rename, the stored slug, board URL, report URL, key prefix, and existing issue keys are byte-identical to before; the board and report render the new name and all issue-card actions still function (D1/D2).
- [ ] Submitting the current name unchanged succeeds as a no-op (D4).
- [ ] Signed-out/non-admin rename POSTs receive the uniform non-enumerable 404 and change nothing; a POST without a valid `_csrf` pair is refused by the CSRF middleware before the handler runs (D5).

##### Size

1 day | 4 scenarios | job_id: `job-instance-project-rename`

#### US-IAPR-03: A bad rename explains itself inline

##### Elevator Pitch

- **Before:** an invalid rename via SQL either silently succeeds into a broken state (empty name, duplicate) or fails with a Postgres error Priya must decode.
- **After:** submitting an empty name in the "Auth v2" row shows "Project name must not be empty" inside that row's `[data-error-slot]` without a page reload; the old name remains everywhere.
- **Decision enabled:** exactly how to fix the input (retype and resubmit in place) versus abandon the rename — decided from the inline message.

##### Problem

Priya is renaming quickly across several projects. When she fat-fingers an
empty submit, pastes a 300-character title, or picks a name another Backend
project already uses, she finds it disorienting when a form silently does
nothing (the htmx 4xx-discard defect this repo already fixed once with
`form-errors.js`) — she needs the reason in the row she is working in.

##### Domain Examples

1. **Empty** — Priya clears the field on the "Auth v2" row and submits: 422; "Project name must not be empty" appears in the row's error slot; the name stays "Auth v2".
2. **Over-long** — Priya pastes a 300-character pitch as the name: 422; "Project name must be at most 256 characters"; nothing changes.
3. **Duplicate** — Team Backend already has "Sandbox"; Priya renames "Auth v2" to "Sandbox" (and, boundary: to "sandbox"): 422; "Project name must be unique within the team"; both projects keep their names.

##### UAT Scenarios (BDD)

###### Scenario: An empty name is refused with an inline reason

- Given Priya is on the instance dashboard row for "Auth v2"
- When she submits the rename form with an empty (or whitespace-only) name
- Then the row's error slot shows "Project name must not be empty" without a page reload
- And the project is still named "Auth v2" on the dashboard, board, and report

###### Scenario: An over-long name is refused with the limit stated

- Given Priya is renaming "Auth v2"
- When she submits a 300-character name
- Then the row's error slot shows the 256-character limit message
- And the project name is unchanged

###### Scenario: A name already used in the team is refused

- Given team Backend contains projects "Auth v2" and "Sandbox"
- When Priya renames "Auth v2" to "Sandbox"
- Then the row's error slot shows "Project name must be unique within the team"
- And both projects keep their original names

###### Scenario: Case and punctuation do not dodge the uniqueness rule

- Given team Backend contains projects "Auth v2" and "Sandbox"
- When Priya renames "Auth v2" to "sandbox"
- Then the rename is refused with the same uniqueness message (D4: case-insensitive name match and slug-collision check)

##### Acceptance Criteria

- [ ] Empty/whitespace-only, >256-character, and within-team duplicate names (case-insensitive name match OR slugified-name collision with another project's stored slug, excluding the project itself) are refused with HTTP 422 and a bare error fragment (D4).
- [ ] The fragment is routed into the submitting row's `[data-error-slot]` by the established `form-errors.js` contract; the form stays mounted and resubmittable without a reload (D6) — verified in the `@needs-browser` lane.
- [ ] On every refused rename, the persisted name is unchanged on all read surfaces.

##### Size

1 day | 4 scenarios | job_id: `job-instance-project-rename`

### [REF] System Constraints

- Mutating htmx triggers MUST carry the `_csrf` body field and mount under the CSRF middleware + session layer (NOT the CSRF-exempt `/api/v1` mount).
- Authz failures on this surface are the uniform non-enumerable 404 — never 401/403/redirect (ADR-002 idiom).
- Validation failures are 400/422 + bare fragment into `[data-error-slot]`; success fragments are bare (no `base.html` extension — double-wrap hazard).
- The instance-admin module is instance-scoped by design (LAYER-1e allow-list); the new listing/rename reads-writes legitimately cross workspaces but must never be reachable by non-super-admins.
- Test lanes: HTTP acceptance lane for status/fragment contracts; fantoccini `@needs-browser` lane for the error-slot swap (the HTTP lane is blind to it — see form-errors.js RCA).

### [REF] Outcome KPIs

Objective: stale project names get corrected through the product, safely, so
the operator never opens a production SQL session for a rename again.

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| 1 | Instance super-admins | Complete a project rename via the dashboard instead of psql | 100% of renames via UI, each under 60s | 0% (no UI path exists) | Server logs: rename POST count; operator self-report (single-operator instance) | Leading |
| 2 | Workspace members | Encounter zero broken board/issue interactions after a rename | 0 broken URLs or dead issue-card actions | Unknown (rename today would break card URLs — D2) | Acceptance suite (US-IAPR-02 scenario 2) + error-rate on board routes post-rename | Guardrail |
| 3 | Instance super-admins | Recover from a refused rename inline, without a reload | 100% of 422s render a visible reason in the row | 0% (no form exists; htmx discards 4xx without the slot) | `@needs-browser` acceptance lane; absence of "form does nothing" reports | Guardrail |

Homelab-scale honesty: this is a single-digit-operator instance; KPIs 1–3 are
verified primarily by the acceptance suite and logs, not analytics tooling.

### [REF] DoD

- All UAT scenarios green in the HTTP lane; error-slot behavior green in the `@needs-browser` lane.
- `check_arch.rs` tenant-scoping checks pass (instance_admin stem stays on the allow-list; no new tenant-scoped module parses workspace ids from requests).
- No new route responds differently to "exists but forbidden" vs "never existed".
- Rename round-trip demonstrated: dashboard rename → board + report render new name at unchanged URLs.
- Merged to main; migrations (if any — none expected) applied cleanly.

### [REF] Out of Scope

- Project deletion or archival.
- Slug changes, URL redirects, or slug regeneration on rename (D1) — a future "change project URL" feature if ever wanted.
- `key_prefix` changes (issue keys are permanent identity).
- Workspace rename, team rename.
- Rename access for workspace/team admins (this is the *instance* surface only; a member-facing rename is a separate feature with different authz).
- Pagination or search of the project list (homelab scale, D3).
- Audit/event-log entry for project renames (the change-events table is issue-scoped today).
- Closing the D7 create-path duplicate-name residual.

### [REF] WS Strategy

No walking skeleton (locked): this is an isolated extension of the shipped
instance-admin surface. Delivery order is US-IAPR-01 → 02 → 03; each slice is
independently demonstrable on the live dashboard.

### [REF] Driving Ports

New behavior the web adapter needs from the core (DESIGN owns shapes/placement):

1. **Instance-wide project listing read** — per workspace: project id, name, key prefix, team name (the existing `list_projects_for_workspace` lacks the id and team display name; instance-wide iteration vs. new query is a DESIGN choice).
2. **Project rename write** — by project id: set `name` only, leaving `slug`/`key_prefix` untouched; enforcing D4 uniqueness (check-then-write acceptable at this scale, race noted).
3. **Board slug-derivation correction (D2 prerequisite)** — board/report rendering must stop re-deriving URL slugs from names (`build_board_page`'s `slugify(project.name)` / `slugify(team_name)`) and use the stored/request slugs, or US-IAPR-02 AC-2 cannot pass.

### [REF] Pre-requisites

None outstanding: the instance dashboard, `require_instance_admin` gating, CSRF
middleware, `form-errors.js`, and both test lanes are all shipped. The only
internal prerequisite is D2 (slug-derivation correction), sequenced inside
US-IAPR-02.

### [REF] DoR Validation

| DoR Item | US-IAPR-01 | US-IAPR-02 | US-IAPR-03 | Evidence |
|---|---|---|---|---|
| 1. Problem in domain language | PASS | PASS | PASS | Each Problem section names Priya's concrete pain (psql for a name fix) |
| 2. Persona specific | PASS | PASS | PASS | Priya Raman, instance super-admin with `instance_admins` row; Marco as non-admin foil |
| 3. 3+ domain examples, real data | PASS | PASS | PASS | Auth v2/AUTH, Sandbox/SBX, Chores/CHR, AUTH-7, Canzan Labs / Bailey Family |
| 4. UAT 3–7 scenarios G/W/T | PASS (3) | PASS (4) | PASS (4) | Embedded above; business-outcome titles |
| 5. AC derived from UAT | PASS | PASS | PASS | Each AC maps to ≥1 scenario |
| 6. Right-sized | PASS 0.5d | PASS 1d | PASS 1d | ≤1 day per slice, 3–4 scenarios each |
| 7. Technical notes/constraints | PASS | PASS | PASS | System Constraints + Driving Ports + D-decisions |
| 8. Dependencies tracked | PASS | PASS | PASS | 02 depends on 01 (row markup) + D2 correction; 03 depends on 02 |
| 9. Outcome KPIs measurable | PASS | PASS | PASS | KPI table with baseline + measurement method |

DoR Status: **PASSED** (all 9 items, all 3 stories). Peer review by
`nw-product-owner-reviewer` not invoked in this lean subagent run — the
orchestrator gates handoff.

## Wave: DISTILL

Acceptance designer: Quinn (nw-acceptance-designer) | Date: 2026-08-22
`[lang-mode] rust` | `[policy-mode] inherit` (`docs/architecture/atdd-infrastructure-policy.md` — row appended for the rename surface)

### [REF] Prior Wave Consultation

| Artifact | Status | Note |
|---|---|---|
| feature-delta DISCUSS (US-IAPR-01..03, D1-D7, KPIs) | ✓ | Scope boundary for every scenario |
| slices 01-03 | ✓ | Scenario-to-slice mapping below |
| design/{architecture-design, component-boundaries, data-models, technology-stack}.md | ✓ | Routes, port signatures, htmx row-swap contract, D2 fix, 422 copy verbatim |
| ADR-PROJECT-RENAME-001/002 | ✓ | Slug-capture rule + rename-write placement honoured by the step design |
| docs/product/jobs.yaml (job-instance-project-rename) | ✓ | All scenarios trace N:1 to the job |
| docs/product/outcomes/registry.yaml | ✓ | Was empty; OUT-1/OUT-2 registered (below) |
| docs/product/journeys/ | ⊘ | Does not exist — journey read from DISCUSS section instead |
| docs/product/kpi-contracts.yaml | ⊘ | Does not exist — no `@kpi` scenarios; DISCUSS KPI table notes the acceptance suite IS the primary KPI-2/KPI-3 instrument |
| wave-decisions.md (discuss/design/devops), DEVOPS artifacts | ⊘ | Lean contract — reconciliation ran against the feature-delta + design docs |

**Reconciliation passed — 0 contradictions** (D1-D7 each confirmed against DESIGN; D2's premise correction is *adopted* by DESIGN, not contradicted; D7 residual recorded, not silently fixed).

### [REF] Scenario Table

Feature file: `crates/foundry-acceptance/tests/features/instance-admin-project-rename.feature` (SSOT, 21 scenarios, ALL `@pending` per-scenario — DELIVER un-pends one at a time). Steps: `crates/foundry-acceptance/src/steps/feature_instance_admin_project_rename.rs`.

| # | Scenario | Slice | Lane | Tags (beyond @iapr @pending) | RED classification (verified run, @pending stripped) |
|---|---|---|---|---|---|
| 1 | Every project in the instance is listed under its workspace | 01 | HTTP | @us-iapr-01 @driving_port @real-io | MISSING_FUNCTIONALITY — no `data-project-row` on shipped dashboard |
| 2 | A workspace with no projects says so | 01 | HTTP | @us-iapr-01 @edge | MISSING_FUNCTIONALITY — no empty-state marker |
| 3 | The project list is invisible to anyone who is not the instance admin | 01 | HTTP | @us-iapr-01 @error @security | ALREADY-GREEN — shipped `require_instance_admin` gate; kept as the regression pin that the richer page adds no oracle |
| 4 | A stale project name is corrected from the dashboard | 02 | HTTP | @us-iapr-02 @driving_port @real-io | MISSING_FUNCTIONALITY — 501 scaffold where 200 row fragment expected |
| 5 | Boards, addresses, and issue keys survive a rename | 02 | HTTP | @us-iapr-02 @driving_port @real-io | MISSING_FUNCTIONALITY — board serves but is not retitled (rename unimplemented); D2 card-URL assertions armed against STORED slugs |
| 6 | Renaming a project to its current name is a quiet success | 02 | HTTP | @us-iapr-02 @edge | MISSING_FUNCTIONALITY — 501 vs 200 no-op fragment |
| 7 | Only the instance admin can rename a project | 02 | HTTP | @us-iapr-02 @error @security | MISSING_FUNCTIONALITY — 501 vs byte-identical uniform 404 |
| 8 | A rename that does not carry the dashboard's matching token is refused | 02 | HTTP | @us-iapr-02 @error @security | MISSING_FUNCTIONALITY — middleware 403 leg already holds; fails on the dashboard-name propagation (listing absent) |
| 9 | A signed-out visitor cannot rename anything | 02 | HTTP | @us-iapr-02 @error @security | MISSING_FUNCTIONALITY — 501 vs uniform 404 |
| 10 | An empty name is refused with the reason stated | 03 | HTTP | @us-iapr-03 @error | MISSING_FUNCTIONALITY — 501 vs 422 + "Project name must not be empty" |
| 11 | A name of only spaces counts as empty | 03 | HTTP | @us-iapr-03 @error @edge | MISSING_FUNCTIONALITY — same |
| 12 | A name past the length limit is refused with the limit stated | 03 | HTTP | @us-iapr-03 @error | MISSING_FUNCTIONALITY — 501 vs 422 + 256-limit copy |
| 13 | A name of exactly the length limit is accepted | 03 | HTTP | @us-iapr-03 @edge | MISSING_FUNCTIONALITY — 501 vs 200 (boundary: 256 accepted) |
| 14 | A name another project in the team already uses is refused | 03 | HTTP | @us-iapr-03 @error | MISSING_FUNCTIONALITY — 501 vs 422 + uniqueness copy |
| 15 | Changing the letter case does not dodge the uniqueness rule | 03 | HTTP | @us-iapr-03 @error | MISSING_FUNCTIONALITY — same (case-insensitive arm of D4) |
| 16 | A name that collides with another project's address is refused | 03 | HTTP | @us-iapr-03 @error | MISSING_FUNCTIONALITY — same (slugify-collision arm of D4: "Sandbox!" → slug "sandbox") |
| 17 | Re-casing a project's own name is a valid rename | 03 | HTTP | @us-iapr-03 @edge | MISSING_FUNCTIONALITY — 501 vs 200 (D4 self-exclusion contract pin) |
| 18 | A rename aimed at a garbled project id is answered like a missing page | 03 | HTTP | @us-iapr-03 @error @security | MISSING_FUNCTIONALITY — 501 vs uniform 404 (no 400 oracle) |
| 19 | A rename aimed at a project that does not exist is answered like a missing page | 03 | HTTP | @us-iapr-03 @error @security | MISSING_FUNCTIONALITY — same |
| 20 | The row updates in place when the rename succeeds | 02 | @needs-browser | @us-iapr-02 @needs-browser @driving_port @real-io | MISSING_FUNCTIONALITY (structural: no rename form exists in any row). Local run refused at the chromedriver readiness probe (driver non-functional in this sandbox — probe-then-refuse, never a skip); `cargo xtask ci` preflights the driver in the @all lane |
| 21 | A refused rename explains itself inside the row being edited | 03 | @needs-browser | @us-iapr-03 @needs-browser @error @real-io | Same as #20; recovery leg (correct + resubmit) reuses #20's steps |

Counts: 21 scenarios; 13 `@error`/`@security` (62% ≥ 40% target); 4 `@edge`. Verified run (with `@pending` stripped): 19 HTTP scenarios → 18 assertion-level failures + 1 already-green; 0 IMPORT/FIXTURE/SETUP failures — fail-for-the-right-reason gate PASSED. All 21 re-tagged `@pending`; full suite + workspace unit tests + `check-arch` + fmt + clippy green afterwards (0 regressions).

### [REF] WS Strategy

None — inherited from the DISCUSS lock ("no walking skeleton: isolated extension of the shipped instance-admin surface"). No `@walking_skeleton` tag is used; scenarios 1/4/5 are the demo-able verticals per slice. Delivery order US-IAPR-01 → 02 → 03 (scenario table order).

### [REF] Adapter / Port Coverage

Every port in scope is driving (HTTP) or driven-internal (Postgres) → REAL per the Architecture of Reference; NO fake is introduced (no clock, email, or external dependency in this feature).

| Port (DESIGN component-boundaries.md) | Class | Covered by |
|---|---|---|
| `GET /admin/instance/workspaces` (listing delta) | Driving | #1, #2, #3 (+ propagation legs of #4, #8) — real HTTP, real session |
| `POST /admin/instance/projects/{project_id}/rename` | Driving | #4-#19 (HTTP), #20-#21 (browser); CSRF-middleware leg #8; authz matrix #7/#9/#18/#19 |
| `Store::list_projects_for_instance` | Driven internal (real PG) | #1, #2 (via dashboard render) — @real-io |
| `Store::project_rename_context` / `list_team_sibling_projects` / `update_project_name` | Driven internal (real PG) | #4-#17 (via the rename use-case); post-write DB assertions read the same per-scenario schema |
| `foundry_services::projects::rename_project` (use-case seam) | Internal (exercised through driving port only) | #4-#19 — never invoked directly by a step (Mandate 1) |
| `foundry_core::slugify` (single production definition) | Internal | #16 (slug-collision arm), #5 (absence of render-time re-derivation) |
| D2 `build_board_page` request-slug threading | Internal (render path) | #5 — board + card edit/state URLs asserted against STORED pre-rename slugs; edit dialog actually fetched |
| htmx row swap + `form-errors.js` `[data-error-slot]` routing | Driving (browser) | #20, #21 — the HTTP lane is byte-blind to both (form-errors RCA) |

### [REF] Oracle Discipline

- **Slug capture**: #5's Given reads `(team_slug, project_slug)` from the per-scenario DB BEFORE the rename; the step module contains NO test-local `slugify` (a re-derivation from the new name would go green over the D2 bug).
- **State-delta universe**: every mutating scenario snapshots the full `projects` row `(name, slug, key_prefix, next_issue_number)` pre-write and asserts post-write via `assert_project_delta` — only `name` may move; undeclared drift fails closed. No-op (#6) asserts the row byte-identical.
- **No green-over-nothing**: #20 plants a page-lifetime JS marker and asserts it survives the swap (a full reload would pass a naive "name visible" check); #21 asserts the message INSIDE the submitting row's `[data-error-slot]`, not anywhere on the page.
- **Non-enumerability**: refusals are compared BYTE-IDENTICAL against a freshly-fetched never-existed-path answer, not against a hardcoded body.

### [REF] Scaffolds (Mandate 7, repo variant — commit-ready, `cargo check`/clippy/fmt/unit-tests clean)

| File | Scaffold | RED mechanism |
|---|---|---|
| `crates/foundry-core/src/lib.rs` | `pub fn slugify` | `panic!` marker (nothing calls it yet) |
| `crates/foundry-store/src/lib.rs` | `InstanceProjectRow`, `ProjectRenameContext`, `list_projects_for_instance`, `project_rename_context`, `list_team_sibling_projects`, `update_project_name` | `panic!` markers; pinned SQL in doc comments |
| `crates/foundry-services/src/projects.rs` (+ `pub mod projects;`) | `RenameProjectRequest/Outcome/Error`, `rename_project`, `Services::rename_project` | `panic!` marker; ordered-check contract in doc comment |
| `crates/foundry-app/src/instance_admin.rs` | `RenameForm`, `submit_project_rename` | Returns clean **501** with RED-scaffold body (the `admin_tokens.rs` precedent — a `panic!` would abort the axum connection and mask the assertion) |
| `crates/foundry-app/src/lib.rs` | Route `POST /admin/instance/projects/{project_id}/rename` mounted UNDER `csrf_middleware` + `session_layer` | Route present ⇒ the CSRF-403 and status assertions read real responses |

All carry `SCAFFOLD: true` markers; `grep -rn "SCAFFOLD: true" crates/ --include="*.rs"` tracks DELIVER burn-down. Not scaffolded (deliberately): templates/views (`instance_project_row.html`, dashboard extension) and the D2 signature change — behaviour-changing, DELIVER-owned; the listing scenarios RED on the absent markup instead.

### [REF] Test Placement

`crates/foundry-acceptance/tests/features/*.feature` + `src/steps/feature_*.rs` + `src/world.rs` fields + force-link in `tests/acceptance.rs` — the exact keycloak-sso / web-provisioning-flow precedent (cucumber-rs; per-scenario `@pending` is this repo's skip marker, excluded from every lane). Browser oracle in the same feature file under `@needs-browser` (form-error-display precedent: in `@all`/CI, excluded from the fast default lane).

### [REF] Pre-requisites (for DELIVER)

1. D2 fix (`build_board_page` request-slug threading + `slugify` move to foundry-core + new check-arch rule) is sequenced INSIDE #5 — un-pend #5 only after slice-02's write path lands.
2. #8's dashboard-propagation leg and #4's reload leg depend on slice-01 listing — un-pend in slice order.
3. Browser lane needs a working chromedriver (preflighted by `cargo xtask ci`); the driver in this DISTILL sandbox was non-functional (probe refused — recorded, not skipped).
4. No migration, no new dependency, no fake to build.

### [REF] Outcomes Registry

`docs/product/outcomes/registry.yaml` was present but EMPTY (`outcomes: []`) — no row format to inherit, no `nwave-ai` CLI in this repo. Registered directly in YAML using the nw-distill field vocabulary: **OUT-1** (operation: rename display name, slug-stable invariant noted) and **OUT-2** (operation: instance-wide grouped listing, cross-tenant scope noted).

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DISCUSS#D1 | Rename changes display name only; slug/URLs/key_prefix/issue keys immutable | n/a | Scenario #5 pins it end-to-end; `assert_project_delta` fails closed on any identity-column drift |
| DISCUSS#D2 | Board interactions keep working after a rename (slug-derivation correction prerequisite) | ADR-PROJECT-RENAME-001 | #5 asserts card edit/state URLs against STORED pre-rename slugs and fetches the edit dialog; step module bans test-local slugify |
| DISCUSS#D3 | Instance-wide listing grouped by workspace on the existing dashboard, no pagination | n/a | #1/#2 assert grouping positionally and the explicit empty state; no search/pagination scenario exists |
| DISCUSS#D4 | Trim; ≤256 chars; team-scoped uniqueness (case-insensitive name OR slug collision); self no-op | ADR-PROJECT-RENAME-002 | #10-#17 enumerate every arm incl. both boundary sides (256 accepted, 300 refused) and the self-exclusion recase |
| DISCUSS#D5 | Uniform non-enumerable 404 for every authz refusal; `_csrf` mandatory on the htmx POST | n/a | #3/#7/#9/#18/#19 compare refusals byte-identical to a never-existed path; #8 proves the middleware refuses pre-handler |
| DISCUSS#D6 | 422 + bare fragment into the row's `[data-error-slot]`; form stays mounted | n/a | HTTP lane pins status+fragment+marker (#10-#16); browser lane pins the DOM routing + recovery (#21) |
| DISCUSS#D7 | Create-path duplicate residual stays OUT of scope | n/a | No scenario exercises the create path; TOCTOU race deliberately untested (data-models §4 acceptance) |
| DESIGN#component-boundaries | Port signatures for store/services/handler; malformed id parsed IN handler → 404 | ADR-PROJECT-RENAME-002 | Scaffolds pin the signatures verbatim; #18 pins 404-not-400 |
| DISCUSS#WS-Strategy | No walking skeleton (isolated extension of shipped surface) | n/a | No `@walking_skeleton` tag; slice-ordered un-pending replaces the skeleton-first sequence |
