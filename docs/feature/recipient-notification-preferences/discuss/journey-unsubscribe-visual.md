# Journey (visual): Recipient Unsubscribe — one click from the email, security mail always through

> Feature: `recipient-notification-preferences` (v1 = recipient unsubscribe) | Personas: **Sam Okafor** (the
> recipient — an invitee with no account), **Maria Santos** (an account-holding member), **Ops/Compliance
> Olivia** (secondary), and **Malicious Mallory** (the adversary the token must defeat).
> Goal: let a recipient stop a workspace's **suppressible** notifications (`workspace_invite`, `member_invite`)
> with **one click from the email**, keyed per `(email_lower, workspace_id)`, while the **security-critical**
> events (`password_reset`, `password_changed`, `member_removed`) are **never** suppressed; and let an account
> holder review per-workspace status and **resubscribe**.
> Scope (v1 = US-01..US-04): the token + public route + `0014` table + suppression filter + the security
> invariant + non-enumerable/prefetch-safety, proven on **both** invite events. US-05..US-07 (signed-in status,
> resubscribe, operator visibility) are fast-follow. **Per-category / digests / quiet-hours are OUT OF SCOPE.**

## Why this is a thin extension, not greenfield

The predecessor (`notification-delivery-providers`) left exactly the seams this needs, and the invite flow
already solved the hard security shapes:

- `Notifier::notify` is an **INFALLIBLE** concurrent fan-out (`notify.rs:237`, loop `:244-280`, `deliver()` at
  `:252`) — the single point where a suppressible delivery can be skipped before it reaches a provider.
- The `NotificationEvent` catalog is a **closed enum** (`notify.rs:46-77`) we simply partition into
  **suppressible** {`workspace_invite`, `member_invite`} and **mandatory** {`password_reset`,
  `password_changed`, `member_removed`}.
- `InviteToken` (`foundry-auth/src/lib.rs:354-390`) already signs a workspace-resource action with HMAC over
  `"{id}|{unix_ts}"` and a **constant-time** verify — the exact model for an `UnsubscribeToken` binding
  `"{email_lower}|{workspace_id}"`, which lets a **logged-out** recipient act from an email link.
- The invite refusal (`invite_refusal_page`, `invites_accept.rs:332-339`) already returns a **fixed 200 OK,
  byte-identical** page for every invalid reason — the non-enumerable response a tampered unsubscribe token
  reuses verbatim in shape.
- CSRF (`csrf.rs:137`) and session (`session.rs`) already protect the app's POSTs and identify signed-in users.

Only four things are genuinely new: the `UnsubscribeToken`, the `0014_notification_unsubscribes` table (**the
one migration this feature adds** — the predecessor added zero), the suppression filter, and the suppression
count. Each is isolated behind an ODD so requirements stay solution-neutral.

## The personas, concretely

**Sam Okafor** (`sam.okafor@acme.example`) was invited to **Northwind** and keeps getting invite reminders. He
has **no Foundry account** — he's just an email address on an invite. His only lever today is a blunt inbox
filter that would also bury a password-reset. He wants to stop *these* emails, from the email itself.

**Maria Santos** (`maria.santos@acme.example`) is a member of **Northwind**, **Contoso**, and **Initech**. She
thinks she unsubscribed from one of them via an email link months ago but can't remember which. She wants a
signed-in place to check and to turn a workspace back on.

**Ops/Compliance Olivia** (`olivia.okonkwo@acme.example`) needs unsubscribe **honored** and the **volume
visible** for list-hygiene/compliance — but must **not** be handed a list of who opted out. And she needs a
**guarantee** that a `password_reset` or `member_removed` is never withheld.

**Malicious Mallory** wants to (a) probe whether `sam.okafor@acme.example` is a live Foundry recipient, and
(b) get a mail scanner's link-prefetch to silently unsubscribe people. The token design must defeat both.

## Emotional arc

Two arcs — **Problem Relief → Confidence** (Sam) and **Confidence Building** (Maria).

### Recipient (Sam) — Problem Relief → Confidence
```
TRAPPED                 WARY                    RELIEF                  REASSURED
"these Northwind   -->  "is this link safe?  -->  "one click, the   -->  "and it still delivers my
 invites won't          will I lose             invites stopped"         security mail. Clean."
 stop and I have        important mail?"
 no account"            reads reassurance
 annoyed / trapped      cautious                 relieved                reassured + in control
```

### Account holder (Maria) — Confidence Building
```
UNCERTAIN               INFORMED                IN CONTROL
"did I mute        -->  "Northwind is muted, -->  "one click, Northwind's
 something? which        the others aren't"         back on. I decide."
 workspace?"            clear per-ws status        resubscribed
 uncertain              informed                   confident / in control
```

Sam's peak tension is the **click** ("will this cost me important mail, or confirm my address to a stalker?").
Collapse it two ways: the confirm page **explicitly promises** security mail still arrives (and that promise is
literally guaranteed by NFR-3), and the link is **non-enumerable + prefetch-safe** so it's safe to click and
safe to sit in an inbox. Maria's tension is **uncertainty** ("what's my state?"); collapse it with a clear
per-workspace status list and a one-click resubscribe. The SAD paths stay calm: a tampered link gets the same
gentle "no longer valid" page as any invalid one (no scary security wording, no leak); a prefetch does nothing.

---

## Capability 1 — Unsubscribe from the email (logged-out): emit → click → confirm → suppress

```
[Step U1: EMIT]            [Step U2: CLICK (GET)]      [Step U3: CONFIRM (POST)]     [Step U4: SUPPRESS]
suppressible email    -->  recipient opens the    -->  recipient confirms;      -->  notifier drops future
carries the                link logged-out;             token verified, opt-out       suppressible events to
unsubscribe link           NON-destructive              recorded (0014 table)          the pair; SECURITY mail
  Feels: annoyed             confirm page               Feels: relieved +              always through
  Artifacts:                 Feels: wary->reassured      reassured                    Feels: quiet inbox
   ${tok},${public_url}      Artifacts:                  Artifacts:                    Artifacts:
                             ${unsubscribe_target}       ${unsubscribe_row}            ${suppression_decision}
```

### Step U1 — A suppressible notification carries the unsubscribe link

```
+-- Email: "You're invited to Northwind" --------------------------+
|  ...invitation body + accept link...                             |
|  ----------------------------------------------------------------|
|  Don't want these? Unsubscribe from Northwind invitations:       |
|    ${public_url}/unsubscribe?token=${tok}                        |
+------------------------------------------------------------------+
```

Only the two **suppressible** events carry the link (BR-1). `${tok}` is an `UnsubscribeToken` binding **this
recipient email + this workspace_id** — both already in scope at the emit sites (`bootstrap.rs:266`,
`member_invites.rs:204`) — signed with `SESSION_SECRET` exactly like `InviteToken`. Mandatory events
(`password_reset`, `password_changed`, `member_removed`) carry **no** link. The link host `${public_url}` is
the same `FOUNDRY_PUBLIC_URL` the invite accept-link already uses (`main.rs:122`).

### Step U2 — Recipient opens the link, logged-out (non-destructive GET)

```
+-- GET /unsubscribe?token=${tok}   (public, no session) ----------+
|  Stop invitation emails from "Northwind"?                        |
|                                                                  |
|  You'll still receive security-critical notifications            |
|  (password resets, account changes, removals).                   |
|                                                                  |
|             [ Confirm unsubscribe ]   (POST)                     |
+------------------------------------------------------------------+
   GET renders the page ONLY — no state change (prefetch-safe, NFR-2)
   tampered/unknown token -> uniform "no longer valid" refusal (NFR-1)
```

The **GET is non-destructive** (NFR-2): a mail-client or scanner prefetch does **not** unsubscribe anyone. The
page **reassures** up front that security mail still arrives — collapsing Sam's peak anxiety before he acts. A
tampered/unknown token short-circuits here to the uniform non-enumerable refusal (see Sad paths). The confirm
form is served with a CSRF cookie (`csrf.rs:54`) so the POST can be validated.

### Step U3 — Recipient confirms; token verified, opt-out recorded (POST)

```
+-- POST /unsubscribe (confirm, CSRF-checked) ---------------------+
|  verify ${tok} (constant-time HMAC)  ->  (email_lower, ws_id)    |
|     invalid  ->  uniform refusal (records NOTHING)               |
|  insert_unsubscribe(email_lower, workspace_id)     [idempotent]  |
|  ->  "Done. You won't receive further invitations from           |
|       Northwind. You'll still get security-critical mail."        |
+------------------------------------------------------------------+
```

The POST is **CSRF-protected** (NFR-5) and **re-verifies the token server-side** — it never trusts a
client-supplied email (BR-5). It writes `${unsubscribe_row}` to the new
`notification_unsubscribes(email_lower, workspace_id, unsubscribed_at)` table (`0014`), idempotently (BR-8).
That row is the **single source** consumed by suppression (U4), the status page (S1), and resubscribe (S2).

### Step U4 — Notifier suppresses future suppressible events; security mail always through

```
+-- Notifier.notify  (suppressible check BEFORE deliver) ----------+
|  if event in {workspace_invite, member_invite}:                  |
|     if unsubscribed(email_lower, workspace_id):                  |
|         SKIP deliver()  ; count outcome=suppressed               |
|     else deliver()  (exactly as today)                           |
|  else  (password_reset / password_changed / member_removed):     |
|     ALWAYS deliver()   <-- MANDATORY, never suppressed (BR-3)     |
+------------------------------------------------------------------+
   workspace_invite -> [unsubscribed] -> suppressed (counted)
   password_reset   -> [unsubscribed] -> DELIVERED  (mandatory)
```

This is the crux (NFR-3). The suppressible set is a **bounded allow-list**, so a mandatory event — or any
future event — is **delivered by default** and can never be accidentally suppressed. Mandatory > unsubscribe,
always. The filter is **additive**: with an empty table, the notifier behaves exactly as it does today (NFR-7).
Whether the check lives in `notify()` (which needs workspace context added to `Notification` — it carries none
today) or at the emit sites is **ODD-3**.

---

## Capability 2 — Manage subscription when signed in: status → resubscribe

```
[Step S1: STATUS (GET)]                    [Step S2: RESUBSCRIBE (POST, CSRF)]
member opens /account/notifications   -->  member clicks Resubscribe for a muted workspace
  sees per-workspace Subscribed/Muted        opt-out row cleared; workspace -> Subscribed
  Feels: uncertain -> informed               Feels: in control
  Artifacts: ${member_identity},             Artifacts: ${unsubscribe_row} (cleared)
             ${workspace_status_list}
```

### Step S1 — Signed-in member views per-workspace status

```
+-- GET /account/notifications   (Maria, signed in) ---------------+
|  Notifications — where you're subscribed                         |
|    Northwind ............... Muted       [ Resubscribe ]         |
|    Contoso ................. Subscribed                          |
|    Initech ................. Subscribed                          |
|  (only YOUR workspaces, only YOUR status — NFR-6)                |
+------------------------------------------------------------------+
```

Identity comes from the **session** (`SessionUser`, `session.rs`) — never a client-supplied email (NFR-6). The
page lists only the workspaces Maria belongs to (membership lookups, `foundry-store/src/lib.rs:1048,1955`) and
derives each status from the single source `${unsubscribe_row}`. This is a **new user-facing surface**, so WCAG
2.1 AA applies (NFR-8) — labelled controls, status conveyed as text, keyboard-operable. It registers beside
`/account/password` (`lib.rs:415-418`).

### Step S2 — Signed-in member resubscribes a muted workspace (CSRF POST)

```
+-- POST /account/notifications/resubscribe   (CSRF-checked) ------+
|  scope = the session member's own (email_lower, workspace_id)    |
|  delete_unsubscribe(email_lower, workspace_id)      [idempotent] |
|  ->  Northwind ............. Subscribed                          |
+------------------------------------------------------------------+
```

CSRF-protected (NFR-5), scoped to the session member's own pairs (NFR-6), idempotent (BR-8). Clearing the row
makes U4 deliver that pair's suppressible events again — the same single source drives both suppression and
status, so they can't diverge. (Resubscribe for **account-less** recipients, who can't sign in, is **ODD-6** —
a token-based resubscribe / an undo on the confirmation page is the candidate.)

---

## Capability 3 — Operator visibility (US-07): opt-out volume, no PII

```
+-- GET /metrics   (existing sidecar) -----------------------------+
|  foundry_notification_deliveries_total{event="workspace_invite", |
|      outcome="suppressed"}  34                                   |
|  foundry_notification_deliveries_total{event="member_invite",    |
|      outcome="suppressed"}  8                                    |
|  (no recipient email, no token, anywhere — NFR-4)                |
+------------------------------------------------------------------+
```

A suppressed delivery is counted on the shipped `/metrics` seam (`notify.rs:39,291-297`) — a `suppressed`
outcome or a sibling counter (**ODD-5**), bounded-label, **PII-free**. Olivia reads opt-out volume by event; the
suppressed count for every **mandatory** event is always **0** (US-02), which she can verify. A cardinality/PII
guard fails closed on an unbounded or recipient label.

---

## Sad / error paths — first-class

### Public unsubscribe sad paths (recipient-facing, security-critical)

```
+-- tampered / unknown / prefetched link --------------------------+
|  GET/POST with an altered, made-up, or expired token:            |
|    -> uniform "This link is no longer valid" page (fixed 200,    |
|       byte-identical body — same as an invite refusal)           |
|    -> records NOTHING; reveals NOTHING about existence           |
|  bare GET prefetch of a VALID link:                              |
|    -> renders the confirm page only; records NOTHING             |
+------------------------------------------------------------------+
```

| # | Sad path | Trigger | What the recipient/attacker sees | Handling |
|---|----------|---------|----------------------------------|----------|
| U-E1 | **Tampered / unknown / expired token** | Mallory flips a byte / invents a token | uniform "no longer valid" refusal | fixed status + byte-identical body; records nothing (NFR-1, FR-5, BR-4) |
| U-E2 | **Existence probe** | real address vs fake, both bad tokens | **identical** response | no differential; existence not leaked (NFR-1, R3) |
| U-E3 | **Prefetch (scanner GET)** | mail security scanner fetches the raw link | confirm page rendered, nothing recorded | GET non-destructive; only confirm POST mutates (NFR-2, R2) |
| U-E4 | **Missing/invalid CSRF on confirm** | forged cross-site POST | `403`, no change | shipped CSRF middleware (NFR-5) |
| U-E5 | **Already unsubscribed** | Sam confirms twice | "already unsubscribed" success | idempotent no-op, no duplicate row (BR-8) |

### Suppression sad paths (the security invariant — the crux)

```
+-- mandatory event for an unsubscribed recipient -----------------+
|  password_reset / password_changed / member_removed             |
|    -> ALWAYS delivered (mandatory > unsubscribe)                 |
|    -> NEVER counted suppressed                                   |
+------------------------------------------------------------------+
```

| # | Sad path | Trigger | What happens | Handling |
|---|----------|---------|--------------|----------|
| U4-E1 | **Mandatory suppressed** | a bug suppresses a security event | must be impossible | bounded allow-list + never-suppress `@property` reds on regression (NFR-3, BR-3, R1) |
| U4-E2 | **Suppression lookup errors** in the INFALLIBLE `notify()` | store hiccup mid-delivery | must not worsen today's contract | ODD-3 defines the fail stance (R5) |
| U4-E3 | **Wrong-workspace suppression** | email in two workspaces | only the muted pair is suppressed | per-`(email,workspace)` key (FR-9, R6) |

### Signed-in sad paths (least-privilege)

| # | Sad path | Trigger | What happens | Handling |
|---|----------|---------|--------------|----------|
| S-E1 | **Steer to another recipient** | crafted email param | only the session member's own scope returned | identity from session (NFR-6, BR-6) |
| S-E2 | **Resubscribe outside scope** | POST naming a foreign pair | refused | session-scoped mutation (NFR-6) |
| S-E3 | **PII in the suppression metric** | a label carries the recipient | caught before ship | no-PII `@property` (NFR-4, R4) |

> All sad paths share the design intent: **a tampered/prefetched link is inert and mute; an unsubscribed
> recipient never loses a security email; and no signed-in member can see or touch anyone else's state.**

---

## Integration checkpoints

1. **Token round-trip**: the `${tok}` embedded at U1 verifies at U2/U3 (constant-time HMAC, `foundry-auth`);
   a tampered token yields the uniform refusal and records nothing. Single source: `SESSION_SECRET` signing.
2. **Confirm → state**: only a **valid-token, CSRF-checked confirm POST** writes `${unsubscribe_row}` (U3);
   a GET (U2) never does (prefetch-safe). Single source of subscription state: the `0014` table.
3. **State → suppression**: U4 suppresses a **suppressible** event iff `${unsubscribe_row}` exists for
   `(email_lower, workspace_id)`; **mandatory events skip the check** (never suppressed). Additive vs today.
4. **State → status → resubscribe**: S1 renders status from the same `${unsubscribe_row}`; S2 clears it,
   scoped to the **session** identity; the same source drives suppression, status, and resubscribe — no
   divergence.
5. **Least-privilege**: S1/S2 use `${member_identity}` from the session only; a member can never view or mutate
   another recipient's state (NFR-6). A litmus reds if a request can be steered to a foreign pair.
6. **PII-free observability**: the suppression count (U4→S3) carries `event` (+ at most `workspace`), never a
   recipient email or token (NFR-4). A guard fails closed on a PII/unbounded label.
7. **Backwards-compat**: with the `0014` table empty, every existing delivery flow (invite + reset + removal)
   behaves exactly as today; new behaviour appears only once a row exists (NFR-7).

## Web / config parity note

Unlike the predecessor (which added **no** UI, NFR-7 N/A there), this feature adds **two real HTML surfaces** —
the public confirm page and the signed-in settings page — so **accessibility is in scope** (NFR-8). The public
unsubscribe path is deliberately **logged-out** (token-only, no session), matching the shipped `/invites/accept`
public cluster (`lib.rs:371-374`); the signed-in path sits under session + CSRF beside `/account/password`
(`lib.rs:415-418`). Recipients dogfood by clicking a real invite email's unsubscribe link against a local
mail catcher and watching the next invite get suppressed on `/metrics`.
</content>
