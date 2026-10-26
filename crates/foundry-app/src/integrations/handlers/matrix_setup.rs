//! Matrix's setup: connecting the bot, a Matrix route's room enrolment, its
//! test message, and leaving a room no route uses (ADR-EIP-006 decisions 1
//! and 2).

use super::route_form::{FormAnswer, RouteDoor, RouteForm};
use super::routes::{managed_route, test_notice};
use super::workspace::workspace_page;
use super::{
    refusal, Caller, ADDRESS_REFUSED, BOT_IN_ROOM, LOG_TARGET, MATRIX_NOT_CONNECTED,
    MATRIX_TOKEN_REFUSED, MATRIX_UNREACHABLE, NO_CHAT_CONNECTION, WORKSPACE_PATH,
};
use crate::bootstrap::resource_not_found_page;
use crate::integrations::matrix::{ConnectRefusal, MatrixBot, RoomFacts, RoomRefusal};
use crate::integrations::{credential_slot, matrix};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use foundry_integrations::{
    encode_matrix, test_message, ChannelKind, DestinationEnrolment, DestinationRef, ReasonCode,
};
use foundry_services::integrations::{ManagedRoute, MatrixLogin, RouteDraft};
use foundry_store::integrations::ConnectionWriteOutcome;
use tower_sessions::Session;

/// After a Matrix route's delete: the bot leaves its room when no other
/// route of the workspace uses it. A leave that fails goes to the log; the
/// delete stands and the room keeps what was posted.
pub(super) async fn leave_unrouted_room(state: &AppState, caller: &Caller, managed: &ManagedRoute) {
    let route = &managed.route;
    if ChannelKind::from_wire_name(&route.kind) != Some(ChannelKind::Matrix) {
        return;
    }
    let still_routed = caller
        .services
        .room_still_routed(&caller.principal(), &route.kind, &route.destination_ref)
        .await;
    if !matches!(still_routed, Ok(false)) {
        return;
    }
    let bot = managed
        .target
        .matrix_connection
        .as_ref()
        .and_then(|connection| state.integrations.current().matrix_bot(connection));
    let left = match bot {
        Some(bot) => bot
            .withdraw(&DestinationRef(route.destination_ref.clone()))
            .await
            .map_err(|reason| reason.wire_name()),
        None => Err("no_bot"),
    };
    if let Err(reason) = left {
        tracing::warn!(
            target: LOG_TARGET,
            reason,
            "matrix route delete: the bot did not leave the room; the route is deleted"
        );
    }
}

/// `POST /workspace/integrations/matrix` (admins): the homeserver's address
/// is judged before it is contacted, the token checked with `whoami`, and
/// only an accepted token is sealed and saved.
pub async fn connect_matrix(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let caller = match Caller::admin(&state, &session).await {
        Ok(caller) => caller,
        Err(response) => return response,
    };
    let field = |name: &str| {
        fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default()
    };
    let (entered, token) = (field("homeserver"), field("access_token"));
    let notice = match save_matrix(&state, &caller, &entered, token).await {
        Ok(()) => return Redirect::to(WORKSPACE_PATH).into_response(),
        Err(notice) => notice,
    };
    workspace_page(&state, &caller, &headers, Some(&notice)).await
}

/// Check the bot's token at the homeserver, seal it for its row and save
/// the connection; else the words that say why nothing was saved.
async fn save_matrix(
    state: &AppState,
    caller: &Caller,
    entered: &str,
    token: String,
) -> Result<(), String> {
    let integrations = state.integrations.current();
    let unreachable = || MATRIX_UNREACHABLE.replace("{homeserver}", entered);
    let Some(secrets) = integrations.settings().secrets.as_ref() else {
        return Err(MATRIX_NOT_CONNECTED.to_string());
    };
    let Some((homeserver, host)) = matrix::homeserver_address(entered) else {
        return Err(ADDRESS_REFUSED.to_string());
    };
    let token = secrecy::SecretString::from(token);
    let bot = MatrixBot::new(integrations.outbound().clone(), &homeserver, token.clone());
    let bot_user_id = bot.whoami().await.map_err(|refusal| {
        tracing::warn!(target: LOG_TARGET, reason = ?refusal, "matrix connect: not connected");
        match refusal {
            ConnectRefusal::Address => ADDRESS_REFUSED.to_string(),
            ConnectRefusal::TokenRefused => MATRIX_TOKEN_REFUSED.to_string(),
            ConnectRefusal::Unreachable => unreachable(),
        }
    })?;
    let principal = caller.principal();
    let connection_id = caller
        .services
        .matrix_connection_id(&principal)
        .await
        .map_err(|_| MATRIX_NOT_CONNECTED.to_string())?;
    let sealed = secrets
        .seal(&token, &credential_slot(connection_id))
        .map_err(|_| MATRIX_NOT_CONNECTED.to_string())?;
    let login = MatrixLogin {
        homeserver,
        host,
        bot_user_id,
    };
    match caller
        .services
        .connect_matrix(&principal, login, connection_id, sealed, state.clock.now())
        .await
    {
        Ok(ConnectionWriteOutcome::Connected | ConnectionWriteOutcome::Reconnected) => Ok(()),
        _ => Err(MATRIX_NOT_CONNECTED.to_string()),
    }
}

/// Save a Matrix route: bring the bot into the room, read the room, and save
/// the route only for an unencrypted room the bot joined; then say so on the
/// project's page. A refused room is explained on the form.
pub(super) async fn create_matrix_route(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    door: &RouteDoor,
    form: &RouteForm,
) -> Response {
    let refused = |notice: String| async move {
        door.form(
            state,
            caller,
            headers,
            form,
            FormAnswer::with(Some(&notice), StatusCode::UNPROCESSABLE_ENTITY),
        )
        .await
    };
    let Some(bot) = door.matrix.as_ref() else {
        return refused(NO_CHAT_CONNECTION.to_string()).await;
    };
    let room = form.destination.trim().to_string();
    let (room_id, facts) = match enrol_in_room(caller, bot, &room).await {
        Ok(joined) => joined,
        Err(refusal) => return refused(door.room_refusal(refusal, &room)).await,
    };
    let draft = RouteDraft {
        kind: foundry_services::integrations::MATRIX.to_string(),
        destination_ref: room_id.0.clone(),
        destination_label: room.clone(),
        events: form.events.clone(),
        content_level: form.content_level.clone(),
        lanes: form.lanes.clone(),
        shared_confirmed: false,
    };
    let saved = caller
        .services
        .create_route(
            &caller.principal(),
            &door.team_slug,
            &door.project_slug,
            draft,
            state.clock.now(),
        )
        .await;
    if let Err(err) = saved {
        withdraw_unless_routed(caller, bot, &room_id).await;
        return refusal(err);
    }
    let notice = BOT_IN_ROOM
        .replace("{room}", &room)
        .replace("{name}", &facts.name)
        .replace("{members}", &facts.members.to_string());
    door.project_page(state, caller, headers, notice).await
}

/// Bring `bot` into `room` and read it: the room's id and what it shows, for
/// an unencrypted room the bot joined. Else why not; the bot leaves a room it
/// joined but cannot use, unless another route keeps it there.
async fn enrol_in_room(
    caller: &Caller,
    bot: &MatrixBot,
    room: &str,
) -> Result<(DestinationRef, RoomFacts), RoomRefusal> {
    let room_id = DestinationRef(bot.room_id(room).await?);
    let joined = match bot.enrol(&room_id).await {
        Ok(()) => bot.room(&room_id.0).await,
        Err(ReasonCode::BotNotInRoom) => Err(RoomRefusal::NotInvited),
        Err(_) => Err(RoomRefusal::Unreachable),
    };
    match joined {
        Ok(facts) if !facts.encrypted => Ok((room_id, facts)),
        other => {
            let refusal = other.map_or_else(|refusal| refusal, |_| RoomRefusal::Encrypted);
            if refusal != RoomRefusal::NotInvited {
                withdraw_unless_routed(caller, bot, &room_id).await;
            }
            Err(refusal)
        }
    }
}

/// After a Matrix save that failed: the bot leaves `room_id` only when no
/// route of the workspace uses it (a room another project routes to keeps
/// its bot), and never when that cannot be read.
async fn withdraw_unless_routed(caller: &Caller, bot: &MatrixBot, room_id: &DestinationRef) {
    let still_routed = caller
        .services
        .room_still_routed(
            &caller.principal(),
            foundry_services::integrations::MATRIX,
            &room_id.0,
        )
        .await;
    if matches!(still_routed, Ok(false)) {
        let _ = bot.withdraw(room_id).await;
    }
}

/// Send test on a saved Matrix route: K-7 once to its room, as a fresh
/// transaction recorded nowhere; then back to the project's page.
pub(super) async fn send_matrix_test(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    door: &RouteDoor,
    route_id: &str,
) -> Response {
    let managed = managed_route(caller, &door.team_slug, &door.project_slug, route_id).await;
    let ManagedRoute { route, .. } = match managed {
        Ok(managed) => managed,
        Err(response) => return response,
    };
    let (Some(ChannelKind::Matrix), Some(bot)) = (
        ChannelKind::from_wire_name(&route.kind),
        door.matrix.as_ref(),
    ) else {
        return resource_not_found_page();
    };
    let message = encode_matrix(&test_message(
        &door.target.project_name,
        &door.target.key_prefix,
    ));
    let txn_id = uuid::Uuid::new_v4().to_string();
    let outcome = bot.send(&route.destination_ref, &message, &txn_id).await;
    let notice = test_notice(outcome, &route.destination_label);
    door.project_page(state, caller, headers, notice).await
}
