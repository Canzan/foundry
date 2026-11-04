//! The route form: its fields as posted, the answer a door gives with it,
//! and the door that renders it for a project (DDR-25, K-4, K-6).

use crate::csrf::ensure_csrf_cookie;
use crate::AppState;
use askama::Template;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::routes::{base_path, project_page, stored_kind};
use super::{
    refusal, render_with, Caller, AUDIENCE, CANNOT_POST, INVITE_THE_BOT, MATRIX_UNREACHABLE,
    PRIVATE_CHANNELS, ROOM_ENCRYPTED, SHARED_CHANNEL,
};
use crate::integrations::matrix::{MatrixBot, RoomRefusal};
use crate::integrations::no_connection::{ConnectionWords, NoConnectionCause};
use crate::integrations::{MatrixOffer, SlackOffer};
use foundry_integrations::{
    ChannelKind, Destination, DestinationCatalog, DestinationRef, MessageLines,
};
use foundry_services::integrations::{RouteTarget, UnreadableConnections};
use foundry_store::integrations::RouteSettings;

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
    /// What shows in the form's place without a usable connection of the
    /// form's kind, and the fix a workspace admin is offered (ADR-IW-001).
    no_connection: Option<ConnectionWords>,
    channels: Vec<Choice>,
    events: Vec<Choice>,
    lanes: Vec<Choice>,
    levels: Vec<Choice>,
    /// The Slack picker's note on private channels; none for a Matrix room.
    private_channels: Option<&'static str>,
    /// The kind the form posts: the door's, so every door it reaches applies
    /// the route's kind (ADR-IW-001).
    kind: &'static str,
    /// A new Matrix route: its room is typed, not picked.
    typed_room: bool,
    /// Send test on the form: Slack's only. A saved Matrix route's test is
    /// on the project page (ADR-EIP-006 decision 2).
    send_test: bool,
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

    /// Refused without a usable connection of the form's kind: its cause
    /// shows in the form's place, K-19 included (ADR-IW-001, DD-2).
    pub(super) fn refused() -> Self {
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
    /// What the instance offers for Slack and for Matrix, this workspace's
    /// connections judged.
    slack_offer: SlackOffer,
    matrix_offer: MatrixOffer,
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
            slack_offer: integrations.slack_offer(unreadable),
            matrix_offer: integrations.matrix_offer(unreadable),
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

    /// Why there is no connection of the form's kind this Foundry can use,
    /// if there is none (a lost connection still counts as one).
    pub(super) fn cause(&self) -> Option<NoConnectionCause> {
        match self.kind {
            ChannelKind::Matrix => {
                NoConnectionCause::of_matrix(self.matrix_offer, self.matrix.is_some())
            }
            _ => NoConnectionCause::of_slack(self.slack_offer, self.bot.is_some()),
        }
    }

    /// The door for a route of `kind`.
    pub(super) fn of_kind(self, kind: ChannelKind) -> Self {
        Self { kind, ..self }
    }

    /// The door for editing the saved `route`, of the route's own kind.
    pub(super) fn editing(self, route: &RouteSettings) -> Self {
        Self {
            kind: stored_kind(&route.kind),
            action_path: format!("{}/{}/edit", self.routes_path, route.id),
            editing: Some(Destination {
                reference: DestinationRef(route.destination_ref.clone()),
                label: route.destination_label.clone(),
                shared_with_another_organisation: false,
            }),
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

    /// The route form for `form` with `answer`; why not, without a usable
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
        let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
        let nav = caller.nav(state, csrf.clone()).await;
        let status = answer.status;
        let page = self.page(nav, csrf, form, answer, channels);
        render_with(status, page.render(), set_cookie)
    }

    /// The form page for `form` with `answer`, offering `channels`.
    fn page(
        &self,
        nav: crate::nav::NavContext,
        csrf: String,
        form: &RouteForm,
        answer: FormAnswer,
        channels: Vec<Destination>,
    ) -> RouteFormPage {
        let chosen = channels.iter().find(|c| c.reference.0 == form.destination);
        let audience = chosen.map(|c| AUDIENCE.replace("{destination}", &c.label));
        let shared_channel = chosen
            .filter(|c| c.shared_with_another_organisation)
            .map(|c| SHARED_CHANNEL.replace("{destination}", &c.label));
        let slack = self.kind != ChannelKind::Matrix;
        RouteFormPage {
            nav,
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
            no_connection: self
                .cause()
                .map(|cause| cause.words(self.target.viewer_is_workspace_admin)),
            kind: self.kind.wire_name(),
            typed_room: !slack && self.editing.is_none(),
            send_test: slack,
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
            private_channels: slack.then_some(PRIVATE_CHANNELS),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn door(viewer_is_workspace_admin: bool) -> RouteDoor {
        let routes_path = "/team/t/project/p/integrations/routes".to_string();
        RouteDoor {
            target: RouteTarget {
                workspace_id: uuid::Uuid::nil(),
                project_id: uuid::Uuid::nil(),
                project_name: "Identity Platform".to_string(),
                key_prefix: "IDP".to_string(),
                connection: None,
                matrix_connection: None,
                lanes: Vec::new(),
                viewer_is_workspace_admin,
            },
            bot: None,
            matrix: None,
            slack_offer: SlackOffer::NotSetUp,
            matrix_offer: MatrixOffer::Ready,
            kind: ChannelKind::Slack,
            team_slug: "t".to_string(),
            project_slug: "p".to_string(),
            action_path: routes_path.clone(),
            routes_path,
            editing: None,
        }
    }

    fn saved_route(kind: &str) -> RouteSettings {
        RouteSettings {
            id: uuid::Uuid::nil(),
            kind: kind.to_string(),
            destination_ref: "!room:example.org".to_string(),
            destination_label: "#identity".to_string(),
            events: vec!["issue.created".to_string()],
            content_level: "summary".to_string(),
            lanes: Vec::new(),
            problem_reason: None,
        }
    }

    #[test]
    fn the_edit_door_names_the_cause_of_the_saved_routes_own_kind() {
        for (kind, cause) in [
            (
                "matrix",
                NoConnectionCause::NotConnected(ChannelKind::Matrix),
            ),
            ("slack", NoConnectionCause::NotSetUp(ChannelKind::Slack)),
        ] {
            for admin in [true, false] {
                let edit = door(admin).editing(&saved_route(kind));
                assert_eq!(edit.cause(), Some(cause), "a saved {kind} route");
            }
        }
    }

    /// The edit form of a saved route as rendered for the workspace admin.
    fn rendered_edit_form(kind: &str) -> String {
        let route = saved_route(kind);
        let edit = door(true).editing(&route);
        let nav = crate::nav::NavContext::for_page(
            "Canzan Labs".to_string(),
            "Ada Lovelace".to_string(),
            false,
            true,
            "csrf".to_string(),
            crate::nav::NavSection::Home,
            "/".to_string(),
        );
        let form = RouteForm {
            destination: route.destination_ref.clone(),
            ..RouteForm::new(None)
        };
        let channels = edit.editing.clone().into_iter().collect();
        let page = edit.page(
            nav,
            "csrf".to_string(),
            &form,
            FormAnswer::plain(),
            channels,
        );
        // The form as a usable connection shows it: no cause in its place.
        RouteFormPage {
            no_connection: None,
            ..page
        }
        .render()
        .expect("the route form renders")
    }

    #[test]
    fn a_saved_routes_edit_form_posts_its_own_kind_and_offers_send_test_for_slack_only() {
        for (kind, send_test) in [("matrix", false), ("slack", true)] {
            let html = rendered_edit_form(kind);
            assert!(
                html.contains(&format!(r#"name="kind" value="{kind}""#)),
                "a saved {kind} route's edit form posts kind={kind}"
            );
            assert_eq!(
                html.contains("Send test"),
                send_test,
                "a saved {kind} route's edit form offers Send test: {send_test}"
            );
        }
    }
}
