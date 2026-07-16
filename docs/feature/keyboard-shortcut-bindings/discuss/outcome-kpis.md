# Outcome KPIs — keyboard-shortcut-bindings

## Feature: keyboard-shortcut-bindings

### Objective

**Make the help page tell the truth.** Within this feature, every one of the seven shortcuts Foundry already
advertises works in the browser, guarded so it never steals a keystroke — so a keyboard-first maintainer can
learn, file, find, move, open and escape without leaving the home row.

### The baseline is not "low" — it is zero, and it is a broken promise

Most features start from *some* satisfaction. This one starts from **floor with a negative sign**: the product
**advertises seven shortcuts in writing** (`SHORTCUTS`, `keyboard.rs:48-56`, rendered into a `<dl>` the user
reads) and honours **none** of them. That is not an absent feature; it is a **documented commitment that fails
on contact**. The north-star metric is therefore the ratio between what the product **claims** and what it
**does** — a number that is currently **0/7** and whose only acceptable end state is **7/7**.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| **KPI-1** | Keyboard-first maintainers (Mei) on any signed-in page | Press an advertised shortcut and get its advertised action | **7 of 7** advertised shortcuts work (from **0 of 7**) | **0/7** — every advertised shortcut is unbound | Browser-level scenario per shortcut, each of which **must fail on `main` today** (NFR-1) | Leading (primary) |
| **KPI-2** | Anyone typing into any Foundry text field | Type shortcut characters and system chords without triggering a shortcut | **0** shortcut activations from a keystroke aimed at a text field; **0** hijacked `Ctrl`/`Cmd`/`Alt` chords | N/A — nothing is bound, so nothing is captured **yet**; the invariant is established **as** the layer lands | The text-input-guard `@property` litmus + the modifier-guard scenario | **Guardrail (hard)** |
| **KPI-3** | Maintainers triaging a board | Complete file / find / select-and-open **without a pointer** | **0** mouse actions required for each of the three loops (from *every* loop requiring one) | Every loop currently requires a pointer (or the no-JS full page) | Browser-level scenarios: `c`→file, `/`→find, `j`/`k`→`Enter`→open | Leading |
| **KPI-4** | Maintainers using assistive technology | Perceive which issue is selected as it changes | Selection change is announced on **100%** of `j`/`k` moves; ring meets WCAG 2.1 AA and never relies on colour alone | N/A — no selection concept exists | Screen-reader scenario + contrast check (mechanism per **ODD-7**) | **Guardrail** |
| **KPI-5** | The product's own help page | Advertise only shortcuts that work | **0** advertised-but-unbound shortcuts; **0** bound-but-unadvertised | **7** advertised-but-unbound | `@property`: bound set == `SHORTCUTS` (both read the same constant) | **Guardrail (anti-regression)** |

### Metric Hierarchy

- **North Star**: **KPI-1 — the advertised-to-working ratio (0/7 → 7/7).** It is a ratio, not a count; it is the
  literal definition of the problem; and it is the only number that says whether the product still lies.
- **Leading Indicators**: KPI-3 (mouse actions eliminated per loop) — the behaviour change that *makes* KPI-1
  worth anything. A shortcut that "works" but that Mei doesn't use hasn't moved the job.
- **Guardrail Metrics (must NOT degrade)**:
  - **KPI-2 — typing is never captured.** The one metric whose failure makes the feature **net-negative**.
  - **KPI-4 — a11y of the ring selection.** Named because D-4 rejected roving tabindex; this is the debt.
  - **KPI-5 — promise/fulfilment parity.** Stops this exact bug from recurring for shortcut #8.
  - **No-JS parity** — every existing no-JS scenario passes unchanged (NFR-4).
  - **Drag-and-drop parity** — every existing drag scenario passes unchanged (NFR-8).
  - **Zero server delta** — no route, endpoint or migration added (FR-11).

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| KPI-1 | The acceptance suite | One browser-level scenario per shortcut; count passing / 7 | Per slice (the ratio is the slice's headline number) | DELIVER |
| KPI-2 | The acceptance suite | The text-input-guard `@property` + modifier-guard scenario; **must red on revert** | Every run (`cargo xtask ci`) | DELIVER |
| KPI-3 | The acceptance suite | Per-loop scenarios asserting the loop completes with **0** pointer actions | Per slice | DELIVER |
| KPI-4 | Screen-reader scenario + automated contrast check | Mechanism per ODD-7 | Slice 05 | DESIGN → DELIVER |
| KPI-5 | The acceptance suite | `@property`: enumerate the help overlay's `dt[data-shortcut]` values and assert each is bound (both derive from `SHORTCUTS`) | Every run | DELIVER |

> **Measurement is blocked on tooling — and that is the finding, not a footnote.** Every KPI above needs a
> **browser-capable driver**. The shipped harness (`InProcHarness` + reqwest + `scraper`,
> `us_12_keyboard_nav.rs:48-56`) is **port-to-port and cannot press a key**. This is exactly why the layer went
> missing without anyone noticing: **the instrument that would have caught it does not exist.**
> Resolving it is **ODD-9** and it must be resolved before any KPI here can be read. Per `AGENTS.md`, whatever
> driver is chosen belongs in `cargo xtask ci` (never in `ci.yml` alone).

### Hypothesis

We believe that **binding the seven shortcuts Foundry already advertises, behind a guard that never captures a
keystroke**, for **keyboard-first maintainers working a project board** will achieve **a product whose
documented shortcuts are all real (7/7) and whose core loops need no pointer**.

We will know this is true when **Mei Tanaka presses each of the seven advertised keys and gets its advertised
action (KPI-1 = 7/7)**, **can still type `"cache invalidation on login"` into a title field unharmed (KPI-2 =
0 captures)**, and **completes the file / find / open loops with zero mouse actions (KPI-3)**.

We will know this is **false** if the guard proves unable to distinguish typing from commanding across real
inputs and IME composition — in which case the honest outcome is to **unbind the character keys and shrink the
advertised list** rather than ship a layer that eats keystrokes.

### What we are deliberately NOT measuring

- **Shortcut usage frequency / adoption analytics.** Foundry ships no client telemetry and this feature adds
  none. Adding tracking to measure a keyboard layer would be a larger privacy decision than the feature itself.
  KPI-1 measures **capability delivered**; KPI-3 measures **the loop is pointer-free**; whether Mei *chooses*
  `j` over her mouse is not instrumented and should not be.
- **Time-to-file / triage throughput.** The appealing metric ("issues filed per minute") is a **lagging**
  indicator polluted by how many bugs exist that week. We target the leading behaviour (the loop needs no
  pointer) instead.
- **Server-side metrics.** This is a client-only feature with **zero server delta** (FR-11); the existing
  `/metrics` sidecar has nothing to say about a keypress, and adding a counter for one would be instrumentation
  theatre.

### Smell-test results

| Check | KPI-1 | KPI-2 | KPI-3 | KPI-4 | KPI-5 |
|-------|-------|-------|-------|-------|-------|
| Measurable today? | **No — ODD-9** (needs a browser driver) | **No — ODD-9** | **No — ODD-9** | **No — ODD-9** | **No — ODD-9** |
| Rate/ratio not total? | Yes (x/7) | Yes (0 per attempt) | Yes (0 per loop) | Yes (% of moves) | Yes (0 divergences) |
| Outcome not output? | Yes — "presses a key and gets the action", not "ship keyboard.js" | Yes | Yes | Yes | Yes |
| Has baseline? | Yes — **0/7** | Yes — N/A→invariant | Yes — every loop needs a pointer | Yes — no selection exists | Yes — **7** advertised-but-unbound |
| Team can influence? | Yes — directly | Yes | Yes | Yes | Yes |
| Has guardrails? | Yes — KPI-2/4/5 + no-JS + drag + zero-server-delta | — | — | — | — |

**One honest failure across the board**: *"Measurable today?"* is **No** for every KPI, for a single shared
reason — **ODD-9**, the harness cannot press keys. Per the framework's own rule ("if No → add instrumentation to
requirements"), that instrumentation is **NFR-1** (browser-observable acceptance) and **ODD-9** (the driver),
both first-class in the handoff. Every other check passes.

### Handoff to DEVOPS (platform-architect)

1. **Data collection requirements**: none server-side (client-only feature, zero server delta). The one real
   need is a **browser-capable acceptance driver** wired into `cargo xtask ci` — **ODD-9**. Per `AGENTS.md`, if a
   gate belongs in CI it belongs in `cargo xtask ci` so it runs locally too; **never add it to `ci.yml` alone**.
2. **Dashboard / monitoring needs**: none. No new metric series; the `/metrics` sidecar is untouched.
3. **Alerting thresholds**: none new. The guardrails are **test litmuses**, not runtime alerts —
   KPI-2 (typing never captured) and KPI-5 (bound == advertised) fail the build, which is the right place for
   them: a keystroke-eating regression must never reach a user, and there is no runtime signal that would catch
   it anyway.
4. **Baseline measurement**: already established and unambiguous — **0 of 7 advertised shortcuts work**, verified
   by grep (zero `keydown` handlers in application code; the only match is inside the vendored `alpine.min.js`).
   No pre-release measurement window is needed.
