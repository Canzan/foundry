//! external-integration-plugins — the acceptance driver the
//! `feature_external_integration_plugins` step file delegates to (DISTILL
//! 2026-10-21, every scenario `@pending`, ADR-025 scaffolded RED).
//!
//! It extends, and never duplicates, the team-membership driver
//! ([`crate::support::team_membership`], `tm`): the roster, the
//! workspace/team/project rows, page readers and the forged-form oracle are
//! its. This module owns, and nothing else:
//!
//! - **The vocabulary** (Mandate 12, criterion 1): [`InstanceSpec`],
//!   [`EmailSetup`], [`Kind`], [`EventChoice`], [`ContentLevel`], [`Scope`],
//!   [`Route`], [`SlackAnswer`], [`WorkspaceDoor`], [`RouteAttempt`],
//!   [`HeldReturn`], [`BoardAction`], [`Universe`], [`Change`], [`EipState`].
//! - **The doors** DESIGN pinned ([REF] Driving Ports, deliverables A and B):
//!   `/workspace/integrations` and its Slack setup, install, return and
//!   disconnect, `/workspace/integrations/matrix`, the public
//!   `/integrations/slack/events`, the project's `…/integrations` pages and
//!   `routes/new|preview|test`, `routes`, `routes/{id}/edit|pause|resume|delete|
//!   history`, `/account/settings` with `/account/settings/project-emails`,
//!   `/email/stop` and `/email/stop/one-click`, `foundry doctor
//!   rotate-secrets`; plus the shipped board doors that produce events. A write
//!   made "from a page" submits the page's own form.
//! - **The outside world's doors**: what the Slack double was asked
//!   ([`crate::support::slack_double`]), what a Matrix room shows
//!   ([`crate::support::matrix_homeserver`]), what the email recorder (or the
//!   pinned mailpit) received, and what a subprocess `foundry` printed and
//!   exported.
//! - **The universe** (Mandate 8, the repo's Rust reading): [`Universe`] is read
//!   ONLY through those doors (Priya's view of the workspace integrations page
//!   and every project's routes, the activity list, the messages each channel
//!   shows, the emails each person received). A `When` captures it first; a
//!   `Then` compares the after-image with the before-image plus the declared
//!   [`Change`]s, fail-closed.
//!
//! Preconditions (people, teams, projects, lanes, an issue number) are seeded
//! with SQL, never an expected outcome (No Fixture Theater): every connection,
//! route, message and email the scenarios assert on is made by the product.
//! Issues the scenarios name are filed and moved through the product's own
//! board doors before any route exists (a route receives only what happens
//! after it is saved, DDR-27).
//!
//! Page seams DISTILL pins (DELIVER may change them together with this file):
//! the Slack connection line `[data-connection="slack"]` (Matrix
//! `[data-connection="matrix"]`), the integration activity
//! `[data-integration-activity] li`, one element per route
//! `[data-route-destination="<destination>"]` with `data-route-id="<uuid>"`
//! whose text holds the K-9 status, the history entries `[data-delivery]`,
//! the preview `[data-route-preview]`, the channel picker `select[name=destination]`,
//! and the route form's fields `kind`, `destination`, `events` (repeated),
//! `lanes` (repeated), `content_level` (`link_only|summary|summary_with_comment`),
//! `shared_confirmed`. Project emails: the section `[data-project-emails]`, one
//! `input[name=choice]` per `(project_id, event, scope)` checkbox.
//!
//! Harness seam (DELIVER 01-05): [`rewire`] installs the scenario's
//! [`InstanceSpec`] (Slack settings pointed at the double, secrets key, and
//! every other setting [`instance_env`] spells) into the in-process app's
//! `AppState::integrations`, read through the binary's own settings reader.
//!
//! The states this feature drives (AT-completeness C2a):
//!
//! ```text
//!  connection:  none ──Add to Slack (approve)──▶ connected ──refusal/uninstall event──▶ lost
//!               lost ──reconnect (same team)──▶ connected      lost ──reconnect (other team)──▶ lost
//!               connected|lost ──disconnect──▶ disconnected ──Add to Slack──▶ connected
//!  route:       Delivering ⇄ Retrying (transient) ; any ──pause──▶ Paused ──resume──▶ Delivering
//!               Delivering ──channel refusal──▶ Needs attention ──resume (re-check)──▶ Delivering | Needs attention
//!               Delivering ──filter lane deleted──▶ Needs attention (lane) ; ──24 h give-up──▶ Needs attention (gave up)
//!               connection lost ⇒ Paused: connection lost ; connection disconnected ⇒ Paused: Slack was disconnected
//!  delivery:    pending ──sent──▶ delivered | pending ──transient──▶ pending(+1 attempt) ──give_up_at──▶ failed
//!               pending ──pause|disconnect|route delete|project gone|access lost|muted──▶ skipped
//!  email choice: off ──save──▶ on ──stop (confirm|one-click)──▶ off ; GET of a stop link: unchanged
//!  illegal:     non-admin ─any workspace door→ not-found · non-Manage ─any route door→ not-found
//!               tokenless POST → forged form · forged/replayed/expired Slack return → K-3, nothing stored
//! ```

use crate::steps::handler_instrumentation::FoundrySubprocess;
use crate::support::harness::{ensure_postgres, InProcHarness};
use crate::support::html_assertions::{parse, select_all};
use crate::support::matrix_homeserver::{self as mx, Account, FaultProxy, Homeserver, Names};
use crate::support::multi_replica_harness::{MultiReplicaHarness, ReplicaTime, WallTickingClock};
use crate::support::notify_recorder::DeliveryRecorder;
use crate::support::slack_double::{
    self as sd, Approval, Mood, PostedMessage, SlackDouble, SlackTeam,
};
use crate::support::team_membership::{
    self as tm, Actor, Answer, ProjectRef, TeamRef, WorkspaceRole,
};
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use reqwest::header::{CONTENT_TYPE, COOKIE, LOCATION};
use reqwest::StatusCode;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ===========================================================================
// Doors and locked copy
// ===========================================================================

/// Slack's own words in a refusal (a planted token-shaped string, a vendor
/// detail and a URL secret): the history must show none of them.
pub const RAW_VENDOR_TEXT: &str = "xoxb-leaked raw-vendor-detail https://hooks.slack.com/secret";
const RAW_VENDOR_WORDS: &[&str] = &["xoxb-leaked", "raw-vendor-detail", "hooks.slack.com"];

/// The least bot permissions slices 01-03 need (ADR-EIP-005 decision 3, DA-3).
pub const BOT_SCOPES: &[&str] = &[
    "chat:write",
    "chat:write.public",
    "channels:read",
    "groups:read",
];
pub const WORKSPACE_INTEGRATIONS: &str = "/workspace/integrations";
pub const SLACK_SETUP: &str = "/workspace/integrations/slack/setup";
pub const SLACK_INSTALL: &str = "/workspace/integrations/slack/install";
pub const SLACK_RETURN: &str = "/workspace/integrations/slack/return";
pub const SLACK_DISCONNECT: &str = "/workspace/integrations/slack/disconnect";
pub const MATRIX_CONNECT: &str = "/workspace/integrations/matrix";
/// US-EIP-07 for Matrix (DESIGN Driving Ports; user decision 2026-10-25:
/// built like Slack's).
pub const MATRIX_DISCONNECT: &str = "/workspace/integrations/matrix/disconnect";
pub const SLACK_EVENTS: &str = "/integrations/slack/events";
pub const ACCOUNT_SETTINGS: &str = "/account/settings";
pub const PROJECT_EMAILS: &str = "/account/settings/project-emails";
pub const EMAIL_STOP: &str = "/email/stop";
pub const EMAIL_STOP_ONE_CLICK: &str = "/email/stop/one-click";

/// The bounded wait for an eventual state (NFR-EIP-1: 10 s at p95; a send that
/// Slack never answers needs the client's timeout first, so statuses get more).
pub const DELIVERY_BUDGET: Duration = Duration::from_secs(10);
pub const STATUS_BUDGET: Duration = Duration::from_secs(45);
pub const EMAIL_BUDGET: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(200);

/// The secrets key the instance is given (a sentinel for the leak scans).
pub const SECRETS_KEY: &str = "ZWlwLXNlbnRpbmVsLXNlY3JldHMta2V5LTMyYnl0ZXM";
/// The fixed test Ed25519 public key (`foundry_auth::test_keys`), as the
/// operator's process is given it.
const TEST_MACHINE_TOKEN_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\\nMCowBQYDK2VwAyEAwtFPs8Jcuncc+E7dXqG/oolI3P6Hamrpd8zVKPvRmg0=\\n-----END PUBLIC KEY-----";
pub const NEW_SECRETS_KEY: &str = "ZWlwLXNlbnRpbmVsLW5ldy1zZWNyZXRzLWtleS0zMmI";

const PEOPLE: &[(&str, &str)] = &[
    ("Priya Raman", "priya@canzan.dev"),
    ("Lena Park", "lena.park@canzan.dev"),
    ("Sam Okafor", "sam.okafor@canzan.dev"),
    ("Tomás Rivera", "tomas.rivera@canzan.dev"),
    ("Marco", "marco@canzan.dev"),
];

// ===========================================================================
// Vocabulary
// ===========================================================================

/// How the instance is set up (the operator's settings, DESIGN Wave Decisions
/// handoff). [`rewire`] hands it to the app.
#[derive(Debug, Clone)]
pub struct InstanceSpec {
    pub slack_app: SlackAppSetup,
    pub secrets_key: Option<String>,
    pub previous_secrets_key: Option<String>,
    pub email: EmailSetup,
    pub give_up_seconds: u64,
    pub lease_seconds: u64,
    pub poll_ms: u64,
    /// False for subprocess and replica scenarios: the in-process app then
    /// runs no delivery worker of its own on the shared schema.
    pub in_process_worker: bool,
}

impl Default for InstanceSpec {
    fn default() -> Self {
        Self {
            slack_app: SlackAppSetup::None,
            secrets_key: Some(SECRETS_KEY.to_string()),
            previous_secrets_key: None,
            email: EmailSetup::Recorder,
            give_up_seconds: 86_400,
            lease_seconds: 45,
            poll_ms: 200,
            in_process_worker: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackAppSetup {
    None,
    /// All three settings, pointed at the double.
    Complete,
    /// Two of three (refuses start).
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailSetup {
    /// The in-process EmailTransport recorder (the notify_recorder pattern).
    Recorder,
    /// No `smtp` or `email_api` provider: K-13.
    None,
    /// The shipped SmtpProvider to the pinned mailpit (`@needs-smtp`).
    SmtpRelay {
        host: String,
        smtp_port: u16,
        http_port: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Slack,
    Matrix,
}

impl Kind {
    pub fn parse(word: &str) -> Self {
        match word {
            "Slack" => Self::Slack,
            "Matrix" => Self::Matrix,
            other => {
                panic!("a route kind is Slack or Matrix in deliverables A and B, not {other:?}")
            }
        }
    }
    pub fn wire(self) -> &'static str {
        match self {
            Self::Slack => "slack",
            Self::Matrix => "matrix",
        }
    }
    /// The workspace admin's disconnect door: GET asks, POST confirms.
    pub fn disconnect_path(self) -> &'static str {
        match self {
            Self::Slack => SLACK_DISCONNECT,
            Self::Matrix => MATRIX_DISCONNECT,
        }
    }
}

/// The v1 chat and email events (D-2, `EventKind` wire names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventChoice {
    NewIssue,
    IssueMoved,
    NewComment,
    IssueDeleted,
}

impl EventChoice {
    pub const ALL: [Self; 4] = [
        Self::NewIssue,
        Self::IssueMoved,
        Self::NewComment,
        Self::IssueDeleted,
    ];

    pub fn parse(word: &str) -> Self {
        match word.trim() {
            "New issue" => Self::NewIssue,
            "Issue moved" => Self::IssueMoved,
            "New comment" => Self::NewComment,
            "Issue deleted" => Self::IssueDeleted,
            other => panic!("the v1 events are New issue, Issue moved, New comment and Issue deleted, not {other:?}"),
        }
    }
    /// "New comment and New issue" / "every chat event".
    pub fn parse_list(words: &str) -> Vec<Self> {
        if words.trim() == "every chat event" {
            return Self::ALL.to_vec();
        }
        words.split(" and ").map(Self::parse).collect()
    }
    /// An optional list ("" = none).
    pub fn parse_list_opt(words: &str) -> Vec<Self> {
        if words.trim().is_empty() {
            Vec::new()
        } else {
            Self::parse_list(words)
        }
    }
    pub fn wire(self) -> &'static str {
        match self {
            Self::NewIssue => "issue.created",
            Self::IssueMoved => "issue.moved",
            Self::NewComment => "comment.added",
            Self::IssueDeleted => "issue.deleted",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::NewIssue => "New issue",
            Self::IssueMoved => "Issue moved",
            Self::NewComment => "New comment",
            Self::IssueDeleted => "Issue deleted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentLevel {
    LinkOnly,
    Summary,
    SummaryWithCommentText,
}

impl ContentLevel {
    pub fn parse(word: &str) -> Self {
        match word.trim() {
            "Link only" => Self::LinkOnly,
            "Summary" => Self::Summary,
            "Summary with comment text" => Self::SummaryWithCommentText,
            other => panic!("the content levels are Link only, Summary and Summary with comment text, not {other:?}"),
        }
    }
    pub fn wire(self) -> &'static str {
        match self {
            Self::LinkOnly => "link_only",
            Self::Summary => "summary",
            Self::SummaryWithCommentText => "summary_with_comment",
        }
    }
}

/// An email choice's scope (US-EIP-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Mine,
}

impl Scope {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "all issues" => Self::All,
            "issues he created or commented on" | "issues she created or commented on" => {
                Self::Mine
            }
            other => {
                panic!("a scope is all issues or issues he created or commented on, not {other:?}")
            }
        }
    }
    pub fn wire(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Mine => "mine",
        }
    }
}

/// A route as a scenario describes it.
#[derive(Debug, Clone)]
pub struct Route {
    pub project: String,
    pub kind: Kind,
    pub destination: String,
    pub events: Vec<EventChoice>,
    /// Lane labels Issue moved is limited to (empty = any lane).
    pub lanes: Vec<String>,
    pub level: ContentLevel,
    pub shared_confirmed: bool,
}

impl Route {
    pub fn new(project: &str, kind: Kind, destination: &str) -> Self {
        Self {
            project: project.to_string(),
            kind,
            destination: destination.to_string(),
            events: vec![EventChoice::NewIssue],
            lanes: Vec::new(),
            level: ContentLevel::Summary,
            shared_confirmed: false,
        }
    }
}

/// What Slack answers Foundry's next post with (the classifier rows, DESIGN
/// ADR-EIP-005 / DEVOPS-2).
#[derive(Debug, Clone, PartialEq)]
pub enum SlackAnswer {
    NoAnswer,
    SlowDown(u64),
    ErrorPage,
    Unknown,
    Archived,
    NotInChannel,
    ChannelNotFound,
    TokenRevoked,
    AccountInactive,
    InvalidAuth,
    RefusalWithText(String),
}

impl SlackAnswer {
    pub fn parse(words: &str) -> Self {
        let w = words.trim();
        if w == "a refusal carrying its own raw text" {
            return Self::RefusalWithText(RAW_VENDOR_TEXT.to_string());
        }
        if let Some(rest) = w.strip_prefix("a request to slow down for ") {
            let secs = rest.trim_end_matches(" seconds").parse().expect("seconds");
            return Self::SlowDown(secs);
        }
        match w {
            "no answer" => Self::NoAnswer,
            "an error page instead of an answer" => Self::ErrorPage,
            "a refusal Foundry does not know" => Self::Unknown,
            "the channel is archived" => Self::Archived,
            "Foundry is not in the channel" => Self::NotInChannel,
            "the channel was not found" => Self::ChannelNotFound,
            "the token was revoked" => Self::TokenRevoked,
            "the Slack workspace is inactive" => Self::AccountInactive,
            "the credential is not valid" => Self::InvalidAuth,
            other => panic!("the Slack double cannot answer {other:?}"),
        }
    }
    fn mood(&self) -> Mood {
        let code = |c: &str| Mood::Refuses {
            code: c.into(),
            text: None,
        };
        match self {
            Self::NoAnswer => Mood::Silent,
            Self::SlowDown(s) => Mood::SlowDown(*s),
            Self::ErrorPage => Mood::ErrorPage,
            Self::Unknown => code("eip_code_nobody_documented"),
            Self::Archived => code("is_archived"),
            Self::NotInChannel => code("not_in_channel"),
            Self::ChannelNotFound => code("channel_not_found"),
            Self::TokenRevoked => code("token_revoked"),
            Self::AccountInactive => code("account_inactive"),
            Self::InvalidAuth => code("invalid_auth"),
            Self::RefusalWithText(t) => Mood::Refuses {
                code: "eip_code_nobody_documented".into(),
                text: Some(t.clone()),
            },
        }
    }
}

/// The workspace-level doors the authority outline walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceDoor {
    IntegrationsPage,
    SlackSetup,
    AddToSlack,
    SlackReturn,
}

impl WorkspaceDoor {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "the workspace integrations page" => Self::IntegrationsPage,
            "the Slack setup page" => Self::SlackSetup,
            "Add to Slack" => Self::AddToSlack,
            "the Slack return address" => Self::SlackReturn,
            other => panic!("unknown workspace door {other:?}"),
        }
    }
    fn path(self) -> String {
        match self {
            Self::IntegrationsPage => WORKSPACE_INTEGRATIONS.into(),
            Self::SlackSetup => SLACK_SETUP.into(),
            Self::AddToSlack => SLACK_INSTALL.into(),
            Self::SlackReturn => {
                format!("{SLACK_RETURN}?code=eip-code-nobody-issued&state=eip-state-nobody-issued")
            }
        }
    }
}

/// What a person tries on a project's routes (the authority outline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteAttempt {
    OpenPage,
    OpenNewSlackRoute,
    SubmitNewRoute(String),
}

impl RouteAttempt {
    pub fn parse(words: &str) -> Self {
        let w = words.trim();
        if let Some(dest) = w.strip_prefix("submits a new route to ") {
            return Self::SubmitNewRoute(dest.to_string());
        }
        match w {
            "opens the integrations page" => Self::OpenPage,
            "opens a new Slack route" => Self::OpenNewSlackRoute,
            other => panic!("unknown route attempt {other:?}"),
        }
    }
}

/// A Slack return address the scenario holds (US-EIP-01 forged returns).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldReturn {
    NeverIssued,
    AlreadyUsed,
    OlderThanTenMinutes,
    AnotherSession,
}

impl HeldReturn {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "a state Foundry never issued" => Self::NeverIssued,
            "a state that was already used" => Self::AlreadyUsed,
            "a state older than 10 minutes" => Self::OlderThanTenMinutes,
            "a state issued to another session" => Self::AnotherSession,
            other => panic!("unknown Slack return {other:?}"),
        }
    }
}

/// A board action that produces (or must not produce) an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardAction {
    File {
        project: String,
        title: String,
    },
    Move {
        key: String,
        to_lane: String,
    },
    Reorder {
        key: String,
        above: String,
        lane: String,
    },
    Comment {
        key: String,
        body: String,
    },
    DeleteIssue {
        key: String,
    },
}

/// The observable universe (Mandate 8), read only through doors.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Universe {
    /// The Slack connection line as Priya reads it (empty when absent).
    pub slack_connection: String,
    pub matrix_connection: String,
    /// The integration activity lines, newest first.
    pub activity: Vec<String>,
    /// `(project, destination) -> status text` on each project's page (Priya's view).
    pub routes: BTreeMap<(String, String), String>,
    /// `channel -> messages it shows`.
    pub shown: BTreeMap<String, usize>,
    /// Calls that change Slack (`apps.uninstall`).
    pub slack_uninstalls: usize,
    /// `recipient -> emails received`.
    pub emails: BTreeMap<String, usize>,
    /// Projects read while deleted: their page answered "not found", as a
    /// deleted project's page must. Only a step that deleted the project on
    /// purpose may read it this way; everywhere else a page that does not
    /// answer leaves the universe unreadable.
    pub gone: BTreeSet<String>,
}

/// One declared movement of the universe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    SlackConnection,
    MatrixConnection,
    ActivityLines(usize),
    RouteAppeared {
        project: String,
        destination: String,
    },
    RouteGone {
        project: String,
        destination: String,
    },
    RouteStatus {
        project: String,
        destination: String,
    },
    Shown {
        channel: String,
    },
    SlackUninstall,
    Emailed {
        to: String,
    },
}

/// One email a person received (through the recorder or the relay).
#[derive(Debug, Clone, Default)]
pub struct ReceivedEmail {
    pub to: String,
    pub subject: String,
    pub body: String,
    /// Lower-cased header name → value (List-Unsubscribe, List-Unsubscribe-Post).
    pub headers: BTreeMap<String, String>,
}

/// The per-scenario state of this feature (`FoundryWorld::eip`).
#[derive(Default)]
pub struct EipState {
    pub spec: InstanceSpec,
    pub slack: Option<Arc<SlackDouble>>,
    pub homeserver: Option<Homeserver>,
    pub proxy: Option<FaultProxy>,
    pub names: Names,
    pub accounts: HashMap<String, Account>,
    pub rooms: HashMap<String, String>,
    pub process: Option<FoundrySubprocess>,
    pub replicas: Option<MultiReplicaHarness>,
    /// `(base, email)` → `foundry_session=…`.
    pub sessions: HashMap<(String, String), String>,
    pub csrf: String,
    pub held_return: Option<String>,
    pub draft: Option<Route>,
    pub before: Option<Universe>,
    pub when_started: Option<Instant>,
    pub when_answer: Option<Answer>,
    pub last_key: Option<String>,
    /// The project each issue key belongs to (keys are seeded or filed here).
    pub issue_projects: HashMap<String, String>,
    pub next_numbers: HashMap<String, i64>,
    pub received: Option<ReceivedEmail>,
    pub cli: Option<(i32, String, String)>,
    pub startup: Option<(Option<i32>, String)>,
    pub shown_before: BTreeMap<String, usize>,
    /// The never-sent issue's (title, description): the title may be sent,
    /// the description never.
    pub description: Option<(String, String)>,
    pub comment_text: Option<String>,
    /// The title of the last issue filed in a When (user text for the ping rows).
    pub last_title: Option<String>,
    pub comment_id: Option<String>,
    pub commented_key: Option<String>,
    pub notice_lines: Option<(String, String)>,
    pub mailpit: Option<(String, u16)>,
    /// The page a person has in front of them before the When (the board,
    /// a delete dialog, a stop confirm), kept apart from the universe reads.
    pub in_front: Option<Answer>,
    /// The proxy's connection count when the When began, and the decoy on a
    /// refused loopback address (the "nothing was asked" oracle).
    pub proxy_before: Option<usize>,
    pub decoy: Option<mx::Decoy>,
    /// Slack or the homeserver went silent in this scenario: an absence check
    /// then waits STATUS_BUDGET (Foundry's client timeout comes first).
    pub silence_seen: bool,
    /// The operator's Foundry was killed between two attempts and started
    /// again: a message still waiting is due at its retry, not at once.
    pub restarted_between_attempts: bool,
    /// The channel whose next post has a scripted Slack answer ("Slack will
    /// answer Foundry's next post to #auth-dev with …"): a later "Sam has
    /// filed …" waits until Slack has been asked and Foundry has recorded it.
    pub scripted_answer_channel: Option<String>,
    /// The homeserver has a scripted answer for Foundry's next send (a fault
    /// armed, or no answer at all): a later "Sam has filed …" waits until
    /// Foundry's first send has reached the homeserver, as Slack's does.
    pub homeserver_scripted: bool,
    /// The room of the scenario's last Matrix route (whose history tells
    /// when Foundry has recorded the homeserver's answer).
    pub matrix_destination: Option<String>,
}

impl std::fmt::Debug for EipState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EipState")
            .field("spec", &self.spec)
            .field("slack", &self.slack)
            .finish_non_exhaustive()
    }
}

// ===========================================================================
// Access: which Foundry, which session
// ===========================================================================

fn now(world: &FoundryWorld) -> time::OffsetDateTime {
    use foundry_app::Clock as _;
    tm::harness(world).fake_clock.now()
}

/// The Foundry the scenario talks to: the operator's process, the replicas'
/// proxy, or the in-process app.
pub fn base(world: &FoundryWorld) -> String {
    if let Some(p) = &world.eip.process {
        return format!("http://{}", p.main_addr);
    }
    if let Some(r) = &world.eip.replicas {
        return format!("http://{}", r.proxy.addr);
    }
    tm::harness(world).base_url()
}

fn http(world: &FoundryWorld) -> reqwest::Client {
    tm::http(world)
}

fn email_of(world: &FoundryWorld, who: &str) -> Option<String> {
    match tm::resolve_actor(world, who) {
        Actor::SignedOut => None,
        Actor::Person(email) => Some(email),
    }
}

async fn cookie(world: &mut FoundryWorld, who: Option<&str>) -> String {
    let csrf = format!("foundry_csrf={}", world.eip.csrf);
    let Some(email) = who else { return csrf };
    let key = (base(world), email.to_string());
    if !world.eip.sessions.contains_key(&key) {
        let session = sign_in_at(&http(world), &key.0, email, tm::ROSTER_PASSWORD).await;
        world.eip.sessions.insert(key.clone(), session);
    }
    format!("{}; {csrf}", world.eip.sessions[&key])
}

/// `harness::establish_session` against any Foundry (the subprocess, the proxy).
async fn sign_in_at(http: &reqwest::Client, base: &str, email: &str, password: &str) -> String {
    let get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("GET /sign-in");
    let csrf = get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            s.strip_prefix("foundry_csrf=")
                .and_then(|r| r.split(';').next())
                .map(str::to_string)
        })
        .expect("/sign-in mints foundry_csrf");
    let resp = http
        .post(format!("{base}/sign-in"))
        .header(COOKIE, format!("foundry_csrf={csrf}"))
        .form(&[
            ("email", email),
            ("password", password),
            ("_csrf", csrf.as_str()),
        ])
        .send()
        .await
        .expect("POST /sign-in");
    let set = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .map(str::to_string)
        .unwrap_or_else(|| panic!("{email} could not sign in at {base}"));
    set.split(';').next().unwrap_or(&set).to_string()
}

async fn get(world: &mut FoundryWorld, who: &str, path: &str) -> Answer {
    let email = email_of(world, who);
    let cookie = cookie(world, email.as_deref()).await;
    let url = if path.starts_with("http") {
        path.to_string()
    } else {
        format!("{}{path}", base(world))
    };
    let resp = http(world)
        .get(&url)
        .header(COOKIE, cookie)
        .send()
        .await
        .unwrap_or_else(|e| panic!("GET {path}: {e}"));
    let answer = tm::into_answer(path, resp).await;
    keep(world, email, answer.clone());
    answer
}

async fn post(
    world: &mut FoundryWorld,
    who: &str,
    path: &str,
    fields: &[(String, String)],
    with_token: bool,
) -> Answer {
    let email = email_of(world, who);
    let mut cookie = cookie(world, email.as_deref()).await;
    let mut form: Vec<(String, String)> = fields
        .iter()
        .filter(|(k, _)| k != "_csrf")
        .cloned()
        .collect();
    if with_token {
        form.push(("_csrf".into(), world.eip.csrf.clone()));
    } else {
        cookie = cookie
            .split("; ")
            .filter(|c| !c.starts_with("foundry_csrf="))
            .collect::<Vec<_>>()
            .join("; ");
    }
    let resp = http(world)
        .post(format!("{}{path}", base(world)))
        .header(COOKIE, cookie)
        .form(&form)
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST {path}: {e}"));
    let answer = tm::into_answer(path, resp).await;
    keep(world, email, answer.clone());
    answer
}

fn keep(world: &mut FoundryWorld, email: Option<String>, answer: Answer) {
    let actor = email.map(Actor::Person).unwrap_or(Actor::SignedOut);
    world
        .tm
        .current
        .insert(tm::actor_key(&actor), answer.clone());
    tm::record(world, &actor, answer);
}

fn went_through(answer: &Answer, what: &str) {
    assert!(
        answer.status.is_success() || answer.status.is_redirection(),
        "precondition: {what} went through the product; {}",
        tm::describe(answer)
    );
}

/// Follow a redirect answer within Foundry (one hop), as the browser would.
async fn follow(world: &mut FoundryWorld, who: &str, answer: Answer) -> Answer {
    match (answer.status.is_redirection(), answer.location.clone()) {
        (true, Some(loc)) if !loc.starts_with("http") || loc.starts_with(&base(world)) => {
            let path = loc.strip_prefix(&base(world)).unwrap_or(&loc).to_string();
            get(world, who, &path).await
        }
        _ => answer,
    }
}

async fn eventually<F, Fut>(budget: Duration, what: &str, mut probe: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < budget {
        match probe().await {
            Ok(()) => return,
            Err(why) => last = why,
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("{what} within {budget:?}; last seen: {last}");
}

fn text(answer: &Answer) -> String {
    tm::page_text(&answer.body)
}

fn texts_of(body: &str, selector: &str) -> Vec<String> {
    let doc = parse(body);
    select_all(&doc, selector)
        .into_iter()
        .map(|e| {
            e.text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn attr_of(body: &str, selector: &str, attr: &str) -> Vec<String> {
    let doc = parse(body);
    select_all(&doc, selector)
        .into_iter()
        .filter_map(|e| e.value().attr(attr).map(str::to_string))
        .collect()
}

// ===========================================================================
// Background: the instance, the workspace, its people, projects and issues
// ===========================================================================

/// The in-process app's delivery worker, rebuilt as `spawn_app` builds it but
/// on a [`WallTickingClock`]: the scenario's fake clock (every `advance`
/// included) plus the wall time since the Background. A real instance's
/// worker sees time pass between attempts, so per-destination pacing (one
/// attempt a second, `not_before = now + 1 s`) lets a second message to the
/// same channel go; on the frozen clock it never would. Only the worker
/// ticks: the board (filing times, history, activity dates) still reads the
/// frozen clock, so "14:02" stays 14:02, and "24 hours pass" still jumps the
/// worker. Scoped to this feature's Background; no other lane is touched.
fn run_worker_on_wall_time(world: &mut FoundryWorld) {
    use foundry_app::integrations::{
        store_capabilities::StoreCapabilities, worker::IntegrationsWorker,
    };
    let harness = world
        .harness
        .as_mut()
        .expect("the Background spawned foundry for this scenario");
    if let Some(frozen) = harness.app.worker_task.take() {
        frozen.abort();
    }
    let state = &harness.app.state;
    let clock: Arc<dyn foundry_app::Clock> =
        Arc::new(WallTickingClock::new(harness.fake_clock.clone()));
    harness.app.worker_task = Some(
        IntegrationsWorker::new(
            StoreCapabilities::new(state.store.clone()),
            state.integrations.clone(),
            clock,
        )
        .spawn(),
    );
}

/// "the Canzan Labs workspace with these people" — spawn foundry, seed the
/// workspace, its teams (from the table) and people.
pub async fn given_workspace_people(world: &mut FoundryWorld, step: &Step) {
    let rows = table_rows(step);
    world.harness = Some(InProcHarness::spawn(tm::ts("2026-10-21T14:02:00Z")).await);
    run_worker_on_wall_time(world);
    world.http = Some(
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(false)
            .build()
            .expect("build reqwest client"),
    );
    world.tm.csrf = format!("eip-{}", uuid::Uuid::new_v4().simple());
    world.eip.csrf = world.tm.csrf.clone();
    tm::seed_workspace(world, tm::HOME, "2025-06-01T00:00:00Z").await;
    for team in ["Backend", "Ops"] {
        seed_team(world, team).await;
    }
    for row in rows.iter().skip(1) {
        let (name, role, teams) = (
            row[0].as_str(),
            row[1].as_str(),
            row.get(2).map(String::as_str).unwrap_or(""),
        );
        let email = PEOPLE
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, e)| e.to_string())
            .unwrap_or_else(|| panic!("{name:?} is not one of this feature's people"));
        tm::seed_person(
            world,
            tm::HOME,
            name,
            &email,
            WorkspaceRole::parse(role),
            teams,
            None,
        )
        .await;
    }
}

async fn seed_team(world: &mut FoundryWorld, name: &str) {
    let team = TeamRef {
        id: uuid::Uuid::now_v7(),
        workspace: tm::HOME.to_string(),
        name: name.to_string(),
        slug: tm::slug_of(name),
    };
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team.id)
        .bind(tm::workspace_id(world, tm::HOME))
        .bind(&team.name)
        .bind(&team.slug)
        .execute(&tm::pool(world))
        .await
        .expect("seed a team");
    world.tm.teams.insert(name.to_string(), team);
}

fn table_rows(step: &Step) -> Vec<Vec<String>> {
    step.table
        .as_ref()
        .expect("this step carries a table")
        .rows
        .iter()
        .map(|r| r.iter().map(|c| c.trim().to_string()).collect())
        .collect()
}

/// "Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops".
pub async fn given_projects(world: &mut FoundryWorld, projects: &[(String, String, String)]) {
    for (name, key, team) in projects {
        tm::seed_project(world, team, name, key).await;
        // The lane labels the scenarios read ("In progress").
        sqlx::query("UPDATE lanes SET label = 'In progress' WHERE slug = 'in_progress' AND project_id = (SELECT id FROM projects WHERE key_prefix = $1)")
            .bind(key)
            .execute(&tm::pool(world))
            .await
            .expect("label the In progress lane");
    }
}

/// "Identity Platform has the lanes Backlog, Todo, In progress, QA, Done and Spikes".
pub async fn given_lanes(world: &mut FoundryWorld, project: &str, lanes: &str) {
    let labels: Vec<String> = lanes
        .replace(" and ", ", ")
        .split(", ")
        .map(|s| s.trim().to_string())
        .collect();
    // `UNIQUE (project_id, position)` is checked per statement: move the
    // seeded lanes out of the way first, then place every lane in order.
    sqlx::query("UPDATE lanes SET position = position + 100 WHERE project_id = (SELECT id FROM projects WHERE name = $1)")
        .bind(project)
        .execute(&tm::pool(world))
        .await
        .expect("make room for the scenario's lanes");
    for (position, label) in labels.iter().enumerate() {
        sqlx::query(
            "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
             SELECT gen_random_uuid(), p.id, p.workspace_id, $2, $3, $4 FROM projects p WHERE p.name = $1
             ON CONFLICT (project_id, slug) DO UPDATE SET label = EXCLUDED.label, position = EXCLUDED.position",
        )
        .bind(project)
        .bind(lane_slug(label))
        .bind(label)
        .bind(position as i32)
        .execute(&tm::pool(world))
        .await
        .expect("seed a lane");
    }
}

pub fn lane_slug(label: &str) -> String {
    label.trim().to_ascii_lowercase().replace(' ', "_")
}

/// The slug of the lane `project` shows as `label` today. A rename changes a
/// lane's label and keeps its slug ("Done" renamed "Shipped" is still `done`),
/// so a move names the lane by what the board reads now. A label no lane
/// carries falls back to the slug the label would seed, so the board answers
/// for an unknown lane itself.
pub async fn current_lane_slug(world: &FoundryWorld, project: &str, label: &str) -> String {
    let slug: Option<String> = sqlx::query_scalar(
        "SELECT l.slug FROM lanes l JOIN projects p ON p.id = l.project_id
          WHERE p.name = $1 AND l.label = $2",
    )
    .bind(project)
    .bind(label.trim())
    .fetch_optional(&tm::pool(world))
    .await
    .expect("read the lane's current slug");
    slug.unwrap_or_else(|| lane_slug(label))
}

/// "Identity Platform holds these issues" — each filed and moved through the
/// product as the person named, before any route exists.
pub async fn given_issues(world: &mut FoundryWorld, project: &str, step: &Step) {
    for row in table_rows(step).iter().skip(1) {
        let (key, title, lane, by) = (
            row[0].as_str(),
            row[1].as_str(),
            row[2].as_str(),
            row[3].as_str(),
        );
        let number: i64 = key
            .rsplit('-')
            .next()
            .and_then(|n| n.parse().ok())
            .expect("a key ends in its number");
        set_next_number(world, project, number).await;
        file(world, first_name(by), project, title).await;
        if lane != "Backlog" {
            move_issue(world, first_name(by), key, lane).await;
        }
    }
}

fn first_name(full: &str) -> &str {
    full.split(' ').next().unwrap_or(full)
}

/// "the next issue filed in Identity Platform becomes AUTH-12".
pub async fn given_next_issue(world: &mut FoundryWorld, project: &str, key: &str) {
    let number: i64 = key
        .rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .expect("a key ends in its number");
    set_next_number(world, project, number).await;
}

async fn set_next_number(world: &mut FoundryWorld, project: &str, number: i64) {
    sqlx::query("UPDATE projects SET next_issue_number = $2 WHERE name = $1")
        .bind(project)
        .bind(number as i32)
        .execute(&tm::pool(world))
        .await
        .expect("set the project's next issue number");
    world.eip.next_numbers.insert(project.to_string(), number);
}

/// "it is 14:02 on 21 Oct 2026".
pub fn given_time(world: &mut FoundryWorld, hhmm: &str) {
    let at = tm::ts(&format!("2026-10-21T{hhmm}:00Z"));
    tm::harness(world).fake_clock.set(at);
}

/// The operator's settings as a scenario states them.
pub async fn given_instance(world: &mut FoundryWorld, words: &str) {
    match words.trim() {
        "the Foundry Slack app set up" => {
            ensure_slack(world).await;
            world.eip.spec.slack_app = SlackAppSetup::Complete;
        }
        "no Foundry Slack app set up" => world.eip.spec.slack_app = SlackAppSetup::None,
        "the Foundry Slack app set up but no secrets key" => {
            ensure_slack(world).await;
            world.eip.spec.slack_app = SlackAppSetup::Complete;
            world.eip.spec.secrets_key = None;
        }
        other => panic!("unknown instance setup {other:?}"),
    }
    rewire(world).await;
}

pub async fn given_email_setup(world: &mut FoundryWorld, can_send: bool) {
    world.eip.spec.email = if can_send {
        EmailSetup::Recorder
    } else {
        EmailSetup::None
    };
    rewire(world).await;
}

async fn ensure_slack(world: &mut FoundryWorld) -> Arc<SlackDouble> {
    if world.eip.slack.is_none() {
        world.eip.slack = Some(SlackDouble::spawn().await);
    }
    world.eip.slack.clone().expect("the Slack double")
}

fn slack(world: &FoundryWorld) -> Arc<SlackDouble> {
    world
        .eip
        .slack
        .clone()
        .expect("a Given set the instance's Foundry Slack app up (the Slack double)")
}

/// The environment the operator gives Foundry for this scenario's spec.
pub fn instance_env(world: &FoundryWorld) -> Vec<(&'static str, String)> {
    let spec = &world.eip.spec;
    let mut env = vec![
        (
            "FOUNDRY_INTEGRATIONS_GIVE_UP_SECONDS",
            spec.give_up_seconds.to_string(),
        ),
        (
            "FOUNDRY_INTEGRATIONS_LEASE_SECONDS",
            spec.lease_seconds.to_string(),
        ),
        ("FOUNDRY_INTEGRATIONS_POLL_MS", spec.poll_ms.to_string()),
        (
            "FOUNDRY_PUBLIC_URL",
            "https://foundry.canzan.dev".to_string(),
        ),
    ];
    if let Some(key) = &spec.secrets_key {
        env.push(("FOUNDRY_SECRETS_KEY", key.clone()));
    }
    // The shipped SMTP provider pointed at the pinned relay (DEVOPS-4).
    if let EmailSetup::SmtpRelay {
        host, smtp_port, ..
    } = &spec.email
    {
        env.push(("NOTIFICATION_PROVIDERS", "smtp".to_string()));
        env.push(("SMTP_HOST", host.clone()));
        env.push(("SMTP_PORT", smtp_port.to_string()));
        env.push(("SMTP_FROM", "foundry@canzan.dev".to_string()));
        env.push(("SMTP_TLS_VERIFY", "false".to_string()));
    }
    if let Some(key) = &spec.previous_secrets_key {
        env.push(("FOUNDRY_SECRETS_KEY_PREVIOUS", key.clone()));
    }
    if let Some(double) = &world.eip.slack {
        match spec.slack_app {
            SlackAppSetup::None => {}
            SlackAppSetup::Complete | SlackAppSetup::Partial => {
                env.push(("SLACK_CLIENT_ID", sd::CLIENT_ID.to_string()));
                env.push(("SLACK_CLIENT_SECRET", sd::CLIENT_SECRET.to_string()));
                if spec.slack_app == SlackAppSetup::Complete {
                    env.push(("SLACK_SIGNING_SECRET", sd::SIGNING_SECRET.to_string()));
                }
                env.push(("SLACK_API_BASE_URL", double.api_base_url()));
                env.push(("SLACK_AUTHORIZE_URL", double.authorize_url()));
            }
        }
    }
    env
}

/// The scenario's [`InstanceSpec`] as `IntegrationSettings`, read through the
/// same `IntegrationSettings::from_lookup` the binary reads its environment
/// with. `worker` false adds `FOUNDRY_INTEGRATIONS_WORKER=off`.
fn scenario_settings(
    world: &FoundryWorld,
    worker: bool,
    default_public_url: &str,
    who: &str,
) -> foundry_app::integrations::config::IntegrationSettings {
    use foundry_app::integrations::config::IntegrationSettings;
    let mut env = instance_env(world);
    if !worker {
        env.push(("FOUNDRY_INTEGRATIONS_WORKER", "off".to_string()));
    }
    let lookup = |name: &str| {
        env.iter()
            .find(|(setting, _)| *setting == name)
            .map(|(_, value)| value.clone())
    };
    IntegrationSettings::from_lookup(lookup, default_public_url)
        .unwrap_or_else(|err| panic!("{who} refuses the scenario's settings: {err}"))
}

/// Hand the scenario's [`InstanceSpec`] to the in-process app — ALL of it,
/// as [`instance_env`] spells it, through the same
/// `IntegrationSettings::from_lookup` the binary reads its environment with,
/// so every setting the app reads reaches it (today the Slack trio and URL
/// seams, the secrets keys and the public URL; the worker and email settings
/// as their readers land). The app reads the installed integrations per
/// request, so this takes effect at once.
///
/// When Foundry runs as replicas, every replica gets the same settings with
/// its own delivery worker on (each replica is an instance, as production
/// wires one); the in-process app then runs no worker of its own.
pub async fn rewire(world: &mut FoundryWorld) {
    use foundry_app::integrations::Integrations;
    let harness = tm::harness(world);
    // Subprocess and replica scenarios: the in-process app runs no delivery
    // worker of its own on the shared schema.
    let settings = scenario_settings(
        world,
        world.eip.spec.in_process_worker,
        &harness.base_url(),
        "the in-process app",
    );
    harness
        .app
        .state
        .integrations
        .install(Integrations::new(settings));
    if let Some(replicas) = &world.eip.replicas {
        let public_url = replicas.base_url();
        replicas.install_integrations(|| scenario_settings(world, true, &public_url, "a replica"));
    }
}

// ===========================================================================
// Slack: connect, reconnect, disconnect, return addresses, events
// ===========================================================================

/// "Priya chooses Add to Slack and approves Foundry for the Slack workspace X"
/// (also: reconnects). Follows Foundry's redirect to Slack's approval screen,
/// approves there, and follows Slack back to the return address.
pub async fn connect_slack(world: &mut FoundryWorld, who: &str, approval: Approval) -> Answer {
    let double = ensure_slack(world).await;
    let start = get(world, who, SLACK_INSTALL).await;
    let to_slack = start.location.clone().unwrap_or_default();
    assert!(
        start.status.is_redirection() && to_slack.starts_with(&double.authorize_url()),
        "Add to Slack sends {who} to Slack's approval screen (ADR-EIP-005 decision 4); {}",
        tm::describe(&start)
    );
    double.next_approval(approval);
    let back = http(world)
        .get(&to_slack)
        .send()
        .await
        .expect("Slack's approval screen");
    let back = back
        .headers()
        .get(LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .expect("Slack's approval screen redirects back to Foundry");
    let path = back
        .strip_prefix(&base(world))
        .map(str::to_string)
        .unwrap_or_else(|| {
            let i = back
                .find(SLACK_RETURN)
                .unwrap_or_else(|| panic!("Slack returns to Foundry's return address, got {back}"));
            back[i..].to_string()
        });
    let answer = get(world, who, &path).await;
    follow(world, who, answer).await
}

pub async fn when_connect_slack(world: &mut FoundryWorld, who: &str, team: &str) {
    capture_before(world).await;
    let answer = connect_slack(world, who, Approval::Approve(SlackTeam::named(team))).await;
    world.eip.when_answer = Some(answer);
}

pub async fn when_cancel_slack(world: &mut FoundryWorld, who: &str) {
    capture_before(world).await;
    let answer = connect_slack(world, who, Approval::Cancel).await;
    world.eip.when_answer = Some(answer);
}

/// "Priya has connected Canzan Labs to Slack" — through the product's own flow.
pub async fn given_slack_connected(world: &mut FoundryWorld) {
    if world.eip.spec.slack_app == SlackAppSetup::None {
        given_instance(world, "the Foundry Slack app set up").await;
    }
    let answer = connect_slack(
        world,
        "Priya",
        Approval::Approve(SlackTeam::named("Canzan Labs")),
    )
    .await;
    went_through(&answer, "Priya's Add to Slack");
    assert!(
        text(&answer).contains("Connected to Canzan Labs"),
        "precondition: Priya reads that Canzan Labs is connected; {}",
        tm::describe(&answer)
    );
}

/// "Canzan Labs has no Slack connection" / "… no Slack or Matrix connection".
pub async fn given_no_connection(world: &mut FoundryWorld) {
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "precondition: Priya opens the workspace integrations page; {}",
        tm::describe(&page)
    );
    assert!(
        !text(&page).contains("Connected to") && !text(&page).contains("Connected as"),
        "precondition: no connection is shown; {}",
        tm::describe(&page)
    );
}

/// "Priya holds a Slack return address with <state>".
pub async fn given_held_return(world: &mut FoundryWorld, which: HeldReturn) {
    let double = ensure_slack(world).await;
    let url = match which {
        HeldReturn::NeverIssued => format!(
            "{SLACK_RETURN}?code=eip-code-nobody-issued&state=eip-state-{}",
            uuid::Uuid::new_v4().simple()
        ),
        HeldReturn::AlreadyUsed | HeldReturn::OlderThanTenMinutes | HeldReturn::AnotherSession => {
            // Start the flow (in another of Priya's sessions for the last row),
            // let Slack approve, and keep the address Slack sends back.
            if which == HeldReturn::AnotherSession {
                let key = (base(world), tm::resolve_person(world, "Priya").email);
                let other = sign_in_at(&http(world), &key.0, &key.1, tm::ROSTER_PASSWORD).await;
                let first = world.eip.sessions.insert(key.clone(), other);
                let held = approved_return(world, &double).await;
                match first {
                    Some(s) => world.eip.sessions.insert(key, s),
                    None => world.eip.sessions.remove(&key),
                };
                held
            } else {
                let held = approved_return(world, &double).await;
                if which == HeldReturn::AlreadyUsed {
                    let used = get(world, "Priya", &held).await;
                    went_through(&used, "the first use of the Slack return");
                } else {
                    tm::harness(world)
                        .fake_clock
                        .advance(Duration::from_secs(11 * 60));
                }
                held
            }
        }
    };
    world.eip.held_return = Some(url);
}

async fn approved_return(world: &mut FoundryWorld, double: &SlackDouble) -> String {
    let start = get(world, "Priya", SLACK_INSTALL).await;
    let to_slack = start.location.clone().unwrap_or_default();
    assert!(
        start.status.is_redirection(),
        "precondition: Add to Slack redirects to Slack; {}",
        tm::describe(&start)
    );
    double.next_approval(Approval::Approve(SlackTeam::named("Canzan Labs")));
    let back = http(world)
        .get(&to_slack)
        .send()
        .await
        .expect("Slack's approval screen");
    let back = back
        .headers()
        .get(LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("a return")
        .to_string();
    let i = back
        .find(SLACK_RETURN)
        .expect("Slack returns to the return address");
    back[i..].to_string()
}

pub async fn when_open_held_return(world: &mut FoundryWorld) {
    capture_before(world).await;
    world
        .eip
        .shown_before
        .insert("exchanged".into(), slack(world).exchanged_codes().len());
    let path = world
        .eip
        .held_return
        .clone()
        .expect("a Given held a Slack return address");
    let answer = get(world, "Priya", &path).await;
    let answer = follow(world, "Priya", answer).await;
    world.eip.when_answer = Some(answer);
}

pub fn then_code_never_exchanged(world: &FoundryWorld) {
    let held = world.eip.held_return.clone().expect("a held return");
    let code = held
        .split("code=")
        .nth(1)
        .and_then(|r| r.split('&').next())
        .unwrap_or_default()
        .to_string();
    let exchanged = slack(world).exchanged_codes();
    let before = world
        .eip
        .shown_before
        .get("exchanged")
        .copied()
        .unwrap_or(0);
    assert!(
        !exchanged[before.min(exchanged.len())..].contains(&code),
        "Foundry never asked Slack to exchange the forged return's code; exchanged since the When: {:?}",
        &exchanged[before.min(exchanged.len())..]
    );
}

/// "Slack tells Foundry the Foundry app was uninstalled from Canzan Labs".
pub async fn slack_uninstall_event(world: &mut FoundryWorld) {
    let double = slack(world);
    let base = base(world);
    let (status, body) = double
        .post_event(
            &base,
            SLACK_EVENTS,
            &sd::uninstall_envelope(&SlackTeam::named("Canzan Labs"), sd::APP_ID),
            Some(sd::SIGNING_SECRET),
            Duration::ZERO,
        )
        .await;
    assert!(
        status.is_success(),
        "Foundry accepts Slack's signed uninstall event; {status}: {body}"
    );
}

pub async fn when_slack_uninstall_event(world: &mut FoundryWorld) {
    capture_before(world).await;
    slack_uninstall_event(world).await;
}

/// The outline of messages that are not Slack's.
pub async fn when_post_event(world: &mut FoundryWorld, message: &str) {
    capture_before(world).await;
    let double = slack(world);
    let base = base(world);
    let labs = SlackTeam::named("Canzan Labs");
    let (envelope, secret, skew) = match message.trim() {
        "an uninstall of Canzan Labs that is not signed" => (
            sd::uninstall_envelope(&labs, sd::APP_ID),
            None,
            Duration::ZERO,
        ),
        "an uninstall of Canzan Labs signed with the wrong secret" => (
            sd::uninstall_envelope(&labs, sd::APP_ID),
            Some("eip-not-the-signing-secret"),
            Duration::ZERO,
        ),
        "an uninstall of Canzan Labs signed more than 5 minutes ago" => (
            sd::uninstall_envelope(&labs, sd::APP_ID),
            Some(sd::SIGNING_SECRET),
            Duration::from_secs(6 * 60),
        ),
        "an uninstall of another Slack workspace" => (
            sd::uninstall_envelope(&SlackTeam::named("Canzan Personal"), sd::APP_ID),
            Some(sd::SIGNING_SECRET),
            Duration::ZERO,
        ),
        "an uninstall of another Slack app in Canzan Labs" => (
            sd::uninstall_envelope(&labs, "A0SOMEOTHERAPP"),
            Some(sd::SIGNING_SECRET),
            Duration::ZERO,
        ),
        other => panic!("unknown events message {other:?}"),
    };
    let (status, body) = double
        .post_event(&base, SLACK_EVENTS, &envelope, secret, skew)
        .await;
    world.eip.when_answer = Some(Answer {
        path: SLACK_EVENTS.into(),
        status,
        content_type: None,
        location: None,
        session: None,
        body,
    });
}

/// "Foundry refuses as unauthorised | acknowledges without acting on the message".
pub fn then_event_answer(world: &FoundryWorld, how: &str) {
    let answer = world
        .eip
        .when_answer
        .as_ref()
        .expect("the When posted to the events address");
    match how.trim() {
        "refuses as unauthorised" => assert_eq!(
            answer.status,
            StatusCode::UNAUTHORIZED,
            "Foundry refuses the message as unauthorised; {}",
            answer.body
        ),
        "acknowledges without acting on" => assert!(
            answer.status.is_success(),
            "Foundry acknowledges the message (Slack retries otherwise); {} {}",
            answer.status,
            answer.body
        ),
        other => panic!("unknown events answer {other:?}"),
    }
}

pub async fn when_url_verification(world: &mut FoundryWorld) {
    let double = slack(world);
    let base = base(world);
    let challenge = format!("eip-challenge-{}", uuid::Uuid::new_v4().simple());
    let (status, body) = double
        .post_event(
            &base,
            SLACK_EVENTS,
            &sd::url_verification_envelope(&challenge),
            Some(sd::SIGNING_SECRET),
            Duration::ZERO,
        )
        .await;
    world.eip.comment_text = Some(challenge);
    world.eip.when_answer = Some(Answer {
        path: SLACK_EVENTS.into(),
        status,
        content_type: None,
        location: None,
        session: None,
        body,
    });
}

pub fn then_challenge_answered(world: &FoundryWorld) {
    let answer = world
        .eip
        .when_answer
        .as_ref()
        .expect("the When verified the events address");
    let challenge = world.eip.comment_text.as_deref().expect("the challenge");
    assert!(
        answer.status.is_success() && answer.body.contains(challenge),
        "Foundry echoes Slack's challenge; {} {}",
        answer.status,
        answer.body
    );
}

/// "the Foundry app has been removed from Canzan Labs' Slack without Foundry being told".
pub fn given_app_removed(world: &mut FoundryWorld) {
    slack(world).remove_app("token_revoked");
}

pub async fn when_disconnect_dialog(world: &mut FoundryWorld, who: &str, kind: Kind) {
    capture_before(world).await;
    let answer = get(world, who, kind.disconnect_path()).await;
    world.eip.when_answer = Some(answer);
}

pub async fn disconnect(
    world: &mut FoundryWorld,
    who: &str,
    kind: Kind,
    with_token: bool,
) -> Answer {
    let door = kind.disconnect_path();
    let dialog = get(world, who, door).await;
    let fields = form_fields(&dialog, door);
    let answer = post(world, who, door, &fields, with_token).await;
    follow(world, who, answer).await
}

pub async fn given_disconnected(world: &mut FoundryWorld, kind: Kind) {
    let answer = disconnect(world, "Priya", kind, true).await;
    went_through(&answer, "Priya's disconnect");
}

/// A form on the page whose action is `action` (its hidden fields), or none.
fn form_fields(answer: &Answer, action: &str) -> Vec<(String, String)> {
    tm::forms(&answer.body)
        .into_iter()
        .find(|f| f.action == action || f.action.ends_with(action))
        .map(|f| f.fields)
        .unwrap_or_default()
}

// ===========================================================================
// Routes: paths, forms, create, preview, test, lifecycle
// ===========================================================================

pub fn project_path(world: &FoundryWorld, project: &str) -> String {
    let p: ProjectRef = tm::project_ref(world, project);
    format!("/team/{}/project/{}", p.team_slug, p.slug)
}

pub fn integrations_path(world: &FoundryWorld, project: &str) -> String {
    format!("{}/integrations", project_path(world, project))
}

fn destination_ref(world: &mut FoundryWorld, route: &Route) -> String {
    match route.kind {
        Kind::Slack => slack(world).channel_id(&route.destination),
        Kind::Matrix => world.eip.names.real(&route.destination),
    }
}

fn route_fields(world: &mut FoundryWorld, route: &Route) -> Vec<(String, String)> {
    let mut fields = vec![
        ("kind".to_string(), route.kind.wire().to_string()),
        ("destination".to_string(), destination_ref(world, route)),
        ("content_level".to_string(), route.level.wire().to_string()),
    ];
    for e in &route.events {
        fields.push(("events".into(), e.wire().into()));
    }
    for l in &route.lanes {
        fields.push(("lanes".into(), lane_slug(l)));
    }
    if route.shared_confirmed {
        fields.push(("shared_confirmed".into(), "on".into()));
    }
    fields
}

/// A route from the step's words: events ("every chat event", "New issue"),
/// lanes ("Done", "QA or Done", empty = any), the project, the destination, a
/// level (empty = Summary).
pub fn route_from(
    events: &str,
    lanes: &str,
    project: &str,
    destination: &str,
    level: &str,
) -> Route {
    let kind = if destination.contains(':') {
        Kind::Matrix
    } else {
        Kind::Slack
    };
    let mut route = Route::new(project, kind, destination);
    route.events = EventChoice::parse_list(events);
    route.lanes = lanes
        .split(" or ")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    route.level = if level.is_empty() {
        ContentLevel::Summary
    } else {
        ContentLevel::parse(level)
    };
    route
}

/// The route a lead is part-way through creating ("with Link only", "with no
/// event chosen", or nothing).
pub fn draft_route(project: &str, kind: Kind, destination: &str, with: &str) -> Route {
    let mut route = Route::new(project, kind, destination);
    match with {
        "" => {}
        "no event chosen" => route.events.clear(),
        level => route.level = ContentLevel::parse(level),
    }
    route
}

/// "pauses"/"paused" → `pause`; "resumes"/"resumed" → `resume`.
pub fn verb(word: &str) -> &'static str {
    match word {
        "pauses" | "paused" | "pause" => "pause",
        "resumes" | "resumed" | "resume" => "resume",
        other => panic!("a route is paused or resumed, not {other:?}"),
    }
}

/// "<who> has routed <events> on <project> to <destination>" — through the
/// route form, saved, and visible on the page.
pub async fn given_routed(world: &mut FoundryWorld, who: &str, route: Route) {
    let path = format!("{}/routes", integrations_path(world, &route.project));
    let fields = route_fields(world, &route);
    let answer = post(world, who, &path, &fields, true).await;
    let answer = follow(world, who, answer).await;
    went_through(&answer, &format!("{who}'s route to {}", route.destination));
    if route.kind == Kind::Matrix {
        world.eip.matrix_destination = Some(route.destination.clone());
    }
    let status = route_status(world, &route.project, &route.destination).await;
    assert!(
        status.is_some(),
        "precondition: the project's integrations page lists the route to {}; {}",
        route.destination,
        tm::describe(&answer)
    );
}

/// "<who> is creating a <Kind> route from <project> to <destination>".
pub async fn given_creating(world: &mut FoundryWorld, who: &str, route: Route) {
    let new = format!(
        "{}/routes/new?kind={}",
        integrations_path(world, &route.project),
        route.kind.wire()
    );
    let page = get(world, who, &new).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "precondition: {who} opens a new {:?} route; {}",
        route.kind,
        tm::describe(&page)
    );
    world.eip.draft = Some(route);
}

pub fn draft_mut(world: &mut FoundryWorld) -> &mut Route {
    world
        .eip
        .draft
        .as_mut()
        .expect("a Given began creating a route")
}

/// "<who> has ticked | left blank the shared-channel confirmation": ticking
/// needs the box, so the route form for the draft's channel must render it
/// (K-6's tick); leaving it blank sends no `shared_confirmed`.
pub async fn given_shared_tick(world: &mut FoundryWorld, who: &str, ticked: bool) {
    if ticked {
        let route = world
            .eip
            .draft
            .clone()
            .expect("a Given began creating a route");
        let channel = destination_ref(world, &route);
        let path = format!(
            "{}/routes/new?kind={}&destination={}",
            integrations_path(world, &route.project),
            route.kind.wire(),
            urlencoding::encode(&channel)
        );
        let page = get(world, who, &path).await;
        let routes = format!("{}/routes", integrations_path(world, &route.project));
        let doc = scraper::Html::parse_document(&page.body);
        let form = scraper::Selector::parse("form").expect("selector");
        let tick = scraper::Selector::parse(r#"input[type="checkbox"][name="shared_confirmed"]"#)
            .expect("selector");
        let rendered = doc
            .select(&form)
            .filter(|f| f.value().attr("action") == Some(routes.as_str()))
            .any(|f| f.select(&tick).next().is_some());
        assert!(
            rendered,
            "precondition: the route form for {} offers the shared-channel tick box; {}",
            route.destination,
            tm::describe(&page)
        );
    }
    draft_mut(world).shared_confirmed = ticked;
}

pub async fn when_save_route(world: &mut FoundryWorld, who: &str) {
    capture_before(world).await;
    let route = world
        .eip
        .draft
        .clone()
        .expect("a Given began creating a route");
    let path = format!("{}/routes", integrations_path(world, &route.project));
    let fields = route_fields(world, &route);
    let answer = post(world, who, &path, &fields, true).await;
    let answer = follow(world, who, answer).await;
    world.eip.when_answer = Some(answer);
}

pub async fn when_send_test(world: &mut FoundryWorld, who: &str) {
    capture_before(world).await;
    let route = world
        .eip
        .draft
        .clone()
        .expect("a Given began creating a route");
    let path = format!("{}/routes/test", integrations_path(world, &route.project));
    let fields = route_fields(world, &route);
    let answer = post(world, who, &path, &fields, true).await;
    world.eip.when_answer = Some(answer);
}

pub async fn given_tests_sent(world: &mut FoundryWorld, who: &str, n: usize) {
    for i in 0..n {
        let route = world.eip.draft.clone().expect("a draft route");
        let path = format!("{}/routes/test", integrations_path(world, &route.project));
        let fields = route_fields(world, &route);
        let answer = post(world, who, &path, &fields, true).await;
        went_through(&answer, &format!("test message {}", i + 1));
    }
}

/// The channel names the rendered route form carries for `route`'s channel
/// (its hidden `channel_label.{id}` fields), as a browser would post them.
async fn rendered_channel_labels(
    world: &mut FoundryWorld,
    who: &str,
    route: &Route,
) -> Vec<(String, String)> {
    let channel = destination_ref(world, route);
    let base = integrations_path(world, &route.project);
    let new = format!(
        "{base}/routes/new?kind={}&destination={}",
        route.kind.wire(),
        urlencoding::encode(&channel)
    );
    let page = get(world, who, &new).await;
    form_fields(&page, &format!("{base}/routes"))
        .into_iter()
        .filter(|(name, _)| name.starts_with("channel_label."))
        .collect()
}

pub async fn when_preview(world: &mut FoundryWorld, who: &str) {
    let route = world
        .eip
        .draft
        .clone()
        .expect("a Given began creating a route");
    // The form the lead previews from was rendered before the preview: the
    // channel names it carries ride along, and Slack's listing for that
    // render is not the preview's.
    let labels = rendered_channel_labels(world, who, &route).await;
    capture_before(world).await;
    world
        .eip
        .shown_before
        .insert("calls".into(), slack(world).calls().len());
    let path = format!(
        "{}/routes/preview",
        integrations_path(world, &route.project)
    );
    let mut fields = route_fields(world, &route);
    fields.extend(labels);
    let answer = post(world, who, &path, &fields, true).await;
    world.eip.when_answer = Some(answer);
}

/// "Lena chooses #auth-dev as the destination of a new Slack route on X" —
/// the scripting-off form re-posts for the destination's notices.
pub async fn when_choose_destination(
    world: &mut FoundryWorld,
    who: &str,
    destination: &str,
    project: &str,
) {
    capture_before(world).await;
    let route = Route::new(project, Kind::Slack, destination);
    let channel = slack(world).channel_id(destination);
    let new = format!(
        "{}/routes/new?kind=slack&destination={}",
        integrations_path(world, project),
        urlencoding::encode(&channel)
    );
    let answer = get(world, who, &new).await;
    world.eip.draft = Some(route);
    world.eip.when_answer = Some(answer);
}

pub async fn open_new_route(world: &mut FoundryWorld, who: &str, project: &str) -> Answer {
    let path = format!(
        "{}/routes/new?kind=slack",
        integrations_path(world, project)
    );
    get(world, who, &path).await
}

pub async fn when_open_new_route(world: &mut FoundryWorld, who: &str, project: &str) {
    capture_before(world).await;
    let answer = open_new_route(world, who, project).await;
    world.eip.when_answer = Some(answer);
}

/// "Lena chooses New route on Identity Platform's integrations page".
pub async fn when_choose_new_route(world: &mut FoundryWorld, who: &str, project: &str) {
    capture_before(world).await;
    let page = get(world, who, &integrations_path(world, project)).await;
    let link = tm::link_to(&page, "New route");
    let answer = get(world, who, &link).await;
    world.eip.when_answer = Some(answer);
}

/// The route id behind a destination on the project's page (Priya's or the
/// lead's view).
async fn route_id(world: &mut FoundryWorld, who: &str, project: &str, destination: &str) -> String {
    let page = get(world, who, &integrations_path(world, project)).await;
    let shown = shown_destination(world, destination);
    let doc = parse(&page.body);
    select_all(&doc, "[data-route-destination]")
        .into_iter()
        .find(|e| {
            e.value().attr("data-route-destination") == Some(shown.as_str())
                || e.value().attr("data-route-destination") == Some(destination)
        })
        .and_then(|e| e.value().attr("data-route-id").map(str::to_string))
        .unwrap_or_else(|| {
            panic!(
                "precondition: {project}'s integrations page lists the route to {destination}; {}",
                tm::describe(&page)
            )
        })
}

fn shown_destination(world: &FoundryWorld, destination: &str) -> String {
    world
        .eip
        .names
        .map
        .iter()
        .find(|(s, _)| s == destination)
        .map(|(_, r)| r.clone())
        .unwrap_or_else(|| destination.to_string())
}

/// The project a destination's route lives on in this scenario (the last
/// route made to it).
fn project_of_destination(world: &FoundryWorld, destination: &str) -> String {
    if destination == "#ops-alerts" {
        "Homelab Ops".into()
    } else {
        let _ = world;
        "Identity Platform".into()
    }
}

/// A route lifecycle action (`pause`, `resume`, `delete`) on the route to `destination`.
pub async fn route_action(
    world: &mut FoundryWorld,
    who: &str,
    action: &str,
    destination: &str,
    with_token: bool,
) -> Answer {
    let project = project_of_destination(world, destination);
    let owner = if project == "Homelab Ops" {
        "Priya"
    } else {
        "Lena"
    };
    let id = route_id(world, owner, &project, destination).await;
    let path = format!(
        "{}/routes/{id}/{action}",
        integrations_path(world, &project)
    );
    let fields = if action == "delete" {
        let dialog = get(world, owner, &path).await;
        form_fields(&dialog, &path)
    } else {
        Vec::new()
    };
    let answer = post(world, who, &path, &fields, with_token).await;
    follow(world, who, answer).await
}

pub async fn given_route_action(
    world: &mut FoundryWorld,
    who: &str,
    action: &str,
    destination: &str,
) {
    let answer = route_action(world, who, action, destination, true).await;
    went_through(
        &answer,
        &format!("{who}'s {action} of the route to {destination}"),
    );
}

pub async fn when_route_action(
    world: &mut FoundryWorld,
    who: &str,
    action: &str,
    destination: &str,
) {
    capture_before(world).await;
    let answer = route_action(world, who, action, destination, true).await;
    world.eip.when_answer = Some(answer);
}

pub async fn when_route_action_without_token(
    world: &mut FoundryWorld,
    who: &str,
    action: &str,
    destination: &str,
) {
    capture_before(world).await;
    let answer = route_action(world, who, action, destination, false).await;
    world.eip.when_answer = Some(answer);
}

/// "Lena chooses Delete on the route to #auth-dev" (the GET dialog).
pub async fn delete_dialog(world: &mut FoundryWorld, who: &str, destination: &str) -> Answer {
    let project = project_of_destination(world, destination);
    let id = route_id(world, "Lena", &project, destination).await;
    let path = format!("{}/routes/{id}/delete", integrations_path(world, &project));
    get(world, who, &path).await
}

pub async fn when_delete_dialog(world: &mut FoundryWorld, who: &str, destination: &str) {
    capture_before(world).await;
    let answer = delete_dialog(world, who, destination).await;
    world.eip.in_front = Some(answer.clone());
    world.eip.when_answer = Some(answer);
}

pub async fn when_confirm_delete(world: &mut FoundryWorld, who: &str) {
    let dialog = world
        .eip
        .in_front
        .clone()
        .expect("a Given opened the delete dialog");
    capture_before(world).await;
    let action = tm::forms(&dialog.body)
        .into_iter()
        .find(|f| f.action.ends_with("/delete"))
        .unwrap_or_else(|| {
            panic!(
                "the delete dialog carries its confirm form; {}",
                tm::describe(&dialog)
            )
        });
    let answer = post(world, who, &action.action, &action.fields, true).await;
    let answer = follow(world, who, answer).await;
    world.eip.when_answer = Some(answer);
}

/// "Lena has added New comment to the route to #auth-releases" (the edit form).
pub async fn edit_add_event(
    world: &mut FoundryWorld,
    who: &str,
    event: EventChoice,
    destination: &str,
) -> Answer {
    let project = project_of_destination(world, destination);
    let id = route_id(world, who, &project, destination).await;
    let path = format!("{}/routes/{id}/edit", integrations_path(world, &project));
    let page = get(world, who, &path).await;
    let mut fields = form_fields(&page, &path);
    fields.retain(|(k, _)| k != "events" || !k.is_empty());
    let mut checked: Vec<String> = attr_of(&page.body, "input[name=events][checked]", "value");
    checked.push(event.wire().to_string());
    fields.retain(|(k, _)| k != "events");
    for e in checked {
        fields.push(("events".into(), e));
    }
    for (k, v) in checked_fields(&page.body, "lanes") {
        fields.push((k, v));
    }
    let answer = post(world, who, &path, &fields, true).await;
    follow(world, who, answer).await
}

fn checked_fields(body: &str, name: &str) -> Vec<(String, String)> {
    attr_of(body, &format!("input[name={name}][checked]"), "value")
        .into_iter()
        .map(|v| (name.to_string(), v))
        .collect()
}

pub async fn when_edit_open(world: &mut FoundryWorld, who: &str, destination: &str) {
    capture_before(world).await;
    let project = project_of_destination(world, destination);
    let id = route_id(world, "Lena", &project, destination).await;
    let path = format!("{}/routes/{id}/edit", integrations_path(world, &project));
    let answer = get(world, who, &path).await;
    world.eip.when_answer = Some(answer);
}

/// "<who> opens the history of the route to <destination>".
pub async fn open_history(world: &mut FoundryWorld, who: &str, destination: &str) -> Answer {
    let project = project_of_destination(world, destination);
    let id = route_id(world, "Lena", &project, destination).await;
    let path = format!("{}/routes/{id}/history", integrations_path(world, &project));
    get(world, who, &path).await
}

pub async fn when_open_history(world: &mut FoundryWorld, who: &str, destination: &str) {
    capture_before(world).await;
    let answer = open_history(world, who, destination).await;
    world.eip.when_answer = Some(answer);
}

/// The route row's status text on the project's page (Priya sees every
/// project), or `None` when the route is not listed.
pub async fn route_status(
    world: &mut FoundryWorld,
    project: &str,
    destination: &str,
) -> Option<String> {
    let page = get(world, "Priya", &integrations_path(world, project)).await;
    let shown = shown_destination(world, destination);
    let doc = parse(&page.body);
    select_all(&doc, "[data-route-destination]")
        .into_iter()
        .find(|e| {
            e.value().attr("data-route-destination") == Some(shown.as_str())
                || e.value().attr("data-route-destination") == Some(destination)
        })
        .map(|e| {
            e.text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .map(|t| world.eip.names.as_scenario(&t))
}

pub async fn then_route_reads(
    world: &mut FoundryWorld,
    destination: &str,
    status: &str,
    with_time: bool,
) {
    let project = project_of_destination(world, destination);
    let mut seen = String::new();
    let start = Instant::now();
    while start.elapsed() < STATUS_BUDGET {
        seen = route_status(world, &project, destination)
            .await
            .unwrap_or_default();
        let time_ok = !with_time || regex_lite_time(&seen);
        if seen.contains(status) && time_ok {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!(
        "the route to {destination} reads {status:?}{} within {STATUS_BUDGET:?}; it reads {seen:?}",
        if with_time {
            " with the time of its last delivery"
        } else {
            ""
        }
    );
}

fn regex_lite_time(s: &str) -> bool {
    s.contains("last delivery")
        && s.as_bytes().windows(5).any(|w| {
            w[0].is_ascii_digit()
                && w[1].is_ascii_digit()
                && w[2] == b':'
                && w[3].is_ascii_digit()
                && w[4].is_ascii_digit()
        })
}

pub async fn then_route_reads_prefix(world: &mut FoundryWorld, destination: &str, prefix: &str) {
    then_route_reads(world, destination, prefix, false).await;
}

pub async fn then_routes_each_read(
    world: &mut FoundryWorld,
    destinations: &[String],
    status: &str,
) {
    for d in destinations {
        then_route_reads(world, d, status, false).await;
    }
}

pub async fn then_lists_route(
    world: &mut FoundryWorld,
    project: &str,
    destination: &str,
    listed: bool,
    status: Option<&str>,
) {
    let seen = route_status(world, project, destination).await;
    match (listed, seen) {
        (true, Some(s)) => {
            if let Some(status) = status {
                assert!(s.contains(status), "the route to {destination} reads {status:?}; it reads {s:?}");
            }
        }
        (true, None) => panic!("{project}'s integrations page lists the route to {destination}"),
        (false, None) => {}
        (false, Some(s)) => panic!("{project}'s integrations page no longer lists the route to {destination}; it reads {s:?}"),
    }
}

/// "<project> still has no route" — Priya's view lists no route at all (and the
/// control: the page itself opens).
pub async fn then_no_route(world: &mut FoundryWorld, project: &str) {
    let page = get(world, "Priya", &integrations_path(world, project)).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "control: Priya opens {project}'s integrations page; {}",
        tm::describe(&page)
    );
    let rows = attr_of(
        &page.body,
        "[data-route-destination]",
        "data-route-destination",
    );
    assert!(rows.is_empty(), "{project} has no route; it lists {rows:?}");
}

// ===========================================================================
// Board actions (the event sources, shipped doors)
// ===========================================================================

fn key_number(key: &str) -> i64 {
    key.rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{key:?} is an issue key"))
}

fn project_of_key(world: &FoundryWorld, key: &str) -> String {
    if key.starts_with("OPS-") {
        return "Homelab Ops".into();
    }
    world
        .eip
        .issue_projects
        .get(key)
        .cloned()
        .unwrap_or_else(|| "Identity Platform".into())
}

/// File an issue through the board's own form; returns its key.
pub async fn file(world: &mut FoundryWorld, who: &str, project: &str, title: &str) -> String {
    let number = *world.eip.next_numbers.get(project).unwrap_or(&1);
    let prefix = if project == "Homelab Ops" {
        "OPS"
    } else {
        "AUTH"
    };
    let key = format!("{prefix}-{number}");
    let path = format!("{}/issues", project_path(world, project));
    // The board's form files into the first lane; it takes no lane.
    let fields = vec![
        ("title".to_string(), title.to_string()),
        ("description".to_string(), String::new()),
    ];
    let answer = post(world, who, &path, &fields, true).await;
    went_through(&answer, &format!("{who} filing {title:?}"));
    world
        .eip
        .next_numbers
        .insert(project.to_string(), number + 1);
    world
        .eip
        .issue_projects
        .insert(key.clone(), project.to_string());
    world.eip.last_key = Some(key.clone());
    world.eip.last_title = Some(title.to_string());
    key
}

pub async fn move_issue(world: &mut FoundryWorld, who: &str, key: &str, lane: &str) {
    let project = project_of_key(world, key);
    let path = format!(
        "{}/issues/{}/state",
        project_path(world, &project),
        key_number(key)
    );
    let answer = post(
        world,
        who,
        &path,
        &[(
            "state".into(),
            current_lane_slug(world, &project, lane).await,
        )],
        true,
    )
    .await;
    went_through(&answer, &format!("{who} moving {key} to {lane}"));
}

pub async fn reorder(world: &mut FoundryWorld, who: &str, key: &str, lane: &str) {
    let project = project_of_key(world, key);
    let path = format!(
        "{}/issues/{}/state",
        project_path(world, &project),
        key_number(key)
    );
    let answer = post(
        world,
        who,
        &path,
        &[(
            "state".into(),
            current_lane_slug(world, &project, lane).await,
        )],
        true,
    )
    .await;
    went_through(&answer, &format!("{who} reordering {key} inside {lane}"));
}

pub async fn comment(world: &mut FoundryWorld, who: &str, key: &str, body: &str) {
    let project = project_of_key(world, key);
    let path = format!(
        "{}/issues/{}/comments",
        project_path(world, &project),
        key_number(key)
    );
    let answer = post(
        world,
        who,
        &path,
        &[("body".into(), body.to_string())],
        true,
    )
    .await;
    went_through(&answer, &format!("{who} commenting on {key}"));
    world.eip.comment_text = Some(body.to_string());
    world.eip.commented_key = Some(key.to_string());
}

/// The newest comment's address on an issue, read from its page the way the
/// browser finds it: the comment card's Delete button `hx-delete`s it, and its
/// Edit button `hx-get`s the edit form beneath it.
async fn latest_comment_path(world: &mut FoundryWorld, who: &str, key: &str) -> String {
    let project = project_of_key(world, key);
    let issue = format!(
        "{}/issues/{}",
        project_path(world, &project),
        key_number(key)
    );
    let page = get(world, who, &issue).await;
    let marker = format!("{issue}/comments/");
    let doc = parse(&page.body);
    select_all(&doc, "[hx-delete], [hx-get]")
        .into_iter()
        .filter_map(|e| {
            e.value()
                .attr("hx-delete")
                .or_else(|| e.value().attr("hx-get"))
                .map(str::to_string)
        })
        .filter(|a| a.starts_with(&marker))
        .map(|a| a.trim_end_matches("/edit").to_string())
        .last()
        .unwrap_or_else(|| {
            panic!(
                "precondition: {key}'s page offers its comment's Edit and Delete; {}",
                tm::describe(&page)
            )
        })
}

/// An htmx request as the browser sends it from a comment card: the session
/// and CSRF cookies, `HX-Request`, and either the form's fields (with its
/// `_csrf`) or the `x-csrf-token` header the Delete button echoes.
async fn htmx(
    world: &mut FoundryWorld,
    who: &str,
    method: reqwest::Method,
    path: &str,
    form: Option<&[(String, String)]>,
) -> Answer {
    let email = email_of(world, who);
    let cookie = cookie(world, email.as_deref()).await;
    let mut request = http(world)
        .request(method.clone(), format!("{}{path}", base(world)))
        .header(COOKIE, cookie)
        .header("HX-Request", "true");
    request = match form {
        Some(fields) => {
            let mut fields: Vec<(String, String)> = fields
                .iter()
                .filter(|(k, _)| k != "_csrf")
                .cloned()
                .collect();
            fields.push(("_csrf".into(), world.eip.csrf.clone()));
            request.form(&fields)
        }
        None => request.header("x-csrf-token", world.eip.csrf.clone()),
    };
    let resp = request
        .send()
        .await
        .unwrap_or_else(|e| panic!("{method} {path}: {e}"));
    let answer = tm::into_answer(path, resp).await;
    keep(world, email, answer.clone());
    answer
}

/// Edit (Edit, change the text, Save) or delete (Delete) the newest comment on
/// `key`, through the comment card's own htmx doors.
pub async fn change_comment(world: &mut FoundryWorld, who: &str, key: &str, delete: bool) {
    let path = latest_comment_path(world, who, key).await;
    let answer = if delete {
        htmx(world, who, reqwest::Method::DELETE, &path, None).await
    } else {
        let form = htmx(
            world,
            who,
            reqwest::Method::GET,
            &format!("{path}/edit"),
            None,
        )
        .await;
        went_through(&form, &format!("{who} opening the comment's edit form"));
        let doc = parse(&form.body);
        let patch = select_all(&doc, "form[hx-patch]")
            .into_iter()
            .find_map(|e| e.value().attr("hx-patch").map(str::to_string))
            .unwrap_or_else(|| {
                panic!(
                    "precondition: the edit form saves through its hx-patch; {}",
                    tm::describe(&form)
                )
            });
        htmx(
            world,
            who,
            reqwest::Method::PATCH,
            &patch,
            Some(&[(
                "body_markdown".into(),
                "Edited: can you verify the Grafana login?".into(),
            )]),
        )
        .await
    };
    went_through(
        &answer,
        &format!(
            "{who} {} the comment",
            if delete { "deleting" } else { "editing" }
        ),
    );
}

pub async fn delete_issue(world: &mut FoundryWorld, who: &str, key: &str) {
    let project = project_of_key(world, key);
    let path = format!(
        "{}/issues/{}/delete",
        project_path(world, &project),
        key_number(key)
    );
    let dialog = get(world, who, &path).await;
    let fields = form_fields(&dialog, &path);
    let answer = post(world, who, &path, &fields, true).await;
    went_through(&answer, &format!("{who} deleting {key}"));
}

pub async fn board_action(world: &mut FoundryWorld, action: BoardAction, who: &str) {
    match action {
        BoardAction::File { project, title } => {
            file(world, who, &project, &title).await;
        }
        BoardAction::Move { key, to_lane } => move_issue(world, who, &key, &to_lane).await,
        BoardAction::Reorder { key, lane, .. } => reorder(world, who, &key, &lane).await,
        BoardAction::Comment { key, body } => comment(world, who, &key, &body).await,
        BoardAction::DeleteIssue { key } => delete_issue(world, who, &key).await,
    }
}

/// A When that is a board action: snapshot first, then act, timing the answer.
pub async fn when_board(world: &mut FoundryWorld, who: &str, action: BoardAction) {
    capture_before(world).await;
    world.eip.when_started = Some(Instant::now());
    board_action(world, action, who).await;
}

/// "the lane Spikes holds N cards": each filed through the board's form (a
/// New issue event, which the scenarios' Issue deleted / Issue moved routes
/// are never given), then placed in the lane by the store, the way a board
/// that has held them for a while looks. The board's move door is not used:
/// the route is already saved, and a move into the lane would be an Issue
/// moved event the lane-move scenario's Then would count.
pub async fn given_lane_cards(world: &mut FoundryWorld, lane: &str, n: usize) {
    let project = "Identity Platform";
    for i in 0..n {
        let key = file(world, "Lena", project, &format!("Spike {}", i + 1)).await;
        let placed = sqlx::query(
            "UPDATE issues SET state = $3, position = $4
              WHERE number = $2 AND project_id = (SELECT id FROM projects WHERE name = $1)",
        )
        .bind(project)
        .bind(key_number(&key) as i32)
        .bind(lane_slug(lane))
        .bind(i as i32)
        .execute(&tm::pool(world))
        .await
        .expect("place a card in the lane");
        assert_eq!(
            placed.rows_affected(),
            1,
            "precondition: {key} is in {lane}"
        );
    }
}

/// Delete a lane through its dialog: its cards deleted, or moved to `to`.
pub async fn delete_lane(world: &mut FoundryWorld, who: &str, lane: &str, move_to: Option<&str>) {
    let path = format!(
        "{}/lanes/{}/delete",
        project_path(world, "Identity Platform"),
        lane_slug(lane)
    );
    let dialog = get(world, who, &path).await;
    let mut fields = form_fields(&dialog, &path);
    fields.retain(|(k, _)| k != "fate" && k != "destination");
    match move_to {
        None => fields.push(("fate".into(), "delete".into())),
        Some(to) => {
            fields.push(("fate".into(), "move".into()));
            fields.push(("destination".into(), lane_slug(to)));
        }
    }
    let answer = post(world, who, &path, &fields, true).await;
    went_through(&answer, &format!("{who} deleting the lane {lane}"));
}

pub async fn rename_lane(world: &mut FoundryWorld, who: &str, lane: &str, to: &str) {
    let path = format!(
        "{}/lanes/{}/edit",
        project_path(world, "Identity Platform"),
        lane_slug(lane)
    );
    let answer = post(world, who, &path, &[("label".into(), to.to_string())], true).await;
    went_through(&answer, &format!("{who} renaming the lane {lane}"));
}

/// File an issue with a description and an attachment (the never-sent row).
pub async fn file_with_secrets(
    world: &mut FoundryWorld,
    who: &str,
    title: &str,
    description: &str,
    attachment: &str,
) {
    let project = "Identity Platform";
    let key = file(world, who, project, title).await;
    let edit = format!(
        "{}/issues/{}/edit",
        project_path(world, project),
        key_number(&key)
    );
    let answer = post(
        world,
        who,
        &edit,
        &[
            ("title".into(), title.into()),
            ("description".into(), description.into()),
        ],
        true,
    )
    .await;
    went_through(&answer, "the description");
    world.eip.description = Some((title.to_string(), description.to_string()));
    let email = email_of(world, who);
    let cookie = cookie(world, email.as_deref()).await;
    let url = format!(
        "{}{}/issues/{}/attachments",
        base(world),
        project_path(world, project),
        key_number(&key)
    );
    let part = reqwest::multipart::Part::bytes(b"LDAP_BIND_PASSWORD=redacted\n".to_vec())
        .file_name(attachment.to_string());
    // A multipart post carries the double-submit token in the header, as the
    // board's upload (static/js/csrf-upload.js) mirrors the cookie into it.
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("_csrf", world.eip.csrf.clone());
    let resp = http(world)
        .post(url)
        .header(COOKIE, cookie)
        .header("x-csrf-token", world.eip.csrf.clone())
        .multipart(form)
        .send()
        .await
        .expect("upload the attachment");
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "precondition: the attachment uploads; {}",
        resp.status()
    );
}

/// "Lena has deleted the project Identity Platform" (its confirm page).
pub async fn delete_project(world: &mut FoundryWorld, who: &str, project: &str) {
    let path = format!("{}/delete", project_path(world, project));
    let dialog = get(world, who, &path).await;
    let fields = form_fields(&dialog, &path);
    let answer = post(world, who, &path, &fields, true).await;
    went_through(&answer, &format!("{who} deleting {project}"));
}

/// "the instance operator restores Identity Platform" — the prebuilt binary.
pub async fn restore_project(world: &mut FoundryWorld, project: &str) {
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM projects WHERE name = $1")
        .bind(project)
        .fetch_one(&tm::pool(world))
        .await
        .expect("the project's id (operator input)");
    let base_url = ensure_postgres().await;
    let schema = tm::harness(world).schema.clone();
    let database_url = format!("{base_url}?options=-csearch_path%3D{schema}");
    let out = tokio::task::spawn_blocking(move || {
        assert_cmd::Command::cargo_bin("foundry")
            .expect("the prebuilt foundry binary")
            .env("DATABASE_URL", database_url)
            .args(["doctor", "restore-project", &id.to_string()])
            .output()
            .expect("run foundry doctor restore-project")
    })
    .await
    .expect("join the operator's command");
    assert!(
        out.status.success(),
        "precondition: the operator restores {project}; {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

pub async fn remove_from_team(world: &mut FoundryWorld, who: &str, person: &str, team: &str) {
    let slug = tm::team_ref(world, team).slug;
    let id = tm::resolve_person(world, person).id;
    let path = format!("/team/{slug}/members/{id}/remove");
    let answer = post(world, who, &path, &[], true).await;
    went_through(&answer, &format!("{who} removing {person} from {team}"));
}

// ===========================================================================
// Slack moods and time
// ===========================================================================

pub fn slack_mood(world: &mut FoundryWorld, mood: Mood) {
    if matches!(mood, Mood::Silent | Mood::Held) {
        world.eip.silence_seen = true;
    }
    slack(world).set_standing(mood);
}

pub fn slack_heals(world: &mut FoundryWorld) {
    slack(world).heal();
}

pub async fn when_slack_heals(world: &mut FoundryWorld) {
    capture_before(world).await;
    slack(world).heal();
}

/// Foundry's call timeout towards Slack (`foundry-app` `integrations::slack`
/// `CALL_TIMEOUT`, 10 s) plus a margin: after it, a send Slack never answered
/// has been settled by Foundry (given up on, or scheduled to retry).
const HUNG_SEND_SETTLES: Duration = Duration::from_secs(12);

/// "Slack answers again and a minute passes": Slack heals, the send it left
/// hanging times out on Foundry's side (real time), then the fake clock moves
/// 60 s, past the 30 s retry, so a message still waiting would now be sent.
pub async fn when_slack_heals_and_a_minute_passes(world: &mut FoundryWorld) {
    capture_before(world).await;
    slack(world).heal();
    tokio::time::sleep(HUNG_SEND_SETTLES).await;
    advance(world, Duration::from_secs(60)).await;
}

pub fn slack_next_answer(world: &mut FoundryWorld, channel: &str, answer: SlackAnswer) {
    world.eip.scripted_answer_channel = Some(channel.to_owned());
    let double = slack(world);
    match &answer {
        SlackAnswer::Archived => double.archive(channel),
        SlackAnswer::NotInChannel => double.remove_foundry_from(channel),
        // No answer lasts: every post to the channel hangs until Slack answers
        // again, so Foundry's retries meet the same silence (a one-off hang
        // would let the first retry deliver). Connections are still accepted,
        // so each attempt reaches the double and is counted.
        SlackAnswer::NoAnswer => {
            world.eip.silence_seen = true;
            double.set_channel_standing(channel, answer.mood());
        }
        _ => double.queue_post_mood(channel, answer.mood()),
    }
}

/// Advance every injected Clock the scenario's Foundry uses (the in-process
/// app's and, when it runs as replicas, theirs), then give the worker a poll.
/// Subprocess scenarios run on real time and shorten windows by setting.
pub async fn advance(world: &mut FoundryWorld, by: Duration) {
    tm::harness(world).fake_clock.advance(by);
    if let Some(r) = &world.eip.replicas {
        r.fake_clock.advance(by);
    }
    tokio::time::sleep(Duration::from_millis(world.eip.spec.poll_ms * 3)).await;
}

pub async fn when_advance(world: &mut FoundryWorld, by: Duration) {
    capture_before(world).await;
    advance(world, by).await;
}

/// "Slack asked Foundry to retry Sam's comment on AUTH-7 after 30 seconds".
pub async fn given_rate_limited_comment(world: &mut FoundryWorld, key: &str, secs: u64) {
    slack(world).queue_post_mood("#auth-dev", Mood::SlowDown(secs));
    comment(world, "Sam", key, "Rate-limited comment.").await;
    let double = slack(world);
    eventually(
        DELIVERY_BUDGET,
        "Slack is asked once and answers slow down",
        || {
            let double = double.clone();
            async move {
                if double.posts_to("#auth-dev").iter().any(|p| !p.accepted) {
                    Ok(())
                } else {
                    Err("no post yet".into())
                }
            }
        },
    )
    .await;
}

// ===========================================================================
// What channels show
// ===========================================================================

fn key_line(m: &PostedMessage, key: &str) -> bool {
    m.texts().iter().any(|t| t.contains(key))
}

pub async fn then_message(
    world: &mut FoundryWorld,
    channel: &str,
    first: &str,
    second: &str,
    link: Option<bool>,
) {
    let double = slack(world);
    let (first, second) = (first.to_string(), second.to_string());
    eventually(
        DELIVERY_BUDGET,
        &format!("{channel} shows {first:?} / {second:?}"),
        || {
            let double = double.clone();
            let (first, second) = (first.clone(), second.clone());
            let channel = channel.to_string();
            async move {
                let shown = double.shown_in(&channel);
                if shown.iter().any(|m| {
                    let t = m.texts();
                    t.iter().any(|x| x.contains(&first)) && t.iter().any(|x| x.contains(&second))
                }) {
                    Ok(())
                } else {
                    Err(format!(
                        "{:?}",
                        shown.iter().map(PostedMessage::texts).collect::<Vec<_>>()
                    ))
                }
            }
        },
    )
    .await;
    if let Some(with_link) = link {
        let m = double
            .shown_in(channel)
            .into_iter()
            .find(|m| m.texts().iter().any(|x| x.contains(&first)))
            .expect("the message");
        let has = m.texts().iter().any(|t| t.contains("Open in Foundry"));
        assert_eq!(
            has,
            with_link,
            "the message {} Open in Foundry; {:?}",
            if with_link { "carries" } else { "has no" },
            m.body
        );
    }
}

pub async fn then_exactly_one(
    world: &mut FoundryWorld,
    channel: &str,
    first: &str,
    second: Option<&str>,
) {
    then_message(world, channel, first, second.unwrap_or(first), None).await;
    tokio::time::sleep(Duration::from_millis(world.eip.spec.poll_ms * 5)).await;
    let shown = slack(world).shown_in(channel);
    let before = world
        .eip
        .before
        .as_ref()
        .and_then(|u| u.shown.get(channel).copied())
        .unwrap_or(0);
    let new = &shown[before.min(shown.len())..];
    assert_eq!(
        new.len(),
        1,
        "{channel} shows exactly one new message; it shows {:?}",
        new.iter().map(PostedMessage::texts).collect::<Vec<_>>()
    );
}

/// After a restart between attempts the waiting message is sent at its
/// scheduled retry: `RetryPolicy::FIXED_30S` (30 s after the failed attempt
/// settled), plus a margin for the restart, the poll and the per-destination
/// pacing. No delivery lease is in play (FOUNDRY_INTEGRATIONS_LEASE_SECONDS,
/// 45 s): the kill came after the attempt was recorded, which clears it.
const RETRY_AFTER_RESTART_BUDGET: Duration = Duration::from_secs(30 + 15);

/// How long a message may take to show: the NFR delivery budget, or the
/// scheduled retry after a restart between attempts.
fn showing_budget(world: &FoundryWorld) -> Duration {
    if world.eip.restarted_between_attempts {
        RETRY_AFTER_RESTART_BUDGET
    } else {
        DELIVERY_BUDGET
    }
}

pub async fn then_shows_key(world: &mut FoundryWorld, channel: &str, key: &str) {
    let double = slack(world);
    eventually(
        showing_budget(world),
        &format!("{channel} shows {key}'s message"),
        || {
            let double = double.clone();
            let (channel, key) = (channel.to_string(), key.to_string());
            async move {
                if double.shown_in(&channel).iter().any(|m| key_line(m, &key)) {
                    Ok(())
                } else {
                    Err("not yet".into())
                }
            }
        },
    )
    .await;
}

pub async fn then_shows_key_once(world: &mut FoundryWorld, channel: &str, key: &str) {
    then_shows_key(world, channel, key).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let n = slack(world)
        .shown_in(channel)
        .iter()
        .filter(|m| key_line(m, key))
        .count();
    assert_eq!(
        n, 1,
        "{channel} shows {key}'s message exactly once; it shows it {n} times"
    );
}

/// How long the arrival of messages held back by an outage is watched, once
/// Slack or the homeserver has been silent in the scenario: STATUS_BUDGET
/// (45 s) instead of the 10 s delivery budget. Healing Slack does not answer a
/// send already in flight, so the earliest arrival is after Foundry's 10 s call
/// timeout; a send that failed at once instead waits out DDR-5's second backoff
/// (20 s, up to 24 s with its jitter) from its start; then the next message to
/// the same channel waits its turn under per-destination pacing (NFR-EIP-3,
/// one a second). Each of those is a ratified behaviour, so no conforming
/// Foundry can show both messages within 10 s of Slack answering again.
fn arrival_window(world: &FoundryWorld) -> Duration {
    if world.eip.silence_seen {
        STATUS_BUDGET
    } else {
        DELIVERY_BUDGET
    }
}

/// "#auth-dev shows AUTH-12's New issue message before its Moved message".
pub async fn then_order(
    world: &mut FoundryWorld,
    channel: &str,
    first: (&str, &str),
    second: (&str, &str),
    each_once: bool,
) {
    let double = slack(world);
    let want = [
        (first.0.to_string(), first.1.to_string()),
        (second.0.to_string(), second.1.to_string()),
    ];
    eventually(
        arrival_window(world),
        &format!("{channel} shows both messages"),
        || {
            let double = double.clone();
            let want = want.clone();
            let channel = channel.to_string();
            async move {
                let shown = double.shown_in(&channel);
                let found = want.iter().all(|(k, w)| {
                    shown
                        .iter()
                        .any(|m| key_line(m, k) && m.texts().iter().any(|t| t.contains(w)))
                });
                if found {
                    Ok(())
                } else {
                    let texts: Vec<Vec<String>> = shown.iter().map(|m| m.texts()).collect();
                    Err(format!("not both yet; {channel} shows {texts:?}"))
                }
            }
        },
    )
    .await;
    let shown = slack(world).shown_in(channel);
    let pos = |k: &str, w: &str| {
        shown
            .iter()
            .position(|m| key_line(m, k) && m.texts().iter().any(|t| t.contains(w)))
            .expect("shown")
    };
    assert!(
        pos(first.0, first.1) < pos(second.0, second.1),
        "{channel} shows {first:?} before {second:?}"
    );
    if each_once {
        for (k, w) in [first, second] {
            let n = shown
                .iter()
                .filter(|m| key_line(m, k) && m.texts().iter().any(|t| t.contains(w)))
                .count();
            assert_eq!(
                n, 1,
                "{channel} shows {k}'s {w} message exactly once; {n} times"
            );
        }
    }
}

/// How long an absence is watched: the 10 s delivery budget, or
/// STATUS_BUDGET (45 s) once Slack or the homeserver has been silent in the
/// scenario, because a hung send must hit Foundry's client timeout before
/// anything could arrive.
fn absence_window(world: &FoundryWorld) -> Duration {
    if world.eip.silence_seen {
        STATUS_BUDGET
    } else {
        DELIVERY_BUDGET
    }
}

/// "nothing is posted to <channel> within 10 seconds": the NFR window, or
/// STATUS_BUDGET when Slack was silent earlier in the scenario (a send that
/// hung must time out before anything could arrive).
pub async fn then_nothing_posted(world: &mut FoundryWorld, channel: &str) {
    let before = world
        .eip
        .before
        .as_ref()
        .and_then(|u| u.shown.get(channel).copied())
        .unwrap_or(0);
    tokio::time::sleep(absence_window(world)).await;
    let shown = slack(world).shown_in(channel);
    assert!(
        shown.len() <= before,
        "nothing new is posted to {channel}; it now shows {:?}",
        shown[before.min(shown.len())..]
            .iter()
            .map(PostedMessage::texts)
            .collect::<Vec<_>>()
    );
}

pub fn then_no_message_for(world: &FoundryWorld, channel: &str) {
    assert!(
        slack(world).posts_to(channel).is_empty(),
        "Slack received no post for {channel}"
    );
}

pub fn then_test_count(world: &FoundryWorld, channel: &str, n: usize) {
    let tests = slack(world)
        .shown_in(channel)
        .iter()
        .filter(|m| {
            m.texts()
                .iter()
                .any(|t| t.contains("Test message from Foundry"))
        })
        .count();
    assert_eq!(tests, n, "{channel} holds exactly {n} test messages");
}

pub async fn then_shows_text(world: &mut FoundryWorld, channel: &str, wanted: &str) {
    let double = slack(world);
    let wanted = wanted.to_string();
    eventually(
        DELIVERY_BUDGET,
        &format!("{channel} shows {wanted:?}"),
        || {
            let double = double.clone();
            let (channel, wanted) = (channel.to_string(), wanted.clone());
            async move {
                if double
                    .shown_in(&channel)
                    .iter()
                    .any(|m| m.texts().iter().any(|t| t.contains(&wanted)))
                {
                    Ok(())
                } else {
                    Err("not yet".into())
                }
            }
        },
    )
    .await;
}

/// A new comment's message on `key`: Summary's "<actor> commented ·
/// <project>" line, or Link only's "<key> · <project> · New comment" (D-2's
/// event name; the older "Issue commented on" wording also reads as one).
fn comment_message(m: &PostedMessage, key: &str) -> bool {
    key_line(m, key)
        && m.texts().iter().any(|t| {
            t.contains("commented")
                || (t.starts_with(&format!("{key} · ")) && t.ends_with(" · New comment"))
        })
}

/// "#auth-dev shows AUTH-7's comment message [with|without] "<text>"".
pub async fn then_comment_message(
    world: &mut FoundryWorld,
    channel: &str,
    key: &str,
    text_shown: Option<(bool, &str)>,
) {
    let double = slack(world);
    eventually(
        DELIVERY_BUDGET,
        &format!("{channel} shows {key}'s comment message"),
        || {
            let double = double.clone();
            let (channel, key) = (channel.to_string(), key.to_string());
            async move {
                let shown = double.shown_in(&channel);
                if shown.iter().any(|m| comment_message(m, &key)) {
                    Ok(())
                } else {
                    Err(format!(
                        "{:?}",
                        shown.iter().map(PostedMessage::texts).collect::<Vec<_>>()
                    ))
                }
            }
        },
    )
    .await;
    if let Some((with, text)) = text_shown {
        let m = slack(world)
            .shown_in(channel)
            .into_iter()
            .rev()
            .find(|m| comment_message(m, key))
            .expect("the comment message");
        let has = m.all_text().contains(text);
        assert_eq!(
            has,
            with,
            "the comment message {} the comment text; {}",
            if with { "carries" } else { "does not carry" },
            m.body
        );
    }
}

/// The 300-character excerpt rows (D-3): the message's excerpt block is the
/// whole comment, or exactly its first 300 characters followed by "…", and
/// nothing past the cut appears anywhere in the message.
pub async fn then_excerpt(world: &mut FoundryWorld, channel: &str, length: usize, whole: bool) {
    let comment = world
        .eip
        .comment_text
        .clone()
        .expect("the When posted a comment");
    assert_eq!(comment.chars().count(), length);
    then_message(world, channel, "commented", "commented", None).await;
    let m = slack(world)
        .shown_in(channel)
        .into_iter()
        .last()
        .expect("the comment message");
    let texts = m.texts();
    let all = texts.join("\n");
    let excerpt = if whole {
        comment.clone()
    } else {
        let first: String = comment.chars().take(300).collect();
        format!("{first}…")
    };
    assert!(
        texts.contains(&excerpt),
        "the excerpt block reads {}; {all}",
        if whole {
            "the whole comment, uncut"
        } else {
            "the first 300 characters then \"…\""
        }
    );
    if !whole {
        // Nothing past the cut is sent: no text runs on beyond the 300th
        // character, and a tail long enough to be told apart (the numbered
        // words never repeat) appears nowhere.
        let beyond: String = comment.chars().take(301).collect();
        let rest: String = comment.chars().skip(300).collect();
        assert!(
            !all.contains(&beyond),
            "no text runs on past the 300th character; {all}"
        );
        assert!(
            rest.chars().count() < 8 || !all.contains(&rest),
            "nothing past the 300th character is sent; {all}"
        );
    }
}

/// A comment of exactly `n` characters whose words are numbered, so no stretch
/// of it repeats: what lies past the 300th character appears nowhere before it.
pub fn comment_of_length(n: usize) -> String {
    (1..)
        .map(|i| format!("note{i:03} "))
        .flat_map(|word| word.chars().collect::<Vec<_>>())
        .take(n)
        .collect()
}

/// The never-pings oracle plus readability: every word of the user's text
/// (four letters or more, the mention markers stripped) is still in what the
/// channel shows.
pub async fn then_no_ping(world: &mut FoundryWorld, channel: &str) {
    let input = world
        .eip
        .comment_text
        .clone()
        .or_else(|| world.eip.last_title.clone())
        .expect("the When's user text");
    let words: Vec<String> = input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(str::to_string)
        .collect();
    let double = slack(world);
    let w = words.clone();
    let c = channel.to_string();
    eventually(
        DELIVERY_BUDGET,
        &format!("{channel} shows the user's text"),
        || {
            let (double, w, c) = (double.clone(), w.clone(), c.clone());
            async move {
                let shown = double.shown_in(&c);
                if shown.iter().any(|m| {
                    let t = m.texts().join(" ");
                    w.iter().all(|x| t.contains(x.as_str()))
                }) {
                    Ok(())
                } else {
                    Err("not yet".into())
                }
            }
        },
    )
    .await;
    let m = double
        .shown_in(channel)
        .into_iter()
        .find(|m| {
            let t = m.texts().join(" ");
            words.iter().all(|x| t.contains(x.as_str()))
        })
        .expect("the message");
    assert!(
        !sd::notifies_anyone(&m),
        "the message notifies nobody (D-3, DDR-12); {}",
        m.body
    );
}

/// "no message in any channel contains any part of AUTH-13's description, …":
/// first a positive observable (the When's comment on the key reached every
/// routed channel), then the absence checks over everything Slack received.
pub async fn then_never_sent(world: &mut FoundryWorld, key: &str, attachment: &str) {
    for channel in ["#general", "#auth-dev", "#auth-releases"] {
        then_shows_key(world, channel, key).await;
    }
    let (title, description) = world.eip.description.clone().expect("the description");
    // A word the title also uses ("vault") is sent as the title, so only the
    // description's own fragments ("password", "cn=admin", "secret/ldap")
    // tell the description apart.
    let title_words: Vec<&str> = title.split(' ').collect();
    let fragments: Vec<&str> = description
        .split([' ', ','])
        .filter(|w| w.len() >= 5 && !title_words.contains(w))
        .collect();
    assert!(
        !fragments.is_empty(),
        "the description has words of its own"
    );
    for m in slack(world).all_posts() {
        let all = m.all_text();
        for f in &fragments {
            assert!(
                !all.contains(f),
                "no message carries {f:?} of the description; {all}"
            );
        }
        assert!(
            !all.contains(attachment),
            "no message names the attachment; {all}"
        );
        assert!(
            !all.contains("@canzan.dev"),
            "no message carries an email address; {all}"
        );
        let ids: Vec<&str> = all
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .collect();
        assert!(
            !ids.iter()
                .any(|w| w.len() == 36 && w.matches('-').count() == 4),
            "no message carries an internal id; {all}"
        );
        let other_keys = all
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .filter(|w| w.starts_with("AUTH-") && *w != key)
            .count();
        assert_eq!(
            other_keys, 0,
            "no message names an issue other than {key}; {all}"
        );
    }
}

pub async fn then_message_count(
    world: &mut FoundryWorld,
    channel: &str,
    n: usize,
    each: Option<&str>,
) {
    let double = slack(world);
    let before = world
        .eip
        .before
        .as_ref()
        .and_then(|u| u.shown.get(channel).copied())
        .unwrap_or(0);
    eventually(
        DELIVERY_BUDGET,
        &format!("{channel} shows {n} new messages"),
        || {
            let double = double.clone();
            let channel = channel.to_string();
            async move {
                let k = double.shown_in(&channel).len().saturating_sub(before);
                if k >= n {
                    Ok(())
                } else {
                    Err(format!("{k}"))
                }
            }
        },
    )
    .await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let shown = slack(world).shown_in(channel);
    let new = &shown[before.min(shown.len())..];
    assert_eq!(new.len(), n, "{channel} shows exactly {n} new messages");
    if let Some(each) = each {
        assert!(
            new.iter()
                .all(|m| m.texts().iter().any(|t| t.contains(each))),
            "each reads {each:?}"
        );
    }
}

/// "#auth-dev receives 6 messages, no two of them less than one second apart".
pub async fn then_paced(world: &mut FoundryWorld, channel: &str, n: usize) {
    then_message_count(world, channel, n, None).await;
    let posts = slack(world).posts_to(channel);
    for pair in posts.windows(2) {
        let gap = pair[1].at.duration_since(pair[0].at);
        assert!(
            gap >= Duration::from_millis(950),
            "no two posts to {channel} less than a second apart; gap {gap:?}"
        );
    }
}

/// "Slack was asked to post AUTH-12's message once | twice | more than once",
/// after the When moved the clock: a bounded wait on the count reaching the
/// expectation, then (for exact counts) the count is checked as final.
pub async fn then_attempts(world: &mut FoundryWorld, channel: &str, key: &str, how: &str) {
    let double = slack(world);
    let count = |d: &SlackDouble| {
        d.posts_to(channel)
            .iter()
            .filter(|m| key_line(m, key))
            .count()
    };
    let at_least = match how {
        "once" => 1,
        "twice" | "more than once" => 2,
        other => panic!("unknown attempt count {other:?}"),
    };
    let (c, k) = (channel.to_string(), key.to_string());
    eventually(
        STATUS_BUDGET,
        &format!("Slack is asked to post {key}'s message {how}"),
        || {
            let double = double.clone();
            let (c, k) = (c.clone(), k.clone());
            async move {
                let n = double
                    .posts_to(&c)
                    .iter()
                    .filter(|m| key_line(m, &k))
                    .count();
                if n >= at_least {
                    Ok(())
                } else {
                    Err(format!("{n}"))
                }
            }
        },
    )
    .await;
    match how {
        "once" => assert_eq!(count(&double), 1, "Slack was asked once and never again"),
        "twice" => assert_eq!(count(&double), 2, "Slack was asked exactly twice"),
        _ => {}
    }
}

/// "Slack is asked to post AUTH-12's message only once within 10 seconds":
/// once Slack has been asked, the 10 s window is watched and the count must
/// still be one. Slack is holding that post, so the replica that sent it still
/// holds its live claim (the 45 s lease); the other replica must not take the
/// delivery over and send it too. The window stays inside the claimant's own
/// settle-and-retry (Foundry's 10 s call timeout, then `RetryPolicy::FIXED_30S`),
/// so a correct Foundry asks Slack exactly once in it.
pub async fn then_asked_once_within(world: &mut FoundryWorld, channel: &str, key: &str) {
    slack_asked_to_post(
        world,
        channel,
        &format!("Slack is asked to post {key}'s message"),
    )
    .await;
    tokio::time::sleep(DELIVERY_BUDGET).await;
    let asked: Vec<Instant> = slack(world)
        .posts_to(channel)
        .iter()
        .filter(|m| key_line(m, key))
        .map(|m| m.at)
        .collect();
    assert_eq!(
        asked.len(),
        1,
        "Slack is asked to post {key}'s message only once while the replica sending it \
         still holds its claim; it was asked {} times, {:?} after the first",
        asked.len(),
        asked
            .iter()
            .map(|t| t.saturating_duration_since(*asked.first().unwrap_or(t)))
            .collect::<Vec<_>>()
    );
}

/// "Open in Foundry opens AUTH-12 for Sam and gives Marco the not-found page".
pub async fn then_link_opens(world: &mut FoundryWorld, channel: &str, key: &str) {
    let m = slack(world)
        .shown_in(channel)
        .into_iter()
        .find(|m| key_line(m, key))
        .expect("the message");
    let url = m
        .button_urls()
        .into_iter()
        .next()
        .expect("the message has an Open in Foundry button");
    let project = project_of_key(world, key);
    let path = format!(
        "{}/issues/{}",
        project_path(world, &project),
        key_number(key)
    );
    assert!(
        url.ends_with(&path) && url.starts_with("https://foundry.canzan.dev"),
        "Open in Foundry is the public address plus the issue's slugs; {url}"
    );
    let sam = get(world, "Sam", &path).await;
    assert_eq!(
        sam.status,
        StatusCode::OK,
        "Sam opens {key}; {}",
        tm::describe(&sam)
    );
    let marco = get(world, "Marco", &path).await;
    let unknown = never_existed_for(world, "Marco").await;
    assert_eq!(
        (marco.status, text(&marco)),
        (unknown.status, text(&unknown)),
        "Marco gets the not-found page"
    );
}

pub async fn never_existed_for(world: &mut FoundryWorld, who: &str) -> Answer {
    let path = format!("/never-existed-{}", uuid::Uuid::now_v7());
    get(world, who, &path).await
}

pub async fn then_not_found(world: &mut FoundryWorld, who: &str) {
    let answer = tm::last(world).clone();
    let unknown = never_existed_for(world, who).await;
    assert_eq!(
        (answer.status, text(&answer)),
        (unknown.status, text(&unknown)),
        "{who} gets the same not-found page any unknown address gets; {}",
        tm::describe(&answer)
    );
}

// ===========================================================================
// Pages Priya and Lena read
// ===========================================================================

pub async fn when_open(world: &mut FoundryWorld, who: &str, path: &str) {
    capture_before(world).await;
    let answer = get(world, who, path).await;
    world.eip.when_answer = Some(answer);
}

pub async fn when_open_door(world: &mut FoundryWorld, who: &str, door: WorkspaceDoor) {
    when_open(world, who, &door.path()).await;
}

pub async fn when_route_attempt(
    world: &mut FoundryWorld,
    who: &str,
    attempt: RouteAttempt,
    project: &str,
) {
    capture_before(world).await;
    let answer = match attempt {
        RouteAttempt::OpenPage => get(world, who, &integrations_path(world, project)).await,
        RouteAttempt::OpenNewSlackRoute => open_new_route(world, who, project).await,
        RouteAttempt::SubmitNewRoute(dest) => {
            let mut route = Route::new(project, Kind::Slack, &dest);
            route.events = EventChoice::ALL.to_vec();
            let path = format!("{}/routes", integrations_path(world, project));
            let fields = route_fields(world, &route);
            post(world, who, &path, &fields, true).await
        }
    };
    world.eip.when_answer = Some(answer);
}

/// The text a person reads now (the When's answer, or the page in front of them).
pub fn then_reads(world: &FoundryWorld, who: &str, wanted: &str) {
    let answer = world
        .eip
        .when_answer
        .clone()
        .unwrap_or_else(|| tm::last(world).clone());
    let seen = world.eip.names.as_scenario(&text(&answer));
    assert!(
        seen.contains(wanted),
        "{who} reads {wanted:?}; {}",
        tm::describe(&answer)
    );
}

pub async fn then_page_reads(world: &mut FoundryWorld, who: &str, path: &str, wanted: &[&str]) {
    let path = path.to_string();
    let mut seen = String::new();
    let start = Instant::now();
    while start.elapsed() < STATUS_BUDGET {
        let page = get(world, who, &path).await;
        seen = text(&page);
        if wanted.iter().all(|w| seen.contains(w)) {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("{path} reads {wanted:?}; it reads {seen:?}");
}

pub async fn then_back_on_integrations(world: &mut FoundryWorld, who: &str, wanted: &str) {
    let answer = world.eip.when_answer.clone().expect("the When's answer");
    assert_eq!(
        answer.path.split('?').next(),
        Some(WORKSPACE_INTEGRATIONS),
        "{who} is back on the workspace integrations page; {}",
        tm::describe(&answer)
    );
    then_reads(world, who, wanted);
}

pub async fn then_activity_reads(world: &mut FoundryWorld, line: &str) {
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    let lines = texts_of(&page.body, "[data-integration-activity] li");
    assert!(
        lines.iter().any(|l| l.contains(line)),
        "the integration activity reads {line:?}; it lists {lines:?}"
    );
}

pub async fn then_activity_gains(world: &mut FoundryWorld, n: usize, names: &[&str]) {
    let before = world.eip.before.clone().unwrap_or_default().activity;
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    let lines = texts_of(&page.body, "[data-integration-activity] li");
    let new: Vec<&String> = lines.iter().filter(|l| !before.contains(l)).collect();
    assert_eq!(
        new.len(),
        n,
        "the integration activity gains {n} line(s); new: {new:?}"
    );
    for line in new {
        for name in names {
            assert!(
                line.contains(name),
                "the new activity line names {name:?}; {line:?}"
            );
        }
    }
}

pub async fn then_add_to_slack(world: &mut FoundryWorld, offered: bool) {
    let page = match world.eip.when_answer.clone() {
        Some(a) if a.path == WORKSPACE_INTEGRATIONS => a,
        _ => get(world, "Priya", WORKSPACE_INTEGRATIONS).await,
    };
    assert_eq!(
        page.status,
        StatusCode::OK,
        "Priya opens the workspace integrations page; {}",
        tm::describe(&page)
    );
    let links = attr_of(&page.body, "a[href]", "href");
    let has =
        links.iter().any(|h| h.ends_with(SLACK_INSTALL)) && text(&page).contains("Add to Slack");
    assert_eq!(
        has,
        offered,
        "Priya is {}offered Add to Slack; {}",
        if offered { "" } else { "not " },
        tm::describe(&page)
    );
}

pub async fn then_priya_still_finds(world: &mut FoundryWorld, who: &str, path: &str) {
    let page = get(world, who, path).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "control: {who} still finds {path}; {}",
        tm::describe(&page)
    );
}

pub async fn then_manifest(world: &mut FoundryWorld) {
    let answer = world.eip.when_answer.clone().expect("the setup page");
    let body = &answer.body;
    for needle in [
        "https://foundry.canzan.dev/workspace/integrations/slack/return",
        "https://foundry.canzan.dev/integrations/slack/events",
    ] {
        assert!(
            body.contains(needle),
            "the manifest names {needle}; {}",
            tm::describe(&answer)
        );
    }
}

pub fn then_manifest_scopes(world: &FoundryWorld, scopes: &[&str]) {
    let answer = world.eip.when_answer.clone().expect("the setup page");
    for s in scopes {
        assert!(answer.body.contains(s), "the manifest asks for {s}");
    }
    for extra in [
        "channels:history",
        "users:read",
        "chat:write.customize",
        "user_scope",
    ] {
        assert!(
            !answer.body.contains(extra),
            "the manifest asks for no {extra}"
        );
    }
}

/// "no Slack credential appears on it, in its markup or in its headers".
pub fn then_no_credential(world: &FoundryWorld) {
    let answer = world
        .eip
        .when_answer
        .clone()
        .unwrap_or_else(|| tm::last(world).clone());
    let mut sentinels = vec![
        sd::CLIENT_SECRET.to_string(),
        sd::SIGNING_SECRET.to_string(),
        SECRETS_KEY.to_string(),
    ];
    if let Some(double) = &world.eip.slack {
        sentinels.extend(double.issued_tokens());
    }
    for s in sentinels {
        assert!(
            !answer.body.contains(&s),
            "{} carries no credential",
            answer.path
        );
        assert!(
            !answer.location.clone().unwrap_or_default().contains(&s),
            "{}'s redirect carries no credential",
            answer.path
        );
    }
}

/// The pages the credential outline opens.
pub fn page_path(world: &FoundryWorld, page: &str) -> String {
    match page.trim() {
        "the workspace integrations page" => WORKSPACE_INTEGRATIONS.into(),
        "the Slack setup page" => SLACK_SETUP.into(),
        "Identity Platform's integrations page" => integrations_path(world, "Identity Platform"),
        "a new Slack route on Identity Platform" => format!(
            "{}/routes/new?kind=slack",
            integrations_path(world, "Identity Platform")
        ),
        other => panic!("unknown page {other:?}"),
    }
}

pub fn then_picker(world: &FoundryWorld, channels: &[&str], offered: bool) {
    let answer = world.eip.when_answer.clone().expect("the new route form");
    let options = texts_of(&answer.body, "select[name=destination] option");
    for c in channels {
        assert_eq!(
            options.iter().any(|o| o.contains(c)),
            offered,
            "the channel picker {} {c}; {options:?}",
            if offered { "offers" } else { "does not offer" }
        );
    }
}

pub fn then_form_offers_events(world: &FoundryWorld) {
    let answer = world.eip.when_answer.clone().expect("the route form");
    let values = attr_of(&answer.body, "input[name=events]", "value");
    let want: Vec<String> = EventChoice::ALL
        .iter()
        .map(|e| e.wire().to_string())
        .collect();
    assert_eq!(
        values, want,
        "the route form offers exactly the four v1 chat events"
    );
}

pub fn then_form_offers_lanes(world: &FoundryWorld, lanes: &str) {
    let answer = world.eip.when_answer.clone().expect("the route form");
    let offered = texts_of(&answer.body, "[data-lane-filter] label");
    for l in lanes.replace(" and ", ", ").split(", ") {
        assert!(
            offered.iter().any(|o| o.contains(l.trim())),
            "Issue moved can be limited to {l}; {offered:?}"
        );
    }
}

pub fn then_levels(world: &FoundryWorld) {
    let answer = world.eip.when_answer.clone().expect("the route form");
    let values = attr_of(&answer.body, "input[name=content_level]", "value");
    assert_eq!(
        values,
        ["link_only", "summary", "summary_with_comment"],
        "three content levels"
    );
    let chosen = attr_of(&answer.body, "input[name=content_level][checked]", "value");
    assert_eq!(chosen, ["summary"], "Summary is chosen (OD-6)");
}

pub fn then_preview(world: &FoundryWorld, lines: &[&str], link: bool, absent: &[&str]) {
    let answer = world.eip.when_answer.clone().expect("the preview");
    let preview = texts_of(&answer.body, "[data-route-preview]").join(" ");
    for l in lines {
        assert!(preview.contains(l), "the preview reads {l:?}; {preview:?}");
    }
    assert_eq!(
        preview.contains("Open in Foundry"),
        link,
        "the preview's Open in Foundry"
    );
    for a in absent {
        assert!(
            !preview.contains(a),
            "the preview shows no {a:?}; {preview:?}"
        );
    }
}

pub fn then_preview_asked_nothing(world: &FoundryWorld) {
    let before = world.eip.shown_before.get("calls").copied().unwrap_or(0);
    let calls = slack(world).calls();
    assert_eq!(
        &calls[before.min(calls.len())..],
        &[] as &[String],
        "the preview asked Slack nothing (DDR-25)"
    );
}

pub fn then_history_line(world: &FoundryWorld, line: &str) {
    let answer = world
        .eip
        .when_answer
        .clone()
        .unwrap_or_else(|| tm::last(world).clone());
    let entries = texts_of(&answer.body, "[data-delivery]");
    assert!(
        entries.iter().any(|e| e.contains(line)),
        "the history shows {line:?}; {entries:?}"
    );
}

pub async fn then_history_reads(world: &mut FoundryWorld, destination: &str, line: &str) {
    let mut entries = Vec::new();
    let start = Instant::now();
    while start.elapsed() < STATUS_BUDGET {
        let page = open_history(world, "Lena", destination).await;
        entries = texts_of(&page.body, "[data-delivery]");
        if entries.iter().any(|e| e.contains(line)) {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("the history of the route to {destination} reads {line:?}; {entries:?}");
}

pub fn then_history_hides(world: &FoundryWorld, words: &[&str]) {
    let answer = world.eip.when_answer.clone().expect("the history");
    for w in words {
        assert!(!answer.body.contains(w), "the history shows none of {w:?}");
    }
}

pub async fn then_board_quick(world: &mut FoundryWorld, key: &str, budget: Duration) {
    let started = world.eip.when_started.expect("the When was timed");
    let took = started.elapsed();
    assert!(
        took <= budget,
        "filing answered in its usual time; took {took:?}"
    );
    let project = project_of_key(world, key);
    let board = get(world, "Sam", &project_path(world, &project)).await;
    assert!(
        text(&board).contains(key),
        "{key} is on the board; {}",
        tm::describe(&board)
    );
}

// ===========================================================================
// The universe (Mandate 8)
// ===========================================================================

/// Read the universe through Priya's pages, the outside doubles and the email
/// recorder. Fails (panics) if a page it must read is not readable: an
/// unreadable universe would make every delta check vacuous.
pub async fn universe(world: &mut FoundryWorld) -> Universe {
    universe_with_gone(world, &[]).await
}

/// The universe while the named projects are deleted: each of their pages
/// must answer "not found" (and is recorded as gone); every other page must
/// still be readable.
pub async fn universe_with_gone(world: &mut FoundryWorld, gone: &[&str]) -> Universe {
    let workspace = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    let mut projects = Vec::new();
    for project in ["Identity Platform", "Homelab Ops"] {
        let page = get(world, "Priya", &integrations_path(world, project)).await;
        projects.push((project.to_string(), page));
    }
    let mut u =
        universe_from_pages(&workspace, &projects, gone).unwrap_or_else(|why| panic!("{why}"));
    if let Some(double) = &world.eip.slack {
        for c in [
            "#auth-dev",
            "#auth-releases",
            "#general",
            "#ops-alerts",
            "#ext-auth-vendor",
            "#backend-private",
        ] {
            u.shown.insert(c.to_string(), double.shown_in(c).len());
        }
        u.slack_uninstalls = double.uninstall_count();
    }
    for (_, email) in PEOPLE {
        u.emails
            .insert(email.to_string(), received_emails(world, email).len());
    }
    u
}

/// The page part of the universe, pure: every page must have answered 200,
/// except the pages of projects named `gone`, which must answer 404.
pub fn universe_from_pages(
    workspace: &Answer,
    projects: &[(String, Answer)],
    gone: &[&str],
) -> Result<Universe, String> {
    let readable = |a: &Answer| {
        if a.status == StatusCode::OK {
            Ok(())
        } else {
            Err(format!(
                "the universe cannot be read: {} answered {}",
                a.path, a.status
            ))
        }
    };
    readable(workspace)?;
    let seam = |a: &Answer, sel: &str| {
        if select_all(&parse(&a.body), sel).is_empty() {
            Err(format!(
                "the universe cannot be read: {} has no {sel} seam",
                a.path
            ))
        } else {
            Ok(())
        }
    };
    seam(workspace, "[data-connection]")?;
    seam(workspace, "[data-integration-activity]")?;
    let mut u = Universe {
        slack_connection: texts_of(&workspace.body, "[data-connection=\"slack\"]").join(" "),
        matrix_connection: texts_of(&workspace.body, "[data-connection=\"matrix\"]").join(" "),
        activity: texts_of(&workspace.body, "[data-integration-activity] li"),
        ..Universe::default()
    };
    for (project, page) in projects {
        if gone.contains(&project.as_str()) {
            if page.status != StatusCode::NOT_FOUND {
                return Err(format!(
                    "the universe cannot be read: {project} should be deleted, yet {} answered {}",
                    page.path, page.status
                ));
            }
            u.gone.insert(project.clone());
            continue;
        }
        readable(page)?;
        seam(page, "[data-routes]")?;
        let doc = parse(&page.body);
        for e in select_all(&doc, "[data-route-destination]") {
            let dest = e
                .value()
                .attr("data-route-destination")
                .unwrap_or_default()
                .to_string();
            let status = e
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            u.routes.insert((project.clone(), dest), status);
        }
    }
    Ok(u)
}

pub async fn capture_before(world: &mut FoundryWorld) {
    let u = universe(world).await;
    world.eip.before = Some(u);
}

/// Compare the after-image with the before-image plus the declared changes,
/// fail-closed (anything that moved and was not declared fails).
pub async fn assert_state_delta(world: &mut FoundryWorld, changes: &[Change]) {
    let before = world
        .eip
        .before
        .clone()
        .expect("the When captured the universe first");
    let after = universe(world).await;
    if let Some(why) = delta_violation(&before, &after, changes) {
        panic!("{why}");
    }
}

/// The pure state-delta oracle: `None` when `after` equals `before` plus
/// exactly the declared changes AND every declared change actually happened.
pub fn delta_violation(before: &Universe, after: &Universe, changes: &[Change]) -> Option<String> {
    let mut expected = before.clone();
    for c in changes {
        match c {
            Change::SlackConnection => {
                if after.slack_connection == before.slack_connection {
                    return Some("the Slack connection was declared to change and did not".into());
                }
                expected.slack_connection = after.slack_connection.clone();
            }
            Change::MatrixConnection => {
                if after.matrix_connection == before.matrix_connection {
                    return Some("the Matrix connection was declared to change and did not".into());
                }
                expected.matrix_connection = after.matrix_connection.clone();
            }
            Change::ActivityLines(n) => {
                if after.activity.len() != before.activity.len() + n {
                    return Some(format!(
                        "the activity gains {n} line(s); it went from {} to {}",
                        before.activity.len(),
                        after.activity.len()
                    ));
                }
                expected.activity = after.activity.clone();
            }
            Change::RouteAppeared {
                project,
                destination,
            } => {
                let k = (project.clone(), destination.clone());
                match (before.routes.get(&k), after.routes.get(&k)) {
                    (None, Some(v)) => {
                        expected.routes.insert(k, v.clone());
                    }
                    _ => {
                        return Some(format!(
                            "the route {k:?} was declared to appear and did not"
                        ))
                    }
                }
            }
            Change::RouteStatus {
                project,
                destination,
            } => {
                let k = (project.clone(), destination.clone());
                match (before.routes.get(&k), after.routes.get(&k)) {
                    (Some(b), Some(a)) if a != b => {
                        expected.routes.insert(k, a.clone());
                    }
                    _ => {
                        return Some(format!(
                            "the route {k:?} was declared to change status and did not"
                        ))
                    }
                }
            }
            Change::RouteGone {
                project,
                destination,
            } => {
                let k = (project.clone(), destination.clone());
                if !before.routes.contains_key(&k) || after.routes.contains_key(&k) {
                    return Some(format!("the route {k:?} was declared gone and is not"));
                }
                expected.routes.remove(&k);
            }
            Change::Shown { channel } => {
                let (b, a) = (
                    before.shown.get(channel).copied().unwrap_or(0),
                    after.shown.get(channel).copied().unwrap_or(0),
                );
                if a <= b {
                    return Some(format!("{channel} was declared to show more and did not"));
                }
                expected.shown.insert(channel.clone(), a);
            }
            Change::SlackUninstall => {
                if after.slack_uninstalls <= before.slack_uninstalls {
                    return Some(
                        "Slack was declared to be asked to remove Foundry's access and was not"
                            .into(),
                    );
                }
                expected.slack_uninstalls = after.slack_uninstalls;
            }
            Change::Emailed { to } => {
                let (b, a) = (
                    before.emails.get(to).copied().unwrap_or(0),
                    after.emails.get(to).copied().unwrap_or(0),
                );
                if a <= b {
                    return Some(format!("{to} was declared to be emailed and was not"));
                }
                expected.emails.insert(to.clone(), a);
            }
        }
    }
    (after != &expected).then(|| {
        format!(
            "nothing changed but the declared {changes:?}; expected {expected:?}, got {after:?}"
        )
    })
}

pub async fn then_nothing_changed(world: &mut FoundryWorld) {
    assert_state_delta(world, &[]).await;
}

pub async fn then_only_connection_changed(world: &mut FoundryWorld) {
    assert_state_delta(world, &[Change::SlackConnection, Change::ActivityLines(1)]).await;
}

pub async fn then_connected_and_delivering(world: &mut FoundryWorld, kind: Kind) {
    let u = universe(world).await;
    let (line, connected) = match kind {
        Kind::Slack => (&u.slack_connection, "Connected to Canzan Labs"),
        Kind::Matrix => (&u.matrix_connection, "Connected as "),
    };
    assert!(
        line.contains(connected),
        "Canzan Labs' {kind:?} stays connected; its line reads {line:?}"
    );
    for ((p, d), s) in &u.routes {
        assert!(
            s.contains("Delivering"),
            "{p}'s route to {d} reads Delivering; {s:?}"
        );
    }
}

// ===========================================================================
// Emails
// ===========================================================================

/// Every recorder that can deliver mail in this scenario: the in-process
/// app's and, when Foundry runs as replicas, theirs. A recorder shared by
/// both is listed once, so no email is counted twice.
fn email_recorders(world: &FoundryWorld) -> Vec<Arc<DeliveryRecorder>> {
    let mut recorders: Vec<Arc<DeliveryRecorder>> = Vec::new();
    let candidates = world
        .harness
        .as_ref()
        .map(|h| &h.fake_email)
        .into_iter()
        .chain(world.eip.replicas.as_ref().map(|r| &r.fake_email));
    for recorder in candidates {
        if !recorders.iter().any(|known| Arc::ptr_eq(known, recorder)) {
            recorders.push(Arc::clone(recorder));
        }
    }
    recorders
}

/// What a person has received: every recorder that can deliver mail in the
/// scenario (account mail through the Notifier; DELIVER routes the
/// EmailTransport recorder into the same surface, with headers), or the
/// mailpit relay for `@needs-smtp`.
pub fn received_emails(world: &FoundryWorld, email: &str) -> Vec<ReceivedEmail> {
    email_recorders(world)
        .iter()
        .flat_map(|r| r.sent())
        .filter(|d| d.to.eq_ignore_ascii_case(email) && d.outcome == "delivered")
        .map(|d| ReceivedEmail {
            to: d.to,
            subject: d.subject,
            body: d.body,
            headers: BTreeMap::new(),
        })
        .collect()
}

pub async fn then_email_subject(
    world: &mut FoundryWorld,
    who: &str,
    subject: &str,
    budget: Duration,
) {
    let email = tm::resolve_person(world, who).email;
    let subject = subject.to_string();
    let start = Instant::now();
    while start.elapsed() < budget {
        if let Some(e) = received_emails(world, &email)
            .into_iter()
            .find(|e| e.subject.contains(&subject))
        {
            world.eip.received = Some(e);
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!(
        "{who} receives an email {subject:?} within {budget:?}; received {:?}",
        received_emails(world, &email)
            .iter()
            .map(|e| e.subject.clone())
            .collect::<Vec<_>>()
    );
}

pub async fn then_email_about(
    world: &mut FoundryWorld,
    who: &str,
    key: &str,
    footer: Option<&str>,
) {
    let email = tm::resolve_person(world, who).email;
    let start = Instant::now();
    while start.elapsed() < EMAIL_BUDGET {
        if let Some(e) = received_emails(world, &email)
            .into_iter()
            .find(|e| e.subject.contains(key) || e.body.contains(key))
        {
            if let Some(f) = footer {
                assert!(
                    tm::page_text(&e.body).contains(f) || e.body.contains(f),
                    "the email's footer reads {f:?}; {}",
                    e.body
                );
            }
            world.eip.received = Some(e);
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("{who} receives an email about {key} within {EMAIL_BUDGET:?}");
}

pub async fn then_no_email_about(world: &mut FoundryWorld, who: &str, key: &str) {
    tokio::time::sleep(DELIVERY_BUDGET).await;
    let email = tm::resolve_person(world, who).email;
    let got: Vec<String> = received_emails(world, &email)
        .into_iter()
        .filter(|e| e.subject.contains(key) || e.body.contains(key))
        .map(|e| e.subject)
        .collect();
    assert!(
        got.is_empty(),
        "{who} receives no email about {key}; got {got:?}"
    );
}

pub async fn then_email_count(
    world: &mut FoundryWorld,
    who: &str,
    subject: &str,
    n: usize,
    budget: Duration,
) {
    let email = tm::resolve_person(world, who).email;
    tokio::time::sleep(budget.min(DELIVERY_BUDGET)).await;
    let got = received_emails(world, &email)
        .into_iter()
        .filter(|e| e.subject.contains(subject))
        .count();
    assert_eq!(
        got, n,
        "{who} has received exactly {n} email(s) {subject:?}"
    );
}

pub async fn then_no_connection_email(world: &mut FoundryWorld, who: &str, kind: Kind) {
    let subject = match kind {
        Kind::Slack => "lost its Slack connection",
        Kind::Matrix => "lost its Matrix connection",
    };
    then_email_count(world, who, subject, 0, DELIVERY_BUDGET).await;
}

pub fn then_received_footer(world: &FoundryWorld, footer: &str) {
    let e = world.eip.received.as_ref().expect("an email was received");
    assert!(
        tm::page_text(&e.body).contains(footer) || e.body.contains(footer),
        "its footer reads {footer:?}; {}",
        e.body
    );
}

pub fn then_received_carries(world: &FoundryWorld, comment: &str) {
    let e = world.eip.received.as_ref().expect("an email was received");
    assert!(
        e.body.contains(comment) && e.body.contains("Open in Foundry"),
        "it carries the comment and Open in Foundry; {}",
        e.body
    );
}

/// Choose project emails through the settings form.
pub async fn choose_emails(
    world: &mut FoundryWorld,
    who: &str,
    events: &[EventChoice],
    scope: Scope,
    project: &str,
) -> Answer {
    let page = get(world, who, ACCOUNT_SETTINGS).await;
    let project_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM projects WHERE name = $1")
        .bind(project)
        .fetch_one(&tm::pool(world))
        .await
        .expect("the project's id");
    let mut fields = form_fields(&page, PROJECT_EMAILS);
    fields.retain(|(k, _)| k != "choice");
    for e in events {
        fields.push((
            "choice".into(),
            format!("{}:{}:{}", project_id.0, e.wire(), scope.wire()),
        ));
    }
    let answer = post(world, who, PROJECT_EMAILS, &fields, true).await;
    follow(world, who, answer).await
}

pub async fn given_chosen(
    world: &mut FoundryWorld,
    who: &str,
    events: &str,
    scope: &str,
    project: &str,
) {
    let answer = choose_emails(
        world,
        who,
        &EventChoice::parse_list(events),
        Scope::parse(scope),
        project,
    )
    .await;
    went_through(&answer, &format!("{who}'s project email choice"));
    assert!(
        text(&answer).contains("Saved."),
        "precondition: {who} reads the saved confirmation; {}",
        tm::describe(&answer)
    );
}

/// "Sam ticks … and saves" on the settings page he followed from the board.
pub async fn when_tick_and_save(
    world: &mut FoundryWorld,
    who: &str,
    event: &str,
    scope: &str,
    project: &str,
) {
    let page = world
        .eip
        .in_front
        .clone()
        .expect("a Given followed Settings from the board");
    assert_eq!(
        page.status,
        StatusCode::OK,
        "precondition: the settings page Sam followed opens; {}",
        tm::describe(&page)
    );
    capture_before(world).await;
    let answer = choose_emails(
        world,
        who,
        &[EventChoice::parse(event)],
        Scope::parse(scope),
        project,
    )
    .await;
    world.eip.when_answer = Some(answer);
}

/// "Sam has no project email choices and has followed Settings in the sidebar
/// of the Identity Platform board": the board's own link, followed.
pub async fn given_followed_settings(world: &mut FoundryWorld, who: &str, project: &str) {
    then_no_choices(world, who).await;
    let path = project_path(world, project);
    let board = get(world, who, &path).await;
    assert_eq!(
        board.status,
        StatusCode::OK,
        "precondition: {who} is on the {project} board"
    );
    let settings = tm::link_to(&board, "Settings");
    let page = get(world, who, &settings).await;
    world.eip.in_front = Some(page);
}

pub async fn then_settings_show(
    world: &mut FoundryWorld,
    who: &str,
    chosen: &[EventChoice],
    not_chosen: &[EventChoice],
    project: &str,
) {
    let page = get(world, who, ACCOUNT_SETTINGS).await;
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM projects WHERE name = $1")
        .bind(project)
        .fetch_one(&tm::pool(world))
        .await
        .expect("project id");
    let checked = attr_of(
        &page.body,
        "[data-project-emails] input[name=choice][checked]",
        "value",
    );
    for e in chosen {
        assert!(
            checked
                .iter()
                .any(|c| c.starts_with(&format!("{id}:{}:", e.wire()))),
            "{} is chosen; {checked:?}",
            e.label()
        );
    }
    for e in not_chosen {
        assert!(
            !checked
                .iter()
                .any(|c| c.starts_with(&format!("{id}:{}:", e.wire()))),
            "{} is not chosen; {checked:?}",
            e.label()
        );
    }
}

pub async fn then_settings_offer(world: &mut FoundryWorld, who: &str, offered: &str, hidden: &str) {
    let page = world.eip.when_answer.clone().expect("the settings page");
    let section = texts_of(&page.body, "[data-project-emails]").join(" ");
    assert!(
        section.contains(offered) && !section.contains(hidden),
        "{who} is offered {offered} and not {hidden}; {section:?}"
    );
}

pub fn then_nothing_ticked(world: &FoundryWorld) {
    let page = world.eip.when_answer.clone().expect("the settings page");
    let checked = attr_of(
        &page.body,
        "[data-project-emails] input[name=choice][checked]",
        "value",
    );
    assert!(
        checked.is_empty(),
        "nothing is chosen by default (OD-10); {checked:?}"
    );
}

pub fn then_offers_every_choice(world: &FoundryWorld) {
    let page = world.eip.when_answer.clone().expect("the settings page");
    let values = attr_of(
        &page.body,
        "[data-project-emails] input[name=choice]",
        "value",
    );
    for e in EventChoice::ALL {
        for s in [Scope::All, Scope::Mine] {
            assert!(
                values
                    .iter()
                    .any(|v| v.ends_with(&format!(":{}:{}", e.wire(), s.wire()))),
                "{} for {} is offered",
                e.label(),
                s.wire()
            );
        }
    }
}

pub fn then_no_save(world: &FoundryWorld) {
    let page = world.eip.when_answer.clone().expect("the settings page");
    assert!(
        tm::forms(&page.body)
            .iter()
            .all(|f| f.action != PROJECT_EMAILS),
        "no project email form is offered"
    );
}

pub async fn then_no_choices(world: &mut FoundryWorld, who: &str) {
    let page = get(world, who, ACCOUNT_SETTINGS).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "control: {who} opens their settings"
    );
    let checked = attr_of(
        &page.body,
        "[data-project-emails] input[name=choice][checked]",
        "value",
    );
    assert!(
        checked.is_empty(),
        "{who} has no project email choices; {checked:?}"
    );
}

pub async fn when_choose_foreign(world: &mut FoundryWorld, who: &str, event: &str, project: &str) {
    capture_before(world).await;
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM projects WHERE name = $1")
        .bind(project)
        .fetch_one(&tm::pool(world))
        .await
        .expect("project id");
    let fields = vec![(
        "choice".to_string(),
        format!("{id}:{}:all", EventChoice::parse(event).wire()),
    )];
    let answer = post(world, who, PROJECT_EMAILS, &fields, true).await;
    world.eip.when_answer = Some(answer);
}

pub async fn mute(world: &mut FoundryWorld, who: &str) {
    let ws = tm::workspace_id(world, tm::HOME).to_string();
    let answer = post(
        world,
        who,
        "/account/settings/mute",
        &[("workspace_id".into(), ws)],
        true,
    )
    .await;
    went_through(&answer, &format!("{who} muting Canzan Labs"));
}

/// "Sam has received a New comment email from Identity Platform" (Lena comments).
pub async fn given_received_comment_email(world: &mut FoundryWorld, who: &str) {
    comment(world, "Lena", "AUTH-7", "A comment Sam is emailed about.").await;
    then_email_about(world, who, "AUTH-7", None).await;
}

fn stop_link(world: &FoundryWorld) -> String {
    let e = world.eip.received.as_ref().expect("an email was received");
    let i = e
        .body
        .find(EMAIL_STOP)
        .unwrap_or_else(|| panic!("the email carries a Stop these emails link; {}", e.body));
    let rest = &e.body[i..];
    rest.split(|c: char| c.is_whitespace() || c == '"' || c == '<' || c == '>')
        .next()
        .unwrap_or(rest)
        .replace("&amp;", "&")
}

pub async fn given_followed_stop(world: &mut FoundryWorld, who: &str) {
    let link = stop_link(world);
    let page = get(world, "a signed-out visitor", &link).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "precondition: {who}'s stop link opens its confirm page; {}",
        tm::describe(&page)
    );
    world.eip.in_front = Some(page);
}

pub async fn when_confirm_stop(world: &mut FoundryWorld) {
    let page = world
        .eip
        .in_front
        .clone()
        .expect("a Given followed the stop link");
    capture_before(world).await;
    let form = tm::forms(&page.body)
        .into_iter()
        .find(|f| f.action.starts_with(EMAIL_STOP))
        .expect("the stop confirm form");
    let answer = post(
        world,
        "a signed-out visitor",
        &form.action,
        &form.fields,
        true,
    )
    .await;
    let answer = follow(world, "a signed-out visitor", answer).await;
    world.eip.when_answer = Some(answer);
}

pub async fn given_stopped(world: &mut FoundryWorld) {
    given_followed_stop(world, "Sam").await;
    when_confirm_stop(world).await;
    let answer = world.eip.when_answer.clone().expect("the stop answer");
    assert!(
        text(&answer).contains("Stopped:"),
        "precondition: the stop went through; {}",
        tm::describe(&answer)
    );
}

pub async fn when_scanner_opens(world: &mut FoundryWorld) {
    capture_before(world).await;
    let link = stop_link(world);
    let _ = get(world, "a signed-out visitor", &link).await;
}

pub async fn when_one_click(world: &mut FoundryWorld) {
    capture_before(world).await;
    let e = world.eip.received.clone().expect("an email was received");
    let header = e
        .headers
        .get("list-unsubscribe")
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "the email carries List-Unsubscribe (RFC 8058); headers {:?}",
                e.headers
            )
        });
    let url = header.trim_matches(|c| c == '<' || c == '>').to_string();
    let resp = http(world)
        .post(&url)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body("List-Unsubscribe=One-Click")
        .send()
        .await
        .expect("the mail client's one-click POST");
    let answer = tm::into_answer(&url, resp).await;
    world.eip.when_answer = Some(answer);
}

/// The tampered-link outline.
pub async fn when_tampered(world: &mut FoundryWorld, uses: &str, tampered: &str) {
    capture_before(world).await;
    let link = stop_link(world);
    let forged = match tampered.trim() {
        "with its signature altered" => {
            let mut s = link.clone();
            let last = s.pop().unwrap_or('A');
            s.push(if last == 'A' { 'B' } else { 'A' });
            s
        }
        "naming New issue instead of New comment" => link.replace("comment.added", "issue.created"),
        "naming Homelab Ops instead of Identity Platform" => {
            let (a,): (uuid::Uuid,) =
                sqlx::query_as("SELECT id FROM projects WHERE name = 'Identity Platform'")
                    .fetch_one(&tm::pool(world))
                    .await
                    .expect("id");
            let (b,): (uuid::Uuid,) =
                sqlx::query_as("SELECT id FROM projects WHERE name = 'Homelab Ops'")
                    .fetch_one(&tm::pool(world))
                    .await
                    .expect("id");
            link.replace(&a.to_string(), &b.to_string())
        }
        other => panic!("unknown tampering {other:?}"),
    };
    let answer = match uses.trim() {
        "opens" => get(world, "a signed-out visitor", &forged).await,
        "confirms" => {
            let query = forged
                .split_once('?')
                .map(|(_, q)| q.to_string())
                .unwrap_or_default();
            let fields: Vec<(String, String)> = query
                .split('&')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            post(world, "a signed-out visitor", EMAIL_STOP, &fields, true).await
        }
        "sends as a one-click to" => {
            let url = format!(
                "{}{}",
                base(world),
                forged.replace(EMAIL_STOP, EMAIL_STOP_ONE_CLICK)
            );
            let resp = http(world)
                .post(&url)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body("List-Unsubscribe=One-Click")
                .send()
                .await
                .expect("one-click");
            tm::into_answer(&url, resp).await
        }
        other => panic!("unknown use {other:?}"),
    };
    world.eip.when_answer = Some(answer);
}

/// The shipped uniform refusal for an invalid link (`/unsubscribe` with a bad token).
pub async fn then_invalid_link_refusal(world: &mut FoundryWorld) {
    let answer = world.eip.when_answer.clone().expect("the When's answer");
    let reference = get(
        world,
        "a signed-out visitor",
        &format!("{EMAIL_STOP}?t=eip-not-a-signed-link"),
    )
    .await;
    assert_eq!(
        (answer.status, text(&answer)),
        (reference.status, text(&reference)),
        "the same refusal page any invalid stop link gets; {}",
        tm::describe(&answer)
    );
}

pub async fn when_password_reset(world: &mut FoundryWorld, who: &str) {
    capture_before(world).await;
    let email = tm::resolve_person(world, who).email;
    let answer = post(
        world,
        "a signed-out visitor",
        "/forgot-password",
        &[("email".into(), email)],
        true,
    )
    .await;
    world.eip.when_answer = Some(answer);
}

pub async fn then_reset_email(world: &mut FoundryWorld, who: &str) {
    let email = tm::resolve_person(world, who).email;
    let start = Instant::now();
    while start.elapsed() < DELIVERY_BUDGET {
        if email_recorders(world)
            .iter()
            .any(|r| r.recorded("log", "password_reset", "delivered") > 0)
            || received_emails(world, &email)
                .iter()
                .any(|e| e.subject.to_ascii_lowercase().contains("reset"))
        {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("{who} receives the password-reset email");
}

/// `@needs-smtp`: the pinned mailpit relay (DEVOPS-4). Started per scenario.
pub async fn given_smtp_relay(world: &mut FoundryWorld) {
    use testcontainers::core::IntoContainerPort;
    use testcontainers::runners::AsyncRunner;
    use testcontainers::{GenericImage, ImageExt};
    // The shipped SmtpProvider REQUIRES STARTTLS (`starttls_relay`, notify.rs):
    // mailpit offers it with the committed test certificate (verification is
    // off through SMTP_TLS_VERIFY=false, the encryption is not).
    let tls = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tls");
    let container = GenericImage::new(mx::MAILPIT_IMAGE, mx::MAILPIT_TAG)
        .with_exposed_port(1025.tcp())
        .with_exposed_port(8025.tcp())
        .with_mount(testcontainers::core::Mount::bind_mount(tls, "/tls"))
        .with_env_var("MP_SMTP_TLS_CERT", "/tls/mailpit-test.crt")
        .with_env_var("MP_SMTP_TLS_KEY", "/tls/mailpit-test.private-key")
        .with_env_var("MP_SMTP_REQUIRE_STARTTLS", "true")
        .with_label("foundry-acceptance", "eip")
        .start()
        .await
        .expect("start the pinned mailpit (@needs-smtp needs Docker)");
    let smtp = container
        .get_host_port_ipv4(1025.tcp())
        .await
        .expect("smtp port");
    let api = container
        .get_host_port_ipv4(8025.tcp())
        .await
        .expect("api port");
    world.eip.spec.email = EmailSetup::SmtpRelay {
        host: "127.0.0.1".into(),
        smtp_port: smtp,
        http_port: api,
    };
    world.eip.mailpit = Some(("127.0.0.1".into(), api));
    // Kept until the end of the run and removed beside the homeserver
    // (`matrix_homeserver::shutdown_mailpit`); an interrupted run's leftover
    // carries the label for `docker rm`.
    mx::keep_mailpit(container);
    rewire(world).await;
}

pub async fn then_relay_one_click(world: &mut FoundryWorld, to: &str) {
    let (host, port) = world.eip.mailpit.clone().expect("the relay");
    let client = reqwest::Client::new();
    let mut id = None;
    let start = Instant::now();
    while start.elapsed() < EMAIL_BUDGET && id.is_none() {
        let list: serde_json::Value = client
            .get(format!("http://{host}:{port}/api/v1/messages"))
            .send()
            .await
            .expect("mailpit")
            .json()
            .await
            .unwrap_or_default();
        id = list["messages"].as_array().and_then(|m| {
            m.iter()
                .find(|x| {
                    x["To"]
                        .as_array()
                        .is_some_and(|t| t.iter().any(|a| a["Address"] == to))
                })
                .and_then(|x| x["ID"].as_str().map(str::to_string))
        });
        if id.is_none() {
            tokio::time::sleep(POLL).await;
        }
    }
    let id = id.unwrap_or_else(|| panic!("the relay holds an email to {to}"));
    let headers: serde_json::Value = client
        .get(format!("http://{host}:{port}/api/v1/message/{id}/headers"))
        .send()
        .await
        .expect("headers")
        .json()
        .await
        .unwrap_or_default();
    let lu = headers["List-Unsubscribe"].to_string();
    let lup = headers["List-Unsubscribe-Post"].to_string();
    assert!(
        lu.contains(EMAIL_STOP_ONE_CLICK),
        "List-Unsubscribe names the one-click stop address; {lu}"
    );
    assert!(
        lup.contains("List-Unsubscribe=One-Click"),
        "List-Unsubscribe-Post; {lup}"
    );
    let msg: serde_json::Value = client
        .get(format!("http://{host}:{port}/api/v1/message/{id}"))
        .send()
        .await
        .expect("message")
        .json()
        .await
        .unwrap_or_default();
    world.eip.received = Some(ReceivedEmail {
        to: to.into(),
        subject: msg["Subject"].as_str().unwrap_or_default().into(),
        body: msg["Text"].as_str().unwrap_or_default().into(),
        headers: BTreeMap::new(),
    });
}

// ===========================================================================
// The operator's process, replicas, settings and the doctor
// ===========================================================================

/// "Foundry runs as the operator's process with the Foundry Slack app set up":
/// the prebuilt binary on this scenario's schema, the in-process app's worker off.
pub async fn given_process(world: &mut FoundryWorld) {
    ensure_slack(world).await;
    world.eip.spec.slack_app = SlackAppSetup::Complete;
    world.eip.spec.in_process_worker = false;
    rewire(world).await;
    start_process(world).await;
}

async fn database_url(world: &FoundryWorld) -> (String, String) {
    let base_url = ensure_postgres().await;
    let schema = tm::harness(world).schema.clone();
    (
        format!("{base_url}?options=-csearch_path%3D{schema}"),
        schema,
    )
}

pub async fn start_process(world: &mut FoundryWorld) {
    let (url, schema) = database_url(world).await;
    let env: Vec<(&str, String)> = instance_env(world);
    let process = FoundrySubprocess::spawn_with_env_overrides(&url, schema, 1, &env)
        .await
        .unwrap_or_else(|e| panic!("start the operator's foundry: {e}"));
    world.eip.process = Some(process);
    world
        .eip
        .sessions
        .retain(|(b, _), _| !b.starts_with("http://127.0.0.1"));
}

/// "Foundry has been killed and started again while Slack was still down":
/// the first attempt (already asked of Slack, see `given_filed`) settles,
/// Foundry's 10 s call timeout, so the route reads it is retrying; then
/// SIGKILL and start again, between that attempt and the 30 s retry. A kill
/// in the middle of the send would leave its 45 s lease live; that case is
/// the replica lane's (scenarios 66, 131).
pub async fn kill_between_attempts(world: &mut FoundryWorld) {
    then_route_reads(world, "#auth-dev", "Retrying: Slack did not answer", false).await;
    restart_process(world).await;
    world.eip.restarted_between_attempts = true;
}

/// SIGKILL (kill_on_drop) and start again.
pub async fn restart_process(world: &mut FoundryWorld) {
    world.eip.process = None;
    tokio::time::sleep(Duration::from_millis(300)).await;
    start_process(world).await;
}

/// The operator's process output (stdout and stderr, both kept on disk).
pub fn process_output(world: &FoundryWorld) -> String {
    let schema = tm::harness(world).schema.clone();
    let dir = std::path::PathBuf::from("/tmp/foundry-slice6");
    let mut out = String::new();
    for f in [
        format!("{schema}.stdout.log"),
        format!("{schema}.stderr.log"),
    ] {
        out.push_str(&std::fs::read_to_string(dir.join(f)).unwrap_or_default());
    }
    if let Some((_, s)) = &world.eip.startup {
        out.push_str(s);
    }
    out
}

pub fn then_output_clean(world: &FoundryWorld) {
    let out = process_output(world);
    let mut sentinels = vec![
        sd::CLIENT_SECRET.to_string(),
        sd::SIGNING_SECRET.to_string(),
        SECRETS_KEY.to_string(),
        NEW_SECRETS_KEY.to_string(),
    ];
    if let Some(d) = &world.eip.slack {
        sentinels.extend(d.issued_tokens());
    }
    for s in sentinels {
        assert!(
            !out.contains(&s),
            "Foundry's output never contains a credential (EIP-KPI-5)"
        );
    }
}

pub async fn restart_with_keys(
    world: &mut FoundryWorld,
    key: Option<&str>,
    previous: Option<&str>,
) {
    world.eip.spec.secrets_key = key.map(str::to_string);
    world.eip.spec.previous_secrets_key = previous.map(str::to_string);
    restart_process(world).await;
}

/// "the operator starts Foundry with <setting>" (expects a refusal).
pub async fn when_start_with(world: &mut FoundryWorld, setting: &str) {
    ensure_slack(world).await;
    world.eip.spec.slack_app = SlackAppSetup::Complete;
    match setting.trim() {
        "a secrets key that is not 32 bytes" => {
            world.eip.spec.secrets_key = Some("dG9vLXNob3J0".into())
        }
        "only two of the three Foundry Slack app settings" => {
            world.eip.spec.slack_app = SlackAppSetup::Partial
        }
        "a delivery lease shorter than 35 seconds" => world.eip.spec.lease_seconds = 20,
        other => panic!("unknown setting {other:?}"),
    }
    let (url, schema) = database_url(world).await;
    let env = instance_env(world);
    let out = tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("foundry"));
        // Every setting the operator's process needs (as FoundrySubprocess
        // gives it), so the only thing wrong is the row's setting.
        cmd.env("DATABASE_URL", url)
            .env("FOUNDRY_DB_SCHEMA", schema)
            .env("FOUNDRY_HOST", "127.0.0.1")
            .env("FOUNDRY_PORT", "0")
            .env("METRICS_HOST", "127.0.0.1")
            .env("METRICS_PORT", "0")
            .env(
                "SESSION_SECRET",
                "eip-test-secret-must-be-at-least-32-bytes-long-yes",
            )
            .env("SESSION_COOKIE_SECURE", "false")
            .env("MACHINE_TOKEN_PUBLIC_KEYS", TEST_MACHINE_TOKEN_PUBLIC_KEY)
            .env("FOUNDRY_SKIP_MIGRATIONS", "1")
            .env("RUST_LOG_FORMAT", "pretty")
            .env("NO_COLOR", "1");
        for (k, v) in env {
            cmd.env(k, v);
        }
        let child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("start foundry");
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut child = child;
        loop {
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        child.wait_with_output().expect("collect output")
    })
    .await
    .expect("join");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    world.eip.startup = Some((out.status.code(), text));
}

pub fn then_refused_start(world: &FoundryWorld, name: &str) {
    let (code, out) = world
        .eip
        .startup
        .clone()
        .expect("the operator started foundry");
    assert!(
        code.is_some() && code != Some(0),
        "Foundry refuses to start; exit {code:?}"
    );
    let probe = match name.trim() {
        "the secrets key" => "FOUNDRY_SECRETS_KEY",
        "the Slack app settings" => "SLACK_",
        "the delivery lease" => "FOUNDRY_INTEGRATIONS_LEASE_SECONDS",
        other => panic!("unknown setting name {other:?}"),
    };
    assert!(
        out.contains("health.startup.refused") && out.contains(probe),
        "the refusal names {name}; {out}"
    );
}

/// The operator's metrics (subprocess only).
pub async fn scrape(world: &FoundryWorld) -> crate::support::metrics_scrape::ScrapeSnapshot {
    let p = world
        .eip
        .process
        .as_ref()
        .expect("the operator's process exports metrics");
    crate::support::metrics_scrape::scrape_metrics(p.metrics_addr).await
}

/// Every expected series of the delivery counter is registered and reads 0
/// (kind × event × outcome, DDR-22).
pub async fn then_counters_zero(world: &mut FoundryWorld) {
    let s = scrape(world).await;
    let samples = s.samples_for("foundry_integration_deliveries_total");
    for kind in ["slack", "matrix", "email"] {
        for event in EventChoice::ALL {
            for outcome in ["delivered", "retried", "failed", "skipped"] {
                let found = samples.iter().find(|m| {
                    m.labels.get("kind").map(String::as_str) == Some(kind)
                        && m.labels.get("event").map(String::as_str) == Some(event.wire())
                        && m.labels.get("outcome").map(String::as_str) == Some(outcome)
                });
                let found = found.unwrap_or_else(|| {
                    panic!(
                        "the series {{kind={kind}, event={}, outcome={outcome}}} is registered",
                        event.wire()
                    )
                });
                assert_eq!(
                    found.value,
                    0.0,
                    "{{kind={kind}, event={}, outcome={outcome}}} reads 0",
                    event.wire()
                );
            }
        }
    }
}

pub async fn then_counter_delivered(world: &mut FoundryWorld) {
    let mut last = 0.0;
    let start = Instant::now();
    while start.elapsed() < DELIVERY_BUDGET {
        let s = scrape(world).await;
        last = s
            .samples_for("foundry_integration_deliveries_total")
            .iter()
            .filter(|m| {
                m.labels.get("kind").map(String::as_str) == Some("slack")
                    && m.labels.get("event").map(String::as_str) == Some("issue.created")
                    && m.labels.get("outcome").map(String::as_str) == Some("delivered")
            })
            .map(|m| m.value)
            .sum();
        if (last - 1.0).abs() < f64::EPSILON {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("the delivery counters show one Slack New issue delivered; {last}");
}

pub async fn then_counter_labels_closed(world: &mut FoundryWorld) {
    let s = scrape(world).await;
    for name in [
        "foundry_integration_deliveries_total",
        "foundry_integration_deliveries_pending",
        "foundry_outbox_undispatched",
    ] {
        let keys = s.label_keys_for(name);
        for k in keys {
            assert!(
                ["kind", "event", "outcome", "state"].contains(&k.as_str()),
                "{name} carries only closed labels; found {k}"
            );
        }
    }
}

pub async fn then_degraded(world: &mut FoundryWorld) {
    let s = scrape(world).await;
    assert_eq!(
        s.sum_for("foundry_integration_secrets_degraded"),
        1.0,
        "the degraded gauge reads 1"
    );
    let failures: f64 = s
        .samples_for("probe_failures_total")
        .iter()
        .filter(|m| m.labels.get("probe_name").map(String::as_str) == Some("secrets"))
        .map(|m| m.value)
        .sum();
    assert!(failures >= 1.0, "a secrets probe failure is counted");
}

pub async fn then_not_lost(world: &mut FoundryWorld) {
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    assert!(
        !text(&page).contains("lost its Slack connection"),
        "the connection is not marked lost; {}",
        tm::describe(&page)
    );
    then_no_connection_email(world, "Priya", Kind::Slack).await;
}

pub async fn then_board_opens(world: &mut FoundryWorld, who: &str, project: &str) {
    let board = get(world, who, &project_path(world, project)).await;
    assert_eq!(
        board.status,
        StatusCode::OK,
        "the board still opens; {}",
        tm::describe(&board)
    );
}

/// `foundry doctor rotate-secrets` with the process's settings.
pub async fn when_rotate(world: &mut FoundryWorld) {
    let (url, _) = database_url(world).await;
    let env = instance_env(world);
    let out = tokio::task::spawn_blocking(move || {
        let mut cmd =
            assert_cmd::Command::cargo_bin("foundry").expect("the prebuilt foundry binary");
        cmd.env("DATABASE_URL", url)
            .args(["doctor", "rotate-secrets"]);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.output().expect("run foundry doctor rotate-secrets")
    })
    .await
    .expect("join");
    world.eip.cli = Some((
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into(),
        String::from_utf8_lossy(&out.stderr).into(),
    ));
}

pub fn then_rotated(world: &FoundryWorld, n: usize) {
    let (code, out, err) = world.eip.cli.clone().expect("the operator ran the doctor");
    assert_eq!(code, 0, "rotate-secrets succeeds; {err}");
    assert!(
        out.contains(&format!("{n} credential")),
        "it reports {n} credential sealed again; {out}"
    );
    assert!(
        !out.contains(SECRETS_KEY) && !out.contains(NEW_SECRETS_KEY),
        "it prints no key"
    );
}

/// "Foundry runs as two replicas on one database with the Foundry Slack app set up".
pub async fn given_replicas(world: &mut FoundryWorld) {
    ensure_slack(world).await;
    world.eip.spec.slack_app = SlackAppSetup::Complete;
    world.eip.spec.in_process_worker = false;
    rewire(world).await;
    let h = tm::harness(world);
    // Replicas run like real instances: their time ticks with the wall
    // (1-per-second pacing), and `advance` still jumps a lease.
    let replicas = MultiReplicaHarness::spawn_sharing_schema_with_time(
        2,
        h,
        now(world),
        ReplicaTime::WallTicking,
    )
    .await;
    world.eip.replicas = Some(replicas);
    // Each replica now delivers with the scenario's settings.
    rewire(world).await;
}

/// "the replica sending AUTH-12's message has stopped before hearing back":
/// every running replica stops (the one holding the claim among them) and a
/// fresh replica starts on the same schema, as a restarted pod would.
pub async fn stop_sending_replica(world: &mut FoundryWorld) {
    slack_asked_to_post(
        world,
        "#auth-dev",
        "a replica has sent AUTH-12's message to Slack",
    )
    .await;
    // The restarted pod reads the same time the stopped ones did (their
    // advances and the wall time that passed), so a lease they took expires
    // when the scenario says.
    let at = world
        .eip
        .replicas
        .as_ref()
        .map_or_else(|| now(world), MultiReplicaHarness::now);
    world.eip.replicas = None;
    let h = tm::harness(world);
    world.eip.replicas = Some(
        MultiReplicaHarness::spawn_sharing_schema_with_time(1, h, at, ReplicaTime::WallTicking)
            .await,
    );
    // The restarted pod starts with the operator's settings, its worker on.
    rewire(world).await;
}

/// "the replica sending AUTH-12's message stopped before hearing back, and the
/// other replica kept running": once Slack has been asked, only the replica
/// holding AUTH-12's claim stops (its delivery worker with it); no fresh
/// replica starts, so the survivor must take the delivery over after the lease.
pub async fn stop_claiming_replica_only(world: &mut FoundryWorld, key: &str) {
    slack_asked_to_post(
        world,
        "#auth-dev",
        "a replica has sent the message to Slack",
    )
    .await;
    let claimant = claiming_replica(world, key).await;
    let replicas = world
        .eip
        .replicas
        .as_mut()
        .expect("Foundry runs as two replicas");
    replicas.stop_replica(claimant);
}

/// Which replica holds `key`'s delivery claim: the pending delivery's
/// `claim_id`, named to its taker by the worker's claimant seam (DIST-22).
/// Exactly one replica took it.
async fn claiming_replica(world: &FoundryWorld, key: &str) -> usize {
    let replicas = world
        .eip
        .replicas
        .as_ref()
        .expect("Foundry runs as two replicas");
    let claim_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT claim_id FROM integration_deliveries WHERE issue_key = $1 AND status = 'pending'",
    )
    .bind(key)
    .fetch_one(&replicas.shared_pool)
    .await
    .unwrap_or_else(|e| panic!("precondition: {key}'s delivery is pending; {e}"));
    let claim_id = claim_id.unwrap_or_else(|| panic!("precondition: {key}'s delivery is claimed"));
    match replicas.replicas_that_took_claim(claim_id).as_slice() {
        [claimant] => *claimant,
        takers => panic!("exactly one replica took {key}'s claim; took it: {takers:?}"),
    }
}

pub async fn file_many(world: &mut FoundryWorld, who: &str, n: usize, project: &str) {
    capture_before(world).await;
    let mut futures = Vec::new();
    for i in 0..n {
        let number = *world.eip.next_numbers.get(project).unwrap_or(&1) + i as i64;
        world
            .eip
            .issue_projects
            .insert(format!("AUTH-{number}"), project.to_string());
        let path = format!("{}{}/issues", base(world), project_path(world, project));
        let email = email_of(world, who);
        let cookie = cookie(world, email.as_deref()).await;
        let req = http(world).post(path).header(COOKIE, cookie).form(&[
            ("title", format!("Burst {}", i + 1)),
            ("description", String::new()),
            ("_csrf", world.eip.csrf.clone()),
        ]);
        futures.push(req.send());
    }
    for r in futures::future::join_all(futures).await {
        let r = r.expect("file");
        assert!(
            r.status().is_success() || r.status().is_redirection(),
            "precondition: filed"
        );
    }
}

// ===========================================================================
// Matrix
// ===========================================================================

async fn hs(world: &mut FoundryWorld) -> Homeserver {
    if world.eip.homeserver.is_none() {
        let server = Homeserver::connect().await;
        world.eip.proxy = Some(FaultProxy::spawn(server.base.clone()).await);
        world.eip.homeserver = Some(server);
        world.eip.names = Names::new();
    }
    world.eip.homeserver.clone().expect("the homeserver")
}

async fn account(world: &mut FoundryWorld, scenario_name: &str) -> Account {
    if let Some(a) = world.eip.accounts.get(scenario_name) {
        return a.clone();
    }
    let server = hs(world).await;
    let real = world.eip.names.real(scenario_name);
    let local = real
        .trim_start_matches('@')
        .split(':')
        .next()
        .expect("a local part")
        .to_string();
    let a = server.register(&local).await;
    world
        .eip
        .accounts
        .insert(scenario_name.to_string(), a.clone());
    a
}

/// "the homeserver matrix.canzan.dev has the bot account … and Lena's account …"
/// — the Matrix feature's instance step: besides the accounts it hands the
/// scenario's settings (secrets key, delivery worker) to the app, as the Slack
/// features' "the instance has the Foundry Slack app set up" does.
pub async fn given_homeserver_accounts(world: &mut FoundryWorld, bot: &str, lena: &str) {
    account(world, bot).await;
    account(world, lena).await;
    rewire(world).await;
}

pub async fn given_room(
    world: &mut FoundryWorld,
    alias: &str,
    name: Option<&str>,
    members: usize,
    encrypted: bool,
) {
    let server = hs(world).await;
    let lena = account(world, "@lena:canzan.dev").await;
    let real = world.eip.names.real(alias);
    let local = real
        .trim_start_matches('#')
        .split(':')
        .next()
        .expect("alias local")
        .to_string();
    let room = server
        .create_room(&lena, &local, name.unwrap_or("Backend private"), encrypted)
        .await;
    for i in 1..members {
        let m = account(world, &format!("@member{i}:canzan.dev")).await;
        server.invite(&lena, &room, &m.user_id).await;
        server.join(&m, &room).await;
    }
    world.eip.rooms.insert(alias.to_string(), room);
}

fn room_id(world: &FoundryWorld, alias: &str) -> String {
    world
        .eip
        .rooms
        .get(alias)
        .cloned()
        .unwrap_or_else(|| panic!("the scenario has the room {alias}"))
}

pub async fn invite_bot(world: &mut FoundryWorld, room: &str) {
    let server = hs(world).await;
    let lena = account(world, "@lena:canzan.dev").await;
    let bot = account(world, "@foundry:canzan.dev").await;
    server
        .invite(&lena, &room_id(world, room), &bot.user_id)
        .await;
}

pub async fn kick_bot(world: &mut FoundryWorld, room: &str) {
    let server = hs(world).await;
    let lena = account(world, "@lena:canzan.dev").await;
    let bot = account(world, "@foundry:canzan.dev").await;
    server
        .kick(&lena, &room_id(world, room), &bot.user_id)
        .await;
}

pub async fn encrypt_room(world: &mut FoundryWorld, room: &str) {
    let server = hs(world).await;
    let lena = account(world, "@lena:canzan.dev").await;
    server.enable_encryption(&lena, &room_id(world, room)).await;
}

pub async fn log_bot_out(world: &mut FoundryWorld) {
    let server = hs(world).await;
    let bot = account(world, "@foundry:canzan.dev").await;
    server.logout(&bot.access_token).await;
}

/// "Priya connects Matrix with <address> and <token>".
pub async fn connect_matrix(
    world: &mut FoundryWorld,
    who: &str,
    address: &str,
    token: &str,
) -> Answer {
    let server = hs(world).await;
    let bot = account(world, "@foundry:canzan.dev").await;
    let token_value = match token.trim() {
        "the bot's access token" => bot.access_token.clone(),
        "a new access token for the bot" => server.login(&bot.user_id).await,
        "an access token the homeserver refuses" => "syt_eip_refused_token".into(),
        other => panic!("unknown token {other:?}"),
    };
    let fields = vec![
        ("homeserver".to_string(), address.to_string()),
        ("access_token".to_string(), token_value),
    ];
    let answer = post(world, who, MATRIX_CONNECT, &fields, true).await;
    follow(world, who, answer).await
}

pub async fn when_connect_matrix(world: &mut FoundryWorld, who: &str, address: &str, token: &str) {
    capture_before(world).await;
    hs(world).await;
    world.eip.proxy_before = world.eip.proxy.as_ref().map(FaultProxy::connections);
    let answer = connect_matrix(world, who, address, token).await;
    world.eip.when_answer = Some(answer);
}

pub async fn given_matrix_connected(world: &mut FoundryWorld) {
    let answer = connect_matrix(
        world,
        "Priya",
        mx::FOUNDRY_ADDRESS,
        "the bot's access token",
    )
    .await;
    went_through(&answer, "Priya's Connect Matrix");
    let seen = world.eip.names.as_scenario(&text(&answer));
    assert!(
        seen.contains("Connected as @foundry:canzan.dev"),
        "precondition: Priya reads the Matrix connection; {seen}"
    );
}

pub async fn then_no_matrix_connection(world: &mut FoundryWorld) {
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "control: Priya opens the workspace integrations page"
    );
    assert!(
        !text(&page).contains("Connected as"),
        "Canzan Labs has no Matrix connection; {}",
        tm::describe(&page)
    );
}

/// "Priya is offered Connect Matrix on the workspace integrations page": the
/// Matrix form asks for a homeserver and a token afresh (its button reads
/// Connect Matrix, not Replace token) and no connection line remains.
pub async fn then_connect_matrix_offered(world: &mut FoundryWorld) {
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "control: Priya opens the workspace integrations page"
    );
    let form = tm::forms(&page.body)
        .into_iter()
        .any(|f| f.action == MATRIX_CONNECT || f.action.ends_with(MATRIX_CONNECT));
    let seen = text(&page);
    assert!(
        form && seen.contains("Connect Matrix") && !seen.contains("Replace token"),
        "Priya is offered Connect Matrix; {}",
        tm::describe(&page)
    );
    assert!(
        !seen.contains("Connected as"),
        "no Matrix connection line remains; {}",
        tm::describe(&page)
    );
}

/// "Priya still finds Disconnect Matrix on the workspace integrations page":
/// the control for a refused disconnect — the admin's page offers the door
/// (so the refusal is about the person, not a missing door), and it opens.
pub async fn then_disconnect_offered(world: &mut FoundryWorld, kind: Kind) {
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    let door = kind.disconnect_path();
    let links = attr_of(&page.body, "a[href]", "href");
    assert!(
        links.iter().any(|h| h.ends_with(door))
            && text(&page).contains(&format!("Disconnect {kind:?}")),
        "control: Priya finds Disconnect {kind:?} on the workspace integrations page; {}",
        tm::describe(&page)
    );
    let dialog = get(world, "Priya", door).await;
    assert_eq!(
        dialog.status,
        StatusCode::OK,
        "control: Priya's Disconnect {kind:?} opens; {}",
        tm::describe(&dialog)
    );
}

/// "Foundry no longer holds the bot's access token": no saved-token stamp on
/// the page, and the workspace's Matrix connection holds no sealed credential
/// (DDR-9: a disconnected connection holds none).
pub async fn then_matrix_token_forgotten(world: &mut FoundryWorld) {
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    assert!(
        select_all(&parse(&page.body), "[data-token-saved]").is_empty(),
        "the access token field shows no saved token; {}",
        tm::describe(&page)
    );
    let (held,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM integration_connections \
         WHERE kind = 'matrix' AND sealed_credential IS NOT NULL",
    )
    .fetch_one(&tm::pool(world))
    .await
    .expect("the Matrix connection's credential (operator view)");
    assert_eq!(held, 0, "Foundry holds no Matrix access token");
}

/// "nothing reached the listener on this machine or the homeserver".
pub fn then_homeserver_untouched(world: &FoundryWorld) {
    let before = world.eip.proxy_before.unwrap_or(0);
    let now = world
        .eip
        .proxy
        .as_ref()
        .map(FaultProxy::connections)
        .unwrap_or(0);
    assert_eq!(now, before, "no request reached the homeserver");
    let d = world
        .eip
        .decoy
        .as_ref()
        .expect("the When started a listener on this machine");
    assert_eq!(
        d.connections(),
        0,
        "no connection reached 127.0.0.1:{}",
        d.port
    );
}

pub async fn then_bot_member(world: &mut FoundryWorld, room: &str, member: bool) {
    let server = hs(world).await;
    let lena = account(world, "@lena:canzan.dev").await;
    let bot = account(world, "@foundry:canzan.dev").await;
    let members = server.joined_members(&lena, &room_id(world, room)).await;
    assert_eq!(
        members.contains(&bot.user_id),
        member,
        "@foundry:canzan.dev {} a member of {room}",
        if member { "is" } else { "is not" }
    );
}

pub async fn matrix_notices(world: &mut FoundryWorld, room: &str) -> Vec<serde_json::Value> {
    let server = hs(world).await;
    let lena = account(world, "@lena:canzan.dev").await;
    let bot = account(world, "@foundry:canzan.dev").await;
    server
        .messages(&lena, &room_id(world, room))
        .await
        .into_iter()
        .filter(|e| e["sender"] == bot.user_id.as_str() && e["content"]["msgtype"] == "m.notice")
        .collect()
}

pub async fn then_notice(world: &mut FoundryWorld, room: &str, bold: &str, line: &str) {
    let start = Instant::now();
    while start.elapsed() < DELIVERY_BUDGET {
        let notices = matrix_notices(world, room).await;
        if let Some(n) = notices.iter().find(|n| {
            n["content"]["formatted_body"]
                .as_str()
                .unwrap_or_default()
                .contains(bold)
        }) {
            let f = n["content"]["formatted_body"].as_str().unwrap_or_default();
            assert!(
                f.contains(&format!("<strong>{bold}</strong>"))
                    || f.contains(&format!("<b>{bold}</b>")),
                "{bold} is in bold; {f}"
            );
            assert!(
                f.contains(line) && f.contains("Open in Foundry"),
                "the notice carries {line:?} and Open in Foundry; {f}"
            );
            let body = n["content"]["body"].as_str().unwrap_or_default();
            assert!(
                body.contains(bold) && body.contains(line),
                "the plain text carries the same lines; {body}"
            );
            world.eip.notice_lines = Some((bold.to_string(), line.to_string()));
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("{room} shows a notice with {bold:?} within {DELIVERY_BUDGET:?}");
}

/// The room shows a notice carrying `wanted`, within the arrival window (the
/// 10 s delivery budget, or 45 s once the homeserver was silent: a send left
/// hanging must hit Foundry's client timeout before its retry can deliver).
pub async fn then_notice_text(world: &mut FoundryWorld, room: &str, wanted: &str) {
    let window = arrival_window(world);
    let start = Instant::now();
    while start.elapsed() < window {
        if matrix_notices(world, room).await.iter().any(|n| {
            n["content"]["body"]
                .as_str()
                .unwrap_or_default()
                .contains(wanted)
        }) {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("{room} shows a notice {wanted:?} within {window:?}");
}

pub async fn then_no_notice_about(world: &mut FoundryWorld, room: &str, key: &str) {
    tokio::time::sleep(absence_window(world)).await;
    let n = matrix_notices(world, room)
        .await
        .into_iter()
        .filter(|n| {
            n["content"]["body"]
                .as_str()
                .unwrap_or_default()
                .contains(key)
        })
        .count();
    assert_eq!(n, 0, "{room} shows no notice about {key}");
}

/// The never-pings oracle for Matrix plus readability: every word of the
/// user's text (four letters or more) survives in the notice's plain body.
pub async fn then_matrix_no_ping(world: &mut FoundryWorld, room: &str) {
    let input = world
        .eip
        .comment_text
        .clone()
        .expect("the When's user text");
    let words: Vec<String> = input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(str::to_string)
        .collect();
    let start = Instant::now();
    while start.elapsed() < DELIVERY_BUDGET {
        let notices = matrix_notices(world, room).await;
        if let Some(n) = notices.iter().rev().find(|n| {
            n["content"]["body"]
                .as_str()
                .unwrap_or_default()
                .contains("commented")
        }) {
            assert!(!mx::notifies(n), "the notice notifies nobody (DDR-13); {n}");
            let body = n["content"]["body"].as_str().unwrap_or_default();
            for w in &words {
                assert!(
                    body.contains(w.as_str()),
                    "the text arrives readable ({w}); {body}"
                );
            }
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("{room} shows the comment notice");
}

pub async fn when_matrix_test(world: &mut FoundryWorld, who: &str, room: &str) {
    capture_before(world).await;
    let id = route_id(world, who, "Identity Platform", room).await;
    let path = format!(
        "{}/routes/test",
        integrations_path(world, "Identity Platform")
    );
    let answer = post(world, who, &path, &[("route_id".into(), id)], true).await;
    world.eip.when_answer = Some(answer);
}

pub async fn then_matrix_token_hidden(world: &mut FoundryWorld) {
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    let tokens: Vec<String> = world
        .eip
        .accounts
        .values()
        .map(|a| a.access_token.clone())
        .collect();
    for t in tokens {
        assert!(
            !page.body.contains(&t),
            "the bot's access token appears nowhere on the page"
        );
    }
}

// ===========================================================================
// Small compositions the steps call (one call per step, Mandate 12)
// ===========================================================================

pub fn slack_membership(world: &mut FoundryWorld, channel: &str, foundry_in: bool) {
    let double = slack(world);
    if foundry_in {
        double.invite_foundry_to(channel);
    } else {
        double.remove_foundry_from(channel);
    }
}

pub async fn given_route_reads(world: &mut FoundryWorld, destination: &str, status: &str) {
    let status = if status.is_empty() {
        "Needs attention"
    } else {
        status
    };
    then_route_reads(world, destination, status, false).await;
}

pub async fn given_edit_add_event(
    world: &mut FoundryWorld,
    who: &str,
    event: EventChoice,
    destination: &str,
) {
    let answer = edit_add_event(world, who, event, destination).await;
    went_through(
        &answer,
        &format!("{who}'s edit of the route to {destination}"),
    );
}

pub async fn when_edit_add_event(
    world: &mut FoundryWorld,
    who: &str,
    event: EventChoice,
    destination: &str,
) {
    capture_before(world).await;
    let answer = edit_add_event(world, who, event, destination).await;
    world.eip.when_answer = Some(answer);
}

pub async fn then_history_still_opens(world: &mut FoundryWorld, destination: &str) {
    let page = open_history(world, "Lena", destination).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "control: Lena still finds the history; {}",
        tm::describe(&page)
    );
}

pub async fn when_disconnect(world: &mut FoundryWorld, who: &str, kind: Kind, with_token: bool) {
    capture_before(world).await;
    let answer = disconnect(world, who, kind, with_token).await;
    world.eip.when_answer = Some(answer);
}

pub fn then_uninstalls(world: &FoundryWorld, n: usize) {
    assert_eq!(
        slack(world).uninstall_count(),
        n,
        "Slack was asked {n} time(s) to remove Foundry's access"
    );
}

pub fn then_no_removal(world: &FoundryWorld, channel: &str) {
    let calls = slack(world).calls();
    assert!(
        !calls.iter().any(|c| c == "chat.delete"),
        "Slack was asked to remove no message from {channel}; calls {calls:?}"
    );
    assert!(
        !slack(world).shown_in(channel).is_empty(),
        "control: {channel} still shows the earlier message"
    );
}

pub async fn given_filed(
    world: &mut FoundryWorld,
    who: &str,
    project: &str,
    title: &str,
    shown: bool,
) {
    let attempts_before = world
        .eip
        .proxy
        .as_ref()
        .map(|p| p.requests_to(mx::ATTEMPT_PATH));
    let key = file(world, who, project, title).await;
    if shown {
        then_shows_key(world, "#auth-dev", &key).await;
    } else if let Some(channel) = world.eip.scripted_answer_channel.clone() {
        // Slack has a scripted answer for this channel's next post: the
        // delivery is settled once Slack was asked and Foundry recorded what
        // it answered, so the When reads what Foundry made of the answer
        // (never a delivery the dispatcher has not created yet).
        slack_asked_to_post(
            world,
            &channel,
            "Foundry has asked Slack to post the message",
        )
        .await;
        delivery_settled(world, &channel, &key).await;
    } else if world.eip.slack.is_some() && slack(world).leaves_posts_unanswered() {
        // Slack is silent or holding: the message's 24-hour clock starts at
        // its first attempt, so a later "24 hours pass" must not move the
        // clock before Foundry has asked Slack once. A Matrix scenario has no
        // Slack double: its Given that follows (a route status, a room's
        // notices) says what the filing must have led to.
        slack_asked_to_post(
            world,
            "#auth-dev",
            "Foundry has asked Slack to post the message",
        )
        .await;
    } else if world.eip.homeserver_scripted {
        // The homeserver has a scripted answer for Foundry's next send (as
        // Slack's path above): the filing is a precondition only once
        // Foundry's first attempt has reached the homeserver (its pre-send
        // read of the room's encryption) and the route's history shows it,
        // so the When's clock move or healing comes after that attempt, never
        // before the dispatcher has created the delivery. An armed answer is
        // settled once Foundry has recorded it; a send the homeserver never
        // answers stays in flight into the When.
        homeserver_attempted(world, attempts_before.unwrap_or(0)).await;
        let room = world
            .eip
            .matrix_destination
            .clone()
            .expect("a Given routed the project to a Matrix room");
        delivery_attempted(world, &room, &key).await;
        if !world.eip.silence_seen {
            delivery_settled(world, &room, &key).await;
        }
    }
}

/// Wait until Foundry has begun more send attempts at the homeserver than
/// `before`: the delivery has been dispatched and its first attempt made.
async fn homeserver_attempted(world: &FoundryWorld, before: usize) {
    let proxy = world
        .eip
        .proxy
        .as_ref()
        .expect("the homeserver's fault proxy");
    let start = Instant::now();
    while start.elapsed() < DELIVERY_BUDGET {
        if proxy.requests_to(mx::ATTEMPT_PATH) > before {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!(
        "Foundry attempts to send the notice to the homeserver within {DELIVERY_BUDGET:?}; \
         {} attempt(s) reached it, {before} before the filing",
        proxy.requests_to(mx::ATTEMPT_PATH)
    );
}

/// Wait until the route's history shows `key`'s delivery attempted: no
/// longer "Waiting" (Foundry has claimed it and begun its first attempt).
async fn delivery_attempted(world: &mut FoundryWorld, channel: &str, key: &str) {
    let start = Instant::now();
    let mut entries = Vec::new();
    while start.elapsed() < STATUS_BUDGET {
        let page = open_history(world, "Lena", channel).await;
        entries = texts_of(&page.body, "[data-delivery]");
        if entries
            .iter()
            .any(|e| e.contains(&format!(" · {key} · ")) && !e.ends_with(" · Waiting"))
        {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("Foundry begins sending {key}'s message to {channel}; history {entries:?}");
}

/// Wait until the route's history shows `key`'s delivery past its first
/// attempt: no longer "Waiting" or "Sending", so Foundry has recorded what
/// Slack or the homeserver answered, or its silence (delivered, retrying,
/// failed or skipped).
async fn delivery_settled(world: &mut FoundryWorld, channel: &str, key: &str) {
    let start = Instant::now();
    let mut entries = Vec::new();
    while start.elapsed() < STATUS_BUDGET {
        let page = open_history(world, "Lena", channel).await;
        entries = texts_of(&page.body, "[data-delivery]");
        let settled = entries.iter().any(|e| {
            e.contains(&format!(" · {key} · "))
                && !e.ends_with(" · Waiting")
                && !e.ends_with(" · Sending")
        });
        if settled {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("Foundry records the answer to {key}'s message to {channel}; history {entries:?}");
}

/// Wait until Slack has been asked at least once to post to `channel`
/// (accepted or not): the delivery has been dispatched and attempted.
async fn slack_asked_to_post(world: &FoundryWorld, channel: &str, what: &str) {
    let double = slack(world);
    let channel = channel.to_owned();
    eventually(DELIVERY_BUDGET, what, || {
        let double = double.clone();
        let channel = channel.clone();
        async move {
            if double.posts_to(&channel).is_empty() {
                Err("no post".into())
            } else {
                Ok(())
            }
        }
    })
    .await;
}

pub async fn given_commented_and_shown(world: &mut FoundryWorld, who: &str, key: &str, body: &str) {
    comment(world, who, key, body).await;
    then_message(world, "#auth-dev", key, "commented", None).await;
}

pub async fn when_change_comment(world: &mut FoundryWorld, who: &str, delete: bool) {
    capture_before(world).await;
    let key = world.eip.commented_key.clone().expect("a Given commented");
    change_comment(world, who, &key, delete).await;
}

pub async fn then_shows_key_times(world: &mut FoundryWorld, channel: &str, key: &str, once: bool) {
    if once {
        then_shows_key_once(world, channel, key).await;
    } else {
        then_shows_key(world, channel, key).await;
    }
}

pub async fn then_excerpt_of_when(world: &mut FoundryWorld, channel: &str, whole: bool) {
    let n = world
        .eip
        .comment_text
        .as_deref()
        .map(|c| c.chars().count())
        .expect("the When's comment");
    then_excerpt(world, channel, n, whole).await;
}

pub async fn when_delete_lane(
    world: &mut FoundryWorld,
    who: &str,
    lane: &str,
    move_to: Option<&str>,
) {
    capture_before(world).await;
    delete_lane(world, who, lane, move_to).await;
}

/// The before-image is read while the project is still deleted: its page
/// answers "not found" (recorded as gone), the rest is read as usual.
pub async fn when_restore(world: &mut FoundryWorld, project: &str) {
    world.eip.before = Some(universe_with_gone(world, &[project]).await);
    restore_project(world, project).await;
}

pub async fn given_sam_commented(world: &mut FoundryWorld, key: &str, body: &str, deleted: bool) {
    comment(world, "Sam", key, body).await;
    if deleted {
        change_comment(world, "Sam", key, true).await;
    }
}

/// A precondition the Background already makes true ("Sam has never touched
/// AUTH-9": Lena filed it and nobody else wrote to it).
pub fn given_nothing(world: &mut FoundryWorld) {
    let _ = world;
}

pub async fn then_notice_plain_text(world: &mut FoundryWorld) {
    let (bold, line) = world
        .eip
        .notice_lines
        .clone()
        .expect("a Then found the notice");
    let room = "#auth:canzan.dev";
    let notices = matrix_notices(world, room).await;
    let n = notices
        .iter()
        .find(|n| {
            n["content"]["body"]
                .as_str()
                .unwrap_or_default()
                .contains(&bold)
        })
        .expect("the notice");
    let body = n["content"]["body"].as_str().unwrap_or_default();
    assert!(
        body.contains(&bold) && body.contains(&line),
        "the plain text carries both lines; {body}"
    );
}

pub async fn then_notice_about(world: &mut FoundryWorld, room: &str, key: &str, some: bool) {
    if some {
        then_notice_text(world, room, key).await;
    } else {
        then_no_notice_about(world, room, key).await;
    }
}

/// "24 hours" / "30 seconds" as a duration.
pub fn duration(n: u64, unit: &str) -> Duration {
    match unit {
        "hours" => Duration::from_secs(n * 3600),
        "minute" | "minutes" => Duration::from_secs(n * 60),
        "seconds" => Duration::from_secs(n),
        "days" => Duration::from_secs(n * 86_400),
        other => panic!("a wait is in days, hours, minutes or seconds, not {other:?}"),
    }
}

/// "the visitor is sent to sign in, as from any page that needs a session":
/// the shipped `redirect_to("/sign-in")` answer (issues.rs, comments.rs) — a
/// 303 See Other to `/sign-in` with no body, the shape the signed-out landing
/// scenario checks (feature_remaining_surfaces `landing_redirects`). Control:
/// the signed-out landing page answers exactly so, so the oracle is the
/// product's own, not a guess.
pub async fn then_sent_to_sign_in(world: &mut FoundryWorld) {
    let answer = tm::last(world).clone();
    let landing = get(world, "a signed-out visitor", "/").await;
    assert_eq!(
        (landing.status, landing.location.as_deref()),
        (StatusCode::SEE_OTHER, Some("/sign-in")),
        "control: a signed-out visitor is sent from the landing page to sign in; {}",
        tm::describe(&landing)
    );
    assert_eq!(
        (answer.status, answer.location.as_deref()),
        (StatusCode::SEE_OTHER, Some("/sign-in")),
        "a signed-out visitor at {} is sent to sign in (303 to /sign-in); {}",
        answer.path,
        tm::describe(&answer)
    );
}

/// Control for a refused foreign choice: the member's own project still
/// takes a choice (so the refusal is about the project, not a missing door).
pub async fn then_own_choice_accepted(world: &mut FoundryWorld, who: &str, project: &str) {
    let answer = choose_emails(world, who, &[EventChoice::NewIssue], Scope::All, project).await;
    assert!(
        text(&answer).contains("Saved."),
        "control: {who}'s choice for {project} is accepted; {}",
        tm::describe(&answer)
    );
}

/// "#auth-dev shows AUTH-7's comment message [with|without "<text>"]".
pub async fn then_comment_message_words(
    world: &mut FoundryWorld,
    channel: &str,
    key: &str,
    with: &str,
    text: &str,
) {
    let shown = match with {
        "" => None,
        "with" => Some((true, text)),
        "without" => Some((false, text)),
        other => panic!("a comment message is shown with or without its text, not {other:?}"),
    };
    then_comment_message(world, channel, key, shown).await;
}

/// File `n` issues, then wait until the channel shows every one of them.
pub async fn given_filed_many_shown(world: &mut FoundryWorld, who: &str, n: usize, project: &str) {
    let before = slack(world).shown_in("#auth-dev").len();
    for i in 0..n {
        file(world, who, project, &format!("Retention filler {}", i + 1)).await;
    }
    let double = slack(world);
    eventually(
        Duration::from_secs(120),
        "#auth-dev shows every filed issue",
        || {
            let double = double.clone();
            async move {
                let k = double.shown_in("#auth-dev").len().saturating_sub(before);
                if k >= n {
                    Ok(())
                } else {
                    Err(format!("{k} of {n}"))
                }
            }
        },
    )
    .await;
}

/// The route's history lists (or no longer lists) the issue's delivery.
pub async fn then_history_lists(
    world: &mut FoundryWorld,
    destination: &str,
    key: &str,
    listed: bool,
) {
    let page = open_history(world, "Lena", destination).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "Lena opens the history; {}",
        tm::describe(&page)
    );
    let entries = texts_of(&page.body, "[data-delivery]");
    let has = entries
        .iter()
        .any(|e| e.contains(&format!(" {key} ")) || e.contains(&format!("· {key} ·")));
    assert_eq!(
        has,
        listed,
        "the history {} {key}; {} entries",
        if listed { "lists" } else { "no longer lists" },
        entries.len()
    );
}

/// The fault proxy in front of the homeserver: silent, or forwarding.
pub fn homeserver_answers(world: &mut FoundryWorld, answers: bool) {
    if !answers {
        world.eip.silence_seen = true;
        world.eip.homeserver_scripted = true;
    }
    let proxy = world
        .eip
        .proxy
        .as_ref()
        .expect("the homeserver's fault proxy");
    proxy.set_mode(if answers {
        mx::PROXY_FORWARD
    } else {
        mx::PROXY_SILENT
    });
}

pub async fn when_homeserver_answers(world: &mut FoundryWorld) {
    capture_before(world).await;
    homeserver_answers(world, true);
}

/// "the page Sam confirmed on asked …" (the page held in front of him).
pub fn then_in_front_reads(world: &FoundryWorld, wanted: &str) {
    let page = world
        .eip
        .in_front
        .as_ref()
        .expect("a Given put a page in front");
    assert!(
        text(page).contains(wanted),
        "the page asked {wanted:?}; {}",
        tm::describe(page)
    );
}

/// "the homeserver will answer Foundry's next send with …".
pub fn homeserver_next_answer(world: &mut FoundryWorld, answer: &str) {
    world.eip.homeserver_scripted = true;
    let proxy = world
        .eip
        .proxy
        .as_ref()
        .expect("the homeserver's fault proxy");
    proxy.arm(
        mx::SEND_PATH,
        match answer.trim() {
            "a request to slow down for 30 seconds" => mx::Fault::SlowDown,
            "a server error" => mx::Fault::ServerError,
            other => panic!("the fault proxy cannot answer {other:?}"),
        },
    );
}

/// "the operator has run foundry doctor rotate-secrets" (and it succeeded).
pub async fn given_rotated(world: &mut FoundryWorld) {
    when_rotate(world).await;
    let (code, _, err) = world.eip.cli.clone().expect("the doctor ran");
    assert_eq!(code, 0, "precondition: rotate-secrets succeeds; {err}");
}

pub async fn then_history_lists_once(world: &mut FoundryWorld, destination: &str, key: &str) {
    let page = open_history(world, "Lena", destination).await;
    let n = texts_of(&page.body, "[data-delivery]")
        .iter()
        .filter(|e| e.contains(&format!("· {key} ·")))
        .count();
    assert_eq!(
        n,
        1,
        "the history lists {key} exactly once; {}",
        tm::describe(&page)
    );
}

pub async fn then_each_once(world: &mut FoundryWorld, channel: &str, keys: &[String]) {
    for k in keys {
        then_shows_key(world, channel, k).await;
    }
    // Settle: a duplicate from the other replica would arrive within the window.
    tokio::time::sleep(DELIVERY_BUDGET).await;
    let shown = slack(world).shown_in(channel);
    for k in keys {
        let n = shown.iter().filter(|m| key_line(m, k)).count();
        assert_eq!(
            n, 1,
            "{channel} shows {k}'s message exactly once; {n} times"
        );
    }
}

fn credential_sentinels(world: &FoundryWorld) -> Vec<String> {
    let mut v = vec![
        sd::CLIENT_SECRET.to_string(),
        sd::SIGNING_SECRET.to_string(),
        SECRETS_KEY.to_string(),
        NEW_SECRETS_KEY.to_string(),
    ];
    if let Some(d) = &world.eip.slack {
        v.extend(d.issued_tokens());
    }
    v.extend(world.eip.accounts.values().map(|a| a.access_token.clone()));
    v
}

pub fn then_posts_carry_no_credential(world: &FoundryWorld) {
    let sentinels = credential_sentinels(world);
    for m in slack(world).all_posts() {
        let all = m.all_text();
        for s in &sentinels {
            assert!(!all.contains(s.as_str()), "no message carries a credential");
        }
    }
}

pub async fn then_notices_carry_no_credential(world: &mut FoundryWorld, room: &str) {
    let sentinels = credential_sentinels(world);
    for n in matrix_notices(world, room).await {
        let all = n.to_string();
        for s in &sentinels {
            assert!(!all.contains(s.as_str()), "no notice carries a credential");
        }
    }
}

/// "Lena chooses #auth:canzan.dev as the destination of a new Matrix route".
pub async fn when_choose_room(world: &mut FoundryWorld, who: &str, room: &str, project: &str) {
    capture_before(world).await;
    let real = world.eip.names.real(room);
    let new = format!(
        "{}/routes/new?kind=matrix&destination={}",
        integrations_path(world, project),
        urlencoding::encode(&real)
    );
    let answer = get(world, who, &new).await;
    world.eip.draft = Some(Route::new(project, Kind::Matrix, room));
    world.eip.when_answer = Some(answer);
}

/// Whether a notice's plain body is `key`'s new-comment notice at any content
/// level: Link only reads "{key} · {project} · New comment" (D-3 with D-2's
/// event name); Summary and Summary with comment text open with "{key} {title}"
/// and carry K-8's "{actor} commented · {project}" line.
fn is_comment_notice(body: &str, key: &str) -> bool {
    let link_only = body.starts_with(&format!("{key} · ")) && body.contains(" · New comment");
    let summary = body.starts_with(&format!("{key} ")) && body.contains(" commented · ");
    link_only || summary
}

/// The comment notice's formatted and plain bodies carry (or not) the text.
pub async fn then_comment_notice(
    world: &mut FoundryWorld,
    room: &str,
    key: &str,
    with: bool,
    text: &str,
) {
    let start = Instant::now();
    while start.elapsed() < DELIVERY_BUDGET {
        let notices = matrix_notices(world, room).await;
        if let Some(n) = notices
            .iter()
            .rev()
            .find(|n| is_comment_notice(n["content"]["body"].as_str().unwrap_or_default(), key))
        {
            let body = n["content"]["body"].as_str().unwrap_or_default();
            let formatted = n["content"]["formatted_body"].as_str().unwrap_or_default();
            assert_eq!(
                body.contains(text),
                with,
                "the plain body {} the comment text; {body}",
                if with { "carries" } else { "omits" }
            );
            assert_eq!(
                formatted.contains(text),
                with,
                "the formatted body {} the comment text; {formatted}",
                if with { "carries" } else { "omits" }
            );
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("{room} shows {key}'s comment notice within {DELIVERY_BUDGET:?}");
}

/// "the workspace's integration activity gains the line "…"" (R-4 pattern):
/// exactly one new line, and it reads exactly so.
pub async fn then_activity_gains_line(world: &mut FoundryWorld, line: &str) {
    let before = world.eip.before.clone().unwrap_or_default().activity;
    let page = get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    let lines = texts_of(&page.body, "[data-integration-activity] li");
    let new: Vec<&String> = lines.iter().filter(|l| !before.contains(l)).collect();
    assert_eq!(
        new,
        vec![&line.to_string()],
        "the integration activity gains exactly the line {line:?}"
    );
}

/// "the history shows none of Slack's raw text and no Slack credential or the
/// secrets key".
pub fn then_history_clean(world: &FoundryWorld) {
    let answer = world.eip.when_answer.clone().expect("the history");
    // Positive first, so the absences below are not vacuous: the history page
    // opened and lists the refused delivery.
    assert_eq!(answer.status, StatusCode::OK, "the history opens");
    assert!(
        answer.body.contains("New issue · AUTH-12 · "),
        "the history lists AUTH-12's refused delivery in Foundry's words"
    );
    for w in RAW_VENDOR_WORDS {
        assert!(
            !answer.body.contains(w),
            "the history shows none of Slack's raw text ({w})"
        );
    }
    for c in credential_sentinels(world) {
        assert!(
            !answer.body.contains(c.as_str()),
            "the history shows no credential"
        );
    }
}

/// "When Priya connects Matrix with a homeserver listening on this machine,
/// written as 127.0.0.1 | as an IPv4-mapped IPv6 address": a decoy listens on a
/// fresh port for this scenario (a failed bind fails the scenario).
pub async fn when_connect_matrix_local(world: &mut FoundryWorld, who: &str, form: &str) {
    let decoy = mx::Decoy::bind().await;
    let address = match form.trim() {
        "as 127.0.0.1" => format!("https://127.0.0.1:{}", decoy.port),
        "as an IPv4-mapped IPv6 address" => format!("https://[::ffff:127.0.0.1]:{}", decoy.port),
        other => panic!("unknown address form {other:?}"),
    };
    world.eip.decoy = Some(decoy);
    when_connect_matrix(world, who, &address, "the bot's access token").await;
}

/// "the homeserver gave the answer it was set to give" (fail-closed: an armed
/// fault that was never used means the scenario tested nothing).
pub fn then_fault_given(world: &FoundryWorld) {
    let proxy = world.eip.proxy.as_ref().expect("the fault proxy");
    assert!(
        proxy.fault_given(),
        "the homeserver's armed answer was given to Foundry"
    );
}

pub fn then_sends_counted(world: &FoundryWorld, n: usize) {
    let proxy = world.eip.proxy.as_ref().expect("the fault proxy");
    assert_eq!(
        proxy.requests_to(mx::SEND_PATH),
        n,
        "the homeserver was asked to send {n} time(s)"
    );
}

/// "When the operator finishes the key rotation …" (R-5 runbook steps 2-3).
pub async fn when_finish_rotation(world: &mut FoundryWorld) {
    capture_before(world).await;
    given_rotated(world).await;
    restart_with_keys(world, Some(NEW_SECRETS_KEY), None).await;
}

pub async fn then_secrets_healthy(world: &mut FoundryWorld) {
    let s = scrape(world).await;
    assert_eq!(
        s.sum_for("foundry_integration_secrets_degraded"),
        0.0,
        "the secrets key is not degraded"
    );
    let failures: f64 = s
        .samples_for("probe_failures_total")
        .iter()
        .filter(|m| m.labels.get("probe_name").map(String::as_str) == Some("secrets"))
        .map(|m| m.value)
        .sum();
    assert_eq!(failures, 0.0, "no secrets probe failure");
}

/// "both of Slack's posts for AUTH-12 carried the same message": the same
/// delivery was re-sent (at-least-once), not a second event.
pub fn then_posts_identical(world: &FoundryWorld, key: &str) {
    let posts: Vec<PostedMessage> = slack(world)
        .posts_to("#auth-dev")
        .into_iter()
        .filter(|m| key_line(m, key))
        .collect();
    assert_eq!(posts.len(), 2, "Slack received two posts for {key}");
    assert_eq!(
        posts[0].body, posts[1].body,
        "both posts carry the same message"
    );
}

#[cfg(test)]
mod universe_oracle_tests {
    //! Planted-violation controls for the Mandate 8 oracle (Sentinel / SA
    //! review): an unreadable page and an undeclared change must fail.
    use super::*;

    fn page(path: &str, status: u16, body: &str) -> Answer {
        Answer {
            path: path.into(),
            status: StatusCode::from_u16(status).unwrap(),
            content_type: None,
            location: None,
            session: None,
            body: body.into(),
        }
    }

    const WS: &str = r#"<div data-connection="slack">Connected to Canzan Labs</div><ul data-integration-activity><li>21 Oct 2026 · Priya Raman connected Slack (Canzan Labs)</li></ul>"#;
    const WS_NO_SEAM: &str = r#"<div>Connected to Canzan Labs</div>"#;
    const ROUTES: &str = r##"<div data-routes><div data-route-destination="#auth-dev" data-route-id="r1">#auth-dev Delivering</div></div>"##;

    #[test]
    fn a_project_deleted_on_purpose_is_read_as_gone_and_only_then() {
        let ws = page(WORKSPACE_INTEGRATIONS, 200, WS);
        let not_found = page("/p", 404, "Not found");
        let u = universe_from_pages(
            &ws,
            &[("Identity Platform".into(), not_found.clone())],
            &["Identity Platform"],
        )
        .unwrap();
        assert!(u.gone.contains("Identity Platform") && u.routes.is_empty());
        assert!(
            universe_from_pages(
                &ws,
                &[("Identity Platform".into(), page("/p", 200, ROUTES))],
                &["Identity Platform"],
            )
            .is_err(),
            "a project expected deleted whose page still answers is caught"
        );
        assert!(
            universe_from_pages(
                &ws,
                &[("Homelab Ops".into(), not_found)],
                &["Identity Platform"],
            )
            .is_err(),
            "only the named project may be unreadable"
        );
        let back = universe_from_pages(
            &ws,
            &[("Identity Platform".into(), page("/p", 200, ROUTES))],
            &[],
        )
        .unwrap();
        assert!(
            delta_violation(&u, &back, &[]).is_some(),
            "a gone project coming back is an undeclared change"
        );
    }

    #[test]
    fn an_unreadable_page_fails_the_universe() {
        let ws = page(WORKSPACE_INTEGRATIONS, 200, WS);
        let gone = page(
            "/team/backend/project/identity-platform/integrations",
            404,
            "Not found",
        );
        assert!(universe_from_pages(&ws, &[("Identity Platform".into(), gone)], &[]).is_err());
        assert!(universe_from_pages(&page(WORKSPACE_INTEGRATIONS, 404, ""), &[], &[]).is_err());
    }

    #[test]
    fn a_page_without_its_seams_fails_the_universe() {
        let bare = page(WORKSPACE_INTEGRATIONS, 200, WS_NO_SEAM);
        assert!(
            universe_from_pages(&bare, &[], &[]).is_err(),
            "a 200 page without its seams is unreadable"
        );
        let ws = page(WORKSPACE_INTEGRATIONS, 200, WS);
        let no_routes_seam = page("/p", 200, "<p>Identity Platform</p>");
        assert!(
            universe_from_pages(&ws, &[("Identity Platform".into(), no_routes_seam)], &[]).is_err()
        );
    }

    #[test]
    fn a_declared_change_that_did_not_happen_fails() {
        let ws = page(WORKSPACE_INTEGRATIONS, 200, WS);
        let u = universe_from_pages(
            &ws,
            &[("Identity Platform".into(), page("/p", 200, ROUTES))],
            &[],
        )
        .unwrap();
        let declared = [Change::RouteStatus {
            project: "Identity Platform".into(),
            destination: "#auth-dev".into(),
        }];
        assert!(
            delta_violation(&u, &u, &declared).is_some(),
            "a declared status change that did not happen is caught"
        );
        assert!(delta_violation(&u, &u, &[Change::SlackConnection]).is_some());
        assert!(delta_violation(
            &u,
            &u,
            &[Change::Emailed {
                to: "sam.okafor@canzan.dev".into()
            }]
        )
        .is_some());
    }

    #[test]
    fn an_undeclared_route_change_fails_and_a_declared_one_passes() {
        let ws = page(WORKSPACE_INTEGRATIONS, 200, WS);
        let before = universe_from_pages(
            &ws,
            &[("Identity Platform".into(), page("/p", 200, ROUTES))],
            &[],
        )
        .unwrap();
        let paused = ROUTES.replace("Delivering", "Paused");
        let after = universe_from_pages(
            &ws,
            &[("Identity Platform".into(), page("/p", 200, &paused))],
            &[],
        )
        .unwrap();
        assert!(
            delta_violation(&before, &after, &[]).is_some(),
            "an undeclared status change is caught"
        );
        let declared = [Change::RouteStatus {
            project: "Identity Platform".into(),
            destination: "#auth-dev".into(),
        }];
        assert!(
            delta_violation(&before, &after, &declared).is_none(),
            "the declared change passes"
        );
        let mut extra = after.clone();
        extra.emails.insert("sam.okafor@canzan.dev".into(), 1);
        assert!(
            delta_violation(&before, &extra, &declared).is_some(),
            "an extra email is caught"
        );
    }
}

#[cfg(test)]
mod comment_notice_oracle_tests {
    //! The comment notice is found at every content level (D-2, D-3, K-8),
    //! and no other notice about the same issue is taken for it.
    use super::is_comment_notice;

    const OPEN: &str =
        "\nOpen in Foundry: https://foundry.example/team/backend/project/auth/issues/7";

    #[test]
    fn link_only_summary_and_comment_text_notices_are_the_comment_notice() {
        for body in [
            format!("AUTH-7 · Identity Platform · New comment{OPEN}"),
            format!(
                "AUTH-7 Move Grafana sign-in to Keycloak\nSam Okafor commented · Identity Platform{OPEN}"
            ),
            format!(
                "AUTH-7 Move Grafana sign-in to Keycloak\nSam Okafor commented · Identity Platform\nThe bind DN changed; staging is green.{OPEN}"
            ),
        ] {
            assert!(is_comment_notice(&body, "AUTH-7"), "{body}");
        }
    }

    #[test]
    fn other_notices_and_other_issues_are_not() {
        for body in [
            format!("AUTH-7 · Identity Platform · New issue{OPEN}"),
            format!("AUTH-7 Move Grafana sign-in to Keycloak\nNew issue in Identity Platform by Sam Okafor · In progress{OPEN}"),
            format!("AUTH-12 · Identity Platform · New comment{OPEN}"),
            format!("AUTH-12 Rotate LLDAP bind password\nSam Okafor commented · Identity Platform{OPEN}"),
            format!("AUTH-71 · Identity Platform · New comment{OPEN}"),
        ] {
            assert!(!is_comment_notice(&body, "AUTH-7"), "{body}");
        }
    }
}
