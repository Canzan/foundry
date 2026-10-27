# ADR-EIP-006: Matrix is a per-workspace bot token on the client-server API, refusing encrypted rooms; every admin- or lead-supplied address goes through one outbound address policy enforced at connect time

## Status

Implemented (external-integration-plugins DELIVER, finalized 2026-10-27 at `8c681b2`; not yet
released). The status below stays Proposed until the user accepts it. Departures from this record
are listed in `docs/evolution/2026-10-27-external-integration-plugins.md` (Key decisions).

Proposed (2026-10-21, external-integration-plugins DESIGN). Builds on OD-4 (accepted: the admin enters
homeserver and bot token), OD-5 (accepted: refuse encrypted rooms, K-15) and OD-13 (accepted: loopback,
link-local and metadata always refused; private ranges only through an operator allow-list).

## Context

- Matrix has no app store; a bot account is the integration (US-EIP-08). Element encrypts private
  rooms by default (R9). A homeserver accepts an unencrypted message into an encrypted room, so the
  server will not stop the mistake: Foundry has to check.
- Priya's homeserver is likely on her private network (US-EIP-08 notes), which is exactly where an
  attacker would aim a homeserver or webhook address (R6). Deliverable C's webhooks need the same guard.
- Matrix pings: legacy push rules notify on `@room` in `body` and on a member's display name; with
  intentional mentions (`m.mentions`, Matrix spec v1.7) servers and clients that support it use only
  the explicit mentions list.
- Matrix send is `PUT /_matrix/client/v3/rooms/{room}/send/m.room.message/{txnId}`; the homeserver
  de-duplicates a repeated `txnId` for the same access token.

## Options considered

- **Matrix SDK (`matrix-sdk` crate) vs a thin client on `reqwest`.** The SDK brings end-to-end
  encryption, sync and a large graph; v1 needs `whoami`, `join`, room state reads, `joined_members`
  and `send`. **A thin client on the existing `reqwest` (chosen)**; the SDK is the natural choice
  for F-3 (encrypted rooms).
- **SSRF guard:** (a) validate the URL's host once at save time (rejected: DNS rebinding returns a
  public address at save and a private one at send); (b) **resolve, judge every resolved address and
  connect only to an approved address, on every request, with redirects disabled (chosen)**;
  (c) an egress proxy (operator infrastructure Foundry cannot assume).

## Decision

1. **Connection:** `POST /workspace/integrations/matrix` (admin) with homeserver URL and access token.
   The URL must be `https`; its host must pass the address policy; `GET /_matrix/client/v3/account/whoami`
   must answer a user id. Unreachable, refused and invalid answers save nothing and show the
   US-EIP-08 copy. The token is sealed (ADR-EIP-004). The connection stores homeserver, bot user id and
   when the token was saved. A new token replaces the old one and resumes lost routes.
2. **Route destination check and enrolment**, two ports (ADR-EIP-001):
   - `DestinationCatalog`, read-only, runs when the lead enters a room. It resolves an alias to a
     room id (`GET /directory/room/{alias}`). It reads the bot's membership: if the bot is joined, it
     reads room state; if only invited, it reads the invite's stripped state from a filtered `/sync`.
     Not invited → K-14. `m.room.encryption` present → K-15. Otherwise it returns the room name and
     member count for the confirmation line. Nothing at the homeserver changes.
   - `DestinationEnrolment`, a bounded write, runs only in the route-save use-case after the check
     passed: `POST /join/{room}`. If the join is refused (the invite was withdrawn meanwhile) → K-14
     and nothing saved. If the post-join state shows encryption (turned on in between) → leave,
     K-15, nothing saved. Route delete calls `withdraw` (leave) when no other route of the
     connection uses the room. Universe: the bot's membership of that one room.
   - **Send test for a Matrix route is offered on the saved route, not on the unsaved form.** Before
     save the bot is only invited, and a send would be refused. The route row's "Send test" posts
     K-7 once, with a fresh uuid as `txnId` recorded in no row. Slack routes keep test-before-save
     (US-EIP-02), because Slack posting needs no join.
3. **Before each send** the adapter re-reads `m.room.encryption` (one small GET at homelab volume); an
   encrypted room fails the delivery as `RouteRefused(room_encrypted)` with K-15's reason, never sends.
4. **Message:** `msgtype: m.notice`, plain `body` and `format: org.matrix.custom.html` `formatted_body`
   carrying the K-8 lines (key and title bold, an "Open in Foundry" link), user text HTML-escaped,
   `m.mentions: {}`, and a U+2060 word joiner inserted after every `@` in user text so legacy
   `@room` rules cannot match. `txnId` is the delivery id.
5. **Classifier (closed lists, unknown → transient):**
   - 200 → `Delivered`;
   - 429 / `M_LIMIT_EXCEEDED` → `Transient` with `retry_after_ms`;
   - 401 `M_UNKNOWN_TOKEN` → `ConnectionRefused` (logged-out bot, US-EIP-08 example 5);
   - 403 `M_FORBIDDEN` → `RouteRefused(bot_not_in_room)` (kicked or banned, US-EIP-09 example 4);
   - the pre-send encryption read → `RouteRefused(room_encrypted)`;
   - every other status or `errcode` (5xx, timeouts, non-JSON, unknown codes) → `Transient`, bounded
     by the 24-hour give-up.
6. **Outbound address policy (shared by Matrix now and webhooks in C):**
   - a pure `judge(ip, allow_list) -> Allow | Refuse` in `foundry-integrations`. Always refused:
     loopback (`127.0.0.0/8`, `::1`), "this network" `0.0.0.0/8` and `::`, reserved `240.0.0.0/4`,
     documentation and benchmarking ranges (`192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`,
     `198.18.0.0/15`, `2001:db8::/32`), link-local (`169.254.0.0/16`,
     `fe80::/10`), cloud metadata (`169.254.169.254`, `fd00:ec2::254`), multicast and broadcast. Any
     IPv6 address that embeds an IPv4 address is judged by the embedded IPv4 address: IPv4-mapped
     `::ffff:0:0/96`, IPv4-compatible `::/96`, NAT64 `64:ff9b::/96` and `64:ff9b:1::/48`, and 6to4
     `2002::/16`. Refused unless allow-listed: RFC 1918, CGNAT `100.64.0.0/10`, unique-local
     `fc00::/7`;
   - operator setting `FOUNDRY_OUTBOUND_ALLOW` (comma-separated host names and CIDR ranges, parsed with
     the already-locked `ipnet`); a malformed entry refuses start;
   - **one HTTP client constructor**, `OutboundHttp`, in `foundry-app/src/integrations/`: a `reqwest`
     client with a DNS resolver that resolves the host, judges every address, and hands `reqwest` only
     approved ones (so the connection goes to what was judged); `redirect::Policy::none()`;
     **`no_proxy()`** (reqwest otherwise honours `HTTP_PROXY`, `HTTPS_PROXY` and `ALL_PROXY`, and a
     proxy would resolve the target itself, bypassing the judge); connect 5 s and total 10 s timeouts;
     IP-literal URLs are judged directly. **There is no proxy mode in v1**: an outbound proxy
     resolves names itself, so the judge could not pin the connection, and the SSRF guarantee would
     not hold. Egress through a proxy is a follow-up that needs its own trust-boundary decision.
     The Slack client (fixed, operator-set host) is separate (ADR-EIP-005).
7. **Probe:** at boot the Matrix adapter validates its configuration shape only (there is none
   instance-wide); each connection is verified by `whoami` at connect and on token replacement.

## Consequences

- Positive: one place decides where Foundry may send; DNS rebinding and redirects are closed by
  construction, and C reuses it unchanged (OD-13).
- Positive: the `txnId` makes Matrix retries effectively-once.
- Negative: an extra state read per Matrix send; negligible at 1,000 events a day.
- Negative: the operator must list a private homeserver in `FOUNDRY_OUTBOUND_ALLOW` (documented in the
  Connect Matrix form's refusal: the K-16 sentence applies, see Upstream Issues).
- Negative: `m.mentions` is honoured only by servers and clients supporting spec v1.7; the word joiner
  covers older ones for `@room`; a member whose display name appears in a title may still be notified
  by an old client's legacy rule (documented).
- Enforcement:
  - check-arch `integrations-outbound-client-seam` fails on `reqwest::Client::new` or
    `Client::builder` anywhere in `crates/foundry-app/src/integrations/` outside `OutboundHttp` and the
    fixed-host Slack client;
  - inside `OutboundHttp`, the same rule requires both `redirect(Policy::none())` and `no_proxy()`;
  - proptest over generated addresses (every always-refused range, every embedded-IPv4 form,
    allow-list membership) in `foundry-integrations`;
  - gold test: `HTTPS_PROXY` set to a forwarder pointed at `169.254.169.254`; the request is still
    refused;
  - acceptance against a pinned Synapse container (SC-10), including an encrypted room, a kick and a
    logged-out token.
