# ADR-002: Provider registry + env-config schema + fail-fast validation

## Status
Accepted (DESIGN, Propose mode). Resolves **ODD-2** (Risk R4).

## Context
Providers are selected by configuration (FR-2, BR-5), read in the house 12-factor style — direct
`std::env::var` at the composition root (`main.rs:99,102-262`), no config file, no figment (the DISCUSS
alternatives already rejected a config file). The registry must turn a **listed** provider into a
**constructed/active** one, represent the active set in a deterministic order, and validate at startup so a
misconfigured or unknown provider fails fast (NFR-1, BR-6) instead of silently disabling a channel or crashing
mid-request. Unset selection must reproduce today's `NoopEmailSender` (BR-1, NFR-5).

The shipped precedents grounded: `SESSION_SECRET`/`DATABASE_URL`/`MACHINE_TOKEN_*` are read with
`std::env::var(..).context(..)?` and a malformed value refuses startup (`main.rs:151-232`); the
machine-token block is the exact "read → validate → refuse-with-structured-event" idiom to mirror.

## Decision
**Env schema** (all `std::env::var`):

| Key | Role |
|---|---|
| `NOTIFICATION_PROVIDERS` | comma-separated, ORDERED list of provider kinds; each entry trimmed + lowercased. Unset/empty ⇒ zero active providers (Noop-equivalent). |
| `SMTP_HOST` / `SMTP_PORT` (default 587) / `SMTP_USERNAME` / `SMTP_PASSWORD` / `SMTP_FROM` | `smtp` provider settings; `SMTP_PASSWORD` → `SecretString` (ADR-006). |
| `WEBHOOK_URL` / `WEBHOOK_SIGNING_SECRET` (optional) | `webhook` provider; signing secret → `SecretString`. |
| `EMAIL_API_URL` / `EMAIL_API_KEY` / `EMAIL_API_FROM` | `email_api` provider; key → `SecretString`. |
| `NOTIFICATION_DELIVERY_TIMEOUT_MS` (default 5000) | per-provider fan-out timeout (ADR-003). |
| `NOTIFICATION_PROBE_STRICT` (default false) | promote a network-probe failure to a hard startup refusal (ADR-006). |

**Registry construction** — a `build_notifier()` fn at the composition root:
1. Read `NOTIFICATION_PROVIDERS`; split on `,`, trim, lowercase, drop empties. Empty ⇒ `Notifier` with an
   empty active set (Noop-equivalent) — return early.
2. For each name **in list order**: match against the known kinds `{log, smtp, webhook, email_api}`. An
   **unknown** name → fail-fast: `unknown notification provider "<name>" (known: log, smtp, webhook,
   email_api)` → `health.startup.refused` + `PROBE_FAILURES_TOTAL{probe_name="notification_config"}` + non-zero
   exit.
3. For a known name: read + validate its required env. A **missing required setting** → fail-fast:
   `notification provider "<kind>" is missing required setting <VAR>` (the VAR name, **never** the value) →
   `health.startup.refused` + non-zero exit (NFR-1). Secrets read here are wrapped in `SecretString`
   immediately.
4. Construct the adapter; **probe it** (ADR-006, wire→probe→use). Only a validly-configured, successfully-probed
   provider enters the active set.
5. Assemble `Notifier { providers: Vec<Arc<dyn NotificationProvider>>, delivery_timeout }` — the vector is
   **ordered by the list**, giving deterministic log/metric/test ordering.

**Active-set representation**: `Vec<Arc<dyn NotificationProvider>>` (ordered). "Active" ≡ listed ∧
validly-configured ∧ probe-passed. A provider not listed is **never constructed** (BR-1) — it reads no env and
holds no secret. Injected at `main.rs:265` as `AppState.notifier: Arc<Notifier>`, generalizing `AppState.email`.

## Alternatives Considered
- **A YAML/figment provider-config file** — REJECTED. No config-file loader exists anywhere in the app; every
  setting is `std::env::var`. A file would break operator ergonomics (12-factor parity with `DATABASE_URL`,
  `SMTP_*`-to-be) and add a parser + schema for no gain. (DISCUSS alternative, re-affirmed.)
- **Lazy/first-use construction (build a provider on its first delivery)** — REJECTED. It defers config errors
  from startup to the first notification — the opposite of fail-fast (NFR-1); a typo would silently disable a
  channel until the first reset request. Eager construct-and-probe at boot surfaces the misconfig immediately.
- **Unknown provider name = warn-and-skip** — REJECTED. A fat-fingered `logg` would silently disable
  notifications (the US-01 boundary scenario). Fail-fast on unknown names is typo protection (NFR-1, BR-6).
- **A `HashMap<ProviderKind, Arc<dyn ..>>` active set** — REJECTED. A map loses the operator's list order
  (nondeterministic log/metric ordering, flakier tests). An ordered `Vec` preserves intent and determinism.
- **Read secrets inside each adapter (adapter calls `std::env::var`)** — REJECTED. It scatters secret reads
  across adapters and breaks the composition-root discipline; centralizing the reads in `build_notifier`
  (wrapping in `SecretString` at the boundary) is the single-choke-point for ADR-006.

## Consequences
- Positive: startup is the single validation gate — a misconfigured or unknown provider can never reach a
  request handler (NFR-1, BR-6); an un-configured deploy is exactly today's Noop (BR-1, NFR-5).
- Positive: mirrors the shipped `MACHINE_TOKEN_*` read→validate→refuse idiom, including the structured
  `health.startup.refused` event and the probe-failure counter — one consistent operability posture.
- Negative: adding a provider kind is a code change (a new `match` arm + adapter), not pure config — accepted
  and intended (BR-7 bounded set; a new transport is a reviewed addition, not an env string).
- Probe (Earned Trust): `NOTIFICATION_PROVIDERS=smtp` with `SMTP_HOST` unset aborts non-zero naming `smtp` +
  `SMTP_HOST` with no secret printed; `NOTIFICATION_PROVIDERS=logg` aborts naming the unknown provider + the
  known set; `NOTIFICATION_PROVIDERS` unset boots and delivers nothing (revert-reds-it @property config
  fail-fast, AC-01.4/01.5/02.4).
