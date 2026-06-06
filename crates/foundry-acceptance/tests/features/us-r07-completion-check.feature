# Story: Completion check — no bare-<head> inline format! full page remains
# Feature "Remaining-Surfaces Templating" — north-star KPI guard
# JTBD: htmx-web-1/2 (the cut is finished — every full-page surface is templated)
#
# Driving adapter: NONE — this is a SOURCE-TREE contract, mirroring Feature B's
# `vendored_htmx_files()` on-disk count check (feature_b_web_tier.rs:1003). It
# scans the foundry-app handler sources for the tell of an inline bare-<head>
# full page: a Rust string literal that opens an HTML document (`<!doctype` with
# a `<head>` or `<html><body>`) emitted from a handler instead of an Askama
# template. The north-star KPI from stories.md US-R06 is "0 bare-<head> format!()
# full pages remaining in foundry-app".
#
# RED contract: today MULTIPLE such sites exist (signin.rs::dashboard_root,
# keyboard.rs::render_modal_full_page, events.rs::unauthorized_response,
# attachments.rs::payload_too_large, bootstrap.rs::{dashboard,render_claim_form,
# create_invite,invalid_page}), so the count is > 0 and this scenario fails RED
# for MISSING_FUNCTIONALITY. It flips GREEN only when DELIVER has moved every
# full-page surface into a template extending base.html — i.e. the feature is
# feature-complete. It is deliberately the LAST guard to go green.
#
# Pragmatic scope: this guard counts FULL-PAGE bare-<head> sites only. BARE
# fragments (modals, error divs, OOB rows, the state <span>) legitimately stay
# inline-renderable-or-templated as bare fragments and are NOT counted — the
# fragment-vs-full-page rule (render-contract.md) means a fragment has no <head>
# to begin with, so the <!doctype>/<head> tell already excludes them.

@remaining-surfaces @us-r07 @completion-check @source-tree
Feature: No unstyled inline full page remains in the web tier
  Once every remaining surface is templated, no foundry-app handler emits an
  inline bare-head HTML document any more — every full page extends the shared
  base layout. This guard fails until the cut is complete and is the single
  proof that the north-star "zero inline format! HTML pages left" goal is met.

  Scenario: Every full-page web surface renders from a template, not an inline document
    When the foundry-app handler sources are scanned for inline full-page HTML documents
    Then no handler emits a bare-head inline HTML document
