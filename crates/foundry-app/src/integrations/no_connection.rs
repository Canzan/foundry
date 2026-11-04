//! integrations-wayfinding — why a route door has no connection to use, and
//! what it says about it (ADR-IW-001). The cause comes from the instance's
//! offer for the form's kind ([`Integrations::slack_offer`],
//! [`Integrations::matrix_offer`]) and whether the workspace has a connection
//! of that kind this Foundry can use; its words depend on whether the viewer
//! is a workspace admin. Every no-connection message on the route form comes
//! from here, and so does the project page's connection line (W-6, W-7).
//!
//! [`Integrations::slack_offer`]: super::Integrations::slack_offer
//! [`Integrations::matrix_offer`]: super::Integrations::matrix_offer

use super::handlers::{
    lost_words, unavailable_words, NO_CHAT_CONNECTION, SETUP_PATH, WORKSPACE_PATH,
};
use super::{MatrixOffer, SlackOffer};
use foundry_integrations::{ChannelKind, ConnectionState};
use foundry_services::integrations::LostSlack;
use foundry_store::integrations::ConnectionRow;

/// W-2 for Slack: no Foundry Slack app on this instance.
pub const SLACK_NOT_SET_UP: &str =
    "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once.";
/// W-2's fix for Slack, offered to a workspace admin.
pub const HOW_TO_SET_UP_SLACK: &str = "How to set up Slack";
/// W-2 for Matrix: no secrets key, so no bot token can be sealed.
pub const MATRIX_NOT_SET_UP: &str =
    "Matrix is not set up on this Foundry yet. An instance operator sets it up once.";
/// W-3: the Slack app is set up, the secrets key is not.
pub const SLACK_NO_SECRETS_KEY: &str =
    "Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key.";
/// W-4, for the form's `{kind}`: told to a workspace admin.
pub const NO_KIND_CONNECTION: &str = "This workspace has no {kind} connection yet.";
/// W-4's fix, for the form's `{kind}`.
pub const CONNECT_KIND: &str = "Connect {kind} in workspace integrations";

/// W-6, for the connection's `{kind}` and its `{label}` (a Slack workspace's
/// name, a Matrix homeserver's host).
pub const ROUTES_USE_CONNECTION: &str =
    "Routes here use the workspace's {kind} connection to {label}.";
/// W-6's way to the workspace page, offered to a workspace admin.
pub const MANAGE_WORKSPACE_INTEGRATIONS: &str = "Manage workspace integrations";
/// W-7: told to a workspace admin when the workspace has no chat connection.
pub const NO_CHAT_CONNECTION_YET: &str = "This workspace has no chat connection yet.";
/// W-7's fix.
pub const CONNECT_SLACK_OR_MATRIX: &str = "Connect Slack or Matrix in workspace integrations";

/// What a viewer reads about a connection, in a refused form's place or as a
/// project page's connection line: the words, and a way on where one is
/// offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionWords {
    pub words: String,
    pub fix: Option<FixLink>,
}

/// A way out of what the words describe: a link's words and where it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixLink {
    pub label: String,
    pub href: &'static str,
}

impl FixLink {
    fn new(label: impl Into<String>, href: &'static str) -> Self {
        Self {
            label: label.into(),
            href,
        }
    }
}

/// Why there is no connection of a kind to make a route through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoConnectionCause {
    /// W-2: the instance does not offer the kind at all.
    NotSetUp(ChannelKind),
    /// W-3: the instance has the app but no secrets key.
    NoSecretsKey(ChannelKind),
    /// K-19: the workspace's saved connection does not open here.
    Unreadable(ChannelKind),
    /// W-4 (admin) or K-4 (lead): the workspace has not connected the kind.
    NotConnected(ChannelKind),
}

impl NoConnectionCause {
    /// The cause for a Slack route under `offer`, `None` when the workspace
    /// has a `connected` Slack connection (a lost one included).
    pub fn of_slack(offer: SlackOffer, connected: bool) -> Option<Self> {
        let kind = ChannelKind::Slack;
        match offer {
            SlackOffer::NotSetUp => Some(Self::NotSetUp(kind)),
            SlackOffer::Unavailable => Some(Self::Unreadable(kind)),
            SlackOffer::NoSecretsKey => Some(Self::NoSecretsKey(kind)),
            SlackOffer::Ready => (!connected).then_some(Self::NotConnected(kind)),
        }
    }

    /// The cause for a Matrix route under `offer`, `None` when the workspace
    /// has a `connected` Matrix connection. Without a secrets key Matrix is
    /// not set up: nothing else of it needs an operator.
    pub fn of_matrix(offer: MatrixOffer, connected: bool) -> Option<Self> {
        let kind = ChannelKind::Matrix;
        match offer {
            MatrixOffer::Unavailable => Some(Self::Unreadable(kind)),
            MatrixOffer::NoSecretsKey => Some(Self::NotSetUp(kind)),
            MatrixOffer::Ready => (!connected).then_some(Self::NotConnected(kind)),
        }
    }

    /// What a viewer reads for this cause, and the fix a workspace admin is
    /// offered where one exists. A lead keeps K-4 for an unconnected
    /// workspace, since only an admin can connect it.
    pub fn words(self, viewer_is_workspace_admin: bool) -> ConnectionWords {
        let admin = viewer_is_workspace_admin;
        let (words, fix) = match self {
            Self::NotSetUp(ChannelKind::Matrix) | Self::NoSecretsKey(ChannelKind::Matrix) => {
                (MATRIX_NOT_SET_UP.to_string(), None)
            }
            Self::NotSetUp(_) => (
                SLACK_NOT_SET_UP.to_string(),
                admin.then(|| FixLink::new(HOW_TO_SET_UP_SLACK, SETUP_PATH)),
            ),
            Self::NoSecretsKey(_) => (SLACK_NO_SECRETS_KEY.to_string(), None),
            Self::Unreadable(kind) => (unavailable_words(kind), None),
            Self::NotConnected(kind) if admin => (
                NO_KIND_CONNECTION.replace("{kind}", kind.display_name()),
                Some(FixLink::new(
                    CONNECT_KIND.replace("{kind}", kind.display_name()),
                    WORKSPACE_PATH,
                )),
            ),
            Self::NotConnected(_) => (NO_CHAT_CONNECTION.to_string(), None),
        };
        ConnectionWords { words, fix }
    }
}

/// The workspace's connection of one kind, as the project page tells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedConnection {
    /// W-6: routes post through it; `label` names what it connects to.
    Connected { label: String },
    /// K-10: the vendor took it away; routes are paused.
    Lost(LostSlack),
    /// K-19: saved, but this Foundry cannot read it.
    Unreadable,
}

impl SharedConnection {
    /// How the page tells `row` (not disconnected), whose credential this
    /// Foundry `opens` or not. A lost connection reads as lost either way.
    /// Without a sealed connection to open (`None`) there is no line: the
    /// route door reads that as not connected, and so does the page.
    pub fn of(row: &ConnectionRow, opens: Option<bool>) -> Option<Self> {
        let opens = opens?;
        Some(
            if ConnectionState::from_wire_name(&row.state) == Some(ConnectionState::Lost) {
                Self::Lost(LostSlack {
                    workspace_name: row.workspace_name.clone(),
                    reason: row.lost_reason.clone(),
                })
            } else if opens {
                Self::Connected {
                    label: row.label.clone(),
                }
            } else {
                Self::Unreadable
            },
        )
    }

    fn line(&self, kind: ChannelKind, viewer_is_workspace_admin: bool) -> ConnectionWords {
        let (words, fix) = match self {
            Self::Connected { label } => (
                ROUTES_USE_CONNECTION
                    .replace("{kind}", kind.display_name())
                    .replace("{label}", label),
                viewer_is_workspace_admin
                    .then(|| FixLink::new(MANAGE_WORKSPACE_INTEGRATIONS, WORKSPACE_PATH)),
            ),
            Self::Lost(lost) => (lost_words(kind, lost), None),
            Self::Unreadable => (unavailable_words(kind), None),
        };
        ConnectionWords { words, fix }
    }
}

/// The project page's connection lines: one per kind the workspace has
/// connected, in the order given; when it has none, W-7 and its fix for a
/// workspace admin, K-4 for a lead, since only an admin can connect one.
pub fn project_lines(
    connections: &[(ChannelKind, SharedConnection)],
    viewer_is_workspace_admin: bool,
) -> Vec<ConnectionWords> {
    if connections.is_empty() {
        let line = if viewer_is_workspace_admin {
            ConnectionWords {
                words: NO_CHAT_CONNECTION_YET.to_string(),
                fix: Some(FixLink::new(CONNECT_SLACK_OR_MATRIX, WORKSPACE_PATH)),
            }
        } else {
            ConnectionWords {
                words: NO_CHAT_CONNECTION.to_string(),
                fix: None,
            }
        };
        return vec![line];
    }
    connections
        .iter()
        .map(|(kind, connection)| connection.line(*kind, viewer_is_workspace_admin))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const W2_SLACK: &str =
        "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once.";
    const W2_MATRIX: &str =
        "Matrix is not set up on this Foundry yet. An instance operator sets it up once.";
    const W3: &str =
        "Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key.";
    const K4: &str =
        "This workspace has no chat connection yet. Ask a workspace admin to connect Slack or Matrix.";
    const K19_SLACK: &str = "Slack is unavailable: this Foundry cannot read its saved connection. An instance operator needs to check the secrets key.";
    const K19_MATRIX: &str = "Matrix is unavailable: this Foundry cannot read its saved connection. An instance operator needs to check the secrets key.";
    const SETUP: Option<(&str, &str)> =
        Some(("How to set up Slack", "/workspace/integrations/slack/setup"));

    /// The instance's offer for a form's kind.
    #[derive(Debug, Clone, Copy)]
    enum Offer {
        Slack(SlackOffer),
        Matrix(MatrixOffer),
    }

    /// What a viewer reads: the words, and the fix link's label and href.
    type Read = (String, Option<(String, String)>);

    const ADMIN: bool = true;
    const LEAD: bool = false;
    const CONNECTED: bool = true;
    const NONE: bool = false;

    /// Every (offer, connected, role): what the viewer reads, and the fix
    /// link they are offered. `None` words: the form shows.
    #[rustfmt::skip]
    fn table() -> Vec<(Offer, bool, bool, Option<Read>)> {
        use MatrixOffer as M;
        use Offer::{Matrix, Slack};
        use SlackOffer as S;
        let read = |words: &str, link: Option<(&str, &str)>| {
            Some((words.to_string(), link.map(|(l, h)| (l.to_string(), h.to_string()))))
        };
        let connect = |kind: &str| {
            read(
                &format!("This workspace has no {kind} connection yet."),
                Some((&format!("Connect {kind} in workspace integrations"), "/workspace/integrations")),
            )
        };
        vec![
            (Slack(S::NotSetUp), NONE, ADMIN, read(W2_SLACK, SETUP)),
            (Slack(S::NotSetUp), NONE, LEAD, read(W2_SLACK, None)),
            (Slack(S::NotSetUp), CONNECTED, ADMIN, read(W2_SLACK, SETUP)),
            (Slack(S::NotSetUp), CONNECTED, LEAD, read(W2_SLACK, None)),
            (Slack(S::NoSecretsKey), NONE, ADMIN, read(W3, None)),
            (Slack(S::NoSecretsKey), NONE, LEAD, read(W3, None)),
            (Slack(S::NoSecretsKey), CONNECTED, ADMIN, read(W3, None)),
            (Slack(S::NoSecretsKey), CONNECTED, LEAD, read(W3, None)),
            (Slack(S::Unavailable), NONE, ADMIN, read(K19_SLACK, None)),
            (Slack(S::Unavailable), NONE, LEAD, read(K19_SLACK, None)),
            (Slack(S::Unavailable), CONNECTED, ADMIN, read(K19_SLACK, None)),
            (Slack(S::Unavailable), CONNECTED, LEAD, read(K19_SLACK, None)),
            (Slack(S::Ready), NONE, ADMIN, connect("Slack")),
            (Slack(S::Ready), NONE, LEAD, read(K4, None)),
            (Slack(S::Ready), CONNECTED, ADMIN, None),
            (Slack(S::Ready), CONNECTED, LEAD, None),
            (Matrix(M::NoSecretsKey), NONE, ADMIN, read(W2_MATRIX, None)),
            (Matrix(M::NoSecretsKey), NONE, LEAD, read(W2_MATRIX, None)),
            (Matrix(M::NoSecretsKey), CONNECTED, ADMIN, read(W2_MATRIX, None)),
            (Matrix(M::NoSecretsKey), CONNECTED, LEAD, read(W2_MATRIX, None)),
            (Matrix(M::Unavailable), NONE, ADMIN, read(K19_MATRIX, None)),
            (Matrix(M::Unavailable), NONE, LEAD, read(K19_MATRIX, None)),
            (Matrix(M::Unavailable), CONNECTED, ADMIN, read(K19_MATRIX, None)),
            (Matrix(M::Unavailable), CONNECTED, LEAD, read(K19_MATRIX, None)),
            (Matrix(M::Ready), NONE, ADMIN, connect("Matrix")),
            (Matrix(M::Ready), NONE, LEAD, read(K4, None)),
            (Matrix(M::Ready), CONNECTED, ADMIN, None),
            (Matrix(M::Ready), CONNECTED, LEAD, None),
        ]
    }

    fn cause(offer: Offer, connected: bool) -> Option<NoConnectionCause> {
        match offer {
            Offer::Slack(offer) => NoConnectionCause::of_slack(offer, connected),
            Offer::Matrix(offer) => NoConnectionCause::of_matrix(offer, connected),
        }
    }

    #[test]
    fn every_offer_connection_and_role_reads_its_locked_words() {
        let rows = table();
        assert_eq!(rows.len(), 4 * 2 * 2 + 3 * 2 * 2);
        for (offer, connected, admin, expected) in rows {
            let read = cause(offer, connected).map(|cause| {
                let said = cause.words(admin);
                (
                    said.words,
                    said.fix.map(|fix| (fix.label, fix.href.to_string())),
                )
            });
            assert_eq!(
                read, expected,
                "{offer:?}, connected {connected}, admin {admin}"
            );
            // An admin is never told to ask an admin, nor about the other kind.
            if let Some((words, _)) = read.filter(|_| admin) {
                assert!(
                    !words.contains("Ask a workspace admin"),
                    "{offer:?}: {words}"
                );
                assert!(!words.contains("Slack or Matrix"), "{offer:?}: {words}");
            }
        }
    }

    const W6_SLACK: &str = "Routes here use the workspace's Slack connection to Canzan Personal.";
    const W6_MATRIX: &str =
        "Routes here use the workspace's Matrix connection to matrix.example.org.";
    const W7: &str = "This workspace has no chat connection yet.";
    const LOST_SLACK: &str = "Foundry lost its Slack connection for Canzan Labs: the Foundry app was removed from Slack. Routes are paused.";
    const LOST_MATRIX: &str = "Foundry lost its Matrix connection for Canzan Labs: Foundry's access was revoked. Routes are paused.";
    const MANAGE: Option<(&str, &str)> =
        Some(("Manage workspace integrations", "/workspace/integrations"));
    const CONNECT_EITHER: Option<(&str, &str)> = Some((
        "Connect Slack or Matrix in workspace integrations",
        "/workspace/integrations",
    ));

    fn lost(reason: &str) -> SharedConnection {
        SharedConnection::Lost(LostSlack {
            workspace_name: "Canzan Labs".to_string(),
            reason: Some(reason.to_string()),
        })
    }

    fn connected(label: &str) -> SharedConnection {
        SharedConnection::Connected {
            label: label.to_string(),
        }
    }

    /// A line as the table writes it: the words, and the fix's label and href.
    type Line = (&'static str, Option<(&'static str, &'static str)>);
    /// The workspace's connections, the viewer's role, and the lines read.
    type ProjectRow = (Vec<(ChannelKind, SharedConnection)>, bool, Vec<Line>);

    /// Every kind's connection state, each role, and none connected: the
    /// lines the project page shows.
    #[rustfmt::skip]
    fn project_table() -> Vec<ProjectRow> {
        use ChannelKind::{Matrix, Slack};
        use SharedConnection::Unreadable;
        vec![
            (vec![(Slack, connected("Canzan Personal"))], ADMIN, vec![(W6_SLACK, MANAGE)]),
            (vec![(Slack, connected("Canzan Personal"))], LEAD, vec![(W6_SLACK, None)]),
            (vec![(Slack, lost("app_removed"))], ADMIN, vec![(LOST_SLACK, None)]),
            (vec![(Slack, lost("app_removed"))], LEAD, vec![(LOST_SLACK, None)]),
            (vec![(Slack, Unreadable)], ADMIN, vec![(K19_SLACK, None)]),
            (vec![(Slack, Unreadable)], LEAD, vec![(K19_SLACK, None)]),
            (vec![(Matrix, connected("matrix.example.org"))], ADMIN, vec![(W6_MATRIX, MANAGE)]),
            (vec![(Matrix, connected("matrix.example.org"))], LEAD, vec![(W6_MATRIX, None)]),
            (vec![(Matrix, lost("token_revoked"))], ADMIN, vec![(LOST_MATRIX, None)]),
            (vec![(Matrix, lost("token_revoked"))], LEAD, vec![(LOST_MATRIX, None)]),
            (vec![(Matrix, Unreadable)], ADMIN, vec![(K19_MATRIX, None)]),
            (vec![(Matrix, Unreadable)], LEAD, vec![(K19_MATRIX, None)]),
            (vec![(Slack, connected("Canzan Personal")), (Matrix, Unreadable)], LEAD, vec![(W6_SLACK, None), (K19_MATRIX, None)]),
            (vec![], ADMIN, vec![(W7, CONNECT_EITHER)]),
            (vec![], LEAD, vec![(K4, None)]),
        ]
    }

    #[test]
    fn every_connection_state_and_role_reads_its_project_line() {
        for (connections, admin, expected) in project_table() {
            let lines: Vec<_> = project_lines(&connections, admin)
                .into_iter()
                .map(|line| {
                    (
                        line.words,
                        line.fix.map(|fix| (fix.label, fix.href.to_string())),
                    )
                })
                .collect();
            let expected: Vec<_> = expected
                .into_iter()
                .map(|(words, fix)| {
                    (
                        words.to_string(),
                        fix.map(|(label, href)| (label.to_string(), href.to_string())),
                    )
                })
                .collect();
            assert_eq!(lines, expected, "{connections:?}, admin {admin}");
        }
    }

    #[test]
    fn a_lost_connection_reads_as_lost_and_one_with_nothing_sealed_has_no_line() {
        let row = ConnectionRow {
            kind: "slack".to_string(),
            state: "lost".to_string(),
            lost_reason: Some("app_removed".to_string()),
            workspace_name: "Canzan Labs".to_string(),
            label: "Canzan Personal".to_string(),
            detail: None,
            connected_by_name: None,
            connected_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        let live = ConnectionRow {
            state: "connected".to_string(),
            ..row.clone()
        };
        // (row, whether its sealed credential opens, `None`: nothing sealed)
        let table = [
            (&row, Some(false), Some(lost("app_removed"))),
            (&row, Some(true), Some(lost("app_removed"))),
            (&row, None, None),
            (&live, Some(false), Some(SharedConnection::Unreadable)),
            (&live, Some(true), Some(connected("Canzan Personal"))),
            (&live, None, None),
        ];
        for (row, opens, expected) in table {
            assert_eq!(
                SharedConnection::of(row, opens),
                expected,
                "{} opens {opens:?}",
                row.state
            );
        }
    }

    #[test]
    fn a_matrix_no_secrets_key_cause_reads_matrix_not_set_up_with_no_fix() {
        let words = NoConnectionCause::NoSecretsKey(ChannelKind::Matrix).words(ADMIN);
        assert_eq!(words.words, W2_MATRIX);
        assert_eq!(words.fix, None);
    }
}
