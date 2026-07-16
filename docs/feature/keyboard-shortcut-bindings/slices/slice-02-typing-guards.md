# Slice 02 — The guards: typing is never captured, chords are never hijacked

**Goal**: Mei types `"cache invalidation on login"` into a title field and gets **exactly that string**. She
types `"cjk/?"` and gets `"cjk/?"`. She presses `Cmd+C` and it **copies**. The shortcut layer wakes up the
instant she leaves the field.
**Story**: US-02.

> **Why this ships BEFORE `c` (slice 03), despite `c` scoring higher.** Every one of the seven shortcuts is a
> plain printable character or bare key — *exactly what people type*. Binding them without this guard makes text
> entry **impossible** (Mei cannot type the letter `c` into a title). That is not "less value", it is **negative
> value**: a keystroke-eating layer is **strictly worse than shipping nothing**, and it is the **default outcome
> of the obvious implementation** (bind `keydown`, switch on `key`). We deliberately order the risk before the
> reward so the worse-than-nothing state is never shipped, not even briefly. Slice 01 was safe to ship first
> because it binds **no character key** (`?` is `Shift+/`, `Esc` is not typed into fields).

**IN scope**
- The **text-input guard** (FR-2, NFR-2): no shortcut fires while the event target is a text-entry context.
  Exact predicate — `input` / `textarea` / `contenteditable` / `select` / `role="textbox"` / **IME
  `isComposing`** — is **ODD-4**. (`isComposing` is concrete, not theoretical: Mei uses a Japanese IME.)
- The **modifier guard** (FR-3, NFR-3): `Ctrl` / `Cmd`(Meta) / `Alt` held ⇒ inert. `Cmd+C` copies.
- `Shift` is **explicitly not a suppressor** — `?` is `Shift+/` (BR-7).
- **Structural placement**: the guard chain is evaluated **once, before dispatch**, for **all seven** shortcuts
  with **no exemptions** (BR-2) — not seven scattered `if`s (which is how this fails).
- The guard is **contextual, not a global toggle**: leaving the field re-enables shortcuts immediately.
- The **`@property` litmus** that reds on any regression (NFR-2) — the single most important test in the feature.

**OUT of scope**: binding any new shortcut (this slice adds **capability zero** and that is correct — its whole
user-visible value is *the absence of a harm*); `c` (slice 03); `/` (slice 04); `j`/`k`/`Enter` (slice 05).

**Learning hypothesis**: disproves *"a single guard predicate can reliably distinguish **typing** from
**commanding** across every text-entry context Foundry actually renders (the new-issue title/description, the
issue edit modal, comment fields, the search box) and across IME composition, without needing per-shortcut
exceptions"* — if `contenteditable` or `isComposing` behaves inconsistently across browsers, if a shipped field
type escapes the predicate, or if any of the seven turns out to need a bespoke carve-out (which would mean the
guard is not structural and BR-2 is wrong).

> **If this hypothesis is disproven**, the honest response is **not** to ship carve-outs: it is to **unbind the
> character keys and shrink the advertised list** in `SHORTCUTS` so the help page stays truthful (BR-1). Shipping
> a layer that sometimes eats keystrokes is the one outcome worse than the status quo.

**Seams**: the dispatch layer from slice 01; `SHORTCUTS` (`keyboard.rs:48-56`); the real text-entry surfaces the
guard must cover — `partials/new_issue_modal.html:4` (`input[name=title][autofocus]`),
`partials/issue_edit_modal.html:4`, `partials/comment_edit_form.html:1`, and the search box (slice 04).
**Dependencies**: slice 01 (the dispatch layer it guards). DESIGN **ODD-4** (the exact predicate — **blocking**).
**ODD-9** (browser driver — the `@property` litmus is unwritable without it).
**Effort**: ~1 day (the predicate is small; proving it across every real input surface + IME is the work).
**KPI**: KPI-2 — **0** captured keystrokes, **0** hijacked chords (hard guardrail). KPI-1 unchanged at 2/7 —
**deliberately**: this slice buys safety, not capability.
