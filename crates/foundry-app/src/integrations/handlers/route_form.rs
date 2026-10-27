//! The route form: its fields as posted, the answer a door gives with it,
//! and the door that renders it for a project (DDR-25, K-4, K-6).

use crate::csrf::ensure_csrf_cookie;
use crate::AppState;
use askama::Template;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::routes::{base_path, project_page};
use super::{
    refusal, render_with, unavailable_words, Caller, AUDIENCE, CANNOT_POST, INVITE_THE_BOT,
    MATRIX_UNREACHABLE, NO_CHAT_CONNECTION, PRIVATE_CHANNELS, ROOM_ENCRYPTED, SHARED_CHANNEL,
};
use crate::integrations::matrix::{MatrixBot, RoomRefusal};
use foundry_integrations::{
    ChannelKind, Destination, DestinationCatalog, DestinationRef, MessageLines,
};
use foundry_services::integrations::{RouteTarget, UnreadableConnections};

#[derive(Template)]
#[template(path = "integrations/route_form.html")]
struct RouteFormPage {
    nav: crate::nav::NavContext,
    csrf: String,
    heading: String,
    /// Where Save posts: the new route, or the edited one.
    action_path: String,
    test_path: String,
    preview_path: String,
    notice: Option<String>,
    /// K-5 for the chosen channel.
    audience: Option<String>,
    /// K-6 when the chosen channel is shared with another organisation.
    shared_channel: Option<String>,
    shared_tick: bool,
    shared_confirmed: bool,
    /// The preview, once asked for; `Some(None)` when there is no issue yet.
    preview: Option<Option<MessageLines>>,
    no_issue_yet: String,
    no_connection: bool,
    channels: Vec<Choice>,
    events: Vec<Choice>,
    lanes: Vec<Choice>,
    levels: Vec<Choice>,
    private_channels: &'static str,
    /// What shows in the form's place without a usable connection: K-19 for
    /// a saved connection this Foundry cannot read, else K-4.
    no_chat_connection: String,
    /// A Matrix route: its room is typed, not picked.
    matrix: bool,
}

/// One option of the form: its value, its label, and whether it is chosen.
struct Choice {
    value: String,
    label: String,
    chosen: bool,
}

/// The chat events (D-2), as the form offers them.
const EVENT_CHOICES: [(&str, &str); 4] = [
    ("issue.created", "New issue"),
    ("issue.moved", "Issue moved"),
    ("comment.added", "New comment"),
    ("issue.deleted", "Issue deleted"),
];

/// The content levels (D-3); Summary is the default (OD-6).
const LEVEL_CHOICES: [(&str, &str); 3] = [
    ("link_only", "Link only"),
    ("summary", "Summary"),
    ("summary_with_comment", "Summary with comment text"),
];

/// The field prefix carrying a channel's name, `channel_label.{id}`.
const CHANNEL_LABEL_FIELD: &str = "channel_label.";

/// What the route form submitted.
#[derive(Debug, Clone, Default)]
pub(super) struct RouteForm {
    pub(super) kind: String,
    pub(super) destination: String,
    /// Each offered channel's name, by its id, as the picker showed it: so a
    /// preview, which asks Slack nothing, still names the channel (K-5).
    pub(super) channel_labels: Vec<(String, String)>,
    pub(super) events: Vec<String>,
    pub(super) content_level: String,
    pub(super) lanes: Vec<String>,
    pub(super) shared_confirmed: bool,
}

impl RouteForm {
    pub(super) fn read(fields: &[(String, String)]) -> Self {
        let mut form = RouteForm {
            content_level: "summary".to_string(),
            ..RouteForm::default()
        };
        for (name, value) in fields {
            match name.as_str() {
                "kind" => form.kind = value.clone(),
                "destination" => form.destination = value.clone(),
                "events" => form.events.push(value.clone()),
                "content_level" => form.content_level = value.clone(),
                "lanes" => form.lanes.push(value.clone()),
                "shared_confirmed" => form.shared_confirmed = true,
                other => {
                    if let Some(id) = other.strip_prefix(CHANNEL_LABEL_FIELD) {
                        form.channel_labels.push((id.to_string(), value.clone()));
                    }
                }
            }
        }
        form
    }

    /// The chosen channel's name as the picker showed it, else its id.
    pub(super) fn destination_label(&self) -> String {
        self.channel_labels
            .iter()
            .find(|(id, _)| *id == self.destination)
            .map_or_else(|| self.destination.clone(), |(_, label)| label.clone())
    }

    /// A fresh form: New issue at Summary, for `destination`.
    pub(super) fn new(destination: Option<String>) -> Self {
        RouteForm {
            destination: destination.unwrap_or_default(),
            events: vec!["issue.created".to_string()],
            content_level: "summary".to_string(),
            ..RouteForm::default()
        }
    }
}

/// What a route form answer shows beside the form.
pub(super) struct FormAnswer {
    notice: Option<String>,
    status: StatusCode,
    preview: Option<Option<MessageLines>>,
}

impl FormAnswer {
    pub(super) fn with(notice: Option<&str>, status: StatusCode) -> Self {
        Self {
            notice: notice.map(str::to_string),
            status,
            preview: None,
        }
    }

    pub(super) fn plain() -> Self {
        Self::with(None, StatusCode::OK)
    }

    /// The destination check refused the channel (Save and Send test alike).
    pub(super) fn cannot_post() -> Self {
        Self::with(Some(CANNOT_POST), StatusCode::UNPROCESSABLE_ENTITY)
    }

    /// Refused on a connection this Foundry cannot read: K-19 shows in the
    /// form's place (DD-2).
    pub(super) fn unavailable() -> Self {
        Self::with(None, StatusCode::UNPROCESSABLE_ENTITY)
    }

    /// A preview: it never asks Slack for the channel list (DDR-25).
    pub(super) fn preview(lines: Option<MessageLines>) -> Self {
        Self {
            preview: Some(lines),
            ..Self::plain()
        }
    }

    fn asks_slack(&self) -> bool {
        self.preview.is_none()
    }
}

/// A route door's target (Manage, else the uniform 404) and the workspace's
/// Slack bot, when there is a connection this Foundry can open.
pub(super) struct RouteDoor {
    pub(super) target: RouteTarget,
    pub(super) bot: Option<super::slack::SlackBot>,
    /// The workspace's Matrix bot, when it is connected and opens here.
    pub(super) matrix: Option<MatrixBot>,
    /// The workspace's saved connections this Foundry cannot read (K-19).
    unreadable: UnreadableConnections,
    /// The kind of route the form makes.
    kind: ChannelKind,
    pub(super) team_slug: String,
    pub(super) project_slug: String,
    routes_path: String,
    /// Where Save posts: `routes_path`, or the edited route's edit path.
    action_path: String,
    /// The edited route's destination: the only channel its form offers
    /// (an edit keeps its destination and asks Slack nothing).
    editing: Option<Destination>,
}

impl RouteDoor {
    pub(super) async fn open(
        state: &AppState,
        caller: &Caller,
        team_slug: &str,
        project_slug: &str,
    ) -> Result<Self, Response> {
        let target = caller
            .services
            .route_target(&caller.principal(), team_slug, project_slug)
            .await
            .map_err(refusal)?;
        Ok(Self::at(state, target, team_slug, project_slug))
    }

    /// The door for an already checked `target`. Opening the bot asks Slack
    /// nothing.
    pub(super) fn at(
        state: &AppState,
        target: RouteTarget,
        team_slug: &str,
        project_slug: &str,
    ) -> Self {
        let integrations = state.integrations.current();
        let bot = target.connection.as_ref().and_then(|connection| {
            let sealed = connection.sealed_credential.clone()?;
            integrations.slack_bot(connection.id, sealed)
        });
        let matrix = target
            .matrix_connection
            .as_ref()
            .and_then(|connection| integrations.matrix_bot(connection));
        let unreadable = |connection: Option<&_>| {
            connection.is_some_and(|connection| !integrations.opens(connection))
        };
        let unreadable = UnreadableConnections {
            slack: unreadable(target.connection.as_ref()),
            matrix: unreadable(target.matrix_connection.as_ref()),
        };
        let routes_path = format!("{}/routes", base_path(team_slug, project_slug));
        Self {
            target,
            bot,
            matrix,
            unreadable,
            kind: ChannelKind::Slack,
            team_slug: team_slug.to_string(),
            project_slug: project_slug.to_string(),
            action_path: routes_path.clone(),
            routes_path,
            editing: None,
        }
    }

    /// The connection's homeserver address, as the admin entered it.
    fn matrix_homeserver(&self) -> String {
        self.target
            .matrix_connection
            .as_ref()
            .map(|c| c.external_id.clone())
            .unwrap_or_default()
    }

    /// The words for a room the bot cannot take a route in (K-14, K-15).
    pub(super) fn room_refusal(&self, refusal: RoomRefusal, room: &str) -> String {
        match refusal {
            RoomRefusal::NotInvited => {
                let bot = self
                    .target
                    .matrix_connection
                    .as_ref()
                    .and_then(|c| c.bot_user_id.as_deref())
                    .unwrap_or_default();
                INVITE_THE_BOT.replace("{bot}", bot).replace("{room}", room)
            }
            RoomRefusal::Encrypted => ROOM_ENCRYPTED.replace("{room}", room),
            RoomRefusal::Unreachable => {
                MATRIX_UNREACHABLE.replace("{homeserver}", &self.matrix_homeserver())
            }
        }
    }

    /// The project's integrations page, with `notice` above its routes.
    pub(super) async fn project_page(
        &self,
        state: &AppState,
        caller: &Caller,
        headers: &HeaderMap,
        notice: String,
    ) -> Response {
        project_page(
            state,
            caller,
            headers,
            &self.team_slug,
            &self.project_slug,
            Some(notice),
        )
        .await
    }

    /// The form's heading: the edited route's, or the new route's.
    fn heading(&self) -> String {
        match &self.editing {
            Some(destination) => format!("Edit the route to {}", destination.label),
            None => format!(
                "New {} route for {}",
                self.kind.display_name(),
                self.target.project_name
            ),
        }
    }

    /// K-4: no connection of the form's kind that this Foundry can open.
    fn no_connection(&self) -> bool {
        match self.kind {
            ChannelKind::Matrix => self.matrix.is_none(),
            _ => self.bot.is_none(),
        }
    }

    /// K-19: the workspace's connection of `kind` is saved, but this Foundry
    /// cannot read it (DD-2), so no route write goes through on it.
    pub(super) fn unreadable(&self, kind: ChannelKind) -> bool {
        match kind {
            ChannelKind::Matrix => self.unreadable.matrix,
            _ => self.unreadable.slack,
        }
    }

    /// [`Self::unreadable`] for the form's kind.
    pub(super) fn unavailable(&self) -> bool {
        self.unreadable(self.kind)
    }

    /// What shows in the form's place without a usable connection: K-19 or
    /// K-4.
    fn no_connection_words(&self) -> String {
        if self.unavailable() {
            unavailable_words(self.kind)
        } else {
            NO_CHAT_CONNECTION.to_string()
        }
    }

    /// The door for a route of `kind`.
    pub(super) fn of_kind(self, kind: ChannelKind) -> Self {
        Self { kind, ..self }
    }

    /// The door for editing the route `route_id` to `destination`.
    pub(super) fn editing(self, route_id: uuid::Uuid, destination: Destination) -> Self {
        Self {
            action_path: format!("{}/{route_id}/edit", self.routes_path),
            editing: Some(destination),
            ..self
        }
    }

    /// The channels the picker offers: Slack's list, or for a preview only the
    /// channel the form names, since a preview asks Slack nothing.
    async fn channels(&self, form: &RouteForm, answer: &FormAnswer) -> Vec<Destination> {
        if let Some(destination) = &self.editing {
            return vec![destination.clone()];
        }
        if self.kind == ChannelKind::Matrix {
            // A room is typed; its audience is whoever can read it (K-5).
            return (!form.destination.is_empty())
                .then(|| Destination {
                    reference: DestinationRef(form.destination.clone()),
                    label: form.destination.clone(),
                    shared_with_another_organisation: false,
                })
                .into_iter()
                .collect();
        }
        match &self.bot {
            Some(bot) if answer.asks_slack() => bot.list_destinations().await.unwrap_or_default(),
            Some(_) if !form.destination.is_empty() => vec![Destination {
                reference: DestinationRef(form.destination.clone()),
                label: form.destination_label(),
                shared_with_another_organisation: false,
            }],
            _ => Vec::new(),
        }
    }

    /// The route form for `form` with `answer`; K-4 without a usable
    /// connection, K-5 for the chosen channel and K-6 when it is shared.
    pub(super) async fn form(
        &self,
        state: &AppState,
        caller: &Caller,
        headers: &HeaderMap,
        form: &RouteForm,
        answer: FormAnswer,
    ) -> Response {
        let channels = self.channels(form, &answer).await;
        let chosen = channels.iter().find(|c| c.reference.0 == form.destination);
        let audience = chosen.map(|c| AUDIENCE.replace("{destination}", &c.label));
        let shared_channel = chosen
            .filter(|c| c.shared_with_another_organisation)
            .map(|c| SHARED_CHANNEL.replace("{destination}", &c.label));
        let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
        let page = RouteFormPage {
            nav: caller.nav(state, csrf.clone()).await,
            csrf,
            heading: self.heading(),
            action_path: self.action_path.clone(),
            test_path: format!("{}/test", self.routes_path),
            preview_path: format!("{}/preview", self.routes_path),
            notice: answer.notice,
            audience,
            shared_tick: shared_channel.is_some() || form.shared_confirmed,
            shared_channel,
            shared_confirmed: form.shared_confirmed,
            preview: answer.preview,
            no_issue_yet: format!("{} has no issue to preview yet.", self.target.project_name),
            no_connection: self.no_connection(),
            matrix: self.kind == ChannelKind::Matrix && self.editing.is_none(),
            channels: channels
                .into_iter()
                .map(|c| Choice {
                    chosen: c.reference.0 == form.destination,
                    value: c.reference.0,
                    label: c.label,
                })
                .collect(),
            events: choices(&EVENT_CHOICES, |value| {
                form.events.iter().any(|e| e == value)
            }),
            lanes: self
                .target
                .lanes
                .iter()
                .map(|lane| Choice {
                    chosen: form.lanes.contains(&lane.slug),
                    value: lane.slug.clone(),
                    label: lane.label.clone(),
                })
                .collect(),
            levels: choices(&LEVEL_CHOICES, |value| form.content_level == value),
            private_channels: PRIVATE_CHANNELS,
            no_chat_connection: self.no_connection_words(),
        };
        render_with(answer.status, page.render(), set_cookie)
    }
}

fn choices(table: &[(&str, &str)], chosen: impl Fn(&str) -> bool) -> Vec<Choice> {
    table
        .iter()
        .map(|(value, label)| Choice {
            value: value.to_string(),
            label: label.to_string(),
            chosen: chosen(value),
        })
        .collect()
}
