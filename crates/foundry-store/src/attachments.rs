//! US-11 — `issue_attachments` repository.
//!
//! Storage is inline bytea per `data-access.md`'s slice-3 decision: a
//! single `pg_dump` captures every attachment alongside its issue row
//! (NFR-DATA-01). The row carries denormalised `size_bytes` so the
//! issue-detail page can render labels without re-reading the bytea,
//! and `sha256_hex` so the US-03 restore scenario can prove byte-for-
//! byte integrity without re-scanning.
//!
//! Authorisation is the handler's job — the store is unconditioned on
//! requester identity. The handler resolves the issue through the
//! caller's team and project; `find_attachment_for_issue` then returns
//! the row only when it belongs to that resolved issue.

use crate::{Store, StoreError};
use sqlx::Row;
use thiserror::Error;

/// Full attachment row, including the inline bytes. Used by the
/// download handler — list views call `list_for_issue_summary` which
/// projects everything EXCEPT the bytea.
#[derive(Debug, Clone)]
pub struct AttachmentRow {
    pub id: uuid::Uuid,
    pub issue_id: uuid::Uuid,
    pub workspace_id: uuid::Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256_hex: String,
    pub content: Vec<u8>,
}

/// Light projection for the issue-detail listing — no bytea.
#[derive(Debug, Clone)]
pub struct AttachmentSummary {
    pub id: uuid::Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
}

#[derive(Debug, Error)]
pub enum AttachmentInsertError {
    #[error("issue not found")]
    IssueNotFound,
    #[error(transparent)]
    Store(#[from] sqlx::Error),
}

impl Store {
    /// Insert a single attachment row. Caller has already verified the
    /// uploader is a member of the issue's team. One transaction: the issue's
    /// project, then the live-project guard (project-lifecycle DDD-7), then the
    /// insert; an unknown issue or a deleted project is `IssueNotFound`.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_attachment(
        &self,
        id: uuid::Uuid,
        issue_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        uploader_id: uuid::Uuid,
        filename: &str,
        content_type: &str,
        sha256_hex: &str,
        content: &[u8],
    ) -> Result<AttachmentRow, AttachmentInsertError> {
        let size_bytes = content.len() as i64;
        let mut tx = self.pool().begin().await?;
        let Some(project_id) =
            crate::project_lifecycle::issue_project(&mut tx, workspace_id, issue_id).await?
        else {
            return Err(AttachmentInsertError::IssueNotFound);
        };
        if !crate::project_lifecycle::lock_live_project(&mut tx, project_id).await? {
            return Err(AttachmentInsertError::IssueNotFound);
        }
        let result = sqlx::query(
            "INSERT INTO issue_attachments
                  (id, issue_id, workspace_id, uploader_id, filename,
                   content_type, size_bytes, sha256_hex, content)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(issue_id)
        .bind(workspace_id)
        .bind(uploader_id)
        .bind(filename)
        .bind(content_type)
        .bind(size_bytes)
        .bind(sha256_hex)
        .bind(content)
        .execute(&mut *tx)
        .await;
        match result {
            Ok(_) => {
                tx.commit().await?;
                Ok(AttachmentRow {
                    id,
                    issue_id,
                    workspace_id,
                    filename: filename.to_string(),
                    content_type: content_type.to_string(),
                    size_bytes,
                    sha256_hex: sha256_hex.to_string(),
                    content: content.to_vec(),
                })
            }
            Err(sqlx::Error::Database(db_err)) => {
                // FK violation on issue_id surfaces as 23503.
                if db_err.code().as_deref() == Some("23503") {
                    return Err(AttachmentInsertError::IssueNotFound);
                }
                Err(AttachmentInsertError::Store(sqlx::Error::Database(db_err)))
            }
            Err(err) => Err(AttachmentInsertError::Store(err)),
        }
    }

    /// Fetch one attachment of ONE issue, with full bytes. Returns `None`
    /// when the attachment does not exist OR belongs to any other issue, so
    /// the caller resolves the issue from the URL (team, project, number)
    /// first and an attachment is only ever served through its own issue's
    /// address (fix-issue-attachment-scoping, RCA Defect 2).
    pub async fn find_attachment_for_issue(
        &self,
        attachment_id: uuid::Uuid,
        issue_id: uuid::Uuid,
    ) -> Result<Option<AttachmentRow>, StoreError> {
        let row = sqlx::query(
            "SELECT id, issue_id, workspace_id, filename, content_type,
                    size_bytes, sha256_hex, content
               FROM issue_attachments
              WHERE id = $1 AND issue_id = $2",
        )
        .bind(attachment_id)
        .bind(issue_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|r| AttachmentRow {
            id: r.get(0),
            issue_id: r.get(1),
            workspace_id: r.get(2),
            filename: r.get(3),
            content_type: r.get(4),
            size_bytes: r.get(5),
            sha256_hex: r.get(6),
            content: r.get(7),
        }))
    }

    /// List the attachments on an issue (newest first). Returns the
    /// summary projection without the bytea — the issue-detail page
    /// uses this for rendering filename + size labels.
    pub async fn list_attachments_for_issue(
        &self,
        issue_id: uuid::Uuid,
    ) -> Result<Vec<AttachmentSummary>, StoreError> {
        let rows: Vec<(uuid::Uuid, String, String, i64)> = sqlx::query_as(
            "SELECT id, filename, content_type, size_bytes
               FROM issue_attachments
              WHERE issue_id = $1
              ORDER BY created_at DESC, id DESC",
        )
        .bind(issue_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, filename, content_type, size_bytes)| AttachmentSummary {
                    id,
                    filename,
                    content_type,
                    size_bytes,
                },
            )
            .collect())
    }

    /// Count attachments on an issue. Acceptance assertion helper for
    /// the "no attachments exist" path (post-delete + 413 + 403 paths).
    pub async fn count_attachments_for_issue(
        &self,
        issue_id: uuid::Uuid,
    ) -> Result<i64, StoreError> {
        let row: (i64,) =
            sqlx::query_as("SELECT count(*) FROM issue_attachments WHERE issue_id = $1")
                .bind(issue_id)
                .fetch_one(self.pool())
                .await?;
        Ok(row.0)
    }
}
