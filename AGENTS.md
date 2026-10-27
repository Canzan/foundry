<!-- dgc-policy-v11 -->
# Dual-Graph Context Policy

Retrieve context through the local dual-graph MCP server, in this order:

1. Call `graph_continue` before any file read, grep, or other exploration.
2. `needs_project=true` → call `graph_scan` on the project directory (`pwd`) without asking the user.
3. `skip=true` (under 5 files) → read only the files the request names, or ask the user what to work on.
4. Read each `recommended_files` entry with its own `graph_read` call (parallel is fine); pass a `file::symbol` entry verbatim to read only that symbol.
5. `confidence=high` → stop. `medium` (only if the files fall short) or `low` → call `fallback_rg` with specific terms up to `max_supplementary_greps` times, then `graph_read` up to `max_supplementary_files` more files, then stop. Both caps are hard limits, and exploration stays targeted at every confidence.

Call `graph_retrieve` at most once per turn. After edits, call `graph_register_edit` with the changed files, in `file::symbol` form when the edit targets one function, class, or hook.

When a decision, task, next step, fact, or blocker worth keeping across sessions arises, log it then with `graph_add_memory(type="decision|task|next|fact|blocker", content=..., tags=[...], files=[...])`: `content` under 15 words, `files` possibly empty. `graph_add_memory` is the only writer of `context-store.json`.

## Session End

User signals the session is over ("bye", "done", "wrap up") → rewrite `CONTEXT.md`
in the project root: **Current Task** (one sentence), **Key Decisions** and
**Next Steps** (at most 3 bullets each), under 20 lines in total.

# Development Workflow

Trunk-based: commit and push on `main`, open no pull requests, and branch only
when the user asks.

- Commit only after `cargo xtask smoke` prints `xtask smoke :: all gates green`.
- Push only after `cargo xtask ci` prints `xtask ci :: all gates green`.

Writing or changing code or tests → [`CODING_STANDARDS.md`](./CODING_STANDARDS.md).

Adding a CI check, or a gate fails on a missing prerequisite →
[`DEVELOPER.md`, "The local gate"](./DEVELOPER.md#the-local-gate--cargo-xtask-ci).

Adding or changing a compose acceptance harness or the image it builds →
`crates/foundry-acceptance/src/support/compose_harness.rs` module docs.
