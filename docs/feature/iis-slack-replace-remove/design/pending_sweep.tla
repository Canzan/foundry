\* iis-slack-replace-remove: an expired pending Slack app is eventually swept.
\*
\* Claim (liveness): whenever the pending row is expired, it eventually stops
\* being expired. Fairness assumption: the worker's hourly retention sweep
\* runs (WF on Sweep) and the clock advances (WF on Tick). Without that
\* fairness the claim is false, which is why TLC and not a finite table
\* decides it.
---- MODULE pending_sweep ----
EXTENDS Naturals

CONSTANTS Admins, Apps, Ttl, Horizon
VARIABLES pending, pendingBy, pendingExpires, now

NoApp == "none"
NoAdmin == "nobody"
vars == <<pending, pendingBy, pendingExpires, now>>

TypeOK == /\ pending \in Apps \cup {NoApp}
          /\ pendingBy \in Admins \cup {NoAdmin}
          /\ now \in 0..Horizon
          /\ pendingExpires \in 0..(Horizon + Ttl)

Init == /\ pending = NoApp
        /\ pendingBy = NoAdmin
        /\ pendingExpires = 0
        /\ now = 0

Hold(a, p) == /\ pending' = p
              /\ pendingBy' = a
              /\ pendingExpires' = now + Ttl
              /\ now' = now

Confirm == /\ pending # NoApp
           /\ now < pendingExpires
           /\ pending' = NoApp /\ pendingBy' = NoAdmin /\ pendingExpires' = 0
           /\ now' = now

Tick == /\ now < Horizon
        /\ now' = now + 1
        /\ UNCHANGED <<pending, pendingBy, pendingExpires>>

Sweep == /\ pending # NoApp
         /\ now >= pendingExpires
         /\ pending' = NoApp /\ pendingBy' = NoAdmin /\ pendingExpires' = 0
         /\ now' = now

Next == \/ \E a \in Admins, p \in Apps : Hold(a, p)
        \/ Confirm
        \/ Tick
        \/ Sweep

Expired == pending # NoApp /\ now >= pendingExpires

Spec == Init /\ [][Next]_vars /\ WF_vars(Sweep) /\ WF_vars(Tick)

\* The claim.
SweptEventually == [](Expired => <>(~Expired))

\* --- Fairness protocol variants, used by the three recorded TLC runs.
\* 1. No fairness: the claim must FAIL, which is what makes the fairness
\*    assumption load-bearing rather than decorative.
SpecNoFairness == Init /\ [][Next]_vars

\* 2. A progress mutant WITH fairness: a sweep that only deletes one admin's
\*    rows leaves another admin's expired row forever.
SweepMutant == /\ Sweep
               /\ pendingBy = "priya"
NextMutant == \/ \E a \in Admins, p \in Apps : Hold(a, p)
              \/ Confirm
              \/ Tick
              \/ SweepMutant
SpecMutant == Init /\ [][NextMutant]_vars /\ WF_vars(SweepMutant) /\ WF_vars(Tick)
====
