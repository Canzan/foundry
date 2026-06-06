# Story: keyboard-fragments-templating — the two remaining inline-format!()
# BARE htmx fragments in keyboard.rs (render_search_fragment, show_keyboard_help)
# move to Askama partials. Move-only; selector-and-substring-identical.
# JTBD: htmx-web-1 (restyle/re-word a screen without touching Rust).
#
# WHY THIS FILE IS LEAN (no "renders styled" scenarios):
#   Unlike the full-page surfaces of remaining-surfaces-templating, these are
#   BARE fragments with byte-identical markup — no base.html, no new /static
#   link, no NEW observable user delta. The move is therefore proven by the
#   EXISTING us-12-keyboard-nav suite staying green (the regression net), NOT
#   by new "renders styled" scenarios. See coverage-matrix.md for the rationale.
#
# WHAT IS GENUINELY NEW HERE (3 scenarios):
#   1. US-K01 gap — us-12 pins li.search-result[data-issue-key] + .title text,
#      but NOT the ul.search-results wrapper, the .key span, nor the empty
#      data-empty="true" state. The DESIGN render contract lists all three as
#      byte-stable markers. This scenario pins the wrapper + key + empty state
#      so the move cannot silently drop them. (Passes today; stays green after
#      the move — a regression-net tightening, not a RED.)
#   2. US-K02 gap — us-12 pins each dt[data-shortcut]+dd pair, but NOT the
#      section.keyboard-help[role="dialog"][aria-label] container nor the
#      header>h2 heading. The DESIGN render contract lists both as byte-stable
#      markers. This scenario pins the dialog container + heading. (Passes
#      today; stays green after the move — regression-net tightening.)
#   3. US-K01/K02 completion guard — a SOURCE-TREE contract (mirrors
#      remaining-surfaces US-R07 `inline_full_page_sites` + Feature B's
#      `vendored_htmx_files`). It scans keyboard.rs for the two inline-format!()
#      BARE-FRAGMENT HTML sites (the `<ul class="search-results"...>` and
#      `<section class="keyboard-help"...>` string literals) and asserts ZERO
#      remain. This is RED NOW (2 sites) and flips GREEN only when DELIVER has
#      moved both fragments into Askama partials. This is the one genuine new
#      acceptance check that drives the feature.
#
# Driving ports / adapters: inherited, no new ports (ATDD policy inherit).
#   - GET /team/{team}/project/{slug}/search?q=...  (search fragment)
#   - GET /keyboard-help                            (help overlay)
#   - source-tree scan of crates/foundry-app/src/keyboard.rs (completion guard)

@keyboard-fragments-templating @us-k01 @us-k02 @keyboard
Feature: The keyboard search and help fragments render from templates, not inline Rust
  The last two inline-format!() bare htmx fragments in foundry-app move to
  Askama partials. The move is byte-stable, so the existing us-12-keyboard-nav
  suite is the regression net. These scenarios tighten the two render-contract
  markers us-12 leaves thin and add the completion guard that proves the inline
  HTML is gone for good.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project already has issues AUTH-1 through AUTH-3
    And Mei is signed in

  # --- US-K01 gap: search-results wrapper + key + empty state ----------
  # us-12 asserts li.search-result[data-issue-key] + .title but never the
  # ul.search-results wrapper, the .key span, or the empty-state marker.

  @real-io @us-k01
  Scenario: A search match renders inside the search-results list with a key and a title
    Given the "Auth v2" project already has an issue titled "Refresh token rotation broken on Safari"
    When Mei searches "Auth v2" for the query "refresh"
    Then the search fragment is a search-results list
    And the matched result carries the issue key "AUTH-4" and a title

  @real-io @us-k01
  Scenario: A search with no matches renders the empty search-results list
    When Mei searches "Auth v2" for the query "zzz-no-such-issue"
    Then the search fragment is an empty search-results list

  # --- US-K02 gap: help dialog container + heading ----------------------
  # us-12 asserts each dt[data-shortcut]+dd pair but never the dialog
  # container nor the heading.

  @real-io @us-k02
  Scenario: The keyboard-help overlay is a labelled dialog with a heading
    When Mei requests the keyboard-help overlay
    Then the help overlay is a dialog labelled "Keyboard shortcuts"
    And the help overlay shows the heading "Keyboard shortcuts"

  # --- US-K01/K02 completion guard (RED now = 2 sites) ------------------
  # Source-tree contract; flips GREEN only when DELIVER has moved both bare
  # fragments into Askama partials.

  @source-tree @us-k01 @us-k02 @completion-check
  Scenario: No inline format!() HTML remains in the keyboard surfaces
    When the keyboard handler source is scanned for inline fragment HTML
    Then no keyboard handler emits an inline fragment HTML literal
