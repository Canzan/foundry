# ADR-007: A real-browser acceptance lane via fantoccini (ODD-9)

## Status
Accepted — 2026-07-15 (Morgan, DESIGN wave). Feature-local. Resolves **ODD-9** (blocking, all slices).
**Reverses a recorded prior decision** — see `upstream-changes.md` §2. **This is the root-cause fix**: the
instrument that would have caught the missing layer did not exist, which is exactly why nobody noticed.

## Context
Every AC in this feature asserts **key-pressed → user-observable outcome** (NFR-1). The shipped acceptance
suite cannot express a single one of them: it drives HTTP with `reqwest` and parses HTML with `scraper`,
proving the *server contracts the client would call*. It never presses a key. That is why
`GET /keyboard-help` is green while `?` does nothing.

**Two verified facts reshape this ODD.**

1. **The premise that a browser needs new serving plumbing is false.** `InProcHarness::spawn`
   (`support/harness.rs:287,322-440`) → `spawn_app_with_listener` → `spawn_app`
   (`foundry-app/src/lib.rs:726-746`) already does:
   ```rust
   let listener = TcpListener::bind("127.0.0.1:0").await?;   // real socket, ephemeral port
   let addr = listener.local_addr()?;
   tokio::spawn(async move { axum::serve(listener, router).with_graceful_shutdown(…).await.ok(); });
   ```
   exposed as `harness.base_url()` → `http://127.0.0.1:{port}` (`harness.rs:442-444`). "In-process" means
   *same OS process as the test binary*, **not** *no socket*. `tower::oneshot` is used only in
   `foundry-app`'s own unit tests, never in the acceptance crate. **A real browser can point at
   `base_url()` today.** ODD-9 is therefore a *client* problem only.

2. **The repo already made, and recorded, the decision that caused this bug.**
   `tests/features/us-12-keyboard-nav.feature:18-23`:
   > *"Pure browser interaction (the actual `c` / `/` / `j` / `k` / `Enter` / `?` key handling, the modal
   > focus management, the highlight after a realtime swap) lives in alpine.js and is **OUT of automated
   > scope per the JTBD-backend-MVP no-Playwright decision**. The @manual scenario at the bottom is the QA
   > drill script."*
   The `@manual` drill at `:87-95` is the fig leaf that let seven advertised-and-unbound shortcuts ship
   green. This ADR reverses that decision.

Per AGENTS.md, whatever driver is chosen belongs in **`cargo xtask ci`** — *"Never add a bespoke check to
`ci.yml` alone"* — and must coexist with the shipped `@docker-compose` / `@needs-pgclient` lanes.

## Decision

### 1. `fantoccini` — the crate, defended
`fantoccini` (MIT/Apache-2.0), added to `[workspace.dependencies]` beside the acceptance stack
(`Cargo.toml:102-109`), used by `foundry-acceptance` only. It must clear `deny.toml` (`cargo deny check`,
`xtask/src/main.rs:176`).

Rationale: it speaks **W3C WebDriver**, so the lane is **driver-agnostic** (chromedriver *or* geckodriver)
rather than locked to one browser's private protocol; it is async/tokio, matching the harness and the
runner exactly; its API surface is small and covers everything needed (navigate, `send_keys`, the Actions
API for chords, `find`/attribute reads, `execute` for JS); it is maintained by a well-known author. The
user's pick is the right one and I endorse it on the merits.

### 2. `BrowserHarness` — reuse `InProcHarness` as-is
```
BrowserHarness = InProcHarness (unchanged)  +  fantoccini::Client → InProcHarness::base_url()
```
No new app-construction path, no new serving code, no compose. One app build means the browser lane and the
port-to-port suite exercise **the same app**, so they cannot diverge. DB provisioning (testcontainers
Postgres `16-alpine`, per-scenario schema) is inherited untouched (`harness.rs:40-92,121-173`).

### 3. Lane `@needs-browser`, wired into `cargo xtask ci`
- **Added to the default fast loop's exclusion list** (`acceptance.rs:245-252`), beside `@docker-compose`
  / `@needs-pgclient` / `@slow`, so a browserless `cargo test -p foundry-acceptance` still works.
- **Included in `all`** (`acceptance.rs:158-191`) — which is what `cargo xtask ci` runs. CI must install
  Chrome + chromedriver exactly as it installs `postgresql-client-16`
  (`.github/workflows/ci.yml:91`). **It is not excluded from `all`**; see §4.
- **Prerequisite preflight** in `run_ci`, mirroring `pg_dump_at_least_16()` (`xtask/src/main.rs:335-358`)
  in shape: probe `chromedriver --version` **and** the browser, parse majors, assert they match, and on
  failure print an actionable per-OS hint (`brew install --cask chromedriver` /
  `apt-get install -y chromium-driver`) and exit non-zero.
- **Fix the env-injection trap first.** `run_steps` sets `FOUNDRY_ACCEPTANCE_TAGS` by **label substring**
  (`xtask/src/main.rs:250-257`: `if label.contains("foundry-acceptance") { … "all" }`). A second acceptance
  step cannot be distinguished by label. Change the step tuple to carry its own env
  (`(&str, Vec<&str>, Vec<(&str,&str)>)`) — which also keeps `run_smoke` a strict, drift-proof subset.
- **Register the step module** in `acceptance.rs:34-122` or the `inventory::submit!` steps silently vanish.

### 4. Earned Trust: **probe, then refuse — never skip.** *(the most important clause here)*
The bug exists because **the instrument did not exist**. A lane that *silently skips* when chromedriver is
missing or version-skewed **recreates the exact failure mode this feature exists to close**: a green suite
over an absent capability. Therefore:

- The lane **never** `#[ignore]`s, never soft-passes, never auto-skips on a missing driver. A missing or
  skewed driver **fails `cargo xtask ci`** with a structured, actionable diagnostic.
- **It probes before it trusts.** At lane start, before any scenario: start a session; navigate to
  `base_url()`; assert `[data-kb-ready]` appears (the ADR-001 readiness marker); round-trip one key.
- **The probe that matters most — the substrate that is documented to lie.** `harness.rs:401-406` says, in
  the code:
  > *"The harness binds to 127.0.0.1 (plain HTTP) but we still emit `Secure` in the `Set-Cookie` header —
  > the test only inspects the header text, **not whether the browser would send the cookie back over
  > HTTP**."*
  `reqwest` does not care. **A real browser does.** It happens to work (Chrome/Firefox treat `127.0.0.1`
  as a trustworthy origin and accept `Secure` cookies there) — but that is an assumption about a substrate
  free to change. The probe **signs in and asserts still-signed-in** so this fails as *one* clear
  diagnostic at lane start rather than as every scenario mysteriously failing at sign-in.
- **Waits are conditions, never sleeps.** `[data-kb-ready]` before any key; `#modal-root [data-modal]`
  after `c`; `document.activeElement` for focus; `.issue-card[aria-selected=true]` for the ring. A
  bounded `wait().at_most(...)` on an explicit condition — never `sleep`.
- **Lifecycle**: one chromedriver **process** per lane, one **session** per scenario — the same
  built-once / isolated-per-scenario shape AGENTS.md mandates for compose images. Headless
  (`--headless=new`), with a **fixed window size** so the `scrollIntoView` scenario (AC-05.3) is
  deterministic rather than dependent on the runner's screen.

### 5. Retire the `@manual` drill
When slice 05 lands 7/7, `us-12-keyboard-nav.feature:87-95` is superseded by automation and its module doc
(`:18-23`) states a decision that is no longer true. Both go; the drill's steps become the lane's scenarios.

## Alternatives Considered
- **`thirtyfour`** — REJECTED, narrowly, and it is a reasonable choice. Also W3C WebDriver, also
  maintained, with a richer built-in wait/element API we would otherwise hand-roll. Rejected for surface
  area: we need ~6 operations, and fantoccini's smaller API is easier to keep honest against the
  "conditions, not sleeps" rule (a rich implicit-wait API invites accidental timing dependence). This is a
  preference between two good options, not a defect in `thirtyfour` — if fantoccini's ergonomics bite in
  slice 01, switching is a contained change behind `BrowserHarness`.
- **`headless_chrome` / `chromiumoxide` (CDP)** — REJECTED. CDP is Chrome-only, forfeiting the
  driver-agnosticism W3C WebDriver buys; `headless_chrome` has had maintenance gaps; and CDP's auto-managed
  browser download is a network dependency at test time (see below).
- **Playwright / Selenium via a Node or JVM toolchain** — REJECTED. It would import a second language
  toolchain into a Rust repo with an explicit no-toolchain posture, and the lane could not live in
  `cargo xtask ci` as a first-class cargo step. (Note the prior decision this ADR reverses was framed as
  *"no-Playwright"* — that framing conflated *Playwright* with *browser testing*. Reversing it does **not**
  mean adopting Playwright; it means the Rust-native lane the repo can actually own.)
- **Drive the browser against `ComposeStack` instead of `InProcHarness`** — REJECTED. Compose gives the real
  production image on a host-mapped port (`compose_harness.rs:133-152`), but at ~30-60 s per scenario and a
  Docker dependency, for a **client-only** feature where the server is irrelevant. `InProcHarness` already
  provides a real origin at near-zero cost. (Compose remains right for US-01's deployment lane.)
- **Download/manage the driver at test time (webdriver-manager style)** — REJECTED. It adds a network fetch
  to every run — a classic flake source — and the repo has no precedent for it. The shipped pattern is
  **host prerequisite + preflight check + install hint** (docker, `cargo-deny`, `pg_dump`), and this lane
  follows it.
- **Run chromedriver in Docker (`selenium/standalone-chrome`)** — REJECTED for v1, though it is the most
  reproducible option and worth revisiting if host version-skew proves painful. It pins browser+driver
  exactly and needs no host install, but the browser is then in a container while the app is on the host's
  ephemeral port, requiring `host.docker.internal` (plus `--add-host=host-gateway` on Linux) — fiddly
  networking for a lane whose whole appeal is that the origin is already there.
- **Exclude `@needs-browser` from `all` (so `cargo xtask ci` passes without a browser)** — **REJECTED, and
  this is the decision that matters most.** It is the tempting, friction-free option and it **rebuilds the
  bug**: a gate that is green without ever pressing a key is precisely the state that let seven advertised
  shortcuts ship unbound. If the lane is optional, it will be absent when it matters. It runs in `all`, and
  a missing driver is a **hard failure with an install hint** — the same contract `pg_dump` already has.

## Consequences
- **Positive — the root cause is closed.** `cargo xtask ci` can now press a key. Every NFR-1 AC becomes
  expressible, and the class of bug "advertised in the help page, never bound in the browser" becomes
  **impossible to ship green**.
- **Positive**: far cheaper than DISCUSS priced it — no serving plumbing, no compose, no new app path.
  `InProcHarness` + a WebDriver session + `base_url()`.
- **Positive**: the `[data-kb-ready]` marker does double duty — the lane's wait condition *and* US-02's
  "the layer is live" precondition (D15's paired assertion), so the anti-vacuity guard has a real hook.
- **Negative / accepted**: **a new host prerequisite** (Chrome + version-matched chromedriver) for every
  developer and for CI. This is the cost of the fix and it is paid in the same currency as `pg_dump` and
  Docker. Platform-architect owes chromedriver in the CI image.
- **Negative / accepted**: browser scenarios are slower and inherently more flake-prone than port-to-port.
  Mitigated structurally: conditions not sleeps, one session per scenario, fixed window size, mock clock
  inherited. Flakiness is a first-class concern, not a footnote — if a scenario needs a sleep to pass, the
  scenario is wrong.
- **Negative / accepted — two honest limits, named so DISTILL does not write unimplementable ACs.**
  1. **IME composition cannot be truly driven.** WebDriver `send_keys` does not produce composition events.
     ADR-002's guard 1 must be exercised with JS-dispatched `CompositionEvent` + `KeyboardEvent{isComposing:
     true}` via `client.execute()`. Listeners fire for untrusted events, so this **does** exercise our
     predicate — but it is **not** a real IME, and a real-IME regression could still reach Mei.
  2. **Clipboard cannot be read headless.** AC-02.3 must assert **non-activation** (`Cmd+C`/`Ctrl+C` opens
     no modal) and `defaultPrevented === false`, **not** "the text was copied". Note also that on Linux CI
     the copy chord is `Ctrl`, not `Meta` — the AC should assert **both** modifiers are inert rather than
     assert a platform copy behaviour.
- **Probe (Earned Trust), restated because it is the point**: this ADR's own value depends on the lane
  actually running. The preflight + the startup probe + `all`-inclusion are what make that empirically
  true rather than aspirational. **A skipped browser lane is indistinguishable from the bug it exists to
  prevent.**
