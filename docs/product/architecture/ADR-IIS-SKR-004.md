# IIS\-SKR\-004 The reload envelope becomes a checked bound
Status: Accepted

## Context

```text
Two things in IIS-SKR-003 no longer hold. Its Consequences say the gate window model is unchecked, which the receipt beside the model contradicts: it was replayed in the shared Lima VM local-dev. And its Decision sizes the window from a 20 s envelope that nothing enforced, with I-28a named as the backstop above it - but config.rs floored FOUNDRY_INTEGRATIONS_RELOAD_MS at 1 ms with no ceiling, and the shipped I-28a sentence asks only about servers of an earlier release. A uniform 60000 setting leaves beats up to 70 s apart against the 40 s window, so a server that still opens the key reads as gone and Start over offers to discard the credentials it is serving with. The window of IIS-SKR-002 covered that uniform case, so the constant window introduced the region. Everything else IIS-SKR-003 decided holds unchanged.
```

## Decision

```text
The envelope becomes a checked bound on the setting. FOUNDRY_INTEGRATIONS_RELOAD_MS is admitted from 1 ms to 20 s inclusive; above that the settings read refuses with a config error naming the setting, the value, the bound and why the gate needs it, beside the MIN_LEASE_SECONDS check that already does this, and main.rs refuses to start on it through the catch-all it already has. The window stays the constant 40 s, two beat intervals plus that bound, and the gate still reads no setting. The backstop claim is withdrawn: I-28a covers only the server of an earlier release that beats no row at all, which is what its words say.
```

## Alternatives

```text
Clamping the setting with a logged warning, refused: the declared setting and the effective one would differ, so the operator reads one value while the gate uses another, and a warning is missed exactly when nobody is watching. A gate window of two beat intervals plus the larger of the bound and this server's own setting, refused: it makes the window depend again on the reading server's setting, which is the mixed-setting hole this round closed, and above the retention hour it would read rows the prune has removed. Extending the I-28a copy so the typed confirmation covers the setting hazard, refused: it would move a checkable bound onto a human sentence, and it changes locked product copy for a condition the software can refuse by itself. Leaving the bound as an assumption, refused: it is the same class of defect as the one this delta fixed, one setting further out.
```

## Consequences

```text
A deployment polling more slowly than 20 s must lower the setting before this release; that breaking change is stated in the RELEASING.md settings table and the product brief settings list, and the refusal names the remedy. One check now serves two promises: the gate window and the 60-second figure of K4. The model is checked, not unchecked: in local-dev with quint 0.32.0 and openjdk 21.0.12.1, both invariants NoError under quint verify at bound 44, the tightness witness refuted and mutants m8 and m9 killed, recorded command by command with its exit status in docs/feature/iis-secrets-key-replace/design/live_replica_gate.receipt.txt. The runtime stays DESIGNED_NOT_BUILT: no production code is built yet, and the executable projections of the window law and of the bound are owed by S2.
```
