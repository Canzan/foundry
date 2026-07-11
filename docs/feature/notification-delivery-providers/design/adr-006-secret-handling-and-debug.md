# ADR-006: Secret-safe port (drop Debug + SecretString) + Earned-Trust startup probe

## Status
Accepted (DESIGN, Propose mode). Resolves **ODD-8** (Risk R3); specifies the Earned-Trust probe contract
(nw-solution-architect principle 12) that ODD-3/ODD-2 depend on.

## Context
Provider secrets — `SMTP_PASSWORD`, `WEBHOOK_SIGNING_SECRET`, `EMAIL_API_KEY` — must **never** appear in logs,
errors, metric labels, or `Debug` output (NFR-2, BR-4, R3). The shipped `EmailSender` port carries a `Debug`
supertrait (`email.rs:19`: `Send + Sync + Debug + 'static`), which makes a secret-holding provider a
leak-by-default hazard: a derived `#[derive(Debug)]` on an adapter, or any `{:?}` of the trait object, would
print the secret. ODD-8 asks: keep the `Debug` bound and hand-write redacting `Debug` impls, or drop the
bound? Separately, principle 12 (Earned Trust) requires every driven adapter that depends on something external
(SMTP relay, webhook endpoint, vendor API) to **demonstrate empirically at startup that it can honor its
contract in the real environment** — a `probe()`.

Shipped seams grounded: `secrecy::SecretString` 0.10 is already a dep (`Cargo.toml:68`) and already wraps
`SESSION_SECRET` (`main.rs:253`) and the machine-token signing key (`main.rs:208`, wrapped, "NEVER logged on
any path"). The Earned-Trust probe idiom is shipped: `PROBE_NAMES = &["store","metrics","machine_token"]`
(`main.rs:46`), `health.startup.refused` structured events + `PROBE_FAILURES_TOTAL{probe_name}`
(`main.rs:178-232,320-324`), and the metrics self-scrape probe (`metrics_server.rs:227-340`). `AppState`
derives only `Clone`, NOT `Debug` (`lib.rs:65`) — so nothing requires the notifier field to be `Debug`.

## Decision — secret handling (ODD-8)
**Drop the `Debug` supertrait from `NotificationProvider`** (`Send + Sync + 'static` only) **and wrap every
provider secret in `secrecy::SecretString`.** Five defense-in-depth layers:
1. **Port not `Debug`-bound** → `Arc<dyn NotificationProvider>` and `Notifier` are never `{:?}`-formatted; a
   future `#[derive(Debug)]` on an adapter cannot leak through the trait object. (Safe: `AppState` derives only
   `Clone`; dropping the bound compiles.)
2. **Secrets in `SecretString`** → even a concrete adapter that derives `Debug` for its own tests prints
   `SecretString([REDACTED])`, never the value.
3. **`DeliveryError` messages hand-built + secret-free** (ADR-003/004) → no `anyhow`/`lettre`/`reqwest` error
   (which can render a credentialed URL) is passed through raw; the adapter constructs an operator-safe string.
4. **Metric labels are the bounded enums only** (ADR-004) → no value path from a secret to a label.
5. **The revert-reds-it no-leak @property litmus** → a full four-provider deliver cycle's logs + `/metrics`
   scrape contain no `SMTP_PASSWORD`/`WEBHOOK_SIGNING_SECRET`/`EMAIL_API_KEY` value and no reset/invite token;
   removing a redaction REDs it.

**Where secrets are read**: once, at the composition root (`build_notifier`, ADR-002), each secret env var is
read and **immediately** wrapped in `SecretString`, then moved into the adapter. The adapter calls
`.expose_secret()` ONLY at the point of constructing the transport request (SMTP auth, the HMAC key, the
`Authorization` header) — never in a log/error/label/Debug path.

## Decision — Earned-Trust startup probe (principle 12)
**Every provider implements `async fn probe(&self) -> Result<(), DeliveryError>`, and `build_notifier`
enforces the invariant "wire → probe → use": a constructed provider enters the active set only after its probe
passes.** A probe result is handled exactly like the shipped store/metrics/machine_token probes:
`health.startup.passed | degraded | refused` structured events + `PROBE_FAILURES_TOTAL{probe_name}`.

Probe content per adapter (exercise the specific substrate lie, do NOT side-effect the channel):
- **`log`**: trivially `Ok` (stdout is always writable) — or write a probe marker line. No external dep.
- **`smtp`**: open a TCP + TLS connection to `SMTP_HOST:SMTP_PORT` and complete the SMTP EHLO/handshake (and,
  where the relay permits, AUTH) **without sending mail**. Lie caught: "the relay is configured but
  unreachable / rejects the credentials."
- **`webhook`**: DNS-resolve + TCP-connect to the `WEBHOOK_URL` host (no POST — a webhook receiver must not be
  side-effected by a health ping). Lie caught: "the URL is a typo / the host is unreachable."
- **`email_api`**: DNS-resolve + TCP+TLS-connect to the `EMAIL_API_URL` host (no send). Lie caught: "the
  endpoint is wrong / unreachable." (A lightweight authenticated HEAD MAY be used where the vendor documents a
  side-effect-free health route.)

**Config-strict / network-soft policy** — the honest reconciliation of Earned-Trust-probe with
best-effort-delivery (NFR-3/NFR-6): the **config half** of every probe (required settings present, URL parses,
`from` parses, secret present) is **always fail-fast** (a config lie refuses startup — NFR-1). The **network
half** (SMTP handshake, webhook/vendor connect) is by default a **`health.startup.degraded`** signal (warn +
counted, app still boots) — because best-effort delivery means Foundry must still start and serve the app even
if an operator's SMTP relay is *currently* down; refusing to boot the whole app because a chat webhook is
briefly unreachable would be worse than the best-effort contract promises. `NOTIFICATION_PROBE_STRICT=true`
promotes network-probe failure to a hard `health.startup.refused` for operators who prefer fail-fast-on-unreachable.

**Three-layer enforcement of the probe contract** (semantically orthogonal, single-bypass-safe):
1. **Subtype** — `mypy`-equivalent in Rust: `build_notifier` is typed so a provider only becomes
   `Arc<dyn NotificationProvider>` in the active `Vec` after `probe()` returns `Ok` (the "probed" state is the
   only constructor of an active entry). The trait *requires* `probe`, so the compiler enforces presence.
2. **Structural** — a `cargo xtask check-arch` / pre-commit AST check walks `notify/` adapter sources: every
   `impl NotificationProvider` must have a non-empty `probe` body (not `Ok(())` for the network-dependent
   adapters), and no adapter `Debug`-formats `self` or interpolates an exposed secret.
3. **Behavioral** — a CI test exercising catalogued substrate lies: a provider pointed at an unreachable host
   fails its probe → `build_notifier` refuses (strict) / degrades (default) with the structured event; the
   no-leak @property confirms the refusal/degrade message carries no secret. (`import-linter`-style import
   checks are insufficient — they cannot assert method-presence or behavior; hence the AST + behavioral layers.)

**Self-application**: layer 3 includes a test that a provider claiming to probe actually connects (a probe
that returns `Ok` without touching the substrate fails the gold test) — the probe-of-the-probe.

`PROBE_NAMES` (`main.rs:46`) is extended with the bounded set `{notification_config, notification_smtp,
notification_webhook, notification_email_api}` (still bounded — the cardinality guard holds).

## Alternatives Considered
- **Keep the `Debug` supertrait + hand-written redacting `Debug` per adapter** — REJECTED. It relies on every
  current AND future contributor remembering to hand-write a redacting `Debug` (and to keep it correct as
  fields change) — an architecture rule with no enforcement, which erodes (principle 11). Dropping the bound
  removes the leak *class* by construction; `SecretString` redaction is then belt-and-suspenders.
- **Drop `Debug` but keep secrets as plain `String`** — REJECTED. Layer 1 alone protects the trait object, but
  a plain `String` secret could still leak via a hand-written adapter log or an `anyhow` passthrough.
  `SecretString` (layer 2) + secret-free `DeliveryError` (layer 3) close those paths; it is already the house
  idiom for `SESSION_SECRET`/signer.
- **No startup probe (validate config only, discover reachability at first delivery)** — REJECTED (principle
  12). "Every dependency you don't probe is an act of faith you made for the user." A misconfigured relay would
  be discovered only when Maria's reset silently fails; the probe surfaces it at boot. The network-soft default
  keeps this honest against the best-effort contract.
- **Hard-fail startup on any network-probe failure (no soft default)** — REJECTED as the default. It would make
  Foundry's boot depend on every configured external transport being up *right now* — a transient relay blip
  would take the whole app down, worse than best-effort promises. Offered as opt-in (`NOTIFICATION_PROBE_STRICT`).
- **A probe that sends a real test notification** — REJECTED. It would side-effect the channel (a real email /
  a real chat post on every boot) — unacceptable. Probes are reachability/handshake only, never a delivery.

## Consequences
- Positive: secret leakage is closed by construction (no `Debug` on the port) + redaction (`SecretString`) +
  secret-free errors + bounded labels — five independent layers (R3, NFR-2, BR-4). The design reuses the exact
  shipped secret idiom (`SecretString` for `SESSION_SECRET`/signer) and the exact shipped probe idiom
  (`health.startup.*` + `PROBE_FAILURES_TOTAL`).
- Positive: config lies fail fast (NFR-1); substrate lies are surfaced at boot (Earned Trust) without breaking
  best-effort (network-soft default). The probe contract is enforced at three orthogonal layers, so a bypass in
  one is caught by another (principle 11/12).
- Negative: dropping `Debug` means the notifier/providers can't be `{:?}`-dumped in an ad-hoc debug print —
  accepted (that is the point); operators get the structured log line + metric instead.
- Negative: `PROBE_STRICT` is a second boot-behavior mode to document + test — accepted; it is one bounded env
  flag with a clear default.
- Probe (Earned Trust): a startup with `smtp` pointed at an unreachable host emits `health.startup.degraded
  probe=notification_smtp` (default) or refuses under strict; a full four-provider deliver cycle's logs +
  `/metrics` contain no secret value and no token (revert-reds-it, @property secret non-leakage, AC-01.3/02.6/
  04.2/05.3); a provider whose `probe` returns `Ok` without connecting fails the self-application gold test.
