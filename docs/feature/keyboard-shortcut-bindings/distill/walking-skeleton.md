# DISTILL — Walking Skeleton: keyboard-shortcut-bindings

> There is NO product walking skeleton (DISCUSS D8, DESIGN confirmed): this is a brownfield feature — the
> three server contracts are shipped, routed and green. For DISTILL the "skeleton" is the FIRST browser
> scenario that proves the whole INSTRUMENT works end-to-end. It is the precondition for every other
> scenario being worth writing, and it is what ADR-007 exists to build.

## The skeleton scenario (slice 01, scenario #1)

```gherkin
@pending @needs-browser @slice1 @us-01 @lane-probe @walking_skeleton @driving_port @real-io
Scenario: The browser lane can drive a real key against the served app end to end
  Given the browser lane has started chromedriver and navigated to the AUTH board
  Then the page reports the keyboard layer is ready
  And Mei is still signed in after the browser accepts the session cookie over plain HTTP
  When Mei presses "?"
  Then the keyboard shortcut list appears as an overlay over the board
```

## Why this is the right end-to-end proof

It is NOT "do the layers connect" — the server layers already connect and ship green. It answers "can the
INSTRUMENT press a key and observe a user-visible outcome?", which is the exact capability whose absence let
seven advertised shortcuts ship unbound (ODD-9, the root cause). The single scenario forces DELIVER to stand
up every part of the lane and prove they compose:

1. **chromedriver up** — the host prerequisite the xtask preflight guards (probe-then-refuse; a
   missing/skewed driver is a HARD failure, never a skip).
2. **`InProcHarness` served on its real port** — reused AS-IS. `InProcHarness::spawn` already binds
   `127.0.0.1:0` + `axum::serve` and exposes `base_url()` (`foundry-app/src/lib.rs:726-746`,
   `harness.rs:442-444`). No new serving plumbing (upstream-changes §1). One app-construction path means the
   browser lane and the port-to-port suite exercise the SAME app.
3. **Navigate signed-in** — the `BrowserHarness` signs Mei in and drives a `fantoccini` session to
   `base_url()`.
4. **`[data-kb-ready]` appears** — the ADR-001 readiness marker. The lane WAITS on this condition (never a
   sleep) and it doubles as US-02's "the layer is live" precondition.
5. **The Secure-cookie-over-plain-HTTP probe** — `harness.rs:401-406` emits `Secure` on the session cookie
   over plain HTTP; `reqwest` ignores it, a real browser MAY refuse it. Signing in and asserting STILL
   signed in makes a substrate change fail as ONE clear diagnostic at lane start rather than as every
   scenario mysteriously failing at sign-in.
6. **Press `?` -> the help overlay appears** — the first real key-pressed -> user-visible outcome, closing
   the loop the reqwest+scraper suite never could.

## Walking Skeleton Strategy (Architecture of Reference)

**REAL driving + REAL driven-internal via the production composition root.** The lane uses the shipped
`InProcHarness` (real axum app + testcontainers Postgres) and a real browser session. There are no doubles:
selection never reaches the server (BR-5), and the three routes `?`/`c`/`/` exercise are the shipped
handlers, byte-for-byte unchanged. The only NEW substrate is the browser+driver, whose contract is enforced
by the ADR-007 probe (the correct instrument for a driver binary), not a consumer-driven contract.

## Litmus (non-technical stakeholder): "yes, that is what users need"

Mei reads the help page, presses `?` on the board, and the shortcut list appears right where she is —
without leaving her work. A stakeholder confirms that is the job (`fast-keyboard-issue-flow`), stated
without a single technical term. The skeleton also demonstrates, to an engineer, that the gate can now
press a key — so the class of bug "advertised in the help page, never bound in the browser" becomes
impossible to ship green.

## Why the skeleton carries the feature's uncertainty

Everything downstream is an increment on the lane this scenario stands up:
- Slice 02 (guards) reuses the same session + the `[data-kb-ready]` hook for its paired assertion.
- Slices 03/04/05 add key presses and DOM assertions against the same `BrowserHarness`.
- The `@grep-litmus` retirement (#38) and the `@manual` real-IME / real-screen-reader drill (#39) are the
  two things the lane deliberately does NOT cover — the source-tree check and the substrates CI cannot
  faithfully drive — and are called out precisely so nobody mistakes a green lane for their absence.

Build order is fixed by ADR-007: **the lane lands FIRST (slice 01) or nothing else is assertable.**
