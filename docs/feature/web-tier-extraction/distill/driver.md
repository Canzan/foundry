# DISTILL Driver Design — Feature A "Programmatic Foundry"

Owner: acceptance-designer (DISTILL). Companions: `step-skeletons.md` (step
signatures + the DELIVER wiring list), `coverage-matrix.md` (AC→scenario trace),
`red-classification.md` (the pre-DELIVER fail-for-the-right-reason gate).

This feature REUSES the existing `foundry-acceptance` cucumber-rs harness wholesale.
There is no new harness; the four `.feature` files are driven by the SAME
`InProcHarness` (in-process axum + testcontainers Postgres + per-scenario schema) the
backend-mvp slices use. This document records only what is NEW for Feature A.

## 1. Harness reuse — no new infrastructure

- **Crate**: `crates/foundry-acceptance` (unchanged). The four Feature-A `.feature`
  files live in `crates/foundry-acceptance/tests/features/` (the canonical location the
  runner scans) and are MIRRORED under `docs/feature/web-tier-extraction/distill/features/`
  for the DISTILL doc set (the same dual-location convention backend-mvp uses).
- **World**: `FoundryWorld` gains a small block of `fa_*` scratch fields (project name,
  credential string + revoked flag, watching flag, browser-signin flag, last HTML body,
  guard violation kind / exit code / stderr). Additive only — no existing field changed.
- **Step module**: `crates/foundry-acceptance/src/steps/feature_a_programmatic.rs`, one
  file for all four stories (cucumber-rs requires globally-unique step phrases; one file
  avoids cross-file collisions, the same one-file-per-concern idiom backend-mvp uses).
  Registered in `lib.rs` (`pub mod feature_a_programmatic;`) and force-linked in
  `tests/acceptance.rs` (`use ... as _feature_a;`).
- **Background reuse**: the workspace / team / project / signed-in Given phrases are
  REUSED verbatim from `us_06_signin.rs` / `us_07_project_create.rs` / `us_08_file_issue.rs`.
  Only Feature-A-specific phrases (read-as-data, machine-credential, API writes, boundary
  check) are declared in the new step file.

## 2. The walking skeleton — how Slice 1 closes the loop

Story-map.md §Walking Skeleton names Slice 1 (US-W05a) the demo-able skeleton:

> *Devansh runs `curl -H "Accept: application/json" .../issues` and gets a JSON array
> of the Auth v2 board's issues — served by foundry-api from the same core call the UI
> uses, with zero HTML — while the full existing acceptance suite stays green.*

The skeleton scenario *"An integrator reads the board's issues as data"* closes that
loop end-to-end through the real protocol:

1. **Given** real Postgres (testcontainers) seeded with a workspace, a Backend team, an
   Auth v2 project, and issues AUTH-2 (in progress) + AUTH-3 (backlog), and Mei signed in
   (the Slice-1 transitional auth per slice-01-json-read.md — the browser session
   authenticates the read before the token surface lands in Slice 2).
2. **When** the integrator issues a real `GET /api/v1/teams/backend/projects/auth-v2/issues`
   over HTTP against the in-process binary's bound port.
3. **Then** the answer is a data list containing AUTH-2 + AUTH-3, each carrying key +
   title + state, with AUTH-2 in_progress and AUTH-3 backlog, and **no markup**.

The litmus test (Mandate 5 / critique Dim 5): a non-technical stakeholder reads it as
"yes — an outside script can read our board as data." The Then steps are user/consumer
observations (a data list, the right issues, no markup), not internal side effects.

Two further US-W05a scenarios prove the load-bearing claims around the skeleton:
- *the same core path* — the JSON answer and the HTML board list the same issues
  (NFR-WEB-BND-05, core neutrality — the riskiest DISCUSS assumption).
- *unauthorized refused* — the read enforces membership equivalent to the web tier.

## 3. Driving the API — real HTTP through the in-process binary

Every US-W05a/b/c When step builds the `/api/v1/...` URL against
`InProcHarness::base_url()` and sends a real `reqwest` request (GET/POST/PATCH) with
`Accept: application/json` (+ `Authorization: Bearer <credential>` for the auth/write
scenarios). The team slug is resolved from the project name via a read-only
`projects JOIN teams` query (preconditions are real rows, not fabricated state). The
response status/headers/body are captured into the world for the Then assertions.

This is layer 3 (subprocess-equivalent: real adapter, real I/O, ~100ms) per the layered
test discipline — example-only, no PBT machinery (Mandate 9/11). Sad paths (missing /
malformed / forged / expired / revoked / wrong-alg / out-of-scope) are enumerated as
named scenarios, never generated.

## 4. Driving the boundary guard — subprocess, not the binary (US-W06)

The US-W06 scenarios run `cargo run -p xtask -- check-arch` as a subprocess against the
real workspace root (computed from `CARGO_MANIFEST_DIR`), capturing the exit code +
combined stdout/stderr. Today the subcommand does not exist, so `xtask` exits non-zero
with an "unknown subcommand" usage — which makes the clean-tree scenario fail RED for
the right reason (the guard is unimplemented). DELIVER implements the subcommand and the
cargo-deny rule, then supplies the planted-violation tree copies (see step-skeletons.md
"Boundary guard wiring"). These scenarios are tagged `@boundary-guard` so CI can shard
them onto the lint lane (no Postgres needed).

## 5. The RED scaffold crates (Mandate 7)

DISTILL committed two non-breaking scaffold crates so the suite is RED, not BROKEN:

- `crates/foundry-services/` — the shared seam (ADR-W04/W07): `Principal`, `ServiceError`,
  `board::list_board_issues`, `issues::{create_issue, change_issue_state}`,
  `comments::{create_comment, edit_comment}`. Bodies `panic!("... RED scaffold")`.
- `crates/foundry-api/` — the JSON adapter (ADR-W01): the route handler entry points, the
  `token_auth::verify_bearer` surface, the `ErrorBody`/`IssueJson` wire shapes, and
  `status_for(ServiceError)`. Bodies `panic!`. Axum is deliberately OUT of the scaffold
  (DELIVER adds it with the real handlers).

Both are workspace members but NOT yet depended on by `foundry-app` or the binary, so
they compile standalone and leave the existing suite green. The `/api/v1` routes are not
yet merged into `build_router`, which is exactly why the API scenarios reach a 404 and
fail RED at their assertions. The `// SCAFFOLD: true` + `pub const __SCAFFOLD__` markers
let the boundary guard's "no scaffolds remain" sweep find them
(`grep -rn "SCAFFOLD: true" crates/`).

## 6. The green-suite invariant (NFR-WEB-COMPAT-01)

The single hard constraint: the pre-existing acceptance suite must stay green and the
workspace must keep compiling. This DISTILL:
- added two standalone scaffold crates (no reverse dependency),
- added one step module + additive world fields,
- added two workspace members + force-link import,
- edited NO existing production code, NO existing step file, NO `AppState`, NO
  `build_router`.
Verified: `cargo build --workspace --tests` succeeds with zero warnings; no existing
scenario was modified. The `AppState::machine_token_verifier` field and the
`build_router .merge(foundry_api::routes(...))` composition are DELIVER's to add (they
touch the ~4 harness AppState construction sites — see architecture.md §Composition &
harness blast radius, and step-skeletons.md).

## 7. Local + CI invocation

```bash
# Whole feature (needs Docker for testcontainers Postgres):
cargo test -p foundry-acceptance --test acceptance      # default lane runs all incl. @feature-a

# The boundary-guard subprocess scenarios shard onto the lint lane (no DB):
#   tagged @boundary-guard — DELIVER wires the runner to select them there.
```

The default lane already picks up the four new feature files (they carry no excluded
tag). DELIVER may add a `@feature-a`-only fast subset selection if the inner loop wants
it, but the default lane is correct as-is.
