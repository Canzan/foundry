//! What a sender reads at send time: a delivery's context, a member's email
//! facts, and the route form's preview issue (DDR-25, ADR-EIP-007).

use super::StoredKind;
use crate::{Store, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

/// What a sender needs to attempt one claimed delivery, read at send time
/// (ADR-EIP-002 decision 8). The event's issue facts are `None` when the issue
/// is gone; the project's when it was deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryContext {
    pub delivery_id: i64,
    pub event_kind: String,
    pub give_up_at: OffsetDateTime,
    pub route_kind: String,
    pub content_level: String,
    /// The route is paused, its connection disconnected, or the event
    /// happened before the route was last resumed (DDR-27): it is not sent.
    pub route_paused: bool,
    pub connection_id: Option<Uuid>,
    pub connection_state: Option<String>,
    pub destination_ref: String,
    pub project: Option<DeliveryProject>,
    pub issue: Option<DeliveryIssue>,
    /// What the event's outbox row recorded as it happened.
    pub event: DeliveryEvent,
}

/// An event as its (first) outbox row recorded it (ADR-EIP-003): lanes are
/// the lanes it named, by their current label (a lane gone reads as its
/// slug); the actor is who acted. Every field is `None` when the row did not
/// record it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryEvent {
    /// How many outbox rows the delivery stands for: more than one is a
    /// burst (D-11).
    pub issue_count: usize,
    pub issue_number: Option<i32>,
    /// A deleted issue's title.
    pub title: Option<String>,
    pub lane_label: Option<String>,
    pub from_lane_label: Option<String>,
    pub actor_name: Option<String>,
    /// A new comment's text as it reads now (D-3: sent only at Summary with
    /// comment text); `None` for any other event or a comment gone since.
    pub comment_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryProject {
    pub name: String,
    pub key_prefix: String,
    pub slug: String,
    pub team_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryIssue {
    pub number: i32,
    pub title: String,
    pub lane_label: String,
    pub author_name: Option<String>,
}

/// What the send-time checks of one project email read now (ADR-EIP-007
/// decision 3): the member's address, their choice for the email's project
/// and event, whether they can open the project (a member of its team),
/// whether they muted its workspace, and whether they filed the issue or
/// have a comment on it that is not deleted.
const EMAIL_FACTS_SQL: &str = "
    SELECT u.email_display, ch.scope, u.id, d.project_id, d.event_kind,
           EXISTS (SELECT 1 FROM live_projects p
                     JOIN team_memberships tm ON tm.team_id = p.team_id
                    WHERE p.id = d.project_id AND tm.user_id = u.id),
           EXISTS (SELECT 1 FROM notification_unsubscribes n
                    WHERE n.email_lower = u.email_lower AND n.workspace_id = d.workspace_id),
           EXISTS (SELECT 1 FROM issues i
                    WHERE i.id = d.issue_id AND i.project_id = d.project_id
                      AND i.author_id = u.id)
           OR EXISTS (SELECT 1 FROM comments cm
                       WHERE cm.issue_id = d.issue_id AND cm.author_id = u.id
                         AND cm.deleted_at IS NULL)
      FROM integration_deliveries d
      JOIN users u ON u.id = d.recipient_user_id
      LEFT JOIN integration_email_choices ch
             ON ch.user_id = u.id AND ch.project_id = d.project_id AND ch.event = d.event_kind
     WHERE d.id = $1";

/// One project email's send-time facts, read now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailSendFacts {
    pub recipient: String,
    /// The choice the email is sent under: its member, project and event
    /// (its stop link names exactly this choice).
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub event: String,
    /// The member's choice's scope (`all`, `mine`); `None` once it is gone.
    pub scope: Option<String>,
    pub can_open: bool,
    pub muted: bool,
    pub involved: bool,
}

/// The delivery, its route, the project and issue now, and the event as its
/// first outbox row recorded it. A project deleted after the event reads as
/// gone even once restored (D-8: what waited at the delete is never sent); a
/// disconnected connection reads as a paused route (R-9) (lanes by slug; [`LANE_LABELS_SQL`] reads
/// their labels now). A move row written before `from_state` was
/// added (a rolling deploy) finds its from-lane in the status change the same
/// transaction wrote (ADR-EIP-003 decision 4).
const DELIVERY_CONTEXT_SQL: &str = "
    SELECT d.event_kind, d.give_up_at, COALESCE(r.kind, 'email'),
           COALESCE(r.content_level, 'summary_with_comment'),
           r.paused_at IS NOT NULL OR c.state IS NOT DISTINCT FROM 'disconnected'
               OR COALESCE(o.created_at < r.receives_from, FALSE),
           r.connection_id, c.state, COALESCE(r.destination_ref, ''),
           p.name, p.key_prefix, p.slug, t.slug,
           i.number, i.title, i.state, u.display_name,
           cardinality(d.outbox_ids), (o.payload->>'number')::int, o.payload->>'title',
           o.payload->>'state', ef.slug, a.display_name, d.project_id, cm.body_markdown
      FROM integration_deliveries d
      LEFT JOIN integration_routes r ON r.id = d.route_id
      LEFT JOIN integration_connections c ON c.id = r.connection_id
      LEFT JOIN outbox o ON o.id = d.outbox_ids[1]
      LEFT JOIN live_projects p ON p.id = d.project_id
            AND NOT EXISTS (SELECT 1 FROM project_lifecycle_events e
                             WHERE e.project_id = d.project_id AND e.kind = 'deleted'
                               AND e.created_at >= o.created_at)
      LEFT JOIN teams t ON t.id = p.team_id
      LEFT JOIN issues i ON i.id = d.issue_id AND i.project_id = d.project_id
      LEFT JOIN users u ON u.id = i.author_id
      LEFT JOIN LATERAL (
          SELECT COALESCE(o.payload->>'from_state', (
                     SELECT e.old_value FROM issue_change_events e
                      WHERE o.event_type = 'IssueUpdated' AND e.issue_id = d.issue_id
                        AND e.field = 'status' AND e.new_value = o.payload->>'state'
                        AND e.created_at <= o.created_at
                      ORDER BY e.created_at DESC LIMIT 1)) AS slug
      ) ef ON true
      LEFT JOIN users a ON a.id = (o.payload->>'author_id')::uuid
      LEFT JOIN comments cm ON cm.id = (o.payload->>'comment_id')::uuid
                            AND cm.issue_id = d.issue_id
     WHERE d.id = $1";

/// A project's newest issue as a message shows it now: its lane by label (a
/// lane gone reads as its slug) and its author by display name.
const LATEST_ISSUE_SQL: &str = "
    SELECT i.number, i.title, COALESCE(l.label, i.state), u.display_name
      FROM issues i
      LEFT JOIN lanes l ON l.project_id = i.project_id AND l.slug = i.state
      LEFT JOIN users u ON u.id = i.author_id
     WHERE i.project_id = $1
     ORDER BY i.number DESC
     LIMIT 1";

/// The current labels of a project's lanes, by slug.
const LANE_LABELS_SQL: &str =
    "SELECT slug, label FROM lanes WHERE project_id = $1 AND slug = ANY($2)";

impl Store {
    /// The project's newest issue, for the route form's preview (DDR-25):
    /// one read, nothing written.
    pub async fn latest_issue(
        &self,
        project_id: Uuid,
    ) -> Result<Option<DeliveryIssue>, StoreError> {
        let row: Option<(i32, String, String, Option<String>)> = sqlx::query_as(LATEST_ISSUE_SQL)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(
            row.map(|(number, title, lane_label, author_name)| DeliveryIssue {
                number,
                title,
                lane_label,
                author_name,
            }),
        )
    }

    /// What the send-time checks of the project email `delivery_id` read
    /// now; `None` when it is no member's email.
    pub async fn email_send_facts(
        &self,
        delivery_id: i64,
    ) -> Result<Option<EmailSendFacts>, StoreError> {
        type Row = (String, Option<String>, Uuid, Uuid, String, bool, bool, bool);
        let row: Option<Row> = sqlx::query_as(EMAIL_FACTS_SQL)
            .bind(delivery_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(
            |(recipient, scope, user_id, project_id, event, can_open, muted, involved)| {
                EmailSendFacts {
                    recipient,
                    user_id,
                    project_id,
                    event,
                    scope,
                    can_open,
                    muted,
                    involved,
                }
            },
        ))
    }

    /// What a sender needs to attempt `delivery_id`, read now: the route, its
    /// connection, the project (through `live_projects`), the issue and the
    /// event as its outbox row recorded it, in one query.
    pub async fn delivery_context(
        &self,
        delivery_id: i64,
    ) -> Result<Option<DeliveryContext>, StoreError> {
        use sqlx::Row;
        let Some(row) = sqlx::query(DELIVERY_CONTEXT_SQL)
            .bind(delivery_id)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        let project_name: Option<String> = row.try_get(8)?;
        let key_prefix: Option<String> = row.try_get(9)?;
        let project_slug: Option<String> = row.try_get(10)?;
        let team_slug: Option<String> = row.try_get(11)?;
        let number: Option<i32> = row.try_get(12)?;
        let title: Option<String> = row.try_get(13)?;
        let issue_lane: Option<String> = row.try_get(14)?;
        let author_name: Option<String> = row.try_get(15)?;
        let project = match (project_name, key_prefix, project_slug) {
            (Some(name), Some(key_prefix), Some(slug)) => Some(DeliveryProject {
                name,
                key_prefix,
                slug,
                team_slug,
            }),
            _ => None,
        };
        let event_lane: Option<String> = row.try_get(19)?;
        let from_lane: Option<String> = row.try_get(20)?;
        let project_id: Uuid = row.try_get(22)?;
        let slugs: Vec<&String> = [&issue_lane, &event_lane, &from_lane]
            .into_iter()
            .flatten()
            .collect();
        let labels: Vec<(String, String)> = sqlx::query_as(LANE_LABELS_SQL)
            .bind(project_id)
            .bind(&slugs)
            .fetch_all(&self.pool)
            .await?;
        // A lane gone since reads as its slug.
        let label_of = |slug: Option<String>| {
            slug.map(|slug| {
                labels
                    .iter()
                    .find(|(known, _)| *known == slug)
                    .map_or(slug.clone(), |(_, label)| label.clone())
            })
        };
        let issue = match (number, title, label_of(issue_lane)) {
            (Some(number), Some(title), Some(lane_label)) => Some(DeliveryIssue {
                number,
                title,
                lane_label,
                author_name,
            }),
            _ => None,
        };
        let issue_count: i32 = row.try_get(16)?;
        let event = DeliveryEvent {
            issue_count: usize::try_from(issue_count).unwrap_or(1),
            issue_number: row.try_get(17)?,
            title: row.try_get(18)?,
            lane_label: label_of(event_lane),
            from_lane_label: label_of(from_lane),
            actor_name: row.try_get(21)?,
            comment_text: row.try_get(23)?,
        };
        Ok(Some(DeliveryContext {
            delivery_id,
            event_kind: row.try_get(0)?,
            give_up_at: row.try_get(1)?,
            route_kind: row.try_get::<StoredKind, _>(2)?.wire(),
            content_level: row.try_get(3)?,
            route_paused: row.try_get(4)?,
            connection_id: row.try_get(5)?,
            connection_state: row.try_get(6)?,
            destination_ref: row.try_get(7)?,
            project,
            issue,
            event,
        }))
    }
}
