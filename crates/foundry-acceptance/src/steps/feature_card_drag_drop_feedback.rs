//! card-drag-drop-feedback step definitions
//! (`tests/features/card-drag-drop-feedback.feature`, ALL `@pending`: scaffolded
//! RED per ADR-025; DELIVER un-pends one slice at a time and never re-authors).
//!
//! No production seam is scaffolded (DESIGN DDD-10): the feature changes only
//! `board-dnd.js`, the stylesheet and `partials/board_columns.html`, which
//! DELIVER owns. Every seam these steps touch is SHIPPED: the board page, the
//! card popup delete, the ⋯ lane menu, the header drag, `POST …/state`, and
//! `board-live.js`. The RED therefore comes from one honest place: the board's
//! browser tier does not yet do what the scenario asks, observed through the
//! DESIGN-pinned DOM hooks (DDD-8f):
//!
//!   * "accepts the drag"        -> the synthetic `dragover` is `defaultPrevented`
//!   * "shown as activated"      -> `[data-card-drop-target]` on the lane
//!   * "a marker shows …"        -> one `[data-card-drop-marker]`; `data-before-key` is the slot
//!   * "shows the placeholder"   -> the lane's `.empty` is DISPLAYED, not merely present (DDD-5)
//!   * "no move request is sent" -> the page-side `fetch` spy (DDD-8d)
//!
//! THE NO-RELOAD RULE (D2, DDD-8e). Opening the board is always a Given, and no
//! drag When ever navigates. `browser_harness::install_drag_observers` stamps
//! `window.__cdfMark` on the board document; every drag step asserts it is still
//! there. `feature_board_lane_reorder::drag_a_card` is NOT reused, because it
//! reloads first and a reload re-binds every listener, which hides the bug.
//!
//! THE REPLACE RULE. A Given that "refreshes the board in place" stamps the live
//! `#board-columns` node and waits until a DIFFERENT node stands in its place.
//! Without that, a menu click that silently did nothing would leave the old,
//! still-wired lanes on screen and the regression would pass on HEAD. The one
//! exception is the header drag, which moves the existing lane node and replaces
//! nothing; see `refresh_in_place`.
//!
//! ANTI-VACUITY. A negative oracle ("no lane is lit", "no marker", "the
//! placeholder is unchanged") is true on HEAD simply because none of that exists
//! yet. Each such scenario therefore carries a positive control in the same
//! scenario ("a card dragged over Done straight afterwards does light it"), so
//! it goes RED for the right reason on HEAD and cannot pass vacuously later.
//! "Returns to its slot" asserts the move request was actually SENT and REFUSED
//! first, because a card that never left its slot "returns" trivially.
//!
//! LAYER: browser E2E against the real in-process app, real Postgres
//! (per-scenario schema) and a real containerised Chrome. Example-based only
//! (Mandates 9 + 11). Synthetic drag events prove wiring; each slice's manual
//! dogfood check proves feel (D12).

use crate::steps::feature_canzan_theme::{contrast_ratio, hex, parse_colour, Rgb};
use crate::support::browser_harness::{self, DragSpot, ForeignPayload, SpiedRequest};
use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use secrecy::SecretString;
use sqlx::PgPool;
use std::time::{Duration, Instant};

const TEST_NOW: &str = "2026-03-01T12:00:00Z";
const PRIYA_EMAIL: &str = "priya.cdf@canzan.test";
const PRIYA_PASSWORD: &str = "priya-correct-horse-battery-staple";
const IDENTITY: &str = "Identity Platform";
const HOMELAB: &str = "Homelab Ops";
const BROWSER_WAIT: Duration = Duration::from_secs(10);

// --- DESIGN-pinned hooks (DDD-8f, ADR-BOARD-CARD-002). If DELIVER renames one,
// --- the stylesheet, board-dnd.js and this module move in the SAME change.
const ACTIVATED: &str = "[data-card-drop-target]";
const MARKER: &str = "[data-card-drop-marker]";
/// The popup's Delete control and the confirm dialog (shipped, issue-card-delete).
const POPUP_DELETE: &str = "[data-action=\"delete-issue\"]";
const DELETE_CONFIRM: &str = "[data-modal=\"delete-issue\"] button[type='submit']";

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
        .expect("a Given must have opened the board in a browser")
        .clone()
}

fn second_tab(world: &FoundryWorld) -> fantoccini::Client {
    world
        .cdf_second_tab
        .as_ref()
        .expect("a Given must have opened the second tab")
        .clone()
}

fn project_of_key(key: &str) -> &'static str {
    if key.starts_with("AUTH-") {
        IDENTITY
    } else if key.starts_with("OPS-") {
        HOMELAB
    } else {
        panic!("{key:?} belongs to no board in this feature")
    }
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

async fn seed_user(world: &FoundryWorld) -> uuid::Uuid {
    let pool = pool(world);
    let hash = foundry_auth::hash_password(&SecretString::new(PRIYA_PASSWORD.to_string().into()))
        .await
        .expect("hash password");
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(PRIYA_EMAIL)
    .bind(PRIYA_EMAIL)
    .bind("Priya Raman")
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("insert Priya");
    id
}

async fn seed_project(
    world: &mut FoundryWorld,
    name: &str,
    slug: &str,
    prefix: &str,
) -> uuid::Uuid {
    let pool = pool(world);
    let ws = world.cdf_workspace_id.expect("workspace seeded first");
    let team = world.cdf_team_id.expect("team seeded first");
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
    world.cdf_project_ids.insert(name.to_string(), id);
    let stored: (String, String) = sqlx::query_as(
        "SELECT t.slug, p.slug FROM projects p JOIN teams t ON t.id = p.team_id WHERE p.id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .expect("read back stored slugs");
    world.cdf_project_slugs.insert(name.to_string(), stored);
    id
}

async fn seed_lanes(world: &FoundryWorld, project_id: uuid::Uuid, lanes: &[(&str, &str)]) {
    let ws = world.cdf_workspace_id.expect("workspace seeded first");
    for (position, (slug, label)) in lanes.iter().enumerate() {
        sqlx::query(
            "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(project_id)
        .bind(ws)
        .bind(slug)
        .bind(label)
        .bind(position as i32)
        .execute(&pool(world))
        .await
        .unwrap_or_else(|err| panic!("seed lane {slug:?}: {err}"));
    }
}

async fn seed_issues(world: &FoundryWorld, project: &str, lane: &str, keys: &[&str]) {
    let project_id = world.cdf_project_ids[project];
    let ws = world.cdf_workspace_id.expect("workspace seeded first");
    let author = world.cdf_priya_id.expect("Priya seeded first");
    for (position, key) in keys.iter().enumerate() {
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, state, position, author_id)
                  VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(project_id)
        .bind(ws)
        .bind(number_of(key))
        .bind(format!("Work item {key}"))
        .bind(lane)
        .bind(position as i32)
        .bind(author)
        .execute(&pool(world))
        .await
        .unwrap_or_else(|err| panic!("seed {key} into {lane:?}: {err}"));
    }
}

// --------------------------------------------------------------- board probes

async fn lane_slug_on_screen(client: &fantoccini::Client, label: &str) -> String {
    let found = js(
        client,
        "var lanes = document.querySelectorAll('#board-columns section.column');
         for (var i = 0; i < lanes.length; i++) {
           var h = lanes[i].querySelector('h3');
           if (h && h.textContent.trim() === arguments[0]) { return lanes[i].getAttribute('data-column'); }
         }
         return null;",
        vec![serde_json::json!(label)],
    )
    .await;
    found
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| panic!("no lane labelled {label:?} is on the board on screen"))
}

async fn board_snapshot(client: &fantoccini::Client) -> Vec<(String, Vec<String>)> {
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

/// `(lane slug, key above, key below)` of a card on the live board.
type Place = (String, Option<String>, Option<String>);

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

/// Every marker on the board: `(lane slug, before-key, vertical midline)`.
/// An absent `data-before-key` reads as `"?"` so it can never pass for the
/// empty string, which means "the end".
async fn markers(client: &fantoccini::Client) -> Vec<(String, String, f64)> {
    let raw = js(
        client,
        &format!(
            "return Array.prototype.map.call(document.querySelectorAll('{MARKER}'), function (m) {{
               var lane = m.closest('[data-column]'); var r = m.getBoundingClientRect();
               var before = m.getAttribute('data-before-key');
               return [lane ? lane.getAttribute('data-column') : '', before === null ? '?' : before, (r.top + r.bottom) / 2];
             }});"
        ),
        vec![],
    )
    .await;
    serde_json::from_value(raw).expect("markers shape")
}

async fn card_rect(client: &fantoccini::Client, key: &str) -> (f64, f64) {
    let raw = js(
        client,
        "var c = document.querySelector('#board-columns [data-issue-key=\"' + arguments[0] + '\"]');
         if (!c) { return null; } var r = c.getBoundingClientRect(); return [r.top, r.bottom];",
        vec![serde_json::json!(key)],
    )
    .await;
    serde_json::from_value::<Option<(f64, f64)>>(raw)
        .expect("card rect shape")
        .unwrap_or_else(|| panic!("{key} is not on the board"))
}

/// Every lane's and card's `[left, top, width, height]`, keyed `"lane <slug>"`
/// and `"card <key>"`: the layout a lane's activation must not disturb (AC-2.6).
async fn layout_rects(client: &fantoccini::Client) -> serde_json::Value {
    js(
        client,
        "var out = {};
         document.querySelectorAll('#board-columns section.column').forEach(function (l) {
           var r = l.getBoundingClientRect(); out['lane ' + l.getAttribute('data-column')] = [r.left, r.top, r.width, r.height]; });
         document.querySelectorAll('#board-columns .issue-card').forEach(function (c) {
           var r = c.getBoundingClientRect(); out['card ' + c.getAttribute('data-issue-key')] = [r.left, r.top, r.width, r.height]; });
         return out;",
        vec![],
    )
    .await
}

/// A probe-side JS function, `opaque(el)`: the first fully opaque background
/// colour at or above `el` (white when there is none), as `rgb(r, g, b)`. What a
/// boundary is actually seen against.
const OPAQUE_BACKGROUND_JS: &str = "function opaque(el) {
  while (el) {
    var bg = getComputedStyle(el).backgroundColor.match(/rgba?\\(([^)]+)\\)/);
    if (bg) { var p = bg[1].split(',').map(parseFloat); if (p.length < 4 || p[3] >= 1) { return 'rgb(' + p.slice(0, 3).join(', ') + ')'; } }
    el = el.parentElement;
  }
  return 'rgb(255, 255, 255)';
}";

/// The best contrast any fully opaque colour the probe read under `keys`
/// reaches against `against`; 0 when it read none.
fn best_boundary_contrast(probe: &serde_json::Value, keys: &[&str], against: Rgb) -> f64 {
    keys.iter()
        .filter_map(|k| probe[*k].as_str().and_then(parse_colour))
        .filter(|(_, alpha)| *alpha >= 1.0)
        .map(|(c, _)| contrast_ratio(c, against))
        .fold(0.0f64, f64::max)
}

/// Every lane's look: `(slug, card keys, placeholder displayed, placeholder markup)`.
type Look = (String, Vec<String>, bool, String);

async fn lane_looks(client: &fantoccini::Client) -> Vec<Look> {
    let raw = js(
        client,
        "return Array.prototype.map.call(document.querySelectorAll('#board-columns section.column'), function (l) {
           var keys = Array.prototype.map.call(l.querySelectorAll('.issue-card'), function (c) { return c.getAttribute('data-issue-key'); });
           var p = l.querySelector(':scope > .empty');
           var shown = !!p && getComputedStyle(p).display !== 'none' && p.getClientRects().length > 0;
           return [l.getAttribute('data-column'), keys, shown, p ? p.outerHTML : ''];
         });",
        vec![],
    )
    .await;
    serde_json::from_value(raw).expect("lane looks shape")
}

async fn look_of(client: &fantoccini::Client, slug: &str) -> Look {
    lane_looks(client)
        .await
        .into_iter()
        .find(|(s, ..)| s == slug)
        .unwrap_or_else(|| panic!("lane {slug:?} is not on the board"))
}

fn is_move_request(r: &SpiedRequest) -> bool {
    r.method.eq_ignore_ascii_case("POST") && r.url.contains("/issues/") && r.url.ends_with("/state")
}

async fn move_requests(client: &fantoccini::Client) -> Vec<SpiedRequest> {
    browser_harness::spied_requests(client)
        .await
        .into_iter()
        .filter(is_move_request)
        .collect()
}

/// Wait until every request the spy saw has been answered.
async fn settle_requests(client: &fantoccini::Client) {
    let deadline = Instant::now() + BROWSER_WAIT;
    loop {
        let pending: Vec<SpiedRequest> = browser_harness::spied_requests(client)
            .await
            .into_iter()
            .filter(|r| r.status.is_none())
            .collect();
        if pending.is_empty() {
            return;
        }
        if Instant::now() > deadline {
            panic!("requests still unanswered after {BROWSER_WAIT:?}: {pending:?}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn stamp_board(client: &fantoccini::Client) {
    js(
        client,
        "document.getElementById('board-columns').__cdfStamp = true; return true;",
        vec![],
    )
    .await;
}

/// Wait until `#board-columns` is a DIFFERENT node from the stamped one: the
/// board was replaced in place, not merely edited.
async fn await_board_replaced(client: &fantoccini::Client, what: &str) {
    let deadline = Instant::now() + BROWSER_WAIT;
    loop {
        let replaced = js(
            client,
            "var b = document.getElementById('board-columns'); return !!b && b.__cdfStamp !== true;",
            vec![],
        )
        .await
        .as_bool()
        .unwrap_or(false);
        if replaced {
            return;
        }
        if Instant::now() > deadline {
            panic!(
                "{what} did not replace #board-columns in place within {BROWSER_WAIT:?}. The \
                 scenario needs a real in-place refresh; without one it would pass over lanes \
                 that were never replaced"
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_card_gone(client: &fantoccini::Client, key: &str, what: &str) {
    let deadline = Instant::now() + BROWSER_WAIT;
    while card_place(client, key).await.is_some() {
        if Instant::now() > deadline {
            panic!("{key} is still on the board {BROWSER_WAIT:?} after {what}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// -------------------------------------------------------------- the browser

async fn ensure_browser(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    if world.browser.is_none() {
        let client = if world.cdf_dark {
            browser_harness::new_dark_session().await
        } else {
            browser_harness::new_session().await
        };
        browser_harness::sign_in_through_browser(
            &client,
            harness(world),
            PRIYA_EMAIL,
            PRIYA_PASSWORD,
        )
        .await;
        world.browser = Some(client);
    }
}

fn board_url(world: &FoundryWorld, project: &str) -> String {
    let (team, slug) = world
        .cdf_project_slugs
        .get(project)
        .unwrap_or_else(|| panic!("{project:?} must be seeded by the Background"));
    format!("{}/team/{team}/project/{slug}", harness(world).base_url())
}

/// Navigate to the board (the ONLY navigation a drag scenario makes before its
/// drag), then install the fetch spy and the no-reload mark.
async fn open_board(world: &mut FoundryWorld, project: &str) {
    ensure_browser(world).await;
    world.cdf_current_project = Some(project.to_string());
    let client = browser(world);
    client
        .goto(&board_url(world, project))
        .await
        .expect("navigate to the board");
    browser_harness::wait_for_board_ready(&client).await;
    world.cdf_mark = Some(browser_harness::install_drag_observers(&client).await);
    if world.cdf_server_placeholder.is_none() {
        // The server's own placeholder, read off a lane that is empty at load.
        if let Some((_, _, true, markup)) = lane_looks(&client)
            .await
            .into_iter()
            .find(|(_, keys, shown, _)| keys.is_empty() && *shown)
        {
            world.cdf_server_placeholder = Some(markup);
        }
    }
}

async fn assert_not_reloaded(world: &FoundryWorld) {
    let mark = world
        .cdf_mark
        .clone()
        .expect("the board must have been opened by a Given");
    browser_harness::assert_not_reloaded(&browser(world), &mark).await;
}

/// Reload the board (only ever as an ORACLE, after the drag), once every
/// request has been answered, and re-arm the observers.
async fn reload_board(world: &mut FoundryWorld) {
    let client = browser(world);
    settle_requests(&client).await;
    client.refresh().await.expect("reload the board");
    browser_harness::wait_for_board_ready(&client).await;
    world.cdf_mark = Some(browser_harness::install_drag_observers(&client).await);
}

// ------------------------------------------------------------ drag gestures

async fn start_drag(world: &mut FoundryWorld, key: &str) {
    let client = browser(world);
    world.cdf_origin = card_place(&client, key).await;
    assert!(
        world.cdf_origin.is_some(),
        "{key} is not on the board to be dragged"
    );
    world.cdf_dragging = Some(key.to_string());
    world.cdf_moves_before = Some(move_requests(&client).await.len());
    browser_harness::drag_start(&client, key).await;
}

async fn drag_over(world: &mut FoundryWorld, spot: DragSpot<'_>) -> bool {
    let claimed = browser_harness::drag_over(&browser(world), spot).await;
    world.cdf_over_claimed = Some(claimed);
    claimed
}

async fn drop_here(world: &mut FoundryWorld) -> bool {
    let dropped = browser_harness::drag_drop(&browser(world), DragSpot::SamePoint).await;
    world.cdf_drop_claimed = Some(dropped);
    dropped
}

async fn end_drag(world: &mut FoundryWorld) {
    let key = world
        .cdf_dragging
        .clone()
        .expect("a card drag must be in progress");
    browser_harness::drag_end(&browser(world), &key).await;
}

/// Release the dragged `key` where the pointer is and end the drag. `label`
/// must have accepted the drop: a real browser fires `drop` only on a lane that
/// claimed the dragover.
async fn release_accepted(world: &mut FoundryWorld, key: &str, label: &str) {
    let dropped = drop_here(world).await;
    end_drag(world).await;
    assert!(
        dropped,
        "MISSING_FUNCTIONALITY: {label} did not accept the drop of {key}"
    );
}

/// Pick up `key`, carry it to `spot`, and release it there: the whole gesture.
/// A lane that does not claim the drag is this suite's honest RED.
async fn drag_and_drop(world: &mut FoundryWorld, key: &str, spot: DragSpot<'_>) {
    assert_not_reloaded(world).await;
    start_drag(world, key).await;
    let claimed = drag_over(world, spot).await;
    let dropped = drop_here(world).await;
    end_drag(world).await;
    settle_requests(&browser(world)).await;
    assert!(
        claimed && dropped,
        "MISSING_FUNCTIONALITY: the lane under {key} did not accept the drop (dragover \
         defaultPrevented = {claimed}; a real browser only fires drop on a claimed target, so \
         drop dispatched = {dropped}). On HEAD the lanes a board refresh put on screen carry no \
         listener (rca-drag-after-board-replace.md)"
    );
}

async fn lane_spot_slug(world: &FoundryWorld, label: &str) -> String {
    lane_slug_on_screen(&browser(world), label).await
}

/// Where a foreign drag is dropped, resolved against the board on screen.
enum DropPlace {
    /// "the gap between Backlog and In-Progress": the board seam between two
    /// lanes (their slugs), inside `#board-columns` but in no lane.
    Gap(String, String),
    /// "the empty space below AUTH-7 in Done": just below that card.
    Below(String),
    /// "Done", "In-Progress": the lane (its slug), at its end slot.
    Lane(String),
}

impl DropPlace {
    fn spot(&self) -> DragSpot<'_> {
        match self {
            DropPlace::Gap(left, right) => DragSpot::BoardGap(left, right),
            DropPlace::Below(key) => DragSpot::Below(key),
            DropPlace::Lane(slug) => DragSpot::LaneEnd(slug),
        }
    }
}

async fn resolve_where(world: &FoundryWorld, place: &str) -> DropPlace {
    if let Some(rest) = place.strip_prefix("the gap between ") {
        let (left, right) = rest
            .split_once(" and ")
            .unwrap_or_else(|| panic!("unreadable gap {place:?}"));
        let left = lane_spot_slug(world, left).await;
        let right = lane_spot_slug(world, right).await;
        return DropPlace::Gap(left, right);
    }
    if let Some(rest) = place.strip_prefix("the empty space below ") {
        let (key, _) = rest
            .split_once(" in ")
            .unwrap_or_else(|| panic!("unreadable place {place:?}"));
        return DropPlace::Below(key.to_string());
    }
    DropPlace::Lane(lane_spot_slug(world, place).await)
}

// ---------------------------------------------------- in-place board refresh

async fn popup_delete(world: &mut FoundryWorld, key: &str) {
    let client = browser(world);
    stamp_board(&client).await;
    let card = format!("#board-columns [data-issue-key='{key}']");
    client
        .wait()
        .at_most(BROWSER_WAIT)
        .for_element(Locator::Css(&card))
        .await
        .unwrap_or_else(|_| panic!("{key} must be on the board to open its popup"))
        .click()
        .await
        .expect("open the card's popup");
    client
        .wait()
        .at_most(BROWSER_WAIT)
        .for_element(Locator::Css(POPUP_DELETE))
        .await
        .expect("the popup offers Delete (shipped, issue-card-delete)")
        .click()
        .await
        .expect("choose Delete");
    client
        .wait()
        .at_most(BROWSER_WAIT)
        .for_element(Locator::Css(DELETE_CONFIRM))
        .await
        .expect("the confirm dialog offers its submit")
        .click()
        .await
        .expect("confirm the delete");
    await_board_replaced(&client, &format!("the popup delete of {key}")).await;
    wait_for_card_gone(&client, key, "its popup delete").await;
    assert_not_reloaded(world).await;
}

async fn open_lane_menu_item(world: &FoundryWorld, label: &str, item: &str) {
    let client = browser(world);
    browser_harness::wait_for_kb_ready(&client).await;
    let slug = lane_slug_on_screen(&client, label).await;
    client
        .find(Locator::Css(&format!(
            "button[data-action=\"toggle-lane-menu\"][data-lane=\"{slug}\"]"
        )))
        .await
        .unwrap_or_else(|err| panic!("the {label:?} lane has no ⋯ menu trigger: {err}"))
        .click()
        .await
        .expect("open the lane menu");
    let menu = format!("[data-lane-menu=\"{slug}\"]");
    let deadline = Instant::now() + BROWSER_WAIT;
    loop {
        let open = js(
            &client,
            "var m = document.querySelector(arguments[0]); return !!m && !m.hidden;",
            vec![serde_json::json!(menu)],
        )
        .await
        .as_bool()
        .unwrap_or(false);
        if open {
            break;
        }
        if Instant::now() > deadline {
            panic!("the {label:?} lane menu did not open");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    client
        .find(Locator::Css(&format!("{menu} {item}")))
        .await
        .unwrap_or_else(|err| panic!("the {label:?} menu has no {item}: {err}"))
        .click()
        .await
        .expect("choose the lane menu item");
}

async fn submit_lane_dialog(world: &FoundryWorld, modal: &str, label: &str) {
    let client = browser(world);
    let field = client
        .wait()
        .at_most(BROWSER_WAIT)
        .for_element(Locator::Css(&format!(
            "[data-modal=\"{modal}\"] input[name=\"label\"]"
        )))
        .await
        .unwrap_or_else(|_| panic!("the {modal} dialog must open with a name field"));
    field.clear().await.expect("clear the name field");
    field.send_keys(label).await.expect("type the lane name");
    client
        .find(Locator::Css(&format!(
            "[data-modal=\"{modal}\"] button[type=\"submit\"]"
        )))
        .await
        .expect("the dialog's submit")
        .click()
        .await
        .expect("submit the lane dialog");
}

async fn wait_for_lane_label(client: &fantoccini::Client, label: &str, present: bool) {
    let deadline = Instant::now() + BROWSER_WAIT;
    loop {
        let found = js(
            client,
            "var hs = document.querySelectorAll('#board-columns section.column > h3');
             for (var i = 0; i < hs.length; i++) { if (hs[i].textContent.trim() === arguments[0]) { return true; } }
             return false;",
            vec![serde_json::json!(label)],
        )
        .await
        .as_bool()
        .unwrap_or(false);
        if found == present {
            return;
        }
        if Instant::now() > deadline {
            panic!(
                "the lane {label:?} is {} the board",
                if present { "not on" } else { "still on" }
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn move_lane_right_from_menu(world: &mut FoundryWorld, label: &str) {
    let client = browser(world);
    stamp_board(&client).await;
    open_lane_menu_item(world, label, "[data-action=\"move-lane-right\"]").await;
    await_board_replaced(&client, &format!("Move list right on {label:?}")).await;
    settle_requests(&client).await;
    assert_not_reloaded(world).await;
}

async fn insert_lane_after(world: &mut FoundryWorld, new_label: &str, anchor: &str) {
    let client = browser(world);
    stamp_board(&client).await;
    open_lane_menu_item(world, anchor, "button[hx-get$=\"/insert/after\"]").await;
    submit_lane_dialog(world, "insert-lane", new_label).await;
    await_board_replaced(&client, &format!("inserting {new_label:?}")).await;
    wait_for_lane_label(&client, new_label, true).await;
    assert_not_reloaded(world).await;
}

async fn rename_lane_from_menu(world: &FoundryWorld, label: &str, new_label: &str) {
    let client = browser(world);
    stamp_board(&client).await;
    open_lane_menu_item(world, label, "button[hx-get$=\"/edit\"]").await;
    submit_lane_dialog(world, "edit-lane", new_label).await;
    await_board_replaced(&client, &format!("renaming {label}")).await;
    wait_for_lane_label(&client, new_label, true).await;
}

async fn delete_lane_moving_cards(world: &FoundryWorld, label: &str, destination: &str) {
    let client = browser(world);
    stamp_board(&client).await;
    let destination = lane_slug_on_screen(&client, destination).await;
    open_lane_menu_item(world, label, "button[hx-get$=\"/delete\"]").await;
    client
        .wait()
        .at_most(BROWSER_WAIT)
        .for_element(Locator::Css(
            "[data-modal=\"delete-lane\"] select[name=\"destination\"]",
        ))
        .await
        .expect("the delete-lane dialog offers a destination");
    js(
        &client,
        "var s = document.querySelector('[data-modal=\"delete-lane\"] select[name=\"destination\"]');
         s.value = arguments[0]; s.dispatchEvent(new Event('change', { bubbles: true })); return s.value;",
        vec![serde_json::json!(destination)],
    )
    .await;
    client
        .find(Locator::Css(
            "[data-modal=\"delete-lane\"] button[name=\"fate\"][value=\"move\"]",
        ))
        .await
        .expect("the move fate")
        .click()
        .await
        .expect("delete the lane, moving its cards");
    await_board_replaced(&client, &format!("deleting the {label} list")).await;
    wait_for_lane_label(&client, label, false).await;
}

/// Drag `label`'s header to the left of `target` with pointer events. This
/// moves the existing lane node and replaces nothing (`board-lane-dnd.js`
/// pointerup never calls `applyBoard`), so it asserts the move was accepted
/// and shows on screen instead of awaiting a replace.
async fn drag_header_left_of(world: &FoundryWorld, label: &str, target: &str) {
    let client = browser(world);
    let moving = lane_slug_on_screen(&client, label).await;
    let anchor = lane_slug_on_screen(&client, target).await;
    let ok = js(
        &client,
        "var src = document.querySelector('[data-lane-drag=\"' + arguments[0] + '\"]');
         var dst = document.querySelector('[data-column=\"' + arguments[1] + '\"]');
         if (!src || !dst) { return false; }
         var a = src.getBoundingClientRect(), b = dst.getBoundingClientRect();
         var y = a.top + a.height / 2, dropX = b.left + b.width * 0.1;
         function at(x, type) {
           src.dispatchEvent(new PointerEvent(type, { pointerType: 'mouse', bubbles: true, cancelable: true,
             isPrimary: true, clientX: x, clientY: y, pointerId: 1 }));
         }
         at(a.left + a.width / 2, 'pointerdown');
         for (var i = 1; i <= 8; i++) { at(a.left + (dropX - a.left) * i / 8, 'pointermove'); }
         at(dropX, 'pointerup');
         return true;",
        vec![serde_json::json!(moving), serde_json::json!(anchor)],
    )
    .await
    .as_bool()
    .unwrap_or(false);
    assert!(
        ok,
        "the {label} header or the {target} lane is not on the board"
    );
    settle_requests(&client).await;
    let moved = browser_harness::spied_requests(&client)
        .await
        .into_iter()
        .any(|r| r.url.ends_with(&format!("/lanes/{moving}/move")) && r.status == Some(200));
    assert!(
        moved,
        "the header drag must have moved {label} and been accepted"
    );
    let order: Vec<String> = board_snapshot(&client)
        .await
        .into_iter()
        .map(|(s, _)| s)
        .collect();
    let (moving_at, anchor_at) = (
        order.iter().position(|s| *s == moving),
        order.iter().position(|s| *s == anchor),
    );
    assert!(
        moving_at < anchor_at,
        "{label} must now sit left of {target}; the board reads {order:?}"
    );
}

/// The six in-place refreshes DISCUSS names. Five REPLACE `#board-columns`; the
/// header drag moves the existing lane node (see [`drag_header_left_of`]), so
/// its arm asserts the move persisted, not a replace. Recorded as a DISTILL
/// finding in feature-delta.md.
async fn refresh_in_place(world: &mut FoundryWorld, how: &str) {
    match how {
        "deletes AUTH-42 from its popup" => popup_delete(world, "AUTH-42").await,
        "renames Done to \"Shipped\" from its lane menu" => {
            rename_lane_from_menu(world, "Done", "Shipped").await
        }
        "inserts a list \"Review\" after In-Progress" => {
            insert_lane_after(world, "Review", "In-Progress").await
        }
        "deletes the Done list, moving its cards into Backlog" => {
            delete_lane_moving_cards(world, "Done", "Backlog").await
        }
        "drags the Done header to the left of In-Progress" => {
            drag_header_left_of(world, "Done", "In-Progress").await
        }
        other => panic!("unknown in-place refresh {other:?}"),
    }
    assert_not_reloaded(world).await;
}

// ============================================================== Background

#[given(
    regex = r"^Priya works two boards: Identity Platform with lanes Backlog, In-Progress and Done, and Homelab Ops with lanes Backlog, Staging, In-Progress and Done$"
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
    world.cdf_workspace_id = Some(ws);
    let team = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team)
        .bind(ws)
        .bind("Platform")
        .bind("platform")
        .execute(&pool)
        .await
        .expect("insert team");
    world.cdf_team_id = Some(team);
    let priya = seed_user(world).await;
    world.cdf_priya_id = Some(priya);
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

    let auth = seed_project(world, IDENTITY, "identity-platform", "AUTH").await;
    seed_lanes(
        world,
        auth,
        &[
            ("backlog", "Backlog"),
            ("in_progress", "In-Progress"),
            ("done", "Done"),
        ],
    )
    .await;
    let ops = seed_project(world, HOMELAB, "homelab-ops", "OPS").await;
    seed_lanes(
        world,
        ops,
        &[
            ("backlog", "Backlog"),
            ("staging", "Staging"),
            ("in_progress", "In-Progress"),
            ("done", "Done"),
        ],
    )
    .await;
}

#[given(
    regex = r"^Identity Platform's Backlog holds AUTH-41, AUTH-42 and AUTH-43, its In-Progress holds AUTH-3, AUTH-12 and AUTH-19 in that order, and its Done holds AUTH-7$"
)]
async fn given_identity_cards(world: &mut FoundryWorld) {
    seed_issues(
        world,
        IDENTITY,
        "backlog",
        &["AUTH-41", "AUTH-42", "AUTH-43"],
    )
    .await;
    seed_issues(
        world,
        IDENTITY,
        "in_progress",
        &["AUTH-3", "AUTH-12", "AUTH-19"],
    )
    .await;
    seed_issues(world, IDENTITY, "done", &["AUTH-7"]).await;
}

#[given(
    regex = r"^Homelab Ops's Backlog holds OPS-3, its Staging is empty, its In-Progress holds only OPS-7, and its Done holds OPS-9$"
)]
async fn given_homelab_cards(world: &mut FoundryWorld) {
    seed_issues(world, HOMELAB, "backlog", &["OPS-3"]).await;
    seed_issues(world, HOMELAB, "in_progress", &["OPS-7"]).await;
    seed_issues(world, HOMELAB, "done", &["OPS-9"]).await;
}

#[given(regex = r"^Homelab Ops's Done also holds (OPS-\d+)$")]
async fn given_homelab_done_also(world: &mut FoundryWorld, key: String) {
    let project_id = world.cdf_project_ids[HOMELAB];
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, state, position, author_id)
              VALUES ($1, $2, $3, $4, $5, 'done',
                      (SELECT count(*) FROM issues WHERE project_id = $2 AND state = 'done')::int, $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(world.cdf_workspace_id.expect("workspace"))
    .bind(number_of(&key))
    .bind(format!("Work item {key}"))
    .bind(world.cdf_priya_id.expect("Priya"))
    .execute(&pool(world))
    .await
    .unwrap_or_else(|err| panic!("seed {key} into Done: {err}"));
}

// ================================================================== Given

#[given(regex = r"^the device is set to the (light|dark) palette$")]
async fn given_palette(world: &mut FoundryWorld, palette: String) {
    world.cdf_dark = palette == "dark";
}

#[given(regex = r"^Homelab Ops is open in a browser$")]
async fn given_homelab_open(world: &mut FoundryWorld) {
    open_board(world, HOMELAB).await;
}

#[given(regex = r"^the Identity Platform board (?:is open in a browser|has just been loaded)$")]
async fn given_identity_open(world: &mut FoundryWorld) {
    open_board(world, IDENTITY).await;
}

#[given(
    regex = r"^the Identity Platform board is open in a browser, (freshly loaded|after Priya deleted AUTH-42 from its popup)$"
)]
async fn given_identity_open_in_state(world: &mut FoundryWorld, state: String) {
    open_board(world, IDENTITY).await;
    if state.starts_with("after") {
        popup_delete(world, "AUTH-42").await;
    }
}

#[given(regex = r"^Homelab Ops is open in a browser and ([\w-]+) shows the placeholder$")]
async fn given_homelab_open_with_placeholder(world: &mut FoundryWorld, label: String) {
    open_board(world, HOMELAB).await;
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, &label).await;
    let look = look_of(&client, &slug).await;
    assert!(
        look.1.is_empty() && look.2,
        "precondition: {label} must be empty and show the placeholder at load; it reads {look:?}"
    );
    world.cdf_looks_before = Some(lane_looks(&client).await);
}

#[given(regex = r"^(Homelab Ops|the Identity Platform board) is open in two tabs$")]
async fn given_two_tabs(world: &mut FoundryWorld, which: String) {
    let project = if which == "Homelab Ops" {
        HOMELAB
    } else {
        IDENTITY
    };
    open_board(world, project).await;
    let second = browser_harness::new_session().await;
    browser_harness::sign_in_through_browser(&second, harness(world), PRIYA_EMAIL, PRIYA_PASSWORD)
        .await;
    second
        .goto(&board_url(world, project))
        .await
        .expect("the second tab opens the board");
    browser_harness::wait_for_board_ready(&second).await;
    browser_harness::install_drag_observers(&second).await;
    world.cdf_second_tab = Some(second);
}

/// The "before" half of the remote-empty claim: the placeholder must be HIDDEN
/// while the lane still holds its card, so that "shows the placeholder" after
/// the delete proves the delete turned it on rather than it showing all along.
#[given(
    regex = r"^([\w-]+) in the second tab does not show the placeholder while it holds (\w+-\d+)$"
)]
async fn given_second_tab_no_placeholder_while_holding(
    world: &mut FoundryWorld,
    label: String,
    key: String,
) {
    let second = second_tab(world);
    let slug = lane_slug_on_screen(&second, &label).await;
    assert_eq!(
        look_of(&second, &slug).await.1,
        vec![key.clone()],
        "precondition: {label} in the second tab must hold {key} alone"
    );
    assert_placeholder(world, &second, &label, false).await;
}

#[given(regex = r"^Priya has deleted (\w+-\d+) from its popup without reloading$")]
async fn given_popup_deleted(world: &mut FoundryWorld, key: String) {
    popup_delete(world, &key).await;
}

#[given(regex = r"^Priya has moved ([\w-]+) right from its lane menu without reloading$")]
async fn given_lane_moved_right(world: &mut FoundryWorld, label: String) {
    move_lane_right_from_menu(world, &label).await;
}

#[given(
    regex = r#"^Priya has inserted a list "([^"]+)" after ([\w-]+) from its lane menu, without reloading$"#
)]
async fn given_lane_inserted(world: &mut FoundryWorld, label: String, anchor: String) {
    insert_lane_after(world, &label, &anchor).await;
}

#[given(
    regex = r#"^Priya (deletes AUTH-42 from its popup|renames Done to "Shipped" from its lane menu|inserts a list "Review" after In-Progress|deletes the Done list, moving its cards into Backlog|drags the Done header to the left of In-Progress) without reloading$"#
)]
async fn given_refreshes_in_place(world: &mut FoundryWorld, how: String) {
    refresh_in_place(world, &how).await;
}

/// "Another operator deleted it after Priya's board loaded" is the window in
/// which the live board has not yet heard of the delete: modelled as a
/// store-level delete that announces nothing, so the card stays on Priya's
/// board and her drop meets the server's uniform refusal.
#[given(regex = r"^another operator deleted (\w+-\d+) after Priya's board loaded$")]
async fn given_deleted_elsewhere(world: &mut FoundryWorld, key: String) {
    delete_elsewhere(world, &key).await;
}

async fn delete_elsewhere(world: &FoundryWorld, key: &str) {
    let project_id = world.cdf_project_ids[project_of_key(key)];
    let done = sqlx::query("DELETE FROM issues WHERE project_id = $1 AND number = $2")
        .bind(project_id)
        .bind(number_of(key))
        .execute(&pool(world))
        .await
        .unwrap_or_else(|err| panic!("delete {key} behind Priya's back: {err}"));
    assert_eq!(
        done.rows_affected(),
        1,
        "{key} must have existed to be deleted"
    );
}

#[given(regex = r"^Priya started dragging (\w+-\d+) and cancelled it with Escape$")]
async fn given_cancelled_drag(world: &mut FoundryWorld, key: String) {
    start_drag(world, &key).await;
    let origin = world.cdf_origin.clone().expect("origin").0;
    drag_over(world, DragSpot::LaneEnd(&origin)).await;
    end_drag(world).await;
}

#[given(regex = r"^Priya is dragging (\w+-\d+) over ([\w-]+)$")]
async fn given_dragging_over(world: &mut FoundryWorld, key: String, label: String) {
    if world.browser.is_none() {
        open_board(world, project_of_key(&key)).await;
    }
    when_drags_over(world, key, label.clone()).await;
    then_lane_activated(world, label).await;
}

#[given(regex = r"^the marker shows between (\w+-\d+) and (\w+-\d+) while Priya drags (\w+-\d+)$")]
async fn given_marker_between(world: &mut FoundryWorld, above: String, below: String, key: String) {
    assert_not_reloaded(world).await;
    start_drag(world, &key).await;
    drag_over(world, DragSpot::Between(&above, &below)).await;
    assert_marker_at(world, &MarkerAt::Between(above, below)).await;
}

// =================================================================== When

#[when(regex = r"^Priya drags (\w+-\d+) from ([\w-]+) over ([\w-]+)$")]
async fn when_drags_from_over(world: &mut FoundryWorld, key: String, _from: String, to: String) {
    when_drags_over(world, key, to).await;
}

#[when(regex = r"^Priya drags (\w+-\d+) over ([\w-]+)$")]
async fn when_drags_over(world: &mut FoundryWorld, key: String, label: String) {
    assert_not_reloaded(world).await;
    let client = browser(world);
    world.cdf_rects_before = Some(layout_rects(&client).await);
    let slug = lane_slug_on_screen(&client, &label).await;
    start_drag(world, &key).await;
    drag_over(world, DragSpot::LaneEnd(&slug)).await;
}

#[when(
    regex = r"^Priya drags (\w+-\d+) over ([\w-]+) (between (\w+-\d+) and (\w+-\d+)|above the middle of (\w+-\d+)|below (\w+-\d+))$"
)]
#[allow(clippy::too_many_arguments)] // one argument per regex capture group
async fn when_drags_over_slot(
    world: &mut FoundryWorld,
    key: String,
    _lane: String,
    _whole: String,
    above: String,
    below: String,
    above_middle: String,
    below_card: String,
) {
    assert_not_reloaded(world).await;
    start_drag(world, &key).await;
    if !above.is_empty() {
        drag_over(world, DragSpot::Between(&above, &below)).await;
    } else if !above_middle.is_empty() {
        drag_over(world, DragSpot::AboveMiddleOf(&above_middle)).await;
    } else {
        drag_over(world, DragSpot::Below(&below_card)).await;
    }
}

/// An upward reorder that first passes over the dragged card's OWN top half,
/// then carries on to the gap between `above` and `below`. Every marker seen on
/// the way is recorded for [`then_marker_never_above_itself`] (AC-3.4).
///
/// The first hover is the only point where counting the card as its own
/// neighbour changes the slot: below the midline of the card above it and above
/// its own midline. The aim is proved from live geometry, so a layout change
/// cannot quietly move it out of that band and leave the scenario vacuous.
#[when(regex = r"^Priya drags (\w+-\d+) up from its own slot to between (\w+-\d+) and (\w+-\d+)$")]
async fn when_drags_up_from_own_slot(
    world: &mut FoundryWorld,
    key: String,
    above: String,
    below: String,
) {
    assert_not_reloaded(world).await;
    start_drag(world, &key).await;
    let card_above = world
        .cdf_origin
        .as_ref()
        .and_then(|origin| origin.1.clone())
        .unwrap_or_else(|| panic!("{key} must have a card above it to be dragged up"));
    let client = browser(world);
    world.cdf_marker_readings.clear();

    drag_over(world, DragSpot::AboveMiddleOf(&key)).await;
    let aim: (f64, f64) = serde_json::from_value(
        js(
            &client,
            "var p = window.__synthDrag.lastPoint; return [p.x, p.y];",
            vec![],
        )
        .await,
    )
    .expect("the last drag point");
    let (own_top, own_bottom) = card_rect(&client, &key).await;
    let (above_top, above_bottom) = card_rect(&client, &card_above).await;
    let own_mid = (own_top + own_bottom) / 2.0;
    let above_mid = (above_top + above_bottom) / 2.0;
    assert!(
        aim.1 > above_mid && aim.1 < own_mid,
        "the pointer must be over {key}'s own slot: below {card_above}'s middle ({above_mid}) and above {key}'s own middle ({own_mid}); it is at {}",
        aim.1
    );
    world.cdf_marker_readings.push(markers(&client).await);

    drag_over(world, DragSpot::Between(&above, &below)).await;
    world.cdf_marker_readings.push(markers(&client).await);
}

#[when(regex = r"^Priya drags (\w+-\d+) into ([\w-]+) and drops it$")]
async fn when_drags_into(world: &mut FoundryWorld, key: String, label: String) {
    let slug = lane_spot_slug(world, &label).await;
    drag_and_drop(world, &key, DragSpot::LaneEnd(&slug)).await;
}

#[when(
    regex = r"^Priya drags (\w+-\d+) from ([\w-]+) into (?:the empty )?([\w-]+?)(?: lane)? and drops it$"
)]
async fn when_drags_from_into(world: &mut FoundryWorld, key: String, _from: String, label: String) {
    when_drags_into(world, key, label).await;
}

#[when(regex = r"^Priya drags (\w+-\d+), the only card in ([\w-]+), into ([\w-]+) and drops it$")]
async fn when_drags_only_card(world: &mut FoundryWorld, key: String, from: String, label: String) {
    let client = browser(world);
    let from_slug = lane_slug_on_screen(&client, &from).await;
    let look = look_of(&client, &from_slug).await;
    assert_eq!(
        look.1,
        vec![key.clone()],
        "precondition: {key} must be the only card in {from}"
    );
    when_drags_into(world, key, label).await;
}

#[when(regex = r"^Priya drags (\w+-\d+) between (\w+-\d+) and (\w+-\d+) and drops it$")]
async fn when_drags_between_and_drops(
    world: &mut FoundryWorld,
    key: String,
    above: String,
    below: String,
) {
    drag_and_drop(world, &key, DragSpot::Between(&above, &below)).await;
}

#[when(regex = r"^Priya drags (\w+-\d+) to the top of ([\w-]+) and drops it$")]
async fn when_drags_to_top(world: &mut FoundryWorld, key: String, label: String) {
    let slug = lane_spot_slug(world, &label).await;
    drag_and_drop(world, &key, DragSpot::LaneTop(&slug)).await;
}

#[when(regex = r"^she moves it over ([\w-]+)$")]
async fn when_moves_over(world: &mut FoundryWorld, label: String) {
    let slug = lane_spot_slug(world, &label).await;
    drag_over(world, DragSpot::LaneEnd(&slug)).await;
}

#[when(regex = r"^she moves the card over ([\w-]+) below (\w+-\d+)$")]
async fn when_moves_card_below(world: &mut FoundryWorld, _label: String, key: String) {
    drag_over(world, DragSpot::Below(&key)).await;
}

#[when(regex = r"^she (moves it over the page header|carries it out of the browser window)$")]
async fn when_leaves_every_lane(world: &mut FoundryWorld, how: String) {
    let client = browser(world);
    if how.starts_with("moves") {
        drag_over(world, DragSpot::PageHeader).await;
    } else {
        // Leaving the window: a dragleave with no relatedTarget and no
        // dragover after it. DDD-3 clears on the next animation frame.
        let lane = lane_slug_on_screen(&client, "Done").await;
        browser_harness::drag_leave(&client, DragSpot::LaneEnd(&lane), None).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// DDD-8f: the AC-2.2 oracle is read BETWEEN the dragleave and the next
/// dragover, which is where a clear-on-every-leave implementation fails.
#[when(regex = r"^the pointer passes over (\w+-\d+) inside ([\w-]+)$")]
async fn when_pointer_passes_over_card(world: &mut FoundryWorld, key: String, label: String) {
    let client = browser(world);
    let lane = lane_slug_on_screen(&client, &label).await;
    browser_harness::drag_enter(&client, DragSpot::OnCard(&key)).await;
    browser_harness::drag_leave(
        &client,
        DragSpot::LaneEnd(&lane),
        Some(DragSpot::OnCard(&key)),
    )
    .await;
}

#[when(regex = r"^the drag reports the same pointer position several more times$")]
async fn when_still_pointer(world: &mut FoundryWorld) {
    let client = browser(world);
    world.cdf_marker_readings.clear();
    for _ in 0..5 {
        browser_harness::drag_over(&client, DragSpot::SamePoint).await;
        world.cdf_marker_readings.push(markers(&client).await);
    }
}

#[when(
    regex = r"^she (drops it|drops it on [\w-]+|drops it and the server refuses the move|drops it on [\w-]+ and the server refuses the move|presses Escape|releases it over the page header)$"
)]
async fn when_drag_ends(world: &mut FoundryWorld, how: String) {
    let key = world.cdf_dragging.clone().expect("a card drag in progress");
    if how.ends_with("and the server refuses the move") {
        delete_elsewhere(world, &key).await;
    }
    if let Some(rest) = how.strip_prefix("drops it on ") {
        let label = rest.trim_end_matches(" and the server refuses the move");
        let slug = lane_spot_slug(world, label).await;
        drag_over(world, DragSpot::LaneEnd(&slug)).await;
    }
    if how.starts_with("drops it") {
        let dropped = drop_here(world).await;
        assert!(
            dropped,
            "MISSING_FUNCTIONALITY: the lane did not accept the drop of {key} (its dragover was \
             not claimed)"
        );
    } else if how.starts_with("releases") {
        drag_over(world, DragSpot::PageHeader).await;
        drop_here(world).await;
    }
    // Escape, a release outside any lane and a drop all end with dragend.
    end_drag(world).await;
    settle_requests(&browser(world)).await;
}

#[when(
    regex = r"^Priya drags (\w+-\d+) over ([\w-]+) and (presses Escape|releases it over the page header)$"
)]
async fn when_hover_then_cancel(world: &mut FoundryWorld, key: String, label: String, how: String) {
    when_drags_over(world, key, label).await;
    when_drag_ends(world, how).await;
}

#[when(
    regex = r#"^(a file "[^"]+"|a text selection from another app) is (?:then )?dragged in from outside the page and dropped on (.+)$"#
)]
async fn when_foreign_drop(world: &mut FoundryWorld, what: String, place: String) {
    assert_not_reloaded(world).await;
    let client = browser(world);
    world.cdf_board_before = Some(board_snapshot(&client).await);
    world.cdf_looks_before = Some(lane_looks(&client).await);
    world.cdf_moves_before = Some(move_requests(&client).await.len());
    world.cdf_url_before = Some(client.current_url().await.expect("url").to_string());
    begin_foreign(&client, &what).await;
    let resolved = resolve_where(world, &place).await;
    let claimed = browser_harness::drag_over(&client, resolved.spot()).await;
    let dropped = browser_harness::drag_drop(&client, DragSpot::SamePoint).await;
    world.cdf_over_claimed = Some(claimed);
    world.cdf_drop_claimed = Some(dropped);
}

async fn begin_foreign(client: &fantoccini::Client, what: &str) {
    if let Some(name) = what
        .strip_prefix("a file \"")
        .and_then(|r| r.strip_suffix('"'))
    {
        browser_harness::drag_start_foreign(client, ForeignPayload::File(name)).await;
    } else {
        browser_harness::drag_start_foreign(client, ForeignPayload::Text("release notes, pasted"))
            .await;
    }
}

#[when(regex = r#"^a file "([^"]+)" is dragged in from outside the page over ([\w-]+)$"#)]
async fn when_foreign_over(world: &mut FoundryWorld, name: String, label: String) {
    assert_not_reloaded(world).await;
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, &label).await;
    browser_harness::drag_start_foreign(&client, ForeignPayload::File(&name)).await;
    let claimed = browser_harness::drag_over(&client, DragSpot::LaneEnd(&slug)).await;
    world.cdf_over_claimed = Some(claimed);
}

#[when(
    regex = r"^Priya drags (\w+-\d+) out of the second tab and drops it on ([\w-]+) in the first$"
)]
async fn when_drags_from_other_tab(world: &mut FoundryWorld, key: String, label: String) {
    let first = browser(world);
    let second = second_tab(world);
    world.cdf_board_before = Some(board_snapshot(&first).await);
    world.cdf_looks_before = Some(lane_looks(&second).await);
    browser_harness::drag_start(&second, &key).await;
    // The first tab sees no dragstart: only a transfer carrying the card's key
    // as text/plain, exactly as our own card drag sets it.
    browser_harness::drag_start_foreign(&first, ForeignPayload::Text(&key)).await;
    let slug = lane_slug_on_screen(&first, &label).await;
    world.cdf_over_claimed =
        Some(browser_harness::drag_over(&first, DragSpot::LaneEnd(&slug)).await);
    world.cdf_drop_claimed = Some(browser_harness::drag_drop(&first, DragSpot::SamePoint).await);
    browser_harness::drag_end(&second, &key).await;
}

#[when(
    regex = r"^Priya deletes (\w+-\d+)(?:, the only card in ([\w-]+),)? from its popup in the first tab$"
)]
async fn when_deletes_in_first_tab(world: &mut FoundryWorld, key: String, only_in: String) {
    let second = second_tab(world);
    if !only_in.is_empty() {
        let slug = lane_slug_on_screen(&second, &only_in).await;
        assert_eq!(
            look_of(&second, &slug).await.1,
            vec![key.clone()],
            "precondition: {key} is alone in {only_in}"
        );
    }
    world.cdf_second_looks_before = Some(lane_looks(&second).await);
    popup_delete(world, &key).await;
}

// =================================================================== Then

#[then(regex = r"^([\w-]+) accepts the drag$")]
async fn then_accepts(world: &mut FoundryWorld, label: String) {
    assert_not_reloaded(world).await;
    assert_eq!(
        world.cdf_over_claimed,
        Some(true),
        "MISSING_FUNCTIONALITY: {label} did not claim the card drag: the synthetic dragover on the \
         lane now on screen was NOT defaultPrevented. The board was refreshed in place without a \
         reload, and board-dnd.js bound its dragover/drop to the lanes present at page load \
         (rca-drag-after-board-replace.md, AC-1.1)"
    );
}

async fn assert_card_in(world: &FoundryWorld, key: &str, label: &str) {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, label).await;
    let place = card_place(&client, key).await;
    assert_eq!(
        place.as_ref().map(|p| p.0.as_str()),
        Some(slug.as_str()),
        "{key} must be in {label}; it is at {place:?}"
    );
}

#[then(regex = r"^after she drops it, (\w+-\d+) is in ([\w-]+)$")]
async fn then_after_drop_in(world: &mut FoundryWorld, key: String, label: String) {
    release_accepted(world, &key, &label).await;
    settle_requests(&browser(world)).await;
    assert_card_in(world, &key, &label).await;
}

#[then(regex = r"^(\w+-\d+) is in ([\w-]+)$")]
async fn then_is_in(world: &mut FoundryWorld, key: String, label: String) {
    assert_card_in(world, &key, &label).await;
    let accepted = move_requests(&browser(world)).await.into_iter().any(|r| {
        r.url
            .ends_with(&format!("/issues/{}/state", number_of(&key)))
            && r.status == Some(200)
    });
    assert!(
        accepted,
        "the move of {key} must have been sent and accepted"
    );
}

#[then(regex = r"^after a reload (\w+-\d+) is still in ([\w-]+)$")]
async fn then_after_reload_still_in(world: &mut FoundryWorld, key: String, label: String) {
    reload_board(world).await;
    assert_card_in(world, &key, &label).await;
}

async fn lane_keys(world: &FoundryWorld, label: &str) -> Vec<String> {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, label).await;
    look_of(&client, &slug).await.1
}

#[then(regex = r"^([\w-]+) reads ((?:\w+-\d+)(?:, \w+-\d+)*)$")]
async fn then_lane_reads(world: &mut FoundryWorld, label: String, list: String) {
    assert_eq!(
        lane_keys(world, &label).await,
        keys_of(&list),
        "{label} on screen"
    );
}

#[then(regex = r"^after a reload ([\w-]+) reads ((?:\w+-\d+)(?:, \w+-\d+)*)$")]
async fn then_after_reload_reads(world: &mut FoundryWorld, label: String, list: String) {
    reload_board(world).await;
    assert_eq!(
        lane_keys(world, &label).await,
        keys_of(&list),
        "{label} after a reload"
    );
}

#[then(regex = r"^after a reload (\w+-\d+) is first in ([\w-]+)$")]
async fn then_after_reload_first(world: &mut FoundryWorld, key: String, label: String) {
    reload_board(world).await;
    assert_eq!(
        lane_keys(world, &label).await.first(),
        Some(&key),
        "{key} first in {label}"
    );
}

#[then(regex = r"^after she drops it and reloads, (\w+-\d+) is (first|last) in ([\w-]+)$")]
async fn then_drop_reload_position(
    world: &mut FoundryWorld,
    key: String,
    end: String,
    label: String,
) {
    release_accepted(world, &key, &label).await;
    reload_board(world).await;
    let keys = lane_keys(world, &label).await;
    let at = if end == "first" {
        keys.first()
    } else {
        keys.last()
    };
    assert_eq!(
        at,
        Some(&key),
        "{key} must be {end} in {label} after a reload; it reads {keys:?}"
    );
}

#[then(regex = r"^after she drops it and reloads, ([\w-]+) reads ((?:\w+-\d+)(?:, \w+-\d+)*)$")]
async fn then_drop_reload_reads(world: &mut FoundryWorld, label: String, list: String) {
    let key = world.cdf_dragging.clone().expect("a card drag in progress");
    release_accepted(world, &key, &label).await;
    reload_board(world).await;
    assert_eq!(
        lane_keys(world, &label).await,
        keys_of(&list),
        "{label} after a reload"
    );
}

fn only_move(requests: &[SpiedRequest]) -> &SpiedRequest {
    assert_eq!(
        requests.len(),
        1,
        "exactly one move request must have been sent: {requests:?}"
    );
    &requests[0]
}

#[then(regex = r"^the move request names (\w+-\d+) as the card above$")]
async fn then_request_names_above(world: &mut FoundryWorld, above: String) {
    let requests = move_requests(&browser(world)).await;
    let request = only_move(&requests);
    let body = request.body.clone().unwrap_or_default();
    assert!(
        body.ends_with(&format!("&after={above}")) && body.starts_with("state="),
        "the move request must carry state=<lane>&after={above}, byte for byte as shipped \
         (AC-1.4); it carried {body:?}"
    );
    assert!(
        !request.csrf.is_empty(),
        "the move request must carry its x-csrf-token header"
    );
}

#[then(regex = r"^the move request carries the destination lane and names no card above$")]
async fn then_request_top(world: &mut FoundryWorld) {
    let requests = move_requests(&browser(world)).await;
    let request = only_move(&requests);
    assert_eq!(
        request.body.as_deref(),
        Some("state=in_progress"),
        "a drop at the top sends state alone, byte for byte as shipped (AC-1.4, D9)"
    );
    assert!(
        !request.csrf.is_empty(),
        "the move request must carry its x-csrf-token header"
    );
    assert_eq!(request.status, Some(200), "the move must be accepted");
}

async fn assert_nothing_moved(
    world: &FoundryWorld,
    client: &fantoccini::Client,
    before: &[(String, Vec<String>)],
) {
    let now = board_snapshot(client).await;
    assert_eq!(
        now, before,
        "MISSING_FUNCTIONALITY: a card MOVED on a drop that was not a card drag begun on this \
         page (D4, AC-1.5/1.6). On HEAD the in-flight card survives a cancelled drag, so the \
         next foreign drop is mistaken for it"
    );
    let sent = move_requests(client).await.len();
    let baseline = world.cdf_moves_before.unwrap_or(0);
    assert_eq!(sent, baseline, "no move request may be sent");
}

#[then(regex = r"^no card moves and no move request is sent$")]
async fn then_nothing_moves(world: &mut FoundryWorld) {
    let client = browser(world);
    let before = world
        .cdf_board_before
        .clone()
        .expect("board captured before the foreign drop");
    assert_nothing_moved(world, &client, &before).await;
}

#[then(regex = r"^the tab still shows the Identity Platform board$")]
async fn then_tab_still_board(world: &mut FoundryWorld) {
    let client = browser(world);
    let url = client.current_url().await.expect("url").to_string();
    assert_eq!(
        Some(url),
        world.cdf_url_before,
        "the tab must not have navigated"
    );
    browser_harness::wait_for_board_ready(&client).await;
    // A synthetic event cannot navigate, so the swallow is observed as its
    // cause: the board must claim the dragover (else a real browser refuses
    // the drop and opens the file) AND cancel the drop (else it opens it).
    assert_eq!(
        (world.cdf_over_claimed, world.cdf_drop_claimed),
        (Some(true), Some(true)),
        "MISSING_FUNCTIONALITY: the board did not swallow the foreign drop (dragover claimed, \
         drop claimed) — a real browser would open the file in this tab (D4, AC-1.5)"
    );
}

#[then(regex = r"^the board shows that it will not take the drop$")]
async fn then_board_refuses_the_drop(world: &mut FoundryWorld) {
    // The board claims a foreign dragover only to swallow it; its answer on
    // the dragover is `dropEffect`, which a real browser shows as the cursor.
    // `none` is the honest no-drop cursor (DDD-6, ADR-BOARD-CARD-001). Only a
    // card drag begun on this page may answer `move`.
    let client = browser(world);
    let effect = browser_harness::last_drop_effect(&client).await;
    assert_eq!(
        effect.as_deref(),
        Some("none"),
        "MISSING_FUNCTIONALITY: the board offered to take a drop it can only swallow: the \
         foreign dragover's dropEffect was not \"none\", so a real browser shows a move cursor \
         for something that names no card on this page (DDD-6, ADR-BOARD-CARD-001, AC-1.5)"
    );
}

#[then(regex = r"^no card moves in either tab and no move request is sent$")]
async fn then_nothing_moves_either_tab(world: &mut FoundryWorld) {
    let first = browser(world);
    let second = second_tab(world);
    let before = world.cdf_board_before.clone().expect("first tab captured");
    assert_nothing_moved(world, &first, &before).await;
    let second_before: Vec<(String, Vec<String>)> = world
        .cdf_looks_before
        .clone()
        .expect("second tab captured")
        .into_iter()
        .map(|(s, k, _, _)| (s, k))
        .collect();
    assert_eq!(
        board_snapshot(&second).await,
        second_before,
        "no card may move in the second tab"
    );
    assert!(
        move_requests(&second).await.is_empty(),
        "the second tab must send no move request"
    );
    assert_eq!(
        world.cdf_drop_claimed,
        Some(true),
        "the first tab must swallow the drop, not leave it to the browser (D4)"
    );
}

async fn wait_for_place(world: &FoundryWorld, key: &str, expected: &Place) -> Option<Place> {
    let client = browser(world);
    let deadline = Instant::now() + BROWSER_WAIT;
    loop {
        let now = card_place(&client, key).await;
        if now.as_ref() == Some(expected) || Instant::now() > deadline {
            return now;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn assert_back_at_origin(world: &FoundryWorld, key: &str) {
    let origin = world
        .cdf_origin
        .clone()
        .expect("origin captured at dragstart");
    let now = wait_for_place(world, key, &origin).await;
    assert_eq!(
        now.as_ref(),
        Some(&origin),
        "{key} must be back in its exact origin slot"
    );
}

#[then(regex = r"^(\w+-\d+) is still in its slot in ([\w-]+)$")]
async fn then_still_in_slot(world: &mut FoundryWorld, key: String, _label: String) {
    assert_back_at_origin(world, &key).await;
}

async fn assert_refused(world: &FoundryWorld, key: &str) {
    let refused = move_requests(&browser(world)).await.into_iter().any(|r| {
        r.url
            .ends_with(&format!("/issues/{}/state", number_of(key)))
            && r.status == Some(404)
    });
    assert!(
        refused,
        "the move of {key} must have been SENT and REFUSED (uniform 404); a card that never left \
         its slot would 'return' trivially"
    );
}

#[then(regex = r"^(\w+-\d+) returns to its original slot in ([\w-]+)$")]
async fn then_returns(world: &mut FoundryWorld, key: String, _label: String) {
    assert_refused(world, &key).await;
    assert_back_at_origin(world, &key).await;
}

#[then(regex = r"^(\w+-\d+) is (between (\w+-\d+) and (\w+-\d+)|back in its slot in ([\w-]+))$")]
async fn then_is_where(
    world: &mut FoundryWorld,
    key: String,
    _whole: String,
    above: String,
    below: String,
    _lane: String,
) {
    if above.is_empty() {
        assert_back_at_origin(world, &key).await;
    } else {
        let place = card_place(&browser(world), &key)
            .await
            .expect("card on board");
        assert_eq!(
            (place.1, place.2),
            (Some(above), Some(below)),
            "{key}'s neighbours"
        );
    }
}

// ---- activation -----------------------------------------------------------

#[then(regex = r"^([\w-]+) is (?:still )?shown as activated$")]
async fn then_lane_activated(world: &mut FoundryWorld, label: String) {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, &label).await;
    let lit = activated_lanes(&client).await;
    assert!(
        lit.contains(&slug),
        "MISSING_FUNCTIONALITY: {label} is not shown as activated while the card is over it \
         ({ACTIVATED} on lanes: {lit:?}; US-CDF-02, DDD-3)"
    );
}

#[then(regex = r"^no other lane is shown as activated$")]
async fn then_no_other_lane(world: &mut FoundryWorld) {
    let lit = activated_lanes(&browser(world)).await;
    assert_eq!(lit.len(), 1, "exactly one lane may be activated: {lit:?}");
}

#[then(regex = r"^([\w-]+) is shown as activated and ([\w-]+) is not$")]
async fn then_one_not_other(world: &mut FoundryWorld, on: String, off: String) {
    then_lane_activated(world, on.clone()).await;
    let client = browser(world);
    let off_slug = lane_slug_on_screen(&client, &off).await;
    let lit = activated_lanes(&client).await;
    assert!(
        !lit.contains(&off_slug),
        "{off} must no longer be activated: {lit:?}"
    );
    assert_eq!(lit.len(), 1, "exactly one lane may be activated: {lit:?}");
}

#[then(regex = r"^no lane is shown as activated$")]
async fn then_no_lane(world: &mut FoundryWorld) {
    let lit = activated_lanes(&browser(world)).await;
    assert!(lit.is_empty(), "no lane may stay activated: {lit:?}");
}

#[then(regex = r"^a card dragged over ([\w-]+) straight afterwards does light it$")]
async fn then_control_lights(world: &mut FoundryWorld, label: String) {
    let key = if world.cdf_current_project.as_deref() == Some(HOMELAB) {
        "OPS-3"
    } else {
        "AUTH-41"
    };
    when_drags_over(world, key.to_string(), label.clone()).await;
    then_lane_activated(world, label).await;
}

#[then(regex = r"^the activated lane's boundary measures at least 3:1 against the page$")]
async fn then_activation_contrast(world: &mut FoundryWorld) {
    let client = browser(world);
    if world.cdf_dark {
        assert!(
            browser_harness::device_prefers_dark(&client).await,
            "the device must prefer dark"
        );
    }
    let lit = activated_lanes(&client).await;
    assert!(
        !lit.is_empty(),
        "MISSING_FUNCTIONALITY: no lane is activated to measure ({ACTIVATED})"
    );
    let probe = js(
        &client,
        &format!(
            "var l = document.querySelector('#board-columns [data-column=\"' + arguments[0] + '\"]');
             var cs = getComputedStyle(l);
             {OPAQUE_BACKGROUND_JS}
             return {{ outline: cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) >= 1 ? cs.outlineColor : null,
                      border: cs.borderTopStyle !== 'none' && parseFloat(cs.borderTopWidth) >= 1 ? cs.borderTopColor : null,
                      page: opaque(l.parentElement) }};"
        ),
        vec![serde_json::json!(lit[0])],
    )
    .await;
    let (page, _) = parse_colour(probe["page"].as_str().unwrap_or_default()).expect("page colour");
    let best = best_boundary_contrast(&probe, &["outline", "border"], page);
    assert!(
        best >= 3.0,
        "the activated lane's boundary measures only {best:.2}:1 against the page {} (D6, WCAG \
         1.4.11); probe: {probe}",
        hex(page)
    );
}

#[then(regex = r"^no card and no column has moved or changed size$")]
async fn then_no_reflow(world: &mut FoundryWorld) {
    let before = world
        .cdf_rects_before
        .clone()
        .expect("rects captured before the drag");
    let after = layout_rects(&browser(world)).await;
    let (b, a) = (
        before.as_object().expect("rects"),
        after.as_object().expect("rects"),
    );
    assert_eq!(
        b.len(),
        a.len(),
        "the same cards and columns must be on the board"
    );
    for (name, rect) in b {
        let was: Vec<f64> = serde_json::from_value(rect.clone()).expect("rect");
        let now: Vec<f64> = serde_json::from_value(a[name].clone()).expect("rect");
        let moved = was.iter().zip(&now).any(|(x, y)| (x - y).abs() > 0.5);
        assert!(
            !moved,
            "{name} moved or resized when the lane activated: {was:?} -> {now:?} (AC-2.6)"
        );
    }
}

// ---- the marker -----------------------------------------------------------

enum MarkerAt {
    Between(String, String),
    Above(String),
    Below(String),
}

async fn assert_marker_at(world: &FoundryWorld, at: &MarkerAt) {
    let client = browser(world);
    let found = markers(&client).await;
    assert!(
        !found.is_empty(),
        "MISSING_FUNCTIONALITY: no marker shows where the card would land ({MARKER}; US-CDF-03, DDD-4)"
    );
    assert_eq!(found.len(), 1, "exactly one marker may show: {found:?}");
    let (lane, before, mid) = found[0].clone();
    let (want_before, anchor) = match at {
        MarkerAt::Between(_, below) => (below.clone(), below.clone()),
        MarkerAt::Above(key) => (key.clone(), key.clone()),
        MarkerAt::Below(key) => {
            let place = card_place(&client, key).await.expect("card on board");
            (place.2.unwrap_or_default(), key.clone())
        }
    };
    let anchor_lane = card_place(&client, &anchor)
        .await
        .expect("anchor on board")
        .0;
    assert_eq!(
        lane, anchor_lane,
        "the marker must sit in the lane of {anchor}"
    );
    assert_eq!(
        before, want_before,
        "the marker's slot (data-before-key; empty is the end)"
    );
    let tolerance = 6.0;
    match at {
        MarkerAt::Between(above, below) => {
            let (_, a_bottom) = card_rect(&client, above).await;
            let (b_top, _) = card_rect(&client, below).await;
            assert!(
                mid >= a_bottom - tolerance && mid <= b_top + tolerance,
                "the marker must show between {above} and {below} (midline {mid}, gap {a_bottom}..{b_top})"
            );
        }
        MarkerAt::Above(key) => {
            let (top, _) = card_rect(&client, key).await;
            assert!(
                mid <= top + tolerance,
                "the marker must show above {key} (midline {mid}, card top {top})"
            );
        }
        MarkerAt::Below(key) => {
            let (_, bottom) = card_rect(&client, key).await;
            assert!(
                mid >= bottom - tolerance,
                "the marker must show below {key} (midline {mid}, card bottom {bottom})"
            );
        }
    }
}

#[then(
    regex = r"^(?:a|the) marker shows (between (\w+-\d+) and (\w+-\d+)|above (\w+-\d+)|below (\w+-\d+))$"
)]
async fn then_marker_shows(
    world: &mut FoundryWorld,
    _whole: String,
    above: String,
    below: String,
    above_of: String,
    below_of: String,
) {
    let at = if !above.is_empty() {
        MarkerAt::Between(above, below)
    } else if !above_of.is_empty() {
        MarkerAt::Above(above_of)
    } else {
        MarkerAt::Below(below_of)
    };
    assert_marker_at(world, &at).await;
}

/// AC-3.4: the dragged card is never the marker's neighbour. A marker "above"
/// a card is one whose `data-before-key` names it, so the oracle is that no
/// marker recorded during the drag names the dragged card.
#[then(regex = r"^the marker was never shown above (\w+-\d+) itself while she dragged it$")]
async fn then_marker_never_above_itself(world: &mut FoundryWorld, key: String) {
    assert!(
        !world.cdf_marker_readings.is_empty(),
        "the drag must have recorded the markers it showed"
    );
    assert!(
        world
            .cdf_marker_readings
            .iter()
            .all(|seen| !seen.is_empty()),
        "MISSING_FUNCTIONALITY: every hover inside the lane must show a marker: {:?}",
        world.cdf_marker_readings
    );
    for (i, seen) in world.cdf_marker_readings.iter().enumerate() {
        assert!(
            seen.iter().all(|(_, before, _)| *before != key),
            "the dragged card's own slot was offered: hover {i} showed the marker above {key} itself (data-before-key names the dragged card; AC-3.4, invariant 5): {seen:?}"
        );
    }
}

#[then(regex = r"^it is the only marker on the board$")]
async fn then_only_marker(world: &mut FoundryWorld) {
    let found = markers(&browser(world)).await;
    assert_eq!(found.len(), 1, "exactly one marker on the board: {found:?}");
}

#[then(regex = r"^the only marker on the board shows below (\w+-\d+)$")]
async fn then_only_marker_below(world: &mut FoundryWorld, key: String) {
    assert_marker_at(world, &MarkerAt::Below(key)).await;
}

#[then(regex = r"^the marker still shows between (\w+-\d+) and (\w+-\d+)$")]
async fn then_marker_still(world: &mut FoundryWorld, above: String, below: String) {
    assert!(
        !world.cdf_marker_readings.is_empty(),
        "the pointer must have held still"
    );
    let first = world.cdf_marker_readings[0].clone();
    for (i, reading) in world.cdf_marker_readings.iter().enumerate() {
        assert_eq!(
            reading.len(),
            1,
            "exactly one marker on repeat {i}: {reading:?}"
        );
        let moved = reading[0].0 != first[0].0
            || reading[0].1 != first[0].1
            || (reading[0].2 - first[0].2).abs() > 0.5;
        assert!(
            !moved,
            "a still pointer moved the marker on repeat {i}: {first:?} -> {reading:?} (AC-3.5)"
        );
    }
    assert_marker_at(world, &MarkerAt::Between(above, below)).await;
}

#[then(regex = r"^no marker shows anywhere on the board$")]
async fn then_no_marker(world: &mut FoundryWorld) {
    let found = markers(&browser(world)).await;
    assert!(
        found.is_empty(),
        "no marker may outlive or precede a card drag: {found:?}"
    );
}

#[then(regex = r"^a card dragged over ([\w-]+) straight afterwards does show a marker$")]
async fn then_control_marker(world: &mut FoundryWorld, label: String) {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, &label).await;
    start_drag(world, "AUTH-41").await;
    drag_over(world, DragSpot::LaneEnd(&slug)).await;
    let found = markers(&client).await;
    assert_eq!(
        found.len(),
        1,
        "MISSING_FUNCTIONALITY: a card dragged over {label} shows no marker, so 'no marker for a \
         file' proves nothing yet ({MARKER}): {found:?}"
    );
}

#[then(regex = r"^the marker measures at least 3:1 against the lane behind it$")]
async fn then_marker_contrast(world: &mut FoundryWorld) {
    let client = browser(world);
    if world.cdf_dark {
        assert!(
            browser_harness::device_prefers_dark(&client).await,
            "the device must prefer dark"
        );
    }
    let probe = js(
        &client,
        &format!(
            "var m = document.querySelector('{MARKER}'); if (!m) {{ return null; }}
             var cs = getComputedStyle(m);
             {OPAQUE_BACKGROUND_JS}
             return {{ fill: cs.backgroundColor,
                      border: cs.borderTopStyle !== 'none' && parseFloat(cs.borderTopWidth) >= 1 ? cs.borderTopColor : null,
                      behind: opaque(m.parentElement) }};"
        ),
        vec![],
    )
    .await;
    assert!(
        !probe.is_null(),
        "MISSING_FUNCTIONALITY: no marker shows to measure ({MARKER})"
    );
    let (behind, _) =
        parse_colour(probe["behind"].as_str().unwrap_or_default()).expect("lane colour");
    let best = best_boundary_contrast(&probe, &["fill", "border"], behind);
    assert!(
        best >= 3.0,
        "the marker measures only {best:.2}:1 against the lane {} (D8); probe: {probe}",
        hex(behind)
    );
}

// ---- placeholders -----------------------------------------------------------

async fn assert_placeholder(
    world: &FoundryWorld,
    client: &fantoccini::Client,
    label: &str,
    shown: bool,
) {
    let slug = lane_slug_on_screen(client, label).await;
    let deadline = Instant::now() + BROWSER_WAIT;
    let mut look = look_of(client, &slug).await;
    while look.2 != shown && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
        look = look_of(client, &slug).await;
    }
    if shown {
        assert!(
            look.2,
            "MISSING_FUNCTIONALITY: {label} holds no card yet does not display the \"No issues \
             yet\" placeholder (US-CDF-04, DDD-5); it reads {look:?}"
        );
        assert!(
            look.1.is_empty(),
            "{label} displays the placeholder but holds cards: {look:?}"
        );
        if let Some(server) = &world.cdf_server_placeholder {
            assert_eq!(
                &look.3, server,
                "the placeholder must be the server's own markup (AC-4.2)"
            );
        }
    } else {
        assert!(
            !look.2,
            "MISSING_FUNCTIONALITY: {label} still displays the \"No issues yet\" placeholder \
             beside a card (US-CDF-04, DDD-5); it reads {look:?}"
        );
    }
}

#[then(regex = r"^([\w-]+) shows (\w+-\d+) and no placeholder$")]
async fn then_shows_card_no_placeholder(world: &mut FoundryWorld, label: String, key: String) {
    assert_card_in(world, &key, &label).await;
    assert_placeholder(world, &browser(world), &label, false).await;
}

#[then(
    regex = r"^([\w-]+) shows the placeholder(?:, with the same words and markup a freshly loaded empty lane shows)?$"
)]
async fn then_shows_placeholder(world: &mut FoundryWorld, label: String) {
    assert_placeholder(world, &browser(world), &label, true).await;
}

#[then(regex = r"^([\w-]+) shows its placeholder again$")]
async fn then_placeholder_again(world: &mut FoundryWorld, label: String) {
    assert_placeholder(world, &browser(world), &label, true).await;
}

#[then(regex = r"^(\w+-\d+) is back in ([\w-]+) and ([\w-]+) shows no placeholder$")]
async fn then_back_no_placeholder(
    world: &mut FoundryWorld,
    key: String,
    _lane: String,
    label: String,
) {
    assert_refused(world, &key).await;
    assert_back_at_origin(world, &key).await;
    assert_placeholder(world, &browser(world), &label, false).await;
}

async fn reload_and_compare(world: &mut FoundryWorld, only: Option<&str>) {
    let client = browser(world);
    settle_requests(&client).await;
    let mut before = lane_looks(&client).await;
    reload_board(world).await;
    let mut after = lane_looks(&browser(world)).await;
    if let Some(label) = only {
        let slug = lane_slug_on_screen(&browser(world), label).await;
        before.retain(|l| l.0 == slug);
        after.retain(|l| l.0 == slug);
    }
    assert_eq!(after, before, "a reload must change nothing visible (D11)");
}

#[then(regex = r"^after a reload ([\w-]+) looks exactly the same$")]
async fn then_reload_same(world: &mut FoundryWorld, label: String) {
    reload_and_compare(world, Some(&label)).await;
}

#[then(regex = r"^after a reload both lanes look exactly the same$")]
async fn then_reload_both_same(world: &mut FoundryWorld) {
    reload_and_compare(world, None).await;
}

#[then(regex = r"^([\w-]+) still shows its placeholder, unchanged$")]
async fn then_placeholder_unchanged(world: &mut FoundryWorld, label: String) {
    let client = browser(world);
    let slug = lane_slug_on_screen(&client, &label).await;
    let before = world
        .cdf_looks_before
        .clone()
        .expect("looks captured")
        .into_iter()
        .find(|l| l.0 == slug)
        .expect("lane captured");
    assert_eq!(
        look_of(&client, &slug).await,
        before,
        "{label} must be exactly as it was"
    );
}

/// The positive control that keeps "unchanged" from passing vacuously: a real
/// drop on the same lane DOES take its placeholder away.
#[then(regex = r"^once Priya drops (\w+-\d+) on ([\w-]+) its placeholder is no longer displayed$")]
async fn then_control_drop_hides(world: &mut FoundryWorld, key: String, label: String) {
    let slug = lane_spot_slug(world, &label).await;
    drag_and_drop(world, &key, DragSpot::LaneEnd(&slug)).await;
    assert_placeholder(world, &browser(world), &label, false).await;
}

#[then(regex = r"^the second tab drops (\w+-\d+) without a reload$")]
async fn then_second_tab_drops(world: &mut FoundryWorld, key: String) {
    let second = second_tab(world);
    let mark = js(&second, "return window.__cdfMark || '';", vec![]).await;
    wait_for_card_gone(&second, &key, "the delete in the first tab").await;
    let still = js(&second, "return window.__cdfMark || '';", vec![]).await;
    assert_eq!(mark, still, "the second tab must not have reloaded");
    assert!(
        !still.as_str().unwrap_or_default().is_empty(),
        "the second tab's mark is armed"
    );
}

#[then(regex = r"^([\w-]+) in the second tab shows the placeholder$")]
async fn then_second_tab_placeholder(world: &mut FoundryWorld, label: String) {
    let second = second_tab(world);
    assert_placeholder(world, &second, &label, true).await;
}

#[then(regex = r"^every other lane in the second tab is unchanged$")]
async fn then_second_tab_others(world: &mut FoundryWorld) {
    let second = second_tab(world);
    let before = world
        .cdf_second_looks_before
        .clone()
        .expect("second tab captured");
    let after = lane_looks(&second).await;
    let changed: Vec<&String> = before
        .iter()
        .filter(|b| after.iter().find(|a| a.0 == b.0) != Some(*b))
        .map(|b| &b.0)
        .collect();
    assert_eq!(
        changed.len(),
        1,
        "exactly one lane may change in the second tab; changed: {changed:?}"
    );
}

#[then(regex = r"^([\w-]+) in the second tab shows (\w+-\d+) and no placeholder$")]
async fn then_second_tab_card_no_placeholder(world: &mut FoundryWorld, label: String, key: String) {
    let second = second_tab(world);
    let slug = lane_slug_on_screen(&second, &label).await;
    let deadline = Instant::now() + BROWSER_WAIT;
    let mut look = look_of(&second, &slug).await;
    while look.1 != vec![key.clone()] && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
        look = look_of(&second, &slug).await;
    }
    assert_eq!(
        look.1,
        vec![key.clone()],
        "{label} in the second tab must hold {key} alone"
    );
    assert!(
        !look.2,
        "{label} holds {key} and must not display the placeholder: {look:?}"
    );
}
