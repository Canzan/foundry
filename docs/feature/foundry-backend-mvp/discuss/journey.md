# Foundry Backend MVP — UX Journey

Two journeys are JTBD-critical:

1. **Operator installing Foundry** — outcome #1 (under-an-hour setup). First impression decides whether evaluation continues.
2. **User filing an issue** — outcome #4 (Linear-feel speed). Most-frequent action; if this isn't fast, the product fails.

Each journey below is annotated with:

- A step-by-step flow.
- Emotional arc (start / middle / end).
- TUI/UI mockup per step.
- Shared artifacts that thread through the journey.
- Failure modes.
- The Gherkin scenarios from `stories.md` that anchor each step.

---

## Journey 1 — Operator Installing Foundry

**Persona**: Devansh Iyer, SRE at 12-person Series-A startup. 60 minutes free before lunch. Has Docker, no Rust, no patience.

**Trigger**: Linear contract renewal in Q3 makes him evaluate self-host alternatives.

**Goal**: Decide *today* whether Foundry is worth a team demo.

**Success criterion**: Foundry running at `localhost:3000`, admin account claimed, at least one teammate invited — all in under 30 minutes of his hour.

### Emotional Arc

| Phase | Emotion | Confidence |
|-------|---------|-----------|
| Start | Skeptical / busy / "this had better work" | 30% |
| First page-load | Curious / hopeful | 55% |
| Admin claim | Engaged / "this is actually fast" | 75% |
| Invite link copied | Confident / "I'll demo this Friday" | 90% |
| Decision point | Decided to proceed with evaluation | 95% |

### Step-by-Step Flow

```
[Trigger: Linear renewal Q3]
        |
        v
[Step 1: git clone]          Feels: still skeptical
        |                    Output: usual clone progress
        v
[Step 2: cp .env.example .env]   Feels: ".env looks short — good"
        |                    Shared artifact: FOUNDRY_PORT (default 3000)
        v
[Step 3: docker compose up -d]   Feels: anxious during 2-min pull
        |                    Shared artifact: $BOOTSTRAP_URL
        v
[Step 4: docker compose logs]    Feels: relief — found the URL
        |                    Shared artifact: $BOOTSTRAP_URL displayed
        v
[Step 5: Open URL in browser]   Feels: curious
        |                    Sees: Claim Admin form
        v
[Step 6: Claim admin + name WS] Feels: "this is fast"
        |                    Shared artifact: $WORKSPACE_NAME, $USER_EMAIL
        v
[Step 7: Workspace dashboard]   Feels: surprised at the polish
        |                    Sees: empty board, "Invite teammates" CTA
        v
[Step 8: Copy invite link]      Feels: confident
        |                    Shared artifact: $INVITE_LINK
        v
[Decision: Demo it Friday?]     Yes (90% of completed-installs)
```

### Step Mockups

#### Step 3 — `docker compose up -d` output

```
$ docker compose up -d
[+] Pulling 2/2
 ✔ foundry  Pulled                              42.1s
 ✔ postgres Pulled                              28.4s
[+] Running 3/3
 ✔ Network foundry_default      Created         0.1s
 ✔ Container foundry-postgres-1 Started         1.4s
 ✔ Container foundry-foundry-1  Started         2.1s
```

#### Step 4 — Logs containing the bootstrap URL

```
$ docker compose logs foundry
foundry-1  | 2026-05-23T14:02:11Z INFO  foundry::startup  Starting Foundry v0.1.0
foundry-1  | 2026-05-23T14:02:11Z INFO  foundry::startup  Running migrations…
foundry-1  | 2026-05-23T14:02:11Z INFO  foundry::startup  Migrations complete (1 applied)
foundry-1  | 2026-05-23T14:02:11Z INFO  foundry::startup  HTTP listening on 0.0.0.0:3000
foundry-1  | [BOOTSTRAP] Visit http://localhost:3000/bootstrap?token=8f3a4b7c... (valid 30 min)
foundry-1  | 2026-05-23T14:02:11Z INFO  foundry::http     Ready to serve requests
```

#### Step 5 — Landing page (pre-claim)

```
+--------------------------------------------------------------+
|                       FOUNDRY                                |
|                                                              |
|         A self-hosted tracker your team owns.                |
|                                                              |
|   Foundry is running. To claim the admin account, visit      |
|   the bootstrap URL from your container logs.                |
|                                                              |
|     docker compose logs foundry | grep BOOTSTRAP             |
|                                                              |
+--------------------------------------------------------------+
```

#### Step 6 — Claim admin form

```
+--------------------------------------------------------------+
|             Claim your Foundry admin account                 |
|                                                              |
|   Email           [____________________________]             |
|   Password        [____________________________]             |
|   Display name    [____________________________]             |
|   Workspace name  [____________________________]             |
|                                                              |
|                                  [  Create  ]                |
|                                                              |
+--------------------------------------------------------------+
```

#### Step 7 — Workspace dashboard (post-claim)

```
+---------------------------------------------------+
| Acme Eng                              [Devansh ▾] |
+--------------------+------------------------------+
| Teams              |                              |
|   ✓ General        |   Welcome to Foundry         |
|                    |                              |
| Projects           |   ┌───────────────────────┐  |
|   ✓ Sandbox        |   │ Invite teammates →    │  |
|                    |   └───────────────────────┘  |
|                    |                              |
|                    |   ┌───────────────────────┐  |
|                    |   │ Create a project →    │  |
|                    |   └───────────────────────┘  |
+--------------------+------------------------------+
```

#### Step 8 — Invite link modal

```
+--------------------------------------------------------------+
|                   Invite teammates to Acme Eng               |
|                                                              |
|   Share this link (valid 7 days):                            |
|     https://foundry.acme.com/join?token=q9zP...              |
|                                                              |
|                              [ Copy link ]  [ Email invite ] |
|                                                              |
+--------------------------------------------------------------+
```

### Shared Artifacts (Journey 1)

| Artifact | Source | Consumers | Risk |
|----------|--------|-----------|------|
| `$BOOTSTRAP_URL` | `bootstrap_tokens` table on first migration | Container logs (Step 4), browser URL (Step 5) | HIGH — wrong URL means no admin claim |
| `$FOUNDRY_PORT` | `.env` (`FOUNDRY_PORT`, default 3000) | docker-compose `ports`, $BOOTSTRAP_URL, README | HIGH — port mismatch silently breaks install |
| `$WORKSPACE_NAME` | User input on claim form (Step 6) | All pages, page titles | LOW |
| `$INVITE_LINK` | Generated per invite | Clipboard, email body, recipient browser | HIGH — token expiry must match advertised TTL |
| `$SESSION_SECRET` | `.env` (`SESSION_SECRET`) | Cookie signing, bootstrap-token HMAC, invite-token HMAC | CRITICAL — rotation invalidates outstanding tokens |

### Failure Modes (Journey 1)

| Step | Failure | UX response |
|------|---------|------------|
| Step 3 | Image pull blocked by proxy | Error from Docker, not Foundry; FAQ entry covers it |
| Step 3 | Port 3000 in use | Foundry exits on bind error; logs say "Address in use — set FOUNDRY_PORT in .env" |
| Step 4 | Bootstrap URL truncated by terminal width | URL printed on its own line, no leading whitespace |
| Step 5 | User clicks expired URL | Page shows "This link expired. Run `foundry admin reset-bootstrap` to mint a new one." |
| Step 6 | Workspace name has bad chars | Inline validation: "Letters, numbers, spaces only" |
| Step 7 | DB hiccup on first save | Foundry retries once; on failure shows "Something went wrong. Try again." with request_id |

### UAT Anchors

Step → primary Gherkin scenarios from stories.md:

- Step 1-3 → `Fresh-machine install completes in under five minutes` (US-01)
- Step 4 → `Bootstrap URL is discoverable from logs` (US-01)
- Step 5-6 → `First-run admin claim creates workspace and admin user` (US-05)
- Step 7 → (same)
- Step 8 → `Admin generates a shareable invite link` (US-05)

### What "Hour to Demo" Looks Like (Devansh's Timeline)

| Time | Action | Foundry's role |
|------|--------|----------------|
| 14:01 | `git clone` | — |
| 14:02 | edit .env (just kept defaults) | — |
| 14:03 | `docker compose up -d` | Pull images, run migrations, print bootstrap URL |
| 14:05 | `docker compose logs foundry` | Show $BOOTSTRAP_URL |
| 14:06 | Visit bootstrap URL | Show claim form |
| 14:07 | Submit claim | Create admin + workspace, default team, default project |
| 14:08 | Dashboard | Show invite CTA |
| 14:09 | Copy invite link | Generate signed token, copy to clipboard |
| 14:10 | Paste in Slack #engineering | — |
| 14:15-14:30 | 6 teammates click invite link | Each: claim form → workspace member |
| 14:35 | Devansh has 7 active users | — |
| **34 minutes total — JTBD outcome #1 hit.** |

---

## Journey 2 — User Filing an Issue

**Persona**: Mei Chen, member of Acme Eng, Backend team. Mid-flow on a code review when she notices a bug.

**Trigger**: "I just noticed Safari refresh tokens are broken — file it before I forget."

**Goal**: Capture the issue with enough context that future-Mei can pick it up.

**Success criterion**: From thought to filed-and-visible-in-board in under 10 seconds for a title-only issue, under 60 seconds for a fully-detailed one.

### Emotional Arc

| Phase | Emotion | Confidence |
|-------|---------|-----------|
| Start | Mid-task / mildly annoyed at distraction | 50% |
| Modal opens | "Same as Linear" / muscle memory engages | 80% |
| Typing | Focused / flow state | 85% |
| Cmd-Enter | Brief satisfaction (toast confirms) | 95% |
| Returns to original task | Relief — context preserved | 90% |

### Step-by-Step Flow

```
[Trigger: Mei sees the Safari bug]
        |
        v
[Step 1: Press 'c' from anywhere]    Feels: muscle memory
        |                            Latency: <100ms to modal
        v
[Step 2: Modal opens, title focused] Feels: trust
        |                            Shared artifact: $PROJECT_CONTEXT (auto-filled)
        v
[Step 3: Type title]                 Feels: flow
        |                            Hot path: 0 round-trips
        v
[Step 4: Cmd-Enter]                  Feels: satisfaction
        |                            Shared artifact: $ISSUE_KEY (AUTH-7)
        v
[Step 5: Modal closes, toast shown] Feels: confirmed
        |                            Toast: "Created AUTH-7"
        v
[Step 6: Issue appears in board]    Feels: real-time
        |                            via SSE to all viewers
        v
[Done — Mei back to code review]
```

### Step Mockups

#### Step 2 — Issue-create modal (opened with `c`)

```
+-----------------------------------------------------------+
|  New issue in Auth v2                                [Esc]|
+-----------------------------------------------------------+
|                                                           |
|  ┌─────────────────────────────────────────────────────┐  |
|  │ Refresh token rotation broken on Safari_            │  |
|  └─────────────────────────────────────────────────────┘  |
|                                                           |
|  Add description (markdown supported)                     |
|  ┌─────────────────────────────────────────────────────┐  |
|  │                                                     │  |
|  │                                                     │  |
|  └─────────────────────────────────────────────────────┘  |
|                                                           |
|  Assignee  [None ▾]   Labels [+]    Priority [Med ▾]      |
|  State     [Backlog ▾]                                    |
|                                                           |
|                            [Cancel]  [Create  Cmd+Enter] |
+-----------------------------------------------------------+
```

#### Step 5 — Toast after submit

```
                                          ┌──────────────────┐
                                          │ ✓ Created AUTH-7 │
                                          └──────────────────┘
```

#### Step 6 — Issue appears in Backlog column (live)

```
+-- Auth v2 ------------------------------------------------+
|                                                           |
| Backlog              Todo              In-Progress        |
| ┌───────────────┐   ┌───────────────┐ ┌───────────────┐  |
| │ AUTH-7        │   │ AUTH-3        │ │ AUTH-2        │  |
| │ Refresh token │   │ Verify magic  │ │ OIDC discovery│  |
| │ broken on Saf │   │ link expiry   │ │ stub          │  |
| └───────────────┘   └───────────────┘ └───────────────┘  |
| ┌───────────────┐                                        |
| │ AUTH-6        │                                        |
| └───────────────┘                                        |
+-----------------------------------------------------------+
```

### Shared Artifacts (Journey 2)

| Artifact | Source | Consumers | Risk |
|----------|--------|-----------|------|
| `$PROJECT_CONTEXT` | Current URL path (`/team/backend/project/auth-v2`) | Modal pre-fill, POST body | MEDIUM — wrong context creates issue in wrong project |
| `$ISSUE_KEY` | Postgres sequence `projects.next_issue_number` | Toast, board card, URL | HIGH — must be sequential per project, no gaps |
| `$USER_ID` | Session cookie | Author field, audit log, SSE filtering | HIGH — auth check at endpoint |
| `$REQUEST_ID` | Server middleware, UUIDv7 | Logs, error responses, X-Request-Id header | LOW |

### Failure Modes (Journey 2)

| Step | Failure | UX response |
|------|---------|------------|
| Step 1 | `c` pressed inside a text input | Suppressed — the `c` goes to the input |
| Step 2 | Modal slow to open (>200 ms) | Visible "skeleton" while loading; degraded but functional |
| Step 4 | Empty title submit | Inline "Title is required"; modal stays open |
| Step 4 | Server returns 5xx | Toast: "Failed to create — try again" with request_id; modal stays open with content preserved |
| Step 4 | Network drops mid-submit | Browser retry; same idempotency caveat (UUID-keyed POST) |
| Step 6 | SSE not connected | Card still appears because the writing client gets the POST response; other viewers' updates rely on SSE per US-09 |

### UAT Anchors

- Step 1-2 → `c opens the issue-create modal globally` (US-12)
- Step 3-4 → `Quick issue creation with just a title` (US-08)
- Step 4-5 → `Issue keys are sequential per project` (US-08)
- Step 6 → `New issue appears in real time` (US-09)

### Hot-Path Latency Budget

| Step | Budget | Mechanism |
|------|--------|-----------|
| `c` → modal visible | 200 ms | htmx `hx-get` returns pre-rendered modal HTML; client caches after first |
| Type title | — | client-only |
| Cmd-Enter → toast | 300 ms | POST `/issues` returns minimal JSON; toast triggered on response |
| POST → board re-render (own client) | 500 ms | htmx swap of board card |
| POST → board re-render (other clients via SSE) | 1000 ms median, 2000 ms P95 (NFR-PERF-03) |
| **End-to-end "thought → on board" (own client)** | **≤2 seconds** for title-only |
| **End-to-end "thought → on others' boards"** | **≤3 seconds** for title-only |

---

## Cross-Journey Vocabulary

The CLI / URL / UI vocabulary must be consistent between the operator and user journeys:

| Concept | Operator CLI | URL Path | UI Label |
|---------|-------------|----------|----------|
| Workspace | `foundry admin workspace` (deferred) | `/` | Workspace name in header |
| Team | `foundry admin team` (deferred) | `/team/:slug` | "Teams" sidebar section |
| Project | `foundry admin project` (deferred) | `/team/:t/project/:p` | "Projects" sidebar section |
| Issue | (none; UI-only in MVP) | `/issue/:key` | Issue card |
| Bootstrap | `[BOOTSTRAP]` log marker | `/bootstrap?token=...` | "Claim admin account" |
| Invite | (none) | `/join?token=...` | "Invite teammates" |
| Health | (LB probe) | `/healthz`, `/readyz` | — |
| Metrics | (Prom scrape) | `:9090/metrics` | — |

Inconsistencies here would silently break operator-to-user handoff (e.g., admin invites a user; URL the user receives doesn't match the URL the admin saw).
