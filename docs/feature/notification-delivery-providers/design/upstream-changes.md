# Upstream Changes — notification-delivery-providers

DESIGN-wave findings where the shipped code diverges from, or refines, what the DISCUSS artifacts recorded.
Per trunk-based policy, parent DISCUSS docs are NOT modified; the corrections are recorded here for
DISTILL/DELIVER. None changes the feature's behavior; each either CONFIRMS a DISCUSS assumption by direct
`file:line` verification or REDUCES the feature's footprint. **No DISCUSS doc is edited; no shipped code is
modified by this DESIGN pass (it is a design document only).**

## Verification verdict — every DISCUSS citation is accurate

All seams cited in `../discuss/requirements.md` (grounding table) were re-verified against the shipped tree
and are **correct as stated**: `EmailSender`/`NoopEmailSender`/`FakeEmailSender` (`email.rs:19-22,26-34,38-89`),
the injection point (`main.rs:265`), `AppState.email` (`lib.rs:92`), the port re-export (`lib.rs:63`), the
three call sites (`signin.rs:235`, `bootstrap.rs:258`, `member_invites.rs:189`), the declared-unused `lettre`
(`Cargo.toml:85-90`), the metrics facade/sidecar (`metrics_server.rs:45-77`), the token-mutations emission
(`rate_limit.rs:98,198-203`), register-at-0 (`main.rs:355-369`), the ADR-011 cardinality test
(`metrics_server.rs:99-108,374-428`), the `EventPayload` envelope (`foundry-realtime/src/lib.rs:66-105`), the
`outbox` gauge (`main.rs:29`), and the env/`std::env::var` config style (`main.rs:99,102-262`). The honest
brownfield caveat (no real SMTP transport today; `main.rs:265` = `NoopEmailSender`) is re-affirmed.

## Finding 1 — the HTTP client is `reqwest` 0.12, ALREADY a workspace dep (resolves a DISCUSS open question)

The task brief asked DESIGN to "verify what HTTP client the repo already uses, don't add a redundant one." The
DISCUSS artifacts left the webhook/hosted-API transport client unspecified.

> Original (DISCUSS `requirements.md` FR-7/FR-8, `user-stories.md` US-04/05): "adds an HTTP client transport"
> (client unnamed).

**Finding**: `reqwest = { version = "0.12", ... "rustls-tls", "json", ... }` is already declared at
`Cargo.toml:104`. The webhook + hosted-email-API adapters **reuse it** — **no new HTTP client** is added
(architecture.md tech-stack, ADR-003 context). This REDUCES footprint (the DISCUSS "adds an HTTP client
transport" becomes "reuses the present reqwest"). Note the `metrics_server.rs:227` probe hand-rolls an HTTP GET
specifically to avoid a *production-side* reqwest dep in that DEVOPS slice; here reqwest is already a
production-reachable dep and is the correct client for arbitrary vendor APIs.

## Finding 2 — `lettre`'s declared feature set already covers async tokio rustls SMTP (no feature change)

> Original (DISCUSS grounding table): "`lettre` dependency — declared but never called ... workspace
> `Cargo.toml:85-90`."

**Finding**: the declared features are `tokio1`, `tokio1-rustls-tls`, `smtp-transport`, `builder`
(`Cargo.toml:85-90`) — already sufficient for `AsyncSmtpTransport<Tokio1Executor>` over rustls. The
`SmtpProvider` realizes the transport with **no feature change and no new crate** (architecture.md tech-stack).
This CONFIRMS the DISCUSS "realize `lettre` behind the port" at zero dependency cost.

## Finding 3 — `hmac`/`sha2`/`secrecy` are all present ⇒ ZERO new crates for the whole feature

> Original (DISCUSS `user-stories.md` US-04 technical notes): webhook "optionally signed with
> `WEBHOOK_SIGNING_SECRET`" (signing crate unspecified); ODD-8 "secret-safe `Debug`" (mechanism unspecified).

**Finding**: `hmac = "0.12"`, `sha2 = "0.10"`, `secrecy = "0.10"` are all declared (`Cargo.toml:61-62,68`).
Webhook HMAC-SHA256 signing reuses hmac/sha2; secret wrapping reuses `SecretString` (already wrapping
`SESSION_SECRET` at `main.rs:253` and the machine-token signer at `main.rs:208`). Combined with Findings 1–2,
the feature adds **ZERO new crates**. This CONFIRMS and strengthens the DISCUSS reuse-over-reinvent verdict.

## Finding 4 — `AppState` is `Clone`-only (not `Debug`) ⇒ dropping the port `Debug` supertrait is free

> Original (DISCUSS `wave-decisions.md` D6 / ODD-8): "The port's `Debug` supertrait (`email.rs:19`) makes
> secret-safe `Debug` a first-class concern" — framed as a choice between a redacting `Debug` and dropping the
> bound.

**Finding**: `AppState` derives only `#[derive(Clone)]` (`lib.rs:65`), and no shipped code `{:?}`-formats
`AppState.email` or relies on `EmailSender: Debug`. So **dropping** the `Debug` supertrait from the generalized
`NotificationProvider` (ADR-006, the chosen ODD-8 resolution) compiles cleanly with no ripple. This REFINES the
ODD-8 framing: dropping the bound is not just safer than a redacting `Debug` — it is also *free* (no dependent
code requires it).

## Finding 5 — the Earned-Trust probe idiom is already shipped ⇒ provider probes plug straight in

> Original (DISCUSS): no probe/health-startup concept is mentioned for providers (the DISCUSS predates the
> DESIGN Earned-Trust obligation).

**Finding**: the repo ships a full Earned-Trust startup-probe idiom — `PROBE_NAMES` (`main.rs:46`),
`health.startup.refused|passed` structured events + `PROBE_FAILURES_TOTAL{probe_name}`
(`main.rs:178-232,320-324`), and the metrics self-scrape probe (`metrics_server.rs:227-340`). The provider
`probe()` contract (ADR-006) reuses this idiom verbatim (extending the bounded `PROBE_NAMES` set with
`notification_*`). This is an ADDITION the DESIGN wave makes per principle 12, grounded in a shipped pattern —
not a divergence from DISCUSS, recorded so DISTILL/DELIVER expect provider probes at boot.

## Net effect

All five findings CONFIRM the reuse-heavy verdict and REDUCE scope to: ONE new port + envelope, ONE dispatcher,
FOUR adapters, ONE composition-root config+probe loader, ONE delivery-metric seam — **ZERO new crates, ZERO
migration, ZERO new infra**. The one honest brownfield caveat (no real SMTP transport today; slice 02 realizes
it) is re-affirmed, not changed. No DISCUSS doc is edited; no shipped code is modified by this DESIGN pass.
