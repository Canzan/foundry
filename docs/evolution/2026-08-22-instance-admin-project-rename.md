# Evolution — instance-admin-project-rename (fix a project's name from the dashboard, not from psql)

**Finalized**: 2026-08-22
**Commits**: DISCUSS+DESIGN+DISTILL docs landed with `39890ad`; DELIVER `eb244e1` → `3e81d74` (6
DES-monitored TDD steps across 3 slices), refactor `39890ad`/`8fb0dd0`/`70d74b1` (L1/L2/L4; L3/L5/L6
no-change with rationale), mutant-killers `3fa6095`, mutation report `f615449`. Trunk-based; DES
integrity exit 0; adversarial review APPROVED (0 findings); mutation **24/24 viable killed (100%)**.
Feature dir PRESERVED. **Not pushed.**
**Scope**: instance super-admins see every project in the instance grouped by workspace on
`/admin/instance/workspaces` and rename a project's display name in place (htmx row swap, `_csrf`,
422s inline in the row). Slug, board/report URLs, `key_prefix`, and issue keys are byte-immutable.
**No new migration** (schema stays `0014`), no new dependency, no fake.

## Business context

Priya Raman, the self-hosting instance operator, could only fix a stale project name with
`UPDATE projects SET name…` in a production `psql` session — outside every authz, CSRF, and
validation rail the app has, and (worse) without any way to know from the schema that a rename
would have broken her boards. This feature makes the rename a product action: find the project on
the dashboard, retype the name, done — boards intact.

## Key decisions (D1–D7)

- **D1** — rename changes the display name only; slug/URLs/key_prefix/issue keys immutable.
- **D2 (premise correction)** — the brief assumed a name-only rename was safe, but
  `build_board_page` re-derived slugs from *names* at render time, so a rename would have killed
  every issue-card edit/state URL on the board. DISCUSS pinned the observable behavior ("board
  interactions keep working"), DESIGN owned the fix (ADR-PROJECT-RENAME-001), and DELIVER landed it
  as an enabling refactor (`8e3edcc`): request-path slugs threaded through `BoardLocation`,
  `slugify` moved to `foundry-core` as the single production definition, and a new `check-arch`
  rule fails the build if `fn slugify(` reappears under `crates/foundry-app/src`.
- **D3** — instance-wide listing grouped by workspace on the existing dashboard; no
  pagination/search (homelab scale).
- **D4** — trim; ≤256 chars (issues.title precedent); team-scoped uniqueness = case-insensitive
  name match OR slugified-name collision with a stored slug, excluding self; self-rename is a
  quiet no-op. Implemented as `classify_rename` in `foundry-services` (ADR-PROJECT-RENAME-002).
- **D5** — authz refusals are the byte-identical uniform 404 on listing and rename; the htmx POST
  carries `_csrf` and mounts under the CSRF middleware + session layer.
- **D6** — validation failures are 422 + bare fragment routed into the row's `[data-error-slot]`
  by the established `form-errors.js` contract.
- **D7** — residual accepted and recorded: the *create* path can still create a duplicate name
  post-rename (checks slug collision only); deliberately not "fixed" silently.

## Steps completed (6/6, execution-log.json)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | Instance-wide project listing under each workspace + empty state | `eb244e1` |
| 02-01 | D2 enabler: request-path slugs, single production slugify, check-arch rule | `8e3edcc` |
| 02-02 | Rename write path: store queries, `rename_project` use-case, submit handler | `cd97471` |
| 02-03 | End-to-end proof: board survival, self-recase, browser row swap | `ad359b1` |
| 03-01 | Every validation refusal arm with exact copy, 256/300 boundary pair | `cf4de86` |
| 03-02 | Browser lane: refused rename explains itself inside the editing row | `3e81d74` |

Post-merge gate: fresh 21/21 scenarios (132/132 steps), 2026-08-22 — 19 HTTP + 2 `@needs-browser`.

## Lessons

1. **The browser lane caught a defect the HTTP lane was byte-blind to.** In 03-02 the row's
   rename form inherited htmx `hx-swap="outerHTML"`, so the first error swap consumed the
   `[data-error-slot]` element itself — one message, then silence on every later refusal (the
   exact "form does nothing" class this feature exists to kill). Fixed with a
   `data-error-target` child-selector in the row partial so swaps replace the slot's contents,
   never the slot; `form-errors.js` byte-unchanged. HTTP status/fragment assertions can never see
   this; keep funding the `@needs-browser` lane for DOM-routing contracts.
2. **A 256-boundary off-by-one mutant was invisible to proptest sampling.** cargo-mutants
   surfaced a surviving `>` → `>=` on the length limit: the `1..400` proptest range never
   guarantees sampling exactly 256, so the property suite passed over the mutant. Killed with an
   exact 256/257 example pair. Property tests need deterministic boundary examples beside them.
3. **chromedriver/Chrome host skew bites.** The DISTILL sandbox's driver was non-functional
   (probe-then-refuse, never a silent skip), and one transient "chrome not reachable" flake
   appeared during mutation lane runs; both resolved by pinning chromedriver and Chrome to
   matching major 151 on the host. The `cargo xtask ci` driver preflight is the guard.
4. **Mutation tooling notes** (mutation-report.md): cargo-mutants 25.3.1's `--test-package`
   resolves back to the mutated package, so the 7 handler-level mutants were verified by hand
   against the acceptance lane; testcontainers baselines flake under parallel mutant workdirs —
   run container-layer passes `-j 1`.

## Measured KPI baselines (no kpi-contracts.yaml in this repo — recorded here)

- **KPI-1** (renames via UI, <60s): the UI rename path now exists and the full journey —
  dashboard → retype → row swap — is demonstrated in a real browser (scenario #20) well inside
  60s; baseline before this feature was 0% (no path existed).
- **KPI-2** (0 broken board URLs after a rename): pinned by scenario #5 — board and report serve
  at the pre-rename URLs, retitled, with issue AUTH-7's key and card actions intact.
- **KPI-3** (100% of tested 422s render inline): pinned by scenario #21 — every refusal shows
  its reason inside the submitting row's `[data-error-slot]`, including the resubmit-after-fix
  recovery leg.

## Permanent artifacts

- `docs/product/architecture/adr-project-rename-001-request-slugs-not-derived.md`
- `docs/product/architecture/adr-project-rename-002-rename-write-placement.md`
- `docs/product/architecture/brief.md` — "Names are labels; slugs are identity" invariant section
- `docs/product/jobs.yaml` — `job-instance-project-rename`
- `docs/product/outcomes/registry.yaml` — OUT-1 (rename, slug-stable), OUT-2 (instance-wide listing)
- `docs/feature/instance-admin-project-rename/` — full wave history incl.
  `feature-delta.md` (DISCUSS/DISTILL/DELIVER) and `deliver/mutation/mutation-report.md`

## Open / deferred

- **D7 residual** — create path can still mint a duplicate display name post-rename.
- Slug changes/redirects, `key_prefix` changes, workspace/team rename, member-facing rename,
  rename audit events, list pagination/search — all explicitly out of scope (DISCUSS).
- **TOCTOU race** on the uniqueness check-then-write — accepted at homelab scale
  (data-models §4); the `rows_affected` guard is mutation-pinned.
