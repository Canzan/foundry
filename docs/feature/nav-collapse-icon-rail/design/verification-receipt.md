# Checker receipts — nav-collapse-icon-rail

REPLAYED IN FULL after the design review. The first recording could not be
replayed by the reviewer because `/` in the shared Lima VM `local-dev` was at
100 percent; the host freed it, and every command below was then re-run from a
fresh scratch directory. `df -h /` at the start of the replay: `394G` total,
`130G` used, `248G` available, 35 percent. `head_guard.qnt` also gained the
names over-approximation the review asked for, so its bytes and its results are
new, not merely re-run.

Every command ran in `local-dev` through `scripts/test-in-lima.sh`, from
`/tmp/qir` — a scratch copy outside the repository, recreated with `rm -rf`
before the replay, so `_apalache-out` stays out of the tree. `quint --version`
= `0.32.0`; Apalache is the backend `quint verify` launches on the VM's `java`.
Each command ran under `timeout 180`.

Model bytes checked, `sha256sum`, at the replay:

| file | sha256 |
|---|---|
| `icon_rail.qnt` | `0a61ac1540f4f0fdf453c76aea6e322f0b35e6906ab1b571bca8a63e942f81e3` |
| `head_guard.qnt` | `94646414d26425398de4da4dac4bf0a14defd01ffda57eaffb8da239f62ea4ac` |

`icon_rail.qnt` is the product exploration's model, adopted unchanged.
`head_guard.qnt` extends the exploration's model twice: with the `recognition`
dimension (`wide` / `narrow`) and an aliased-stamp head, so the review's
narrow-classifier finding is falsifiable; and with `settledOnly`, which
separates what an asset NAMES from what it really DOES before the paint, so the
names test's over-approximation is expressible. Its earlier sha256 was
`ae46926dc9ef6d3d339eb3a01be31a64b196b3e2991b21ebfb987c5f84a4460b`.

| command | result |
|---|---|
| `quint typecheck icon_rail.qnt` | exit 0 |
| `quint typecheck head_guard.qnt` | exit 0 |

## Sampling runs on the clean models

| command | result |
|---|---|
| `quint run icon_rail.qnt --init=init_icons_keep --step=step --invariants noTrap matrixNoTrap noInvisibleFocusable focusNeverInvisible matrixNoInvisibleFocusable nextSignInStartsCollapsed --seed=7 --max-steps=8 --max-samples=20000` | exit 0, `[ok] No violation found` |
| `quint run head_guard.qnt --init=init_defer_attr_wide --step=step --invariants totalVerdicts noThirdVerdict gateMatchesPaint classifierSeesEveryStamper overApproximationNeverSilent stampsBeforePaint_acceptedHead --seed=11 --max-steps=12 --max-samples=5000` | exit 0, `[ok] No violation found` |

## Exhaustive verification (Apalache), bound 6

| command | result |
|---|---|
| `quint verify head_guard.qnt --init=init_defer_attr_wide --step=step --invariant=totalVerdicts --max-steps=6` | exit 0, `The outcome is: NoError` |
| `quint verify head_guard.qnt … --invariant=noThirdVerdict --max-steps=6` | exit 0, `NoError` |
| `quint verify head_guard.qnt … --invariant=classifierSeesEveryStamper --max-steps=6` | exit 0, `NoError` |
| `quint verify head_guard.qnt … --invariant=gateMatchesPaint --max-steps=6` | exit 0, `NoError` |
| `quint verify head_guard.qnt … --invariant=overApproximationNeverSilent --max-steps=6` | exit 0, `NoError` |
| `quint verify icon_rail.qnt --init=init_icons_keep --step=step --invariant=noTrap --max-steps=6` | exit 0, `NoError` |
| `quint verify icon_rail.qnt … --invariant=matrixNoTrap --max-steps=6` | exit 0, `NoError` |
| `quint verify icon_rail.qnt … --invariant=noInvisibleFocusable --max-steps=6` | exit 0, `NoError` |
| `quint verify icon_rail.qnt … --invariant=focusNeverInvisible --max-steps=6` | exit 0, `NoError` |
| `quint verify icon_rail.qnt … --invariant=matrixNoInvisibleFocusable --max-steps=6` | exit 0, `NoError` |
| `quint verify icon_rail.qnt … --invariant=nextSignInStartsCollapsed --max-steps=6` | exit 0, `NoError` |

Stopping bound: 6 steps under Apalache on both models, each run well inside the
180 s budget. No run timed out, so no bound was reduced, narrowed or retried.
The two static tables (`matrix`, twelve state combinations; `verdicts`,
twenty-four head-script shapes) are exhaustive over their combinations by
construction at every bound.

## One mutant per law — `quint run` found every kill

Each mutant is a behavioural variant selected by its own `init` action; it
removes exactly the behaviour its invariant guards and leaves the invariant
untouched. `quint run --invariant` found a counterexample on all six, so
`quint verify` was not needed for the kills.

| mutant (init action) | invariant | result |
|---|---|---|
| `init_hide_none` — the collapsed rail hides its links with `display: none` | `matrixNoTrap` | `[violation]` — killed |
| `init_hide_clip` — the collapsed rail hides its links visually, leaving them focusable | `matrixNoInvisibleFocusable` | `[violation]` — killed |
| `init_icons_clear` — Sign out clears the stored collapse choice | `nextSignInStartsCollapsed` | `[violation]` — killed |
| `init_as_written_wide` — "every other head script defers" read as written | `noThirdVerdict` | `[violation]` — killed |
| `init_defer_attr_narrow` — the classifier recognises only the direct idioms | `classifierSeesEveryStamper` | `[violation]` — killed |
| `init_defer_attr_narrow` — the same | `gateMatchesPaint` | `[violation]` — killed |

## Reachability, completion and hazard witnesses — refutation IS the evidence

| command | result | what the counterexample shows |
|---|---|---|
| `quint run icon_rail.qnt --init=init_icons_keep --invariant=w_collapseThenActivateIcon --seed=7 --max-steps=8 --max-samples=20000` | `[violation]` | an operator collapses the rail and then activates a destination from the icon rail |
| `quint run icon_rail.qnt … --invariant=w_railSignOutThenSignIn` | `[violation]` | she signs out from the collapsed rail and her next sign-in starts collapsed |
| `quint run head_guard.qnt --init=init_defer_attr_wide --invariant=w_acceptedHeadDone --seed=11 --max-steps=12 --max-samples=5000` | `[violation]` | the accepted head reaches `done`: the paint-order model terminates |
| `quint run head_guard.qnt --init=init_defer_attr_narrow --invariant=w_aliasAcceptedWithLateStamp` | `[violation]` | the narrow classifier ACCEPTS a head whose aliased stamp lands after the first paint — the review finding, as a trace |
| `quint run head_guard.qnt --init=init_defer_attr_wide --invariant=settledOnlyNeverRenderBlocking` | `[violation]` | THE NAMES TEST HAZARD, deliberately refutable: a settled-DOM asset that names storage and `documentElement`, made PLAIN to satisfy the gate's refusal, is accepted while sitting on the render-blocking path. No gate arm covers this; the resolution is the recorded design guidance, and `overApproximationNeverSilent` proves the refusal that precedes it is never silent |

## Findings the clean models carry rather than repair

| command | result | what it shows |
|---|---|---|
| `quint run icon_rail.qnt --init=init_icons_keep --invariant=expandedShowsCurrentRights --seed=7 --max-steps=8 --max-samples=20000` | `[violation]` | after a change of rights with no page load the nav still shows the rights of the last load, in both geometries. Pre-existing, not caused by collapsing; an admitted state, not repaired here. |

## What these receipts do NOT establish

The models are DESIGN evidence only; no runtime behaviour is observed by any
command above. Not modelled, so not evidenced here: pixel layout, including the
480 px bar and the icon wrapping; screen-reader output beyond what is visible
and focusable; a second tab; how the gate finds the names inside an asset whose
identifiers are built at runtime; inline scripts and scripts added at runtime;
and whether an asset that names storage and `documentElement` really touches
them before the paint, which no names test can decide. The icon wrapping, the
label flyout and the accessible names need the real browser observation, which
belongs to the acceptance oracle.
