# Manual Drills — Slice 4 / US-13 Contributor Onboarding

Two human-run drills that exercise the contributor experience US-13
promises. Each drill has a start condition, a stopwatch, named
checkpoints, and an unambiguous pass / fail criterion.

When to run: at release-candidate cuts, before any 1.0 announcement,
and any time a maintainer wants a fresh data point for the JTBD
outcome-3 onboarding survey baseline.

Who can run them: any developer who has not actively worked on Foundry
in the last week (so the README is the contract, not the muscle
memory). Maintainers can run them on their own machine if no fresh
laptop is available — note the cache-state caveat in each drill.

---

## Drill A — Time-to-green-tests (≤ 10 minutes)

Pinned by feature-file scenario #5:
*"A new contributor reaches green tests within ten minutes on a fresh laptop"*

### Target

Wall-clock time from `git clone` to a green `cargo test -p
foundry-acceptance` run: **≤ 10 minutes**.

### Pre-conditions

- A laptop with a recent operating system (macOS 14+ or Ubuntu 22.04+
  or equivalent).
- A Rust toolchain installed via `rustup` (any version — `rustup`
  will auto-install the Foundry MSRV from `rust-toolchain.toml` on
  first `cargo` invocation).
- A working Docker daemon (Docker Desktop, OrbStack, Colima, Lima,
  or system Docker).
- A network connection that can reach `github.com`, `crates.io`, and
  `docker.io` (or the configured Docker registry mirror).

### Out of scope

- "Cold rustup install" — installing rustup is a 1-minute step
  outside Foundry's contract. The contributor brings a Rust toolchain
  manager; the project supplies the MSRV.
- "No cached Docker images" — if the contributor already pulled
  `postgres:11-alpine` for another project, the testcontainers cold-
  start is faster. Note the cache state in the result.
- "No cached cargo registry" — same reasoning. Note state in result.

### Procedure

1. Start a stopwatch.
2. `git clone https://github.com/foundry-project/foundry.git` (or the
   appropriate fork URL).
3. `cd foundry`.
4. Read the README's Quickstart section once.
5. Run each command in the Quickstart in sequence.
6. When `cargo test -p foundry-acceptance` exits, stop the stopwatch.

### Checkpoints

Note the wall-clock time at each checkpoint:

| Checkpoint | What | Target |
|---|---|---|
| T1 | `git clone` finishes | < 30 s |
| T2 | `cargo build --all` finishes (first run, cold cargo cache) | < 4 min |
| T3 | `docker compose up postgres` reports healthy | < 1 min |
| T4 | `sqlx migrate run` (or equivalent) finishes | < 30 s |
| T5 | `cargo test -p foundry-acceptance` finishes green | < 5 min |
| **Total (T5 - T0)** | **wall-clock to green** | **≤ 10 min** |

### Pass / fail

- **PASS**: total ≤ 10 minutes AND every command in the Quickstart
  succeeds on the first try AND no documentation outside the README
  was consulted.
- **CONDITIONAL PASS**: total ≤ 15 minutes (the JTBD survey "50% of
  first-time clones produce a green local test run within 30 minutes"
  threshold). Log the gap and what consumed the extra time.
- **FAIL**: total > 15 minutes OR any command in the Quickstart failed
  with an unclear error OR the contributor needed to consult
  CONTRIBUTING.md or any other source.

### Reporting

Record the result in the GitHub Discussions onboarding survey thread:

```
Drill A result — YYYY-MM-DD
- runner: <name or handle>
- laptop: <macOS 14.x / Ubuntu 22.04 / etc.>
- docker: <Docker Desktop / Colima / etc.>
- cache state: <cold / warm cargo / warm docker / both warm>
- T1: __min __s
- T2: __min __s
- T3: __min __s
- T4: __min __s
- T5: __min __s
- TOTAL: __min __s
- verdict: PASS / CONDITIONAL PASS / FAIL
- notes: <anything that surprised the runner>
```

---

## Drill B — Visible change after one-line edit (≤ 30 seconds recompile)

Pinned by feature-file scenario #6:
*"A contributor sees their one-line change reflected in the running app"*

### Target

Wall-clock time from saving a one-line template edit to seeing the
change in the browser: **≤ 30 seconds**.

### Pre-conditions

- Drill A has just been run successfully (or the contributor has
  separately reached a green `cargo test -p foundry-acceptance` and
  a running Foundry dev server bound at the README-documented local
  URL).
- `cargo watch` is installed (`cargo install cargo-watch` — a one-
  time step; if absent, the drill measures cold install + first
  recompile and notes the cache state).
- A browser pointed at the README-documented local URL is open.

### Procedure

1. Note the current heading text rendered at the local URL (e.g. the
   workspace dashboard's `<h1>`).
2. In a separate terminal in the workspace root, start the documented
   watch-and-rebuild command (e.g. `cargo watch -x run`).
3. Wait for the watch process to report "compiling" → "running".
4. Open `src/templates/dashboard.html` (or the heading-bearing template
   named in the README's hot-reload section).
5. Change the heading text by exactly one word. Save.
6. Start a stopwatch the moment the editor reports "saved".
7. Switch to the browser. Refresh.
8. The moment the new heading text is visible, stop the stopwatch.

### Pass / fail

- **PASS**: total ≤ 30 seconds AND the new heading is visible AND no
  manual intervention beyond browser refresh was required.
- **CONDITIONAL PASS**: total > 30 s but ≤ 60 s. Log what consumed
  the extra time (incremental compile time on this hardware, watch
  tooling slow to detect, etc.).
- **FAIL**: total > 60 s OR the watch process did not detect the edit
  OR the browser still shows the old heading after refresh.

### Reporting

```
Drill B result — YYYY-MM-DD
- runner: <name or handle>
- laptop: <macOS 14.x / Ubuntu 22.04 / etc.>
- watch tool: cargo watch v__
- edit-to-visible: __ s
- verdict: PASS / CONDITIONAL PASS / FAIL
- notes: <anything that surprised the runner>
```

---

## Anti-cheating notes (for honest measurement)

- Do not run either drill on a machine with an active Foundry working
  copy + warm `target/` cache. The point is the *new contributor's*
  experience; a warm cache invalidates the result.
- Do not consult the maintainer Slack / Discord / GitHub Discussions
  during the drill. If a command is unclear, that *is* a finding —
  surface it in the result notes; the drill is observing the README
  contract, not the maintainer's improvisation.
- Do not edit the README during the drill. If you find a defect, log
  it after the drill ends.

---

## Why these are `@manual` rather than automated

Automating either drill would replace the measurement under test with
its proxy:

- **Drill A** measures the *human* experience (pause-to-read tempo,
  the moment of "wait, do I need sudo?", the lookup of "what is the
  default Postgres password?"). Automating the timer would measure CI
  cache state + compile time, not the contributor experience the AC
  names.
- **Drill B** measures the *full* edit → save → reload → see loop,
  including the browser. Automating the browser side would need a
  headless browser harness that exercises the production HTML
  rendering — heavy infrastructure that does not exist elsewhere in
  the suite, in service of a property a human can observe in a
  glance.

The trade-off is intentional and matches the slice-1 / slice-3
precedent for `@manual` operator drills (US-01's "evaluating operator
reaches admin claim form within 30 minutes" and US-12's `@manual`
keyboard-shortcut browser drill).
