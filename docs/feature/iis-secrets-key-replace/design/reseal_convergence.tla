\* iis-secrets-key-replace: the window where replicas hold different keys
\* closes, and nothing queued in it is lost (Q2, P4).
\*
\* Claim: after the replace bumps the config version in its own transaction,
\* every replica eventually holds the new secrets key with no restart, and a
\* delivery waiting through the window eventually goes out. Fairness
\* assumption: each replica's reload task runs, and each replica's delivery
\* attempt runs (WF on Reload(r) and Deliver(r)). The 60-second figure of the
\* promise is the reload interval (FOUNDRY_INTEGRATIONS_RELOAD_MS, default
\* 5000) times the bound this model proves convergence within; the model
\* proves the eventual convergence, NOT the wall-clock figure, which follows
\* from that configured interval.
---- MODULE reseal_convergence ----
EXTENDS Naturals

CONSTANTS Replicas, MaxGen
VARIABLES version, stored, applied, held, queued

vars == <<version, stored, applied, held, queued>>

TypeOK == /\ version \in 0..MaxGen
          /\ stored \in 0..MaxGen
          /\ applied \in [Replicas -> 0..MaxGen]
          /\ held \in [Replicas -> 0..MaxGen]
          /\ queued \in BOOLEAN

Init == /\ version = 0
        /\ stored = 0
        /\ applied = [r \in Replicas |-> 0]
        /\ held = [r \in Replicas |-> 0]
        /\ queued = FALSE

\* The replace: one transaction stores the new key generation, seals every
\* saved value under it and bumps the version. A delivery is waiting.
Replace == /\ version < MaxGen
           /\ stored' = stored + 1
           /\ version' = version + 1
           /\ queued' = TRUE
           /\ UNCHANGED <<applied, held>>

\* A reload adopts the version and re-derives the secrets key it holds.
Reload(r) == /\ applied[r] # version
             /\ applied' = [applied EXCEPT ![r] = version]
             /\ held' = [held EXCEPT ![r] = stored]
             /\ UNCHANGED <<version, stored, queued>>

\* A delivery goes out from a replica that holds the key its credential is
\* sealed under; on a stale replica it is SecretUnavailable and stays queued.
Deliver(r) == /\ queued
              /\ held[r] = stored
              /\ queued' = FALSE
              /\ UNCHANGED <<version, stored, applied, held>>

Next == Replace \/ \E r \in Replicas : (Reload(r) \/ Deliver(r))

Spec == Init /\ [][Next]_vars
             /\ \A r \in Replicas : (WF_vars(Reload(r)) /\ WF_vars(Deliver(r)))

\* The claims.
EveryReplicaOpens == \A r \in Replicas : [](held[r] # stored => <>(held[r] = stored))
DeliveryArrives == [](queued => <>(~queued))

\* --- Fairness protocol variants, used by the three recorded TLC runs.
SpecNoFairness == Init /\ [][Next]_vars

\* A progress mutant WITH fairness: the reload adopts the version but does not
\* re-derive the secrets key, so the window never closes.
ReloadKeepsKey(r) == /\ applied[r] # version
                     /\ applied' = [applied EXCEPT ![r] = version]
                     /\ UNCHANGED <<version, stored, held, queued>>
NextMutant == Replace \/ \E r \in Replicas : (ReloadKeepsKey(r) \/ Deliver(r))
SpecMutant == Init /\ [][NextMutant]_vars
                   /\ \A r \in Replicas : (WF_vars(ReloadKeepsKey(r)) /\ WF_vars(Deliver(r)))
====
