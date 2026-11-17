# Checker receipts — nav-collapse-toggle

Every command below ran in the shared Lima VM `local-dev`, the only environment
this project runs tools in, from a scratch copy outside the repository so
`_apalache-out` stays out of the tree. `quint --version` = `0.32.0`; Apalache is
the backend `quint verify` launches, on the VM's `java`.

Model bytes checked, `shasum -a 256`:

| file | sha256 |
|---|---|
| `nav_collapse_choice.qnt` | `b6be4c22cdf78e977bc031b13988affb0f1bc834a507708fa5cebcb9413af5c0` |

## Clean model

| command | result |
|---|---|
| `quint typecheck nav_collapse_choice.qnt` | exit 0 |
| `quint run … --invariant=railShowsThisDocumentsOwnChoice --max-steps=14 --max-samples=20000` | `[ok]` |
| `quint run … --invariant=aSyncedDocumentShowsTheLastChoice --max-steps=14 --max-samples=20000` | `[ok]` |
| `quint run … --invariant=storageHoldsTheLastPress --max-steps=14 --max-samples=20000` | `[ok]` |
| `quint verify … --invariant=railShowsThisDocumentsOwnChoice --max-steps=6` | `[ok]` |
| `quint verify … --invariant=aSyncedDocumentShowsTheLastChoice --max-steps=6` | `[ok]` |
| `quint verify … --invariant=storageHoldsTheLastPress --max-steps=6` | `[ok]` |

Stopping bound: 6 steps under Apalache, two documents, six actions. No run
timed out, so no bound was reduced or retried.

## Reachability witnesses — refutation IS the evidence

| command | result | what the counterexample shows |
|---|---|---|
| `quint verify … --invariant=neverCollapsed --max-steps=4` | `[violation]` | the collapsed rail is reachable from the admitted initial state |
| `quint run … --invariant=neverReexpanded --max-steps=10 --max-samples=50000` | `[violation]` | collapse then expand again returns the rail to its start (P3) |
| `quint verify … --invariant=neverReexpanded --max-steps=6` | `[violation]` | the same, exhaustively within the bound |

## One mutant per invariant

Each mutant removes exactly the behaviour its invariant guards and leaves the
invariant untouched. `quint run --invariant` found a counterexample on all three,
so `quint verify` was not needed for the kills.

| mutant | invariant | result |
|---|---|---|
| `mutants/m1_load_ignores_the_stored_choice.qnt` | `aSyncedDocumentShowsTheLastChoice` | `[violation]` — killed |
| `mutants/m2_press_does_not_move_the_rail.qnt` | `railShowsThisDocumentsOwnChoice` | `[violation]` — killed |
| `mutants/m3_press_never_writes_the_key.qnt` | `storageHoldsTheLastPress` | `[violation]` — killed |

## The refutation that changed the design

The first statement of the persistence law was refuted on the CLEAN model, not
on a mutant: `quint run --invariant=freshDocumentShowsTheLastChoice
--max-steps=6 --seed=0x3d94e375386da308` returned a two-state counterexample —
document A loads with no key, the operator presses the control in document B,
the key then holds `collapsed` and A still shows expanded while A's own control
has never been pressed. The guard was law scope, not a carrier defect: the
ordering fact is whether a document read the key more recently than the last
press, so the guard became `syncedA`/`syncedB` and the two-tab divergence is now
an admitted state rather than a silent hole. Recorded as the design's refuted law.

## What no run here establishes

* **Nothing about paint ordering.** "No flash of the expanded nav" is a fact
  about when the browser is permitted to paint relative to a render-blocking
  `<head>` script. It is not a property of this state machine and no command
  above touches it.
* **Nothing about the runtime.** No production byte of this feature exists yet.
  Every design claim these runs support is `DESIGNED_NOT_BUILT`.
* **No timing, throughput or availability claim.** A step count is not time.
