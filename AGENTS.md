<!-- dgc-policy-v11 -->
# Dual-Graph Context Policy

This project uses a local dual-graph MCP server for efficient context retrieval.

## MANDATORY: Always follow this order

1. **Call `graph_continue` first** — before any file exploration, grep, or code reading.

2. **If `graph_continue` returns `needs_project=true`**: call `graph_scan` with the
   current project directory (`pwd`). Do NOT ask the user.

3. **If `graph_continue` returns `skip=true`**: project has fewer than 5 files.
   Do NOT do broad or recursive exploration. Read only specific files if their names
   are mentioned, or ask the user what to work on.

4. **Read `recommended_files`** using `graph_read` — **one call per file**.
   - `graph_read` accepts a single `file` parameter (string). Call it separately for each
     recommended file. Do NOT pass an array or batch multiple files into one call.
   - `recommended_files` may contain `file::symbol` entries (e.g. `src/auth.ts::handleLogin`).
     Pass them verbatim to `graph_read(file: "src/auth.ts::handleLogin")` — it reads only
     that symbol's lines, not the full file.
   - Example: if `recommended_files` is `["src/auth.ts::handleLogin", "src/db.ts"]`,
     call `graph_read(file: "src/auth.ts::handleLogin")` and `graph_read(file: "src/db.ts")`
     as two separate calls (they can be parallel).

5. **Check `confidence` and obey the caps strictly:**
   - `confidence=high` -> Stop. Do NOT grep or explore further.
   - `confidence=medium` -> If recommended files are insufficient, call `fallback_rg`
     at most `max_supplementary_greps` time(s) with specific terms, then `graph_read`
     at most `max_supplementary_files` additional file(s). Then stop.
   - `confidence=low` -> Call `fallback_rg` at most `max_supplementary_greps` time(s),
     then `graph_read` at most `max_supplementary_files` file(s). Then stop.

## Token Usage

A `token-counter` MCP is available for tracking live token usage.

- To check how many tokens a large file or text will cost **before** reading it:
  `count_tokens({text: "<content>"})`
- To log actual usage after a task completes (if the user asks):
  `log_usage({input_tokens: <est>, output_tokens: <est>, description: "<task>"})`
- To show the user their running session cost:
  `get_session_stats()`

Live dashboard URL is printed at startup next to "Token usage".

## Rules

- Do NOT use `rg`, `grep`, or bash file exploration before calling `graph_continue`.
- Do NOT do broad/recursive exploration at any confidence level.
- `max_supplementary_greps` and `max_supplementary_files` are hard caps - never exceed them.
- Do NOT dump full chat history.
- Do NOT call `graph_retrieve` more than once per turn.
- After edits, call `graph_register_edit` with the changed files. Use `file::symbol` notation (e.g. `src/auth.ts::handleLogin`) when the edit targets a specific function, class, or hook.

## Context Store

Whenever you make a decision, identify a task, note a next step, fact, or blocker during a conversation, call `graph_add_memory`.

**To add an entry:**
```
graph_add_memory(type="decision|task|next|fact|blocker", content="one sentence max 15 words", tags=["topic"], files=["relevant/file.ts"])
```

**Do NOT write context-store.json directly** — always use `graph_add_memory`. It applies pruning and keeps the store healthy.

**Rules:**
- Only log things worth remembering across sessions (not every minor detail)
- `content` must be under 15 words
- `files` lists the files this decision/task relates to (can be empty)
- Log immediately when the item arises — not at session end

## Session End

When the user signals they are done (e.g. "bye", "done", "wrap up", "end session"), proactively update `CONTEXT.md` in the project root with:
- **Current Task**: one sentence on what was being worked on
- **Key Decisions**: bullet list, max 3 items
- **Next Steps**: bullet list, max 3 items

Keep `CONTEXT.md` under 20 lines total. Do NOT summarize the full conversation — only what's needed to resume next session.

# Development Workflow

This project uses **trunk-based development**.

- **Commit directly to `main` (trunk).** Do NOT create feature branches by default, and do NOT switch off `main` to do work unless the user explicitly asks.
- **No pull requests.** Never open a PR. Land work by committing to `main` and pushing.
- Commit when the user asks; keep commits small and frequent in the trunk-based style.

### MANDATORY pre-push gate: `cargo xtask ci` must be green

**Do NOT push to the GitHub repository unless every CI task passes locally.** CI
minutes are not a debugging tool — the remote workflow runs the *exact same*
command you run locally, so a red push is always avoidable.

- **One command = the entire CI.** `cargo xtask ci` replicates every CI check, in
  order, stopping on the first failure: `cargo fmt --check`, `clippy
  --all-targets --release -D warnings`, the **check-arch boundary guard**
  (AST source-walk + the cargo-deny dependency-direction layer), `cargo build
  --all --release`, the workspace unit/integration tests, `cargo deny check`,
  and the full acceptance suite (`FOUNDRY_ACCEPTANCE_TAGS=all`, including the
  `@docker-compose` and `@needs-pgclient` groups). `.github/workflows/ci.yml` is
  a thin wrapper that runs this same `cargo xtask ci` — nothing runs in CI that
  you cannot run locally.
- **Run it before every push** and require a green `xtask ci :: all gates green`.
  A local pass means a CI pass; if CI is the first place a check runs, that is a
  process failure — add the check to `xtask::run_ci`, not just to the workflow.
- **Prerequisites** (xtask checks for each and prints an install hint if
  missing): a reachable Docker daemon (Colima/OrbStack/Docker Desktop) for the
  `@docker-compose` group, `cargo-deny` (`cargo install --locked cargo-deny`), a
  **PostgreSQL 16+ client** (`pg_dump`/`pg_restore` on PATH — macOS `brew install
  postgresql@16`, Debian/Ubuntu `apt-get install -y postgresql-client-16`) for
  the US-03 backup lane, and a `.env` (auto-seeded from `.env.example`).
- **Never add a bespoke check to `ci.yml` alone.** If a gate belongs in CI it
  belongs in `cargo xtask ci` so it runs locally too — that single-source-of-truth
  invariant is what keeps "green locally" and "green in CI" identical.

### MANDATORY pre-commit smoke: `cargo xtask smoke`

`cargo xtask ci` is the full pre-**push** gate, but it is slow (release build +
the whole `@docker-compose` / `@needs-pgclient` acceptance suite). For the tight
edit loop, **run `cargo xtask smoke` before every commit.** It runs the same
`fmt`, `clippy`, check-arch boundary guard, and `cargo test --workspace (excl.
foundry-acceptance) --release` steps as CI — a strict subset drawn verbatim from
`run_ci`, so it can never drift from CI — while skipping only the acceptance
suite and `cargo deny`. It is the check that catches the single most common
avoidable red push: a unit/integration test that fails under `--release`.

- **Commit gate**: a green `xtask smoke :: all gates green` before you `git commit`.
- **Push gate**: a green `xtask ci :: all gates green` before you `git push`
  (unchanged, still mandatory — smoke is a fast pre-filter, NOT a replacement).
- Smoke's test step uses Postgres testcontainers, so a reachable Docker daemon is
  still required for it (same as those tests inside `cargo xtask ci`).

## Acceptance Docker images

The `@docker-compose` acceptance scenarios (US-01) build the `foundry` service
image from source. **Build exactly one named, reusable image per run and clean
it up afterward — never one image per test.**

- The compose file pins the service image via `image: ${FOUNDRY_IMAGE:-foundry:latest}`
  (`docker-compose.yml`). Without this, compose names each build `<project>-foundry`,
  and because the harness uses a unique `COMPOSE_PROJECT_NAME` per scenario
  (`foundry-at-<uuid>`), every scenario would leak its own image.
- The acceptance suite overrides `FOUNDRY_IMAGE` to a single shared tag
  (`compose_harness::SHARED_IMAGE` = `foundry-acceptance:latest`). The runner
  (`tests/acceptance.rs`) calls `compose_harness::build_shared_image()` ONCE
  before the compose lanes and `compose_harness::remove_shared_image()` ONCE
  after, so the suite leaves no images behind.
- If you add a new harness that builds an image, reuse `SHARED_IMAGE` (or add a
  similarly named, built-once/removed-once tag) — do NOT let compose mint a
  per-project image. Containers and volumes stay per-scenario for isolation;
  only the image is shared.

## Dead code

This project has not reached a stable release. **Remove dead/legacy code outright — do not leave it inert.**

- When a route, function, guard, or path is superseded, **delete it** in the same change rather than leaving it commented-out, feature-flagged-off, or unreachable "for safety."
- Pre-stable, there is no backward-compatibility obligation, so defence-in-depth dead code is just carry: prefer a clean tree and rely on git history to recover anything removed.
- Re-evaluate this policy once a stable version is released (at that point, deprecate-then-remove may be warranted instead).
