# DELIVER wave decisions: instance-integration-setup (Deliverable A)

## Demo Evidence — 2026-11-07

Method for every story: the real release `foundry` binary (HEAD a871310) run by
hand in the Lima VM `foundry-test`. It ran against a throwaway `postgres:16-alpine`
container with a temp `FOUNDRY_DATA_DIR` and `FOUNDRY_PUBLIC_URL=https://foundry.example.org`.
The first admin was claimed through the `[BOOTSTRAP]` link (`POST /bootstrap`, 303,
session cookie set), and the flows were driven with curl and a cookie jar. The Slack
values are the planted test values from the user stories, never the `.env` ones. The
captured lines are visible page text with the HTML tags stripped. The container and
temp dirs were removed afterwards.

Command: `scripts/test-in-lima.sh bash -c "<demo script>"`, exit 0.

### US-IIS-08: a standalone Foundry makes its own instance key (observed)

Started with no key settings:

```
-rw------- 1 jeffbailey jeffbailey 44 Nov  7 07:47 instance.key
    instance.key mode 600, 44 bytes
    key value occurrences in server log: 0
GET /admin/instance/integrations -> HTTP 200
    | Instance key: made by Foundry in <tmp>/data08/instance.key. Back up this file apart from the database. Every Foundry server must use the same file. Without it, saved connections must be reconnected.
    | Generate secrets key
```

Generate then answered I-21 (see US-IIS-02).

### US-IIS-01: see what this Foundry has and lacks (observed)

```
GET /admin/instance/workspaces -> HTTP 200
    | <a href="/admin/instance/integrations">Integrations          (I-1)
GET /admin/instance/integrations -> HTTP 200
    | Integrations for this Foundry
    | This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials.   (I-3)
    | Matrix: needs the secrets key (above). Nothing else is set up for Matrix on this Foundry.       (I-9)
```

The Slack section (I-5, manifest, Redirect URL and Events URL) appears once the secrets
key is set, as `page_state` defines it:

```
    | Slack: not set up. Create the Foundry Slack app in Slack from the manifest below, then enter its credentials.
    | https://foundry.example.org/workspace/integrations/slack/return
    | https://foundry.example.org/integrations/slack/events
    manifest display_information hits: 1
```

With `FOUNDRY_PUBLIC_URL` absent (I-29):
`FOUNDRY_PUBLIC_URL is not set, so the addresses below use http://localhost:38082, which Slack cannot reach. …`

Finding: with `FOUNDRY_PUBLIC_URL=` set to an empty string, the page reads
"…below use , which Slack cannot reach" and the manifest URLs come out relative
(`/integrations/slack/events`). Cause: `main.rs:122` takes the raw variable as the
fallback, while `IntegrationSettings` treats an empty value as unset. This is not
fixed here.

### US-IIS-02: generate the secrets key (observed)

```
POST /admin/instance/integrations/secrets-key -> HTTP 200
    | Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it.   (I-21)
    | Secrets key: set. Credentials entered here are sealed with it.                                                          (I-2)
    | Matrix: ready. Workspace admins connect their own Matrix bot on their workspace's integrations page.                     (I-8)
GET /workspace/integrations -> HTTP 200
    | <form method="post" action="/workspace/integrations/matrix">   Connect Matrix
```

There was no restart: the same process answered both requests.

### US-IIS-03: enter the Slack app and Slack works at once (observed)

```
    Add to Slack on workspace page before save: 0
POST /admin/instance/integrations/slack -> HTTP 200
    | Saved. Workspace admins can now choose Add to Slack.          (I-10)
    | Slack: set up here · app 6123456789012.7234567890123 · saved by Priya Raman · 7 Nov 2026   (I-6)
    | Client secret: set
    | Signing secret: set
    secret values echoed in page: 0
GET /workspace/integrations -> HTTP 200
    | Add to Slack
    | 7 Nov 2026 · Priya Raman generated the secrets key           (I-18)
    | 7 Nov 2026 · Priya Raman saved the Slack app (6123456789012.7234567890123)
```

### US-IIS-06: upgrade with the key and the Slack app in the environment (observed)

This was a fresh database with the legacy `FOUNDRY_SECRETS_KEY` and the planted
`SLACK_CLIENT_ID`, `SLACK_CLIENT_SECRET` and `SLACK_SIGNING_SECRET` set. No key file
was made (the data dir held 0 files).

```
GET /admin/instance/integrations -> HTTP 200
    | Instance key: set by the operator.
    | Credentials saved here are sealed directly with the instance key. Move to a secrets key to manage it here.  (I-2e0)
    | Slack: set in Foundry's environment, app 6123456789012.7234567890123. To manage it here, enter the same app below; …  (I-7)
POST /admin/instance/integrations/slack -> HTTP 200
    | Saved. The app in Foundry's environment stays in effect until SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from it and Foundry restarts; then this one is used.  (I-17)
    | Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here.  (I-2e)
POST /admin/instance/integrations/secrets-key/move -> HTTP 200
    | Moved. 1 saved credential is sealed under a new secrets key. Nobody needs to reconnect.   (I-22)
    | Secrets key: set. Credentials entered here are sealed with it.
GET /workspace/integrations -> HTTP 200
    | Add to Slack
```

### `foundry doctor rotate-secrets`

This ran against the US-IIS-08 database with the server stopped. The operator key was in
`FOUNDRY_INSTANCE_KEY` (new, <redacted>) and the file key in
`FOUNDRY_INSTANCE_KEY_PREVIOUS` (<redacted>). Exit code 0.

```
rotate-secrets: 0 credential(s) sealed again under key b93fed4562e955b5
rotate-secrets: the secrets key is now wrapped under instance key 916175606244d13b
```

After a restart with the operator key, the page read `Instance key: set by the operator.`,
`Secrets key: set. …` and the saved Slack app (I-6). It did not show I-30, because the
rotation had already re-wrapped the key, so no file takeover happened at boot.

## Post-merge integration gate — 2026-11-07

All lanes ran in the Lima VM `foundry-test` through `scripts/test-in-lima.sh`, after
`cargo build -p foundry-app --bin foundry` and `eip_warm`, with
`FOUNDRY_EIP_HANDWRITTEN_SLACK` unset, `CARGO_BUILD_JOBS=4` and HEAD a871310.

| Lane | Scenarios | Steps | Result |
|------|-----------|-------|--------|
| instance-integration-setup | 59 (59 passed) | 597 (597 passed) | PASS |
| external-integration-plugins | 181 (181 passed), 10 features | 1942 (1942 passed) | PASS |
| integrations-wayfinding | 54 (54 passed) | 400 (400 passed) | PASS |
| docker-compose (`.env` from `.env.example`) | 4 (4 passed) | 23 (23 passed) | PASS |
| `cargo xtask ci` (FOUNDRY_XTASK_INCLUDE_DOCKER=1) | — | — | FAIL |

`xtask ci` verdict: `` xtask ci :: FAILED at `cargo test --workspace (excl. foundry-acceptance) --release` with status 101 ``.
`foundry-app --lib`: 172 passed, 1 failed.

```
integrations::key_file::tests::two_servers_racing_on_one_directory_hold_one_key ... FAILED
panicked at crates/foundry-app/src/integrations/key_file.rs:307:40:
expected a held key, got Unwritable
```

The test is not one of the known flakes. Run alone 30 times, it passed 19 and failed 11,
each time with `got Unwritable`, so this is a reproducible regression. A likely cause,
not confirmed: one racer's `remove_leftovers` deletes another racer's
`instance.key.tmp.*` before that racer's `link(2)`, and the resulting `NotFound` is
reported as Unwritable (I-27). The gate stopped at this step, so the full acceptance
`all` step did not run. Nothing was fixed.
