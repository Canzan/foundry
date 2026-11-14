# IIS\-SKR\-002 Live\-replica gate window follows the beat cadence
Status: Accepted

## Context

```text
IIS-SKR-001 set the live-replica gate window to three reload intervals. A server writes its integration_replica_states beat every 10 s (BEAT_EVERY of crates/foundry-app/src/integrations/reload.rs), at the first reload tick at least that long after its last beat, and that cadence does not follow FOUNDRY_INTEGRATIONS_RELOAD_MS. So at every setting below about 3.34 s the window was shorter than the gap between two beats, and at the 5000 ms default it only equalled it: a server that still opened the stored secrets key read as not live between its beats, and Start over was offered while it still used the credentials. P8 of US-IIS-07 is the promise at stake. Everything else IIS-SKR-001 decided holds unchanged.
```

## Decision

```text
The gate window is derived from the beat cadence, not from the reload interval: two beat intervals plus one reload interval, so 20 s + FOUNDRY_INTEGRATIONS_RELOAD_MS, 25 s at the 5000 ms default and 20.2 s at the 200 ms the acceptance lane sets. The longest gap a running server can leave between two beats is one beat interval plus one reload interval, so the window keeps one whole beat interval of margin over it, which absorbs a failed beat row write, a slow tick or a stalled scheduler of up to one beat interval. The cadence stays where it is owned: BEAT_EVERY stays in the replica reload task (ADR-IIS-004 r8) and REPLICA_STATE_RETENTION in the retention sweep (DD-10); each only becomes pub(crate), so the gate derives its window from the one cadence fact and the retention bound reads the retention constant instead of a restated hour. The window takes no cap: for every setting up to an hour less two beat intervals it stays inside the hour the prune keeps a row for, and above that the prune, not the window, bounds what the gate can see at all. Everything else of IIS-SKR-001 is carried over: the two locked write use-cases, the single stored key with no grace window, the gate re-read inside the POST transaction, Replace withheld under I-26 and I-4, the typed confirmation, and the two ReadOnlyDoor confirm pages.
```

## Alternatives

```text
Keeping a window of three reload intervals, refused: it is shorter than the gap between two beats at every setting below about 3.34 s, and a model mutant carrying it is refuted at state 4. A beat cadence that follows FOUNDRY_INTEGRATIONS_RELOAD_MS, so that such a window would cover it, refused: the beat is one row write per server per cadence, and the 200 ms the acceptance lane sets would multiply the writes of the shipped reload task by fifty; the reload task also belongs to an earlier value, whose write budget this one must not change. A window capped at the retention hour, refused: the prune already removes the rows a longer window would reach, so the cap would add a second reader of the retention constant and change no behaviour. A fixed window independent of the reload interval, refused: a setting above 20 s would again leave it shorter than the gap between two beats.
```

## Consequences

```text
A server that has stopped keeps counting as a live opener for up to two beat intervals plus one reload interval, 25 s at the default, so Start over appears on the admin's next page read after that; the page already re-reads the gate. A server read between two beats now withholds Start over, which is the state the acceptance oracle gains as its own run at the GET and at the POST. Two constants of earlier values become pub(crate) without moving or changing behaviour. The window law is checked over the whole admitted range of the setting, with the longest beat gap recomputed from the rule of the reload task itself. A Foundry server of an earlier release that never beats a replica state row still cannot be seen, and I-28a stays its human backstop.
```
