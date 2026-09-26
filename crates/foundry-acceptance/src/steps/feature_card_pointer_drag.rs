//! card-pointer-drag (US-CPD-01..03) — step definitions.
//!
//! Scenario SSOT: `tests/features/card-pointer-drag.feature` (tag `@cpd`).
//! Grounding: `docs/feature/card-pointer-drag/feature-delta.md` (DISCUSS
//! D1-D20, DESIGN DDD-1..22) and `spike/findings.md`.
//!
//! DRIVER (DDD-12). Every card gesture is TRUSTED input: W3C WebDriver Actions
//! for the mouse, a touch contact, a pen and the keyboard, through
//! `browser_harness::perform_pointer` / `press_key`. The page event recorder is
//! armed when the board opens, and every "did not lift" failure reports what
//! trusted input actually reached the page, so a RED here says which of the two
//! it is: the driver delivered nothing (BROKEN) or the board did nothing with
//! what it was given (MISSING_FUNCTIONALITY). Only a drag from OUTSIDE the page
//! (a file) and "the browser starts its own drag" use the synthetic `DragEvent`
//! kit, because nothing on this page has a pointer for them (D3).
//!
//! DESIGN-PINNED HOOKS read by the oracles. If DELIVER renames one, the module,
//! the stylesheet and this file move in the SAME change:
//!   * `html[data-card-dragging]` — a card drag is in flight (DDD-7, the
//!     `closeTopLayer()` arm finds it by this marker);
//!   * `[data-card-lifted]` on the origin card, which stays in its slot (DDD-17);
//!   * the carried copy: a visible, fixed-positioned element OUTSIDE
//!     `#board-columns`, `#modal-root` and `#kb-overlay-root` whose text shows
//!     the card's key (DDD-17; its class and attributes are DELIVER's choice);
//!   * `[data-card-drop-target]` and `[data-card-drop-marker][data-before-key]`
//!     (shipped, ADR-BOARD-CARD-002).
//!
//! This module is self-contained on purpose: it seeds its own boards and keeps
//! its state in `world.cpd`, so no shipped step module is edited before DELIVER
//! re-points the shipped card-drag driver (feature-delta, "Driver re-pointing
//! plan").
//!
//! Browser scenarios are layer 4+ (Mandates 8/9/11): example-only, traditional
//! assertions, every sad path named.

use crate::support::browser_harness::{self, DragSpot, ForeignPayload, PointerKind, PointerStep};
use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use secrecy::SecretString;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const TEST_NOW: &str = "2026-03-01T12:00:00Z";
const PRIYA_EMAIL: &str = "priya.cpd@canzan.test";
const PRIYA_PASSWORD: &str = "priya-correct-horse-battery-staple";
const IDENTITY: &str = "Identity Platform";
const HOMELAB: &str = "Homelab Ops";
const BROWSER_WAIT: Duration = Duration::from_secs(10);
/// How long a lift may take to show once its gesture was dispatched. The hold
/// itself (350 ms, DDD-5) is already inside the gesture's W3C `pause`.
const LIFT_WAIT: Duration = Duration::from_millis(2500);
/// How long a dialog that is NOT supposed to open is given to open anyway.
const NO_DIALOG_WAIT: Duration = Duration::from_millis(1200);
/// The W3C `pause` that is a touch or pen hold (DDD-12: >= 400 ms against the
/// 350 ms hold of DDD-5).
const HOLD_MS: u64 = 450;
/// Longest a held-at-the-edge carry may take to scroll the board or the page
/// far enough. The lane precedent scrolls 14 px per move (EDGE_STEP); CDP touch
/// dispatch runs at ~33 ms per event (spike Q7).
const EDGE_SCROLL_WAIT: Duration = Duration::from_secs(30);

const SESSION_MARKER: &str = "html[data-card-dragging]";
const LIFTED: &str = "[data-card-lifted]";
const ACTIVATED: &str = "[data-card-drop-target]";
const MARKER: &str = "[data-card-drop-marker]";
const EDIT_DIALOG: &str = "#modal-root [data-modal=\"edit-issue\"]";
const POPUP_DELETE: &str = "[data-action=\"delete-issue\"]";
const DELETE_CONFIRM: &str = "[data-modal=\"delete-issue\"] button[type='submit']";

/// `(lane slug, key above, key below)` of a card on the live board.
type Place = (String, Option<String>, Option<String>);
/// `(lane slug, card keys in order)` for every lane on screen.
type Snapshot = Vec<(String, Vec<String>)>;

/// Per-scenario state for this feature, kept in `FoundryWorld::cpd`.
#[derive(Debug, Default)]
pub struct CpdState {
    workspace_id: Option<uuid::Uuid>,
    team_id: Option<uuid::Uuid>,
    priya_id: Option<uuid::Uuid>,
    project_ids: HashMap<String, uuid::Uuid>,
    /// Stored `(team slug, project slug)`, read back at seed time.
    project_slugs: HashMap<String, (String, String)>,
    current: Option<String>,
    /// The no-reload mark on the board document.
    mark: Option<String>,
    /// The card in Priya's hand, where it was lifted from, and its move URL.
    key: Option<String>,
    origin: Option<Place>,
    state_url: Option<String>,
    /// The pointer that holds it and where that pointer is now.
    pointer: Option<PointerKind>,
    at: (f64, f64),
    /// Move requests the page had sent before the gesture under test.
    moves_before: usize,
    /// The carried copy's top-left and the pointer, read at the lift.
    ghost_at_lift: Option<(f64, f64)>,
    pointer_at_lift: (f64, f64),
    /// `(board scrollLeft, window scrollX, window scrollY)` at the lift.
    scroll_at_lift: Option<(f64, f64, f64)>,
    /// What the scenario has PROVEN existed during the drag, so a "nothing
    /// remains" oracle can never pass over something that was never there.
    proven_carried: bool,
    proven_lit: bool,
    proven_marker: bool,
    /// Whether any card lifted during a gesture that must not lift one.
    lifted_during: Option<bool>,
    /// The marker `(lane, before-key)` read just before the release.
    marker_at_release: Option<(String, String)>,
    scroll_at_release: Option<(f64, f64, f64)>,
    target_visible_at_release: bool,
    board_before: Option<Snapshot>,
    lanes_before: Option<Vec<String>>,
    url_before: Option<String>,
    board_scroll_before: Option<f64>,
    foreign_claimed: Option<(bool, bool)>,
    native_drag_declined: Option<bool>,
    /// The recorder's account of the press-and-move, for the failure message.
    native_evidence: Option<String>,
}

// ------------------------------------------------------------------ plumbing

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse TEST_NOW")
}

async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
}

fn harness(world: &FoundryWorld) -> &InProcHarness {
    world
        .harness
        .as_ref()
        .expect("harness spawned by the Background")
}

fn pool(world: &FoundryWorld) -> PgPool {
    harness(world).app.state.store.pool().clone()
}

fn browser(world: &FoundryWorld) -> fantoccini::Client {
    world
        .browser
        .as_ref()
        .expect("a Given must have opened the board")
        .clone()
}

fn number_of(key: &str) -> i32 {
    key.rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{key:?} is not an issue key"))
}

fn keys_of(list: &str) -> Vec<String> {
    list.split(',').map(|k| k.trim().to_string()).collect()
}

fn held_key(world: &FoundryWorld) -> String {
    world
        .cpd
        .key
        .clone()
        .expect("a Given must have put a card in Priya's hand")
}

async fn js(
    client: &fantoccini::Client,
    script: &str,
    args: Vec<serde_json::Value>,
) -> serde_json::Value {
    client
        .execute(script, args)
        .await
        .unwrap_or_else(|err| panic!("board probe failed to run: {err}\n  script: {script}"))
}

// ------------------------------------------------------------------- seeding

async fn seed_project(
    world: &mut FoundryWorld,
    name: &str,
    slug: &str,
    prefix: &str,
) -> uuid::Uuid {
    let pool = pool(world);
    let ws = world.cpd.workspace_id.expect("workspace seeded first");
    let team = world.cpd.team_id.expect("team seeded first");
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(team)
    .bind(ws)
    .bind(name)
    .bind(slug)
    .bind(prefix)
    .execute(&pool)
    .await
    .unwrap_or_else(|err| panic!("insert project {name:?}: {err}"));
    world.cpd.project_ids.insert(name.to_string(), id);
    let stored: (String, String) = sqlx::query_as(
        "SELECT t.slug, p.slug FROM projects p JOIN teams t ON t.id = p.team_id WHERE p.id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .expect("read back stored slugs");
    world.cpd.project_slugs.insert(name.to_string(), stored);
    id
}

async fn seed_lane(world: &FoundryWorld, project: &str, slug: &str, label: &str, position: i32) {
    sqlx::query(
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(world.cpd.project_ids[project])
    .bind(world.cpd.workspace_id.expect("workspace seeded first"))
    .bind(slug)
    .bind(label)
    .bind(position)
    .execute(&pool(world))
    .await
    .unwrap_or_else(|err| panic!("seed lane {slug:?}: {err}"));
}

async fn seed_issues(world: &FoundryWorld, project: &str, lane: &str, keys: &[&str], from: i32) {
    for (offset, key) in keys.iter().enumerate() {
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, state, position, author_id)
                  VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(world.cpd.project_ids[project])
        .bind(world.cpd.workspace_id.expect("workspace seeded first"))
        .bind(number_of(key))
        .bind(format!("Work item {key}"))
        .bind(lane)
        .bind(from + offset as i32)
        .bind(world.cpd.priya_id.expect("Priya seeded first"))
        .execute(&pool(world))
        .await
        .unwrap_or_else(|err| panic!("seed {key} into {lane:?}: {err}"));
    }
}

// --------------------------------------------------------------- board probes

async fn lane_slug_on_screen(client: &fantoccini::Client, label: &str) -> String {
    js(
        client,
        "var lanes = document.querySelectorAll('#board-columns section.column');
         for (var i = 0; i < lanes.length; i++) {
           var h = lanes[i].querySelector('h3');
           if (h && h.textContent.trim() === arguments[0]) { return lanes[i].getAttribute('data-column'); }
         }
         return null;",
        vec![serde_json::json!(label)],
    )
    .await
    .as_str()
    .map(str::to_string)
    .unwrap_or_else(|| panic!("no lane labelled {label:?} is on the board on screen"))
}

async fn lane_labels(client: &fantoccini::Client) -> Vec<String> {
    let raw = js(
        client,
        "return Array.prototype.map.call(document.querySelectorAll('#board-columns section.column > h3'),
           function (h) { return h.textContent.trim(); });",
        vec![],
    )
    .await;
    serde_json::from_value(raw).expect("lane labels shape")
}

async fn board_snapshot(client: &fantoccini::Client) -> Snapshot {
    let raw = js(
        client,
        "return Array.prototype.map.call(document.querySelectorAll('#board-columns section.column'), function (l) {
           return [l.getAttribute('data-column'), Array.prototype.map.call(l.querySelectorAll('.issue-card'),
             function (c) { return c.getAttribute('data-issue-key'); })];
         });",
        vec![],
    )
    .await;
    serde_json::from_value(raw).expect("board snapshot shape")
}

async fn lane_keys(client: &fantoccini::Client, label: &str) -> Vec<String> {
    let slug = lane_slug_on_screen(client, label).await;
    board_snapshot(client)
        .await
        .into_iter()
        .find(|(s, _)| *s == slug)
        .map(|(_, keys)| keys)
        .unwrap_or_default()
}

async fn card_place(client: &fantoccini::Client, key: &str) -> Option<Place> {
    let raw = js(
        client,
        "var c = document.querySelector('#board-columns [data-issue-key=\"' + arguments[0] + '\"]');
         if (!c) { return null; }
         var lane = c.closest('[data-column]');
         function near(el, dir) {
           var s = el[dir];
           while (s && !(s.classList && s.classList.contains('issue-card'))) { s = s[dir]; }
           return s ? s.getAttribute('data-issue-key') : null;
         }
         return [lane ? lane.getAttribute('data-column') : '', near(c, 'previousElementSibling'), near(c, 'nextElementSibling')];",
        vec![serde_json::json!(key)],
    )
    .await;
    serde_json::from_value(raw).expect("card place shape")
}

async fn activated_lanes(client: &fantoccini::Client) -> Vec<String> {
    let raw = js(
        client,
        &format!(
            "return Array.prototype.map.call(document.querySelectorAll('{ACTIVATED}'),
               function (el) {{ return el.getAttribute('data-column') || el.tagName.toLowerCase(); }});"
        ),
        vec![],
    )
    .await;
    serde_json::from_value(raw).expect("activated lanes shape")
}

/// Every marker on the board: `(lane slug, before-key)`. An absent
/// `data-before-key` reads `"?"`, so it can never pass for `""` (the end).
async fn markers(client: &fantoccini::Client) -> Vec<(String, String)> {
    let raw = js(
        client,
        &format!(
            "return Array.prototype.map.call(document.querySelectorAll('{MARKER}'), function (m) {{
               var lane = m.closest('[data-column]'); var before = m.getAttribute('data-before-key');
               return [lane ? lane.getAttribute('data-column') : '', before === null ? '?' : before];
             }});"
        ),
        vec![],
    )
    .await;
    serde_json::from_value(raw).expect("markers shape")
}

/// What "lifted" means on the page (DDD-7, DDD-17).
#[derive(Debug, serde::Deserialize)]
struct LiftState {
    /// `html[data-card-dragging]`.
    session: bool,
    /// The origin card carries `[data-card-lifted]`.
    #[serde(rename = "originLifted")]
    origin_lifted: bool,
    /// Cards anywhere on the board marked lifted.
    #[serde(rename = "anyLifted")]
    any_lifted: u64,
    /// Visible carried copies of THIS card: `[left, top, width, height]`.
    ghosts: Vec<(f64, f64, f64, f64)>,
    /// Visible carried copies of ANY card.
    #[serde(rename = "anyGhost")]
    any_ghost: u64,
}

impl LiftState {
    fn lifted(&self) -> bool {
        self.session && self.origin_lifted && !self.ghosts.is_empty()
    }

    fn nothing_carried(&self) -> bool {
        !self.session && self.any_lifted == 0 && self.any_ghost == 0
    }
}

async fn lift_state(client: &fantoccini::Client, key: &str) -> LiftState {
    let raw = js(
        client,
        &format!(
            "var key = arguments[0];
             var board = document.getElementById('board-columns');
             var modal = document.getElementById('modal-root');
             var origin = board ? board.querySelector('[data-issue-key=\"' + key + '\"]') : null;
             var overlay = document.getElementById('kb-overlay-root');
             // A carried copy is any visible, fixed-position element outside the
             // board, the dialog host and the keyboard overlay that shows a card
             // key. Its class and attributes are DELIVER's to choose (DDD-17).
             var ghosts = [], any = 0;
             document.querySelectorAll('body *').forEach(function (el) {{
               if ((board && board.contains(el)) || (modal && modal.contains(el)) || (overlay && overlay.contains(el))) {{ return; }}
               var cs = getComputedStyle(el);
               if (cs.position !== 'fixed' || cs.visibility === 'hidden' || cs.display === 'none' || parseFloat(cs.opacity) === 0) {{ return; }}
               var r = el.getBoundingClientRect();
               if (r.width < 4 || r.height < 4) {{ return; }}
               var text = el.textContent || '';
               if (!/[A-Z][A-Z0-9]*-[0-9]+/.test(text)) {{ return; }}
               any++;
               if (text.indexOf(key) >= 0 || el.getAttribute('data-issue-key') === key) {{ ghosts.push([r.left, r.top, r.width, r.height]); }}
             }});
             return {{
               session: !!document.querySelector('{SESSION_MARKER}'),
               originLifted: !!origin && origin.matches('{LIFTED}'),
               anyLifted: board ? board.querySelectorAll('{LIFTED}').length : 0,
               ghosts: ghosts, anyGhost: any
             }};"
        ),
        vec![serde_json::json!(key)],
    )
    .await;
    serde_json::from_value(raw).expect("lift state shape")
}

/// `(board scrollLeft, window scrollX, window scrollY)`.
async fn scroll_state(client: &fantoccini::Client) -> (f64, f64, f64) {
    let raw = js(
        client,
        "var b = document.getElementById('board-columns');
         return [b ? b.scrollLeft : 0, window.scrollX, window.scrollY];",
        vec![],
    )
    .await;
    serde_json::from_value(raw).expect("scroll state shape")
}

/// The board's furthest scrollLeft.
async fn board_scroll_max(client: &fantoccini::Client) -> f64 {
    js(
        client,
        "var b = document.getElementById('board-columns'); return b ? b.scrollWidth - b.clientWidth : 0;",
        vec![],
    )
    .await
    .as_f64()
    .unwrap_or(0.0)
}

fn is_move_request(r: &browser_harness::SpiedRequest) -> bool {
    r.method.eq_ignore_ascii_case("POST") && r.url.contains("/issues/") && r.url.ends_with("/state")
}

async fn move_requests(client: &fantoccini::Client) -> Vec<browser_harness::SpiedRequest> {
    browser_harness::spied_requests(client)
        .await
        .into_iter()
        .filter(is_move_request)
        .collect()
}

async fn settle_requests(client: &fantoccini::Client) {
    let deadline = Instant::now() + BROWSER_WAIT;
    loop {
        let pending = browser_harness::spied_requests(client)
            .await
            .into_iter()
            .filter(|r| r.status.is_none())
            .count();
        if pending == 0 {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{pending} request(s) still unanswered after {BROWSER_WAIT:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn edit_dialog_title(client: &fantoccini::Client) -> Option<String> {
    js(
        client,
        &format!(
            "var d = document.querySelector('{EDIT_DIALOG}'); if (!d) {{ return null; }}
             var h = d.querySelector('h2'); return h ? h.textContent.trim() : '';"
        ),
        vec![],
    )
    .await
    .as_str()
    .map(str::to_string)
}

async fn csrf_cookie(client: &fantoccini::Client) -> String {
    js(
        client,
        "var m = document.cookie.match(/(?:^|; )foundry_csrf=([^;]*)/); return m ? decodeURIComponent(m[1]) : '';",
        vec![],
    )
    .await
    .as_str()
    .unwrap_or_default()
    .to_string()
}

// -------------------------------------------------------------- the browser

async fn open_session(world: &mut FoundryWorld, how: &str) {
    ensure_harness(world).await;
    assert!(
        world.browser.is_none(),
        "the board is opened once per scenario"
    );
    let client = match how {
        "on a phone" => browser_harness::open_mobile_session().await,
        "in a narrow window at the desk" => {
            let client = browser_harness::new_session().await;
            client
                .set_window_size(440, 900)
                .await
                .expect("narrow the desk window below the 480px breakpoint");
            client
        }
        _ => browser_harness::new_session().await,
    };
    browser_harness::sign_in_through_browser(&client, harness(world), PRIYA_EMAIL, PRIYA_PASSWORD)
        .await;
    world.browser = Some(client);
}

fn board_url(world: &FoundryWorld, project: &str) -> String {
    let (team, slug) = world
        .cpd
        .project_slugs
        .get(project)
        .unwrap_or_else(|| panic!("{project:?} must be seeded by the Background"));
    format!("{}/team/{team}/project/{slug}", harness(world).base_url())
}

/// Navigate to the board, then arm the fetch spy, the no-reload mark and the
/// pointer recorder: the recorder is armed BEFORE any gesture (DDD-12).
async fn open_board(world: &mut FoundryWorld, project: &str, how: &str) {
    open_session(world, how).await;
    world.cpd.current = Some(project.to_string());
    let client = browser(world);
    client
        .goto(&board_url(world, project))
        .await
        .expect("navigate to the board");
    browser_harness::wait_for_board_ready(&client).await;
    browser_harness::wait_for_kb_ready(&client).await;
    arm_observers(world).await;
}

async fn arm_observers(world: &mut FoundryWorld) {
    let client = browser(world);
    world.cpd.mark = Some(browser_harness::install_drag_observers(&client).await);
    browser_harness::install_pointer_recorder(&client).await;
}

async fn assert_not_reloaded(world: &FoundryWorld) {
    let mark = world
        .cpd
        .mark
        .clone()
        .expect("the board was opened by a Given");
    browser_harness::assert_not_reloaded(&browser(world), &mark).await;
}

/// Reload (only ever as an ORACLE, after the gesture), once every request is
/// answered, and re-arm the observers.
async fn reload_board(world: &mut FoundryWorld) {
    let client = browser(world);
    settle_requests(&client).await;
    client.refresh().await.expect("reload the board");
    browser_harness::wait_for_board_ready(&client).await;
    arm_observers(world).await;
}

// ------------------------------------------------------------ the gestures

fn pointer_word(kind: PointerKind) -> &'static str {
    match kind {
        PointerKind::Mouse => "mouse",
        PointerKind::Pen => "pen",
        _ => "touch pointer",
    }
}

/// Put `key` in Priya's hand: read its origin, then press it and either travel
/// past the mouse threshold or hold still for the hold (DDD-5). Returns
/// without asserting the lift, so a caller can observe either outcome.
async fn begin_gesture(world: &mut FoundryWorld, kind: PointerKind, key: &str) {
    assert_not_reloaded(world).await;
    let client = browser(world);
    let origin = card_place(&client, key).await;
    assert!(
        origin.is_some(),
        "{key} is not on the board to be picked up"
    );
    world.cpd.origin = origin;
    world.cpd.key = Some(key.to_string());
    world.cpd.pointer = Some(kind);
    world.cpd.state_url = js(
        &client,
        "var c = document.querySelector('#board-columns [data-issue-key=\"' + arguments[0] + '\"]');
         return c ? c.getAttribute('data-state-url') : null;",
        vec![serde_json::json!(key)],
    )
    .await
    .as_str()
    .map(str::to_string);
    world.cpd.moves_before = move_requests(&client).await.len();
    world.cpd.scroll_at_lift = Some(scroll_state(&client).await);
    let press = browser_harness::card_press_point(&client, key).await;
    let steps: Vec<PointerStep> = match kind {
        PointerKind::Mouse => vec![
            PointerStep::To(press.0, press.1),
            PointerStep::Down,
            PointerStep::Glide(press.0 + 8.0, press.1 + 6.0, 4),
        ],
        _ => vec![
            PointerStep::To(press.0, press.1),
            PointerStep::Down,
            PointerStep::Hold(HOLD_MS),
        ],
    };
    world.cpd.at = browser_harness::perform_pointer(&client, kind, press, &steps).await;
}

/// Wait for `key` to show as lifted, or panic with the classification the
/// recorder supports: no trusted input at all is the DRIVER (BROKEN); trusted
/// input with no lift is the missing feature (RED).
async fn await_lift(client: &fantoccini::Client, kind: PointerKind, key: &str) -> LiftState {
    let deadline = Instant::now() + LIFT_WAIT;
    loop {
        let state = lift_state(client, key).await;
        if state.lifted() {
            return state;
        }
        if Instant::now() > deadline {
            let record = browser_harness::pointer_record(client).await;
            let wanted = match kind {
                PointerKind::Mouse => "a primary press moved past the 6 px threshold",
                _ => "a press held still for the 350 ms hold",
            };
            if record.trusted("pointerdown") == 0 {
                panic!(
                    "BROKEN(driver): no trusted pointerdown reached the page for the {} \
                     gesture on {key}, so nothing about the board can be concluded. {}",
                    pointer_word(kind),
                    record.describe()
                );
            }
            panic!(
                "MISSING_FUNCTIONALITY: {key} did not lift after {wanted} with a {} (US-CPD-0{}, \
                 DDD-5/7/17). Drag in flight on the page = {}, origin marked lifted = {}, carried \
                 copies = {}. The driver DID deliver trusted input: {}",
                pointer_word(kind),
                if kind == PointerKind::Mouse { 1 } else { 2 },
                state.session,
                state.origin_lifted,
                state.ghosts.len(),
                record.describe()
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Lift `key` with `kind` and prove it: a carried copy exists from here on.
async fn lift(world: &mut FoundryWorld, kind: PointerKind, key: &str) {
    begin_gesture(world, kind, key).await;
    let client = browser(world);
    let state = await_lift(&client, kind, key).await;
    world.cpd.proven_carried = true;
    world.cpd.ghost_at_lift = state.ghosts.first().map(|g| (g.0, g.1));
    world.cpd.pointer_at_lift = world.cpd.at;
}

/// A point in the lane of `spot` that a pointer can hold without starting an
/// edge auto-scroll: the spot's own point, moved horizontally inside the
/// visible part of its lane and at least 60 px from the board's edges.
async fn carry_point(client: &fantoccini::Client, spot: DragSpot<'_>, lane: &str) -> (f64, f64) {
    let (x, y) = browser_harness::spot_point(client, spot).await;
    let raw = js(
        client,
        "var b = document.getElementById('board-columns').getBoundingClientRect();
         var l = document.querySelector('#board-columns [data-column=\"' + arguments[0] + '\"]').getBoundingClientRect();
         var lo = Math.max(l.left + 10, b.left + 60, 10), hi = Math.min(l.right - 10, b.right - 60, window.innerWidth - 10);
         return [lo, hi];",
        vec![serde_json::json!(lane)],
    )
    .await;
    let (lo, hi): (f64, f64) = serde_json::from_value(raw).expect("carry bounds shape");
    let x = if lo <= hi { x.clamp(lo, hi) } else { x };
    (x, y)
}

async fn carry_to(world: &mut FoundryWorld, to: (f64, f64)) {
    let client = browser(world);
    let kind = world.cpd.pointer.expect("a card is in Priya's hand");
    let n = if kind == PointerKind::Mouse { 8 } else { 10 };
    world.cpd.at = browser_harness::perform_pointer(
        &client,
        kind,
        world.cpd.at,
        &[PointerStep::Glide(to.0, to.1, n)],
    )
    .await;
}

async fn release(world: &mut FoundryWorld) {
    let client = browser(world);
    let kind = world.cpd.pointer.expect("a card is in Priya's hand");
    browser_harness::perform_pointer(&client, kind, world.cpd.at, &[PointerStep::Up]).await;
    settle_requests(&client).await;
}

async fn carry_between(world: &mut FoundryWorld, above: &str, below: &str) {
    let client = browser(world);
    let lane = card_place(&client, below)
        .await
        .unwrap_or_else(|| panic!("{below} is not on the board"))
        .0;
    let to = carry_point(&client, DragSpot::Between(above, below), &lane).await;
    carry_to(world, to).await;
}

async fn carry_to_lane_end(world: &mut FoundryWorld, label: &str) {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, label).await;
    let to = carry_point(&client, DragSpot::LaneEnd(&slug), &slug).await;
    carry_to(world, to).await;
}

async fn carry_to_lane_top(world: &mut FoundryWorld, label: &str) {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, label).await;
    let to = carry_point(&client, DragSpot::LaneTop(&slug), &slug).await;
    carry_to(world, to).await;
}

/// Wait for `key` to sit in `label`, or panic naming where it is.
async fn await_card_in(client: &fantoccini::Client, key: &str, label: &str) {
    let slug = lane_slug_on_screen(client, label).await;
    let deadline = Instant::now() + BROWSER_WAIT;
    loop {
        let place = card_place(client, key).await;
        if place.as_ref().is_some_and(|p| p.0 == slug) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "MISSING_FUNCTIONALITY: {key} did not land in {label}; it is at {place:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Assert the carried card, the lit lane and the marker EXIST now, and record
/// that they did, so a later "none remains" oracle is discriminating.
async fn prove_carried_over(world: &mut FoundryWorld, label: &str) {
    let client = browser(world);
    let key = held_key(world);
    let slug = lane_slug_on_screen(&client, label).await;
    let state = lift_state(&client, &key).await;
    assert!(
        state.lifted(),
        "MISSING_FUNCTIONALITY: {key} is not carried while over {label}: {state:?}"
    );
    let lit = activated_lanes(&client).await;
    assert_eq!(
        lit,
        vec![slug.clone()],
        "MISSING_FUNCTIONALITY: {label} is not the one lane lit up under the carried {key}"
    );
    let shown = markers(&client).await;
    assert!(
        shown.len() == 1 && shown[0].0 == slug,
        "MISSING_FUNCTIONALITY: exactly one marker must show in {label} under the carried {key}; \
         found {shown:?}"
    );
    world.cpd.proven_carried = true;
    world.cpd.proven_lit = true;
    world.cpd.proven_marker = true;
}

async fn assert_nothing_left(world: &FoundryWorld) {
    let client = browser(world);
    assert!(
        world.cpd.proven_carried && world.cpd.proven_lit && world.cpd.proven_marker,
        "this oracle is only meaningful after the scenario proved the carried card, the lit lane \
         and the marker existed; the Given did not (carried {}, lit {}, marker {})",
        world.cpd.proven_carried,
        world.cpd.proven_lit,
        world.cpd.proven_marker
    );
    let key = held_key(world);
    let state = lift_state(&client, &key).await;
    assert!(
        state.nothing_carried(),
        "MISSING_FUNCTIONALITY: a carried card is left behind after the drag ended: {state:?}"
    );
    assert_eq!(
        activated_lanes(&client).await,
        Vec::<String>::new(),
        "MISSING_FUNCTIONALITY: a lane is still lit after the drag ended"
    );
    assert_eq!(
        markers(&client).await,
        Vec::<(String, String)>::new(),
        "MISSING_FUNCTIONALITY: a marker is still shown after the drag ended"
    );
    settle_requests(&client).await;
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "no move request may be sent when the drag does not land"
    );
}

async fn assert_back_at_origin(world: &FoundryWorld, key: &str) {
    let client = browser(world);
    let origin = world
        .cpd
        .origin
        .clone()
        .expect("the origin read at the lift");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let place = card_place(&client, key).await;
        if place.as_ref() == Some(&origin) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "MISSING_FUNCTIONALITY: {key} is not back in its exact slot: it was at {origin:?} \
             when lifted and is at {place:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Carry the held card to the board's right edge and keep holding there until
/// `done` says so. Panics as MISSING_FUNCTIONALITY when the board never
/// scrolls far enough.
async fn hold_at_right_edge<F>(world: &mut FoundryWorld, what: &str, mut done: F)
where
    F: FnMut(f64, f64) -> bool,
{
    let client = browser(world);
    let kind = world.cpd.pointer.expect("a card is in Priya's hand");
    let raw = js(
        &client,
        "var b = document.getElementById('board-columns').getBoundingClientRect();
         return Math.min(b.right, window.innerWidth) - 12;",
        vec![],
    )
    .await;
    let edge_x = raw.as_f64().expect("board edge");
    let y = world.cpd.at.1;
    world.cpd.at = browser_harness::perform_pointer(
        &client,
        kind,
        world.cpd.at,
        &[PointerStep::Glide(edge_x, y, 6)],
    )
    .await;
    let deadline = Instant::now() + EDGE_SCROLL_WAIT;
    let start = scroll_state(&client).await.0;
    loop {
        let now = scroll_state(&client).await.0;
        let max = board_scroll_max(&client).await;
        if done(now, max) {
            return;
        }
        if Instant::now() > deadline {
            panic!(
                "MISSING_FUNCTIONALITY: holding the carried card at the board's right edge did \
                 not scroll the board until {what} (US-CPD-03, D14/DDD-11): scrollLeft went from \
                 {start} to {now} of {max}"
            );
        }
        browser_harness::perform_pointer(&client, kind, world.cpd.at, &[PointerStep::Jitter(600)])
            .await;
    }
}

async fn lane_fully_visible(client: &fantoccini::Client, slug: &str) -> bool {
    js(
        client,
        "var b = document.getElementById('board-columns').getBoundingClientRect();
         var l = document.querySelector('#board-columns [data-column=\"' + arguments[0] + '\"]');
         if (!l) { return false; } var r = l.getBoundingClientRect();
         return r.left >= Math.max(b.left, 0) - 1 && r.right <= Math.min(b.right, window.innerWidth) + 1;",
        vec![serde_json::json!(slug)],
    )
    .await
    .as_bool()
    .unwrap_or(false)
}

/// Read the marker, then release: the "marker = landing" pair.
async fn release_recording_marker(world: &mut FoundryWorld) {
    let client = browser(world);
    let shown = markers(&client).await;
    assert_eq!(
        shown.len(),
        1,
        "MISSING_FUNCTIONALITY: exactly one marker must show just before the release; found {shown:?}"
    );
    world.cpd.marker_at_release = shown.into_iter().next();
    world.cpd.scroll_at_release = Some(scroll_state(&client).await);
    release(world).await;
}

async fn popup_delete(world: &mut FoundryWorld, key: &str) {
    let client = browser(world);
    js(
        &client,
        "document.getElementById('board-columns').__cpdStamp = true; return true;",
        vec![],
    )
    .await;
    client
        .wait()
        .at_most(BROWSER_WAIT)
        .for_element(Locator::Css(&format!(
            "#board-columns [data-issue-key='{key}']"
        )))
        .await
        .unwrap_or_else(|_| panic!("{key} must be on the board to open its popup"))
        .click()
        .await
        .expect("open the card's popup");
    for (selector, what) in [(POPUP_DELETE, "Delete"), (DELETE_CONFIRM, "the confirm")] {
        client
            .wait()
            .at_most(BROWSER_WAIT)
            .for_element(Locator::Css(selector))
            .await
            .unwrap_or_else(|_| panic!("the popup offers {what} (shipped, issue-card-delete)"))
            .click()
            .await
            .unwrap_or_else(|err| panic!("choose {what}: {err}"));
    }
    let deadline = Instant::now() + BROWSER_WAIT;
    loop {
        let replaced = js(
            &client,
            "var b = document.getElementById('board-columns'); return !!b && b.__cpdStamp !== true;",
            vec![],
        )
        .await
        .as_bool()
        .unwrap_or(false);
        if replaced && card_place(&client, key).await.is_none() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the popup delete of {key} did not refresh the board in place within {BROWSER_WAIT:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_not_reloaded(world).await;
}

// ============================================================== Background

#[given(
    regex = r"^Priya keeps two boards for pointer dragging: Identity Platform with lanes Backlog, In-Progress and Done, and Homelab Ops with lanes Backlog, Staging, In-Progress and Done$"
)]
async fn given_two_boards(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let pool = pool(world);
    let ws = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(ws)
        .bind("Canzan Labs")
        .execute(&pool)
        .await
        .expect("insert workspace");
    world.cpd.workspace_id = Some(ws);
    let team = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team)
        .bind(ws)
        .bind("Platform")
        .bind("platform")
        .execute(&pool)
        .await
        .expect("insert team");
    world.cpd.team_id = Some(team);
    let hash = foundry_auth::hash_password(&SecretString::new(PRIYA_PASSWORD.to_string().into()))
        .await
        .expect("hash password");
    let priya = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(priya)
    .bind(PRIYA_EMAIL)
    .bind(PRIYA_EMAIL)
    .bind("Priya Raman")
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("insert Priya");
    world.cpd.priya_id = Some(priya);
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(ws)
    .bind(priya)
    .execute(&pool)
    .await
    .expect("Priya's workspace membership");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')")
        .bind(team)
        .bind(priya)
        .execute(&pool)
        .await
        .expect("Priya's team membership");

    seed_project(world, IDENTITY, "identity-platform", "AUTH").await;
    for (i, (slug, label)) in [
        ("backlog", "Backlog"),
        ("in_progress", "In-Progress"),
        ("done", "Done"),
    ]
    .iter()
    .enumerate()
    {
        seed_lane(world, IDENTITY, slug, label, i as i32).await;
    }
    seed_project(world, HOMELAB, "homelab-ops", "OPS").await;
    for (i, (slug, label)) in [
        ("backlog", "Backlog"),
        ("staging", "Staging"),
        ("in_progress", "In-Progress"),
        ("done", "Done"),
    ]
    .iter()
    .enumerate()
    {
        seed_lane(world, HOMELAB, slug, label, i as i32).await;
    }
}

#[given(
    regex = r"^on Identity Platform, Backlog holds AUTH-41, AUTH-42 and AUTH-43, In-Progress holds AUTH-3, AUTH-12 and AUTH-19 in that order, and Done holds AUTH-7$"
)]
async fn given_identity_cards(world: &mut FoundryWorld) {
    seed_issues(
        world,
        IDENTITY,
        "backlog",
        &["AUTH-41", "AUTH-42", "AUTH-43"],
        0,
    )
    .await;
    seed_issues(
        world,
        IDENTITY,
        "in_progress",
        &["AUTH-3", "AUTH-12", "AUTH-19"],
        0,
    )
    .await;
    seed_issues(world, IDENTITY, "done", &["AUTH-7"], 0).await;
}

#[given(
    regex = r"^on Homelab Ops, Backlog holds OPS-3, Staging is empty, In-Progress holds OPS-7 and Done holds OPS-9$"
)]
async fn given_homelab_cards(world: &mut FoundryWorld) {
    seed_issues(world, HOMELAB, "backlog", &["OPS-3"], 0).await;
    seed_issues(world, HOMELAB, "in_progress", &["OPS-7"], 0).await;
    seed_issues(world, HOMELAB, "done", &["OPS-9"], 0).await;
}

// ================================================================== Given

#[given(
    regex = r"^(the Identity Platform board|Homelab Ops) is open (at the desk|on a phone|for a pen)$"
)]
async fn given_open(world: &mut FoundryWorld, which: String, how: String) {
    let project = if which == "Homelab Ops" {
        HOMELAB
    } else {
        IDENTITY
    };
    open_board(world, project, &how).await;
}

#[given(
    regex = r"^Homelab Ops has eight lanes and is open (on a phone|in a narrow window at the desk)$"
)]
async fn given_eight_lanes(world: &mut FoundryWorld, how: String) {
    let pool = pool(world);
    sqlx::query("UPDATE lanes SET position = 7 WHERE project_id = $1 AND slug = 'done'")
        .bind(world.cpd.project_ids[HOMELAB])
        .execute(&pool)
        .await
        .expect("move Done to the far end");
    for (i, (slug, label)) in [
        ("review", "Review"),
        ("blocked", "Blocked"),
        ("qa", "QA"),
        ("released", "Released"),
    ]
    .iter()
    .enumerate()
    {
        seed_lane(world, HOMELAB, slug, label, 3 + i as i32).await;
    }
    open_board(world, HOMELAB, &how).await;
    let client = browser(world);
    let done = lane_slug_on_screen(&client, "Done").await;
    assert!(
        !lane_fully_visible(&client, &done).await && board_scroll_max(&client).await > 0.0,
        "precondition: Done must start off-screen on a board that scrolls sideways"
    );
}

#[given(
    regex = r"^Identity Platform's Backlog runs on from AUTH-43 through AUTH-60, twenty cards in all$"
)]
async fn given_long_backlog(world: &mut FoundryWorld) {
    let keys: Vec<String> = (44..=60).map(|n| format!("AUTH-{n}")).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    seed_issues(world, IDENTITY, "backlog", &keys, 3).await;
}

#[given(regex = r"^Priya has lifted (\w+-\d+) with the mouse$")]
async fn given_lifted_mouse(world: &mut FoundryWorld, key: String) {
    lift(world, PointerKind::Mouse, &key).await;
}

#[given(regex = r"^Priya has lifted (\w+-\d+) by holding it with a (touch pointer|pen)$")]
async fn given_lifted_hold(world: &mut FoundryWorld, key: String, with: String) {
    let kind = if with == "pen" {
        PointerKind::Pen
    } else {
        PointerKind::Touch
    };
    lift(world, kind, &key).await;
}

async fn drag_into(world: &mut FoundryWorld, kind: PointerKind, key: &str, label: &str) {
    lift(world, kind, key).await;
    carry_to_lane_end(world, label).await;
    release(world).await;
    await_card_in(&browser(world), key, label).await;
}

#[given(regex = r"^Priya has just dragged (\w+-\d+) into ([\w-]+) with the mouse$")]
async fn given_just_dragged(world: &mut FoundryWorld, key: String, label: String) {
    drag_into(world, PointerKind::Mouse, &key, &label).await;
}

#[given(regex = r"^Priya has just carried (\w+-\d+) into ([\w-]+) by touch$")]
async fn given_just_carried(world: &mut FoundryWorld, key: String, label: String) {
    drag_into(world, PointerKind::Touch, &key, &label).await;
}

#[given(
    regex = r"^Priya is (?:dragging|carrying) (\w+-\d+) over ([\w-]+) with (the mouse|a touch pointer)$"
)]
async fn given_carrying_over(world: &mut FoundryWorld, key: String, label: String, with: String) {
    let kind = if with == "the mouse" {
        PointerKind::Mouse
    } else {
        PointerKind::Touch
    };
    lift(world, kind, &key).await;
    carry_to_lane_end(world, &label).await;
    prove_carried_over(world, &label).await;
}

#[given(regex = r"^she has carried it over ([\w-]+) between (\w+-\d+) and (\w+-\d+)$")]
async fn given_carried_between(
    world: &mut FoundryWorld,
    label: String,
    above: String,
    below: String,
) {
    carry_between(world, &above, &below).await;
    prove_carried_over(world, &label).await;
}

#[given(regex = r"^she has carried it to the right edge until the board scrolled to its end$")]
async fn given_carried_to_end(world: &mut FoundryWorld) {
    hold_at_right_edge(world, "its end", |now, max| max > 0.0 && now >= max - 1.0).await;
}

#[given(
    regex = r"^Priya has deleted (\w+-\d+) from its popup and the board refreshed without reloading$"
)]
async fn given_popup_deleted(world: &mut FoundryWorld, key: String) {
    popup_delete(world, &key).await;
}

#[given(
    regex = r"^Priya has put a touch pointer on (\w+-\d+) without holding it long enough to lift$"
)]
async fn given_touch_down_briefly(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    world.cpd.origin = card_place(&client, &key).await;
    world.cpd.key = Some(key.clone());
    world.cpd.pointer = Some(PointerKind::Touch);
    world.cpd.moves_before = move_requests(&client).await.len();
    let press = browser_harness::card_press_point(&client, &key).await;
    world.cpd.at = browser_harness::perform_pointer(
        &client,
        PointerKind::Touch,
        press,
        &[
            PointerStep::To(press.0, press.1),
            PointerStep::Down,
            PointerStep::Hold(100),
        ],
    )
    .await;
}

#[given(regex = r"^(\w+-\d+) was deleted elsewhere after Priya's board loaded$")]
async fn given_deleted_elsewhere(world: &mut FoundryWorld, key: String) {
    let project = if key.starts_with("AUTH-") {
        IDENTITY
    } else {
        HOMELAB
    };
    let done = sqlx::query("DELETE FROM issues WHERE project_id = $1 AND number = $2")
        .bind(world.cpd.project_ids[project])
        .bind(number_of(&key))
        .execute(&pool(world))
        .await
        .unwrap_or_else(|err| panic!("delete {key} behind Priya's back: {err}"));
    assert_eq!(
        done.rows_affected(),
        1,
        "{key} must have existed to be deleted"
    );
}

// =================================================================== When

#[when(
    regex = r"^Priya drags (\w+-\d+) with the mouse between (\w+-\d+) and (\w+-\d+) and releases it$"
)]
async fn when_drag_between(world: &mut FoundryWorld, key: String, above: String, below: String) {
    lift(world, PointerKind::Mouse, &key).await;
    carry_between(world, &above, &below).await;
    release(world).await;
}

#[when(regex = r"^Priya drags (\w+-\d+) with the mouse to the top of ([\w-]+) and releases it$")]
async fn when_drag_top(world: &mut FoundryWorld, key: String, label: String) {
    lift(world, PointerKind::Mouse, &key).await;
    carry_to_lane_top(world, &label).await;
    release(world).await;
}

#[when(regex = r"^Priya drags (\w+-\d+) with the mouse into ([\w-]+) and releases it$")]
async fn when_drag_into(world: &mut FoundryWorld, key: String, label: String) {
    lift(world, PointerKind::Mouse, &key).await;
    carry_to_lane_end(world, &label).await;
    release(world).await;
}

#[when(regex = r"^she carries it over ([\w-]+) between (\w+-\d+) and (\w+-\d+)$")]
async fn when_carry_between(
    world: &mut FoundryWorld,
    _label: String,
    above: String,
    below: String,
) {
    carry_between(world, &above, &below).await;
}

#[when(regex = r"^she carries it between (\w+-\d+) and (\w+-\d+) and lifts her finger$")]
async fn when_carry_between_release(world: &mut FoundryWorld, above: String, below: String) {
    carry_between(world, &above, &below).await;
    note_feedback_shown(world).await;
    release(world).await;
}

/// Record (never assert) whether the lit lane and the marker were on screen
/// just before a release, so a later "none remains" oracle can require it.
async fn note_feedback_shown(world: &mut FoundryWorld) {
    let client = browser(world);
    world.cpd.proven_lit = activated_lanes(&client).await.len() == 1;
    world.cpd.proven_marker = markers(&client).await.len() == 1;
}

#[when(regex = r"^she carries it to the top of ([\w-]+) and lifts the pen$")]
async fn when_pen_to_top(world: &mut FoundryWorld, label: String) {
    carry_to_lane_top(world, &label).await;
    release(world).await;
}

#[when(regex = r"^she releases it back over (\w+-\d+)$")]
async fn when_release_over_own(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    let to = browser_harness::card_press_point(&client, &key).await;
    carry_to(world, to).await;
    release(world).await;
}

#[when(regex = r"^she lifts her finger without moving it$")]
async fn when_release_in_place(world: &mut FoundryWorld) {
    release(world).await;
}

#[when(regex = r"^she presses (\w+-\d+), moves the mouse 3 pixels and releases$")]
async fn when_press_3px(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    world.cpd.moves_before = move_requests(&client).await.len();
    let p = browser_harness::card_press_point(&client, &key).await;
    browser_harness::perform_pointer(
        &client,
        PointerKind::Mouse,
        p,
        &[
            PointerStep::To(p.0, p.1),
            PointerStep::Down,
            PointerStep::Glide(p.0 + 3.0, p.1, 3),
            PointerStep::Up,
        ],
    )
    .await;
}

#[when(regex = r"^she taps (\w+-\d+)$")]
async fn when_tap(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    world.cpd.moves_before = move_requests(&client).await.len();
    let p = browser_harness::card_press_point(&client, &key).await;
    browser_harness::perform_pointer(
        &client,
        PointerKind::RetryTouch,
        p,
        &[
            PointerStep::To(p.0, p.1),
            PointerStep::Down,
            PointerStep::Hold(80),
            PointerStep::Up,
        ],
    )
    .await;
}

#[when(regex = r"^she presses Escape on the keyboard$")]
async fn when_escape(world: &mut FoundryWorld) {
    browser_harness::press_key(&browser(world), "Escape").await;
}

#[when(regex = r"^Priya presses (\w+-\d+) with the right mouse button and moves it into ([\w-]+)$")]
async fn when_right_button(world: &mut FoundryWorld, key: String, label: String) {
    let client = browser(world);
    world.cpd.key = Some(key.clone());
    world.cpd.origin = card_place(&client, &key).await;
    world.cpd.moves_before = move_requests(&client).await.len();
    let slug = lane_slug_on_screen(&client, &label).await;
    let p = browser_harness::card_press_point(&client, &key).await;
    let to = browser_harness::spot_point(&client, DragSpot::LaneEnd(&slug)).await;
    let at = browser_harness::perform_pointer(
        &client,
        PointerKind::Mouse,
        p,
        &[
            PointerStep::To(p.0, p.1),
            PointerStep::DownSecondary,
            PointerStep::Glide(to.0, to.1, 8),
        ],
    )
    .await;
    world.cpd.lifted_during =
        Some(lift_state(&client, &key).await.session || !activated_lanes(&client).await.is_empty());
    browser_harness::perform_pointer(&client, PointerKind::Mouse, at, &[PointerStep::UpSecondary])
        .await;
    settle_requests(&client).await;
}

#[when(
    regex = r"^Priya drags (\w+-\d+) with the mouse from ([\w-]+) across the ([\w-]+) header and releases it in ([\w-]+)$"
)]
async fn when_drag_across_header(
    world: &mut FoundryWorld,
    key: String,
    _from: String,
    header: String,
    into: String,
) {
    let client = browser(world);
    world.cpd.lanes_before = Some(lane_labels(&client).await);
    lift(world, PointerKind::Mouse, &key).await;
    let slug = lane_slug_on_screen(&client, &header).await;
    let over_header = js(
        &client,
        "var h = document.querySelector('#board-columns [data-lane-drag=\"' + arguments[0] + '\"]').getBoundingClientRect();
         return [h.left + h.width / 2, h.top + h.height / 2];",
        vec![serde_json::json!(slug)],
    )
    .await;
    let over_header: (f64, f64) = serde_json::from_value(over_header).expect("header point");
    carry_to(world, over_header).await;
    carry_to_lane_end(world, &into).await;
    release(world).await;
}

#[when(regex = r"^Priya drags the ([\w-]+) header with the mouse to the left of ([\w-]+)$")]
async fn when_drag_header(world: &mut FoundryWorld, label: String, target: String) {
    let client = browser(world);
    world.cpd.board_before = Some(board_snapshot(&client).await);
    world.cpd.moves_before = move_requests(&client).await.len();
    let from = lane_slug_on_screen(&client, &label).await;
    let to = lane_slug_on_screen(&client, &target).await;
    let raw = js(
        &client,
        "var a = document.querySelector('#board-columns [data-lane-drag=\"' + arguments[0] + '\"]').getBoundingClientRect();
         var b = document.querySelector('#board-columns [data-column=\"' + arguments[1] + '\"]').getBoundingClientRect();
         return [a.left + a.width / 2, a.top + a.height / 2, b.left + b.width * 0.1];",
        vec![serde_json::json!(from), serde_json::json!(to)],
    )
    .await;
    let (x, y, drop_x): (f64, f64, f64) = serde_json::from_value(raw).expect("header drag points");
    let at = browser_harness::perform_pointer(
        &client,
        PointerKind::Mouse,
        (x, y),
        &[
            PointerStep::To(x, y),
            PointerStep::Down,
            PointerStep::Glide(drop_x, y, 10),
        ],
    )
    .await;
    let state = lift_state(&client, "").await;
    world.cpd.lifted_during = Some(state.session || state.any_lifted > 0 || state.any_ghost > 0);
    browser_harness::perform_pointer(&client, PointerKind::Mouse, at, &[PointerStep::Up]).await;
    settle_requests(&client).await;
}

/// A trusted mouse press-and-move on a card, far enough for Chrome to begin
/// its OWN drag of a `draggable` element (measured: W3C mouse actions do start
/// a real HTML5 drag in this lane's Chrome, which then takes the pointer away
/// with `pointercancel`, spike Q1 variant A).
#[when(regex = r"^Priya presses (\w+-\d+) with the mouse and moves it 30 pixels$")]
async fn when_press_move_30(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    world.cpd.key = Some(key.clone());
    let p = browser_harness::card_press_point(&client, &key).await;
    let at = browser_harness::perform_pointer(
        &client,
        PointerKind::Mouse,
        p,
        &[
            PointerStep::To(p.0, p.1),
            PointerStep::Down,
            PointerStep::Glide(p.0 + 30.0, p.1, 6),
        ],
    )
    .await;
    let record = browser_harness::pointer_record(&client).await;
    let attempts: Vec<&serde_json::Value> = record
        .native_drags
        .iter()
        .filter(|d| d["key"].as_str() == Some(key.as_str()))
        .collect();
    world.cpd.native_drag_declined = Some(
        !attempts.is_empty()
            && attempts
                .iter()
                .all(|d| d["cancelled"].as_bool() == Some(true))
            && record.trusted("pointercancel") == 0,
    );
    world.cpd.native_evidence = Some(record.describe());
    browser_harness::press_key(&client, "Escape").await;
    browser_harness::perform_pointer(&client, PointerKind::Mouse, at, &[PointerStep::Up]).await;
    settle_requests(&client).await;
}

#[when(regex = r#"^a file "([^"]+)" from the desktop is dropped on ([\w-]+)$"#)]
async fn when_file_dropped(world: &mut FoundryWorld, name: String, label: String) {
    let client = browser(world);
    world.cpd.board_before = Some(board_snapshot(&client).await);
    world.cpd.moves_before = move_requests(&client).await.len();
    world.cpd.url_before = Some(client.current_url().await.expect("url").to_string());
    let slug = lane_slug_on_screen(&client, &label).await;
    browser_harness::drag_start_foreign(&client, ForeignPayload::File(&name)).await;
    let over = browser_harness::drag_over(&client, DragSpot::LaneEnd(&slug)).await;
    let dropped = browser_harness::drag_drop(&client, DragSpot::SamePoint).await;
    world.cpd.foreign_claimed = Some((over, dropped));
    settle_requests(&client).await;
}

#[when(
    regex = r"^Priya puts a touch pointer on (\w+-\d+) and swipes left before the hold completes$"
)]
async fn when_swipe(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    world.cpd.key = Some(key.clone());
    world.cpd.origin = card_place(&client, &key).await;
    world.cpd.moves_before = move_requests(&client).await.len();
    world.cpd.board_scroll_before = Some(scroll_state(&client).await.0);
    let p = browser_harness::card_press_point(&client, &key).await;
    let at = browser_harness::perform_pointer(
        &client,
        PointerKind::Touch,
        p,
        &[
            PointerStep::To(p.0, p.1),
            PointerStep::Down,
            PointerStep::Glide(p.0 - 150.0, p.1, 6),
        ],
    )
    .await;
    let early = lift_state(&client, &key).await;
    // Stay down past the hold: a hold rule that ignored the tolerance would
    // lift now, with the finger still on the glass.
    browser_harness::perform_pointer(
        &client,
        PointerKind::Touch,
        at,
        &[PointerStep::Hold(HOLD_MS)],
    )
    .await;
    let late = lift_state(&client, &key).await;
    world.cpd.lifted_during = Some(early.session || late.session || !late.ghosts.is_empty());
    browser_harness::perform_pointer(&client, PointerKind::Touch, at, &[PointerStep::Up]).await;
    settle_requests(&client).await;
}

#[when(regex = r"^a second finger goes down on ([\w-]+) and moves$")]
async fn when_second_finger(world: &mut FoundryWorld, label: String) {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, &label).await;
    let first = board_snapshot(&client)
        .await
        .into_iter()
        .find(|(s, _)| *s == slug)
        .and_then(|(_, keys)| keys.into_iter().find(|k| Some(k) != world.cpd.key.as_ref()))
        .unwrap_or_else(|| panic!("{label} holds no other card to put a second finger on"));
    let p = browser_harness::card_press_point(&client, &first).await;
    browser_harness::perform_pointer(
        &client,
        PointerKind::SecondTouch,
        p,
        &[
            PointerStep::To(p.0, p.1),
            PointerStep::Down,
            PointerStep::Glide(p.0, p.1 + 60.0, 5),
            PointerStep::Up,
        ],
    )
    .await;
}

#[when(
    regex = r"^she holds it at the right edge of the board until ([\w-]+) is in view and releases it over ([\w-]+)$"
)]
async fn when_edge_scroll_and_drop(world: &mut FoundryWorld, until: String, over: String) {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, &until).await;
    hold_at_right_edge(world, &format!("{until} was in view"), |now, max| {
        now >= max - 1.0
    })
    .await;
    assert!(
        lane_fully_visible(&client, &slug).await,
        "MISSING_FUNCTIONALITY: {until} never came fully into view while the carried card was \
         held at the board's edge (AC-3.1)"
    );
    world.cpd.target_visible_at_release = true;
    carry_to_lane_end(world, &over).await;
    release_recording_marker(world).await;
}

#[when(regex = r"^she keeps holding it at the edge$")]
async fn when_keep_holding(world: &mut FoundryWorld) {
    let client = browser(world);
    world.cpd.scroll_at_release = Some(scroll_state(&client).await);
    let kind = world.cpd.pointer.expect("a card is in Priya's hand");
    browser_harness::perform_pointer(&client, kind, world.cpd.at, &[PointerStep::Jitter(1500)])
        .await;
}

#[when(
    regex = r"^she holds it near the bottom of the screen until (\w+-\d+) is in view and lets go below it$"
)]
async fn when_hold_bottom(world: &mut FoundryWorld, last: String) {
    let client = browser(world);
    let kind = world.cpd.pointer.expect("a card is in Priya's hand");
    let (_, vh) = browser_harness::viewport(&client).await;
    let x = world.cpd.at.0;
    world.cpd.at = browser_harness::perform_pointer(
        &client,
        kind,
        world.cpd.at,
        &[PointerStep::Glide(x, vh - 12.0, 8)],
    )
    .await;
    let deadline = Instant::now() + EDGE_SCROLL_WAIT;
    loop {
        let bottom = js(
            &client,
            "var c = document.querySelector('#board-columns [data-issue-key=\"' + arguments[0] + '\"]');
             return c ? c.getBoundingClientRect().bottom : 1e9;",
            vec![serde_json::json!(last)],
        )
        .await
        .as_f64()
        .unwrap_or(1e9);
        if bottom <= vh - 24.0 {
            break;
        }
        if Instant::now() > deadline {
            let scroll = scroll_state(&client).await;
            panic!(
                "MISSING_FUNCTIONALITY: holding the carried card near the bottom of the screen \
                 did not scroll the page until {last} was in view (US-CPD-03, DDD-11): page \
                 scrollY is {}, {last}'s bottom is at {bottom} of a {vh} px viewport",
                scroll.2
            );
        }
        browser_harness::perform_pointer(&client, kind, world.cpd.at, &[PointerStep::Jitter(600)])
            .await;
    }
    world.cpd.target_visible_at_release = true;
    let lane = card_place(&client, &last).await.expect("the last card").0;
    let to = carry_point(&client, DragSpot::Below(&last), &lane).await;
    carry_to(world, to).await;
    release_recording_marker(world).await;
}

#[when(regex = r"^an incoming call takes the touch away from Priya$")]
async fn when_system_cancels(world: &mut FoundryWorld) {
    browser_harness::system_cancels_touch(&browser(world)).await;
}

#[when(regex = r"^she lifts her finger over the page header$")]
async fn when_release_over_header(world: &mut FoundryWorld) {
    let client = browser(world);
    let to = browser_harness::spot_point(&client, DragSpot::PageHeader).await;
    carry_to(world, to).await;
    release(world).await;
}

// =================================================================== Then

#[then(regex = r"^([\w-]+) now reads ((?:\w+-\d+)(?:, \w+-\d+)*)$")]
async fn then_lane_now_reads(world: &mut FoundryWorld, label: String, list: String) {
    let client = browser(world);
    let want = keys_of(&list);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let got = lane_keys(&client, &label).await;
        if got == want {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "MISSING_FUNCTIONALITY: {label} must read {want:?} on screen; it reads {got:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[then(regex = r"^a reload shows ([\w-]+) as ((?:\w+-\d+)(?:, \w+-\d+)*)$")]
async fn then_reload_shows(world: &mut FoundryWorld, label: String, list: String) {
    reload_board(world).await;
    assert_eq!(
        lane_keys(&browser(world), &label).await,
        keys_of(&list),
        "{label} after a reload (the move must have persisted)"
    );
}

#[then(regex = r"^a reload shows (\w+-\d+) last in ([\w-]+), after (\w+-\d+)$")]
async fn then_reload_last(world: &mut FoundryWorld, key: String, label: String, after: String) {
    reload_board(world).await;
    let keys = lane_keys(&browser(world), &label).await;
    assert_eq!(
        keys.iter().rev().take(2).rev().cloned().collect::<Vec<_>>(),
        vec![after.clone(), key.clone()],
        "after a reload {label} must end {after}, {key}; it reads {keys:?}"
    );
}

#[then(
    regex = r"^the move request is exactly the one the board has always sent, naming (?:(\w+-\d+) as the card above|no card above)$"
)]
async fn then_request_exact(world: &mut FoundryWorld, above: String) {
    let client = browser(world);
    settle_requests(&client).await;
    let key = held_key(world);
    let all = move_requests(&client).await;
    let sent: Vec<_> = all.iter().skip(world.cpd.moves_before).collect();
    assert_eq!(
        sent.len(),
        1,
        "exactly one move request must have been sent for {key}: {sent:?}"
    );
    let request = sent[0];
    let lane = card_place(&client, &key).await.expect("the dropped card").0;
    let mut body = format!("state={lane}");
    if !above.is_empty() {
        body.push_str(&format!("&after={above}"));
    }
    assert_eq!(
        request.url,
        world.cpd.state_url.clone().unwrap_or_default(),
        "the move goes to the card's own state URL, as shipped (D1, AC-1.3)"
    );
    assert_eq!(
        request.body.as_deref(),
        Some(body.as_str()),
        "the move body, byte for byte (D1)"
    );
    let cookie = csrf_cookie(&client).await;
    assert!(
        !cookie.is_empty() && request.csrf == cookie,
        "the move carries the x-csrf-token header equal to the foundry_csrf cookie (D1); header \
         {:?}, cookie {cookie:?}",
        request.csrf
    );
    assert_eq!(request.status, Some(200), "the move must be accepted");
}

#[then(regex = r"^no edit dialog opens$")]
async fn then_no_dialog(world: &mut FoundryWorld) {
    let client = browser(world);
    tokio::time::sleep(NO_DIALOG_WAIT).await;
    let title = edit_dialog_title(&client).await;
    assert_eq!(
        title,
        None,
        "MISSING_FUNCTIONALITY: a drag must never open the card's edit dialog; the click after \
         the release is suppressed (D6, DDD-6, AC-1.5/2.3). {}",
        browser_harness::pointer_record(&client).await.describe()
    );
}

#[then(regex = r"^(\w+-\d+)'s edit dialog opens$")]
async fn then_dialog_opens(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if edit_dialog_title(&client).await.as_deref() == Some(format!("Edit {key}").as_str()) {
            return;
        }
        if Instant::now() > deadline {
            panic!(
                "{key}'s edit dialog did not open (a click or tap must still open a card, D5/D6, \
                 DDD-6: the click guard resets on the next press). {}",
                browser_harness::pointer_record(&client).await.describe()
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[then(regex = r"^no further move request is sent$")]
async fn then_no_further_request(world: &mut FoundryWorld) {
    let client = browser(world);
    settle_requests(&client).await;
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "a click or tap must send no move request"
    );
}

#[then(regex = r"^a carried (\w+-\d+) has followed the pointer$")]
async fn then_ghost_followed(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    let state = lift_state(&client, &key).await;
    let ghost =
        state.ghosts.first().copied().unwrap_or_else(|| {
            panic!("MISSING_FUNCTIONALITY: no carried {key} is visible: {state:?}")
        });
    let before = world
        .cpd
        .ghost_at_lift
        .expect("the carried copy read at the lift");
    let (p0, p1) = (world.cpd.pointer_at_lift, world.cpd.at);
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    assert!(
        dx.abs() + dy.abs() > 30.0,
        "the carry must have moved the pointer to prove anything"
    );
    assert!(
        ((ghost.0 - before.0) - dx).abs() <= 3.0 && ((ghost.1 - before.1) - dy).abs() <= 3.0,
        "the carried {key} must move with the pointer (D15, DDD-17): the pointer moved by \
         ({dx:.0}, {dy:.0}) but the carried copy by ({:.0}, {:.0})",
        ghost.0 - before.0,
        ghost.1 - before.1
    );
    let (x, y) = p1;
    let gap_x = (ghost.0 - x).max(x - (ghost.0 + ghost.2)).max(0.0);
    let gap_y = (ghost.1 - y).max(y - (ghost.1 + ghost.3)).max(0.0);
    assert!(
        gap_x <= 120.0 && gap_y <= 120.0,
        "the carried {key} must stay by the pointer; it is ({gap_x:.0}, {gap_y:.0}) px away"
    );
}

#[then(regex = r"^([\w-]+) is the one lane lit up$")]
async fn then_one_lane_lit(world: &mut FoundryWorld, label: String) {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, &label).await;
    assert_eq!(
        activated_lanes(&client).await,
        vec![slug],
        "MISSING_FUNCTIONALITY: {label} must be the only lane lit under the carried card (D10), \
         resolved from the point under the pointer (DDD-3)"
    );
    world.cpd.proven_lit = true;
}

#[then(regex = r"^exactly one marker shows, between (\w+-\d+) and (\w+-\d+)$")]
async fn then_one_marker(world: &mut FoundryWorld, above: String, below: String) {
    let client = browser(world);
    let lane = card_place(&client, &below).await.expect("the card below").0;
    let shown = markers(&client).await;
    assert_eq!(
        shown,
        vec![(lane, below.clone())],
        "MISSING_FUNCTIONALITY: exactly one marker must show between {above} and {below} (D10)"
    );
    world.cpd.proven_marker = true;
}

#[then(regex = r"^(\w+-\d+) still shows in its own slot in ([\w-]+), marked as lifted$")]
async fn then_origin_lifted(world: &mut FoundryWorld, key: String, _label: String) {
    let client = browser(world);
    assert_eq!(
        card_place(&client, &key).await,
        world.cpd.origin,
        "the lifted card stays in its origin slot until the drop (DDD-17)"
    );
    let state = lift_state(&client, &key).await;
    assert!(
        state.origin_lifted,
        "MISSING_FUNCTIONALITY: {key}'s origin is not marked lifted"
    );
}

#[then(regex = r"^once she releases it no carried card remains$")]
async fn then_release_clears(world: &mut FoundryWorld) {
    release(world).await;
    let client = browser(world);
    let key = held_key(world);
    assert!(
        world.cpd.proven_carried,
        "the carried card was proven first"
    );
    let state = lift_state(&client, &key).await;
    assert!(
        state.nothing_carried(),
        "MISSING_FUNCTIONALITY: the carried card outlives the release: {state:?}"
    );
}

#[then(regex = r"^no carried card remains$")]
async fn then_no_carried(world: &mut FoundryWorld) {
    let client = browser(world);
    let key = held_key(world);
    assert!(
        world.cpd.proven_carried,
        "the carried card was proven first"
    );
    let state = lift_state(&client, &key).await;
    assert!(
        state.nothing_carried(),
        "MISSING_FUNCTIONALITY: a carried card remains: {state:?}"
    );
}

#[then(regex = r"^no carried card remains and no move request is sent$")]
async fn then_no_carried_no_request(world: &mut FoundryWorld) {
    then_no_carried(world).await;
    let client = browser(world);
    settle_requests(&client).await;
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "a lifted card released where it lifted sends no move"
    );
}

#[then(regex = r"^(\w+-\d+) is back in its exact slot in ([\w-]+)$")]
async fn then_back_at_origin(world: &mut FoundryWorld, key: String, _label: String) {
    assert_back_at_origin(world, &key).await;
}

#[then(
    regex = r"^no lane is lit, no marker shows, no carried card remains and no move request is sent$"
)]
async fn then_nothing_left(world: &mut FoundryWorld) {
    assert_nothing_left(world).await;
}

#[then(regex = r"^the move was sent and refused$")]
async fn then_sent_and_refused(world: &mut FoundryWorld) {
    let client = browser(world);
    settle_requests(&client).await;
    let all = move_requests(&client).await;
    let sent: Vec<_> = all.iter().skip(world.cpd.moves_before).collect();
    assert!(
        sent.len() == 1 && sent[0].status == Some(404),
        "MISSING_FUNCTIONALITY: the touch drop must send exactly one move, which the server \
         refuses with its uniform 404 (the card is gone); sent {sent:?}"
    );
}

#[then(regex = r"^no lane is lit, no marker shows and no carried card remains$")]
async fn then_nothing_left_after_refusal(world: &mut FoundryWorld) {
    let client = browser(world);
    assert!(
        world.cpd.proven_carried && world.cpd.proven_lit && world.cpd.proven_marker,
        "MISSING_FUNCTIONALITY: the carried card, the lit lane and the marker must all have shown \
         before the release for their absence to mean anything (carried {}, lit {}, marker {})",
        world.cpd.proven_carried,
        world.cpd.proven_lit,
        world.cpd.proven_marker
    );
    let key = held_key(world);
    let state = lift_state(&client, &key).await;
    assert!(
        state.nothing_carried(),
        "MISSING_FUNCTIONALITY: a carried card remains: {state:?}"
    );
    assert_eq!(
        activated_lanes(&client).await,
        Vec::<String>::new(),
        "no lane is lit"
    );
    assert_eq!(
        markers(&client).await,
        Vec::<(String, String)>::new(),
        "no marker shows"
    );
}

#[then(regex = r"^releasing the mouse over ([\w-]+) afterwards moves nothing$")]
async fn then_release_after_cancel(world: &mut FoundryWorld, _label: String) {
    let client = browser(world);
    let before = board_snapshot(&client).await;
    release(world).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        board_snapshot(&client).await,
        before,
        "a release after Escape must land nothing (D11)"
    );
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "a release after Escape sends no move"
    );
    assert_eq!(
        edit_dialog_title(&client).await,
        None,
        "and opens no dialog"
    );
}

#[then(regex = r"^pressing Escape again changes nothing on the board$")]
async fn then_second_escape(world: &mut FoundryWorld) {
    let client = browser(world);
    let before = board_snapshot(&client).await;
    let url = client.current_url().await.expect("url").to_string();
    browser_harness::press_key(&client, "Escape").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        board_snapshot(&client).await,
        before,
        "a second Escape changes no card"
    );
    assert_eq!(
        client.current_url().await.expect("url").to_string(),
        url,
        "nor navigates"
    );
    assert_eq!(
        activated_lanes(&client).await,
        Vec::<String>::new(),
        "nor lights a lane"
    );
    assert_eq!(edit_dialog_title(&client).await, None, "nor opens a dialog");
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "nor sends a move"
    );
}

#[then(regex = r"^(\w+-\d+) has not lifted and ([\w-]+) now reads ((?:\w+-\d+)(?:, \w+-\d+)*)$")]
async fn then_not_lifted_reads(world: &mut FoundryWorld, key: String, label: String, list: String) {
    let client = browser(world);
    let record = browser_harness::pointer_record(&client).await;
    assert!(
        record.trusted("pointerdown") > 0,
        "BROKEN(driver): the right-button press never reached the page. {}",
        record.describe()
    );
    assert_eq!(
        world.cpd.lifted_during,
        Some(false),
        "a non-primary button must never start a drag of {key} (D6, AC-1.4)"
    );
    assert_eq!(
        lane_keys(&client, &label).await,
        keys_of(&list),
        "{label} on screen"
    );
}

#[then(regex = r"^no move request is sent for it$")]
async fn then_no_request_for_it(world: &mut FoundryWorld) {
    let client = browser(world);
    settle_requests(&client).await;
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "no move is sent"
    );
}

#[then(regex = r"^the same move with the primary button does lift (\w+-\d+)$")]
async fn then_primary_lifts(world: &mut FoundryWorld, key: String) {
    lift(world, PointerKind::Mouse, &key).await;
}

#[then(regex = r"^the lanes (?:still|now) read ([\w-]+(?:, [\w-]+)*)$")]
async fn then_lanes_read(world: &mut FoundryWorld, list: String) {
    let want: Vec<String> = list.split(',').map(|s| s.trim().to_string()).collect();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let got = lane_labels(&browser(world)).await;
        if got == want {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the board's lanes must read {want:?}; they read {got:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[then(
    regex = r"^no card was lifted, every card is still where it was and no move request is sent$"
)]
async fn then_header_moved_no_card(world: &mut FoundryWorld) {
    let client = browser(world);
    let record = browser_harness::pointer_record(&client).await;
    assert!(
        record.trusted("pointermove") > 0,
        "BROKEN(driver): the header drag delivered no trusted movement. {}",
        record.describe()
    );
    assert_eq!(
        world.cpd.lifted_during,
        Some(false),
        "a gesture begun on a lane header must never lift a card (D13)"
    );
    let mut before = world
        .cpd
        .board_before
        .clone()
        .expect("board before the header drag");
    let mut now = board_snapshot(&client).await;
    before.sort();
    now.sort();
    assert_eq!(now, before, "every card stays in its lane, in order");
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "no card move is sent"
    );
}

#[then(regex = r"^a card dragged straight afterwards still lifts$")]
async fn then_card_still_lifts(world: &mut FoundryWorld) {
    lift(world, PointerKind::Mouse, "AUTH-41").await;
}

#[then(
    regex = r"^the browser never gets to start its own drag, so the pointer stays with the board$"
)]
async fn then_native_declined(world: &mut FoundryWorld) {
    let evidence = world.cpd.native_evidence.clone().unwrap_or_default();
    assert!(
        evidence.contains("dragstart="),
        "BROKEN(driver): the press-and-move produced no native drag attempt at all, so this \
         scenario cannot observe the board declining one. {evidence}"
    );
    assert_eq!(
        world.cpd.native_drag_declined,
        Some(true),
        "MISSING_FUNCTIONALITY: the board let the browser start its own drag of a card (dragstart \
         not cancelled), which takes the pointer away mid-drag with pointercancel (DDD-2/DDD-19, \
         spike Q1). {evidence}"
    );
}

#[then(regex = r"^(\w+-\d+) is still marked draggable$")]
async fn then_still_draggable(world: &mut FoundryWorld, key: String) {
    let value = js(
        &browser(world),
        "var c = document.querySelector('#board-columns [data-issue-key=\"' + arguments[0] + '\"]');
         return c ? c.getAttribute('draggable') : null;",
        vec![serde_json::json!(key)],
    )
    .await;
    assert_eq!(
        value.as_str(),
        Some("true"),
        "{key} keeps draggable=\"true\" (DDD-19; issue-status-move.feature:49)"
    );
}

#[then(regex = r"^nothing else moves and no further move request is sent$")]
async fn then_foreign_nothing(world: &mut FoundryWorld) {
    let client = browser(world);
    assert_eq!(
        board_snapshot(&client).await,
        world
            .cpd
            .board_before
            .clone()
            .expect("board before the file"),
        "MISSING_FUNCTIONALITY: a card moved on a file drop, after a pointer drag (D3)"
    );
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "no move is sent"
    );
}

#[then(regex = r"^the board still shows Identity Platform in the same tab$")]
async fn then_same_tab(world: &mut FoundryWorld) {
    let client = browser(world);
    assert_eq!(
        Some(client.current_url().await.expect("url").to_string()),
        world.cpd.url_before,
        "the tab must not have navigated"
    );
    browser_harness::wait_for_board_ready(&client).await;
    assert_eq!(
        world.cpd.foreign_claimed,
        Some((true, true)),
        "the board must claim the file's dragover and cancel its drop (swallowed, D3)"
    );
}

#[then(regex = r"^neither the board nor the page has scrolled while she carried it$")]
async fn then_no_scroll_while_carrying(world: &mut FoundryWorld) {
    let now = scroll_state(&browser(world)).await;
    let before = world.cpd.scroll_at_lift.expect("scroll read at the lift");
    assert!(
        (now.0 - before.0).abs() < 1.0
            && (now.1 - before.1).abs() < 1.0
            && (now.2 - before.2).abs() < 1.0,
        "MISSING_FUNCTIONALITY: a lifted touch carry must not scroll the board or the page \
         (AC-2.4, DDD-4): (board, page x, page y) went {before:?} -> {now:?}"
    );
}

#[then(regex = r"^the board has scrolled towards ([\w-]+)$")]
async fn then_board_scrolled(world: &mut FoundryWorld, _label: String) {
    let client = browser(world);
    let record = browser_harness::pointer_record(&client).await;
    assert!(
        record.trusted("pointerdown") > 0 && record.trusted("pointermove") > 0,
        "BROKEN(driver): the swipe delivered no trusted touch input. {}",
        record.describe()
    );
    let before = world
        .cpd
        .board_scroll_before
        .expect("scroll before the swipe");
    let now = scroll_state(&client).await.0;
    assert!(
        now > before + 20.0,
        "a swipe that starts on a card must scroll the board (D5, AC-2.2): scrollLeft {before} -> \
         {now}. {}",
        record.describe()
    );
}

#[then(
    regex = r"^(\w+-\d+) has not lifted, no lane is lit, no marker shows and no move request is sent$"
)]
async fn then_swipe_nothing(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    assert_eq!(
        world.cpd.lifted_during,
        Some(false),
        "a touch that moves past the hold tolerance before the hold completes must lift nothing \
         (D5, AC-2.2)"
    );
    assert_eq!(
        activated_lanes(&client).await,
        Vec::<String>::new(),
        "no lane is lit"
    );
    assert_eq!(
        markers(&client).await,
        Vec::<(String, String)>::new(),
        "no marker shows"
    );
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "no move is sent"
    );
    assert_eq!(
        card_place(&client, &key).await,
        world.cpd.origin,
        "{key} has not moved"
    );
}

#[then(regex = r"^holding (\w+-\d+) (?:still|again) straight afterwards does lift it$")]
async fn then_hold_lifts(world: &mut FoundryWorld, key: String) {
    lift(world, PointerKind::RetryTouch, &key).await;
}

#[then(regex = r"^(\w+-\d+) is still carried and ([\w-]+) is still the one lane lit up$")]
async fn then_still_carried(world: &mut FoundryWorld, key: String, label: String) {
    let client = browser(world);
    let state = lift_state(&client, &key).await;
    assert!(
        state.lifted(),
        "a second finger must not take or drop the carried {key} (D16, AC-2.7): {state:?}"
    );
    then_one_lane_lit(world, label).await;
}

#[then(regex = r"^lifting the first finger lands (\w+-\d+) between (\w+-\d+) and (\w+-\d+)$")]
async fn then_first_finger_lands(
    world: &mut FoundryWorld,
    key: String,
    above: String,
    below: String,
) {
    release(world).await;
    let client = browser(world);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let place = card_place(&client, &key).await;
        if place
            .as_ref()
            .is_some_and(|p| p.1.as_deref() == Some(&above) && p.2.as_deref() == Some(&below))
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the first finger's release must land {key} between {above} and {below}; it is at {place:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[then(regex = r"^(\w+-\d+) landed exactly where the marker showed just before she let go$")]
async fn then_landed_at_marker(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    let (lane, before) = world
        .cpd
        .marker_at_release
        .clone()
        .expect("the marker read at the release");
    let place = card_place(&client, &key).await.expect("the dropped card");
    let below = place.2.clone().unwrap_or_default();
    assert_eq!(
        (place.0.clone(), below),
        (lane, before),
        "marker = landing (D10, AC-3.4): {key} landed at {place:?}"
    );
    let all = move_requests(&client).await;
    let sent: Vec<_> = all.iter().skip(world.cpd.moves_before).collect();
    assert_eq!(
        sent.len(),
        1,
        "exactly one move is sent across the auto-scroll: {sent:?}"
    );
    let body = sent[0].body.clone().unwrap_or_default();
    let want_after = place.1.map(|a| format!("&after={a}")).unwrap_or_default();
    assert_eq!(
        body,
        format!("state={}{want_after}", place.0),
        "the move's after is the marker's slot (AC-3.4)"
    );
}

#[then(regex = r"^the board scrolls no further and the page has not scrolled sideways$")]
async fn then_scroll_stopped(world: &mut FoundryWorld) {
    let client = browser(world);
    let before = world
        .cpd
        .scroll_at_release
        .expect("scroll read before holding on");
    let now = scroll_state(&client).await;
    let max = board_scroll_max(&client).await;
    assert!(
        (now.0 - before.0).abs() < 1.0 && now.0 >= max - 1.0,
        "the board must stop at its end (AC-3.2): scrollLeft {} -> {} of {max}",
        before.0,
        now.0
    );
    assert!(
        now.1.abs() < 1.0,
        "the page must not scroll sideways: scrollX = {}",
        now.1
    );
}

#[then(regex = r"^a marker still shows in the lane under her finger$")]
async fn then_marker_under_finger(world: &mut FoundryWorld) {
    let client = browser(world);
    let (x, y) = world.cpd.at;
    let under = js(
        &client,
        "var el = document.elementFromPoint(arguments[0], arguments[1]);
         var l = el ? el.closest('#board-columns [data-column]') : null;
         return l ? l.getAttribute('data-column') : null;",
        vec![serde_json::json!(x), serde_json::json!(y)],
    )
    .await;
    let shown = markers(&client).await;
    assert!(
        shown.len() == 1 && Some(shown[0].0.as_str()) == under.as_str(),
        "MISSING_FUNCTIONALITY: the marker must follow the finger at the board's end: lane under \
         the finger {under:?}, markers {shown:?}"
    );
}

#[then(regex = r"^the page had scrolled down to reach (\w+-\d+)$")]
async fn then_page_scrolled(world: &mut FoundryWorld, _last: String) {
    let before = world.cpd.scroll_at_lift.expect("scroll at the lift");
    let at_release = world.cpd.scroll_at_release.expect("scroll at the release");
    assert!(
        world.cpd.target_visible_at_release && at_release.2 > before.2 + 40.0,
        "the page must scroll down while the carried card is held near the bottom (AC-3.3): \
         scrollY {} -> {}",
        before.2,
        at_release.2
    );
}

#[then(
    regex = r"^(\w+-\d+) has still not lifted once the hold time has passed, and nothing is lit or sent$"
)]
async fn then_hold_abandoned(world: &mut FoundryWorld, key: String) {
    let client = browser(world);
    tokio::time::sleep(Duration::from_millis(HOLD_MS + 200)).await;
    let record = browser_harness::pointer_record(&client).await;
    assert!(
        record.trusted("pointerdown") > 0 && record.trusted("pointercancel") > 0,
        "BROKEN(driver): the interrupted hold needs a trusted press and a trusted cancel. {}",
        record.describe()
    );
    let state = lift_state(&client, &key).await;
    assert!(
        state.nothing_carried(),
        "a pointercancel before the lift abandons the hold (D12, AC-3.5): {state:?}"
    );
    assert_eq!(
        activated_lanes(&client).await,
        Vec::<String>::new(),
        "no lane is lit"
    );
    assert_eq!(
        move_requests(&client).await.len(),
        world.cpd.moves_before,
        "no move is sent"
    );
    assert_eq!(
        card_place(&client, &key).await,
        world.cpd.origin,
        "{key} has not moved"
    );
}
