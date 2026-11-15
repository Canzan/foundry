# IIS\-SKR\-003 Gate window sized from the reload envelope
Status: Superseded by IIS-SKR-004

## Context

```text
IIS-SKR-002 sized the live-replica gate window from the reading server's own FOUNDRY_INTEGRATIONS_RELOAD_MS: two beat intervals plus that interval. The row a server beats carries no reload setting of its own, and this value takes no schema change (K5), so the reading server cannot know the cadence of the server it judges. Two replicas set differently therefore break that window: a 200 ms reader reads a 5000 ms server 24 s after its last beat as stale, and Start over is offered while that server still uses the credentials. K4 already names 20 s as the envelope above which this feature's 60-second figure is lost. Everything else IIS-SKR-002 decided holds unchanged.
```

## Decision

```text
The window is a constant sized from that envelope, and the gate reads no reload setting at all: two beat intervals plus the 20 s envelope, so 40 s, with LiveOpeners::at taking the clock only. Inside the envelope the longest gap any server can leave between two beats is one beat interval plus its own setting, at most 30 s, so the window keeps one whole beat interval of margin at every mix of settings. Above the envelope the gate is not safe, which K6 states and Y11 owns, with I-28a as the backstop. BEAT_EVERY stays owned by the replica reload task and REPLICA_STATE_RETENTION by the retention sweep; both only become pub(crate). The window takes no cap: 40 s is far inside the retention hour at every setting.
```

## Alternatives

```text
A window of two beat intervals plus the reading server's own setting (IIS-SKR-002), refused: mixed settings break it, as the counterexample above shows, and mutant m9 is exactly that version. A deployment invariant that every replica shares one setting, refused as the primary answer: nothing in the product enforces or observes it, so it would be a claim with no producer, while the envelope bound is per server and every operator can read its own configuration. The beat row carrying its writer's own reload interval, refused: it needs a column and a migration, which K5 excludes for this value, and it would still not help a reader that has not yet seen the row. A beat cadence that follows the reload interval, refused: the beat is one row write per server per cadence, and the 200 ms of the acceptance lane would multiply the writes of an earlier value's reload task by fifty.
```

## Consequences

```text
A server that has stopped keeps counting as a live opener for up to 40 s, so Start over appears on the admin's next page read after that; the page already re-reads the gate. The gate no longer reads the integrations settings, so that read disappears from gate_of. The window is one constant for every deployment, which makes the law testable without a setting: the gate's answer must not change when the reading server's setting changes. The claim is not checked: quint, java and apalache-mc are absent from the shared Lima VM local-dev, the only environment this project runs tools in, so the design records this axis Blocked and the runtime DESIGNED_NOT_BUILT until that toolchain is there or a human waives the proof.
```
