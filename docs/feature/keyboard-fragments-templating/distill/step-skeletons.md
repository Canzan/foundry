# DISTILL Step Skeletons — keyboard-fragments-templating (DELIVER wiring)

> What DELIVER must wire to flip the completion guard GREEN. The two gap
> scenarios + the us-12 regression net are already GREEN and need NO production
> change — they exist to prove the move preserves the markup. The ONLY RED is
> the completion guard, which goes GREEN when both inline literals leave the
> Rust source.

## Acceptance artifacts created (this wave)

| Artifact | Path |
|---|---|
| Feature file | `crates/foundry-acceptance/tests/features/us-k01-keyboard-templating.feature` |
| Step module | `crates/foundry-acceptance/src/steps/keyboard_fragments_templating.rs` |
| Module registration | `crates/foundry-acceptance/src/lib.rs` (`pub mod keyboard_fragments_templating;`) |
| Force-link | `crates/foundry-acceptance/tests/acceptance.rs` (`use ... as _keyboard_fragments;`) |

No `keyboard.rs` production change is made by DISTILL (DELIVER owns the move).

## How the completion guard is wired

`keyboard_fragments_templating::inline_html_fragment_sites()` is the **sibling**
of `feature_remaining_surfaces::inline_full_page_sites()`:

- `inline_full_page_sites()` matches `<!doctype` → FULL pages only. Bare
  fragments have no `<head>`, so it is **blind** to the keyboard fragments. We
  did NOT generalize that function (it would conflate the full-page north-star
  KPI with the fragment move); instead we added a **focused sibling** scoped to
  `keyboard.rs` with the two bare-fragment opening tells.
- `inline_html_fragment_sites()` matches the byte-stable bare-fragment tells the
  DESIGN render contract names:
  - `<ul class="search-results"`  (catches both the populated and the
    `data-empty="true"` literals in `render_search_fragment`)
  - `<section class="keyboard-help"`  (catches `show_keyboard_help`)

RED now → 3 sites: `keyboard.rs:232`, `:245`, `:265`. GREEN when 0.

## DELIVER wiring (the move — 2 templates + 2 view-models + render swap)

Per DESIGN surface→template map (inherit Feature B `views.rs` pattern):

### 1. `partials/search_results.html` (BARE — no `{% extends "base.html" %}`)

View-model `SearchResults { items: Vec<SearchResultRow>, empty: bool }` (or
render the empty case in-template via `items.is_empty()`). `SearchResultRow { key, title }`
where `key = "{prefix}-{n}"`. Askama auto-escapes `key`/`title` (replaces the
manual `html_escape`). Must emit, byte-stable:

- populated: `<ul class="search-results">` containing per item
  `<li class="search-result" data-issue-key="{key}"><span class="key">{key}</span> <span class="title">{title}</span></li>`
- empty: `<ul class="search-results" data-empty="true"></ul>`

### 2. `partials/keyboard_help.html` (BARE / overlay — no `base.html`)

View-model `KeyboardHelp { entries: Vec<ShortcutEntry> }`, `ShortcutEntry { key, label }`.
Must emit, byte-stable:

```
<section class="keyboard-help" role="dialog" aria-label="Keyboard shortcuts">
  <header><h2>Keyboard shortcuts</h2></header>
  <dl>{ per entry: <dt data-shortcut="{key}">{key}</dt><dd>{label}</dd> }</dl>
</section>
```

### 3. Swap the render calls in `keyboard.rs`

- `render_search_fragment(...)` → build `SearchResults`/rows, render the Askama
  template, return its `String`. The inline `format!(r#"<ul class="search-results">..."#)`
  and the empty-state literal both disappear from the source.
- `show_keyboard_help()` → build `KeyboardHelp` from `SHORTCUTS`, render the
  Askama template. The inline `format!(r#"<section class="keyboard-help"..."#)`
  literal disappears.

When all three literals are gone, `inline_html_fragment_sites()` returns empty
→ the completion-guard scenario flips GREEN. The us-12 net + the two gap
scenarios must STAY green (byte-identical markup), which is the move's correctness gate.

## Gates DELIVER must honour

- Pre-commit hooks (style) unaffected.
- No new crate/dep (Askama already wired in `Cargo.toml`/`Cargo.lock`).
- HTML-escaping preserved by Askama auto-escape (drop the manual `html_escape`
  calls for the moved fields only).
- One binary; no JS toolchain.
