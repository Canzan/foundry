# ADR-EIP-001: One delivery-channel contract for every integration kind, a pure core crate, and a sibling of NotificationProvider rather than an extension of it

## Status

Proposed (2026-10-21, external-integration-plugins DESIGN, propose mode). Awaiting user
confirmation through the feature's DD list.

## Context

DISCUSS (OD-1, accepted) asks DESIGN to model the plugin contract once for every kind
(Slack, Matrix, member email, webhook, and the OAuth app as a non-delivering kind), so that
deliverables B and C add kinds without rework. CA-2 states that the shipped
`NotificationProvider` port (`crates/foundry-app/src/notify.rs`) is not that contract:

- it carries one `Notification` with one named recipient, built inside the request;
- `Notifier::notify` is infallible and best-effort at-most-once (ADR-007 of
  notification-delivery-providers), so it returns no outcome a retry layer could use;
- its providers are configured by environment, instance-wide.

Routes have a destination per route, many routes per event, asynchronous delivery,
per-workspace connections and a classified outcome that drives retries, pauses and the
connection-lost path (US-EIP-05, -06). The content rules (D-3) are the security heart of the
feature (R5) and must be property-testable without a network.

Quality attributes in play: security (no secret or mention leak, SC-3, SC-4), testability
(renderers and classifiers proven by property tests), maintainability (a new kind is one
adapter plus one encoder), and the existing ports-and-adapters shape (brief.md).

## Options considered

1. **Extend `NotificationProvider`** with project events and an outcome. Rejected: every
   shipped provider and the `Notifier` fan-out would change, SC-7 says the five account
   notifications must behave exactly as before, and the port's one-recipient shape would
   leak into routes.
2. **One fat trait per kind** that connects, lists destinations, renders, delivers and
   classifies. Rejected: it mixes read-only and writing effects behind one interface
   (Principle 12: a port that "only reads" must not expose writes), and puts the content
   rules inside effectful code where a property test needs a network double.
3. **A split contract (chosen):** a kind-independent pure composer, per-kind pure encoders and
   classifiers, a read-only destination catalogue port, and a write-only delivery port.

## Decision

1. **The contract has four parts, all keyed by the closed enum `ChannelKind`**
   (`slack`, `matrix`, `email`, `webhook`; `webhook` is defined now and implemented by
   deliverable C):
   - `compose(EventView, ContentLevel) -> MessageLines`: **pure, kind-independent.** It alone
     decides which fields a message carries (D-3 levels, the never-sent list, the 300-character
     comment cut, the "A Foundry member" fallback, the burst summary of D-11). The route
     preview renders exactly these lines, so "the preview equals the message" holds by
     construction.
   - `encode_<kind>(MessageLines) -> <KindPayload>`: **pure, per kind.** It alone owns the
     kind's escaping and the never-pings rule (Slack: `plain_text` objects, `&`/`<`/`>`
     escaped, `parse: none`, `link_names: false`, the link as a button URL; Matrix: HTML-escaped
     `formatted_body`, plain `body`, `msgtype: m.notice`, an empty `m.mentions`, a word joiner
     after `@` in user text; email: plain text; webhook: the versioned JSON schema).
   - `DestinationCatalog` (driven port, **strictly read-only**): verify a connection, list
     destinations, check one destination (exists, Foundry can post, shared with another
     organisation, encrypted). Used by the route form, the route page and "Resume". It has no
     method that changes anything at the vendor.
   - `DestinationEnrolment` (driven port, **write, bounded**): `enrol(destination)` and
     `withdraw(destination)`. Matrix implements them as room join and leave; Slack's are no-ops
     (the bot posts with `chat:write.public`, or the lead invites it). Only the route save and
     delete use-cases call it. Universe: the bot's membership of that one room.
   - `DeliveryChannel` (driven port, **write-only**): `deliver(destination, payload, delivery_key)
     -> ChannelOutcome`, `kind()`, and `probe()`. The port names no credential type, so the pure
     crate needs no secret dependency. A per-connection channel is built in foundry-app by a
     `ChannelFactory`, which opens the sealed credential and holds it inside the adapter. `probe()` has **no default
     body**, so a kind that does not implement it does not compile.
   - `ChannelOutcome` (named to avoid the shipped `notify::DeliveryOutcome`) is a closed enum
     produced by a **pure per-kind classifier** from the vendor's answer: `Delivered`,
     `Transient { reason, retry_after }`, `RouteRefused { reason }`,
     `ConnectionRefused { reason }`. Each classifier maps a **closed list** of known
     destination-level and connection-level codes; every other answer, including unknown error
     codes, is `Transient` (bounded by the 24-hour give-up), so an unrecognised vendor fault never
     parks a route. Reasons are a closed `ReasonCode` enum mapped to Foundry's words; a vendor's
     raw text never leaves the adapter (US-EIP-05 AC 5).
   - The metric `outcome` label is derived from the delivery's resulting state, not from
     `ChannelOutcome` directly: `Delivered` → `delivered`; `Transient` with a later attempt →
     `retried`; any terminal failure → `failed`; a send-time skip → `skipped`. The shipped
     `notify::DeliveryOutcome` (`delivered`, `failed`) is untouched.
2. **Composition reads a hydrated `EventView`, never the outbox payload.** The view is built
   at send time from the store (current names, D-8) plus the few fields that only the payload
   can still hold (ADR-EIP-003). The outbox payload carries `author_email` for comments today;
   no encoder can reach it because no encoder receives the payload.
3. **What is shared with the notification port (CA-2):** the transport adapters and the error
   taxonomy, not the port. `SmtpProvider` and `EmailApiProvider` additionally implement a new
   narrow `EmailTransport` port (ADR-EIP-007); `DeliveryError::Transient | Permanent` maps
   onto `DeliveryOutcome`. `Notifier`, `NotificationEvent` and the env `WebhookProvider` are
   unchanged except for one new suppressible account event (D-15).
4. **Crate placement:**
   - new crate **`foundry-integrations`**: domain types (`ChannelKind`, `EventKind`,
     `ContentLevel`, `RouteSpec`, `ReasonCode`, `RouteStatus`), the port traits, and every pure
     function (compose, encoders, classifiers, route matching, burst coalescing, retry policy,
     status derivation, outbound address judgement). It depends on `foundry-core`, `serde`,
     `serde_json`, `async-trait`, `uuid`, `time` and `ipnet` only. No `tokio`, `reqwest`, `sqlx`,
     `lettre`, `foundry-store`;
   - `foundry-store`, module `integrations.rs`: tables, the dispatch and claim transactions;
   - `foundry-services`, module `integrations.rs`: the authority checks (`TeamAccess`,
     workspace admin), route and connection use-cases and their audit writes. Deliverable C's
     `/api/v1` handlers call the same use-cases (OD-14, Q9);
   - `foundry-app`, directory `src/integrations/`: the Slack, Matrix and email adapters, the
     secret box wiring, the outbound HTTP client, the delivery worker, the HTML handlers and
     the composition in `main.rs`.
5. **Restricted store capabilities (Principle 12e).** Worker components never hold `Arc<Store>`.
   Following the shipped `StoreSuppression` precedent (a foundry-app adapter wrapping `Arc<Store>`
   behind a narrow trait), the composition root injects one narrow trait per component:
   - `EventViewReads` (Hydrator): read-only;
   - `DispatchLog` (Dispatcher): one method, which takes undispatched groups, applies a plan and
     marks them in one transaction;
   - `DeliveryClaims` (Sender): claim and fenced record only;
   - `DeliverySweeps` (sweeps): expire, cancel and prune only;
   - `ConnectionHealth`: the `connected → lost` compare-and-set only.
   The traits are defined in `foundry-app/src/integrations/` and implemented by thin wrappers over
   `Arc<Store>` in one module, `store_capabilities.rs`.
   The use-cases in `foundry-services/src/integrations.rs` follow the same rule. They take narrow
   traits defined in that module (`RouteStore`, `ConnectionStore`, `EmailChoiceStore`, `AuditLog`)
   and implemented for `Store` in one submodule, `integrations/store_impls.rs`. The use-case
   bodies never name `Store`.
6. **Sealing placement.** The connect handlers in foundry-app (Slack return, Matrix connect) are the
   only callers of `SecretBox::seal`. They pass an opaque `SealedSecret` to the services use-case,
   so services and store never hold plaintext. `SealedSecret` is a bytes newtype in `foundry-core`,
   with no `Debug`, `Display` or `Serialize`. `SecretBox` in `foundry-auth` produces and consumes
   it. No new crate edge is needed.

## Consequences

- Positive: the D-3 rules exist in one pure function and the no-ping rules in one pure
  function per kind, each provable by proptest over generated text with no network.
- Positive: Matrix (B) and webhooks (C) add one encoder, one classifier and one adapter pair;
  routes, history, retries and lifecycle do not change (slice 04's learning hypothesis).
- Positive: read and write effects are separate ports. The route page and preview can never send
  or join. The only vendor writes outside the worker are the explicit Send test (its own endpoint,
  bounded to one message), enrolment on save (Matrix join), withdrawal on delete (Matrix leave),
  and Slack `apps.uninstall` on disconnect.
- Negative: one more workspace crate and two new trait objects to wire at the composition root.
- Negative: two email paths coexist (account mail through `Notifier`, project mail through
  `EmailTransport`); they share transports, so a relay change affects both, which is intended.

## Enforcement

- `probe()` without a default body: compile-time.
- check-arch `integrations-pure-core` is an **allow-list**. The `[dependencies]` of
  `crates/foundry-integrations/Cargo.toml` may name only `foundry-core`, `serde`, `serde_json`,
  `async-trait`, `uuid`, `time` and `ipnet`; anything else, including any other Foundry crate, is
  a violation.
- check-arch `integrations-store-capability`: no file under `crates/foundry-app/src/integrations/`
  except `store_capabilities.rs` names `foundry_store::Store`, and no file under
  `crates/foundry-services/src/integrations/` except `store_impls.rs` does. Gold tests plant
  `Arc<Store>` in the hydrator and `&Store` in a use-case.
- `deny.toml`: `foundry-integrations` may be depended on by `foundry-services`, `foundry-app`
  and `foundry-acceptance` only (the rule-3 wrapper shape).
- Proptest suites in `foundry-integrations`: compose never emits a description, attachment
  name, email address or foreign id; every encoder, for arbitrary user text, yields no active
  mention sequence (Slack `<!…>`, `<@…>`, `<#…>`, unescaped `<`; Matrix unescaped HTML,
  literal `@room` in `body`).
