# Story: US-B02 — Render the board from vendored assets so it looks like a product
#         (folds US-B06 — the static-asset pipeline scaffolding)
# Feature B "htmx Web Tier" — Slice 1 (Walking Skeleton)
# JTBD: htmx-web-2 (the self-hoster's first screen looks finished, offline)
#
# Driving adapter: the static-asset route served by foundry-app via
# tower_http::ServeDir mounted at /static — see design/assets.md (ServeDir,
# pure pre-vendored blobs under static/vendor + static/css) and
# design/architecture.md (the /static .nest_service in build_router).
# Driven adapters exercised: real filesystem (the committed blobs under
# crates/foundry-app/static/) read by ServeDir — a genuine @real-io adapter.
#
# RED contract: static/ is EMPTY today and build_router mounts NO /static
# route, so every asset GET 404s — genuine MISSING_FUNCTIONALITY RED. DELIVER
# vendors the pinned htmx/Alpine/CSS blobs into static/ and adds the ServeDir
# route. The path-traversal-refusal scenario is satisfied by ServeDir by
# construction once mounted. See docs/feature/htmx-web-tier/distill/step-skeletons.md.
#
# Per Mandate 6 + Mandate 9/11: the static-serving driven adapter has its own
# @real-io @adapter-integration scenario; layer-3 sad paths (missing asset,
# traversal) are enumerated example-based, NOT property-generated.

@feature-b @us-b02 @slice1 @driving_adapter @acme
Feature: The binary serves vendored assets so the board looks like a product offline
  The htmx, Alpine, and Foundry stylesheet that the board needs are shipped
  inside the binary under its own static path and served by the binary itself —
  no CDN, no external fetch — so the board reads as a finished product even on a
  host with no internet access. A referenced asset that is missing is refused,
  and the static route never serves a file outside its own directory.

  @walking_skeleton @real-io @adapter-integration
  Scenario: The vendored htmx, Alpine, and stylesheet are served by the binary
    Given the foundry binary is running
    When a browser requests the vendored htmx script from the static path
    Then the response is delivered successfully with a JavaScript content type
    And the response carries a long-lived cache header
    When a browser requests the vendored Alpine script from the static path
    Then the response is delivered successfully with a JavaScript content type
    When a browser requests the vendored Foundry stylesheet from the static path
    Then the response is delivered successfully with a stylesheet content type

  @real-io
  Scenario: The vendored htmx asset is a real, non-empty script
    Given the foundry binary is running
    When a browser requests the vendored htmx script from the static path
    Then the response body is a non-empty script

  @error @real-io
  Scenario: A request for an asset that is not vendored is refused
    Given the foundry binary is running
    When a browser requests a stylesheet that was never vendored
    Then the request is refused as not found

  @error @real-io @nfr-sec-06
  Scenario: The static route refuses to serve a file outside its own directory
    Given the foundry binary is running
    When a browser tries to reach a file outside the static directory through the static path
    Then the request is refused and no file outside the static directory is served
