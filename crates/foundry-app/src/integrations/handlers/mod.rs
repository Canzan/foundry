//! The integration doors (DESIGN [REF] Driving Ports; ADR-EIP-005 decisions
//! 2 and 4). Every door answers the uniform 404 to a caller without the right
//! (SC-6), decided by the use-case before anything else.
//!
//! - `GET /workspace/integrations`: the Slack connection (K-1), what is
//!   missing (K-2, K-20) or Add to Slack, and the activity list (R-4).
//! - `POST /workspace/integrations/matrix`: connects Matrix (admins,
//!   ADR-EIP-006 decision 1): the homeserver's address must be `https` and
//!   pass the outbound address policy before anything is contacted, and
//!   `whoami` must accept the bot's token; only then is the token sealed for
//!   its connection row and saved. Anything else saves nothing and says why.
//! - `GET /workspace/integrations/slack/setup`: the manifest page (DD-9).
//! - `GET /workspace/integrations/slack/install`: keeps a fresh state in the
//!   session and sends the admin to Slack's approval screen.
//! - `GET /workspace/integrations/slack/return`: takes the state back out
//!   (single use), and only for a matching, fresh state exchanges the code,
//!   seals the bot token for its connection row and saves the connection.
//!   Anything else shows K-3 and stores nothing.
//! - `GET /team/{t}/project/{p}/integrations`: the project's routes with their
//!   K-9 status (Manage).
//! - `GET …/integrations/routes/new?kind=slack`: the route form, its channel
//!   picker listing the channels Foundry can post to, or K-4 without a
//!   connection (Manage).
//! - `POST …/integrations/routes/preview`: the form again with the message the
//!   route would send for the project's newest issue, from the one `compose`
//!   the worker sends with; asks Slack nothing, writes nothing (DDR-25).
//! - `POST …/integrations/routes/test`: exactly one K-7 message to the entered
//!   channel, no row written, six a minute per person (DDR-25, R-5); only to
//!   a channel Slack lists among those Foundry can post to, as Save checks,
//!   else Save's answer and nothing posted.
//! - `POST …/integrations/routes`: checks the channel with Slack, then saves
//!   the route and its audit record; sends nothing (DDR-25). A channel shared
//!   with another organisation is saved only with the K-6 tick. A Matrix
//!   route first brings the bot into the room (`DestinationEnrolment`) and
//!   reads it: a room the bot cannot join, or an encrypted one (which the
//!   bot leaves again), is explained and no route is saved (ADR-EIP-006
//!   decision 2).
//! - `POST …/integrations/routes/test` with a saved Matrix route's
//!   `route_id`: the K-7 message to its room, as a fresh transaction.
//! - `GET …/integrations/routes/{route_id}/history`: every delivery of the
//!   route, newest first, in Foundry's words (US-EIP-05, Manage).
//! - `POST …/integrations/routes/{route_id}/pause` and `…/resume`: pause
//!   (its waiting deliveries skipped, DD-8) or resume (receives what happens
//!   from now, DDR-27; a refusal's stop cleared) the route, audited; asking
//!   for what is already so changes nothing (Manage).
//! - `GET`/`POST …/integrations/routes/{route_id}/edit`: the route form for
//!   the route, and its save: events, content level and lanes (Manage).
//! - `GET …/integrations/routes/{route_id}/delete`: the delete question; its
//!   `POST` deletes the route and what it was waiting to send, never a
//!   message already posted (Manage). A deleted Matrix route's bot leaves its
//!   room when no other route of the workspace uses it; a leave the
//!   homeserver refuses goes to the log and never undoes the delete.
//! - `GET /workspace/integrations/slack/disconnect`: the disconnect question,
//!   counting the routes that stop (admins); its `POST` disconnects: the
//!   connection's credential is forgotten and what its routes waited to send
//!   is skipped, in one transaction, and only then is Slack asked to withdraw
//!   Foundry's access (`apps.uninstall`), so a Slack failure never leaves the
//!   credential stored. Works without scripting; CSRF-protected.
//! - `GET`/`POST /workspace/integrations/matrix/disconnect`: the same door
//!   for Matrix (user decision 2026-10-25): the bot's access token is
//!   forgotten and the routes pause; the bot stays in its rooms.
//! - `POST /integrations/slack/events`: Slack's Events API (DDR-11,
//!   ADR-EIP-005 decision 6), a machine post with no session, believed only
//!   when Slack signed it (401 otherwise): it answers Slack's address check
//!   with its challenge, and the Foundry app's removal from a connected Slack
//!   workspace (`app_uninstalled`, `tokens_revoked`) turns that connection
//!   lost and tells its admins once. Anything else is acknowledged and
//!   changes nothing.
//!
//! On a connection this Foundry cannot read (DD-2, judged per connection),
//! every route write and Send test answers K-19 for its kind and writes
//! nothing; Disconnect and Matrix's Replace token stay open as repairs, and
//! the Slack disconnect question carries R-13 in place of its last sentence.

mod matrix_setup;
mod route_form;
mod routes;
mod slack_setup;
mod workspace;

pub use matrix_setup::connect_matrix;
pub use routes::{
    create_route, delete_route, edit_route, pause_route, preview_route, resume_route, send_test,
    show_delete_route, show_edit_route, show_new_route, show_project, show_route_history,
    NewRouteQuery,
};
pub use slack_setup::{
    finish_slack_install, show_slack_setup, slack_events, start_slack_install, SlackReturn,
};
pub use workspace::{
    disconnect_matrix, disconnect_slack, show_matrix_disconnect, show_slack_disconnect,
    show_workspace,
};

use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::response_with_optional_cookie;
use crate::integrations::slack;
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use foundry_integrations::ChannelKind;
use foundry_services::integrations::{IntegrationError, UnreadableConnections};
use foundry_services::{Principal, Services};
use tower_sessions::Session;

/// The target of the doors' log events: this module's path, which the events
/// carried before the doors moved into submodules, so log filters and
/// searches on it keep matching.
const LOG_TARGET: &str = "foundry_app::integrations::handlers";

pub const WORKSPACE_PATH: &str = "/workspace/integrations";
pub const SETUP_PATH: &str = "/workspace/integrations/slack/setup";
pub const INSTALL_PATH: &str = "/workspace/integrations/slack/install";
pub const RETURN_PATH: &str = slack::RETURN_PATH;
pub const DISCONNECT_PATH: &str = "/workspace/integrations/slack/disconnect";
pub const MATRIX_DISCONNECT_PATH: &str = "/workspace/integrations/matrix/disconnect";
pub const MATRIX_CONNECT_PATH: &str = "/workspace/integrations/matrix";

/// US-EIP-08: the homeserver answered and refused the bot's token.
pub const MATRIX_TOKEN_REFUSED: &str =
    "The homeserver did not accept this access token. Nothing was saved.";
/// US-EIP-08, for the entered `{homeserver}`.
pub const MATRIX_UNREACHABLE: &str = "Foundry could not reach {homeserver}. Nothing was saved.";
/// UI-6 (R-1): the outbound address policy refused the address.
pub const ADDRESS_REFUSED: &str =
    "Foundry will not send to this address. An instance operator can allow it.";
/// The Matrix connection could not be saved (another homeserver is connected).
pub const MATRIX_NOT_CONNECTED: &str = "Matrix was not connected. Nothing changed.";
/// K-14, for the bot `{bot}` and the room `{room}`.
pub const INVITE_THE_BOT: &str =
    "Invite {bot} to {room} in your Matrix client first, then try again.";
/// K-15, for the room `{room}`.
pub const ROOM_ENCRYPTED: &str = "{room} is end-to-end encrypted. Foundry's bot cannot post to encrypted rooms yet. Choose an unencrypted room.";
/// US-EIP-09: the saved route's room, its name and its members.
pub const BOT_IN_ROOM: &str = "Foundry is in {room} (room \"{name}\", {members} members)";

/// K-3.
pub const NOT_CONNECTED: &str = "Slack was not connected. Nothing changed.";
/// US-EIP-06 example 5, for the `{approved}` Slack workspace when the
/// connection is to `{connected}`.
pub const OTHER_SLACK_WORKSPACE: &str =
    "This is a different Slack workspace ({approved}). Routes stay paused. Reconnect to {connected}, or disconnect.";
/// K-4.
pub const NO_CHAT_CONNECTION: &str =
    "This workspace has no chat connection yet. Ask a workspace admin to connect Slack or Matrix.";
/// K-19, for the connection's `{kind}` (DD-2; R-2).
pub const UNAVAILABLE: &str = "{kind} is unavailable: this Foundry cannot read its saved connection. An instance operator needs to check the secrets key.";
/// R-13, for the Slack workspace's name in the possessive, `{owner}`: the
/// disconnect question's last sentence on a connection this Foundry cannot
/// read (DD-2 amended 2026-10-27).
pub const CANNOT_REMOVE_ACCESS: &str =
    "Foundry cannot remove its access to {owner} Slack: remove the Foundry app in Slack by hand.";
/// R-5.
pub const TOO_MANY_TESTS: &str = "Too many test messages. Try again in a minute.";
/// The channel picker's note on private channels (US-EIP-02 example 2).
pub const PRIVATE_CHANNELS: &str =
    "Private channels appear here after someone runs /invite @Foundry in them.";
pub const TEST_SENT: &str = "Test message sent.";
pub const CANNOT_POST: &str = "Foundry cannot post to that channel. Choose another.";
/// K-5, for the chosen `{destination}`.
pub const AUDIENCE: &str =
    "Everyone who can read {destination} will see what this route sends. Foundry cannot check who that is.";
/// US-EIP-07's delete question, for the route to `{destination}`.
pub const DELETE_QUESTION: &str =
    "Delete the route to {destination}? {project} stops posting there. Messages already in {kind} stay.";
/// K-6, for the chosen `{destination}`.
pub const SHARED_CHANNEL: &str =
    "{destination} is shared with another organisation. Tick to confirm that is intended.";

/// The signed-in caller as the use-cases see them.
pub(super) struct Caller {
    user: SessionUser,
    services: Services,
}

impl Caller {
    async fn of(state: &AppState, session: &Session) -> Option<Self> {
        let user = session
            .get::<SessionUser>(SESSION_KEY_USER_ID)
            .await
            .ok()
            .flatten()?;
        Some(Self {
            user,
            services: Services::new(state.store.clone()),
        })
    }

    /// The caller when they are an admin of their workspace; else signing in,
    /// or the uniform not-found page.
    async fn admin(state: &AppState, session: &Session) -> Result<Self, Response> {
        let caller = Self::of(state, session).await.ok_or_else(sign_in)?;
        caller
            .services
            .require_workspace_admin(&caller.principal())
            .await
            .map_err(refusal)?;
        Ok(caller)
    }

    fn principal(&self) -> Principal {
        Principal::Human {
            user_id: self.user.user_id,
            workspace_id: self.user.workspace_id,
        }
    }

    /// Which of the caller's workspace's saved connections this Foundry
    /// cannot read (DD-2, judged per connection). None, and nothing read,
    /// when the boot probe proved every stored credential's key.
    async fn unreadable_connections(
        &self,
        state: &AppState,
    ) -> Result<UnreadableConnections, IntegrationError> {
        let integrations = state.integrations.current();
        if integrations.every_credential_opens() {
            return Ok(UnreadableConnections::default());
        }
        self.services
            .unreadable_connections(&self.principal(), &*integrations)
            .await
    }

    async fn nav(&self, state: &AppState, csrf: String) -> crate::nav::NavContext {
        let is_instance_admin =
            crate::nav::resolve_is_instance_admin(state, self.user.user_id).await;
        crate::nav::NavContext::home_for(
            state,
            self.user.user_id,
            self.user.workspace_id,
            is_instance_admin,
            csrf,
        )
        .await
    }
}

/// A signed-out visitor goes to sign in, as from any page that needs a
/// session (the shipped `redirect_to("/sign-in")`: 303, `Location: /sign-in`).
/// The uniform not-found (SC-6) is for signed-in people without the right.
pub(super) fn sign_in() -> Response {
    Redirect::to("/sign-in").into_response()
}

pub(super) fn refusal(err: IntegrationError) -> Response {
    if let IntegrationError::Store(err) = err {
        tracing::error!(error = %err, "integrations: store read failed");
        return internal_error();
    }
    resource_not_found_page()
}

/// K-19 for a connection of `kind`.
pub(super) fn unavailable_words(kind: ChannelKind) -> String {
    UNAVAILABLE.replace("{kind}", kind.display_name())
}

pub(super) fn internal_error() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

pub(super) fn render(
    rendered: Result<String, askama::Error>,
    set_cookie: Option<String>,
) -> Response {
    render_with(StatusCode::OK, rendered, set_cookie)
}

pub(super) fn render_with(
    status: StatusCode,
    rendered: Result<String, askama::Error>,
    set_cookie: Option<String>,
) -> Response {
    match rendered {
        Ok(html) => response_with_optional_cookie(status, Html(html).into_response(), set_cookie),
        Err(err) => {
            tracing::error!(error = %err, "integrations: render failed");
            internal_error()
        }
    }
}
