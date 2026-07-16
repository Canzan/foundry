# Prioritization: keyboard-shortcut-bindings

## Release Priority

**No walking skeleton** (D-8) — brownfield; the server contracts are shipped, routed, and green, so the
end-to-end path already exists. Priority 1 is a thin real capability, not a skeleton.

| Priority | Slice | Target Outcome | KPI | Rationale |
|----------|-------|---------------|-----|-----------|
| 1 | **01 — help overlay + Esc** (US-01) | The product stops lying: the key that displays the promise fulfils it | KPI-1 (0/7 → 2/7) | Arc starts at **betrayed**, not anxious — credibility must be repaid first. Stands up the dispatch layer all other slices reuse. Forces **ODD-3** (global mount) into the open at slice 1 instead of slice 4. |
| 2 | **02 — the guards** (US-02) | Typing still works; `Cmd+C` still copies | KPI-2 (0 captured keystrokes) | **The riskiest assumption, deliberately before the reward.** Every shortcut is a character people type; without the guard the layer is **worse than nothing**. Must land before the first character-key binding (slice 03). |
| 3 | **03 — `c` + Esc-closes-modal** (US-03, US-07 partial) | File an issue with zero mouse actions | KPI-1 (→ 4/7), KPI-3 | Highest-value single shortcut (first in the help list, most-used key in any tracker). Now **safe** to bind because slice 02 shipped the guard. Paired with `Esc` so it is never a one-way door. |
| 4 | **04 — `/` search** (US-04) | Find an existing issue with zero mouse actions | KPI-1 (→ 5/7), KPI-3 | Independent of selection; completes the *acting* capabilities. Creates the **second j/k surface** (the results list), handing slice 05 a richer target (ODD-6). |
| 5 | **05 — `j`/`k` + `Enter`** (US-05, US-06) | Move and open — the keyboard loop closes | KPI-1 (→ **7/7**), KPI-3, KPI-4 | Carries the most uncertainty (**ODD-1** carrier collision, **ODD-5** swap survival, **ODD-7** a11y) and depends on slices 01+02 being proven. Ships the two together because selection without `Enter` is decoration. Deliberately **deletes a currently-green test**. |

## Value × Urgency / Effort

| Slice | Value | Urgency | Effort | Score | Note |
|-------|-------|---------|--------|-------|------|
| 01 help + Esc | 4 | 5 | 2 | **10.0** | Urgency 5: repays the broken promise and unblocks every other slice (the dispatch layer). |
| 02 guards | 3 | 5 | 2 | **7.5** | Value 3 in isolation (it enables rather than delights) but Urgency 5 — it gates slice 03. Its true value is **preventing a worse-than-nothing state**, which the formula under-counts. |
| 03 `c` | 5 | 4 | 2 | **10.0** | Highest-value single key. Tied with 01; sequenced after because the guard must land first. |
| 04 `/` | 4 | 3 | 2 | **6.0** | Independent; safely fourth. |
| 05 `j`/`k`/`Enter` | 5 | 3 | 3 | **5.0** | Highest value, highest effort/uncertainty. Last by **dependency and risk**, not by effort. |

> **Where the formula is overridden.** Raw scores would run 01/03 → 02 → 04 → 05. We deliberately promote
> **slice 02 (guards) to position 2**, ahead of higher-scoring slice 03. Reason: binding `c` before the guard
> exists ships a state where Mei **cannot type the letter c into a title** — strictly worse than shipping
> nothing. The formula scores value *delivered*; it does not score harm *avoided*. Riskiest-assumption-first
> (Maurya) governs here, and the tie-break rule (Walking Skeleton > Riskiest Assumption > Highest Value) puts
> the riskiest assumption ahead of the highest value in the absence of a skeleton.

## Backlog Suggestions

| Story | Slice | Priority | Outcome Link | Dependencies |
|-------|-------|----------|-------------|--------------|
| US-01 — `?` help overlay in place | 01 | P1 | KPI-1 | None (stands up the dispatch layer) |
| US-02 — typing is never captured | 02 | P1 | KPI-2 | US-01 (the layer it guards) |
| US-03 — `c` files an issue | 03 | P2 | KPI-1, KPI-3 | US-01, **US-02 (hard: must not bind `c` before the guard)** |
| US-07 — `Esc` closes topmost | 01 (overlay) + 03 (modal) | P2 | KPI-1 | US-01; then US-03/US-04/US-06 (the layers it closes) |
| US-04 — `/` searches | 04 | P2 | KPI-1, KPI-3 | US-01, US-02 |
| US-05 — `j`/`k` selection | 05 | P3 | KPI-1, KPI-3, KPI-4 | US-01, US-02; **ODD-1** (carrier retirement) |
| US-06 — `Enter` opens selected | 05 | P3 | KPI-1, KPI-3 | **US-05 (hard: no selection, nothing to open)** |

## Priority Rationale

**Outcome impact, then dependency, then risk** — with one deliberate override.

1. **Credibility before capability.** Mei's arc starts at *betrayed*: the help page listed seven shortcuts, she
   pressed `c`, nothing happened. She half-suspects the whole page is decorative. The cheapest, most direct
   repayment is to make **`?` itself** work — the one shortcut where the promise and its fulfilment are the same
   keystroke. Slice 01 also happens to stand up the dispatch layer everything else reuses, so the emotionally
   right first move is also the architecturally right one.

2. **Risk before reward (the override).** Slice 02 ships no new capability — its entire user-visible value is
   *the absence of a harm*: Mei can still type. It is promoted ahead of higher-scoring `c` because the failure
   mode is not "less value", it is **negative value**: a shortcut layer that eats keystrokes is worse than no
   shortcut layer, and it is the **default outcome of the obvious implementation** (bind `keydown`, switch on
   `key`). Sequencing it before slice 03 means the worse-than-nothing state is never shipped, not even briefly.

3. **Value within the safe zone.** Once the guard holds, order by outcome impact: `c` (file — the most-used
   action) → `/` (find) → `j`/`k`/`Enter` (move and open). Slice 04 before 05 additionally hands slice 05 the
   search-results list as a second selection surface (ODD-6).

4. **Uncertainty last, but not deferred.** Slice 05 carries three open decisions (ODD-1 the `#kb-items`
   collision, ODD-5 swap survival, ODD-7 a11y) and is the one that **deliberately deletes a green test**. It is
   last because it depends on the guard and the dispatch layer being proven — but it is **in this feature**, not
   punted: without it the promise stays 5/7 kept, and `j`/`k`/`Enter` are three of the seven advertised keys.

5. **Nothing is deferred out of the feature.** The locked scope is **all seven** (D-1). Stopping after any slice
   leaves a coherent, shippable, honest subset — but only after slice 05 does the advertised-to-working ratio
   reach **7/7**, which is the actual definition of done here: *the help page tells the truth*.

## Stop-after-any-slice check (each slice is independently shippable)

| Stop after | State | Honest? |
|---|---|---|
| 01 | 2/7 work (`?`, `Esc`-for-overlay). Help is discoverable in place. | Yes — 5 keys still dead, but nothing is broken and typing is unaffected (nothing character-based is bound yet) |
| 02 | 2/7 work; guards proven. | Yes — typing demonstrably safe; the layer is provably harmless |
| 03 | 4/7 work (`?`, `Esc`, `c`). Filing is mouse-free. | Yes — the single most valuable key works safely |
| 04 | 5/7 work. Filing and finding are mouse-free. | Yes |
| 05 | **7/7 work.** The help page tells the truth. | **Yes — the promise is fully kept** |
