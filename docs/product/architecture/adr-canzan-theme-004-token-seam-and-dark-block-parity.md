# ADR-CANZAN-THEME-004: Colour enters the stylesheet at one seam, and the two duplicated dark blocks are kept honest by a structural check

## Status

Accepted (canzan-theme-system DESIGN wave, 2026-08-29)

## Context

DISCUSS D-03 requires the dark palette to be written as **two blocks that are
never merged**, because a media query and an attribute selector cannot be
combined into one rule meaning "either":

```css
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { … } }
:root[data-theme="dark"] { … }
```

This is correct and unavoidable — but it is *deliberate duplication of a
value-binding block*, which is a maintenance hazard with a specific, nasty
failure mode. Add a token to `:root` and to `:root[data-theme="dark"]` but forget
the media block, and dark-by-toggle works perfectly while dark-by-device silently
renders one surface in the light value. It is invisible to whoever added the
token (they will test with the toggle, which is the visible control), and visible
only to system-dark operators — **exactly the persona the feature exists for**
(`env-night-triage`).

DISCUSS also states, as a System Constraint, that after this feature *"no rule
beneath the token block writes a colour"*. It names no enforcer. The DoD says
this is "provable by grep" — which is precisely the mechanism that was available
for the last 43 features and did not prevent:

- **46 colour literals across 30 rules** sitting outside `:root`, with the rail
  (`foundry.8ce38566.css:438-518`), the dashboard block (`:205-294`), the modal
  backdrop and the keyboard-help overlay (`:524-565`) using **zero** tokens;
- **three competing accent hues** — `--accent` `#2452c9` (`:18`), rail indigo
  `#5b5bd6`/`#ecedff`/`#3a3ad1` (`:462,491,492`), card-key indigo
  `#4f46e5`/`#eef2ff` (`:271,272`);
- `.site-header` (`:56-69`) surviving 43 features as dead CSS with no markup
  behind it.

None of that was a failure of care. It was the absence of a watcher. This
feature's central architectural claim — that a palette is a *re-binding of names*
rather than a second stylesheet to keep in sync — is exactly as durable as
whatever enforces it, and today that is nothing. Principle 11: an architecture
rule without enforcement erodes.

Constraint on the enforcer: no Node, no bundler, no build step
(`VENDOR.md:3-4`), which rules out the entire mainstream CSS-linting ecosystem.

## Decision

**One seam.** Colour values appear in exactly three regions of
`foundry.<hash>.css` — the `:root` token block and the two dark blocks — and
nowhere else. Every other rule names a `--cz-*` token. foundry's own colour
tokens (`--bg`, `--fg`, `--muted`, `--border`, `--surface`, `--accent`,
`--accent-contrast`, `--danger`) are retired rather than aliased (D-02), so there
is one live name per colour.

**Enforced by a second `check-arch` rule**, `check_stylesheet_token_seam`,
alongside the asset rule of ADR-CANZAN-THEME-003 and for the same reasons (it
inherits `--root`, the `ci` gate-3 and `smoke` wiring, and the injected-violation
gold-test pattern). Two assertions:

**S1 — no colour beneath the seam.** After stripping `/* … */` comments, no
`#rgb` / `#rrggbb` / `#rrggbbaa` literal and no `rgb(` / `rgba(` / `hsl(` /
`hsla(` functional notation appears outside the three token regions. The regions
are located by their selector text and delimited by brace matching.

Comment stripping is not optional: D-04 requires measured contrast ratios
recorded *inline beside the tokens*, so the file will be dense with comments, and
a scanner that ignores them would produce false positives on the very discipline
this feature introduces.

**Two existing helpers are close but must NOT be reused verbatim**, and the
reason is recorded here so it is not rediscovered as a bug:

- `strip_comment` (`check_arch.rs:839-844`) strips `//` to end-of-line only. CSS
  needs `/* … */`, which **spans lines**. A new stripper is required; the `//`
  one would leave every block comment intact.
- `block_end` (`check_arch.rs:700-722`) is the right brace-matching *idiom*, but
  it carries a Rust-specific escape hatch — `if !started && offset > 3` — for a
  `#[cfg(test)]` attribute on a non-block item. That heuristic is meaningless for
  CSS and would silently truncate a region whose selector list runs more than
  three lines before its `{`. Follow the idiom; do not call the function.

**S2 — the three blocks declare the identical token set.** Collect the set of
**colour**-token names declared in each of the three regions and assert all three
sets are equal. Report the difference by name and by region. This is the
assertion that makes D-03's mandated duplication safe: the blocks may — must —
differ in *values*, and may never differ in *names*.

> **Amendment (2026-08-29, DELIVER roadmap).** As first written this said "the
> set of `--*` custom-property names", which contradicts `component-boundaries.md`
> C1: C1 gives `:root` **sole** ownership of `--radius`, `--cz-gutter` and the
> three type tokens, and the dark regions have no reason to re-declare them.
> Taken literally, the two documents could not both be satisfied.
>
> S2 is therefore scoped to the **colour-token subset** — names whose `:root`
> binding matches S1's own colour-literal detector. The rationale is that S2's
> entire failure mode is a *colour* failure: a token missing from one block,
> revealed only to operators on that one path. Forcing the dark blocks to
> re-declare `--radius: 6px` verbatim would be duplication with no failure mode
> behind it, and duplication that S1 does not protect. The rule's doc comment
> carries the same scoping so it is not rediscovered as a bug.

**Enforcement annotation for DELIVER**

```
Style: modular monolith, ports-and-adapters (unchanged; this feature adds no crate)
Presentation sub-rule: single token seam in the served stylesheet
Language: Rust (guard) over CSS (subject)
Tool: cargo xtask check-arch — house AST/source-walk scanner (no Node, DB6)

Rules to enforce:
- S1  no colour literal outside :root / the two dark blocks
- S2  the three token regions declare an identical set of custom-property names
- R1  every /static/... reference resolves on disk        (ADR-CANZAN-THEME-003)
- R2  every content-hashed filename matches its own sha256 (ADR-CANZAN-THEME-003)
- R3  every VENDOR.md row's recorded sha256 recomputes      (ADR-CANZAN-THEME-003)
```

**Gold test.** Same lane and same pattern as ADR-CANZAN-THEME-003: **`xtask` unit
tests** in the `#[cfg(test)]` module beside `check_arch.rs`, using the existing
`tempfile` dev-dependency (`xtask/Cargo.toml:19-21`) to stage a fixture tree and
calling the rule function directly. Plant `color: #ff0000` in a rule below the
seam and assert S1 fires naming the file and line; delete one token from the media
block only and assert S2 fires naming the token and the region; assert both stay
silent on a clean tree. Without these the rules are claims, not guards — and they
are a DELIVER obligation, since DISTILL Decision 4 correctly places infrastructure
verification outside acceptance scope.

## Consequences

- Positive: the feature's central invariant survives the feature. A future author
  adding a rule cannot reintroduce a literal, and cannot half-add a token to one
  dark block — which is the one defect duplication makes likely and review makes
  hard to catch.
- Positive: S1 makes the "three accent hues" and "46 literals" regressions
  structurally unrepeatable, and makes the DoD line "zero colour literals outside
  the token block, provable by grep" a gate rather than a habit.
- Positive: this is a *different question* from the acceptance sweep scenarios
  (US-CTS-01 S6, US-CTS-02 S5), which measure rendered surfaces in a browser. The
  sweep catches a light rectangle an operator can see; S1/S2 catch a literal in a
  rule nothing has rendered yet, and S2 catches a token missing from a block that
  only system-dark operators would ever reveal. Neither subsumes the other —
  `boundary-guard.md:81-83`'s orthogonal-layers argument, applied to the
  stylesheet.
- Negative: the rule is a scanner, not a CSS parser. Three limits, stated rather
  than discovered: (a) **CSS named colours** (`white`, `red`, `rebeccapurple`)
  are not detected — S1 covers hex and functional notation only. `transparent`,
  `currentColor` and `inherit` are intentionally permitted; a blocklist of the
  ~148 named colours would be a hand-maintained list, which is the staleness this
  design avoids elsewhere. The hole is narrow (nobody writes `color: teal` while
  deliberately re-authoring onto a token contract) and it is covered downstream by
  the rendered sweep. (b) Brace matching is textual and would be confused by a
  brace inside a string literal or `content:` value; the file contains none and
  the rule fails loudly rather than silently if that changes. (c) The rule
  hardcodes the three selector strings, so changing a selector requires changing
  the rule — acceptable, since changing one of those selectors *is* changing the
  theming mechanism and should not be a quiet edit.
- Negative: two more rules on the pre-commit `smoke` path. Both are single-file
  scans of a ~700-line stylesheet; cost is immaterial.
- Not covered: **D-05's opaque-surface rule** ("no text-bearing surface takes a
  translucent jade tint without an opaque background beneath"). This is not
  statically decidable — it requires knowing which elements bear text, which is a
  rendered fact. It stays a review-and-acceptance obligation (US-CTS-02's
  `.card__key` / `.sidebar__item--active` criteria), and this ADR records that it
  is deliberately unenforced rather than leaving the gap to be discovered.
- Follow-up, named: a **weight-range check** (display 600/700, body 400–700, mono
  400/500 per ADR-CANZAN-THEME-001) belongs in this same parser, which already
  walks declarations. Deferred only because the shipped weight set is not final
  until DELIVER measures the blobs.

## Alternatives considered

- **A. Manual grep in the DoD, as feature-delta currently specifies** — Rejected.
  It is the status quo and it costs nothing, which is its whole case. It is also
  the exact mechanism under which 46 literals, three accent hues and one dead
  rule block accumulated across 43 features. A check that runs when someone
  remembers is not a check; the evidence for that is in the file this feature is
  rewriting.
- **B. `stylelint` with `declaration-property-value-disallowed-list`** — Rejected.
  It is the correct tool for the job in any other project: a real CSS parser, no
  false positives on comments or braces, named colours handled, and rules for
  exactly this. It requires Node and a `package.json`, which `VENDOR.md:3-4` and
  assets.md DB6 forbid permanently and deliberately. Not a close call, but worth
  recording that the constraint — not the tool — decided it.
- **C. Rely on the acceptance sweep scenarios alone (US-CTS-01 S6, US-CTS-02 S5)** —
  Rejected as *sufficient*, retained as *complementary*. The sweeps run in the
  `@needs-browser` lane behind Docker (`xtask/src/main.rs:241-252`), so a
  violation survives the entire fast loop; they can only observe surfaces a
  scenario actually navigates to; and they are blind to S2's failure mode
  entirely unless a test happens to run under a system-dark preference *and*
  visit the surface whose token went missing. They answer "does the rendered page
  look right?", which is the right question and not this one.
- **D. Merge the two dark blocks and drive both from a single custom-property
  indirection** — Rejected, and it is the alternative that would make S2
  unnecessary by removing the duplication. It cannot be done: `@media` and an
  attribute selector cannot express "either" in CSS, and every workaround (a
  `light-dark()` call, a JS-written attribute on every load, `color-scheme` alone)
  either drops the three-state semantics D-06 requires, drops browser support, or
  reintroduces the flash the render-blocking script exists to prevent. The
  duplication is a property of the language; the parity check is the response to
  it.
- **E. Put the check in a `#[test]` in `foundry-app` rather than `check-arch`** —
  Rejected for the same decisive reason as ADR-CANZAN-THEME-003 alternative A: it
  could not be pointed at a planted-violation tree, so the guard could never be
  shown to bite.
