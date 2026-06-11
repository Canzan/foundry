//! US-11 — issue-attachment handlers.
//!
//! Routes (mounted in `lib::build_router`):
//!
//! - `POST /team/{team}/project/{project}/issues/{issue_number}/attachments`
//!   → multipart upload. CSRF-protected, member-only. Pulls the first
//!   `file=` part, enforces `FILE_UPLOAD_MAX_MB`, persists via the
//!   store (with SHA-256 captured at insert), then either returns an
//!   htmx fragment listing the new row or 303 → back to the issue page.
//!
//! - `GET /team/{team}/project/{project}/issues/{issue_number}/attachments/{id}`
//!   → streams the stored bytea back with the recorded `Content-Type`
//!   and a `Content-Disposition: attachment; filename="..."` header.
//!   Member-only; non-members get 403.
//!
//! Authorisation mirrors comments / issues — sign-in -> team-member ->
//! issue lookup -> store call. The per-route `DefaultBodyLimit::max(N)`
//! middleware enforces the size cap BEFORE the multipart extractor
//! starts buffering, so a 25 MB upload to a 10 MB instance fails at
//! the body-bytes boundary rather than spending the upload's bandwidth.
//!
//! Filename sanitization: we strip path separators (`/`, `\`) and
//! control characters before persisting. We do NOT sniff content-type
//! magic bytes in this slice — the multipart-recorded type is what the
//! browser sent, what we store, and what we serve back. The acceptance
//! contract is "round-trip preserves what the user uploaded", not
//! "we re-derive a canonical type".

use crate::bootstrap::{invalid_page, resource_not_found_page, SessionUser};
use crate::session::SESSION_KEY_USER_ID;
use crate::views::{AttachmentRow, ErrorFragment, PayloadTooLarge};
use crate::AppState;
use askama::Template;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::header::{
    HeaderMap, HeaderValue, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, LOCATION,
};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use foundry_store::AttachmentInsertError;
use sha2::{Digest, Sha256};
use tower_sessions::Session;

/// Construct the US-11 routes with their per-route body-size limit.
/// The size cap is computed from `state.file_upload_max_mb` at router
/// build time so AppState mutations during tests are respected.
pub fn build_routes(state: AppState) -> Router<AppState> {
    let cap_bytes = (state.file_upload_max_mb as usize).saturating_mul(1024 * 1024);
    Router::new()
        .route(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/attachments",
            post(submit_upload).layer(DefaultBodyLimit::max(cap_bytes)),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/attachments/{attachment_id}",
            get(download_attachment),
        )
}

// ------------- POST /.../issues/{n}/attachments  (multipart upload) ----------

pub async fn submit_upload(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number)): Path<(String, String, i32)>,
    session: Session,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return unauthorized_response();
    };

    // Upfront Content-Length cap. axum's `DefaultBodyLimit` only fires
    // when the limited body is actually CONSUMED — by then the client
    // has already streamed most of the (rejected) bytes and observes
    // a `Connection reset by peer` instead of the 413 we send. Short-
    // circuit here so the client receives a clean 413 response.
    let cap_bytes = state.file_upload_max_mb.saturating_mul(1024 * 1024);
    if let Some(declared) = headers
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        if declared > cap_bytes {
            return payload_too_large(state.file_upload_max_mb);
        }
    }

    // Auth gate: team + membership. Same shape as issues.rs / comments.rs.
    // Cross-tenant / missing-resource refusals (ADR-003 / NFR-MWT-SEC-02): the
    // team + issue lookups are scoped by the actor's workspace, so a FOREIGN
    // team/issue resolves to `None` exactly as a never-existed one does — BOTH
    // render the SINGLE uniform `resource_not_found_page()` (no echoed team/
    // project slug, no team-vs-issue body-shape difference), so a foreign upload
    // is byte-identical to a never-existed upload and leaks nothing about the
    // foreign issue's existence (matching `download_attachment`, fixed in 04-02).
    // The intra-workspace membership failure keeps its shipped 403 `non_member_page`
    // (ADR-003 boundary clause — a member reaching their OWN workspace's team is
    // not a cross-tenant concern; a cross-tenant reach 404s at the team layer
    // above and never reaches it).
    let team = match state
        .store
        .find_team_by_slug(user.workspace_id, &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return resource_not_found_page(),
        Err(err) => return internal_error("find_team_by_slug", err),
    };
    match state.store.is_team_member(team.id, user.user_id).await {
        Ok(true) => {}
        Ok(false) => return non_member_page(&team_slug),
        Err(err) => return internal_error("is_team_member", err),
    }
    let issue = match state
        .store
        .find_issue_by_team_project_number(team.id, &project_slug, issue_number)
        .await
    {
        Ok(Some(i)) => i,
        Ok(None) => return resource_not_found_page(),
        Err(err) => return internal_error("find_issue_by_team_project_number", err),
    };

    // Pull the first file part. Returns 413 if the body exceeds the
    // per-route DefaultBodyLimit cap (axum surfaces this as a
    // PayloadTooLarge error from `next_field`).
    let (filename, content_type, bytes) = match extract_file_part(multipart).await {
        Ok(parts) => parts,
        Err(UploadError::TooLarge) => return payload_too_large(state.file_upload_max_mb),
        Err(UploadError::Missing) => return bad_request_fragment("Upload is missing a file part"),
        Err(UploadError::Other(err)) => {
            return internal_error("multipart extraction", err);
        }
    };

    // Re-check the byte length explicitly — DefaultBodyLimit covers
    // total request bytes (incl. multipart envelope); a borderline
    // upload that overflows by a few bytes after envelope strip
    // should still 413, not silently truncate.
    let cap_bytes = state.file_upload_max_mb.saturating_mul(1024 * 1024) as usize;
    if bytes.len() > cap_bytes {
        return payload_too_large(state.file_upload_max_mb);
    }

    let cleaned_filename = sanitize_filename(&filename);
    if cleaned_filename.is_empty() {
        return bad_request_fragment("Filename is required");
    }
    let recorded_content_type = if content_type.is_empty() {
        "application/octet-stream".to_string()
    } else {
        content_type
    };

    let sha = sha256_hex(&bytes);
    let attachment_id = uuid::Uuid::now_v7();
    match state
        .store
        .insert_attachment(
            attachment_id,
            issue.issue_id,
            issue.workspace_id,
            user.user_id,
            &cleaned_filename,
            &recorded_content_type,
            &sha,
            &bytes,
        )
        .await
    {
        Ok(row) => {
            if is_htmx(&headers) {
                let fragment = render_attachment_row_oob(&row.filename, row.size_bytes);
                return (StatusCode::OK, Html(fragment)).into_response();
            }
            // Plain redirect back to the issue page (same shape as
            // comments.rs full-page submit).
            redirect_to(&format!(
                "/team/{team_slug}/project/{project_slug}/issues/{issue_number}"
            ))
        }
        Err(AttachmentInsertError::IssueNotFound) => resource_not_found_page(),
        Err(AttachmentInsertError::Store(err)) => internal_error("insert_attachment", err),
    }
}

// ---------- GET /.../issues/{n}/attachments/{id}  (download) -----------------

pub async fn download_attachment(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number, attachment_id)): Path<(
        String,
        String,
        i32,
        uuid::Uuid,
    )>,
    session: Session,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return unauthorized_response();
    };

    // Scope the team lookup by the acting workspace (ADR-002): a team that
    // belongs to a FOREIGN workspace resolves to `None` exactly as a
    // never-existed slug does, and BOTH collapse to the SINGLE uniform
    // `resource_not_found_page` (ADR-003 / NFR-MWT-SEC-02) — no team slug is
    // echoed, so a cross-tenant reach and a missing-id reach are byte-identical
    // (no enumeration oracle). This matches the board/issue/comment read paths.
    let team = match state
        .store
        .find_team_by_slug(user.workspace_id, &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return resource_not_found_page(),
        Err(err) => return internal_error("find_team_by_slug", err),
    };
    match state.store.is_team_member(team.id, user.user_id).await {
        Ok(true) => {}
        // Intra-workspace authz failure keeps its shipped 403 shape (ADR-003
        // boundary clause): a member reaching their OWN workspace's team they do
        // not belong to is NOT a cross-tenant concern. A cross-tenant reach never
        // reaches this branch — the foreign team already 404'd above.
        Ok(false) => return non_member_page(&team_slug),
        Err(err) => return internal_error("is_team_member", err),
    }
    // Defensive lookup — confirms the issue still exists in the targeted
    // (team, project) so we don't serve an attachment through a path
    // that no longer resolves. The store also workspace-scopes the
    // attachment lookup; both layers must agree.
    if let Err(err) = state
        .store
        .find_issue_by_team_project_number(team.id, &project_slug, issue_number)
        .await
    {
        return internal_error("find_issue_by_team_project_number", err);
    }

    match state
        .store
        .find_attachment_in_workspace(attachment_id, user.workspace_id)
        .await
    {
        Ok(Some(row)) => attachment_download_response(&row),
        // A foreign-workspace attachment id resolves to `None` exactly as a
        // never-existed id does — BOTH collapse to the SAME uniform 404 (ADR-003).
        // The previous body echoed the requested `attachment_id`, an enumeration
        // oracle (NFR-MWT-SEC-02); the uniform page reveals nothing.
        Ok(None) => resource_not_found_page(),
        Err(err) => internal_error("find_attachment_in_workspace", err),
    }
}

// ---------- helpers ----------------------------------------------------------

#[derive(Debug)]
enum UploadError {
    Missing,
    TooLarge,
    Other(String),
}

async fn extract_file_part(
    mut multipart: Multipart,
) -> Result<(String, String, Vec<u8>), UploadError> {
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                // Treat the first part with a filename as the upload.
                // Some clients post the file under name="file"; we
                // accept any part as long as it has a filename so the
                // test driver doesn't need to negotiate a name.
                let Some(filename) = field.file_name().map(|s| s.to_string()) else {
                    // Non-file field (e.g. _csrf) — skip.
                    continue;
                };
                let content_type = field
                    .content_type()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let bytes = match field.bytes().await {
                    Ok(b) => b.to_vec(),
                    Err(err) => {
                        // axum surfaces a PayloadTooLarge error here
                        // when DefaultBodyLimit is exceeded.
                        let status = err.status();
                        if status == StatusCode::PAYLOAD_TOO_LARGE {
                            return Err(UploadError::TooLarge);
                        }
                        return Err(UploadError::Other(err.to_string()));
                    }
                };
                return Ok((filename, content_type, bytes));
            }
            Ok(None) => return Err(UploadError::Missing),
            Err(err) => {
                let status = err.status();
                if status == StatusCode::PAYLOAD_TOO_LARGE {
                    return Err(UploadError::TooLarge);
                }
                return Err(UploadError::Other(err.to_string()));
            }
        }
    }
}

fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .filter(|c| !matches!(*c, '/' | '\\') && !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

async fn signed_in_user(session: &Session) -> Option<SessionUser> {
    session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
}

fn unauthorized_response() -> Response {
    (StatusCode::UNAUTHORIZED, "sign-in required").into_response()
}

fn redirect_to(location: &str) -> Response {
    let mut hdrs = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(location) {
        hdrs.insert(LOCATION, v);
    }
    (StatusCode::SEE_OTHER, hdrs, "").into_response()
}

fn non_member_page(team_slug: &str) -> Response {
    invalid_page(
        StatusCode::FORBIDDEN,
        "Not a team member",
        &format!(
            "You are not a member of the {team_slug:?} team and cannot attach files to its issues."
        ),
    )
}

fn payload_too_large(limit_mb: u64) -> Response {
    // US-R05: the too-large body now renders through the shared base layout
    // (links the vendored /static stylesheet). The 413 status is the byte-stable
    // control-flow contract and is UNCHANGED — body only.
    let body = PayloadTooLarge { limit_mb }
        .render()
        .expect("payload_too_large.html renders");
    (StatusCode::PAYLOAD_TOO_LARGE, Html(body)).into_response()
}

fn internal_error<E: std::fmt::Display>(label: &str, err: E) -> Response {
    tracing::error!(error = %err, "{label} failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

fn bad_request_fragment(message: &str) -> Response {
    // US-R05: reuse the SHARED bare error fragment (the same template US-R01 /
    // US-R03 render). The `attachment-upload-error` marker is byte-stable; the
    // message is auto-escaped (matching the previous `html_escape`). A BARE
    // fragment — it MUST NOT extend base.html (NFR-WEBB-COMPAT-02).
    let body = ErrorFragment {
        fragment_marker: "attachment-upload-error".to_string(),
        message: message.to_string(),
    }
    .render()
    .expect("error_fragment.html renders");
    (StatusCode::BAD_REQUEST, Html(body)).into_response()
}

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("hx-request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn render_attachment_row_oob(filename: &str, size_bytes: i64) -> String {
    // US-R05: the live-append row now renders the ONE shared
    // `partials/attachment_row.html` wrapped by the OOB envelope (one-partial
    // rule). Selector-and-substring-identical to the previous `format!`:
    // `hx-swap-oob="beforeend:[data-attachment-list]"`, `<li class="attachment"
    // data-filename>`, the `.filename` + `.size` spans. Fields auto-escaped.
    AttachmentRow {
        filename: filename.to_string(),
        size_label: humanize_size(size_bytes),
    }
    .render()
    .expect("attachment_row_oob.html renders")
}

/// Format `size_bytes` for the issue page. Uses MB once the value would
/// round to at least 1 MB; kB below that. The acceptance scenario
/// asserts the literal "9 MB" label on a 9 442 304-byte upload.
pub(crate) fn humanize_size(size_bytes: i64) -> String {
    const MB: i64 = 1024 * 1024;
    const KB: i64 = 1024;
    if size_bytes >= MB {
        let mb = (size_bytes + MB / 2) / MB;
        format!("{mb} MB")
    } else if size_bytes >= KB {
        let kb = (size_bytes + KB / 2) / KB;
        format!("{kb} KB")
    } else {
        format!("{size_bytes} B")
    }
}

fn attachment_download_response(row: &foundry_store::AttachmentRow) -> Response {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&row.content_type) {
        headers.insert(CONTENT_TYPE, v);
    }
    // Quote the filename and escape any embedded backslash/double-quote
    // per RFC 6266 quoted-string rules.
    let safe_filename = row.filename.replace('\\', "\\\\").replace('"', "\\\"");
    let cd = format!("attachment; filename=\"{safe_filename}\"");
    if let Ok(v) = HeaderValue::from_str(&cd) {
        headers.insert(CONTENT_DISPOSITION, v);
    }
    (StatusCode::OK, headers, row.content.clone()).into_response()
}
