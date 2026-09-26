# Slice 01 delivery notes — cards drag on Pointer Events for the mouse

Steps 01-01 to 01-04, all GREEN, 2026-09-26. Production file changed across the
slice: `crates/foundry-app/static/js/board-dnd.js` (plus the DDD-22 check-arch
rule in e92e6dd). The only `.feature` diffs are the `@pending` removals in
`card-pointer-drag.feature`; every other shipped `.feature` is byte-identical.

## Gates (01-04 close)

| Gate | Result |
|---|---|
| Slice gate `FOUNDRY_ACCEPTANCE_TAGS=us-cpd-01` | 12/12 scenarios, 102/102 steps; no `@us-cpd-01 … @pending` left |
| Guards | cdf 54/54 (400 steps), blr 26/26 (141), kb 38/38 (261) |
| POST body | `moveBody` and `dropInto` untouched since 01-02; fetch-spy scenarios in cpd and cdf GREEN |
| `cargo xtask check-arch` | passed |
| `cargo xtask smoke` | all gates green |
| fmt, clippy `-D warnings` | clean |
| Browser | Chrome 151.0.7922.108 (`selenium/standalone-chrome:latest`) |

## #11 — a desktop file is still swallowed right after a pointer drag

It was already GREEN when un-pended, because the DDD-1 swallow was kept verbatim
in 01-02. Its Given (a real mouse drag of AUTH-43 into Done) runs every time. To
show the scenario discriminates, a temporary local fault skipped the swallow
(`dragover`/`drop`) once a lifted release had happened. That turned #11 RED on
its named oracle, "the board must claim the file's dragover and cancel its drop
(swallowed, D3)", and left the other 11 scenarios GREEN. The fault was then
reverted and cmp-verified.

## pointercancel (DDD-8, DDD-9, AC-1.6)

The 01-02 handler already met the contract, so there is no behaviour change. It
is filtered by the session's `pointerId`. After the lift it goes through
`endPress()`, which runs the session's `end()`, the same teardown Escape uses:
no request is sent, the card never left its slot, and nothing is left behind
(no lit lane, marker, ghost, `data-card-dragging` or `[data-card-lifted]`).
Before the lift it only drops the pending press. Only the comments changed: the
header and the handler now say that pointercancel is one of the DDD-8 endings.
The mouse case has no trusted trigger in the lane, because dragstart is
cancelled (DDD-19, spike Q1). The touch instrument is #24 in phase 03.

## Named faults (one seeded at a time; us-cpd-01 run; restored and cmp-verified)

| Fault | Result | Killed on |
|---|---|---|
| Swallow skipped after a session (01-04 RED for #11) | killed | #11: "the board must claim the file's dragover and cancel its drop" |
| Lift threshold 0 | killed | #5: "AUTH-41's edit dialog did not open … the click guard resets on the next press" |
| Click guard never reset | killed | #5: same oracle (recorder: click=1, eaten by the stale guard) |
| dragstart not cancelled | killed | #10: "the board let the browser start its own drag of a card (dragstart not cancelled)". All 12 went RED: the native drag's pointercancel abandons every lift |
| Ghost left behind | killed | #3, #4, #6: "a carried card is left behind / outlives the release" |
| Mouse pointercancel not reverting | survived, by design | the mouse has no trusted pointercancel once dragstart is cancelled; the instrument is touch #24 (phase 03) |
| Click guard never armed | survived, Chrome-specific | Chrome sends no click after a lifted mouse release (01-03), so there is nothing for the guard to eat. Check in Firefox (below) |

No kill was a timeout. Kill rate over the killable named faults: 5/5 (4/4 of the
brief's list plus the #11 fault). Counting both expected survivors: 5/7. The
per-feature mutation gate (≥80% on modified files) runs in Phase 5.

## Dogfood — OWED — user (real mouse; cannot be automated here)

Do this on the same day, in **Chrome** and **Firefox**, against the `./restart.sh`
dev server:

- [ ] Drag a card into another lane with a real mouse, then reload: the move
      persisted at the slot the marker showed.
- [ ] Click a card without moving it: its dialog opens.
- [ ] Drag a card and release it: no dialog opens.
- [ ] Press Escape mid-drag: the card goes back to its slot, nothing is lit or
      marked, no request is sent, and only that layer is peeled.
- [ ] Delete a card from its popup (the board refreshes in place), then drag
      another card: it lands and persists.
- [ ] Drop a real Finder file `keys.png` on a lane, straight after a drag: it is
      swallowed (no-drop cursor, no navigation, no request).
- [ ] **Firefox click guard:** drag a card and release it over its own card or
      a lane. If Firefox delivers a click after the lifted release, confirm no
      dialog opens. This is the only place the "guard never armed" survivor
      can be seen.
