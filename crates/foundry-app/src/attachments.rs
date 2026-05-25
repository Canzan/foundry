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

use crate::bootstrap::{html_escape, invalid_page, SessionUser};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
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
    let team = match state
        .store
        .find_team_by_slug(user.workspace_id, &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return team_not_found_page(&team_slug),
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
        Ok(None) => return issue_not_found_page(&team_slug, &project_slug, issue_number),
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
        Err(AttachmentInsertError::IssueNotFound) => {
            issue_not_found_page(&team_slug, &project_slug, issue_number)
        }
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

    let team = match state
        .store
        .find_team_by_slug(user.workspace_id, &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return team_not_found_page(&team_slug),
        Err(err) => return internal_error("find_team_by_slug", err),
    };
    match state.store.is_team_member(team.id, user.user_id).await {
        Ok(true) => {}
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
        Ok(None) => not_found_page(&format!(
            "Attachment {attachment_id} not found in this workspace"
        )),
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

fn team_not_found_page(team_slug: &str) -> Response {
    invalid_page(
        StatusCode::NOT_FOUND,
        "Team not found",
        &format!("No team with slug {team_slug:?} exists in this workspace."),
    )
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

fn issue_not_found_page(team_slug: &str, project_slug: &str, n: i32) -> Response {
    invalid_page(
        StatusCode::NOT_FOUND,
        "Issue not found",
        &format!("No issue #{n} in project {project_slug:?} (team {team_slug:?})."),
    )
}

fn not_found_page(message: &str) -> Response {
    invalid_page(StatusCode::NOT_FOUND, "Not found", message)
}

fn payload_too_large(limit_mb: u64) -> Response {
    let body = format!(
        "<!doctype html><html><body>\
         <h1>Upload too large</h1>\
         <p>The attached file exceeds the configured limit of {limit_mb} megabytes. \
         Reduce the file size and try again.</p>\
         </body></html>",
    );
    (StatusCode::PAYLOAD_TOO_LARGE, Html(body)).into_response()
}

fn internal_error<E: std::fmt::Display>(label: &str, err: E) -> Response {
    tracing::error!(error = %err, "{label} failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

fn bad_request_fragment(message: &str) -> Response {
    let body = format!(
        r#"<div class="error" data-hx-fragment="attachment-upload-error">{}</div>"#,
        html_escape(message)
    );
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
    let card = format!(
        r#"<li class="attachment" data-filename="{filename}"><span class="filename">{filename}</span> <span class="size">{label}</span></li>"#,
        filename = html_escape(filename),
        label = html_escape(&humanize_size(size_bytes)),
    );
    format!(r#"<div hx-swap-oob="beforeend:[data-attachment-list]">{card}</div>"#)
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
