# DESIGN options: instance-integration-setup (phase 1, for the user's choice)

Scope: application. Mode: propose. Paradigm: OOP (Rust, `nw-software-crafter`).
Nothing here is decided until the user chooses. ADRs, brief.md and
wave-decisions.md follow in phase 2.

## Inputs read

- ✓ `docs/product/architecture/brief.md` ("Project events leave the board",
  Secrets, Slack, For Acceptance Designer / Software Crafter, Wayfinding, Crate graph)
- ✓ ADR-EIP-004, ADR-EIP-005, ADR-IW-001, ADR-IW-002
- ✓ `docs/product/journeys/journey-external-integration-plugins.yaml` (step 0)
- ✓ `discuss/` (wave-decisions D0a–D17, user-stories, outcome-kpis; story-map,
  journey and dor-validation skimmed)
- ✓ AGENTS.md "Deployment independence"
- ✓ `docs/feature/integrations-wayfinding/design/wave-decisions.md`
- ✓ Code: `integrations/{config,mod}.rs`, `worker.rs` (poll loop),
  `foundry-auth/src/secret_box.rs`, `foundry-store/src/integrations/keys.rs`,
  migration 0030, `main.rs` (`probe_secrets`, integrations wiring),
  `admin_cli.rs` (`rotate-secrets`), `foundry-realtime` (LISTEN loop),
  `multi_replica_harness.rs`, the acceptance `restart_in_process_with_key`,
  `deploy/k8s` (3 replicas, `readOnlyRootFilesystem`, per-pod `emptyDir`),
  Dockerfile (distroless, `nonroot`, `WORKDIR /app`), docker-compose.yml (no
  foundry volume)
- ⊘ discover/, diverge/ (none ran)

## What the code gives us (reuse first)

| Existing | Reuse |
|---|---|
| `SecretBox` (AES-256-GCM via `ring`, key ids, row-bound AAD, previous keys) | Wraps the secrets key under the instance key with its own slot; no new crypto, no new crate. |
| `IntegrationsHandle` (`Arc<RwLock<Arc<Integrations>>>`, `install`) | The live-reload swap point. Every handler and the worker already call `current()` per request or per poll, so a swap is seen on the next request/round and in-flight work keeps a consistent snapshot. |
| `integration_key_checks` canaries + boot `probe_secrets` | Extended to the secrets key; the same check runs on every reload. |
| `Store::reseal_credentials` (one transaction, closure re-seals, rollback on refusal) | Generalised to every sealed slot (connections + saved Slack app) for Move, Replace and `doctor rotate-secrets`. |
| `multi_replica_harness` (N in-process apps, one schema) | KPI-IIS-1's "every replica within 60 s" without subprocesses. |
| Dispatcher advisory lock (ADR-EIP-002) | Precedent for a named advisory lock. |

Gap found: the boot probe is written twice (`main.rs::probe_secrets` and the
acceptance `restart_in_process_with_key`). Every option below adds a third
caller (reload), so all of them assume one shared `integrations` boot/reload
function used by `main.rs`, the harness and the reload task.

---

## Decision 1: key hierarchy, settings and the key file

Fixed by D1/D5/D6: secrets key in the DB wrapped by an instance key that never
lives there; operator setting wins over a Foundry-made file; legacy direct seals
keep opening; malformed operator key refuses start.

Common to every option: the wrapped secrets key is a `SealedSecret` sealed by a
`SecretBox` holding the instance key(s), slot
`integration_secrets_keys|{secrets_key_id}|wrapped_key`. The credential
`SecretBox` is `current = secrets key`, `previous = [instance key, previous
instance keys]`, so legacy direct seals (key id = an instance key id) open with
no action (US-IIS-06 AC 1). A credential's key id says which tier sealed it, so
I-2e ("sealed directly") is a key-id comparison, not a stored flag.

### 1a. Setting name

| Option | Summary | Trade-offs |
|---|---|---|
| A | Keep `FOUNDRY_SECRETS_KEY` / `_PREVIOUS` as the instance key's names | Smallest change, no doc churn in deploy secrets. But the UI says "secrets key" for a different key, and I-4/I-26 tell operators to set "the instance key": an operator will put the wrong value in the wrong place. |
| **B (recommended)** | New `FOUNDRY_INSTANCE_KEY` / `FOUNDRY_INSTANCE_KEY_PREVIOUS`; legacy `FOUNDRY_SECRETS_KEY` / `_PREVIOUS` read as aliases. Both names set with different values refuses start naming both settings (the partial-Slack-trio precedent). | Names match the UI's words. One alias table in `config.rs`. Gives the legacy-mode signal in 1e. Docs and `foundry-secret.example.yaml` change (already planned for DELIVER). |
| C | New name only; legacy name refused with a "rename it" message | Violates D5 (legacy must be read). Rejected. |

### 1b. Key file location and format

| Option | Summary | Trade-offs |
|---|---|---|
| **A (recommended)** | `FOUNDRY_DATA_DIR`, default `/var/lib/foundry`; file `instance.key`, mode 0600, one line of base64url (the operator-key format, so the same value can later be copied into `FOUNDRY_INSTANCE_KEY` with no conversion). Image pre-creates `/var/lib/foundry` owned by `nonroot` and declares it a volume; compose mounts a named volume there. | Matches the stories' examples. Distroless needs the directory created in the build stage (DEVOPS). A single binary run as an ordinary user lands in I-27 until `FOUNDRY_DATA_DIR` is set, which I-27 already says. |
| B | Default relative `./data` under the working directory | Writable for a single binary, but `/app` is root-owned in the image and a relative path depends on how the process was launched. |
| C | No default: the file is used only when `FOUNDRY_DATA_DIR` is set | Most explicit, but breaks D6's zero-config standalone deploy (KPI-IIS-6). Rejected. |

### 1c. Create-if-absent on a shared volume

| Option | Summary | Trade-offs |
|---|---|---|
| **A (recommended)** | Write a temp file in the same directory, fsync it, `link(2)` it to `instance.key` (fails `EEXIST` if another server won), fsync the directory, remove the temp, then read back whichever file exists. | POSIX, atomic, never exposes a partial file, works on NFS. Probe: read back, parse 32 bytes, compare with what was written when this server won. |
| B | `open(O_CREAT|O_EXCL)` then write | Atomic create, but a racing reader can see an empty or partial file and must retry. |
| C | `flock` a lock file, then create | Advisory locks are unreliable on network volumes. |

Refinement (all options): do **not** create a file when the database already
holds a secrets key wrapped by a Foundry-made file key. A new file could never
open it; the server shows I-26 ("missing or different") instead of minting a
useless key. This also makes per-replica volumes (the k8s example's `emptyDir`)
fail loudly as I-26 on every replica but the first, never silently. The k8s
example should set `FOUNDRY_INSTANCE_KEY` (DEVOPS/docs).

### 1d. Previous instance key and file → operator (I-30)

| Option | Summary | Trade-offs |
|---|---|---|
| **A (recommended)** | With an operator key set and a key file present, the file's key is loaded as a previous instance key automatically. At boot, a secrets key wrapped by the file's key is re-wrapped under the operator key with a compare-and-set on `wrapped_by_key_id` (one replica wins; it records the I-18 system line). The row keeps the file path it came from, so the page can show I-30. | Matches D6 refinement and AC 5. Cheap (one row). A replica still on the file reads the secrets key it already holds in memory (reload keeps a held key whose id is unchanged, see 3). |
| B | Re-wrap lazily on the first page view | Leaves the instance in a mixed state until someone visits; the system line's timing becomes arbitrary. |
| C | Require the operator to list the file's key in `FOUNDRY_INSTANCE_KEY_PREVIOUS` | Contradicts D6 ("nothing is lost" with no extra step). Rejected. |

### 1e. Legacy mode, and a requirement ambiguity to rule on

**Flag.** Today `FOUNDRY_SECRETS_KEY` alone makes Matrix ready. US-IIS-01
scenario 1 says "instance key set by the operator, no secrets key" shows I-3 and
I-9 (Matrix not ready). Applied to an existing instance that set the key but has
not connected anything yet, that is a regression; it would also break existing
external-integration-plugins acceptance (KPI-IIS-G3), and new-release replicas
would disagree with previous-release ones during the roll.

| Option | Summary | Trade-offs |
|---|---|---|
| **A (recommended, needs 1a-B)** | Mode follows the database first, the setting name second. A stored secrets key means two-tier mode. With none stored: the legacy name `FOUNDRY_SECRETS_KEY` means **direct mode**, exactly today's behaviour (Matrix ready, new credentials sealed directly), page shows I-2e and "Move to a secrets key" (with n = 0 it simply generates); the new name or a file means I-3 and Generate. | G3 stays green, the roll is safe, US-IIS-01 scenario 1 holds for new installs. I-2e's words assume saved credentials exist; an n = 0 variant may need a copy line (user to confirm). |
| B | Any instance key without a stored secrets key means I-3; Matrix off until Generate | Literal reading of US-IIS-01. Regresses existing instances and suites; needs a migration note. |
| C | Generate the secrets key automatically at first start when none is stored | D5 forbids automatic moves at first start (previous-release replicas). Rejected. |

"Move" (US-IIS-06) and "Generate" share one use-case: create-if-absent the
secrets key, then re-seal every value whose key id is an instance-key id, in one
transaction (decision 4).

---

## Decision 2: storing the UI-entered Slack app

| Option | Summary | Trade-offs |
|---|---|---|
| **A (recommended)** | Single-row table `instance_slack_app` (`id` fixed by a CHECK, `client_id` text, `sealed_client_secret`, `sealed_signing_secret` bytea, `saved_by`, `saved_at`). Slots `instance_slack_app|1|{column}`. | Typed, constraint-checked, one obvious place for re-seal and the boot probe to include. Removal (B) is a DELETE. |
| B | Generic `instance_settings` key/value (`name`, `value`, `sealed_value`) | Reusable for future instance settings, but untyped: "all three or none" and "client id plain, secrets sealed" become code conventions, not constraints. |
| C | Reuse `integration_connections` with a NULL workspace | `workspace_id NOT NULL`, one-per-kind and connection states do not fit an app. Rejected. |

Precedence (D9, all options): in effect = environment if all three `SLACK_*`
set; else the saved app if its secrets open; else none. The saved row is kept
while the environment wins (I-17). `SLACK_API_BASE_URL` / `SLACK_AUTHORIZE_URL`
apply to whichever app is in effect and stay env-only (D13). A saved app that
does not open is I-13 (offer `Unavailable`-like, never "not set up" silently).
The boot probe and every re-seal must include the app's sealed columns; the
store should expose "every sealed value with its slot" across both tables
rather than each caller knowing the tables.

---

## Decision 3: live effect on every replica within 60 s

The serving replica re-reads and `install`s right after its POST commits (no
wait). The options differ in how the other replicas learn.

| Option | Summary | Trade-offs |
|---|---|---|
| **A (recommended)** | A single-row `integration_config_version` bumped in the same transaction as every key or app change. Each replica's reload task reads it every `FOUNDRY_INTEGRATIONS_RELOAD_MS` (default 5 s) and, on change, rebuilds `Integrations` (boot-time env settings + DB state + secrets probe) and `install`s it. A failed read or rebuild keeps the last good integrations and logs a warning. | One trivial query per replica per 5 s. Cannot miss a change. Works through any connection pooler. Testable in-process (the harness shortens the interval; `multi_replica_harness` already runs N apps on one schema). Worst-case lag 5 s ≪ 60 s. |
| B | Postgres `LISTEN integration_config` + `pg_notify` in the change transaction (the `foundry-realtime` pattern) | Sub-second, but notifications are lost across a listener reconnect, so a poll is still needed for correctness; one more dedicated connection per replica; transaction-pooling caveat. Effectively A plus more code. |
| C | Reload-on-read: each request re-reads DB state when the cached copy is older than a TTL | Puts a query (and occasionally a Slack client rebuild) on the request path; the worker needs its own trigger; harder to reason about. |

Complement (recommended with A): when a replica meets a sealed value whose key
id it does not hold, it triggers an immediate reload (rate-limited, once per
second) before judging the value unreadable. Key ids are self-describing, so
this nearly closes the window in which a lagging replica shows K-19 after a
Replace or Move.

In-flight work: a request keeps the `Arc<Integrations>` it read; the worker
reads `current()` each round, so a send in flight finishes under the old
snapshot and the next round uses the new one. A failure to open during the
window is the existing `Transient(secret_unavailable)`: never lost, never
emailed (D10). A reload whose stored secrets key id equals the one held keeps
the held key without unwrapping (so a re-wrap under a new instance key never
blinds a replica that has not restarted).

Spike: not needed as a separate task. `install` already swaps integrations
live in the harness; make DISTILL's walking-skeleton scenario (US-IIS-02 sc. 1)
run on two replicas so the first green test is the spike.

---

## Decision 4: atomic create-if-absent and re-seal

| Option | Summary | Trade-offs |
|---|---|---|
| **A (recommended)** | A transaction-scoped advisory lock (`pg_advisory_xact_lock`, one named key) serialises every instance-level key or app write: Generate, Move, Replace, Start over, re-wrap, Slack save/remove. A partial unique index (one `current` secrets key) makes two current keys non-representable as a backstop. Every write that seals a credential (connect handlers, Slack save) takes the same lock **shared** and checks, in its transaction, that its sealing key id is the current one; on mismatch the handler reloads and retries once. | Generate on two replicas stores one key and both answer the resulting state. A lagging replica can never seal under a discarded key, so the old secrets key row can be deleted in the Replace transaction itself: no grace window needed (D4 permits but does not require one). Precedent: the dispatcher's advisory lock. |
| B | Constraints only: `INSERT … ON CONFLICT DO NOTHING` for Generate; `SELECT … FOR UPDATE` on the key row for Replace/Move | Fewer moving parts, but gives no fence for concurrent connect handlers, so Replace must keep the old key as a `previous` row and a later sweep must find and re-seal stragglers. |
| C | SERIALIZABLE transactions with retry | Correct, but every connect path must handle serialization failures; heavier than the problem. |

Transaction boundary (all options): one transaction re-seals every sealed value
(connections and saved Slack app), writes the new key check canary, writes the
wrapped key row, records the activity line and bumps the config version. Any
refusal rolls all of it back (US-IIS-06 AC 2, US-IIS-07 AC 2). The plaintext
never reaches the store: the existing closure shape of `reseal_credentials`.

---

## Decision 5: instance-level Activity

| Option | Summary | Trade-offs |
|---|---|---|
| **A (recommended)** | New append-only table `instance_integration_audit_events` (`actor_id` nullable for "Foundry", closed `action` list, `client_id` nullable, `count` nullable, `created_at`). | No change to a shipped table; closed action list fits I-18 exactly; workspace activity stays untouched (D12). |
| B | Extend `integration_audit_events`: nullable `workspace_id`, more actions, a CHECK tying them | Reuses the reader, but alters a shipped table's NOT NULL and CHECK, and every workspace query must keep excluding NULL rows. |
| C | A general instance-admin audit log for all instance actions | Wider than the feature; other instance actions have their own event tables. |

---

## Decision 6: ADR changes

| Option | Summary | Trade-offs |
|---|---|---|
| **A (recommended)** | New ADRs that supersede named decisions; EIP-004/005 get a status line "Decisions n, m superseded by ADR-IIS-00x" and are otherwise untouched. | History stays readable; each new ADR has its own alternatives. |
| B | Amend EIP-004/005 in place | Fewer files, but rewrites accepted records. |
| C | One combined ADR | Mixes independent decisions (keys, Slack app, reload, audit). |

Proposed set under A:
- **ADR-IIS-001** Two-tier keys: instance key (setting or file) wraps a stored
  secrets key. Supersedes EIP-004 decision 1 and the boot part of 4; amends 3
  (new seal callers: instance page; new slots: Slack app, wrapped key) and 5
  (UI Replace for the secrets key; `doctor rotate-secrets` re-wraps for the
  instance key); 6 becomes the confirmed Start over. Decision 2 (format, AAD)
  unchanged.
- **ADR-IIS-002** The Foundry-made instance key file (location, create-if-absent,
  file → operator).
- **ADR-IIS-003** The Slack app saved by an instance admin, environment wins.
  Supersedes EIP-005 decision 1 / OD-3 in part.
- **ADR-IIS-004** Live reconfiguration across replicas (version poll, reload,
  key fence).
- **ADR-IIS-005** Instance integration activity.
- ADR-IW-001: an amendment note only (B: the words function gains the viewer's
  instance-admin fact; I-19 added after existing links).

---

## Decision 7: other things the stories force

| Item | Recommendation | Alternative |
|---|---|---|
| `FOUNDRY_PUBLIC_URL` unset (I-29) | `IntegrationSettings` keeps whether the URL was configured or fell back; add `Integrations::slack_events_url` and have `slack::manifest` use it. | Compare the URL text to `http://localhost` (fragile). |
| Malformed operator key | Existing `ConfigError::SecretsKey` path; the error names the setting actually used (new or legacy name). | — |
| Malformed or unreadable **key file** | Never refuse start, never overwrite: treat as I-26 ("missing or different"). **Copy gap:** neither I-26 nor I-27 names an unreadable file; the user may want an I-27 variant. | Refuse start (violates "board never goes down"). |
| Test seam for generated keys | `test-support`-gated reader, under check-arch `integrations-test-seam`, that unwraps the stored secrets key for the planted-string grep. The file key needs no seam: the scenario reads its own temp `FOUNDRY_DATA_DIR`. | Log the key at trace level in test builds (risk of leaking into release). |
| Data directory in acceptance | `FOUNDRY_DATA_DIR` read through the scenario lookup, a temp dir per scenario; KPI-IIS-6 in a subprocess lane. | — |
| Enforcement | check-arch: only `integrations/` unwraps the secrets key; only the key-file adapter touches `FOUNDRY_DATA_DIR`; every adapter (key file, keyring, reload) has a probe with a gold test (file race, short read, read-only dir, overlayfs fsync). | — |
| Start over (B) | Discarded connections become `disconnected` (no `lost` transition, so no email), audited per workspace and on the instance. | Mark them lost (sends email; violates D11/KPI-IIS-4 intent). |
| Copy | I-14 with n = 1 ("1 workspace is"); I-2e with n = 0 if 1e-A is chosen. | — |
| Metric | Reuse `probe_failures_total{probe_name="secrets"}` and the degraded line for I-26 on reload, no new metric (DEVOPS may alert on it). | New gauge. |

No requirement D1–D17 was found infeasible. Two need a ruling: 1e (legacy
instance with a key and no credentials) and the key-file copy gap in 7.
