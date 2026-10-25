//! The Matrix adapter (DDR-13, ADR-EIP-006): a thin client for the
//! client-server API, sent only through [`OutboundHttp`], so every address it
//! reaches has passed the outbound address policy.
//!
//! Extension Justification (quality-framework Mandate against Parallel
//! Implementations):
//!   WHY-NEW-FILE: crates/foundry-app/src/integrations/matrix.rs
//!   CLOSEST-EXISTING: integrations/slack.rs (the Slack adapter).
//!   EXTENSION-COST: Slack's adapter talks to one operator-set host with its
//!     own client and OAuth; Matrix talks to an admin-supplied homeserver
//!     through the judging client, with a bot token and room membership.
//!   PARALLEL-RATIONALE: one adapter set per channel kind is the plugin
//!     contract (DDR-1, ADR-EIP-001); both implement the same core ports.
//!
//! - [`MatrixBot::whoami`] checks a bot token at connect (decision 1);
//! - [`MatrixBot::room_id`], [`MatrixBot::room`] read a room (alias, name,
//!   members, encryption), and the [`DestinationEnrolment`] port joins and
//!   leaves it (decision 2);
//! - [`DeliveryChannel::deliver`] re-reads the room's encryption, then sends
//!   one `m.notice` with the delivery id as its `txnId` (decisions 3-4);
//!   [`classify_matrix`] reads the answer (decision 5).
//!
//! The token stays inside the bot; no log line carries it.

use super::outbound::{OutboundError, OutboundHttp};
use async_trait::async_trait;
use foundry_integrations::{
    classify_matrix, ChannelKind, ChannelOutcome, DeliveryChannel, DeliveryKey,
    DestinationEnrolment, DestinationRef, EncodedMessage, MatrixAnswer, ReasonCode,
};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;

const CLIENT_API: &str = "/_matrix/client/v3";

/// Why a bot token was not saved (US-EIP-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectRefusal {
    /// The outbound address policy refuses the homeserver's address.
    Address,
    /// No usable answer: unreachable, TLS failed, a redirect, not Matrix.
    Unreachable,
    /// The homeserver answered and did not accept the token.
    TokenRefused,
}

/// Why a room cannot take a route (US-EIP-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomRefusal {
    /// No such room, or the bot may not join it (not invited).
    NotInvited,
    Encrypted,
    Unreachable,
}

impl RoomRefusal {
    fn reason(self) -> ReasonCode {
        match self {
            Self::NotInvited => ReasonCode::BotNotInRoom,
            Self::Encrypted => ReasonCode::RoomEncrypted,
            Self::Unreachable => ReasonCode::HomeserverNoAnswer,
        }
    }
}

/// What a joined room shows the bot: the route save's confirmation line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomFacts {
    pub name: String,
    pub members: usize,
    pub encrypted: bool,
}

/// A homeserver address as an admin enters it: `https` only, nothing after
/// the host and port. Returns the address Foundry calls and its host.
pub fn homeserver_address(entered: &str) -> Option<(String, String)> {
    let url = reqwest::Url::parse(entered.trim()).ok()?;
    let plain = url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && matches!(url.path(), "" | "/");
    if !plain {
        return None;
    }
    let host = url.host_str()?.to_string();
    let address = match url.port() {
        Some(port) => format!("https://{host}:{port}"),
        None => format!("https://{host}"),
    };
    Some((address, host))
}

/// One connection's bot on its homeserver.
#[derive(Clone)]
pub struct MatrixBot {
    http: OutboundHttp,
    homeserver: String,
    token: SecretString,
}

impl MatrixBot {
    pub fn new(http: OutboundHttp, homeserver: &str, token: SecretString) -> Self {
        Self {
            http,
            homeserver: homeserver.trim_end_matches('/').to_string(),
            token,
        }
    }

    /// One client-server call: its status and its body (`None` when not
    /// JSON).
    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Option<Value>), OutboundError> {
        let url = format!("{}{CLIENT_API}{path}", self.homeserver);
        let mut request = self
            .http
            .request(method, &url)
            .bearer_auth(self.token.expose_secret());
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = self.http.send(request).await?;
        let status = response.status().as_u16();
        let body = response.json::<Value>().await.ok();
        Ok((status, body))
    }

    /// The bot's user id, when the homeserver accepts its token.
    pub async fn whoami(&self) -> Result<String, ConnectRefusal> {
        match self
            .call(reqwest::Method::GET, "/account/whoami", None)
            .await
        {
            Err(OutboundError::Refused) => Err(ConnectRefusal::Address),
            Err(_) => Err(ConnectRefusal::Unreachable),
            Ok((401 | 403, Some(body))) if body["errcode"].is_string() => {
                Err(ConnectRefusal::TokenRefused)
            }
            Ok((200, Some(body))) => body["user_id"]
                .as_str()
                .map(str::to_string)
                .ok_or(ConnectRefusal::Unreachable),
            Ok(_) => Err(ConnectRefusal::Unreachable),
        }
    }

    /// The room an alias (`#…`) names, or a room id (`!…`) as it is.
    pub async fn room_id(&self, room: &str) -> Result<String, RoomRefusal> {
        let room = room.trim();
        if room.starts_with('!') {
            return Ok(room.to_string());
        }
        if !room.starts_with('#') {
            return Err(RoomRefusal::NotInvited);
        }
        let path = format!("/directory/room/{}", urlencoding::encode(room));
        match self.call(reqwest::Method::GET, &path, None).await {
            Ok((200, Some(body))) => body["room_id"]
                .as_str()
                .map(str::to_string)
                .ok_or(RoomRefusal::Unreachable),
            Ok((404, _)) => Err(RoomRefusal::NotInvited),
            _ => Err(RoomRefusal::Unreachable),
        }
    }

    /// One state event of a room: `Ok(Some(content))`, `Ok(None)` when the
    /// room has none, else the homeserver's answer.
    async fn state(
        &self,
        room_id: &str,
        event_type: &str,
    ) -> Result<Option<Value>, Option<(u16, Option<Value>)>> {
        let path = format!(
            "/rooms/{}/state/{event_type}/",
            urlencoding::encode(room_id)
        );
        match self.call(reqwest::Method::GET, &path, None).await {
            Ok((200, body)) => Ok(Some(body.unwrap_or(Value::Null))),
            Ok((404, _)) => Ok(None),
            Ok(answer) => Err(Some(answer)),
            Err(_) => Err(None),
        }
    }

    /// What a room the bot is in shows: its name, its members, whether it is
    /// encrypted.
    pub async fn room(&self, room_id: &str) -> Result<RoomFacts, RoomRefusal> {
        let encrypted = self
            .state(room_id, "m.room.encryption")
            .await
            .map_err(|_| RoomRefusal::Unreachable)?
            .is_some();
        let name = self
            .state(room_id, "m.room.name")
            .await
            .map_err(|_| RoomRefusal::Unreachable)?
            .and_then(|content| content["name"].as_str().map(str::to_string))
            .unwrap_or_default();
        let path = format!("/rooms/{}/joined_members", urlencoding::encode(room_id));
        let members = match self.call(reqwest::Method::GET, &path, None).await {
            Ok((200, Some(body))) => body["joined"].as_object().map_or(0, |m| m.len()),
            _ => return Err(RoomRefusal::Unreachable),
        };
        Ok(RoomFacts {
            name,
            members,
            encrypted,
        })
    }

    async fn join(&self, room_id: &str) -> Result<(), RoomRefusal> {
        let path = format!("/join/{}", urlencoding::encode(room_id));
        match self
            .call(reqwest::Method::POST, &path, Some(&serde_json::json!({})))
            .await
        {
            Ok((200, _)) => Ok(()),
            Ok((403 | 404, _)) => Err(RoomRefusal::NotInvited),
            _ => Err(RoomRefusal::Unreachable),
        }
    }

    async fn leave(&self, room_id: &str) -> Result<(), RoomRefusal> {
        let path = format!("/rooms/{}/leave", urlencoding::encode(room_id));
        match self
            .call(reqwest::Method::POST, &path, Some(&serde_json::json!({})))
            .await
        {
            Ok((200, _)) => Ok(()),
            _ => Err(RoomRefusal::Unreachable),
        }
    }

    /// Send `message` to `room_id` as transaction `txn_id`, after reading
    /// the room's encryption again: an encrypted room is never sent to.
    pub async fn send(
        &self,
        room_id: &str,
        message: &EncodedMessage,
        txn_id: &str,
    ) -> ChannelOutcome {
        match self.state(room_id, "m.room.encryption").await {
            Ok(Some(_)) => {
                return ChannelOutcome::RouteRefused {
                    reason: ReasonCode::RoomEncrypted,
                }
            }
            Ok(None) => {}
            Err(Some((status, body))) => {
                return classify_matrix(Some(MatrixAnswer {
                    status,
                    body: body.as_ref(),
                }))
            }
            Err(None) => return classify_matrix(None),
        }
        let path = format!(
            "/rooms/{}/send/m.room.message/{}",
            urlencoding::encode(room_id),
            urlencoding::encode(txn_id)
        );
        match self
            .call(reqwest::Method::PUT, &path, Some(&message.0))
            .await
        {
            Ok((status, body)) => classify_matrix(Some(MatrixAnswer {
                status,
                body: body.as_ref(),
            })),
            Err(_) => classify_matrix(None),
        }
    }
}

#[async_trait]
impl DeliveryChannel for MatrixBot {
    fn kind(&self) -> ChannelKind {
        ChannelKind::Matrix
    }

    /// The delivery id is the `txnId`: a retried send is one message.
    async fn deliver(
        &self,
        destination: &DestinationRef,
        message: &EncodedMessage,
        key: DeliveryKey,
    ) -> ChannelOutcome {
        self.send(&destination.0, message, &key.0.to_string()).await
    }

    async fn probe(&self) -> ChannelOutcome {
        match self.whoami().await {
            Ok(_) => ChannelOutcome::Delivered,
            Err(ConnectRefusal::TokenRefused) => ChannelOutcome::ConnectionRefused {
                reason: ReasonCode::TokenRevoked,
            },
            Err(_) => classify_matrix(None),
        }
    }
}

/// The bot's membership of one room: join on route save, leave on delete.
#[async_trait]
impl DestinationEnrolment for MatrixBot {
    async fn enrol(&self, destination: &DestinationRef) -> Result<(), ReasonCode> {
        self.join(&destination.0).await.map_err(RoomRefusal::reason)
    }

    async fn withdraw(&self, destination: &DestinationRef) -> Result<(), ReasonCode> {
        self.leave(&destination.0)
            .await
            .map_err(RoomRefusal::reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_homeserver_is_an_https_address_and_nothing_more() {
        assert_eq!(
            homeserver_address("https://matrix.canzan.dev"),
            Some((
                "https://matrix.canzan.dev".into(),
                "matrix.canzan.dev".into()
            ))
        );
        assert_eq!(
            homeserver_address(" https://matrix.canzan.dev/ "),
            Some((
                "https://matrix.canzan.dev".into(),
                "matrix.canzan.dev".into()
            ))
        );
        assert_eq!(
            homeserver_address("https://127.0.0.1:8448"),
            Some(("https://127.0.0.1:8448".into(), "127.0.0.1".into()))
        );
        for refused in [
            "http://matrix.canzan.dev",
            "https://matrix.canzan.dev/_matrix",
            "https://bot@matrix.canzan.dev",
            "https://matrix.canzan.dev?x=1",
            "https://matrix.canzan.dev#x",
            "matrix.canzan.dev",
            "",
        ] {
            assert_eq!(homeserver_address(refused), None, "{refused}");
        }
    }

    #[test]
    fn a_room_refusal_reads_as_its_reason() {
        assert_eq!(RoomRefusal::NotInvited.reason(), ReasonCode::BotNotInRoom);
        assert_eq!(RoomRefusal::Encrypted.reason(), ReasonCode::RoomEncrypted);
        assert_eq!(
            RoomRefusal::Unreachable.reason(),
            ReasonCode::HomeserverNoAnswer
        );
    }
}
