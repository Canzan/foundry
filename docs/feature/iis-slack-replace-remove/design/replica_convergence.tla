\* iis-slack-replace-remove: a committed change reaches every replica.
\*
\* Claim (deadline, as far as a model can carry it): after any instance write
\* bumps the config version, every replica eventually applies that version
\* with no restart. Fairness assumption: each replica's reload task runs
\* (WF on Reload(r)). The 60-second figure of the product promise is the
\* reload interval (FOUNDRY_INTEGRATIONS_RELOAD_MS, default 5000) times the
\* bound this model proves convergence within; the model proves the eventual
\* convergence, NOT the wall-clock figure, which follows from that
\* configured interval.
---- MODULE replica_convergence ----
EXTENDS Naturals

CONSTANTS Replicas, MaxVersion
VARIABLES version, applied

vars == <<version, applied>>

TypeOK == /\ version \in 0..MaxVersion
          /\ applied \in [Replicas -> 0..MaxVersion]

Init == version = 0 /\ applied = [r \in Replicas |-> 0]

\* Replace, the confirm, Remove, Generate or Move: one version bump.
Commit == /\ version < MaxVersion
          /\ version' = version + 1
          /\ applied' = applied

Reload(r) == /\ applied[r] # version
             /\ applied' = [applied EXCEPT ![r] = version]
             /\ version' = version

Next == Commit \/ \E r \in Replicas : Reload(r)

Spec == Init /\ [][Next]_vars /\ \A r \in Replicas : WF_vars(Reload(r))

\* The claim.
Converges == \A r \in Replicas : [](applied[r] # version => <>(applied[r] = version))

\* --- Fairness protocol variants, used by the three recorded TLC runs.
SpecNoFairness == Init /\ [][Next]_vars

\* A progress mutant WITH fairness: a replica that reloads only on an even
\* version never applies an odd one.
ReloadEvenOnly(r) == Reload(r) /\ version % 2 = 0
NextMutant == Commit \/ \E r \in Replicas : ReloadEvenOnly(r)
SpecMutant == Init /\ [][NextMutant]_vars
                   /\ \A r \in Replicas : WF_vars(ReloadEvenOnly(r))
====
