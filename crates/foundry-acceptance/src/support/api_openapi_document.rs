//! api-openapi-document — the acceptance driver (DISTILL 2026-10-19).
//!
//! Every step of `steps/feature_api_openapi_document.rs` is one call into this
//! module. It owns the typed vocabulary of the feature (Mandate 12, the repo's
//! Rust reading): [`Credential`], [`Operation`], [`Situation`], [`DriftPlant`],
//! [`Shape`], [`PageCall`], [`PageToken`], [`ForeignLink`], the [`Answer`] an
//! HTTP door gives, the [`Universe`] of Canzan Labs, and the oracles that judge
//! an answer by the API description alone.
//!
//! # Doors (driving ports only)
//!
//! * `GET /api/v1/openapi.json` and every `/api/v1` operation, over real HTTP
//!   against the in-process router (`InProcHarness`, real Postgres, the fixed
//!   EdDSA test key);
//! * `GET /docs/api` and its files, over HTTP and in a real browser
//!   (`@needs-browser`, `browser_harness`);
//! * the maintainer's drift check and description refresh, as subprocesses
//!   against a staged copy of the repository (`xtask check-arch --root`,
//!   `cargo test -p foundry-api --lib openapi`, `xtask openapi --root`).
//!
//! Nothing here imports a foundry-api or foundry-app type: the description is
//! read as JSON from the wire (or, for the committed copy, from disk), and the
//! running release from foundry-app's manifest, never from a production
//! constant.
//!
//! # The state the scenarios move through (C2a)
//!
//! The SUT has no state machine of its own for reads: the description is a
//! compile-time constant, so every read is a pure function of the release. The
//! writes the matrix produces move Canzan Labs through these observable
//! transitions, each a fail-closed delta over the [`Universe`]:
//!
//! ```text
//!   AUTH-1..7 seeded ──create──▶ + AUTH-8
//!   AUTH-7 (backlog) ──state───▶ AUTH-7 (in_progress | review)
//!   AUTH-7 comments  ──comment─▶ + one comment;  own comment ──edit──▶ body changed
//!   scratch-rotation (live) ──revoke──▶ revoked ──revoke again──▶ revoked (no change)
//!   any refusal (401/403/404/405/410/413/415/422/429, 400) ──▶ no change
//! ```
//!
//! # Universe (Mandate 8)
//!
//! Port-exposed rows only, read with read-only base-table SQL (the house
//! exception, project-lifecycle U2): every issue (`issue:<project>:<number>`),
//! every comment (`comment:<id>`) and every machine token (`token:<label>`,
//! minus `last_used_at`, which every authenticated call touches) of Canzan
//! Labs. Issues are read minus `position`: board ranking is server-internal
//! re-ranking of siblings (card-ranking-within-status ADR-001 R4 — a create
//! shifts its landing lane, a state change renumbers the columns it touches),
//! not a change the answer reports. A refusal's expected delta is empty; a
//! success declares the one key it adds or changes.

use crate::support::harness::{establish_session, seed_lanes_for_project, InProcHarness};
use crate::world::FoundryWorld;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// ===========================================================================
// Fixed facts
// ===========================================================================

/// D1: the description's address.
pub const DOC_PATH: &str = "/api/v1/openapi.json";
/// An address that was never part of the API: the catch-all's answer, the
/// baseline every "the API's not-found" comparison is made against.
pub const NEVER_API_PATH: &str = "/api/v1/no-such-address";
/// The API page (DESIGN DDD-15).
pub const PAGE_PATH: &str = "/docs/api";
/// The page's own files (DDD-16/17/18).
pub const PAGE_FILES: [&str; 3] = [
    "/static/vendor/swagger-ui-5.33.3/swagger-ui-bundle.js",
    "/static/vendor/swagger-ui-5.33.3/swagger-ui.css",
    "/static/js/api-docs.js",
];
/// Every file the vendored directory ships (served, not referenced by the page).
pub const VENDORED_FILES: [&str; 4] = [
    "/static/vendor/swagger-ui-5.33.3/swagger-ui-bundle.js",
    "/static/vendor/swagger-ui-5.33.3/swagger-ui.css",
    "/static/vendor/swagger-ui-5.33.3/LICENSE.txt",
    "/static/vendor/swagger-ui-5.33.3/NOTICE.txt",
];
/// The marker of every file only the API page may load (D12).
pub const PAGE_ONLY_MARKERS: [&str; 2] = ["/static/vendor/swagger-ui-", "/static/js/api-docs.js"];

/// D14 copy, byte for byte.
pub const COPY_SCHEME: &str = "A machine token: an EdDSA-signed JWT sent as `Authorization: Bearer <token>`. A workspace admin mints one, signed in to Foundry, at /admin/tokens; the value is shown once. This API lists and revokes tokens but cannot create one.";
pub const COPY_NOT_FOUND: &str = "Not found. The same answer, byte for byte, for a team, project, issue or comment that does not exist, one your token cannot reach, and a path that is not part of the API.";
pub const COPY_STATE: &str = "The lane the issue is in, by its slug. Each project has its own lanes; the defaults are backlog, todo, in_progress, done and cancelled. `in-progress` and `canceled` are accepted as aliases.";
pub const COPY_UNAUTHORIZED_CLAUSE: &str = "Also answered when the token cannot be checked.";
pub const COPY_PAGE_TITLE: &str = "Foundry API";
pub const COPY_NOSCRIPT: &str =
    "This page needs JavaScript. The OpenAPI document is at /api/v1/openapi.json.";
pub const COPY_TOKEN_LABEL: &str = "Machine token";
/// DDD-19 / ADR-API-OPENAPI-004 decision 1, exactly.
pub const PAGE_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'";

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const PASSWORD: &str = "aod-correct-horse-battery-staple-42";
const FOUNDRY_APP_MANIFEST: &str = include_str!("../../../foundry-app/Cargo.toml");
const TEAM: &str = "backend";
const PROJECT: &str = "auth";
const ISSUE_TITLES: [&str; 7] = [
    "Sign-in page flickers on Safari",
    "Password reset mail lands in spam",
    "Session cookie lacks SameSite",
    "Audit log misses revokes",
    "Rotate the signing key",
    "Rate-limit the reset form",
    "Refresh token rotation broken on Safari",
];

/// The release this checkout builds: foundry-app's `[package] version`, read
/// at compile time (the release-version-footer oracle, never a literal).
pub fn running_release() -> &'static str {
    FOUNDRY_APP_MANIFEST
        .lines()
        .find_map(|line| {
            line.strip_prefix("version = \"")
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .expect("foundry-app/Cargo.toml declares a [package] version")
}

/// The repository root this crate lives in.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/foundry-acceptance sits two levels below the workspace root")
        .to_path_buf()
}

/// `crates/foundry-api/openapi.json` under `root` (DDD-2).
pub fn committed_description_path(root: &Path) -> PathBuf {
    root.join("crates/foundry-api/openapi.json")
}

// ===========================================================================
// Typed vocabulary
// ===========================================================================

/// A credential Sven's client may send with a fetch of the description (D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credential {
    Expired,
    Forged,
    Revoked,
    Malformed,
    BrowserSession,
}

impl Credential {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "an expired machine token" => Self::Expired,
            "a forged machine token" => Self::Forged,
            "a revoked machine token" => Self::Revoked,
            "a malformed bearer value" => Self::Malformed,
            "Priya's signed-in browser session" => Self::BrowserSession,
            other => panic!("unknown credential phrase {other:?}"),
        }
    }
}

/// One documented operation (D15: the 8 API operations plus the description's
/// own), or the catch-all, which is documented as the shared not-found answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    ReadDescription,
    ListIssues,
    CreateIssue,
    ChangeState,
    ReadHistory,
    CreateComment,
    EditComment,
    ListTokens,
    RevokeToken,
    OutsideApi,
}

impl Operation {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "read the API description" => Self::ReadDescription,
            "list a project's issues" => Self::ListIssues,
            "create an issue" => Self::CreateIssue,
            "change an issue's state" => Self::ChangeState,
            "read an issue's history" => Self::ReadHistory,
            "comment on an issue" => Self::CreateComment,
            "edit a comment" => Self::EditComment,
            "list the workspace's tokens" => Self::ListTokens,
            "revoke a token" => Self::RevokeToken,
            "reach an address outside the API's operations" => Self::OutsideApi,
            other => panic!("unknown operation phrase {other:?}"),
        }
    }

    /// The (method, path template) the description keys the operation by.
    pub fn documented_as(self) -> Option<(&'static str, &'static str)> {
        const ISSUES: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}/issues";
        const ISSUE: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}";
        const HISTORY: &str =
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/history";
        const COMMENTS: &str =
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments";
        const COMMENT: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments/{comment_id}";
        const TOKENS: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}/tokens";
        const TOKEN: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}";
        Some(match self {
            Self::ReadDescription => ("get", DOC_PATH),
            Self::ListIssues => ("get", ISSUES),
            Self::CreateIssue => ("post", ISSUES),
            Self::ChangeState => ("patch", ISSUE),
            Self::ReadHistory => ("get", HISTORY),
            Self::CreateComment => ("post", COMMENTS),
            Self::EditComment => ("patch", COMMENT),
            Self::ListTokens => ("get", TOKENS),
            Self::RevokeToken => ("delete", TOKEN),
            Self::OutsideApi => return None,
        })
    }

    /// A method the operation's address does not offer (C6).
    fn other_method(self) -> reqwest::Method {
        use reqwest::Method;
        match self {
            Self::ListIssues | Self::CreateIssue => Method::PUT,
            Self::ChangeState => Method::GET,
            Self::ReadHistory => Method::POST,
            Self::CreateComment => Method::GET,
            Self::EditComment => Method::DELETE,
            Self::ListTokens => Method::PUT,
            Self::RevokeToken => Method::GET,
            Self::ReadDescription | Self::OutsideApi => Method::POST,
        }
    }

    /// The body a well-formed call sends.
    fn good_body(self) -> Option<Value> {
        match self {
            Self::CreateIssue => {
                Some(serde_json::json!({"title": "Safari drops the refresh token"}))
            }
            Self::ChangeState => Some(serde_json::json!({"state": "in_progress"})),
            Self::CreateComment => Some(serde_json::json!({"body": "Seen on Safari 18 too."})),
            Self::EditComment => Some(serde_json::json!({"body": "Seen on Safari 18 and 17."})),
            _ => None,
        }
    }

    /// The JSON field a "missing field" situation misspells.
    fn misspelt_body(self) -> Value {
        match self {
            Self::CreateIssue => serde_json::json!({"titel": "x"}),
            Self::ChangeState => serde_json::json!({"stat": "todo"}),
            _ => serde_json::json!({"bdy": "x"}),
        }
    }

    fn body_field(self) -> &'static str {
        match self {
            Self::CreateIssue => "title",
            Self::ChangeState => "state",
            _ => "body",
        }
    }
}

/// How a matrix row calls its operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situation {
    HisToken,
    NoToken,
    OpsOnlyToken,
    UnknownProject,
    OtherMethod,
    EmptyTitle,
    LongTitle,
    LongDescription,
    NotJson,
    NoJsonType,
    MissingField,
    OverTwoMb,
    UnknownIssue,
    UnknownLane,
    IssueSeven,
    AliasInProgress,
    OwnLane,
    NeverChanged,
    EmptyComment,
    LongComment,
    OwnComment,
    OthersComment,
    OtherIssueComment,
    DeletedComment,
    BadCommentId,
    MeiToken,
    CreateOne,
    BadJti,
    PastLimit,
    AlreadyRevoked,
}

impl Situation {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "with his token" => Self::HisToken,
            "with no token" => Self::NoToken,
            "with the token that reaches only Ops" => Self::OpsOnlyToken,
            "of a project that does not exist" | "in a project that does not exist" => {
                Self::UnknownProject
            }
            "in a way the address does not offer" => Self::OtherMethod,
            "with an empty title" => Self::EmptyTitle,
            "with a title of 257 characters" => Self::LongTitle,
            "with a description of 262,145 characters" => Self::LongDescription,
            "with a body that is not JSON" => Self::NotJson,
            "without saying the body is JSON" => Self::NoJsonType,
            "with a misspelt title field" | "without a state" | "without a comment body" => {
                Self::MissingField
            }
            "with a body over 2 MB" => Self::OverTwoMb,
            "of an issue that does not exist" => Self::UnknownIssue,
            "to a lane the project does not have" => Self::UnknownLane,
            "of an issue numbered \"seven\"" => Self::IssueSeven,
            "to the alias \"in-progress\"" => Self::AliasInProgress,
            "to the project's own lane \"review\"" => Self::OwnLane,
            "of an issue that never changed" => Self::NeverChanged,
            "with an empty comment" => Self::EmptyComment,
            "with a comment of 65,537 characters" => Self::LongComment,
            "of his own" => Self::OwnComment,
            "written by someone else" => Self::OthersComment,
            "of another issue" => Self::OtherIssueComment,
            "that was deleted" => Self::DeletedComment,
            "with a comment id that is not an id" => Self::BadCommentId,
            "with Mei's token" => Self::MeiToken,
            "by creating one" => Self::CreateOne,
            "with a token id that is not an id" => Self::BadJti,
            "past the revoke limit" => Self::PastLimit,
            "that is already revoked" => Self::AlreadyRevoked,
            other => panic!("unknown situation phrase {other:?}"),
        }
    }
}

/// A component the description gives a shape to (DDD-6 names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Issue,
    Comment,
    HistoryEntry,
    Token,
    Error,
    CreateIssueRequest,
    PatchIssueRequest,
    CommentBodyRequest,
}

impl Shape {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "an issue" => Self::Issue,
            "a comment" => Self::Comment,
            "an issue history entry" => Self::HistoryEntry,
            "a token" => Self::Token,
            "the error answer" => Self::Error,
            "a new issue request" => Self::CreateIssueRequest,
            "a state change request" => Self::PatchIssueRequest,
            "a comment request" => Self::CommentBodyRequest,
            other => panic!("unknown shape phrase {other:?}"),
        }
    }

    pub fn component(self) -> &'static str {
        match self {
            Self::Issue => "Issue",
            Self::Comment => "Comment",
            Self::HistoryEntry => "IssueHistoryEntry",
            Self::Token => "Token",
            Self::Error => "Error",
            Self::CreateIssueRequest => "CreateIssueRequest",
            Self::PatchIssueRequest => "PatchIssueRequest",
            Self::CommentBodyRequest => "CommentBodyRequest",
        }
    }
}

/// A drift the maintainer plants in the staged copy (US-AOD-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftPlant {
    /// `pub <field>: String` on a wire struct, constructions updated so it compiles.
    AddField {
        structure: &'static str,
        field: String,
    },
    /// A `ServiceError::Conflict` the list-issues handler can now return.
    Conflict409,
    /// A new `GET /api/v1/teams/{team_slug}/projects` route with no entry.
    NewOperation,
    /// The list-issues route removed from `routes()`.
    RemoveListIssues,
    /// Every crate's version moved to the next patch release.
    BumpRelease,
    /// The list-issues operation's description text changed, nothing else.
    Reword,
    /// The revoke operation removed from the committed description.
    UndocumentRevoke,
}

impl DriftPlant {
    pub fn add_field(field: &str, answer: &str) -> Self {
        let structure = match answer.trim() {
            "the issue answer" => "IssueJson",
            "the error answer" => "ErrorDetail",
            other => panic!("unknown answer phrase {other:?}"),
        };
        Self::AddField {
            structure,
            field: field.to_string(),
        }
    }
}

/// A call tried from the API page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageCall {
    ListIssues,
    RevokeScratch,
}

impl PageCall {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "listing Backend's \"auth\" project issues" => Self::ListIssues,
            "revoking \"scratch-rotation\"" => Self::RevokeScratch,
            other => panic!("unknown page call phrase {other:?}"),
        }
    }
}

/// The token pasted into the page for a refused call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageToken {
    Revoked,
    Expired,
    Forged,
    Mei,
    CiIssueFiler,
}

impl PageToken {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "the revoked token \"old-ci-issue-filer\"" => Self::Revoked,
            "an expired machine token" => Self::Expired,
            "a forged machine token" => Self::Forged,
            "Mei's token" => Self::Mei,
            "\"ci-issue-filer\"" => Self::CiIssueFiler,
            other => panic!("unknown page token phrase {other:?}"),
        }
    }
}

/// The setup a matrix row needs, as its own Given (so the When is one request).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precondition {
    AsBackgroundLeftIt,
    Auth7MovedToTodo,
    ReviewLane,
    SvenCommentedOnAuth7,
    MeiCommentedOnAuth7,
    SvenCommentedOnAuth6,
    SvensCommentDeleted,
    RevokeBudgetUsed,
}

impl Precondition {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "Canzan Labs is as the Background left it" => Self::AsBackgroundLeftIt,
            "AUTH-7 has moved to todo" => Self::Auth7MovedToTodo,
            "Backend's project auth has its own lane \"review\"" => Self::ReviewLane,
            "Sven has commented on AUTH-7" => Self::SvenCommentedOnAuth7,
            "Mei has commented on AUTH-7" => Self::MeiCommentedOnAuth7,
            "Sven has commented on AUTH-6" => Self::SvenCommentedOnAuth6,
            "Sven's comment on AUTH-7 was deleted" => Self::SvensCommentDeleted,
            "Sven has used his revoke budget" => Self::RevokeBudgetUsed,
            other => panic!("unknown precondition phrase {other:?}"),
        }
    }
}

/// What a foreign link names in the page's query string (D10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignLink {
    Description,
    Configuration,
}

impl ForeignLink {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "description" => Self::Description,
            "configuration" => Self::Configuration,
            other => panic!("unknown link phrase {other:?}"),
        }
    }
}

// ===========================================================================
// State
// ===========================================================================

/// A registered (or forged) machine token.
#[derive(Debug, Clone)]
pub struct Minted {
    pub jwt: String,
    pub jti: uuid::Uuid,
}

/// One HTTP answer, as the wire gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub path: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn media_type(&self) -> String {
        self.header("content-type")
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase()
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    fn summary(&self) -> String {
        let text = self.text();
        let cut: String = text.chars().take(400).collect();
        format!(
            "{} answered {} ({}): {cut}",
            self.path,
            self.status,
            self.media_type()
        )
    }
}

/// The observable rows of Canzan Labs (see module docs).
pub type Universe = BTreeMap<String, String>;

/// What a successful matrix row is allowed to change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    Nothing,
    Changes(String),
    AppearsWithPrefix(&'static str),
}

/// The staged tree and the last run of the maintainer's commands.
#[derive(Debug, Default)]
pub struct DriftState {
    pub tree: Option<PathBuf>,
    pub output: String,
    pub passed: Option<bool>,
    pub description_before: Option<Vec<u8>>,
    pub bumped_from: Option<String>,
    pub bumped_to: Option<String>,
}

/// The second origin the browser lane treats as "another site".
#[derive(Debug)]
pub struct ForeignSite {
    pub origin: String,
    pub hits: Arc<Mutex<Vec<String>>>,
    pub task: tokio::task::JoinHandle<()>,
}

impl Drop for ForeignSite {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Debug, Default)]
pub struct AodState {
    pub workspace_id: Option<uuid::Uuid>,
    pub people: BTreeMap<String, uuid::Uuid>,
    pub teams: BTreeMap<String, uuid::Uuid>,
    pub auth_project: Option<uuid::Uuid>,
    pub tokens: BTreeMap<String, Minted>,
    pub sessions: BTreeMap<String, String>,
    pub comments: BTreeMap<&'static str, uuid::Uuid>,
    pub fetched: Option<Answer>,
    pub last: Option<Answer>,
    pub last_op: Option<Operation>,
    pub pair: Option<(Answer, Answer)>,
    pub before: Option<Universe>,
    pub expected: Option<Expected>,
    pub second: Option<InProcHarness>,
    pub reading: Option<Operation>,
    pub shape: Option<Shape>,
    pub drift: DriftState,
    pub foreign: Option<ForeignSite>,
    pub fetched_files: Vec<Answer>,
    pub drift_guard: Option<tokio::sync::OwnedMutexGuard<()>>,
}

// ===========================================================================
// Harness access
// ===========================================================================

fn harness(world: &FoundryWorld) -> &InProcHarness {
    world
        .harness
        .as_ref()
        .expect("the Background started Foundry for this scenario")
}

fn base(world: &FoundryWorld) -> String {
    harness(world).base_url()
}

fn pool(world: &FoundryWorld) -> sqlx::PgPool {
    harness(world).app.state.store.pool().clone()
}

fn http(world: &FoundryWorld) -> reqwest::Client {
    world
        .http
        .clone()
        .expect("the Background built an HTTP client")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse the harness anchor")
}

async fn password_hash() -> String {
    static HASH: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
    HASH.get_or_init(|| async {
        foundry_auth::hash_password(&SecretString::new(PASSWORD.to_string().into()))
            .await
            .expect("hash the fixture password")
    })
    .await
    .clone()
}

async fn into_answer(path: &str, resp: reqwest::Response) -> Answer {
    let status = resp.status().as_u16();
    let headers = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = resp.bytes().await.map(|b| b.to_vec()).unwrap_or_default();
    Answer {
        path: path.to_string(),
        status,
        headers,
        body,
    }
}

async fn send(
    world: &FoundryWorld,
    base_url: &str,
    method: reqwest::Method,
    path: &str,
    headers: &[(&str, String)],
    body: Option<Vec<u8>>,
) -> Answer {
    let mut req = http(world).request(method.clone(), format!("{base_url}{path}"));
    for (name, value) in headers {
        req = req.header(*name, value);
    }
    if let Some(body) = body {
        req = req.body(body);
    }
    let resp = req
        .send()
        .await
        .unwrap_or_else(|e| panic!("{method} {path}: {e}"));
    into_answer(path, resp).await
}

async fn get(world: &FoundryWorld, path: &str, headers: &[(&str, String)]) -> Answer {
    let base_url = base(world);
    send(world, &base_url, reqwest::Method::GET, path, headers, None).await
}

// ===========================================================================
// Background
// ===========================================================================

async fn exec(world: &FoundryWorld, sql: &str, binds: &[uuid::Uuid], what: &str) {
    let mut q = sqlx::query(sql);
    for b in binds {
        q = q.bind(*b);
    }
    q.execute(&pool(world))
        .await
        .unwrap_or_else(|e| panic!("{what}: {e}"));
}

async fn seed_person(world: &mut FoundryWorld, name: &str, email: &str, role: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(email.to_ascii_lowercase())
    .bind(email)
    .bind(name)
    .bind(password_hash().await)
    .execute(&pool(world))
    .await
    .unwrap_or_else(|e| panic!("seed {email}: {e}"));
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(world.aod.workspace_id.expect("workspace seeded"))
    .bind(id)
    .bind(role)
    .execute(&pool(world))
    .await
    .unwrap_or_else(|e| panic!("seed membership of {email}: {e}"));
    world.aod.people.insert(email.to_string(), id);
    id
}

async fn seed_team(world: &mut FoundryWorld, name: &str, slug: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(world.aod.workspace_id.expect("workspace seeded"))
        .bind(name)
        .bind(slug)
        .execute(&pool(world))
        .await
        .unwrap_or_else(|e| panic!("seed team {name}: {e}"));
    world.aod.teams.insert(slug.to_string(), id);
    id
}

async fn seed_team_member(world: &FoundryWorld, team: uuid::Uuid, user: uuid::Uuid, role: &str) {
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, $3)")
        .bind(team)
        .bind(user)
        .bind(role)
        .execute(&pool(world))
        .await
        .expect("seed a team membership");
}

async fn seed_project(
    world: &FoundryWorld,
    team: uuid::Uuid,
    name: &str,
    slug: &str,
    key: &str,
) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(team)
    .bind(world.aod.workspace_id.expect("workspace seeded"))
    .bind(name)
    .bind(slug)
    .bind(key)
    .execute(&pool(world))
    .await
    .unwrap_or_else(|e| panic!("seed project {name}: {e}"));
    seed_lanes_for_project(&pool(world), id).await;
    id
}

/// Register (unless `forged`) and sign a machine token bound to `email`.
async fn mint(
    world: &mut FoundryWorld,
    email: &str,
    label: &str,
    scope_team: Option<uuid::Uuid>,
    exp_offset_secs: i64,
    forged: bool,
) -> Minted {
    let user = world.aod.people[email];
    let ws = world.aod.workspace_id.expect("workspace seeded");
    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::seconds(exp_offset_secs);
    if !forged {
        harness(world)
            .app
            .state
            .store
            .insert_machine_token(jti, user, ws, scope_team, exp, label, user)
            .await
            .unwrap_or_else(|e| panic!("register machine token {label}: {e:?}"));
    }
    let claims = foundry_auth::MachineTokenClaims {
        sub: user,
        scope: scope_team,
        iat: now.unix_timestamp(),
        exp: exp.unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    let jwt = foundry_auth::test_keys::signer()
        .mint(&claims)
        .expect("mint a machine token")
        .expose_secret()
        .to_string();
    let minted = Minted { jwt, jti };
    world.aod.tokens.insert(label.to_string(), minted.clone());
    minted
}

fn bearer(world: &FoundryWorld, label: &str) -> (&'static str, String) {
    let token = world
        .aod
        .tokens
        .get(label)
        .unwrap_or_else(|| panic!("no machine token {label:?} was minted in this scenario"));
    ("authorization", format!("Bearer {}", token.jwt))
}

/// "Foundry is running for the workspace "Canzan Labs", administered by Priya".
pub async fn given_foundry_running(world: &mut FoundryWorld, workspace: &str) {
    world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    world.http = Some(client());
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(workspace)
        .execute(&pool(world))
        .await
        .expect("seed the workspace");
    world.aod.workspace_id = Some(id);
    seed_person(world, "Priya Raman", "priya@canzan.dev", "admin").await;
}

/// "team Backend's project "auth" holds the issues AUTH-1 to AUTH-7": seeded
/// as rows, the issues filed through the SHIPPED create operation.
pub async fn given_backend_issues(world: &mut FoundryWorld) {
    let team = seed_team(world, "Backend", TEAM).await;
    let priya = world.aod.people["priya@canzan.dev"];
    seed_team_member(world, team, priya, "lead").await;
    let project = seed_project(world, team, "Auth", PROJECT, "AUTH").await;
    world.aod.auth_project = Some(project);
    mint(world, "priya@canzan.dev", "aod-fixture", None, 3600, false).await;
    for title in ISSUE_TITLES {
        let answer = call_raw(
            world,
            reqwest::Method::POST,
            &format!("/api/v1/teams/{TEAM}/projects/{PROJECT}/issues"),
            Some("aod-fixture"),
            Some("application/json"),
            Some(serde_json::to_vec(&serde_json::json!({ "title": title })).unwrap()),
        )
        .await;
        assert_eq!(
            answer.status,
            201,
            "filing a fixture issue through the shipped create operation: {}",
            answer.summary()
        );
    }
}

/// "Sven holds the machine token "ci-issue-filer" that Priya minted".
pub async fn given_sven_token(world: &mut FoundryWorld) {
    mint(
        world,
        "priya@canzan.dev",
        "ci-issue-filer",
        None,
        3600,
        false,
    )
    .await;
}

/// "Mei, a member of team Backend, holds her own machine token".
pub async fn given_mei_token(world: &mut FoundryWorld) {
    let mei = seed_person(world, "Mei Tanaka", "mei@canzan.dev", "member").await;
    let team = world.aod.teams[TEAM];
    seed_team_member(world, team, mei, "member").await;
    mint(world, "mei@canzan.dev", "mei-script", None, 3600, false).await;
}

/// "team Ops exists, and Sven also holds a machine token that reaches only Ops".
pub async fn given_ops_only_token(world: &mut FoundryWorld) {
    let ops = seed_team(world, "Ops", "ops").await;
    let priya = world.aod.people["priya@canzan.dev"];
    seed_team_member(world, ops, priya, "lead").await;
    mint(
        world,
        "priya@canzan.dev",
        "ops-only",
        Some(ops),
        3600,
        false,
    )
    .await;
}

/// "Canzan Labs also has the machine tokens "scratch-rotation" and the revoked
/// "old-ci-issue-filer"".
pub async fn given_scratch_and_revoked(world: &mut FoundryWorld) {
    mint(
        world,
        "priya@canzan.dev",
        "scratch-rotation",
        None,
        3600,
        false,
    )
    .await;
    let old = mint(
        world,
        "priya@canzan.dev",
        "old-ci-issue-filer",
        None,
        3600,
        false,
    )
    .await;
    harness(world)
        .app
        .state
        .store
        .revoke_machine_token(old.jti)
        .await
        .expect("revoke the old token");
}

/// "the instance also holds the team "Platform Ops" and its project "Identity Platform"".
pub async fn given_distinct_names(world: &mut FoundryWorld) {
    let team = seed_team(world, "Platform Ops", "platform-ops").await;
    seed_project(world, team, "Identity Platform", "identity-platform", "IDP").await;
}

async fn session_of(world: &mut FoundryWorld, email: &str) -> String {
    if let Some(pair) = world.aod.sessions.get(email) {
        return pair.clone();
    }
    let pair = establish_session(harness(world), &http(world), email, PASSWORD).await;
    world.aod.sessions.insert(email.to_string(), pair.clone());
    pair
}

/// "Priya is signed in to Canzan Labs as its workspace admin".
pub async fn given_priya_signed_in(world: &mut FoundryWorld) {
    session_of(world, "priya@canzan.dev").await;
}

// ===========================================================================
// US-AOD-01: fetching the description
// ===========================================================================

async fn fetch_description(world: &mut FoundryWorld, credential: Option<Credential>) -> Answer {
    let mut headers: Vec<(&str, String)> = vec![("accept", "application/json".to_string())];
    match credential {
        None => {}
        Some(Credential::Expired) => {
            let token = mint(world, "priya@canzan.dev", "aod-expired", None, -3600, false).await;
            headers.push(("authorization", format!("Bearer {}", token.jwt)));
        }
        Some(Credential::Forged) => {
            let token = mint(world, "priya@canzan.dev", "aod-forged", None, 3600, true).await;
            headers.push(("authorization", format!("Bearer {}", token.jwt)));
        }
        Some(Credential::Revoked) => {
            let token = mint(world, "priya@canzan.dev", "aod-revoked", None, 3600, false).await;
            harness(world)
                .app
                .state
                .store
                .revoke_machine_token(token.jti)
                .await
                .expect("revoke the token");
            headers.push(("authorization", format!("Bearer {}", token.jwt)));
        }
        Some(Credential::Malformed) => {
            headers.push(("authorization", "Bearer not-a-machine-token".to_string()));
        }
        Some(Credential::BrowserSession) => {
            let pair = session_of(world, "priya@canzan.dev").await;
            headers.push(("cookie", pair));
        }
    }
    get(world, DOC_PATH, &headers).await
}

pub async fn given_fetched(world: &mut FoundryWorld) {
    let answer = fetch_description(world, None).await;
    world.aod.fetched = Some(answer);
}

pub async fn when_fetches(world: &mut FoundryWorld) {
    let answer = fetch_description(world, None).await;
    world.aod.last = Some(answer);
}

pub async fn when_fetches_with(world: &mut FoundryWorld, credential: Credential) {
    let answer = fetch_description(world, Some(credential)).await;
    world.aod.last = Some(answer);
}

fn last(world: &FoundryWorld) -> &Answer {
    world
        .aod
        .last
        .as_ref()
        .expect("a When step called a door first")
}

/// The served description as JSON, failing (RED) when it is not served.
fn served_description(answer: &Answer) -> Value {
    assert_eq!(
        answer.status,
        200,
        "the API description is not served at {DOC_PATH}: {}",
        answer.summary()
    );
    assert_eq!(
        answer.media_type(),
        "application/json",
        "the API description is not JSON: {}",
        answer.summary()
    );
    serde_json::from_slice(&answer.body)
        .unwrap_or_else(|e| panic!("the API description does not parse as JSON: {e}"))
}

/// Fetch the description fresh from this scenario's instance (the oracle every
/// judgement reads).
async fn description(world: &FoundryWorld) -> Value {
    let answer = get(world, DOC_PATH, &[]).await;
    served_description(&answer)
}

pub fn then_openapi_31_titled(world: &FoundryWorld, title: &str) {
    let doc = served_description(last(world));
    assert_eq!(
        doc["openapi"],
        Value::String("3.1.0".into()),
        "the description is not OpenAPI 3.1.0 (OQ-5): openapi = {}",
        doc["openapi"]
    );
    assert_eq!(
        doc["info"]["title"],
        Value::String(title.into()),
        "info.title"
    );
}

pub fn then_describes_list_scheme_error(world: &FoundryWorld) {
    let doc = served_description(last(world));
    let (method, template) = Operation::ListIssues.documented_as().expect("documented");
    assert!(
        doc["paths"][template][method].is_object(),
        "the description does not describe listing a project's issues ({method} {template})"
    );
    let scheme = &doc["components"]["securitySchemes"]["machineToken"];
    assert_eq!(scheme["type"], "http", "machineToken is an http scheme");
    assert_eq!(
        scheme["scheme"], "bearer",
        "machineToken is a bearer scheme"
    );
    assert_eq!(
        scheme["description"],
        Value::String(COPY_SCHEME.into()),
        "the machine token's description (D14)"
    );
    assert!(
        doc["components"]["schemas"]["Error"].is_object(),
        "the description has no Error schema"
    );
}

pub async fn then_version_is_release_and_footer(world: &FoundryWorld) {
    let doc = served_description(last(world));
    let version = doc["info"]["version"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        version,
        running_release(),
        "info.version is not the running release (OQ-8)"
    );
    let page = get(world, "/sign-in", &[]).await;
    let footer = format!("Foundry v{version}");
    assert!(
        page.text().contains(&footer),
        "the sign-in page footer does not show {footer:?}"
    );
}

pub fn then_only_server_is_root(world: &FoundryWorld) {
    let doc = served_description(last(world));
    assert_eq!(
        doc["servers"],
        serde_json::json!([{ "url": "/" }]),
        "servers must be the single relative entry \"/\" (OQ-7)"
    );
}

pub fn then_equals_committed(world: &FoundryWorld) {
    let served = last(world);
    assert_eq!(served.status, 200, "{}", served.summary());
    let path = committed_description_path(&workspace_root());
    let committed = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "there is no committed API description at {}: {e}",
            path.display()
        )
    });
    assert!(
        served.body == committed,
        "the served description ({} bytes) differs from the committed file ({} bytes)",
        served.body.len(),
        committed.len()
    );
}

pub fn then_same_as_no_credential(world: &FoundryWorld) {
    let baseline = world
        .aod
        .fetched
        .as_ref()
        .expect("the Given fetched with no credential");
    served_description(baseline);
    let answer = last(world);
    assert_eq!(
        answer.status,
        200,
        "a credential changed the answer (D8): {}",
        answer.summary()
    );
    assert!(
        answer.body == baseline.body,
        "a credential changed the bytes of the description (D8)"
    );
}

pub fn then_names_nothing_of_instance(world: &FoundryWorld) {
    let answer = last(world);
    served_description(answer);
    let text = answer.text().to_ascii_lowercase();
    let host = base(world)
        .trim_start_matches("http://")
        .to_ascii_lowercase();
    let mut forbidden: Vec<String> = vec![
        "canzan".into(),
        "platform ops".into(),
        "platform-ops".into(),
        "identity platform".into(),
        "identity-platform".into(),
        "priya".into(),
        "127.0.0.1".into(),
        "localhost".into(),
        host,
    ];
    for token in world.aod.tokens.values() {
        forbidden.push(token.jwt.to_ascii_lowercase());
        forbidden.push(token.jti.to_string());
    }
    let found: Vec<&String> = forbidden
        .iter()
        .filter(|f| text.contains(f.as_str()))
        .collect();
    assert!(
        found.is_empty(),
        "the description names something of the instance serving it (D7): {found:?}"
    );
}

/// "a second Foundry of the same release runs for the workspace "Globex" with its own teams".
pub async fn given_second_instance(world: &mut FoundryWorld, workspace: &str) {
    let second = InProcHarness::spawn(now_anchor()).await;
    let pool = second.app.state.store.pool().clone();
    let ws = uuid::Uuid::now_v7();
    let team = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(ws)
        .bind(workspace)
        .execute(&pool)
        .await
        .expect("seed the second workspace");
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Ledger', 'ledger')",
    )
    .bind(team)
    .bind(ws)
    .execute(&pool)
    .await
    .expect("seed the second instance's team");
    world.aod.second = Some(second);
}

pub async fn when_fetches_both(world: &mut FoundryWorld) {
    let first = get(world, DOC_PATH, &[]).await;
    let second_base = world
        .aod
        .second
        .as_ref()
        .expect("a second instance runs")
        .base_url();
    let second = send(
        world,
        &second_base,
        reqwest::Method::GET,
        DOC_PATH,
        &[],
        None,
    )
    .await;
    world.aod.pair = Some((first, second));
}

pub fn then_same_bytes(world: &FoundryWorld) {
    let (a, b) = world.aod.pair.as_ref().expect("both instances were asked");
    served_description(a);
    served_description(b);
    assert!(
        a.body == b.body,
        "two instances on one release serve different bytes (INV-AOD-3)"
    );
}

pub async fn when_requests_address(world: &mut FoundryWorld, address: &str) {
    let answer = get(world, address, &[]).await;
    world.aod.last = Some(answer);
}

pub async fn when_sends_write(world: &mut FoundryWorld, write: &str) {
    let method = match write.trim() {
        "a POST" => reqwest::Method::POST,
        "a PUT" => reqwest::Method::PUT,
        "a PATCH" => reqwest::Method::PATCH,
        "a DELETE" => reqwest::Method::DELETE,
        other => panic!("unknown write phrase {other:?}"),
    };
    let base_url = base(world);
    let answer = send(
        world,
        &base_url,
        method,
        DOC_PATH,
        &[("content-type", "application/json".to_string())],
        Some(b"{}".to_vec()),
    )
    .await;
    world.aod.last = Some(answer);
}

pub async fn then_api_not_found(world: &FoundryWorld, no_allow: bool) {
    let never = get(world, NEVER_API_PATH, &[]).await;
    let answer = last(world);
    assert_eq!(
        (answer.status, answer.body.clone(), answer.media_type()),
        (never.status, never.body.clone(), never.media_type()),
        "{} does not answer the API's not-found: {}",
        answer.path,
        answer.summary()
    );
    if no_allow {
        assert!(
            answer.header("allow").is_none(),
            "the not-found answer names allowed methods (Changed Assumption 1): {:?}",
            answer.header("allow")
        );
    }
}

pub async fn then_still_served(world: &FoundryWorld) {
    description(world).await;
}

fn effective_security(doc: &Value, op: &Value) -> Value {
    op.get("security")
        .cloned()
        .unwrap_or_else(|| doc.get("security").cloned().unwrap_or(Value::Null))
}

pub fn then_reading_needs_no_credential(world: &FoundryWorld) {
    let doc = served_description(last(world));
    let op = &doc["paths"][DOC_PATH]["get"];
    assert!(
        op.is_object(),
        "the description does not describe itself (OQ-10)"
    );
    assert_eq!(
        effective_security(&doc, op),
        serde_json::json!([]),
        "reading the description must need no credential (security: [])"
    );
}

pub fn then_others_need_machine_token(world: &FoundryWorld) {
    let doc = served_description(last(world));
    let mut checked = 0;
    for (path, method, op) in operations(&doc) {
        if path == DOC_PATH {
            continue;
        }
        let security = effective_security(&doc, op);
        let asks = security
            .as_array()
            .map(|reqs| reqs.iter().any(|r| r.get("machineToken").is_some()))
            .unwrap_or(false);
        assert!(asks, "{method} {path} does not ask for the machine token");
        checked += 1;
    }
    assert!(
        checked >= 1,
        "the description lists no API operation besides itself"
    );
}

// ===========================================================================
// Reading the description as a document
// ===========================================================================

const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

fn operations(doc: &Value) -> Vec<(String, String, &Value)> {
    let mut out = Vec::new();
    if let Some(paths) = doc["paths"].as_object() {
        for (path, item) in paths {
            for method in METHODS {
                if let Some(op) = item.get(method) {
                    out.push((path.clone(), method.to_string(), op));
                }
            }
        }
    }
    out
}

fn resolve<'a>(doc: &'a Value, node: &'a Value) -> &'a Value {
    let mut current = node;
    for _ in 0..8 {
        let Some(reference) = current.get("$ref").and_then(Value::as_str) else {
            return current;
        };
        let pointer = reference
            .strip_prefix('#')
            .unwrap_or_else(|| panic!("only local references are expected: {reference}"));
        current = doc.pointer(pointer).unwrap_or_else(|| {
            panic!("the description's reference {reference} resolves to nothing")
        });
    }
    panic!("a reference chain in the description is longer than 8 hops")
}

fn documented_op(doc: &Value, operation: Operation) -> &Value {
    let (method, template) = operation
        .documented_as()
        .expect("the catch-all is not an operation");
    let op = &doc["paths"][template][method];
    assert!(
        op.is_object(),
        "the description does not describe {method} {template}"
    );
    op
}

/// The documented response for `status` (the shared `NotFound` for the catch-all).
fn documented_response(doc: &Value, operation: Operation, status: u16) -> &Value {
    if operation == Operation::OutsideApi {
        let shared = &doc["components"]["responses"]["NotFound"];
        assert!(
            shared.is_object(),
            "the description has no shared NotFound answer for addresses outside the API (DDD-8)"
        );
        return shared;
    }
    let op = documented_op(doc, operation);
    let raw = op["responses"].get(status.to_string()).unwrap_or_else(|| {
        panic!(
            "the description does not list {status} for {:?}; it lists {:?}",
            operation,
            op["responses"]
                .as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>())
        )
    });
    resolve(doc, raw)
}

pub async fn when_reads_operation(world: &mut FoundryWorld, operation: Operation) {
    world.aod.reading = Some(operation);
    world.aod.last = Some(get(world, DOC_PATH, &[]).await);
}

pub async fn when_reads_description(world: &mut FoundryWorld) {
    world.aod.reading = None;
    world.aod.last = Some(get(world, DOC_PATH, &[]).await);
}

fn reading(world: &FoundryWorld) -> Operation {
    world
        .aod
        .reading
        .expect("a When chose an operation to read")
}

fn parse_list(words: &str) -> BTreeSet<String> {
    words
        .split(',')
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty() && w != "none")
        .collect()
}

pub fn then_lists_exactly(world: &FoundryWorld, answers: &str) {
    let doc = served_description(last(world));
    let op = documented_op(&doc, reading(world));
    let listed: BTreeSet<String> = op["responses"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    assert_eq!(
        listed,
        parse_list(answers),
        "the answers listed for {:?} (DESIGN Statuses table)",
        reading(world)
    );
}

fn error_codes(response: &Value) -> BTreeSet<String> {
    response["content"]["application/json"]["examples"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

pub fn then_codes(world: &FoundryWorld, status: u16, codes: &str) {
    let doc = served_description(last(world));
    let response = documented_response(&doc, reading(world), status);
    assert_eq!(
        error_codes(response),
        parse_list(codes),
        "the error codes of {:?}'s {status} answer (DDD-7: examples keyed by code)",
        reading(world)
    );
    if let Some(examples) = response["content"]["application/json"]["examples"].as_object() {
        for (code, example) in examples {
            assert_eq!(
                example["value"]["error"]["code"],
                Value::String(code.clone()),
                "the example keyed {code} carries another code"
            );
        }
    }
}

pub fn then_no_409_no_invalid_json(world: &FoundryWorld) {
    let doc = served_description(last(world));
    for (path, method, op) in operations(&doc) {
        assert!(
            op["responses"].get("409").is_none(),
            "{method} {path} lists a 409 no use-case can answer"
        );
        if let Some(responses) = op["responses"].as_object() {
            for (status, raw) in responses {
                let codes = error_codes(resolve(&doc, raw));
                assert!(
                    !codes.contains("invalid_json"),
                    "{method} {path} {status} carries the code invalid_json (C4)"
                );
            }
        }
    }
}

pub async fn when_reads_shape(world: &mut FoundryWorld, shape: Shape) {
    world.aod.shape = Some(shape);
    world.aod.last = Some(get(world, DOC_PATH, &[]).await);
}

fn shape_schema(world: &FoundryWorld) -> (Value, Value) {
    let doc = served_description(last(world));
    let shape = world.aod.shape.expect("a When chose a shape");
    let schema = doc["components"]["schemas"][shape.component()].clone();
    assert!(
        schema.is_object(),
        "the description has no {} schema (DDD-6)",
        shape.component()
    );
    let resolved = resolve(&doc, &schema).clone();
    (doc, resolved)
}

fn type_set(schema: &Value) -> BTreeSet<String> {
    match &schema["type"] {
        Value::String(t) => [t.clone()].into(),
        Value::Array(ts) => ts
            .iter()
            .filter_map(|t| t.as_str().map(str::to_string))
            .collect(),
        _ => BTreeSet::new(),
    }
}

pub fn then_fields(world: &FoundryWorld, fields: &str) {
    let (_, schema) = shape_schema(world);
    let props: BTreeSet<String> = schema["properties"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    assert_eq!(
        props,
        parse_list(fields),
        "the fields of {:?}",
        world.aod.shape
    );
}

pub fn then_required(world: &FoundryWorld, required: &str) {
    let (_, schema) = shape_schema(world);
    let listed: BTreeSet<String> = schema["required"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        listed,
        parse_list(required),
        "the required fields of {:?}",
        world.aod.shape
    );
}

pub fn then_nullable(world: &FoundryWorld, nullable: &str) {
    let (doc, schema) = shape_schema(world);
    let mut found = BTreeSet::new();
    if let Some(props) = schema["properties"].as_object() {
        for (name, prop) in props {
            if type_set(resolve(&doc, prop)).contains("null") {
                found.insert(name.clone());
            }
        }
    }
    assert_eq!(
        found,
        parse_list(nullable),
        "the nullable fields of {:?}",
        world.aod.shape
    );
}

pub fn then_no_token_value(world: &FoundryWorld) {
    let (doc, schema) = shape_schema(world);
    let props = schema["properties"]
        .as_object()
        .expect("the token shape has fields");
    assert!(!props.is_empty(), "the token shape has no fields");
    for name in props.keys() {
        let lower = name.to_ascii_lowercase();
        assert!(
            lower != "token"
                && !lower.contains("value")
                && !lower.contains("secret")
                && !lower.contains("hash")
                && !lower.contains("jwt"),
            "the token shape has a field that could hold a token's value: {name}"
        );
    }
    // The whole Token schema and both token operations (descriptions and
    // examples included) carry no JWT: none of this scenario's minted tokens,
    // and nothing with a JWT's `eyJ` prefix.
    let mut scanned = schema.to_string();
    for operation in [Operation::ListTokens, Operation::RevokeToken] {
        scanned.push_str(&documented_op(&doc, operation).to_string());
    }
    let mut forbidden: Vec<String> = vec!["eyJ".to_string()];
    for token in world.aod.tokens.values() {
        forbidden.push(token.jwt.clone());
    }
    let found: Vec<&String> = forbidden
        .iter()
        .filter(|f| scanned.contains(f.as_str()))
        .collect();
    assert!(
        found.is_empty(),
        "the description's token shape or operations carry a token value: {found:?}"
    );
}

pub async fn when_reads_state(world: &mut FoundryWorld) {
    world.aod.shape = Some(Shape::Issue);
    world.aod.last = Some(get(world, DOC_PATH, &[]).await);
}

fn state_schema(world: &FoundryWorld) -> Value {
    let (doc, schema) = shape_schema(world);
    resolve(&doc, &schema["properties"]["state"]).clone()
}

pub fn then_state_open_text(world: &FoundryWorld) {
    let state = state_schema(world);
    assert_eq!(
        type_set(&state),
        ["string".to_string()].into(),
        "state is a string"
    );
    assert!(
        state.get("enum").is_none() && state.get("const").is_none(),
        "state is a closed list (C3): {state}"
    );
}

pub fn then_state_reads(world: &FoundryWorld, copy: &str) {
    let state = state_schema(world);
    assert_eq!(
        state["description"],
        Value::String(copy.into()),
        "state's description (D14)"
    );
}

fn media_types(response: &Value) -> BTreeSet<String> {
    response["content"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

pub fn then_create_plain_text(world: &FoundryWorld) {
    let doc = served_description(last(world));
    for status in [400, 413, 415] {
        let response = documented_response(&doc, Operation::CreateIssue, status);
        assert_eq!(
            media_types(response),
            ["text/plain".to_string()].into(),
            "creating an issue's {status} answer has a plain-text body (C4, C12)"
        );
    }
    let unprocessable = documented_response(&doc, Operation::CreateIssue, 422);
    assert!(
        media_types(unprocessable).contains("text/plain"),
        "creating an issue's 422 lists a plain-text body (C4)"
    );
}

pub fn then_create_envelope_422(world: &FoundryWorld) {
    let doc = served_description(last(world));
    let unprocessable = documented_response(&doc, Operation::CreateIssue, 422);
    assert!(
        media_types(unprocessable).contains("application/json"),
        "creating an issue's 422 lists the error answer"
    );
    assert_eq!(
        error_codes(unprocessable),
        parse_list("title_required, description_too_long"),
        "the codes of creating an issue's 422 error answer"
    );
}

pub fn then_every_404_uniform(world: &FoundryWorld, description: Option<&str>) {
    let doc = served_description(last(world));
    let expected = serde_json::json!({"error": {"code": "not_found", "message": "not found"}});
    let mut checked = 0;
    for (path, method, op) in operations(&doc) {
        let raw = op["responses"]
            .get("404")
            .unwrap_or_else(|| panic!("{method} {path} lists no 404"));
        let response = resolve(&doc, raw);
        assert_eq!(
            response["content"]["application/json"]["examples"]["not_found"]["value"], expected,
            "{method} {path}'s 404 body is not the not_found error answer"
        );
        if let Some(text) = description {
            assert_eq!(
                response["description"],
                Value::String(text.into()),
                "{method} {path}'s 404 description (D14)"
            );
        }
        checked += 1;
    }
    assert!(checked >= 1, "the description lists no operation");
}

pub fn then_401_once(world: &FoundryWorld, clause: &str) {
    let doc = served_description(last(world));
    let mut targets = BTreeSet::new();
    for (path, method, op) in operations(&doc) {
        if path == DOC_PATH {
            continue;
        }
        let raw = op["responses"]
            .get("401")
            .unwrap_or_else(|| panic!("{method} {path} lists no 401"));
        let reference = raw.get("$ref").and_then(Value::as_str).unwrap_or_else(|| {
            panic!("{method} {path}'s 401 is written out, not the shared answer")
        });
        targets.insert(reference.to_string());
    }
    assert_eq!(
        targets.len(),
        1,
        "the 401 is described more than once: {targets:?}"
    );
    let shared = resolve(
        &doc,
        &serde_json::json!({ "$ref": targets.iter().next().expect("one target") }),
    )
    .clone();
    let text = shared["description"].as_str().unwrap_or_default();
    assert!(
        text.contains(clause),
        "the 401 description lacks {clause:?} (C13): {text:?}"
    );
}

pub async fn when_reads_one_issue_address(world: &mut FoundryWorld) {
    world.aod.reading = Some(Operation::ChangeState);
    world.aod.last = Some(get(world, DOC_PATH, &[]).await);
}

pub fn then_only_change_state(world: &FoundryWorld) {
    let doc = served_description(last(world));
    let (_, template) = Operation::ChangeState.documented_as().expect("documented");
    let item = &doc["paths"][template];
    let offered: Vec<&str> = METHODS
        .iter()
        .copied()
        .filter(|m| item.get(*m).is_some())
        .collect();
    assert_eq!(
        offered,
        vec!["patch"],
        "the one-issue address offers {offered:?}"
    );
}

pub fn then_405_with_allow(world: &FoundryWorld) {
    let doc = served_description(last(world));
    let response = documented_response(&doc, reading(world), 405);
    assert!(
        media_types(response).is_empty(),
        "the 405 answer has no body (C6)"
    );
    assert!(
        response["headers"]
            .as_object()
            .map(|h| h.keys().any(|k| k.eq_ignore_ascii_case("allow")))
            .unwrap_or(false),
        "the 405 answer documents no Allow header (DDD-7)"
    );
}

pub async fn when_looks_for_mint(world: &mut FoundryWorld) {
    world.aod.reading = Some(Operation::ListTokens);
    world.aod.last = Some(get(world, DOC_PATH, &[]).await);
}

pub fn then_no_mint_operation(world: &FoundryWorld) {
    let doc = served_description(last(world));
    for (path, method, _) in operations(&doc) {
        assert!(
            !(method == "post" && path.contains("/tokens")),
            "the description offers a way to create a token: {method} {path} (D9)"
        );
    }
    let response = documented_response(&doc, Operation::ListTokens, 405);
    assert!(
        media_types(response).is_empty(),
        "the tokens address's 405 has no body"
    );
}

pub fn then_scheme_reads(world: &FoundryWorld, copy: &str) {
    let doc = served_description(last(world));
    let scheme = &doc["components"]["securitySchemes"]["machineToken"];
    assert_eq!(
        scheme["description"],
        Value::String(copy.into()),
        "machineToken (D14)"
    );
    assert_eq!(
        scheme["bearerFormat"], "JWT",
        "machineToken's bearer format"
    );
}

pub async fn when_opens_mint_address(world: &mut FoundryWorld) {
    let doc = description(world).await;
    let text = doc["components"]["securitySchemes"]["machineToken"]["description"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let address = text
        .split_whitespace()
        .map(|w| w.trim_end_matches([';', '.', ',']))
        .find(|w| w.starts_with("/admin/"))
        .unwrap_or_else(|| panic!("the machine token's description names no address: {text:?}"))
        .to_string();
    let pair = session_of(world, "priya@canzan.dev").await;
    let answer = get(world, &address, &[("cookie", pair)]).await;
    world.aod.last = Some(answer);
}

pub fn then_mint_page(world: &FoundryWorld) {
    let answer = last(world);
    assert_eq!(answer.status, 200, "{}", answer.summary());
    let text = answer.text();
    assert!(
        text.contains("data-mint-form") && text.contains("name=\"label\""),
        "{} is not the page where machine tokens are minted",
        answer.path
    );
}

// ===========================================================================
// The real-request matrix
// ===========================================================================

async fn call_raw(
    world: &FoundryWorld,
    method: reqwest::Method,
    path: &str,
    token: Option<&str>,
    content_type: Option<&str>,
    body: Option<Vec<u8>>,
) -> Answer {
    let mut headers: Vec<(&str, String)> = vec![("accept", "application/json".to_string())];
    if let Some(label) = token {
        headers.push(bearer(world, label));
    }
    if let Some(ct) = content_type {
        headers.push(("content-type", ct.to_string()));
    }
    let base_url = base(world);
    send(world, &base_url, method, path, &headers, body).await
}

fn issue_path(project: &str, number: &str) -> String {
    format!("/api/v1/teams/{TEAM}/projects/{project}/issues/{number}")
}

async fn seed_comment(
    world: &mut FoundryWorld,
    key: &'static str,
    label: &str,
    number: i32,
) -> uuid::Uuid {
    if let Some(id) = world.aod.comments.get(key) {
        return *id;
    }
    let answer = call_raw(
        world,
        reqwest::Method::POST,
        &format!("{}/comments", issue_path(PROJECT, &number.to_string())),
        Some(label),
        Some("application/json"),
        Some(
            serde_json::to_vec(&serde_json::json!({"body": format!("fixture comment {key}")}))
                .unwrap(),
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        201,
        "seeding a comment: {}",
        answer.summary()
    );
    let body: Value = serde_json::from_slice(&answer.body).expect("comment JSON");
    let id: uuid::Uuid = body["id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .expect("the comment answer carries its id");
    world.aod.comments.insert(key, id);
    id
}

async fn seed_review_lane(world: &FoundryWorld) {
    let project = world.aod.auth_project.expect("auth seeded");
    sqlx::query(
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
         SELECT gen_random_uuid(), p.id, p.workspace_id, 'review', 'Review',
                (SELECT COALESCE(MAX(position), -1) + 1 FROM lanes WHERE project_id = p.id)
           FROM projects p WHERE p.id = $1",
    )
    .bind(project)
    .execute(&pool(world))
    .await
    .expect("seed the review lane");
}

/// Spend the revoke budget of the principal behind `ci-issue-filer`.
async fn spend_revoke_budget(world: &mut FoundryWorld) {
    let capacity = 20;
    let mut spare = Vec::new();
    for i in 0..capacity {
        spare.push(
            mint(
                world,
                "priya@canzan.dev",
                &format!("aod-burst-{i}"),
                None,
                3600,
                false,
            )
            .await,
        );
    }
    for token in spare {
        let answer = call_raw(
            world,
            reqwest::Method::DELETE,
            &format!(
                "/api/v1/teams/{TEAM}/projects/{PROJECT}/tokens/{}",
                token.jti
            ),
            Some("ci-issue-filer"),
            None,
            None,
        )
        .await;
        assert!(
            answer.status == 204 || answer.status == 429,
            "spending the revoke budget: {}",
            answer.summary()
        );
    }
}

/// Read the universe of Canzan Labs (see module docs).
pub async fn universe(world: &FoundryWorld) -> Universe {
    let ws = world.aod.workspace_id.expect("workspace seeded");
    let rows: Vec<(String, String)> = sqlx::query_as(
        // `- 'position'`: sibling re-ranking (card-ranking-within-status
        // ADR-001 R4) is not a change the answer reports; see module docs.
        "SELECT 'issue:' || p.slug || ':' || i.number, (to_jsonb(i.*) - 'position')::text
           FROM issues i JOIN projects p ON p.id = i.project_id WHERE i.workspace_id = $1
         UNION ALL
         SELECT 'comment:' || c.id::text, to_jsonb(c.*)::text FROM comments c WHERE c.workspace_id = $1
         UNION ALL
         SELECT 'token:' || t.label, (to_jsonb(t.*) - 'last_used_at')::text
           FROM machine_tokens t WHERE t.workspace_id = $1",
    )
    .bind(ws)
    .fetch_all(&pool(world))
    .await
    .expect("read the universe of Canzan Labs");
    rows.into_iter().collect()
}

/// "Given <precondition>": a matrix row's setup, through the shipped doors where
/// one exists (the API for moves and comments), plain rows otherwise (the
/// review lane; a comment's deleted marker, house exception U2).
pub async fn given_precondition(world: &mut FoundryWorld, precondition: Precondition) {
    use Precondition as P;
    match precondition {
        P::AsBackgroundLeftIt => {}
        P::Auth7MovedToTodo => {
            let moved = call_raw(
                world,
                reqwest::Method::PATCH,
                &issue_path(PROJECT, "7"),
                Some("ci-issue-filer"),
                Some("application/json"),
                Some(br#"{"state":"todo"}"#.to_vec()),
            )
            .await;
            assert_eq!(
                moved.status,
                200,
                "moving AUTH-7 to todo: {}",
                moved.summary()
            );
        }
        P::ReviewLane => seed_review_lane(world).await,
        P::SvenCommentedOnAuth7 => {
            seed_comment(world, "own", "ci-issue-filer", 7).await;
        }
        P::MeiCommentedOnAuth7 => {
            seed_comment(world, "mei", "mei-script", 7).await;
        }
        P::SvenCommentedOnAuth6 => {
            seed_comment(world, "elsewhere", "ci-issue-filer", 6).await;
        }
        P::SvensCommentDeleted => {
            let gone = seed_comment(world, "deleted", "ci-issue-filer", 7).await;
            exec(
                world,
                "UPDATE comments SET deleted_at = now() WHERE id = $1",
                &[gone],
                "delete the comment",
            )
            .await;
        }
        P::RevokeBudgetUsed => spend_revoke_budget(world).await,
    }
}

/// "Sven tries to <operation> <situation>": snapshot the universe, send one request.
pub async fn when_tries(world: &mut FoundryWorld, operation: Operation, situation: Situation) {
    use reqwest::Method;
    use Situation as S;
    let mut project = PROJECT.to_string();
    let mut number = "7".to_string();
    let mut token: Option<&str> = Some("ci-issue-filer");
    let mut content_type: Option<&str> = Some("application/json");
    let mut body = operation.good_body();
    let mut raw_body: Option<Vec<u8>> = None;
    let mut method_override: Option<Method> = None;
    let mut comment_id: Option<String> = None;
    let mut jti: Option<String> = None;
    let mut expected = Expected::Nothing;

    // Every row's setup ran in its own Given (`given_precondition`); this When
    // only reads what those Givens recorded, snapshots, and sends one request.
    if matches!(operation, Operation::EditComment) {
        let key = match situation {
            S::OthersComment => "mei",
            S::OtherIssueComment => "elsewhere",
            S::DeletedComment => "deleted",
            _ => "own",
        };
        let id = world.aod.comments.get(key).unwrap_or_else(|| {
            panic!("the row's Given did not leave a {key:?} comment (precondition missing)")
        });
        comment_id = Some(id.to_string());
    }
    if matches!(operation, Operation::RevokeToken) {
        let scratch = world
            .aod
            .tokens
            .get("scratch-rotation")
            .expect("the Given seeded scratch-rotation")
            .jti;
        jti = Some(scratch.to_string());
    }
    match situation {
        S::HisToken => {}
        S::NoToken => token = None,
        S::OpsOnlyToken => token = Some("ops-only"),
        S::UnknownProject => project = "nowhere".to_string(),
        S::OtherMethod => method_override = Some(operation.other_method()),
        S::EmptyTitle => body = Some(serde_json::json!({"title": "   "})),
        S::LongTitle => body = Some(serde_json::json!({"title": "t".repeat(257)})),
        S::LongDescription => {
            body =
                Some(serde_json::json!({"title": "Too long", "description": "d".repeat(262_145)}))
        }
        S::NotJson => raw_body = Some(br#"{"title": "#.to_vec()),
        S::NoJsonType => content_type = None,
        S::MissingField => body = Some(operation.misspelt_body()),
        S::OverTwoMb => {
            let field = operation.body_field();
            let filler = "x".repeat(2 * 1024 * 1024 + 1);
            raw_body = Some(format!("{{\"{field}\": \"{filler}\"}}").into_bytes());
        }
        S::UnknownIssue => number = "999".to_string(),
        S::UnknownLane => body = Some(serde_json::json!({"state": "qa"})),
        S::IssueSeven => number = "seven".to_string(),
        S::AliasInProgress => body = Some(serde_json::json!({"state": "in-progress"})),
        S::OwnLane => body = Some(serde_json::json!({"state": "review"})),
        S::NeverChanged => number = "1".to_string(),
        S::EmptyComment => body = Some(serde_json::json!({"body": "   "})),
        S::LongComment => body = Some(serde_json::json!({"body": "c".repeat(65_537)})),
        S::OwnComment => {}
        S::OthersComment | S::OtherIssueComment | S::DeletedComment => {}
        S::BadCommentId => comment_id = Some("not-an-id".to_string()),
        S::MeiToken => token = Some("mei-script"),
        S::CreateOne => {
            method_override = Some(Method::POST);
            body = Some(serde_json::json!({"label": "minted-by-a-script"}));
        }
        S::BadJti => jti = Some("not-an-id".to_string()),
        S::PastLimit => {}
        S::AlreadyRevoked => {
            let revoked = world.aod.tokens["old-ci-issue-filer"].jti;
            jti = Some(revoked.to_string());
        }
    }
    let (default_method, path) = match operation {
        Operation::ReadDescription => (Method::GET, DOC_PATH.to_string()),
        Operation::ListIssues => (
            Method::GET,
            format!("/api/v1/teams/{TEAM}/projects/{project}/issues"),
        ),
        Operation::CreateIssue => (
            Method::POST,
            format!("/api/v1/teams/{TEAM}/projects/{project}/issues"),
        ),
        Operation::ChangeState => (Method::PATCH, issue_path(&project, &number)),
        Operation::ReadHistory => (
            Method::GET,
            format!("{}/history", issue_path(&project, &number)),
        ),
        Operation::CreateComment => (
            Method::POST,
            format!("{}/comments", issue_path(&project, &number)),
        ),
        Operation::EditComment => (
            Method::PATCH,
            format!(
                "{}/comments/{}",
                issue_path(&project, &number),
                comment_id.clone().expect("a comment to edit")
            ),
        ),
        Operation::ListTokens => (
            Method::GET,
            format!("/api/v1/teams/{TEAM}/projects/{project}/tokens"),
        ),
        Operation::RevokeToken => (
            Method::DELETE,
            format!(
                "/api/v1/teams/{TEAM}/projects/{project}/tokens/{}",
                jti.clone().expect("a token to revoke")
            ),
        ),
        Operation::OutsideApi => (Method::GET, format!("/api/v1/teams/{TEAM}/projects")),
    };
    let method = method_override.unwrap_or(default_method);
    let sends_body = matches!(method, Method::POST | Method::PATCH | Method::PUT);
    let payload = if !sends_body {
        None
    } else if let Some(raw) = raw_body {
        Some(raw)
    } else {
        body.map(|b| serde_json::to_vec(&b).expect("encode body"))
    };
    let content_type = if payload.is_some() {
        content_type
    } else {
        None
    };

    // The success rows' declared delta.
    let succeeds = matches!(
        (operation, situation),
        (
            _,
            S::HisToken
                | S::AliasInProgress
                | S::OwnLane
                | S::NeverChanged
                | S::OwnComment
                | S::AlreadyRevoked
        )
    );
    if succeeds {
        expected = match (operation, situation) {
            (Operation::CreateIssue, _) => Expected::Changes(format!("issue:{PROJECT}:8")),
            (Operation::ChangeState, _) => Expected::Changes(format!("issue:{PROJECT}:7")),
            (Operation::CreateComment, _) => Expected::AppearsWithPrefix("comment:"),
            (Operation::EditComment, _) => Expected::Changes(format!(
                "comment:{}",
                comment_id.clone().expect("the edited comment")
            )),
            (Operation::RevokeToken, S::HisToken) => {
                Expected::Changes("token:scratch-rotation".to_string())
            }
            _ => Expected::Nothing,
        };
    }

    world.aod.before = Some(universe(world).await);
    world.aod.expected = Some(expected);
    world.aod.last_op = Some(operation);
    let answer = call_raw(world, method, &path, token, content_type, payload).await;
    world.aod.last = Some(answer);
}

/// Validate `value` against one documented schema with the `jsonschema` crate
/// (DESIGN DDD-12 / Test Seams: 0.59, default features off, so no network
/// resolver; an unresolvable `$ref` is an error, never a fetch). The schema is
/// compiled as draft 2020-12, the OAS 3.1 dialect's base, with the document's
/// `components` carried in the same root resource so every
/// `#/components/schemas/…` reference resolves locally.
fn conforms(doc: &Value, schema: &Value, value: &Value) -> Result<(), String> {
    let mut root = schema.clone();
    let object = root
        .as_object_mut()
        .ok_or_else(|| format!("the documented schema is not an object: {schema}"))?;
    object.insert("components".to_string(), doc["components"].clone());
    let validator = jsonschema::draft202012::new(&root)
        .map_err(|e| format!("the documented schema does not compile: {e}"))?;
    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// Judge an answer by the description alone.
fn judge(doc: &Value, operation: Operation, answer: &Answer) {
    let response = documented_response(doc, operation, answer.status);
    let content = response["content"].as_object();
    match content {
        None => assert!(
            answer.body.is_empty(),
            "the description gives {:?}'s {} no body, the server sent one: {}",
            operation,
            answer.status,
            answer.summary()
        ),
        Some(media) if media.is_empty() => assert!(answer.body.is_empty(), "{}", answer.summary()),
        Some(media) => {
            let kind = answer.media_type();
            let entry = media.get(&kind).unwrap_or_else(|| {
                panic!(
                    "the description gives {:?}'s {} the bodies {:?}, the server sent {kind}: {}",
                    operation,
                    answer.status,
                    media.keys().collect::<Vec<_>>(),
                    answer.summary()
                )
            });
            if kind == "application/json" {
                let value: Value = serde_json::from_slice(&answer.body).unwrap_or_else(|e| {
                    panic!("the JSON answer does not parse ({e}): {}", answer.summary())
                });
                if let Some(schema) = entry.get("schema") {
                    if let Err(why) = conforms(doc, schema, &value) {
                        panic!(
                            "{:?}'s {} answer does not match the description: {why}\n{}",
                            operation,
                            answer.status,
                            answer.summary()
                        );
                    }
                }
                if let Some(examples) = entry.get("examples").and_then(Value::as_object) {
                    let code = value["error"]["code"].as_str().unwrap_or_default();
                    let example = examples.get(code).unwrap_or_else(|| {
                        panic!(
                            "{:?}'s {} answer carries the code {code:?}, which the description does not list ({:?})",
                            operation,
                            answer.status,
                            examples.keys().collect::<Vec<_>>()
                        )
                    });
                    assert_eq!(
                        example["value"], value,
                        "the {code} answer differs from the description's example"
                    );
                }
            }
        }
    }
    if let Some(headers) = response["headers"].as_object() {
        for name in headers.keys() {
            assert!(
                answer.header(name).is_some(),
                "the description promises the header {name} on {:?}'s {}; the server sent none",
                operation,
                answer.status
            );
        }
    }
}

pub async fn then_gets_as_documented(world: &FoundryWorld, status: u16) {
    let answer = last(world).clone();
    assert_eq!(answer.status, status, "{}", answer.summary());
    let doc = description(world).await;
    let operation = world.aod.last_op.expect("a matrix When ran");
    judge(&doc, operation, &answer);
}

pub async fn then_nothing_changed(world: &FoundryWorld) {
    let before = world
        .aod
        .before
        .clone()
        .expect("the When snapshotted the universe");
    let after = universe(world).await;
    let changed = diff(&before, &after);
    assert!(
        changed.is_empty(),
        "a refused call changed Canzan Labs: {changed:?}"
    );
}

pub async fn then_changed_only_as_reported(world: &FoundryWorld) {
    let before = world
        .aod
        .before
        .clone()
        .expect("the When snapshotted the universe");
    let after = universe(world).await;
    let changed = diff(&before, &after);
    match world
        .aod
        .expected
        .clone()
        .expect("the When declared its delta")
    {
        Expected::Nothing => assert!(
            changed.is_empty(),
            "a read changed Canzan Labs: {changed:?}"
        ),
        Expected::Changes(key) => assert_eq!(
            changed,
            [key.clone()].into(),
            "Canzan Labs changed otherwise than {key}"
        ),
        Expected::AppearsWithPrefix(prefix) => {
            assert_eq!(changed.len(), 1, "exactly one row changes: {changed:?}");
            let key = changed.iter().next().expect("one");
            assert!(
                key.starts_with(prefix) && !before.contains_key(key),
                "the one change is a new {prefix} row: {key}"
            );
        }
    }
}

fn diff(before: &Universe, after: &Universe) -> BTreeSet<String> {
    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    keys.into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .cloned()
        .collect()
}

// ===========================================================================
// The maintainer's drift check (subprocesses on a staged tree)
// ===========================================================================

/// One drift scenario at a time touches the staged tree: the Background takes
/// this lock and the guard lives in the World until the scenario ends, so its
/// Then steps read the tree under the lock too. Steps never re-lock (the tokio
/// mutex is not re-entrant).
static DRIFT_LOCK: std::sync::LazyLock<Arc<tokio::sync::Mutex<()>>> =
    std::sync::LazyLock::new(|| Arc::new(tokio::sync::Mutex::new(())));

fn drift_dir() -> PathBuf {
    workspace_root().join("target").join("aod-drift")
}

/// "the maintainer works in a copy of the current repository".
pub async fn given_tree_copy(world: &mut FoundryWorld) {
    let guard = DRIFT_LOCK.clone().lock_owned().await;
    world.aod.drift = DriftState::default();
    world.aod.drift_guard = Some(guard);
}

async fn stage_tree() -> PathBuf {
    let root = workspace_root();
    let tree = drift_dir().join("tree");
    tokio::task::spawn_blocking(move || {
        if tree.exists() {
            std::fs::remove_dir_all(&tree).expect("clear the staged tree");
        }
        std::fs::create_dir_all(&tree).expect("create the staged tree");
        // Everything a shipped check-arch rule reads (workflows, Dockerfile,
        // docs, static assets), minus build output, VCS and tool caches.
        const SKIP: [&str; 10] = [
            "target",
            ".git",
            ".codegraph",
            ".dual-graph",
            ".nwave",
            ".claude",
            "venv",
            "mutants.out",
            "mutants.out.old",
            "sbom",
        ];
        let entries: Vec<String> = std::fs::read_dir(&root)
            .expect("list the repository root")
            .map(|e| {
                e.expect("root entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| !SKIP.contains(&name.as_str()) && name != ".DS_Store")
            .collect();
        for entry in entries {
            let from = root.join(&entry);
            if !from.exists() {
                continue;
            }
            // `cp -pR` keeps mtimes, so the dedicated target dir reuses the
            // rest of the workspace; `forget_foundry_api_build` makes sure
            // foundry-api itself is never reused across staged trees.
            let status = std::process::Command::new("cp")
                .arg("-pR")
                .arg(&from)
                .arg(&tree)
                .status()
                .expect("run cp");
            assert!(status.success(), "copy {entry} into the staged tree");
        }
        tree
    })
    .await
    .expect("join staging")
}

async fn ensure_staged(world: &mut FoundryWorld) -> PathBuf {
    if let Some(tree) = &world.aod.drift.tree {
        return tree.clone();
    }
    let tree = stage_tree().await;
    world.aod.drift.tree = Some(tree.clone());
    tree
}

async fn xtask_binary() -> PathBuf {
    let root = workspace_root();
    let cwd = root.clone();
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new(env!("CARGO"))
            .current_dir(&cwd)
            .args(["build", "-q", "-p", "xtask"])
            .status()
    })
    .await
    .expect("join build")
    .expect("build xtask");
    assert!(status.success(), "build xtask for the drift check");
    root.join("target").join("debug").join("xtask")
}

fn edit(tree: &Path, rel: &str, f: impl FnOnce(String) -> String) {
    let path = tree.join(rel);
    let before = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {} in the staged tree: {e}", path.display()));
    let after = f(before.clone());
    assert_ne!(before, after, "the plant did not change {rel}");
    std::fs::write(&path, after).expect("write the plant");
}

fn insert_after_each_construction(text: &str, structure: &str, field_line: &str) -> String {
    let needle = format!("{structure} {{");
    let mut out = String::new();
    let mut rest = text;
    let mut constructions = 0;
    while let Some(at) = rest.find(&needle) {
        let (head, tail) = rest.split_at(at + needle.len());
        out.push_str(head);
        let is_definition = head.ends_with(&format!("pub struct {needle}"));
        if !is_definition {
            out.push_str(field_line);
            constructions += 1;
        }
        rest = tail;
    }
    out.push_str(rest);
    assert!(
        constructions >= 1,
        "found no construction of {structure} to update"
    );
    out
}

fn read_committed(tree: &Path) -> Vec<u8> {
    let path = committed_description_path(tree);
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "there is no committed API description at {}: {e}",
            path.display()
        )
    })
}

fn description_version(bytes: &[u8]) -> String {
    let doc: Value = serde_json::from_slice(bytes).expect("the committed description is JSON");
    doc["info"]["version"]
        .as_str()
        .expect("the committed description has info.version")
        .to_string()
}

fn next_patch(version: &str) -> String {
    let mut parts: Vec<u64> = version
        .split('.')
        .map(|p| p.parse().expect("a numeric version part"))
        .collect();
    let last = parts.last_mut().expect("a version has parts");
    *last += 1;
    parts
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn bump_release(world: &mut FoundryWorld, tree: &Path) {
    let from = running_release().to_string();
    let to = next_patch(&from);
    let crates = std::fs::read_dir(tree.join("crates")).expect("list crates");
    for entry in crates {
        let manifest = entry.expect("crate entry").path().join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&manifest).expect("read manifest");
        let old = format!("version = \"{from}\"");
        if text.contains(&old) {
            std::fs::write(
                &manifest,
                text.replacen(&old, &format!("version = \"{to}\""), 1),
            )
            .expect("bump manifest");
        }
    }
    world.aod.drift.bumped_from = Some(from);
    world.aod.drift.bumped_to = Some(to);
}

fn plant(world: &mut FoundryWorld, tree: &Path, drift: &DriftPlant) {
    const API_LIB: &str = "crates/foundry-api/src/lib.rs";
    match drift {
        DriftPlant::AddField { structure, field } => {
            let line = format!("\n    pub {field}: String,");
            let value = format!("\n            {field}: String::new(),");
            edit(tree, API_LIB, |text| {
                let def = format!("pub struct {structure} {{");
                assert!(text.contains(&def), "no {structure} in foundry-api");
                let text = text.replacen(&def, &format!("{def}{line}"), 1);
                insert_after_each_construction(&text, structure, &value)
            });
        }
        DriftPlant::Conflict409 => edit(tree, API_LIB, |text| {
            let anchor = "    let rows = services\n        .list_board_issues(&principal, &team_slug, &project_slug)";
            assert!(text.contains(anchor), "the list-issues handler moved");
            text.replacen(
                anchor,
                &format!(
                    "    if team_slug.is_empty() {{\n        return Err(ApiError(ServiceError::Conflict));\n    }}\n{anchor}"
                ),
                1,
            )
        }),
        DriftPlant::NewOperation => edit(tree, API_LIB, |text| {
            let anchor = "    Router::new()\n";
            assert!(text.contains(anchor), "routes() moved");
            let text = text.replacen(
                anchor,
                "    Router::new()\n        .route(\"/api/v1/teams/{team_slug}/projects\", get(list_projects_planted))\n",
                1,
            );
            format!(
                "{text}\n/// Planted by the api-openapi-document drift scenario.\nasync fn list_projects_planted(\n    MachinePrincipal(_principal): MachinePrincipal,\n    Path(_team_slug): Path<String>,\n) -> Json<Vec<String>> {{\n    Json(Vec::new())\n}}\n"
            )
        }),
        DriftPlant::RemoveListIssues => edit(tree, API_LIB, |text| {
            let route = "get(list_issues_handler).post(create_issue_handler)";
            assert!(text.contains(route), "the list-issues route moved");
            text.replacen(route, "post(create_issue_handler)", 1)
                .replacen(
                    "async fn list_issues_handler(",
                    "#[allow(dead_code)]\nasync fn list_issues_handler(",
                    1,
                )
        }),
        DriftPlant::BumpRelease => {
            read_committed(tree);
            bump_release(world, tree);
        }
        DriftPlant::Reword => {
            let bytes = read_committed(tree);
            let doc: Value = serde_json::from_slice(&bytes).expect("JSON");
            let (method, template) = Operation::ListIssues.documented_as().expect("documented");
            let op = &doc["paths"][template][method];
            let original = op["description"]
                .as_str()
                .or_else(|| op["summary"].as_str())
                .expect("listing a project's issues has a description or summary")
                .to_string();
            let encoded = serde_json::to_string(&original).expect("encode");
            let reworded =
                serde_json::to_string(&format!("{original} Reworded by the maintainer."))
                    .expect("encode");
            edit(tree, "crates/foundry-api/openapi.json", |text| {
                assert!(
                    text.contains(&encoded),
                    "the description text is not found verbatim"
                );
                text.replacen(&encoded, &reworded, 1)
            });
        }
        DriftPlant::UndocumentRevoke => {
            let bytes = read_committed(tree);
            let mut doc: Value = serde_json::from_slice(&bytes).expect("JSON");
            let (method, template) = Operation::RevokeToken.documented_as().expect("documented");
            let item = doc["paths"][template]
                .as_object_mut()
                .expect("the revoke address is described");
            assert!(item.remove(method).is_some(), "revoke is described");
            let pretty = serde_json::to_string_pretty(&doc).expect("encode");
            std::fs::write(committed_description_path(tree), pretty).expect("write");
        }
    }
}

/// Drops every cached foundry-api fingerprint in the drift target dir, so the
/// next build recompiles foundry-api (and re-reads the `include_bytes!` of
/// `openapi.json`) while the rest of the workspace stays cached.
///
/// mtimes cannot decide this: the staged copy keeps the checkout's mtimes, and
/// the file that differs from the cached build is often one no plant touched
/// (a release bump leaves `openapi.json` at the checkout's old mtime while an
/// earlier refresh built the same version against a rewritten one). Cargo then
/// sees the cached artifact as newer and reuses it — a false green.
fn forget_foundry_api_build(target: &Path) {
    let Ok(profiles) = std::fs::read_dir(target) else {
        return; // nothing cached yet
    };
    for profile in profiles {
        let fingerprints = profile
            .expect("drift target entry")
            .path()
            .join(".fingerprint");
        let Ok(units) = std::fs::read_dir(&fingerprints) else {
            continue;
        };
        for unit in units {
            let unit = unit.expect("fingerprint entry");
            if unit
                .file_name()
                .to_string_lossy()
                .starts_with("foundry-api-")
            {
                std::fs::remove_dir_all(unit.path()).unwrap_or_else(|e| {
                    panic!("forget the cached build {}: {e}", unit.path().display())
                });
            }
        }
    }
}

async fn run_drift_check(world: &mut FoundryWorld) {
    let tree = ensure_staged(world).await;
    let xtask = xtask_binary().await;
    let target = drift_dir().join("target");
    let (output, passed) = tokio::task::spawn_blocking(move || {
        forget_foundry_api_build(&target);
        let arch = std::process::Command::new(&xtask)
            .current_dir(&tree)
            .args(["check-arch", "--root"])
            .arg(&tree)
            .output()
            .expect("run xtask check-arch");
        let tests = std::process::Command::new(env!("CARGO"))
            .current_dir(&tree)
            .env("CARGO_TARGET_DIR", &target)
            .args(["test", "--offline", "-p", "foundry-api", "--lib", "openapi"])
            .output()
            .expect("run the foundry-api description tests");
        let mut text = String::new();
        for out in [&arch, &tests] {
            text.push_str(&String::from_utf8_lossy(&out.stdout));
            text.push_str(&String::from_utf8_lossy(&out.stderr));
        }
        (text, arch.status.success() && tests.status.success())
    })
    .await
    .expect("join the drift check");
    world.aod.drift.output = output;
    world.aod.drift.passed = Some(passed);
}

pub async fn when_runs_drift_check(world: &mut FoundryWorld) {
    run_drift_check(world).await;
}

pub async fn when_plants_and_checks(world: &mut FoundryWorld, drift: DriftPlant) {
    let tree = ensure_staged(world).await;
    plant(world, &tree, &drift);
    run_drift_check(world).await;
}

pub async fn given_bumped(world: &mut FoundryWorld) {
    let tree = ensure_staged(world).await;
    plant(world, &tree, &DriftPlant::BumpRelease);
}

/// DEVOPS R2: `xtask openapi --check --root <tree>`, read-only.
pub async fn when_checks_only(world: &mut FoundryWorld) {
    let tree = ensure_staged(world).await;
    world.aod.drift.description_before = Some(read_committed(&tree));
    let xtask = xtask_binary().await;
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&xtask)
            .current_dir(&tree)
            .args(["openapi", "--check", "--root"])
            .arg(&tree)
            .output()
            .expect("run xtask openapi --check")
    })
    .await
    .expect("join the check");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    world.aod.drift.output = text;
    world.aod.drift.passed = Some(output.status.success());
}

/// The refusal must come from the check itself, not from an unknown subcommand.
pub fn then_check_refuses(world: &FoundryWorld) {
    let (output, passed) = drift(world);
    assert!(
        !passed,
        "the description check accepted a stale description:\n{output}"
    );
    assert!(
        !output.contains("unknown") && !output.contains("Usage"),
        "the description check did not run (no `openapi --check` door):\n{output}"
    );
}

pub async fn when_refreshes(world: &mut FoundryWorld) {
    let tree = ensure_staged(world).await;
    world.aod.drift.description_before = Some(read_committed(&tree));
    let xtask = xtask_binary().await;
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&xtask)
            .current_dir(&tree)
            .args(["openapi", "--root"])
            .arg(&tree)
            .output()
            .expect("run xtask openapi")
    })
    .await
    .expect("join the refresh");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    assert!(
        output.status.success(),
        "the description refresh (`cargo xtask openapi`) failed: {text}"
    );
    world.aod.drift.output = text;
}

fn drift(world: &FoundryWorld) -> (&str, bool) {
    let passed = world
        .aod
        .drift
        .passed
        .expect("the maintainer ran the drift check");
    (world.aod.drift.output.as_str(), passed)
}

pub fn then_drift_passes(world: &FoundryWorld) {
    let (output, passed) = drift(world);
    assert!(passed, "the drift check failed:\n{output}");
}

pub fn then_drift_fails(world: &FoundryWorld) {
    let (output, passed) = drift(world);
    assert!(
        !passed,
        "the drift check passed over a planted drift:\n{output}"
    );
}

pub fn then_reports_compared(world: &FoundryWorld) {
    let (output, _) = drift(world);
    for proof in [
        "api-openapi-operations",
        "api-openapi-statuses",
        "wire_shapes_match_document",
        "document_is_valid_openapi_3_1",
        "info_version_is_the_release",
    ] {
        assert!(
            output.contains(proof),
            "the drift check does not report that it ran {proof}:\n{output}"
        );
    }
}

fn assert_names(world: &FoundryWorld, needles: &[&str]) {
    let (output, _) = drift(world);
    for needle in needles {
        assert!(
            output.contains(needle),
            "the drift check does not name {needle:?}:\n{output}"
        );
    }
}

pub fn then_names_field(world: &FoundryWorld, wherever: &str, field: &str) {
    let (_, template) = Operation::ListIssues.documented_as().expect("documented");
    match wherever.trim() {
        "listing a project's issues and its 200 answer" => {
            assert_names(world, &[template, "200", field]);
        }
        "the error answer" => assert_names(world, &["Error", field]),
        other => panic!("unknown place phrase {other:?}"),
    }
}

pub fn then_names_409(world: &FoundryWorld) {
    let (_, template) = Operation::ListIssues.documented_as().expect("documented");
    assert_names(world, &[template, "409"]);
}

pub fn then_names_operation(world: &FoundryWorld, operation: &str) {
    let (method, path) = operation
        .split_once(' ')
        .unwrap_or_else(|| panic!("an operation reads \"METHOD /path\": {operation:?}"));
    assert_names(world, &[path]);
    let (output, _) = drift(world);
    assert!(
        output.contains(method) || output.contains(&method.to_ascii_lowercase()),
        "the drift check does not name the method {method}:\n{output}"
    );
}

pub fn then_names_version(world: &FoundryWorld) {
    let from = world
        .aod
        .drift
        .bumped_from
        .clone()
        .expect("a bump was planted");
    let to = world
        .aod
        .drift
        .bumped_to
        .clone()
        .expect("a bump was planted");
    assert_names(world, &[&from, &to, "cargo xtask openapi"]);
}

pub fn then_version_is_new(world: &FoundryWorld) {
    let tree = world.aod.drift.tree.clone().expect("a staged tree");
    let to = world
        .aod
        .drift
        .bumped_to
        .clone()
        .expect("a bump was planted");
    assert_eq!(
        description_version(&read_committed(&tree)),
        to,
        "info.version after the refresh"
    );
}

pub fn then_other_bytes_unchanged(world: &FoundryWorld) {
    let tree = world.aod.drift.tree.clone().expect("a staged tree");
    let before = world
        .aod
        .drift
        .description_before
        .clone()
        .expect("captured before the refresh");
    let after = read_committed(&tree);
    let from = format!("\"{}\"", description_version(&before));
    let to = format!("\"{}\"", description_version(&after));
    let before_text = String::from_utf8(before).expect("UTF-8");
    assert_eq!(
        before_text.matches(&from).count(),
        1,
        "the version literal {from} must occur once in the committed description"
    );
    assert_eq!(
        String::from_utf8(after).expect("UTF-8"),
        before_text.replacen(&from, &to, 1),
        "the refresh changed more than info.version"
    );
}

pub async fn then_drift_now_passes(world: &mut FoundryWorld) {
    run_drift_check(world).await;
    let (output, passed) = drift(world);
    assert!(passed, "the drift check fails after the refresh:\n{output}");
}

pub fn then_description_unchanged(world: &FoundryWorld) {
    let tree = world.aod.drift.tree.clone().expect("a staged tree");
    let before = world
        .aod
        .drift
        .description_before
        .clone()
        .expect("captured before the refresh");
    assert!(
        read_committed(&tree) == before,
        "refreshing a current description changed it"
    );
}

// ===========================================================================
// The API page — HTTP lane
// ===========================================================================

pub async fn when_visitor_asks_page(world: &mut FoundryWorld) {
    world.aod.last = Some(get(world, PAGE_PATH, &[]).await);
}

fn page_answer(world: &FoundryWorld) -> &Answer {
    let answer = last(world);
    assert_eq!(
        answer.status,
        200,
        "the API page is not served: {}",
        answer.summary()
    );
    answer
}

pub fn then_page_csp(world: &FoundryWorld, policy: &str) {
    let answer = page_answer(world);
    assert_eq!(
        answer.header("content-security-policy"),
        Some(policy),
        "the API page's content policy (ADR-API-OPENAPI-004)"
    );
}

pub fn then_page_companions(world: &FoundryWorld) {
    let answer = page_answer(world);
    for (name, value) in [
        ("x-frame-options", "DENY"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
    ] {
        assert_eq!(answer.header(name), Some(value), "{name} on the API page");
    }
}

pub async fn then_others_no_csp(world: &FoundryWorld) {
    for path in [DOC_PATH, "/sign-in"] {
        let answer = get(world, path, &[]).await;
        assert!(
            answer.header("content-security-policy").is_none(),
            "{path} carries a content policy; it is the API page's only (D12, SC-3)"
        );
    }
}

pub async fn when_fetches_page_files(world: &mut FoundryWorld) {
    let mut answers = Vec::new();
    for path in PAGE_FILES.iter().chain(VENDORED_FILES.iter()) {
        answers.push(get(world, path, &[]).await);
    }
    world.aod.last = answers.first().cloned();
    world.aod.fetched_files = answers;
}

/// The directory the server serves `/static` from, resolved exactly as
/// `foundry_app::static_dir` does (`crates/foundry-app/src/lib.rs:255-261`): a
/// cwd-relative `static/` when it exists, else foundry-app's crate-local one.
fn served_static_root() -> PathBuf {
    let cwd_relative = PathBuf::from("static");
    if cwd_relative.is_dir() {
        return cwd_relative;
    }
    workspace_root().join("crates/foundry-app/static")
}

pub fn then_files_match_repository(world: &FoundryWorld) {
    let static_root = served_static_root();
    assert!(!world.aod.fetched_files.is_empty(), "no file was fetched");
    for answer in &world.aod.fetched_files {
        assert_eq!(answer.status, 200, "{}", answer.summary());
        let relative = answer
            .path
            .strip_prefix("/static/")
            .unwrap_or_else(|| panic!("{} is not under /static", answer.path));
        let on_disk = std::fs::read(static_root.join(relative))
            .unwrap_or_else(|e| panic!("the repository holds no {}: {e}", answer.path));
        assert!(
            answer.body == on_disk,
            "{} is not served as the repository holds it",
            answer.path
        );
    }
}

pub async fn when_visitor_and_priya_ask_page(world: &mut FoundryWorld) {
    let visitor = get(world, PAGE_PATH, &[]).await;
    let pair = session_of(world, "priya@canzan.dev").await;
    let member = get(world, PAGE_PATH, &[("cookie", pair)]).await;
    world.aod.pair = Some((visitor, member));
}

pub fn then_same_page_no_cookie(world: &FoundryWorld) {
    let (visitor, member) = world.aod.pair.as_ref().expect("both asked");
    for answer in [visitor, member] {
        assert_eq!(answer.status, 200, "{}", answer.summary());
        assert!(
            answer.header("set-cookie").is_none(),
            "the API page sets a cookie (DDD-15)"
        );
    }
    assert!(
        visitor.body == member.body,
        "the API page differs for a signed-in member"
    );
}

// ===========================================================================
// The API page — browser lane
// ===========================================================================

use crate::support::browser_harness;
use fantoccini::Locator;

/// swagger-ui 5.33.3's markup, in ONE place. Everything below is upstream's
/// DOM contract (class names and the `data-path` / `data-param-name`
/// attributes swagger-ui 5 renders), not Foundry's: if the pinned version
/// changes, this module is the only edit. Foundry's own markup (title, heading
/// line, `<noscript>`, the Authorize label and hint) is read as page TEXT, never
/// by selector.
mod swagger_dom {
    pub const OPERATION: &str = ".swagger-ui .opblock";
    pub const SETTLED: &str = ".swagger-ui .opblock, .swagger-ui .errors-wrapper";
    pub const SUMMARY_METHOD: &str = ".opblock-summary-method";
    pub const SUMMARY_PATH: &str = ".opblock-summary-path";
    pub const AUTHORIZE: &str = ".swagger-ui .btn.authorize";
    pub const AUTH_DIALOG: &str = ".swagger-ui .auth-container";
    pub const AUTH_INPUT: &str = ".swagger-ui .auth-container input";
    pub const AUTH_SUBMIT: &str = ".swagger-ui .auth-btn-wrapper .authorize";
    pub const AUTH_CLOSE: &str = ".swagger-ui .btn-done, .swagger-ui .close-modal";
    pub const AUTH_UNLOCKED: &str =
        ".swagger-ui .authorization__btn.unlocked, .swagger-ui .btn.authorize.unlocked";
    pub const SUMMARY: &str = ".opblock-summary";
    pub const TRY_OUT: &str = ".try-out__btn";
    pub const EXECUTE: &str = ".execute";
    pub const LIVE_STATUS: &str = ".live-responses-table .response-col_status";
    pub const LIVE_BODY: &str =
        ".live-responses-table .response-col_description pre, .live-responses-table .microlight";
    pub const CURL: &str = ".live-responses-table .curl, .curl-command .curl, .curl";

    /// One operation's block, by method and documented path.
    pub fn operation(method: &str, path: &str) -> String {
        format!(".swagger-ui .opblock-{method}:has(.opblock-summary-path[data-path=\"{path}\"])")
    }

    /// One path parameter's input inside an operation's block.
    pub fn parameter(block: &str, name: &str) -> String {
        format!("{block} tr[data-param-name=\"{name}\"] input")
    }
}

fn browser(world: &FoundryWorld) -> &fantoccini::Client {
    world.browser.as_ref().expect("a browser session is open")
}

async fn ensure_browser(world: &mut FoundryWorld) {
    if world.browser.is_none() {
        world.browser = Some(browser_harness::new_session().await);
    }
}

async fn open_page(world: &mut FoundryWorld, query: &str) {
    ensure_browser(world).await;
    let url = format!("{}{PAGE_PATH}{query}", base(world));
    browser(world).goto(&url).await.expect("open the API page");
}

async fn wait_for(world: &FoundryWorld, css: &str, what: &str) {
    browser(world)
        .wait()
        .at_most(std::time::Duration::from_secs(20))
        .for_element(Locator::Css(css))
        .await
        .unwrap_or_else(|e| panic!("the API page never showed {what} ({css}): {e}"));
}

async fn script(world: &FoundryWorld, js: &str) -> Value {
    script_with(world, js, Vec::new()).await
}

async fn script_with(world: &FoundryWorld, js: &str, args: Vec<Value>) -> Value {
    browser(world)
        .execute(js, args)
        .await
        .unwrap_or_else(|e| panic!("run a probe in the page: {e}"))
}

async fn settle(world: &FoundryWorld) {
    wait_for(world, swagger_dom::OPERATION, "its operations").await;
}

pub async fn when_opens_page(world: &mut FoundryWorld) {
    open_page(world, "").await;
    settle(world).await;
}

pub async fn when_opens_page_loaded(world: &mut FoundryWorld) {
    open_page(world, "").await;
    wait_until(
        world,
        "return document.readyState === 'complete';",
        "the page to load",
    )
    .await;
    // swagger-ui settles after the load event: either the operations or its own
    // error panel. Neither appears on a page that is not the API page, which the
    // Then steps judge, so this wait is bounded and not asserted.
    let _ = browser(world)
        .wait()
        .at_most(std::time::Duration::from_secs(20))
        .for_element(Locator::Css(swagger_dom::SETTLED))
        .await;
}

/// Poll a script until it returns true (bounded; panics naming `what`).
async fn wait_until(world: &FoundryWorld, js: &str, what: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        if script(world, js).await == Value::Bool(true) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the browser never reached {what}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

pub async fn then_title_and_line(world: &FoundryWorld, title: &str, line: &str) {
    let line = line.replace("{running release}", running_release());
    let seen = script(world, "return [document.title, document.body.innerText];").await;
    assert_eq!(seen[0], Value::String(title.into()), "the API page's title");
    assert!(
        seen[1].as_str().unwrap_or_default().contains(&line),
        "the API page does not read {line:?}"
    );
}

pub async fn then_every_operation_listed(world: &FoundryWorld) {
    let doc = description(world).await;
    let listed = script_with(
        world,
        "return Array.from(document.querySelectorAll(arguments[0])).map(function (b) {
           var m = b.querySelector(arguments[1]);
           var p = b.querySelector(arguments[2]);
           return (m ? m.innerText : '').toLowerCase() + ' ' + (p ? (p.getAttribute('data-path') || p.innerText) : '');
         });",
        vec![
            Value::String(swagger_dom::OPERATION.into()),
            Value::String(swagger_dom::SUMMARY_METHOD.into()),
            Value::String(swagger_dom::SUMMARY_PATH.into()),
        ],
    )
    .await;
    let listed: BTreeSet<String> = listed
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().replace('\u{200b}', "")))
                .collect()
        })
        .unwrap_or_default();
    for (path, method, _) in operations(&doc) {
        let key = format!("{method} {path}");
        assert!(
            listed.contains(&key),
            "the API page does not list {key}; it lists {listed:?}"
        );
    }
}

async fn resource_origins(world: &FoundryWorld) -> Vec<String> {
    let names = script(
        world,
        "return [location.href].concat(performance.getEntriesByType('resource').map(function (e) { return e.name; }));",
    )
    .await;
    names
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub async fn then_all_requests_own_origin(world: &FoundryWorld) {
    let origin = base(world);
    let names = resource_origins(world).await;
    assert!(
        names.len() > 1,
        "the browser recorded no request beyond the page"
    );
    // Control: the page under judgement is the API page with its own files
    // (an absent page also requests nothing from another site).
    for marker in PAGE_ONLY_MARKERS.iter().chain([DOC_PATH].iter()) {
        assert!(
            names.iter().any(|n| n.contains(marker)),
            "the page did not load {marker}, so it is not the API page: {names:?}"
        );
    }
    let foreign: Vec<&String> = names.iter().filter(|n| !n.starts_with(&origin)).collect();
    assert!(
        foreign.is_empty(),
        "the API page made requests to another site: {foreign:?}"
    );
}

async fn log_lines(world: &FoundryWorld) -> Vec<String> {
    browser_harness::browser_log(browser(world))
        .await
        .into_iter()
        .map(|entry| entry["message"].as_str().unwrap_or_default().to_string())
        .collect()
}

pub async fn then_nothing_refused(world: &FoundryWorld) {
    let refused: Vec<String> = log_lines(world)
        .await
        .into_iter()
        .filter(|m| m.contains("Content Security Policy") || m.contains("Refused to"))
        .collect();
    assert!(
        refused.is_empty(),
        "the browser refused content the API page asked for (an eval here is OQ-U4): {refused:?}"
    );
}

async fn start_foreign_site(world: &mut FoundryWorld) {
    if world.aod.foreign.is_some() {
        return;
    }
    let hits: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let page_url = format!("{}{PAGE_PATH}", base(world));
    let recorder = hits.clone();
    let app = axum::Router::new().fallback(move |uri: axum::http::Uri| {
        let recorder = recorder.clone();
        let page_url = page_url.clone();
        async move {
            recorder.lock().expect("hits").push(uri.path().to_string());
            match uri.path() {
                "/frame.html" => axum::response::Html(format!(
                    "<!doctype html><title>elsewhere</title><iframe id=\"embedded\" src=\"{page_url}\"></iframe>"
                ))
                .into_response(),
                "/spec.json" => axum::Json(serde_json::json!({
                    "openapi": "3.1.0",
                    "info": {"title": "Someone else's API", "version": "6.6.6"},
                    "paths": {"/steal": {"get": {"responses": {"200": {"description": "ok"}}}}}
                }))
                .into_response(),
                "/config.json" => axum::Json(serde_json::json!({
                    "url": "/spec.json"
                }))
                .into_response(),
                _ => axum::http::StatusCode::NOT_FOUND.into_response(),
            }
        }
    });
    use axum::response::IntoResponse;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0")
        .await
        .expect("bind the other site");
    let port = listener.local_addr().expect("addr").port();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    world.aod.foreign = Some(ForeignSite {
        origin: format!("http://127.0.0.1:{port}"),
        hits,
        task,
    });
}

pub async fn given_framing_site(world: &mut FoundryWorld) {
    start_foreign_site(world).await;
}

pub async fn when_opens_framing_page(world: &mut FoundryWorld) {
    ensure_browser(world).await;
    let url = format!(
        "{}/frame.html",
        world.aod.foreign.as_ref().expect("site").origin
    );
    browser(world)
        .goto(&url)
        .await
        .expect("open the framing page");
    wait_until(
        world,
        "return document.readyState === 'complete' && !!document.getElementById('embedded');",
        "the framing page",
    )
    .await;
}

pub async fn then_refuses_frame(world: &FoundryWorld) {
    // The browser log is read destructively, so accumulate until the refusal
    // shows or the bound passes (no fixed sleep).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut lines = Vec::new();
    loop {
        lines.extend(log_lines(world).await);
        if lines
            .iter()
            .any(|m| m.contains("frame-ancestors") || m.contains("X-Frame-Options"))
        {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the browser did not refuse to frame the API page: {lines:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

pub async fn given_scripting_disabled(world: &mut FoundryWorld) {
    world.browser = Some(browser_harness::new_session_without_scripting().await);
}

pub async fn when_priya_opens_page(world: &mut FoundryWorld) {
    open_page(world, "").await;
}

pub async fn then_reads_on_page(world: &FoundryWorld, copy: &str) {
    let text = script(world, "return document.body.innerText;").await;
    assert!(
        text.as_str().unwrap_or_default().contains(copy),
        "the API page does not read {copy:?}"
    );
}

pub async fn then_doc_link(world: &FoundryWorld) {
    let hrefs = script(
        world,
        "return Array.from(document.querySelectorAll('a')).map(function (a) { return a.getAttribute('href'); });",
    )
    .await;
    assert!(
        hrefs
            .as_array()
            .map(|a| a.iter().any(|h| h == DOC_PATH))
            .unwrap_or(false),
        "the API page has no link to {DOC_PATH}: {hrefs}"
    );
}

pub async fn given_description_blocked(world: &mut FoundryWorld) {
    ensure_browser(world).await;
    let url = format!("{}{DOC_PATH}", base(world));
    browser_harness::devtools(browser(world), "Network.enable", serde_json::json!({})).await;
    browser_harness::devtools(
        browser(world),
        "Network.setBlockedURLs",
        serde_json::json!({ "urls": [url] }),
    )
    .await;
}

pub async fn then_load_failure_shown(world: &FoundryWorld) {
    // Judged on Foundry's authored D14 copy only: the heading line stays, with
    // its link to the description, and no operation was rendered. swagger-ui's
    // own failure wording is upstream's and is not asserted.
    let line = format!(
        "Foundry API v{}. The OpenAPI document is at {DOC_PATH}.",
        running_release()
    );
    let seen = script_with(
        world,
        "return [document.body.innerText,
                 Array.from(document.querySelectorAll('a')).map(function (a) { return a.getAttribute('href'); }),
                 document.querySelectorAll(arguments[0]).length];",
        vec![Value::String(swagger_dom::OPERATION.into())],
    )
    .await;
    assert!(
        seen[0].as_str().unwrap_or_default().contains(&line),
        "the heading line {line:?} is gone when the description cannot be loaded"
    );
    assert!(
        seen[1]
            .as_array()
            .map(|a| a.iter().any(|h| h == DOC_PATH))
            .unwrap_or(false),
        "the heading line's link to {DOC_PATH} is gone"
    );
    assert_eq!(
        seen[2].as_u64(),
        Some(0),
        "operations rendered although the description was blocked"
    );
}

pub async fn given_page_opened(world: &mut FoundryWorld) {
    when_opens_page(world).await;
    let names = resource_origins(world).await;
    for marker in PAGE_ONLY_MARKERS {
        assert!(
            names.iter().any(|n| n.contains(marker)),
            "the API page did not load its own {marker} files"
        );
    }
}

pub async fn when_priya_opens_board(world: &mut FoundryWorld) {
    let base_url = base(world);
    browser_harness::sign_in_through_browser(
        browser(world),
        harness(world),
        "priya@canzan.dev",
        PASSWORD,
    )
    .await;
    browser(world)
        .goto(&format!("{base_url}/team/{TEAM}/project/{PROJECT}"))
        .await
        .expect("open Backend's board");
    wait_until(
        world,
        "return document.readyState === 'complete';",
        "Backend's board",
    )
    .await;
}

pub async fn then_board_has_no_page_files(world: &FoundryWorld) {
    let names = resource_origins(world).await;
    assert!(
        names[0].contains(&format!("/team/{TEAM}/project/{PROJECT}")),
        "the board did not open: {}",
        names[0]
    );
    let carried: Vec<&String> = names
        .iter()
        .filter(|n| PAGE_ONLY_MARKERS.iter().any(|m| n.contains(m)))
        .collect();
    assert!(
        carried.is_empty(),
        "the board loads the API page's files (D12): {carried:?}"
    );
}

async fn click(world: &FoundryWorld, css: &str, what: &str) {
    wait_for(world, css, what).await;
    browser(world)
        .find(Locator::Css(css))
        .await
        .unwrap_or_else(|e| panic!("find {what}: {e}"))
        .click()
        .await
        .unwrap_or_else(|e| panic!("click {what}: {e}"));
}

async fn authorize(world: &FoundryWorld, jwt: &str) {
    click(world, swagger_dom::AUTHORIZE, "the Authorize button").await;
    wait_for(world, swagger_dom::AUTH_INPUT, "the token field").await;
    browser(world)
        .find(Locator::Css(swagger_dom::AUTH_INPUT))
        .await
        .expect("the token field")
        .send_keys(jwt)
        .await
        .expect("paste the token");
    click(world, swagger_dom::AUTH_SUBMIT, "the dialog's Authorize").await;
    click(world, swagger_dom::AUTH_CLOSE, "the dialog's close").await;
}

async fn try_operation(world: &FoundryWorld, method: &str, path: &str, params: &[(&str, String)]) {
    let block = swagger_dom::operation(method, path);
    click(
        world,
        &format!("{block} {}", swagger_dom::SUMMARY),
        "the operation",
    )
    .await;
    click(
        world,
        &format!("{block} {}", swagger_dom::TRY_OUT),
        "Try it out",
    )
    .await;
    for (name, value) in params {
        let field = swagger_dom::parameter(&block, name);
        wait_for(world, &field, name).await;
        let input = browser(world)
            .find(Locator::Css(&field))
            .await
            .expect("parameter field");
        input.clear().await.expect("clear the parameter");
        input.send_keys(value).await.expect("type the parameter");
    }
    click(
        world,
        &format!("{block} {}", swagger_dom::EXECUTE),
        "Execute",
    )
    .await;
    wait_for(
        world,
        &format!("{block} {}", swagger_dom::LIVE_STATUS),
        "the answer",
    )
    .await;
}

async fn shown_answer(world: &FoundryWorld) -> (u16, String) {
    let seen = script_with(
        world,
        "var s = document.querySelector(arguments[0]);
         var b = document.querySelector(arguments[1]);
         return [s ? s.innerText : '', b ? b.innerText : ''];",
        vec![
            Value::String(swagger_dom::LIVE_STATUS.into()),
            Value::String(swagger_dom::LIVE_BODY.into()),
        ],
    )
    .await;
    let status: u16 = seen[0]
        .as_str()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .parse()
        .unwrap_or(0);
    (status, seen[1].as_str().unwrap_or_default().to_string())
}

const ISSUES_TEMPLATE: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}/issues";
const REVOKE_TEMPLATE: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}";

fn address_params(
    world: &FoundryWorld,
    call: PageCall,
) -> (&'static str, &'static str, Vec<(&'static str, String)>) {
    let mut params = vec![
        ("team_slug", TEAM.to_string()),
        ("project_slug", PROJECT.to_string()),
    ];
    match call {
        PageCall::ListIssues => ("get", ISSUES_TEMPLATE, params),
        PageCall::RevokeScratch => {
            params.push(("jti", world.aod.tokens["scratch-rotation"].jti.to_string()));
            ("delete", REVOKE_TEMPLATE, params)
        }
    }
}

pub async fn when_pastes_and_lists(world: &mut FoundryWorld) {
    when_opens_page(world).await;
    let jwt = world.aod.tokens["ci-issue-filer"].jwt.clone();
    authorize(world, &jwt).await;
    let (method, path, params) = address_params(world, PageCall::ListIssues);
    try_operation(world, method, path, &params).await;
}

pub async fn then_sees_200_with_auth7(world: &FoundryWorld) {
    let (status, body) = shown_answer(world).await;
    assert_eq!(status, 200, "the page shows {status}: {body}");
    assert!(
        body.contains("AUTH-7"),
        "the page's answer does not list AUTH-7: {body}"
    );
}

pub async fn then_call_own_origin(world: &FoundryWorld) {
    let origin = base(world);
    let names = resource_origins(world).await;
    let call = format!("{origin}/api/v1/teams/{TEAM}/projects/{PROJECT}/issues");
    assert!(
        names.iter().any(|n| n.starts_with(&call)),
        "the page made no call to {call}: {names:?}"
    );
    then_all_requests_own_origin(world).await;
}

pub async fn then_curl_with_token(world: &FoundryWorld) {
    let curl = script_with(
        world,
        "var c = document.querySelector(arguments[0]); return c ? c.innerText : '';",
        vec![Value::String(swagger_dom::CURL.into())],
    )
    .await;
    let curl = curl.as_str().unwrap_or_default();
    let jwt = &world.aod.tokens["ci-issue-filer"].jwt;
    assert!(
        curl.contains("curl") && curl.contains(jwt.as_str()),
        "the page shows no command line carrying the token: {curl}"
    );
}

pub async fn when_opens_token_dialog(world: &mut FoundryWorld) {
    when_opens_page(world).await;
    click(world, swagger_dom::AUTHORIZE, "the Authorize button").await;
    wait_for(world, swagger_dom::AUTH_DIALOG, "the token dialog").await;
}

pub async fn then_token_field_copy(world: &FoundryWorld, label: &str, hint: &str) {
    let text = script_with(
        world,
        "var c = document.querySelector(arguments[0]); return c ? c.innerText : '';",
        vec![Value::String(swagger_dom::AUTH_DIALOG.into())],
    )
    .await;
    let text = text.as_str().unwrap_or_default();
    assert!(
        text.contains(label),
        "the token field is not labelled {label:?}: {text}"
    );
    assert!(
        text.contains(hint),
        "the token field does not say {hint:?}: {text}"
    );
}

pub async fn when_pastes_and_revokes(world: &mut FoundryWorld) {
    when_opens_page(world).await;
    let jwt = world.aod.tokens["ci-issue-filer"].jwt.clone();
    authorize(world, &jwt).await;
    let (method, path, params) = address_params(world, PageCall::RevokeScratch);
    try_operation(world, method, path, &params).await;
}

pub async fn then_204_and_revoked(world: &FoundryWorld) {
    let (status, body) = shown_answer(world).await;
    assert_eq!(status, 204, "the page shows {status}: {body}");
    // Judged through the door: the token list says scratch-rotation is revoked.
    let listed = call_raw(
        world,
        reqwest::Method::GET,
        &format!("/api/v1/teams/{TEAM}/projects/{PROJECT}/tokens"),
        Some("ci-issue-filer"),
        None,
        None,
    )
    .await;
    assert_eq!(listed.status, 200, "{}", listed.summary());
    let tokens: Value = serde_json::from_slice(&listed.body).expect("the token list is JSON");
    let scratch = tokens
        .as_array()
        .and_then(|a| a.iter().find(|t| t["label"] == "scratch-rotation"))
        .unwrap_or_else(|| panic!("the token list has no scratch-rotation: {tokens}"));
    assert_eq!(
        scratch["revoked"],
        Value::Bool(true),
        "scratch-rotation is not revoked"
    );
}

pub async fn when_tries_without_token(world: &mut FoundryWorld) {
    when_opens_page(world).await;
    let (method, path, params) = address_params(world, PageCall::ListIssues);
    try_operation(world, method, path, &params).await;
}

pub async fn then_sees_refusal(world: &FoundryWorld, status: u16, code: &str) {
    let (shown, body) = shown_answer(world).await;
    assert_eq!(shown, status, "the page shows {shown}: {body}");
    assert!(
        body.contains(&format!("\"{code}\"")),
        "the page's answer lacks the code {code}: {body}"
    );
}

pub async fn when_tries_refused(world: &mut FoundryWorld, call: PageCall, token: PageToken) {
    let jwt = match token {
        PageToken::Revoked => world.aod.tokens["old-ci-issue-filer"].jwt.clone(),
        PageToken::Mei => world.aod.tokens["mei-script"].jwt.clone(),
        PageToken::CiIssueFiler => world.aod.tokens["ci-issue-filer"].jwt.clone(),
        // Minted as every API feature mints them (the fixed test key): expired
        // is registered with a past `exp`; forged is signed but never registered.
        PageToken::Expired => {
            mint(
                world,
                "priya@canzan.dev",
                "aod-page-expired",
                None,
                -3600,
                false,
            )
            .await
            .jwt
        }
        PageToken::Forged => {
            mint(
                world,
                "priya@canzan.dev",
                "aod-page-forged",
                None,
                3600,
                true,
            )
            .await
            .jwt
        }
    };
    when_opens_page(world).await;
    authorize(world, &jwt).await;
    let (method, path, params) = address_params(world, call);
    try_operation(world, method, path, &params).await;
}

pub async fn given_pasted_and_called(world: &mut FoundryWorld) {
    when_pastes_and_lists(world).await;
}

pub async fn when_reloads(world: &mut FoundryWorld) {
    browser(world).refresh().await.expect("reload the API page");
    settle(world).await;
}

pub async fn then_no_token_filled(world: &FoundryWorld) {
    let unlocked = script_with(
        world,
        "return document.querySelectorAll(arguments[0]).length;",
        vec![Value::String(swagger_dom::AUTH_UNLOCKED.into())],
    )
    .await;
    assert!(
        unlocked.as_u64().unwrap_or(0) >= 1,
        "after a reload the page still holds a token"
    );
}

pub async fn then_no_trace(world: &FoundryWorld) {
    let jwt = world.aod.tokens["ci-issue-filer"].jwt.clone();
    let stores = script(
        world,
        "var all = [location.href, document.cookie];
         for (var i = 0; i < localStorage.length; i++) { var k = localStorage.key(i); all.push(k + '=' + localStorage.getItem(k)); }
         for (var j = 0; j < sessionStorage.length; j++) { var s = sessionStorage.key(j); all.push(s + '=' + sessionStorage.getItem(s)); }
         return all.join('\\n');",
    )
    .await;
    let stores = stores.as_str().unwrap_or_default();
    assert!(
        !stores.contains(&jwt),
        "the browser keeps a trace of the token: {stores}"
    );
}

pub async fn when_pastes_and_tries_every(world: &mut FoundryWorld) {
    when_opens_page(world).await;
    let jwt = world.aod.tokens["ci-issue-filer"].jwt.clone();
    authorize(world, &jwt).await;
    let doc = description(world).await;
    let scratch = world.aod.tokens["scratch-rotation"].jti.to_string();
    for (path, method, _) in operations(&doc) {
        let mut params: Vec<(&str, String)> = Vec::new();
        for (name, value) in [
            ("team_slug", TEAM.to_string()),
            ("project_slug", PROJECT.to_string()),
            ("number", "7".to_string()),
            ("comment_id", uuid::Uuid::nil().to_string()),
            ("jti", scratch.clone()),
        ] {
            if path.contains(&format!("{{{name}}}")) {
                params.push((name, value));
            }
        }
        try_operation(world, &method, &path, &params).await;
    }
}

pub async fn given_foreign_site(world: &mut FoundryWorld) {
    start_foreign_site(world).await;
}

pub async fn when_opens_foreign_link(world: &mut FoundryWorld, link: ForeignLink) {
    let origin = world
        .aod
        .foreign
        .as_ref()
        .expect("the other site runs")
        .origin
        .clone();
    let query = match link {
        ForeignLink::Description => format!("?url={origin}/spec.json"),
        ForeignLink::Configuration => format!("?configUrl={origin}/config.json"),
    };
    open_page(world, &query).await;
    settle(world).await;
}

pub async fn then_own_description_shown(world: &FoundryWorld) {
    let text = script(world, "return document.body.innerText;").await;
    let text = text.as_str().unwrap_or_default();
    assert!(
        !text.contains("Someone else's API"),
        "the page shows the other site's description"
    );
    assert!(
        text.contains("Foundry API v1"),
        "the page does not show the instance's own description"
    );
}

pub fn then_no_request_to_foreign(world: &FoundryWorld) {
    let site = world.aod.foreign.as_ref().expect("the other site runs");
    let hits = site.hits.lock().expect("hits").clone();
    assert!(
        hits.is_empty(),
        "the browser asked the other site for {hits:?}"
    );
}

#[cfg(test)]
mod oracle_self_check {
    //! The matrix oracle is proven on a fixture document before the real one
    //! exists: a `$ref` into `#/components/schemas` resolves locally, a
    //! conforming body passes, and each kind of drift the matrix must catch fails.
    use super::conforms;
    use serde_json::json;

    fn doc() -> serde_json::Value {
        json!({"components": {"schemas": {"Issue": {
            "type": "object",
            "required": ["key", "number", "title", "state"],
            "additionalProperties": false,
            "properties": {
                "key": {"type": "string"}, "number": {"type": "integer"},
                "title": {"type": "string"}, "state": {"type": "string"},
                "old": {"type": ["string", "null"]}
            }
        }}}})
    }

    #[test]
    fn a_conforming_list_passes_and_each_drift_fails() {
        let schema = json!({"type": "array", "items": {"$ref": "#/components/schemas/Issue"}});
        let good = json!([{"key": "AUTH-7", "number": 7, "title": "t", "state": "todo"}]);
        assert!(conforms(&doc(), &schema, &good).is_ok());
        for bad in [
            json!([{"key": "AUTH-7", "number": 7, "title": "t", "state": "todo", "priority": "x"}]),
            json!([{"key": "AUTH-7", "number": "7", "title": "t", "state": "todo"}]),
            json!([{"key": "AUTH-7", "number": 7, "title": "t"}]),
            json!({"key": "AUTH-7"}),
        ] {
            assert!(conforms(&doc(), &schema, &bad).is_err(), "{bad} passed");
        }
    }

    #[test]
    fn an_unresolvable_reference_is_an_error_not_a_fetch() {
        let schema = json!({"$ref": "https://example.invalid/schema.json"});
        assert!(conforms(&doc(), &schema, &json!({})).is_err());
    }
}
