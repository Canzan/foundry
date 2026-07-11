# Shared Artifacts Registry — notification-delivery-providers

Every value that flows across the journey (config → registry → emit → fan-out → observe), its single source
of truth, and its consumers. The **provider secrets** and the **delivery outcome/metric** are the
security-and-operability-critical ones. This generalizes the shipped single-`EmailSender` wiring; the DELTA
artifacts (`provider_registry`, `notification`, `delivery_outcome`, `delivery_metric`, `secrets`) are the
pluggable-delivery seam.

```yaml
shared_artifacts:
  provider_config:
    source_of_truth: "environment variables read once at the composition root (crates/foundry-app/src/main.rs:265, alongside the existing env block main.rs:242-262) via direct std::env::var — NO config file, NO figment (house style, BR-5). NOTIFICATION_PROVIDERS (comma list) + per-provider SMTP_*/WEBHOOK_*/EMAIL_API_* settings."
    consumers:
      - "provider registry construction (parse + validate at startup)"
      - "each provider's transport settings (host/port/url/from)"
    owner: "foundry-app configuration (env) — this feature adds the notification keys"
    integration_risk: "MEDIUM — an unset NOTIFICATION_PROVIDERS means NO active providers (Noop-equivalent, BR-1); a listed-but-misconfigured or unknown provider must fail fast (NFR-1), never silently mis-select a channel."
    validation: "With the list unset, no provider is constructed and delivery is a no-op; with a listed provider missing a required setting, startup aborts non-zero naming provider + setting (no secret)."

  secrets:
    source_of_truth: "SMTP_PASSWORD, WEBHOOK_SIGNING_SECRET, EMAIL_API_KEY — environment variables ONLY"
    consumers:
      - "SMTP provider construction (relay auth)"
      - "Webhook provider (payload signature derivation)"
      - "Hosted email API provider (credential header)"
    owner: "operator env — read into the provider at construction, nowhere else"
    integration_risk: "HIGH (security) — these MUST NEVER appear in any log line, error message, metric label, or Debug output. Because the port carries a Debug supertrait (email.rs:19), a provider holding a secret must not derive a Debug that prints it (ODD-8)."
    validation: "A full deliver cycle across all providers leaves no secret value in logs / errors / /metrics / Debug; reverting the redaction REDs the @property no-leak litmus (NFR-2, BR-4)."

  provider_registry:
    source_of_truth: "built once at startup from ${provider_config} — the ordered set of ACTIVE, validly-configured providers (the factory that replaces Arc::new(NoopEmailSender) at main.rs:265)"
    consumers:
      - "the AppState notifier handle (generalizes AppState.email: Arc<dyn EmailSender>, lib.rs:92)"
      - "the fan-out executor invoked at every call site"
    owner: "this feature (the registry + config-selection seam)"
    integration_risk: "HIGH — the active set built at startup must be exactly what fan-out delivers to; a provider silently active (or inactive) is a delivery or a leakage surprise. A provider not listed is NEVER constructed (BR-1)."
    validation: "The providers fan-out delivers to == the providers listed in NOTIFICATION_PROVIDERS and validly configured; unlisted providers are never constructed (config-fail-fast @property)."

  notifier:
    source_of_truth: "AppState notifier handle — generalizes the shipped AppState.email: Arc<dyn EmailSender> (lib.rs:92), injected upstream in main.rs and consumed via build_router (lib.rs:293)"
    consumers:
      - "every notification call site (signin.rs:235, bootstrap.rs:258, member_invites.rs:189, + new event emitters)"
    owner: "foundry-app web tier — the DI field generalized by this feature"
    integration_risk: "MEDIUM — call sites depend on the notifier's best-effort/non-fatal contract being IDENTICAL to the shipped state.email.send (NFR-5). A change to that contract regresses every existing flow."
    validation: "Each call site emits one notification and continues regardless of delivery outcome; existing FakeEmailSender-based acceptance passes with the notifier substituted."

  notification:
    source_of_truth: "constructed at the emit call site — carries the event type + recipient + content. The ONE thing a developer emits (JOB-2). Exact shape (email-centric vs structured) is ODD-1."
    consumers:
      - "the fan-out executor"
      - "every active provider's deliver() (each renders/sends it in its own transport shape)"
    owner: "the emitting call site (writer) -> the providers (readers)"
    integration_risk: "MEDIUM — the payload must serve BOTH email providers (to/subject/body) AND non-email providers (webhook/chat JSON). This is the port-shape tension (R1, ODD-1); slice 01 carries it."
    validation: "A single emitted notification is delivered by every active provider without the call site knowing which transports are active."

  event_type:
    source_of_truth: "the BOUNDED notification catalog (password_reset, workspace_invite, member_invite, + new member_removed, password_changed) — BR-7. Catalog shape mirrors the house forward-compat envelope EventPayload (foundry-realtime/src/lib.rs:66-105); alignment is ODD-6."
    consumers:
      - "the delivery metric 'event' label (must stay bounded, ADR-011)"
      - "the log line (event= field)"
      - "provider routing / rendering"
    owner: "this feature (the catalog) — a new event type is an explicit catalog addition"
    integration_risk: "MEDIUM — the event a call site emits must equal the 'event' metric label so observability is not mislabeled; the set must stay bounded so the metric cardinality is safe (R6)."
    validation: "The event emitted == the 'event' label counted; a cardinality test fails closed if an unbounded event value appears (bounded-metric @property)."

  delivery_outcome:
    source_of_truth: "each provider's deliver() result — delivered | failed (the error taxonomy is ODD-4)"
    consumers:
      - "the best-effort logger (never propagated to the request)"
      - "the delivery metric 'outcome' label"
    owner: "the fan-out executor (records per provider)"
    integration_risk: "HIGH — a provider's actual result must equal the counted outcome, or the metric misrepresents delivery health. A failed outcome MUST NOT propagate to the request or suppress another provider (NFR-3)."
    validation: "For a forced provider failure, the request still returns normally, the other providers still deliver, and the metric shows exactly one failed for that provider (isolation @property)."

  delivery_metric:
    source_of_truth: "metrics::counter!(\"foundry_notification_deliveries_total\", \"provider\" => .., \"event\" => .., \"outcome\" => ..).increment(1) — mirrors foundry_token_mutations_total emission (rate_limit.rs:198-203), registered at 0 with describe_counter! at startup (main.rs:355-363)"
    consumers:
      - "the existing /metrics Prometheus sidecar (metrics_server.rs:66)"
      - "operator dashboards / alert thresholds (DEVOPS)"
    owner: "this feature (the observability seam) -> reuses the shipped metrics facade + sidecar"
    integration_risk: "MEDIUM — labels MUST stay bounded (provider ∈ {log,smtp,webhook,email_api}; event ∈ catalog; outcome ∈ {delivered,failed}) per ADR-011 (metrics_server.rs:99-108); an unbounded label is a cardinality hazard (R6)."
    validation: "Sum over one notification with N active providers == N, split by outcome; the cardinality test (pattern metrics_server.rs:374-428) fails closed on an unbounded label."

  public_url:
    source_of_truth: "application configuration (AppState.public_url / FOUNDRY_PUBLIC_URL, main.rs:122) — the same value the shipped invite emits use (bootstrap::create_invite)"
    consumers:
      - "notification content that embeds a link (e.g. the reset/invite URL delivered by any provider)"
      - "webhook JSON payloads that reference a Foundry resource URL"
    owner: "foundry-app configuration — shipped, reused verbatim"
    integration_risk: "LOW — reused as-is; the link host in a delivered notification is the same public_url the existing emails already use, so consistency is already exercised."
    validation: "A delivered notification's embedded link host equals the configured public_url (as in the shipped create_invite)."
```

## Consistency checks (for DISTILL / DELIVER)

1. Does every `${variable}` in the journey mockups have a documented source above? **Yes** — all 9 tracked
   (`provider_config`, `secrets`, `provider_registry`, `notifier`, `notification`, `event_type`,
   `delivery_outcome`, `delivery_metric`, `public_url`).
2. **Config → registry**: the providers fan-out delivers to == the providers listed in
   `NOTIFICATION_PROVIDERS` and validly configured; unlisted providers are never constructed. (HIGH)
3. **Isolation**: for any active set and any single provider failing, every other active provider still
   delivers AND the request returns normally; zero request failures attributable to delivery. (HIGH — the crux)
4. **Secret non-leakage**: no `SMTP_PASSWORD` / `WEBHOOK_SIGNING_SECRET` / `EMAIL_API_KEY` value appears in any
   log line, error, metric label, or `Debug` output. (HIGH — security)
5. **Event ↔ metric label**: the `event_type` a call site emits equals the `event` label counted; the label
   domains (`provider`,`event`,`outcome`) stay bounded (ADR-011). (MEDIUM)
6. **Outcome fidelity**: each `delivery_outcome` increments exactly one metric series with the matching
   outcome; the sum over one notification with N active providers is N. (HIGH)
7. **Backwards-compat**: with `NOTIFICATION_PROVIDERS` unset, the notifier is a no-op and every existing flow
   behaves exactly as today (NFR-5). (HIGH — regression guard)
