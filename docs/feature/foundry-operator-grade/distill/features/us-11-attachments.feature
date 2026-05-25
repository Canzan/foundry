# Story: US-11 — User attaches a file to an issue
# Slice: 3 (operator-grade — bytea-in-Postgres is what makes US-03's
#   single-pg_dump backup story whole)
# JTBD: outcome-2 (Data sovereignty — attachments in bytea, no S3)
#
# Driving port: HTTP POST multipart/form-data to
# /team/{team}/project/{project}/issues/{key}/attachments via reqwest's
# multipart feature. Download via HTTP GET to
# /team/{team}/project/{project}/issues/{key}/attachments/{attachment_id}.
# Slice 1 already wires up tower-sessions + the issue routes; slice 3
# adds the attachment endpoints + `issue_attachments` bytea table
# (migration 0004 per data-access.md).
#
# Driven adapters exercised (Strategy C — all real):
#   - real reqwest multipart upload -> real axum multipart extractor
#   - real `issue_attachments` row insertion via sqlx (bytea content)
#   - real bytea download streaming back through axum response body
#   - real Content-Type sniffing via the `mime` or `infer` crate
#     (filename + magic-bytes; the production wave picks the crate)
#   - sha256 invariant computed against the inserted bytea round-trip
#
# NFR coverage: NFR-PERF-02 (10 MB default cap, 50 MB max recommended),
# NFR-SEC-04 (CSRF on multipart POST), NFR-SEC-06 (non-member 403),
# NFR-DATA-01 (bytea storage — attachments included in US-03's pg_dump).
#
# Gherkin discipline (CM-B): scenarios talk in member language ("attaches",
# "downloads", "is refused"). Tooling words (multipart, bytea, sha256) live
# in step-method bodies and the comment block above, NOT in the steps a
# stakeholder reads. HTTP status numbers DO appear in @error scenarios
# because the status IS the user-facing contract there (the form-submitter
# sees a "rejected" response keyed off the status).

@slice3 @us-11 @attachments
Feature: A member attaches a file to an issue and other members download it byte-identically; oversize files are refused with a clear error
  A signed-in team member uploads a file to an issue they can access.
  The server stores the file in the database (single-backup property
  per NFR-DATA-01), capped by the FILE_UPLOAD_MAX_MB env var (default
  10 MB per NFR-PERF-02). Any team member can download the attachment
  back; the download preserves filename, content-type, and byte-for-byte
  contents. Non-members are refused. Files above the cap are rejected
  with a clear over-limit response naming the configured limit; no
  attachment is created. Deleting the parent issue removes its
  attachments too.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a member "hiroshi@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project already has issue AUTH-1
    And the FILE_UPLOAD_MAX_MB env var is set to 10 for this scenario
    And Mei is signed in

  @walking_skeleton @real-io @driving_adapter
  Scenario: A member attaches a screenshot to an issue and a teammate downloads it byte-identically
    When Mei attaches a 256-kilobyte image named "screenshot.png" with content-type "image/png" to "AUTH-1"
    Then the attachment is listed on the AUTH-1 issue page with filename "screenshot.png"
    And the upload is accepted
    When Hiroshi downloads the attachment "screenshot.png" from "AUTH-1"
    Then the downloaded file is byte-identical to the file Mei uploaded
    And the Content-Disposition response header names the file as "screenshot.png"
    And the Content-Type response header is "image/png"

  @real-io
  Scenario: A 9-megabyte attachment under the configured cap uploads successfully
    When Mei attaches a 9-megabyte PDF named "debug-log.pdf" with content-type "application/pdf" to "AUTH-1"
    Then the upload is accepted
    And the attachment is listed on the AUTH-1 issue page with filename "debug-log.pdf" and size "9 MB"
    When Hiroshi downloads the attachment "debug-log.pdf" from "AUTH-1"
    Then the downloaded file is byte-identical to the file Mei uploaded

  @real-io @error @nfr-perf-02
  Scenario: An attachment above the configured cap is refused with an over-limit response and no attachment is created
    When Mei attempts to attach a 25-megabyte file named "huge-video.mov" with content-type "video/quicktime" to "AUTH-1"
    Then the upload is refused with an over-limit (HTTP 413) response
    And the response body mentions the configured limit of 10 megabytes
    And the AUTH-1 issue page lists no attachments

  @real-io @error @nfr-sec-06
  Scenario: A workspace member outside the team cannot attach files to that team's issues
    Given a member "rita@partners.acme.com" belongs to the team "Partners"
    And Rita is signed in
    When Rita attempts to attach a 100-kilobyte file named "leak.txt" with content-type "text/plain" to "AUTH-1"
    Then the upload is refused as forbidden (HTTP 403)
    And the AUTH-1 issue page lists no attachments

  @real-io @error @nfr-sec-06
  Scenario: A workspace member outside the team cannot download attachments from that team's issues
    Given a member "rita@partners.acme.com" belongs to the team "Partners"
    And Mei has attached a 256-kilobyte image named "screenshot.png" to "AUTH-1"
    And Rita is signed in
    When Rita attempts to download the attachment "screenshot.png" from "AUTH-1"
    Then the download is refused as forbidden (HTTP 403)

  @real-io @error
  Scenario: An unauthenticated request to upload an attachment is refused
    Given Mei is signed out
    When an anonymous request attempts to attach a 100-kilobyte file named "anon.txt" with content-type "text/plain" to "AUTH-1"
    Then the upload is refused as unauthenticated (HTTP 401)
    And the AUTH-1 issue page lists no attachments

  @real-io
  Scenario: Deleting the parent issue removes its attachments too
    Given Mei has attached a 256-kilobyte image named "screenshot.png" to "AUTH-1"
    And Mei has attached a 100-kilobyte text file named "notes.txt" to "AUTH-1"
    When the operator deletes the issue "AUTH-1"
    Then no attachments exist for "AUTH-1"
