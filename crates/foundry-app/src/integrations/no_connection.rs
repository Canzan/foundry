//! integrations-wayfinding — why a route door has no connection to use, and
//! what it says about it (ADR-IW-001). The cause comes from the instance's
//! offer for the form's kind ([`Integrations::slack_offer`],
//! [`Integrations::matrix_offer`]) and whether the workspace has a connection
//! of that kind this Foundry can use; its words and the ways out it offers
//! depend on the [`Viewer`]: whether the workspace's own pages are theirs to
//! open, and whether this Foundry's setup is (US-IIS-04). Every no-connection
//! message on the route form comes from here, and so does the project page's
//! connection line (W-6, W-7) and the workspace integrations page's W-2 and
//! W-3.
//!
//! [`Integrations::slack_offer`]: super::Integrations::slack_offer
//! [`Integrations::matrix_offer`]: super::Integrations::matrix_offer

use super::handlers::{
    lost_words, unavailable_words, INSTANCE_PATH, NO_CHAT_CONNECTION, SETUP_PATH, WORKSPACE_PATH,
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
/// I-19: the way to this Foundry's own setup, offered to an instance admin
/// beside a sentence only that setup can clear (US-IIS-04).
pub const SET_UP_INTEGRATIONS: &str = "Set up integrations for this Foundry";

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
/// project page's connection line: the words, and the ways on they are
/// offered, in reading order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionWords {
    pub words: String,
    pub fixes: Vec<FixLink>,
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

/// Who is reading, as far as the ways out they may be offered go: whether the
/// workspace's own pages are theirs to open, and whether this Foundry's setup
/// is. The workspace-admin fact comes from foundry-services with the page's
/// view, the instance-admin fact from this crate's authorization (the nav's,
/// fail-closed); the second never travels down to foundry-services
/// (ADR-IW-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewer {
    is_workspace_admin: bool,
    is_instance_admin: bool,
}

impl Viewer {
    pub const fn new(is_workspace_admin: bool, is_instance_admin: bool) -> Self {
        Self {
            is_workspace_admin,
            is_instance_admin,
        }
    }

    /// Whether the workspace's own pages and their actions are this viewer's
    /// to open: the Slack setup guide, Connect {kind}, Manage workspace
    /// integrations.
    fn manages_the_workspace(self) -> bool {
        self.is_workspace_admin
    }

    /// The ways out of a sentence the workspace itself can clear: its own
    /// `fix`, and only for the viewer who can open it.
    fn offers(self, fix: Option<FixLink>) -> Vec<FixLink> {
        Vec::from_iter(fix.filter(|_| self.is_workspace_admin))
    }

    /// The ways out of a sentence only this Foundry's setup can clear (W-2,
    /// W-3): the workspace's own `fix` where the viewer can open it, then
    /// I-19 where this Foundry's setup is theirs. The link a viewer is
    /// offered today keeps its place, so I-19 is always last (US-IIS-04).
    fn offers_with_setup(self, fix: Option<FixLink>) -> Vec<FixLink> {
        let mut offers = self.offers(fix);
        offers.extend(
            self.is_instance_admin
                .then(|| FixLink::new(SET_UP_INTEGRATIONS, INSTANCE_PATH)),
        );
        offers
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

    /// Whether only this Foundry's own setup can clear this cause (W-2, W-3).
    /// The workspace integrations page shows those sentences and keeps its own
    /// arms for a connection the workspace can make, has lost, or this Foundry
    /// cannot read.
    pub fn awaits_this_foundry(self) -> bool {
        matches!(self, Self::NotSetUp(_) | Self::NoSecretsKey(_))
    }

    /// What a `viewer` reads for this cause, and the ways out they are
    /// offered, in reading order: the workspace's own fix where one exists and
    /// is theirs, then I-19 for a cause only this Foundry's setup can clear.
    /// A lead keeps K-4 for an unconnected workspace, since only an admin can
    /// connect it.
    pub fn words(self, viewer: Viewer) -> ConnectionWords {
        let (words, fixes) = match self {
            Self::NotSetUp(ChannelKind::Matrix) | Self::NoSecretsKey(ChannelKind::Matrix) => (
                MATRIX_NOT_SET_UP.to_string(),
                viewer.offers_with_setup(None),
            ),
            Self::NotSetUp(_) => (
                SLACK_NOT_SET_UP.to_string(),
                viewer.offers_with_setup(Some(FixLink::new(HOW_TO_SET_UP_SLACK, SETUP_PATH))),
            ),
            Self::NoSecretsKey(_) => (
                SLACK_NO_SECRETS_KEY.to_string(),
                viewer.offers_with_setup(None),
            ),
            Self::Unreadable(kind) => (unavailable_words(kind), viewer.offers(None)),
            Self::NotConnected(kind) if viewer.manages_the_workspace() => (
                NO_KIND_CONNECTION.replace("{kind}", kind.display_name()),
                viewer.offers(Some(FixLink::new(
                    CONNECT_KIND.replace("{kind}", kind.display_name()),
                    WORKSPACE_PATH,
                ))),
            ),
            Self::NotConnected(_) => (NO_CHAT_CONNECTION.to_string(), viewer.offers(None)),
        };
        ConnectionWords { words, fixes }
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

    fn line(&self, kind: ChannelKind, viewer: Viewer) -> ConnectionWords {
        let (words, fixes) = match self {
            Self::Connected { label } => (
                ROUTES_USE_CONNECTION
                    .replace("{kind}", kind.display_name())
                    .replace("{label}", label),
                viewer.offers(Some(FixLink::new(
                    MANAGE_WORKSPACE_INTEGRATIONS,
                    WORKSPACE_PATH,
                ))),
            ),
            Self::Lost(lost) => (lost_words(kind, lost), viewer.offers(None)),
            Self::Unreadable => (unavailable_words(kind), viewer.offers(None)),
        };
        ConnectionWords { words, fixes }
    }
}

/// The project page's connection lines: one per kind the workspace has
/// connected, in the order given; when it has none, W-7 and its fix for a
/// workspace admin, K-4 for a lead, since only an admin can connect one.
pub fn project_lines(
    connections: &[(ChannelKind, SharedConnection)],
    viewer: Viewer,
) -> Vec<ConnectionWords> {
    if connections.is_empty() {
        let line = if viewer.manages_the_workspace() {
            ConnectionWords {
                words: NO_CHAT_CONNECTION_YET.to_string(),
                fixes: viewer.offers(Some(FixLink::new(CONNECT_SLACK_OR_MATRIX, WORKSPACE_PATH))),
            }
        } else {
            ConnectionWords {
                words: NO_CHAT_CONNECTION.to_string(),
                fixes: viewer.offers(None),
            }
        };
        return vec![line];
    }
    connections
        .iter()
        .map(|(kind, connection)| connection.line(*kind, viewer))
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
    /// The links a row offers, in order: W-2 Slack's guide, I-19, both, none.
    const SETUP: &[(&str, &str)] =
        &[("How to set up Slack", "/workspace/integrations/slack/setup")];
    const INSTANCE: &[(&str, &str)] = &[(
        "Set up integrations for this Foundry",
        "/admin/instance/integrations",
    )];
    const SETUP_THEN_INSTANCE: &[(&str, &str)] = &[
        ("How to set up Slack", "/workspace/integrations/slack/setup"),
        (
            "Set up integrations for this Foundry",
            "/admin/instance/integrations",
        ),
    ];
    const NOTHING: &[(&str, &str)] = &[];

    /// The instance's offer for a form's kind.
    #[derive(Debug, Clone, Copy)]
    enum Offer {
        Slack(SlackOffer),
        Matrix(MatrixOffer),
    }

    /// What a viewer reads: the words, and the fix links' labels and hrefs in
    /// the order they are offered.
    type Read = (String, Vec<(String, String)>);

    /// The four viewers a no-connection sentence can be read by: a workspace
    /// admin who is not this Foundry's instance admin, a lead who is neither,
    /// an instance admin who is not a workspace admin, and one who is both.
    const ADMIN: Viewer = Viewer::new(true, false);
    const LEAD: Viewer = Viewer::new(false, false);
    const INSTANCE_ADMIN: Viewer = Viewer::new(false, true);
    const BOTH: Viewer = Viewer::new(true, true);
    const CONNECTED: bool = true;
    const NONE: bool = false;

    /// Every (offer, connected, viewer): what the viewer reads, and the links
    /// they are offered in order. `None` words: the form shows.
    #[rustfmt::skip]
    fn table() -> Vec<(Offer, bool, Viewer, Option<Read>)> {
        use MatrixOffer as M;
        use Offer::{Matrix, Slack};
        use SlackOffer as S;
        let read = |words: &str, links: &[(&str, &str)]| {
            Some((
                words.to_string(),
                links.iter().map(|(l, h)| (l.to_string(), h.to_string())).collect(),
            ))
        };
        let connect = |kind: &str| {
            read(
                &format!("This workspace has no {kind} connection yet."),
                &[(&format!("Connect {kind} in workspace integrations"), "/workspace/integrations")],
            )
        };
        vec![
            (Slack(S::NotSetUp), NONE, ADMIN, read(W2_SLACK, SETUP)),
            (Slack(S::NotSetUp), NONE, LEAD, read(W2_SLACK, NOTHING)),
            (Slack(S::NotSetUp), NONE, INSTANCE_ADMIN, read(W2_SLACK, INSTANCE)),
            (Slack(S::NotSetUp), NONE, BOTH, read(W2_SLACK, SETUP_THEN_INSTANCE)),
            (Slack(S::NotSetUp), CONNECTED, ADMIN, read(W2_SLACK, SETUP)),
            (Slack(S::NotSetUp), CONNECTED, LEAD, read(W2_SLACK, NOTHING)),
            (Slack(S::NotSetUp), CONNECTED, INSTANCE_ADMIN, read(W2_SLACK, INSTANCE)),
            (Slack(S::NotSetUp), CONNECTED, BOTH, read(W2_SLACK, SETUP_THEN_INSTANCE)),
            (Slack(S::NoSecretsKey), NONE, ADMIN, read(W3, NOTHING)),
            (Slack(S::NoSecretsKey), NONE, LEAD, read(W3, NOTHING)),
            (Slack(S::NoSecretsKey), NONE, INSTANCE_ADMIN, read(W3, INSTANCE)),
            (Slack(S::NoSecretsKey), NONE, BOTH, read(W3, INSTANCE)),
            (Slack(S::NoSecretsKey), CONNECTED, ADMIN, read(W3, NOTHING)),
            (Slack(S::NoSecretsKey), CONNECTED, LEAD, read(W3, NOTHING)),
            (Slack(S::NoSecretsKey), CONNECTED, INSTANCE_ADMIN, read(W3, INSTANCE)),
            (Slack(S::NoSecretsKey), CONNECTED, BOTH, read(W3, INSTANCE)),
            (Slack(S::Unavailable), NONE, ADMIN, read(K19_SLACK, NOTHING)),
            (Slack(S::Unavailable), NONE, LEAD, read(K19_SLACK, NOTHING)),
            (Slack(S::Unavailable), NONE, INSTANCE_ADMIN, read(K19_SLACK, NOTHING)),
            (Slack(S::Unavailable), NONE, BOTH, read(K19_SLACK, NOTHING)),
            (Slack(S::Unavailable), CONNECTED, ADMIN, read(K19_SLACK, NOTHING)),
            (Slack(S::Unavailable), CONNECTED, LEAD, read(K19_SLACK, NOTHING)),
            (Slack(S::Unavailable), CONNECTED, INSTANCE_ADMIN, read(K19_SLACK, NOTHING)),
            (Slack(S::Unavailable), CONNECTED, BOTH, read(K19_SLACK, NOTHING)),
            (Slack(S::Ready), NONE, ADMIN, connect("Slack")),
            (Slack(S::Ready), NONE, LEAD, read(K4, NOTHING)),
            (Slack(S::Ready), NONE, INSTANCE_ADMIN, read(K4, NOTHING)),
            (Slack(S::Ready), NONE, BOTH, connect("Slack")),
            (Slack(S::Ready), CONNECTED, ADMIN, None),
            (Slack(S::Ready), CONNECTED, LEAD, None),
            (Slack(S::Ready), CONNECTED, INSTANCE_ADMIN, None),
            (Slack(S::Ready), CONNECTED, BOTH, None),
            (Matrix(M::NoSecretsKey), NONE, ADMIN, read(W2_MATRIX, NOTHING)),
            (Matrix(M::NoSecretsKey), NONE, LEAD, read(W2_MATRIX, NOTHING)),
            (Matrix(M::NoSecretsKey), NONE, INSTANCE_ADMIN, read(W2_MATRIX, INSTANCE)),
            (Matrix(M::NoSecretsKey), NONE, BOTH, read(W2_MATRIX, INSTANCE)),
            (Matrix(M::NoSecretsKey), CONNECTED, ADMIN, read(W2_MATRIX, NOTHING)),
            (Matrix(M::NoSecretsKey), CONNECTED, LEAD, read(W2_MATRIX, NOTHING)),
            (Matrix(M::NoSecretsKey), CONNECTED, INSTANCE_ADMIN, read(W2_MATRIX, INSTANCE)),
            (Matrix(M::NoSecretsKey), CONNECTED, BOTH, read(W2_MATRIX, INSTANCE)),
            (Matrix(M::Unavailable), NONE, ADMIN, read(K19_MATRIX, NOTHING)),
            (Matrix(M::Unavailable), NONE, LEAD, read(K19_MATRIX, NOTHING)),
            (Matrix(M::Unavailable), NONE, INSTANCE_ADMIN, read(K19_MATRIX, NOTHING)),
            (Matrix(M::Unavailable), NONE, BOTH, read(K19_MATRIX, NOTHING)),
            (Matrix(M::Unavailable), CONNECTED, ADMIN, read(K19_MATRIX, NOTHING)),
            (Matrix(M::Unavailable), CONNECTED, LEAD, read(K19_MATRIX, NOTHING)),
            (Matrix(M::Unavailable), CONNECTED, INSTANCE_ADMIN, read(K19_MATRIX, NOTHING)),
            (Matrix(M::Unavailable), CONNECTED, BOTH, read(K19_MATRIX, NOTHING)),
            (Matrix(M::Ready), NONE, ADMIN, connect("Matrix")),
            (Matrix(M::Ready), NONE, LEAD, read(K4, NOTHING)),
            (Matrix(M::Ready), NONE, INSTANCE_ADMIN, read(K4, NOTHING)),
            (Matrix(M::Ready), NONE, BOTH, connect("Matrix")),
            (Matrix(M::Ready), CONNECTED, ADMIN, None),
            (Matrix(M::Ready), CONNECTED, LEAD, None),
            (Matrix(M::Ready), CONNECTED, INSTANCE_ADMIN, None),
            (Matrix(M::Ready), CONNECTED, BOTH, None),
        ]
    }

    /// The offered links as the tables write them: label and href, in order.
    fn labelled(fixes: Vec<FixLink>) -> Vec<(String, String)> {
        fixes
            .into_iter()
            .map(|fix| (fix.label, fix.href.to_string()))
            .collect()
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
        assert_eq!(rows.len(), 4 * 2 * 4 + 3 * 2 * 4);
        for (offer, connected, viewer, expected) in rows {
            let read = cause(offer, connected).map(|cause| {
                let said = cause.words(viewer);
                (said.words, labelled(said.fixes))
            });
            assert_eq!(
                read, expected,
                "{offer:?}, connected {connected}, viewer {viewer:?}"
            );
            // An admin is never told to ask an admin, nor about the other kind.
            if let Some((words, _)) = read.filter(|_| viewer.manages_the_workspace()) {
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
    /// The workspace's connections, the viewers that read the same lines, and
    /// those lines.
    type ProjectRow = (Vec<(ChannelKind, SharedConnection)>, [Viewer; 2], Vec<Line>);

    /// The two viewers of one workspace role: being this Foundry's instance
    /// admin changes nothing the project page says, so no line of it gains
    /// I-19 (US-IIS-04).
    const WORKSPACE_ADMINS: [Viewer; 2] = [ADMIN, BOTH];
    const LEADS: [Viewer; 2] = [LEAD, INSTANCE_ADMIN];

    /// Every kind's connection state, each role, and none connected: the
    /// lines the project page shows.
    #[rustfmt::skip]
    fn project_table() -> Vec<ProjectRow> {
        use ChannelKind::{Matrix, Slack};
        use SharedConnection::Unreadable;
        vec![
            (vec![(Slack, connected("Canzan Personal"))], WORKSPACE_ADMINS, vec![(W6_SLACK, MANAGE)]),
            (vec![(Slack, connected("Canzan Personal"))], LEADS, vec![(W6_SLACK, None)]),
            (vec![(Slack, lost("app_removed"))], WORKSPACE_ADMINS, vec![(LOST_SLACK, None)]),
            (vec![(Slack, lost("app_removed"))], LEADS, vec![(LOST_SLACK, None)]),
            (vec![(Slack, Unreadable)], WORKSPACE_ADMINS, vec![(K19_SLACK, None)]),
            (vec![(Slack, Unreadable)], LEADS, vec![(K19_SLACK, None)]),
            (vec![(Matrix, connected("matrix.example.org"))], WORKSPACE_ADMINS, vec![(W6_MATRIX, MANAGE)]),
            (vec![(Matrix, connected("matrix.example.org"))], LEADS, vec![(W6_MATRIX, None)]),
            (vec![(Matrix, lost("token_revoked"))], WORKSPACE_ADMINS, vec![(LOST_MATRIX, None)]),
            (vec![(Matrix, lost("token_revoked"))], LEADS, vec![(LOST_MATRIX, None)]),
            (vec![(Matrix, Unreadable)], WORKSPACE_ADMINS, vec![(K19_MATRIX, None)]),
            (vec![(Matrix, Unreadable)], LEADS, vec![(K19_MATRIX, None)]),
            (vec![(Slack, connected("Canzan Personal")), (Matrix, Unreadable)], LEADS, vec![(W6_SLACK, None), (K19_MATRIX, None)]),
            (vec![], WORKSPACE_ADMINS, vec![(W7, CONNECT_EITHER)]),
            (vec![], LEADS, vec![(K4, None)]),
        ]
    }

    #[test]
    fn every_connection_state_and_role_reads_its_project_line() {
        for (connections, viewers, expected) in project_table() {
            let expected: Vec<_> = expected
                .into_iter()
                .map(|(words, fix)| {
                    (
                        words.to_string(),
                        Vec::from_iter(
                            fix.map(|(label, href)| (label.to_string(), href.to_string())),
                        ),
                    )
                })
                .collect();
            for viewer in viewers {
                let lines: Vec<_> = project_lines(&connections, viewer)
                    .into_iter()
                    .map(|line| (line.words, labelled(line.fixes)))
                    .collect();
                assert_eq!(lines, expected, "{connections:?}, viewer {viewer:?}");
            }
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
        assert_eq!(words.fixes, Vec::new());
    }
}
