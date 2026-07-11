# DISTILL — Self-Review: notification-delivery-providers

> Quinn (nw-acceptance-designer). Self-review against the AD critique dimensions +
> the design mandates, before DELIVER handoff. 27 scenarios, all `@pending`, compiling
> green (`cargo test -p foundry-acceptance --no-run` → exit 0).

## Coverage of every US / AC / NFR

| Item | Covered by | Status |
|---|---|---|
| US-01 (log walking skeleton) | scenarios 1-4 | ✅ AC-01.1..01.6 |
| US-02 (SMTP) | scenarios 5-9 | ✅ AC-02.1..02.6 |
| US-03 (fan-out + observability, v1 gate) | scenarios 10-15 | ✅ AC-03.1..03.7 |
| US-04 (webhook) | scenarios 16-20 | ✅ AC-04.1..04.5 |
| US-05 (hosted email API) | scenarios 21-23 | ✅ AC-05.1..05.5 |
| US-06 (new events) | scenarios 24-27 | ✅ AC-06.1..06.5 |
| NFR-1 config fail-fast | 3, 7, 20, 23 | ✅ |
| NFR-2 secret non-leakage | 3, 4, 9, 18, 23 | ✅ |
| NFR-3 best-effort isolation | 6, 11, 12, 19, 22, 26 | ✅ |
| NFR-4 bounded observability | 5, 10, 14, 15, 16, 21, 27 | ✅ |
| NFR-5 backwards-compat | 2, 8 | ✅ |
| NFR-6 no retry v1 | 22 | ✅ |
| @property (5/5) | isolation 11/26, non-leak 4/9/18/23, completeness 10, config 3/7/20/23, bounded 15/27 | ✅ |

The six DESIGN "Handoff to DISTILL" pins are all covered: (1) fan-out isolation → 11, 12,
19, 22, 26; (2) config fail-fast → 3, 7, 20, 23; (3) secret non-leakage → 4, 9, 18, 23;
(4) metric zero-series + cardinality → 14, 15, 27; (5) fan-out completeness → 10, 13;
(6) backwards-compat → 2, 8.

## Dimension-by-dimension

- **D1 Happy-path bias** — PASS. 15/27 (~56%) carry `@error`/`@security`/`@config`; every
  provider has a failure/isolation scenario. Above the 40% floor.
- **D2 GWT compliance** — PASS. Each scenario is one Given-context / one When-action / Then
  observable outcomes. The one multi-trigger scenario (13, "each existing notification
  fires") is a deliberate fan-out-completeness batch, not a multi-behavior smell.
- **D3 Business-language purity** — PASS with a noted tolerance. Scenario titles and steps
  speak the operator/notification domain (provider, event, outcome, delivered, failed,
  reset, invite). Config KEYS (`SMTP_HOST`, `WEBHOOK_URL`, `EMAIL_API_KEY`,
  `WEBHOOK_SIGNING_SECRET`, `SMTP_PASSWORD`) and provider slugs (log/smtp/webhook/email_api)
  appear as **operator-facing configuration nouns** — these are the operator's ubiquitous
  language, matching the house precedent where paths/headers appear literally
  (navigation-bar, card-ranking). Transport mechanics (HTTP verbs, JSON, status codes) live
  in step bodies, not Gherkin.
- **D4 Coverage completeness** — PASS. Every US and AC maps to ≥1 scenario (table above).
- **D5 Walking-skeleton user-centricity** — PASS. Scenario 1 is framed as an operator goal
  with observable outcomes (see `walking-skeleton.md` litmus).
- **D6 Priority validation** — PASS. The v1 gate (US-01..03) is pinned first and fully; the
  isolation property (the NFR-3 crux, the design's #1 risk) has the most scenarios.
- **D7 Observable-behaviour assertions** — PASS. Every Then reads a driving-port observable
  (recorder saw a delivery, `/metrics` counter, HTTP response returned, startup exit code,
  log/error/metric text) — never an internal struct field. State-delta/Universe (Mandate 8)
  applies to the layer-1/2 Python pilot; at this layer-3+ real-adapter cucumber-rs suite the
  observable is the port surface, asserted directly (Mandate 8 permits traditional
  assertions at layers 4+, and Mandate 11 keeps these sad paths example-based, never
  PBT-generated).
- **D8 Traceability** — PASS (Check A). Every scenario carries a `@us-0N` tag; every US-0N
  has ≥1 scenario. Check B (environment-to-scenario) is N/A — no DEVOPS wave; the harness is
  the single in-process environment.
- **D9 Walking-skeleton boundary proof** — PASS. WS strategy = real internal app + real
  Postgres, external transports faked in-process (documented above + in `test-scenarios.md`).
  Scenario 1 is `@real-io`; deleting the log adapter would red it.

## Driving-port compliance note (Mandate 1)

The step module imports only `crate::world::FoundryWorld` and `cucumber` — zero internal
notifier/adapter imports. Every scenario enters through one of three driving ports:
operator config (`build_notifier()`), a real shipped app flow, or the `/metrics` sidecar +
recorder. No scenario invokes `Notifier`, `NotificationProvider`, or an adapter directly —
that would be Testing Theater.

## External-double rationale

Per the settled decision, external transports are in-process doubles (recording log
provider, local webhook receiver, fake SMTP/hosted-API recorder), mirroring the shipped
`FakeEmailSender`. This keeps the suite hermetic and deterministic (no real SMTP/SendGrid),
lets a synchronous `Then` observe the delivery (await-bounded fan-out, N-ODD-2), and still
proves the real wiring from config through dispatch to observability. The webhook probe's
"no POST" assertion (scenario 17) and the happy-path "real POST" assertion (16) are the two
halves of the N-ODD-3 watch-item.

## One-at-a-time @pending strategy

All 27 scenarios are `@pending`, excluded from every lane. DELIVER unskips slice-by-slice
(US-01 → US-06), replacing each `panic!` scaffold (RED-ready, Mandate 7) with a real body.
Each scenario is one DELIVER TDD cycle; the `@all` lane stays green until each is turned.

## Scaffold status (Mandate 7)

Step bodies `panic!` (assertion-class = RED, not ImportError-class = BROKEN). The module
carries `__SCAFFOLD__` / `SCAFFOLD: true` markers. When DELIVER unskips a scenario before
implementing, it reds for the RIGHT reason (missing functionality), not a test bug —
satisfying the pre-DELIVER fail-for-the-right-reason gate.

## Compile gate

`cargo test -p foundry-acceptance --no-run` → **exit 0** (acceptance binary links; all 40
unique step phrases registered; no missing fn, no duplicate registration, no type
mismatch). Env note: the sandbox's pyenv `cc` shim shadows the real compiler; the gate was
run with `/usr/bin` prepended to PATH so `cc` resolves to the Xcode toolchain — a toolchain
environment detail, not a code issue.
