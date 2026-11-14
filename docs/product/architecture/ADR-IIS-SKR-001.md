# IIS\-SKR\-001 Replace or start over the secrets key
Status: Superseded by IIS-SKR-002

## Context

```text
US-IIS-07 asks for two applies over the stored secrets key: a replacement that re-seals every saved credential so nobody reconnects, and a start over that discards the credentials this Foundry cannot open when the instance key is lost with no backup. DISCUSS D4 left the storage of the new key and any grace window for the old one to DESIGN, and D7 handed DESIGN six interleaving and safety obligations to settle formally. Deliverable A already ships the key mode, the credential box, the instance-integrations advisory lock, the all-or-nothing re-seal walk and post-redirect-get with a one-shot session notice.
```

## Decision

```text
Both applies are one write use-case each on Store, beside store_secrets_key and rotate_instance_key: one sqlx transaction that takes the instance-integrations advisory lock before it reads anything, re-uses reseal_sealed_values, writes the key check, records one activity line and bumps the config version, and rolls everything back on any refusal. The integration_secrets_key row holds exactly one key and no grace window is kept: the replacement re-seals every value in the same transaction, so no replica needs the old key afterwards; a replica that still holds it retries its deliveries as SecretUnavailable and adopts the new key on its next reload. The live-replica gate of the start over reads integration_replica_states inside that same locked transaction, judging a replica live while opens_secrets_key holds and beat_at is within three reload intervals. Replace is withheld under I-26 and under I-4, where the all-or-nothing re-seal could never succeed. The typed confirmation is a StartOverConfirmation value the apply cannot be called without. Both confirm pages are ReadOnlyDoors over PageReads.
```

## Alternatives

```text
A grace window keeping the old key readable until every replica had reloaded, refused: the window already costs nothing but a retry, and a second live key would be a second authority over what opens, with no safe moment to discard it. A gate read only at the GET, refused: the fact is about other processes and can change between two requests. One combined use-case for the replacement and the start over, refused: one stores where a key is already stored and re-seals everything, the other discards what it cannot open; a flag would hide which. A per-credential re-seal committing as it goes, refused: it is exactly the mixed state the model shows a mutant reaching.
```

## Consequences

```text
Every apply is atomic at the cost of holding the instance-integrations lock for one pass over the stored sealed rows, with no vendor call inside. A replacement is invisible to the other replicas for up to one reload interval, during which their pages read the connections as not readable and their deliveries retry; nothing is marked lost and nobody is emailed. A Foundry server of an earlier release that never beats a replica state row cannot be seen by the gate, which is why I-28a asks the human to confirm that none is running: that residual risk is a human answer, not a check. The 60-second figure holds while FOUNDRY_INTEGRATIONS_RELOAD_MS stays near its default.
```
