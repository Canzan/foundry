//! ui-page-frame — one frame owns where every app page starts, a field is as
//! wide as what it holds, a stacked form is a section, Delete is the quiet
//! control of the issue dialog, and the board header is one row.
//!
//! EVERY GEOMETRY HERE IS READ FROM THE LIVE LAYOUT. Not a class name, not a
//! rule in the stylesheet: the box the browser painted. A page can carry every
//! class this work adds and still start its content somewhere else, and a
//! control can carry the quiet class and still resolve the primary fill.
//!
//! WHERE A PAGE "STARTS". The reading walks down from the one box the frame
//! owns, taking the first drawn child at each step, until it reaches the first
//! element that holds words of its OWN — the page's heading. Its border box,
//! less its own margins, is where the page begins; the frame's padding box is
//! read beside it, so a row of pages that all drifted by the same amount cannot
//! read as agreement. The walk also reports every box on the path, which is what
//! turns "the pages disagree" into "THIS page, THIS box".
//!
//! WHY MARGINS ARE TAKEN OFF THAT BOX AND PADDING IS NOT. A heading's own
//! margin is the page's rhythm and belongs to the page; a padded box between the
//! frame and the heading is a SECOND owner of the page geometry, which is the
//! whole defect. So the reading subtracts the content element's own margins and
//! counts every other box on the path that contributes padding, border or margin.
//!
//! THE PAINTED HEADING IS A DIFFERENT READING. Subtracting the margin makes a
//! heading that sits lower still report the frame's edge, which is how pages
//! whose first line is not at the same height can pass. The heading reading
//! takes the painted top of the first heading and does not subtract that margin.
//!
//! ANTI-VACUITY. Every reading is guarded against passing on nothing: a page
//! with no frame, a form with no field, a dialog that never opened and a control
//! list that came back empty each fail with what was read instead.

use crate::steps::feature_canzan_theme::{blend_over, contrast_ratio, parse_colour};
use crate::support::harness::{establish_session, InProcHarness};
use crate::support::{browser_harness, html_assertions as html};
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use reqwest::redirect::Policy;
use secrecy::SecretString;
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
/// A teammate ON her team who may read the board and may not manage it.
const SAM_EMAIL: &str = "sam@acme.com";
const SAM_PASSWORD: &str = "sam-correct-horse-battery-staple";
/// A workspace member NOT on her team, so the member list has somebody to offer.
const LEE_EMAIL: &str = "lee@acme.com";

const BOARD_PATH: &str = "/team/backend/project/sandbox";
const REPORT_PATH: &str = "/team/backend/project/sandbox/report";
/// The instance dashboard, at the address its own rail destination opens.
const INSTANCE_PATH: &str = "/admin/instance/workspaces";

const PHONE_WIDTH_PX: u32 = 390;
const PHONE_HEIGHT_PX: u32 = 844;
const DESK_HEIGHT_PX: u32 = 900;
const WIDER_WIDTH_PX: u32 = 1600;

/// The frame's own padding, as the design declares it: 24px block and 32px
/// inline at desk width, 16px below the shipped phone breakpoint.
const DESK_FRAME_INLINE_PX: f64 = 32.0;
const DESK_FRAME_BLOCK_PX: f64 = 24.0;
const PHONE_FRAME_PX: f64 = 16.0;

/// The three field caps, in CSS pixels at the shipped root size: about 8rem,
/// 24rem and 36rem.
const SHORT_CAP_PX: f64 = 128.0;
const NAME_CAP_PX: f64 = 384.0;
const LONG_CAP_PX: f64 = 576.0;

/// A control of the Foundry family: a 1px hairline and a 32px minimum height.
const FAMILY_BORDER_PX: f64 = 1.0;
const FAMILY_MIN_HEIGHT_PX: f64 = 32.0;

/// Text reads at 4.5:1, a control's boundary at 3:1 — the shipped obligations.
const TEXT_RATIO: f64 = 4.5;
const BOUNDARY_RATIO: f64 = 3.0;

/// Sub-pixel slack on a box read off a live, fractional layout.
const SLACK_PX: f64 = 1.0;
/// Slack on a cap: a field may be a little under its cap, never over it.
const CAP_SLACK_PX: f64 = 2.0;

/// The stored bounds the authority sets, which a sized field must hold without
/// refusing anything.
const PROJECT_NAME_MAX: usize = 256;
const WORKSPACE_NAME_MAX: usize = 24;
const TEAM_NAME_MAX: usize = 64;

/// The generated-name property's recorded seed and sample size: printed on
/// failure so the case replays exactly. The named extremes are added on top
/// of the sample, and a failure shrinks toward the shortest run that still
/// overflows.
const NAME_SEED: u64 = 20_261_122;
const NAME_CASES: usize = 6;
/// A workspace name of wide glyphs, one scalar each, at the stored bound.
const WIDE_GLYPH: char = '字';

/// Every page of the app the frame wraps, by the name the operator would use and
/// the address she opens.
fn framed_pages() -> Vec<(&'static str, String)> {
    vec![
        ("Home", "/".to_string()),
        ("her board", BOARD_PATH.to_string()),
        ("the change report", REPORT_PATH.to_string()),
        ("Teams", "/teams".to_string()),
        ("workspace invites", "/workspace/invites".to_string()),
        ("machine tokens", "/admin/tokens".to_string()),
        ("account settings", "/account/settings".to_string()),
        ("the instance dashboard", INSTANCE_PATH.to_string()),
    ]
}

/// The page the design names as outside the frame. Its confirm view puts
/// `<main>` directly under `<body>` and pads itself. A bare visit with no
/// signed link is the refusal page, which has no main, so the reading opens
/// the signed confirm link.
const UNFRAMED_PAGE: &str = "/unsubscribe";
const UNFRAMED_PAGE_PADDING_PX: f64 = 20.0;

/// The signed confirm link for /unsubscribe. Without it the route answers the
/// refusal page, which has no main, and the padding reading would measure nothing.
async fn signed_unsubscribe(world: &FoundryWorld) -> String {
    let pool = harness_of(world).app.state.store.pool().clone();
    let (workspace_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("the workspace the unsubscribe page names");
    let token = foundry_auth::UnsubscribeToken::new(
        MEI_EMAIL,
        workspace_id,
        &harness_of(world).app.state.session_secret,
    )
    .expect("mint the unsubscribe link the page requires");
    let t = foundry_app::unsubscribe::encode_t(MEI_EMAIL, workspace_id);
    format!(
        "/unsubscribe?t={}&sig={}",
        urlencoding::encode(&t),
        urlencoding::encode(&token.signature)
    )
}

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(http_client());
    }
}

fn harness_of(world: &FoundryWorld) -> &InProcHarness {
    world
        .harness
        .as_ref()
        .expect("the Background must have spawned the application")
}

fn http_of(world: &FoundryWorld) -> &reqwest::Client {
    world.http.as_ref().expect("an http client")
}

fn browser_of(world: &FoundryWorld) -> &fantoccini::Client {
    world
        .browser
        .as_ref()
        .expect("a browser session must have been opened first")
}

fn remember(world: &mut FoundryWorld, record: serde_json::Value) {
    world.last_body = Some(record.to_string());
}

fn recalled(world: &FoundryWorld) -> serde_json::Value {
    let raw = world
        .last_body
        .clone()
        .expect("the stimulus of this scenario recorded what it read");
    serde_json::from_str(&raw).expect("the record is the one the stimulus wrote")
}

fn num(value: &serde_json::Value, key: &str) -> f64 {
    value[key].as_f64().unwrap_or(f64::NAN)
}

fn count_of(value: &serde_json::Value, key: &str) -> u64 {
    value[key].as_u64().unwrap_or_else(|| {
        value[key]
            .as_i64()
            .and_then(|n| u64::try_from(n).ok())
            .or_else(|| value[key].as_f64().map(|n| n as u64))
            .unwrap_or(0)
    })
}

fn text(value: &serde_json::Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

async fn read_with(browser: &fantoccini::Client, script: &str) -> serde_json::Value {
    browser
        .execute(script, Vec::new())
        .await
        .expect("take the live reading this scenario is about")
}

async fn goto(world: &FoundryWorld, address: &str) {
    let base = harness_of(world).base_url();
    browser_of(world)
        .goto(&format!("{base}{address}"))
        .await
        .unwrap_or_else(|err| panic!("open {address}: {err}"));
}

/// A bounded wait on the address the browser settled at — never a sleep on a
/// guess and never an unbounded loop.
async fn wait_until_on(browser: &fantoccini::Client, path: &str) {
    for _ in 0..60 {
        let here = browser
            .execute("return window.location.pathname;", Vec::new())
            .await
            .map(|v| v.as_str().unwrap_or_default().to_string())
            .unwrap_or_default();
        if here == path {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the browser never arrived at {path}, so whatever is read next is some other page");
}

/// A bounded wait on one live condition in the page, reported with what the page
/// held instead when it never became true.
async fn wait_for_truth(browser: &fantoccini::Client, script: &str, what: &str) {
    for _ in 0..100 {
        let answer = browser
            .execute(script, Vec::new())
            .await
            .unwrap_or(serde_json::Value::Null);
        if answer.as_bool() == Some(true) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("{what} never became true in the live page within ten seconds");
}

// ==========================================================================
// The live readings
// ==========================================================================

/// Where this page starts, and every box on the path from the frame to its own
/// words. See the module note for why margins come off and padding does not.
const FRAME_READING: &str = r#"
function drawn(el) {
  const r = el.getBoundingClientRect();
  const cs = getComputedStyle(el);
  return r.width > 0 && r.height > 0 && cs.display !== 'none' && cs.visibility !== 'hidden';
}
function ownWords(el) {
  return Array.from(el.childNodes).some(n => n.nodeType === 3 && n.textContent.trim().length > 0);
}
function px(v) { const n = parseFloat(v); return isNaN(n) ? 0 : n; }
const out = {
  path: window.location.pathname,
  title: document.title,
  scrollWidth: document.documentElement.scrollWidth,
  clientWidth: document.documentElement.clientWidth,
  innerWidth: window.innerWidth
};
const shell = document.querySelector('.app-shell__content');
if (!shell) {
  out.framed = false;
  const outer = document.querySelector('body > main');
  out.unframedMain = !!outer;
  if (outer) {
    const ocs = getComputedStyle(outer);
    out.ownPadInline = px(ocs.paddingLeft);
    out.ownPadBlock = px(ocs.paddingTop);
  }
  return out;
}
out.framed = true;
const scs = getComputedStyle(shell);
const sr = shell.getBoundingClientRect();
out.framePadInline = px(scs.paddingLeft);
out.framePadBlock = px(scs.paddingTop);
out.frameInlineStart = sr.left + px(scs.paddingLeft);
out.frameBlockStart = sr.top + px(scs.paddingTop);
out.frameContentWidth = sr.width - px(scs.paddingLeft) - px(scs.paddingRight);
const board = document.querySelector('.board');
out.laneRowWidth = board ? board.getBoundingClientRect().width : null;
let el = shell;
const chain = [];
for (let guard = 0; guard < 24; guard++) {
  const kid = Array.from(el.children).find(drawn);
  if (!kid) { break; }
  const kcs = getComputedStyle(kid);
  const kr = kid.getBoundingClientRect();
  chain.push({
    tag: kid.tagName.toLowerCase(),
    cls: (kid.className || '').toString(),
    padInline: px(kcs.paddingLeft),
    padBlock: px(kcs.paddingTop),
    marginInline: px(kcs.marginLeft),
    marginBlock: px(kcs.marginTop),
    borderInline: px(kcs.borderLeftWidth),
    maxWidth: kcs.maxWidth,
    left: kr.left,
    top: kr.top
  });
  el = kid;
  if (ownWords(kid)) { break; }
}
out.chain = chain;
out.reachedWords = el !== shell && ownWords(el);
if (el !== shell) {
  const ecs = getComputedStyle(el);
  const er = el.getBoundingClientRect();
  out.contentTag = el.tagName.toLowerCase();
  out.contentWords = (el.textContent || '').trim().slice(0, 60);
  out.contentInlineStart = er.left - px(ecs.marginLeft);
  out.contentBlockStart = er.top - px(ecs.marginTop);
}
/* Every box on the path except the page's own words: each one that contributes
   padding, a border or a margin is a second owner of the page geometry. */
out.extraBoxes = chain.slice(0, Math.max(chain.length - 1, 0)).filter(b =>
  b.padInline > 0 || b.padBlock > 0 || b.marginInline > 0 || b.marginBlock > 0 ||
  b.borderInline > 0 || (b.maxWidth && b.maxWidth !== 'none')
);
const headings = Array.from(shell.querySelectorAll('h1, h2, h3')).filter(drawn);
out.headingFound = headings.length > 0;
if (headings.length > 0) {
  const hr = headings[0].getBoundingClientRect();
  out.headingTop = hr.top;
  out.headingWords = (headings[0].textContent || '').trim().slice(0, 60);
}
return out;
"#;

/// The lanes of the open board, against the frame she can see without scrolling.
const LANE_READING: &str = r#"
function drawn(el) {
  const r = el.getBoundingClientRect();
  const cs = getComputedStyle(el);
  return r.width > 0 && r.height > 0 && cs.display !== 'none' && cs.visibility !== 'hidden';
}
function px(v) { const n = parseFloat(v); return isNaN(n) ? 0 : n; }
const shell = document.querySelector('.app-shell__content');
const out = { framed: !!shell, innerWidth: window.innerWidth };
if (!shell) { return out; }
const scs = getComputedStyle(shell);
const sr = shell.getBoundingClientRect();
out.frameLeft = sr.left + px(scs.paddingLeft);
out.frameRight = sr.right - px(scs.paddingRight);
const board = document.querySelector('#board-columns');
out.boardFound = !!board;
if (!board) { return out; }
const lanes = Array.from(board.querySelectorAll('[data-column]')).filter(drawn);
out.laneCount = lanes.length;
out.lanes = lanes.map(el => {
  const r = el.getBoundingClientRect();
  const heading = el.querySelector('h3');
  return {
    label: heading ? (heading.textContent || '').trim().slice(0, 40) : '',
    left: r.left,
    right: r.right,
    width: r.width
  };
});
out.boardScrollWidth = board.scrollWidth;
out.boardClientWidth = board.clientWidth;
out.boardOverflowX = getComputedStyle(board).overflowX;
out.docScrollWidth = document.documentElement.scrollWidth;
out.docClientWidth = document.documentElement.clientWidth;
return out;
"#;

/// The project cards on Home, against the frame's own content width.
const CARD_READING: &str = r#"
function drawn(el) {
  const r = el.getBoundingClientRect();
  const cs = getComputedStyle(el);
  return r.width > 0 && r.height > 0 && cs.display !== 'none' && cs.visibility !== 'hidden';
}
function px(v) { const n = parseFloat(v); return isNaN(n) ? 0 : n; }
const shell = document.querySelector('.app-shell__content');
const out = { framed: !!shell, path: window.location.pathname };
if (!shell) { return out; }
const scs = getComputedStyle(shell);
const sr = shell.getBoundingClientRect();
out.frameLeft = sr.left + px(scs.paddingLeft);
out.frameWidth = sr.width - px(scs.paddingLeft) - px(scs.paddingRight);
const cards = Array.from(document.querySelectorAll('.card')).filter(drawn);
out.cardCount = cards.length;
out.cards = cards.map(el => {
  const r = el.getBoundingClientRect();
  return {
    words: (el.textContent || '').trim().slice(0, 40),
    left: r.left,
    width: r.width
  };
});
return out;
"#;

/// Every control the operator can use on this page, with the box and the paint
/// the browser gave it and the surface it sits on.
const CONTROL_READING: &str = r#"
function px(v) { const n = parseFloat(v); return isNaN(n) ? 0 : n; }
function behind(el) {
  let node = el.parentElement;
  while (node) {
    const c = getComputedStyle(node).backgroundColor;
    const m = /rgba?\(([^)]+)\)/.exec(c);
    if (m) {
      const parts = m[1].split(',').map(s => parseFloat(s.trim()));
      const alpha = parts.length > 3 ? parts[3] : 1;
      if (alpha > 0) { return c; }
    }
    node = node.parentElement;
  }
  return getComputedStyle(document.body).backgroundColor;
}
function tokenPaint(prop) {
  const probe = document.createElement('span');
  probe.style.background = 'var(' + prop + ')';
  probe.style.borderTop = '1px solid var(' + prop + ')';
  probe.style.position = 'absolute';
  document.body.appendChild(probe);
  const cs = getComputedStyle(probe);
  const paint = { fill: cs.backgroundColor, edge: cs.borderTopColor };
  probe.remove();
  return paint;
}
const surface = tokenPaint('--cz-surface');
const muted = tokenPaint('--cz-muted');
const black = tokenPaint('--cz-black');
const body = getComputedStyle(document.body);
const shell = document.querySelector('.app-shell__content');
const shellBox = shell ? shell.getBoundingClientRect() : null;
const shellCs = shell ? getComputedStyle(shell) : null;
const dialog = document.querySelector('.modal-dialog');
const dialogBox = dialog ? dialog.getBoundingClientRect() : null;
const dialogCs = dialog ? getComputedStyle(dialog) : null;
const card = document.querySelector('.page-card');
const cardBox = card ? card.getBoundingClientRect() : null;
const cardCs = card ? getComputedStyle(card) : null;
const controls = Array.from(document.querySelectorAll('input, select, textarea')).filter(el => {
  const r = el.getBoundingClientRect();
  const cs = getComputedStyle(el);
  return el.type !== 'hidden' && r.width > 0 && r.height > 0 && cs.display !== 'none';
}).map(el => {
  const cs = getComputedStyle(el);
  const r = el.getBoundingClientRect();
  let buttonFace = null, buttonBorder = 0, buttonFill = null, buttonEdge = null;
  if ((el.type || '') === 'file') {
    const bcs = getComputedStyle(el, '::file-selector-button');
    if (bcs) {
      buttonFace = bcs.fontFamily;
      buttonBorder = px(bcs.borderTopWidth);
      buttonFill = bcs.backgroundColor;
      buttonEdge = bcs.borderTopColor;
    }
  }
  let sibling = null;
  const label = el.closest('label');
  const form = el.closest('form');
  if (form) {
    const area = form.querySelector('textarea');
    if (area) {
      const ar = area.getBoundingClientRect();
      sibling = { tag: 'textarea', width: ar.width };
    }
  }
  return {
    tag: el.tagName.toLowerCase(),
    type: (el.type || '').toLowerCase(),
    name: el.name || '',
    width: r.width,
    height: r.height,
    valueLength: (el.value || '').length,
    borderWidth: px(cs.borderTopWidth),
    borderStyle: cs.borderTopStyle,
    borderColour: cs.borderTopColor,
    fill: cs.backgroundColor,
    ink: cs.color,
    face: cs.fontFamily,
    radius: px(cs.borderTopLeftRadius),
    minHeight: cs.minHeight,
    appearance: cs.appearance,
    behind: behind(el),
    buttonFace: buttonFace,
    buttonBorder: buttonBorder,
    buttonFill: buttonFill,
    buttonEdge: buttonEdge,
    parentFace: getComputedStyle(el.parentElement || document.body).fontFamily,
    labelled: label ? (label.textContent || '').trim().slice(0, 40) : '',
    sibling: sibling,
    inFrame: !!el.closest('.app-shell__content'),
    inDialog: !!el.closest('.modal-dialog'),
    onCard: !!el.closest('.page-card')
  };
});
return {
  path: window.location.pathname,
  bodyFace: body.fontFamily,
  pageBackground: body.backgroundColor,
  frameContentWidth: shellBox ? shellBox.width - px(shellCs.paddingLeft) - px(shellCs.paddingRight) : null,
  dialogContentWidth: dialogBox ? dialogBox.width - px(dialogCs.paddingLeft) - px(dialogCs.paddingRight) : null,
  cardContentWidth: cardBox ? cardBox.width - px(cardCs.paddingLeft) - px(cardCs.paddingRight) - px(cardCs.borderLeftWidth) - px(cardCs.borderRightWidth) : null,
  scrollWidth: document.documentElement.scrollWidth,
  clientWidth: document.documentElement.clientWidth,
  tokenSurface: surface.fill,
  tokenMuted: muted.edge,
  tokenBlack: black.fill,
  controls: controls
};
"#;

/// Every section of this page: its headings, the actions inside it, and whether
/// it pads itself; plus the list presentation of the rename rows and the paint of
/// each grouping box.
const SECTION_READING: &str = r#"
function px(v) { const n = parseFloat(v); return isNaN(n) ? 0 : n; }
const shell = document.querySelector('.app-shell__content');
const forms = Array.from(document.querySelectorAll('form')).filter(f => {
  const r = f.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && !f.closest('.sidebar') && f.getAttribute('action') !== '/sign-out';
}).map(f => {
  const section = f.closest('.page-section') || f.closest('section');
  let headings = 0, headingWords = '';
  if (section) {
    const hs = Array.from(section.querySelectorAll('h1, h2, h3')).filter(h => !h.closest('form'));
    headings = hs.length;
    headingWords = hs.map(h => (h.textContent || '').trim()).join(' | ').slice(0, 80);
  }
  const submit = f.querySelector('button[type="submit"], button:not([type]), input[type="submit"]');
  const padInline = section ? px(getComputedStyle(section).paddingLeft) : null;
  function liveForm(node) {
    const r = node.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && !node.closest('.sidebar')
      && node.getAttribute('action') !== '/sign-out';
  }
  const formsInSection = section
    ? Array.from(section.querySelectorAll('form')).filter(liveForm).length
    : 0;
  return {
    action: f.getAttribute('action') || f.getAttribute('hx-post') || '',
    inSection: !!section,
    sectionTag: section ? section.tagName.toLowerCase() : '',
    sectionClass: section ? (section.className || '').toString() : '',
    headings: headings,
    headingWords: headingWords,
    hasSubmit: !!submit,
    submitInSection: !!(submit && section && section.contains(submit)),
    sectionPadInline: padInline,
    formsInSection: formsInSection
  };
});
const renameLists = Array.from(document.querySelectorAll('ul')).filter(ul =>
  ul.querySelector('[data-project-row], [data-workspace-row]')
).map(ul => {
  const cs = getComputedStyle(ul);
  return {
    listStyle: cs.listStyleType,
    padInlineStart: px(cs.paddingInlineStart),
    marginInlineStart: px(cs.marginInlineStart),
    rows: ul.querySelectorAll('[data-project-row], [data-workspace-row]').length
  };
});
const projectRow = document.querySelector('[data-project-row]');
const workspaceHead = document.querySelector('[data-workspace-head]');
const rowDistinction = (projectRow && workspaceHead) ? {
  projectFill: getComputedStyle(projectRow).backgroundColor,
  workspaceFill: getComputedStyle(workspaceHead).backgroundColor,
  projectBorder: px(getComputedStyle(projectRow).borderTopWidth) + px(getComputedStyle(projectRow).borderBottomWidth),
  workspaceBorder: px(getComputedStyle(workspaceHead).borderTopWidth) + px(getComputedStyle(workspaceHead).borderBottomWidth)
} : null;
const groups = Array.from(document.querySelectorAll('fieldset')).map(fs => {
  const cs = getComputedStyle(fs);
  const legend = fs.querySelector('legend');
  return {
    borderStyle: cs.borderTopStyle,
    borderWidth: px(cs.borderTopWidth),
    borderColour: cs.borderTopColor,
    hasLegend: !!legend,
    legendWords: legend ? (legend.textContent || '').trim().slice(0, 40) : ''
  };
});
return {
  path: window.location.pathname,
  forms: forms,
  renameLists: renameLists,
  rowDistinction: rowDistinction,
  groups: groups
};
"#;

/// The two controls of the issue dialog, and the fields beside them.
const DIALOG_READING: &str = r#"
function px(v) { const n = parseFloat(v); return isNaN(n) ? 0 : n; }
function behind(el) {
  let node = el.parentElement;
  while (node) {
    const c = getComputedStyle(node).backgroundColor;
    const m = /rgba?\(([^)]+)\)/.exec(c);
    if (m) {
      const parts = m[1].split(',').map(s => parseFloat(s.trim()));
      const alpha = parts.length > 3 ? parts[3] : 1;
      if (alpha > 0) { return c; }
    }
    node = node.parentElement;
  }
  return getComputedStyle(document.body).backgroundColor;
}
function tokenPaint(prop) {
  const probe = document.createElement('span');
  probe.style.background = 'var(' + prop + ')';
  probe.style.borderTop = '1px solid var(' + prop + ')';
  probe.style.position = 'absolute';
  document.body.appendChild(probe);
  const cs = getComputedStyle(probe);
  const paint = { fill: cs.backgroundColor, edge: cs.borderTopColor };
  probe.remove();
  return paint;
}
const surface = tokenPaint('--cz-surface');
const muted = tokenPaint('--cz-muted');
const black = tokenPaint('--cz-black');
function paint(el) {
  if (!el) { return null; }
  const cs = getComputedStyle(el);
  const r = el.getBoundingClientRect();
  return {
    words: (el.textContent || '').trim().slice(0, 30),
    fill: cs.backgroundColor,
    ink: cs.color,
    borderColour: cs.borderTopColor,
    borderWidth: px(cs.borderTopWidth),
    radius: px(cs.borderTopLeftRadius),
    width: r.width,
    height: r.height,
    behind: behind(el),
    kind: el.tagName.toLowerCase(),
    buttonType: el.getAttribute('type') || '',
    inForm: !!el.closest('form')
  };
}
const dialog = document.querySelector('.modal-dialog');
if (!dialog) { return { opened: false }; }
const deleteControl = dialog.querySelector('[data-action="delete-issue"]');
const save = dialog.querySelector('form button[type="submit"], form input[type="submit"]');
const status = dialog.querySelector('select[name="state"]');
const title = dialog.querySelector('input[name="title"]');
const description = dialog.querySelector('textarea[name="description"]');
const dcs = getComputedStyle(dialog);
const dr = dialog.getBoundingClientRect();
return {
  opened: true,
  dialogContentWidth: dr.width - px(dcs.paddingLeft) - px(dcs.paddingRight),
  dialogFill: dcs.backgroundColor,
  deleteControl: paint(deleteControl),
  save: paint(save),
  status: status ? paint(status) : null,
  statusAppearance: status ? getComputedStyle(status).appearance : null,
  statusMinHeight: status ? getComputedStyle(status).minHeight : null,
  statusFace: status ? getComputedStyle(status).fontFamily : null,
  titleWidth: title ? title.getBoundingClientRect().width : null,
  descriptionWidth: description ? description.getBoundingClientRect().width : null,
  bodyFace: getComputedStyle(document.body).fontFamily,
  tokenSurface: surface.fill,
  tokenMuted: muted.edge,
  tokenBlack: black.fill,
  modalRootWords: (document.querySelector('#modal-root') || { textContent: '' }).textContent.trim().slice(0, 200)
};
"#;

/// The board and change-report header: the key as a labelled value, and the two
/// board controls as drawn boxes.
const HEADER_READING: &str = r#"
function px(v) { const n = parseFloat(v); return isNaN(n) ? 0 : n; }
function tokenPaint(prop) {
  const probe = document.createElement('span');
  probe.style.background = 'var(' + prop + ')';
  probe.style.borderTop = '1px solid var(' + prop + ')';
  probe.style.position = 'absolute';
  document.body.appendChild(probe);
  const cs = getComputedStyle(probe);
  const paint = { fill: cs.backgroundColor, edge: cs.borderTopColor };
  probe.remove();
  return paint;
}
const surface = tokenPaint('--cz-surface');
const muted = tokenPaint('--cz-muted');
function paint(el) {
  if (!el) { return null; }
  const cs = getComputedStyle(el);
  const r = el.getBoundingClientRect();
  return {
    words: (el.textContent || '').trim().slice(0, 40),
    fill: cs.backgroundColor,
    ink: cs.color,
    borderColour: cs.borderTopColor,
    borderWidth: px(cs.borderTopWidth),
    decoration: cs.textDecorationLine,
    left: r.left, right: r.right, top: r.top, bottom: r.bottom,
    width: r.width, height: r.height,
    href: el.getAttribute('href') || '',
    cls: (el.className || '').toString()
  };
}
const keyed = document.querySelector('[data-key-prefix]');
const report = document.querySelector('a.report-link');
const integrations = document.querySelector('a.integrations-link');
const header = document.querySelector('.app-shell__content header');
return {
  path: window.location.pathname,
  keyElement: keyed ? keyed.tagName.toLowerCase() : null,
  keyAttribute: keyed ? keyed.getAttribute('data-key-prefix') : null,
  keyWords: keyed ? (keyed.textContent || '').trim() : null,
  labelWords: header ? (header.textContent || '').trim().slice(0, 120) : null,
  report: paint(report),
  integrations: paint(integrations),
  tokenSurface: surface.fill,
  tokenMuted: muted.edge
};
"#;

// ==========================================================================
// Background: the seats she holds, her teammates, and the card on her board
// ==========================================================================

#[given(regex = r"^Mei holds every seat her workspace and the instance offer$")]
async fn mei_holds_every_seat(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let pool = harness_of(world).app.state.store.pool().clone();
    let promoted = sqlx::query(
        "UPDATE workspace_memberships SET role = 'admin'
         WHERE user_id = (SELECT id FROM users WHERE email_lower = $1)",
    )
    .bind(MEI_EMAIL)
    .execute(&pool)
    .await
    .expect("make the operator an admin of her workspace");
    assert_eq!(
        promoted.rows_affected(),
        1,
        "WHAT: the operator was not made an admin of her workspace. WHY: the pages this reading \
         opens are the ones a workspace admin reaches, so without the seat the reading would \
         measure refusals instead of pages. HOW: seed the admin membership before the reading."
    );
    let granted = sqlx::query(
        "INSERT INTO instance_admins (user_id)
         SELECT id FROM users WHERE email_lower = $1 ON CONFLICT DO NOTHING",
    )
    .bind(MEI_EMAIL)
    .execute(&pool)
    .await
    .expect("make the operator an admin of the instance");
    assert_eq!(
        granted.rows_affected(),
        1,
        "WHAT: the operator was not made an admin of the instance. WHY: the instance dashboard is \
         one of the pages whose frame this reading compares. HOW: seed the instance seat."
    );
}

#[given(regex = r"^a teammate she can add to a team, and an issue she can open on her board$")]
async fn teammates_and_a_card(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let pool = harness_of(world).app.state.store.pool().clone();
    let workspace: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("fetch workspace");
    let team: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1")
        .bind(workspace.0)
        .fetch_one(&pool)
        .await
        .expect("fetch team");
    for (email, display, on_team) in [(SAM_EMAIL, "Sam", true), (LEE_EMAIL, "Lee", false)] {
        let hash = foundry_auth::hash_password(&SecretString::new(SAM_PASSWORD.to_string().into()))
            .await
            .expect("hash teammate password");
        let user_id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(email)
        .bind(email)
        .bind(display)
        .bind(&hash)
        .execute(&pool)
        .await
        .expect("insert teammate");
        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role)
             VALUES ($1, $2, 'member')",
        )
        .bind(workspace.0)
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("insert teammate workspace membership");
        if on_team {
            sqlx::query(
                "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
            )
            .bind(team.0)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("insert teammate team membership");
        }
    }
    let project: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM projects WHERE workspace_id = $1")
        .bind(workspace.0)
        .fetch_one(&pool)
        .await
        .expect("fetch project");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(MEI_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("fetch author");
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
         VALUES ($1, $2, $3, 1, $4, $5, 'backlog', $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project.0)
    .bind(workspace.0)
    .bind("The signing certificate rotation that the release pipeline waits on")
    .bind("Two lines\n\nof what she wrote.")
    .bind(author.0)
    .execute(&pool)
    .await
    .expect("insert the card she opens");
    // The card is number 1. The store hands the next filing whatever
    // `next_issue_number` still holds, which a direct insert does not move,
    // so the next filing would collide with this card and the server would
    // answer 500 before the board had anything to show.
    sqlx::query("UPDATE projects SET next_issue_number = 2 WHERE id = $1")
        .bind(project.0)
        .execute(&pool)
        .await
        .expect("leave the next filing as the card after the one already on the board");
}

// ==========================================================================
// Opening the application
// ==========================================================================

async fn open_browser_as_mei(world: &mut FoundryWorld, browser: fantoccini::Client) {
    world.browser = Some(browser);
    let harness = world.harness.as_ref().expect("harness");
    let browser = world.browser.as_ref().expect("browser");
    browser_harness::sign_in_through_browser(browser, harness, MEI_EMAIL, MEI_PASSWORD).await;
}

#[given(regex = r"^she is signed in and reading Foundry on (a desk|her phone)$")]
async fn signed_in_on_device(world: &mut FoundryWorld, device: String) {
    ensure_harness(world).await;
    let browser = if device == "her phone" {
        browser_harness::open_mobile_session_sized(PHONE_WIDTH_PX, PHONE_HEIGHT_PX).await
    } else {
        browser_harness::new_session().await
    };
    open_browser_as_mei(world, browser).await;
    remember(world, serde_json::json!({ "device": device }));
}

#[given(
    regex = r"^she is signed in and reading Foundry on a screen that asks for the (light|dark) palette$"
)]
async fn signed_in_in_palette(world: &mut FoundryWorld, palette: String) {
    ensure_harness(world).await;
    let browser = if palette == "dark" {
        let browser = browser_harness::new_dark_session().await;
        assert!(
            browser_harness::device_prefers_dark(&browser).await,
            "WHAT: the session opened for the dark palette does not ask for it. WHY: a reading \
             taken in the wrong palette proves nothing about the other one. HOW: open the session \
             with the dark preference."
        );
        browser
    } else {
        let browser = browser_harness::new_session().await;
        assert!(
            !browser_harness::device_prefers_dark(&browser).await,
            "WHAT: the session opened for the light palette asks for dark. WHY: a reading taken \
             in the wrong palette proves nothing about the light one. HOW: open the session with \
             no dark preference."
        );
        browser
    };
    open_browser_as_mei(world, browser).await;
    remember(world, serde_json::json!({ "palette": palette }));
}

#[given(regex = r"^the operator has her own session of the application$")]
async fn operator_has_a_session(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let cookie =
        establish_session(harness_of(world), http_of(world), MEI_EMAIL, MEI_PASSWORD).await;
    remember(world, serde_json::json!({ "cookie": cookie }));
}

#[given(regex = r"^a teammate who may read her board but may not manage it$")]
async fn a_reader_who_may_not_manage(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let cookie =
        establish_session(harness_of(world), http_of(world), SAM_EMAIL, SAM_PASSWORD).await;
    remember(world, serde_json::json!({ "cookie": cookie }));
}

// ==========================================================================
// L1 — every page starts at the one frame edge
// ==========================================================================

async fn read_every_page(world: &mut FoundryWorld) -> serde_json::Value {
    let mut pages = Vec::new();
    for (name, address) in framed_pages() {
        goto(world, &address).await;
        wait_until_on(browser_of(world), &address).await;
        let reading = read_with(browser_of(world), FRAME_READING).await;
        pages.push(serde_json::json!({ "name": name, "address": address, "reading": reading }));
    }
    let unframed_address = signed_unsubscribe(world).await;
    goto(world, &unframed_address).await;
    wait_until_on(browser_of(world), UNFRAMED_PAGE).await;
    let unframed = read_with(browser_of(world), FRAME_READING).await;
    serde_json::json!({ "pages": pages, "unframed": unframed })
}

#[when(regex = r"^she opens each page of the app in turn$")]
async fn when_she_opens_each_page(world: &mut FoundryWorld) {
    let device = text(&recalled(world), "device");
    let readings = read_every_page(world).await;
    remember(
        world,
        serde_json::json!({ "device": device, "first": readings }),
    );
}

#[given(regex = r"^she has opened each page of the app in turn$")]
async fn given_she_has_opened_each_page(world: &mut FoundryWorld) {
    when_she_opens_each_page(world).await;
}

#[when(regex = r"^she opens her window wider$")]
async fn when_she_widens_her_window(world: &mut FoundryWorld) {
    let record = recalled(world);
    browser_of(world)
        .set_window_size(WIDER_WIDTH_PX, DESK_HEIGHT_PX)
        .await
        .expect("open the window wider");
    let wider = read_every_page(world).await;
    remember(
        world,
        serde_json::json!({
            "device": text(&record, "device"),
            "first": record["first"].clone(),
            "wider": wider
        }),
    );
}

fn page_readings(record: &serde_json::Value, which: &str) -> Vec<(String, serde_json::Value)> {
    record[which]["pages"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|page| (text(&page, "name"), page["reading"].clone()))
        .collect()
}

fn assert_pages_were_read(pages: &[(String, serde_json::Value)]) {
    assert_eq!(
        pages.len(),
        framed_pages().len(),
        "WHAT: {read} of the {expected} pages of the app were read. WHY: a frame law that holds \
         over the pages nobody opened holds over nothing. HOW: open every page of the app before \
         comparing their edges.",
        read = pages.len(),
        expected = framed_pages().len()
    );
    for (name, reading) in pages {
        assert_eq!(
            reading["framed"].as_bool(),
            Some(true),
            "WHAT: {name} answered with no frame at all. WHY: every page of this reading is a page \
             the frame wraps; an answer with no frame is a refusal or a sign-in form, and \
             measuring it would report a frame edge nobody saw. HOW: open the page with the seats \
             the Background grants. Reading: {reading}"
        );
        assert_eq!(
            reading["reachedWords"].as_bool(),
            Some(true),
            "WHAT: {name} holds no words of its own inside the frame. WHY: with nothing drawn \
             there is no place for the page to start, so a green reading would mean nothing. HOW: \
             open the page the operator opens. Reading: {reading}"
        );
    }
}

#[then(regex = r"^every page starts its content at the one edge the frame owns$")]
async fn then_every_page_starts_at_the_frame_edge(world: &mut FoundryWorld) {
    let record = recalled(world);
    let device = text(&record, "device");
    let pages = page_readings(&record, "first");
    assert_pages_were_read(&pages);
    let expected_padding = if device == "her phone" {
        PHONE_FRAME_PX
    } else {
        DESK_FRAME_INLINE_PX
    };
    let mut drifted: Vec<String> = Vec::new();
    let mut starts: Vec<(String, f64)> = Vec::new();
    for (name, reading) in &pages {
        let frame = num(reading, "frameInlineStart");
        let content = num(reading, "contentInlineStart");
        let padding = num(reading, "framePadInline");
        starts.push((name.clone(), content));
        if (content - frame).abs() > SLACK_PX {
            drifted.push(format!(
                "{name} starts at {content:.1} where the frame's own padding box starts at \
                 {frame:.1}; the boxes on its path were {chain}",
                chain = reading["chain"]
            ));
        }
        if (padding - expected_padding).abs() > SLACK_PX {
            drifted.push(format!(
                "the frame pads {name} by {padding:.1} where the design declares \
                 {expected_padding:.1} on {device}"
            ));
        }
    }
    let first = starts.first().map(|(_, at)| *at).unwrap_or(f64::NAN);
    for (name, at) in &starts {
        if (at - first).abs() > SLACK_PX {
            drifted.push(format!(
                "{name} starts at {at:.1} while the first page read starts at {first:.1}"
            ));
        }
    }
    assert!(
        drifted.is_empty(),
        "WHAT: {n} page edges do not agree with the one the frame owns on {device}: {drifted:?}. \
         WHY: the operator moves between these pages and the content jumps sideways, which is the \
         defect she reported. HOW: let the frame's own padding box be the only box between the \
         window and a page's first words.",
        n = drifted.len()
    );
}

#[then(regex = r"^every page starts its content at the one height the frame owns$")]
async fn then_every_page_starts_at_the_frame_height(world: &mut FoundryWorld) {
    let record = recalled(world);
    let device = text(&record, "device");
    let pages = page_readings(&record, "first");
    assert_pages_were_read(&pages);
    let expected_padding = if device == "her phone" {
        PHONE_FRAME_PX
    } else {
        DESK_FRAME_BLOCK_PX
    };
    let mut drifted: Vec<String> = Vec::new();
    for (name, reading) in &pages {
        let frame = num(reading, "frameBlockStart");
        let content = num(reading, "contentBlockStart");
        let padding = num(reading, "framePadBlock");
        if (content - frame).abs() > SLACK_PX {
            drifted.push(format!(
                "{name} starts {gap:.1} below the frame's own padding box; the boxes on its path \
                 were {chain}",
                gap = content - frame,
                chain = reading["chain"]
            ));
        }
        if (padding - expected_padding).abs() > SLACK_PX {
            drifted.push(format!(
                "the frame pads {name} by {padding:.1} above where the design declares \
                 {expected_padding:.1} on {device}"
            ));
        }
    }
    assert!(
        drifted.is_empty(),
        "WHAT: {n} pages do not start at the height the frame owns on {device}: {drifted:?}. WHY: \
         the promise is the same top spacing beside the nav on every page, not only the same left \
         edge. HOW: let the frame own the block padding too.",
        n = drifted.len()
    );
}

#[then(regex = r"^exactly one padded box stands between the frame and a page's own words$")]
async fn then_exactly_one_padded_box(world: &mut FoundryWorld) {
    let record = recalled(world);
    let pages = page_readings(&record, "first");
    assert_pages_were_read(&pages);
    let mut offenders: Vec<String> = Vec::new();
    for (name, reading) in &pages {
        let extra = reading["extraBoxes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !extra.is_empty() {
            offenders.push(format!("{name} adds {extra:?}"));
        }
    }
    assert!(
        offenders.is_empty(),
        "WHAT: {n} pages put another box with padding, a border, a margin or a width of its own \
         between the frame and their first words: {offenders:?}. WHY: two owners of one fact drift \
         apart, which is how the pages came to disagree in the first place. HOW: leave the padding \
         to the frame; a page supplies content and never its own page box.",
        n = offenders.len()
    );
}

#[then(regex = r"^nothing on any of those pages runs off the side of her screen$")]
async fn then_no_page_runs_off_the_side(world: &mut FoundryWorld) {
    let record = recalled(world);
    let pages = page_readings(&record, "first");
    assert_pages_were_read(&pages);
    let mut wide: Vec<String> = Vec::new();
    for (name, reading) in &pages {
        let scroll = num(reading, "scrollWidth");
        let client = num(reading, "clientWidth");
        if scroll > client + SLACK_PX {
            wide.push(format!(
                "{name} is {scroll:.0} wide inside a {client:.0} window"
            ));
        }
    }
    assert!(
        wide.is_empty(),
        "WHAT: {wide:?}. WHY: a page wider than the window moves every other element of it \
         sideways and leaves her scrolling to read a heading. HOW: cap a field with a maximum \
         inline size, never a width."
    );
}

#[then(regex = r"^the page that is not framed keeps the page padding of its own$")]
async fn then_the_unframed_page_keeps_its_padding(world: &mut FoundryWorld) {
    let record = recalled(world);
    let unframed = record["first"]["unframed"].clone();
    assert_eq!(
        unframed["framed"].as_bool(),
        Some(false),
        "WHAT: the page read as the one outside the frame answered WITH the frame. WHY: this \
         reading exists to prove the page padding was scoped and not taken away, and a framed \
         answer proves nothing about that. HOW: read a page the frame does not wrap. Reading: \
         {unframed}"
    );
    assert_eq!(
        unframed["unframedMain"].as_bool(),
        Some(true),
        "WHAT: the page outside the frame holds no page box at all. WHY: with nothing to measure \
         the reading would pass on nothing. Reading: {unframed}"
    );
    let inline = num(&unframed, "ownPadInline");
    let block = num(&unframed, "ownPadBlock");
    assert!(
        (inline - UNFRAMED_PAGE_PADDING_PX).abs() <= SLACK_PX
            && (block - UNFRAMED_PAGE_PADDING_PX).abs() <= SLACK_PX,
        "WHAT: the page outside the frame pads itself by {inline:.1} and {block:.1} where it \
         shipped with {UNFRAMED_PAGE_PADDING_PX:.0}. WHY: a page with no frame around it pads \
         itself; taking that padding away from everything to give the framed pages one edge leaves \
         this one pressed against the side of the screen. HOW: scope the page rule to the pages \
         outside the frame instead of deleting it."
    );
}

#[then(regex = r"^the first heading on every page sits at the height the frame owns$")]
async fn then_the_first_heading_sits_at_the_frame_height(world: &mut FoundryWorld) {
    let record = recalled(world);
    let device = text(&record, "device");
    let pages = page_readings(&record, "first");
    assert_pages_were_read(&pages);
    let mut drifted: Vec<String> = Vec::new();
    for (name, reading) in &pages {
        if reading["headingFound"].as_bool() != Some(true) {
            drifted.push(format!(
                "{name} has no heading inside the frame, so there is no first line to measure"
            ));
            continue;
        }
        let top = num(reading, "headingTop");
        let frame = num(reading, "frameBlockStart");
        if !top.is_finite() || (top - frame).abs() > SLACK_PX {
            drifted.push(format!(
                "{name} paints its first heading, {words:?}, at {top:.1} where the frame's own \
                 top edge is {frame:.1}",
                words = text(reading, "headingWords")
            ));
        }
    }
    assert!(
        drifted.is_empty(),
        "WHAT: {n} pages do not sit their first heading on the frame's top edge on {device}: \
         {drifted:?}. WHY: the gap above the first line is the frame's own spacing, and a heading \
         that keeps a margin of its own sits lower on some pages than on others. HOW: let the \
         first heading sit on the frame's padding box on every page, and do not judge that gap \
         after subtracting the heading's margin.",
        n = drifted.len()
    );
}

#[when(regex = r"^she opens her board$")]
async fn when_she_opens_her_board(world: &mut FoundryWorld) {
    let device = text(&recalled(world), "device");
    goto(world, BOARD_PATH).await;
    wait_until_on(browser_of(world), BOARD_PATH).await;
    let lanes = read_with(browser_of(world), LANE_READING).await;
    remember(
        world,
        serde_json::json!({ "device": device, "lanes": lanes }),
    );
}

fn lanes_of(world: &FoundryWorld) -> serde_json::Value {
    let record = recalled(world);
    let lanes = record["lanes"].clone();
    assert_eq!(
        lanes["framed"].as_bool(),
        Some(true),
        "WHAT: the board answered with no frame. WHY: a lane is on the screen only against the \
         frame she can see. Reading: {lanes}"
    );
    assert_eq!(
        lanes["boardFound"].as_bool(),
        Some(true),
        "WHAT: the board page holds no board. WHY: with no lanes there is nothing that can run \
         off the screen, and the reading would pass on nothing. Reading: {lanes}"
    );
    lanes
}

#[then(regex = r"^every lane of her board is fully on the screen$")]
async fn then_every_lane_is_fully_on_the_screen(world: &mut FoundryWorld) {
    let record = recalled(world);
    let device = text(&record, "device");
    let reading = lanes_of(world);
    let lanes = reading["lanes"].as_array().cloned().unwrap_or_default();
    assert!(
        lanes.len() >= 2,
        "WHAT: the board showed {n} lanes on {device}. WHY: a single lane fits a phone, so a \
         reading of one lane cannot catch lanes that run off the side. HOW: open the board of \
         the project this session holds, which has more than one lane. Reading: {reading}",
        n = lanes.len()
    );
    let frame_left = num(&reading, "frameLeft");
    let frame_right = num(&reading, "frameRight");
    let window = num(&reading, "innerWidth");
    let mut off: Vec<String> = Vec::new();
    for lane in &lanes {
        let left = num(lane, "left");
        let right = num(lane, "right");
        let label = text(lane, "label");
        if left < frame_left - SLACK_PX
            || right > frame_right + SLACK_PX
            || left < -SLACK_PX
            || right > window + SLACK_PX
        {
            off.push(format!(
                "{label:?} runs from {left:.0} to {right:.0} inside a frame of {frame_left:.0} \
                 to {frame_right:.0} on a {window:.0} screen"
            ));
        }
    }
    assert!(
        off.is_empty(),
        "WHAT: {n} lanes run off the screen on {device}: {off:?}. WHY: a lane she cannot see \
         without scrolling sideways is a page that does not keep the frame. HOW: let every lane \
         sit inside the frame she can see.",
        n = off.len()
    );
}

#[then(regex = r"^she does not have to scroll the board sideways$")]
async fn then_she_does_not_scroll_the_board_sideways(world: &mut FoundryWorld) {
    let record = recalled(world);
    let device = text(&record, "device");
    let reading = lanes_of(world);
    let scroll = num(&reading, "boardScrollWidth");
    let visible = num(&reading, "boardClientWidth");
    assert!(
        scroll.is_finite() && visible.is_finite() && scroll <= visible + SLACK_PX,
        "WHAT: the board is {scroll:.0} wide inside a strip of {visible:.0} on {device}. WHY: a \
         strip wider than the screen asks her to scroll sideways to read a lane, even when the \
         page itself does not. HOW: let the lanes share the frame instead of running past it."
    );
}

#[then(regex = r"^the board page does not run off the side of her screen$")]
async fn then_the_board_page_does_not_run_off_the_side(world: &mut FoundryWorld) {
    let record = recalled(world);
    let device = text(&record, "device");
    let reading = lanes_of(world);
    let scroll = num(&reading, "docScrollWidth");
    let client = num(&reading, "docClientWidth");
    assert!(
        scroll.is_finite() && client.is_finite() && scroll <= client + SLACK_PX,
        "WHAT: the board page is {scroll:.0} wide inside a {client:.0} window on {device}. WHY: \
         on a phone the board strip scrolls its lanes inside itself while the page document stays \
         within the screen width. HOW: keep horizontal overflow inside the board strip instead of \
         pushing the document wider than the window."
    );
}

#[then(regex = r"^the board lanes scroll inside the board strip$")]
async fn then_the_board_lanes_scroll_inside_the_board_strip(world: &mut FoundryWorld) {
    let record = recalled(world);
    let device = text(&record, "device");
    let reading = lanes_of(world);
    let lanes = reading["lanes"].as_array().cloned().unwrap_or_default();
    assert!(
        lanes.len() >= 2,
        "WHAT: the board showed {n} lanes on {device}. WHY: a single lane fits a phone without \
         scrolling, so a reading of one lane cannot verify the board strip's own scroll. HOW: \
         open the board of the project this session holds, which has more than one lane. Reading: \
         {reading}",
        n = lanes.len()
    );
    let scroll = num(&reading, "boardScrollWidth");
    let visible = num(&reading, "boardClientWidth");
    let overflow_x = text(&reading, "boardOverflowX");
    assert!(
        scroll.is_finite() && visible.is_finite() && scroll > visible + SLACK_PX,
        "WHAT: the board strip is {scroll:.0} wide inside a {visible:.0} container on {device}. \
         WHY: on a phone the lanes keep their minimum width so they scroll inside the board strip \
         instead of collapsing to fit the narrow screen. HOW: keep the mobile column minimum width \
         so the lane strip scrolls inside the board."
    );
    assert!(
        overflow_x == "auto" || overflow_x == "scroll",
        "WHAT: the board strip has overflow-x {overflow_x:?} on {device}. WHY: the board strip \
         must carry its own horizontal scroll on a phone so the lanes stay readable without \
         widening the page. HOW: keep overflow-x: auto on the board strip below the phone \
         breakpoint."
    );
}

#[when(regex = r"^she opens Home$")]
async fn when_she_opens_home(world: &mut FoundryWorld) {
    let device = text(&recalled(world), "device");
    goto(world, "/").await;
    wait_until_on(browser_of(world), "/").await;
    let home = read_with(browser_of(world), CARD_READING).await;
    remember(world, serde_json::json!({ "device": device, "home": home }));
}

#[then(regex = r"^each project card spans the width of the frame$")]
async fn then_each_project_card_spans_the_frame(world: &mut FoundryWorld) {
    let record = recalled(world);
    let device = text(&record, "device");
    let home = record["home"].clone();
    assert_eq!(
        home["framed"].as_bool(),
        Some(true),
        "WHAT: Home answered with no frame. WHY: a card spans the frame only when the frame is \
         there to measure. Reading: {home}"
    );
    assert_eq!(
        home["path"].as_str(),
        Some("/"),
        "WHAT: the card reading was taken on {path} and not on Home. WHY: the cards that must \
         span the frame are the ones on Home. Reading: {home}",
        path = text(&home, "path")
    );
    let cards = home["cards"].as_array().cloned().unwrap_or_default();
    assert!(
        cards.len() >= 2,
        "WHAT: Home showed {n} project cards on {device}. WHY: one card fills the only track \
         that holds it, so it spans the frame whether empty tracks stay or collapse, while two \
         cards still sit in narrow tracks. HOW: open Home with the two projects this session \
         holds. Reading: {home}",
        n = cards.len()
    );
    let frame_width = num(&home, "frameWidth");
    let frame_left = num(&home, "frameLeft");
    let mut narrow: Vec<String> = Vec::new();
    for card in &cards {
        let width = num(card, "width");
        let left = num(card, "left");
        if (width - frame_width).abs() > SLACK_PX || (left - frame_left).abs() > SLACK_PX {
            narrow.push(format!(
                "{words:?} is {width:.0} wide and starts at {left:.0}, inside a frame {frame_width:.0} \
                 wide that starts at {frame_left:.0}",
                words = text(card, "words")
            ));
        }
    }
    assert!(
        narrow.is_empty(),
        "WHAT: {n} project cards do not span the frame on {device}: {narrow:?}. WHY: a card in a \
         narrow track of a wider grid is the centred column in another shape. HOW: let each \
         project card be as wide as the frame and start at the frame's own left edge.",
        n = narrow.len()
    );
}

#[then(regex = r"^every page still starts its content where it did before$")]
async fn then_every_page_starts_where_it_did(world: &mut FoundryWorld) {
    let record = recalled(world);
    let before = page_readings(&record, "first");
    let after = page_readings(&record, "wider");
    assert_pages_were_read(&before);
    assert_pages_were_read(&after);
    let mut moved: Vec<String> = Vec::new();
    for ((name, first), (_, wider)) in before.iter().zip(after.iter()) {
        let was = num(first, "contentInlineStart");
        let now = num(wider, "contentInlineStart");
        let window_was = num(first, "innerWidth");
        let window_now = num(wider, "innerWidth");
        if (was - now).abs() > SLACK_PX {
            moved.push(format!(
                "{name} started at {was:.1} in a {window_was:.0} window and at {now:.1} in a \
                 {window_now:.0} one"
            ));
        }
    }
    assert!(
        moved.is_empty(),
        "WHAT: {moved:?}. WHY: a start that is a function of the window is a start that moves \
         while she reads, which is exactly the centred column being removed. HOW: let the frame's \
         own padding own the start, with no centred column and no maximum content width."
    );
}

#[then(regex = r"^her board still uses the width of the window$")]
async fn then_her_board_still_uses_the_window(world: &mut FoundryWorld) {
    let record = recalled(world);
    let before = page_readings(&record, "first");
    let after = page_readings(&record, "wider");
    let first = before
        .iter()
        .find(|(name, _)| name == "her board")
        .map(|(_, reading)| reading);
    let wider = after
        .iter()
        .find(|(name, _)| name == "her board")
        .map(|(_, reading)| reading);
    let (Some(first), Some(wider)) = (first, wider) else {
        panic!(
            "WHAT: the board was not among the pages read before and after the window grew. WHY: \
             a left edge that stays put can still belong to a frame that stops growing."
        );
    };
    let window_grew = num(wider, "innerWidth") - num(first, "innerWidth");
    let frame_grew = num(wider, "frameContentWidth") - num(first, "frameContentWidth");
    let lanes_grew = num(wider, "laneRowWidth") - num(first, "laneRowWidth");
    assert!(
        window_grew > 100.0,
        "WHAT: the window grew by {window_grew:.0}. WHY: this reading compares a desk window with \
         a wider one, so a window that did not grow proves nothing."
    );
    assert!(
        frame_grew >= window_grew * 0.5,
        "WHAT: the board's frame content grew by {frame_grew:.0} while the window grew by \
         {window_grew:.0}. WHY: a frame with a maximum width keeps its left edge and still stops \
         the board using the window. HOW: give the frame no maximum content width."
    );
    assert!(
        lanes_grew >= window_grew * 0.5,
        "WHAT: the board's lane row grew by {lanes_grew:.0} while the window grew by \
         {window_grew:.0}. WHY: the lanes are what she reads across, and a row that stays the \
         width of a centred column leaves empty space she cannot use. HOW: let the lane row use \
         the frame."
    );
    let lane = num(wider, "laneRowWidth");
    let frame = num(wider, "frameContentWidth");
    assert!(
        lane.is_finite() && (lane - frame).abs() <= 16.0,
        "WHAT: the lane row is {lane:.0} wide inside a frame of {frame:.0}. WHY: a lane row \
         narrower than the frame is a second width inside the one the frame owns."
    );
}

#[then(regex = r"^every page still starts at the one edge the frame owns$")]
async fn then_every_page_still_starts_at_the_frame_edge(world: &mut FoundryWorld) {
    let record = recalled(world);
    let pages = page_readings(&record, "wider");
    assert_pages_were_read(&pages);
    let mut drifted: Vec<String> = Vec::new();
    for (name, reading) in &pages {
        let frame = num(reading, "frameInlineStart");
        let content = num(reading, "contentInlineStart");
        if (content - frame).abs() > SLACK_PX {
            drifted.push(format!(
                "{name} at {content:.1} against the frame's {frame:.1}"
            ));
        }
    }
    assert!(
        drifted.is_empty(),
        "WHAT: in the wider window {drifted:?}. WHY: the one edge must be the frame's at every \
         window width, not only at the width the first reading used. HOW: pad the frame and \
         nothing else."
    );
}

// ==========================================================================
// Y1 — the pages the rail offers are the pages this reading opens
// ==========================================================================

#[when(regex = r"^the pages the rail offers her are listed beside the pages this reading opens$")]
async fn when_rail_places_are_listed(world: &mut FoundryWorld) {
    let cookie = text(&recalled(world), "cookie");
    let base = harness_of(world).base_url();
    let body = http_of(world)
        .get(format!("{base}/"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("open her landing page")
        .text()
        .await
        .expect("read her landing page");
    let doc = html::parse(&body);
    let offered: Vec<String> = html::select_all(&doc, ".sidebar a.sidebar__item")
        .into_iter()
        .filter_map(|el| el.value().attr("href").map(|href| href.to_string()))
        .collect();
    remember(world, serde_json::json!({ "offered": offered }));
}

/// Rail destinations this reading never opens.
///
/// Never asked, and only these: the shortcut list and the integrations pages.
/// Her board is the exact route this reading already opens,
/// `/team/backend/project/sandbox`, so that address matches and needs no
/// exemption. A different destination under `/team/` is not that board.
fn rail_places_this_reading_never_opens<'a>(
    offered: &'a [String],
    read: &[String],
) -> Vec<&'a String> {
    const OUT_OF_SCOPE: &[&str] = &[
        "/keyboard-help",
        "/admin/instance/integrations",
        "/workspace/integrations",
    ];
    offered
        .iter()
        .filter(|href| href.starts_with('/'))
        .filter(|href| !OUT_OF_SCOPE.contains(&href.as_str()))
        .filter(|href| !read.iter().any(|address| address == *href))
        .collect()
}

#[then(regex = r"^the reading opens every page the rail offers her$")]
async fn then_the_reading_opens_every_rail_place(world: &mut FoundryWorld) {
    let record = recalled(world);
    let offered: Vec<String> = record["offered"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        offered.len() >= 5,
        "WHAT: the rail offered {n} destinations. WHY: with an empty rail this guard would pass \
         over nothing. HOW: read the rail of a page she is signed in to. Offered: {offered:?}",
        n = offered.len()
    );
    let read: Vec<String> = framed_pages()
        .into_iter()
        .map(|(_, address)| address)
        .collect();
    let missing = rail_places_this_reading_never_opens(&offered, &read);
    assert!(
        missing.is_empty(),
        "WHAT: the rail offers {missing:?}, which the all-pages frame reading never opens. WHY: \
         the frame stays the only padded box only for the pages somebody reads; a page added to \
         the rail and not to this reading keeps its own edge in silence. HOW: add the page to the \
         reading, or state it out of scope here with its reason."
    );
}

#[cfg(test)]
mod rail_place_guard {
    use super::{rail_places_this_reading_never_opens, BOARD_PATH, REPORT_PATH};

    fn places(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| (*path).to_string()).collect()
    }

    #[test]
    fn her_board_matches_exactly_and_another_team_place_does_not() {
        let offered = places(&[
            "/",
            BOARD_PATH,
            REPORT_PATH,
            "/teams",
            "/keyboard-help",
            "/workspace/integrations",
            "/admin/instance/integrations",
            "/team/backend/members",
        ]);
        let read = places(&["/", BOARD_PATH, REPORT_PATH, "/teams"]);
        let missing = rail_places_this_reading_never_opens(&offered, &read);
        let missing: Vec<&str> = missing.iter().map(|href| href.as_str()).collect();
        assert_eq!(
            missing,
            vec!["/team/backend/members"],
            "WHAT: the unread destinations were {missing:?}. WHY: her board and the change \
             report are exact places this reading opens, and only the shortcut list and the \
             integrations pages are never asked. Another place under /team/ must still be \
             reported. HOW: match her board by its exact route and do not drop a /team/ prefix."
        );
    }
}

// ==========================================================================
// L2 — a field is as wide as what it holds
// ==========================================================================

/// The stored names this property runs over: the three the design names, and the
/// generated rest from the recorded seed.
fn generated_names(seed: u64) -> Vec<(String, String, String)> {
    let mut cases = Vec::new();
    // Named extremes, always included: an unbroken project name, a workspace
    // name of wide glyphs, a one-letter name, and a team name at its bound.
    cases.push((
        "W".repeat(PROJECT_NAME_MAX),
        WIDE_GLYPH.to_string().repeat(WORKSPACE_NAME_MAX),
        "A".to_string(),
    ));
    cases.push((
        "N".to_string(),
        "Acme".to_string(),
        "T".repeat(TEAM_NAME_MAX),
    ));
    let mut rng = StdRng::seed_from_u64(seed);
    for _ in 0..NAME_CASES {
        let project_len = rng.gen_range(1..=PROJECT_NAME_MAX);
        let workspace_len = rng.gen_range(1..=WORKSPACE_NAME_MAX);
        let team_len = rng.gen_range(1..=TEAM_NAME_MAX);
        cases.push((
            woven_name(&mut rng, project_len),
            woven_name(&mut rng, workspace_len),
            woven_name(&mut rng, team_len),
        ));
    }
    cases
}

/// A name of the given length: a mixture of spaced words and unbroken runs,
/// never opening or closing with a space, so every generated case is a name the
/// store already accepts.
fn woven_name(rng: &mut StdRng, length: usize) -> String {
    const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut out = String::with_capacity(length);
    let mut last_was_space = true;
    for at in 0..length {
        let inner = at > 0 && at + 1 < length;
        if inner && !last_was_space && rng.gen_bool(0.12) {
            out.push(' ');
            last_was_space = true;
        } else {
            let pick = rng.gen_range(0..LETTERS.len());
            out.push(char::from(LETTERS[pick]));
            last_was_space = false;
        }
    }
    out
}

#[given(regex = r"^she has stored names of every shape, including these$")]
async fn she_has_stored_names_of_every_shape(world: &mut FoundryWorld, step: &Step) {
    ensure_harness(world).await;
    let named = step
        .table
        .as_ref()
        .map(|table| {
            table
                .rows
                .iter()
                .filter(|row| row.first().map(String::as_str) != Some("what she stored"))
                .count()
        })
        .unwrap_or_default();
    assert!(
        named >= 5,
        "WHAT: the scenario names {named} stored shapes. WHY: the unbroken project name, the wide \
         workspace name, the one-letter name, the team name at its bound and the long email must \
         each be named, or the generated run may never reach them."
    );
    remember(world, serde_json::json!({ "seed": NAME_SEED }));
}

/// Store one generated case, then read every field that shows it.
async fn store_names(world: &mut FoundryWorld, case: &(String, String, String)) {
    let pool = harness_of(world).app.state.store.pool().clone();
    sqlx::query("UPDATE projects SET name = $1")
        .bind(&case.0)
        .execute(&pool)
        .await
        .expect("store the project name of this case");
    sqlx::query("UPDATE workspaces SET name = $1")
        .bind(&case.1)
        .execute(&pool)
        .await
        .expect("store the workspace name of this case");
    sqlx::query("UPDATE teams SET name = $1")
        .bind(&case.2)
        .execute(&pool)
        .await
        .expect("store the team name of this case");
}

#[when(regex = r"^each page that shows those names is opened, and her issue dialog beside them$")]
async fn when_each_page_showing_those_names_is_opened(world: &mut FoundryWorld) {
    let cases = generated_names(NAME_SEED);
    let mut readings = Vec::new();
    for (at, case) in cases.iter().enumerate() {
        store_names(world, case).await;
        for address in [INSTANCE_PATH, "/teams"] {
            goto(world, address).await;
            wait_until_on(browser_of(world), address).await;
            if address == "/teams" {
                let shown = serde_json::to_string(&case.2).expect("a team name is text");
                browser_of(world)
                    .execute(
                        &format!(
                            "const field = document.querySelector('input[name=\"name\"]'); \
                             if (field) field.value = {shown};"
                        ),
                        Vec::new(),
                    )
                    .await
                    .expect("type this case's team name into the team field");
            }
            let reading = read_with(browser_of(world), CONTROL_READING).await;
            readings.push(serde_json::json!({
                "case": at,
                "project": case.0.clone(),
                "workspace": case.1.clone(),
                "team": case.2.clone(),
                "reading": reading
            }));
        }
    }
    // The two pages whose fields carry the short and the long size, read once:
    // they show no stored name, so a case adds nothing to them.
    for address in ["/workspace/invites", "/admin/tokens"] {
        goto(world, address).await;
        wait_until_on(browser_of(world), address).await;
        if address == "/workspace/invites" {
            let long_email = "a".repeat(254);
            browser_of(world)
                .execute(
                    &format!(
                        "const field = document.querySelector('input[type=\"email\"], input[name=\"email\"]'); \
                         if (field) field.value = '{long_email}';"
                    ),
                    Vec::new(),
                )
                .await
                .expect("type the long email into the invite field");
        }
        let reading = read_with(browser_of(world), CONTROL_READING).await;
        readings.push(serde_json::json!({
            "case": "the sized fields of the forms",
            "project": "",
            "workspace": "",
            "team": "",
            "reading": reading
        }));
    }
    // Both dialogs, whose fields are deliberately NOT capped: each title field
    // must stay level with the description under it, even holding a long value.
    let mut dialogs = Vec::new();
    open_the_edit_dialog(world).await;
    hold_a_long_title(world).await;
    dialogs.push(read_with(browser_of(world), DIALOG_READING).await);
    goto(world, BOARD_PATH).await;
    browser_harness::wait_for_board_ready(browser_of(world)).await;
    open_the_new_issue_dialog(world).await;
    hold_a_long_title(world).await;
    dialogs.push(read_with(browser_of(world), DIALOG_READING).await);
    // The sign-in card, outside the frame: its field keeps the card's width.
    browser_of(world)
        .delete_all_cookies()
        .await
        .expect("give up her session so the card page can be read");
    goto(world, "/sign-in").await;
    wait_until_on(browser_of(world), "/sign-in").await;
    let card = read_with(browser_of(world), CONTROL_READING).await;
    remember(
        world,
        serde_json::json!({
            "seed": NAME_SEED,
            "pages": readings,
            "dialogs": dialogs,
            "card": card
        }),
    );
}

/// The cap that applies to a field by the name the form gives it — the public
/// contract of the form, not a class this work adds.
fn cap_for(name: &str) -> f64 {
    match name {
        "ttl_days" => SHORT_CAP_PX,
        "email" => LONG_CAP_PX,
        "destination" => LONG_CAP_PX,
        _ => NAME_CAP_PX,
    }
}

#[then(regex = r"^every field inside the frame stays within the cap that applies where it sits$")]
async fn then_every_framed_field_stays_within_its_cap(world: &mut FoundryWorld) {
    let record = recalled(world);
    let pages = record["pages"].as_array().cloned().unwrap_or_default();
    assert!(
        !pages.is_empty(),
        "WHAT: no page of stored names was read. WHY: a cap law over no field holds over nothing."
    );
    let mut over: Vec<String> = Vec::new();
    let mut fields_read = 0usize;
    let mut longest_text = 0usize;
    let mut saw_wide_workspace = false;
    let mut saw_team_bound = false;
    let mut saw_long_email = false;
    for page in &pages {
        let reading = &page["reading"];
        let frame_width = num(reading, "frameContentWidth");
        for control in reading["controls"].as_array().cloned().unwrap_or_default() {
            let in_frame = control["inFrame"].as_bool() == Some(true);
            let tag = text(&control, "tag");
            let kind = text(&control, "type");
            let is_field = tag == "input" && matches!(kind.as_str(), "text" | "email" | "number");
            if !in_frame || !is_field {
                continue;
            }
            fields_read += 1;
            let name = text(&control, "name");
            let held = control["valueLength"].as_u64().unwrap_or(0) as usize;
            if kind == "email" && held >= 254 {
                saw_long_email = true;
            }
            if kind == "text" {
                longest_text = longest_text.max(held);
                if held == WORKSPACE_NAME_MAX {
                    saw_wide_workspace = true;
                }
                if held == TEAM_NAME_MAX {
                    saw_team_bound = true;
                }
            }
            let width = num(&control, "width");
            let cap = cap_for(&name);
            if width > cap + CAP_SLACK_PX {
                over.push(format!(
                    "the field {name:?} on {path} is {width:.0} wide where its own size caps it at \
                     {cap:.0} (case {case}, stored project {stored:?})",
                    path = text(reading, "path"),
                    case = page["case"],
                    stored = page["project"].as_str().unwrap_or_default().len()
                ));
            }
            if frame_width.is_finite() && width >= frame_width - SLACK_PX {
                over.push(format!(
                    "the field {name:?} on {path} is the whole width of the frame ({width:.0})",
                    path = text(reading, "path")
                ));
            }
        }
    }
    assert!(
        fields_read >= 4,
        "WHAT: {fields_read} fields were read inside the frame. WHY: the pages this scenario opens \
         each carry a form, so a reading with almost no field in it read a refusal instead of a \
         page."
    );
    assert!(
        longest_text >= PROJECT_NAME_MAX && saw_wide_workspace && saw_team_bound && saw_long_email,
        "WHAT: the fields that were read held a longest text of {longest_text}, a workspace name \
         of {WORKSPACE_NAME_MAX} ({saw_wide_workspace}), a team name of {TEAM_NAME_MAX} \
         ({saw_team_bound}) and a 254-letter email ({saw_long_email}). WHY: a cap that was never \
         shown a value at its bound can pass while the bound still overflows. HOW: store each \
         named extreme and type the long email before reading."
    );
    assert!(
        over.is_empty(),
        "WHAT: {n} fields are wider than what they hold allows (seed {seed}): {over:?}. WHY: a \
         field stretched to the whole frame is the 1100px box the operator reported, and it tells \
         her nothing about what belongs in it. HOW: cap each field by its own size class.",
        n = over.len(),
        seed = NAME_SEED
    );
}

#[then(regex = r"^no page she opened runs off the side of her screen$")]
async fn then_no_page_of_names_runs_off(world: &mut FoundryWorld) {
    let record = recalled(world);
    let mut wide: Vec<String> = Vec::new();
    for page in record["pages"].as_array().cloned().unwrap_or_default() {
        let reading = &page["reading"];
        let scroll = num(reading, "scrollWidth");
        let client = num(reading, "clientWidth");
        if scroll > client + SLACK_PX {
            wide.push(format!(
                "{path} is {scroll:.0} wide in a {client:.0} window while it shows a stored name \
                 of {len} letters (case {case})",
                path = text(reading, "path"),
                len = page["project"].as_str().unwrap_or_default().len(),
                case = page["case"]
            ));
        }
    }
    let shrunk = if wide.is_empty() {
        String::new()
    } else {
        shortest_overflowing_run(world).await
    };
    assert!(
        wide.is_empty(),
        "WHAT: {wide:?} (seed {NAME_SEED}). {shrunk} WHY: one unbroken stored name that pushes the \
         page wider moves every other element of it sideways. HOW: cap the field with a maximum \
         inline size so a long value scrolls inside its own box."
    );
}

/// On an overflow, find the shortest unbroken project name that still runs off
/// the page and print it with the seed. This runs only after a failure.
async fn shortest_overflowing_run(world: &mut FoundryWorld) -> String {
    let mut lo = 1usize;
    let mut hi = PROJECT_NAME_MAX;
    let mut found: Option<usize> = None;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let name = "W".repeat(mid);
        store_names(world, &(name, "Acme".to_string(), "Backend".to_string())).await;
        goto(world, INSTANCE_PATH).await;
        wait_until_on(browser_of(world), INSTANCE_PATH).await;
        let reading = read_with(browser_of(world), CONTROL_READING).await;
        let scroll = num(&reading, "scrollWidth");
        let client = num(&reading, "clientWidth");
        if scroll > client + SLACK_PX {
            found = Some(mid);
            if mid == 0 {
                break;
            }
            hi = mid - 1;
        } else {
            lo = mid + 1;
        }
    }
    match found {
        Some(length) => format!(
            "the shortest unbroken project name that still runs off the page is {length} letters \
             (seed {NAME_SEED})."
        ),
        None => format!("no shortened project name reproduced the overflow (seed {NAME_SEED})."),
    }
}

/// Put a long value in the open dialog's title so a field that grows with its
/// value is caught beside the description, which wraps.
async fn hold_a_long_title(world: &FoundryWorld) {
    let long = "W".repeat(PROJECT_NAME_MAX);
    browser_of(world)
        .execute(
            &format!(
                "const field = document.querySelector('.modal-dialog input[name=\"title\"]'); \
                 if (field) field.value = '{long}';"
            ),
            Vec::new(),
        )
        .await
        .expect("type a long title into the open dialog");
}

#[then(regex = r"^in a dialog the title field is exactly as wide as the description under it$")]
async fn then_the_dialog_title_matches_its_description(world: &mut FoundryWorld) {
    let record = recalled(world);
    let dialogs = record["dialogs"].as_array().cloned().unwrap_or_default();
    assert!(
        dialogs.len() >= 2,
        "WHAT: {n} dialogs were read. WHY: the cap must stay out of both the edit dialog and the \
         new-issue dialog. Reading: {record}",
        n = dialogs.len()
    );
    for dialog in &dialogs {
        assert_eq!(
            dialog["opened"].as_bool(),
            Some(true),
            "WHAT: a dialog was not open when this reading was taken. WHY: the dialog is where \
             the cap must NOT reach, so a reading with no dialog proves nothing. Reading: {dialog}"
        );
        let title = num(dialog, "titleWidth");
        let description = num(dialog, "descriptionWidth");
        let dialog_width = num(dialog, "dialogContentWidth");
        assert!(
            title > 0.0 && description > 0.0,
            "WHAT: a dialog's title field is {title:.0} wide and its description {description:.0}. \
             WHY: with either undrawn the comparison is empty. Reading: {dialog}"
        );
        assert!(
            (title - description).abs() <= SLACK_PX,
            "WHAT: a dialog's title field is {title:.0} wide beside a description of \
             {description:.0}. WHY: a cap meant for the framed forms reaching into a dialog leaves \
             the title stopping short of the box under it. HOW: scope the unclassed cap to the \
             frame."
        );
        assert!(
            (title - dialog_width).abs() <= SLACK_PX + 1.0,
            "WHAT: a dialog's title field is {title:.0} wide inside a dialog of {dialog_width:.0}. \
             WHY: in a dialog the field's box IS the dialog's content width; anything else is a \
             cap that reached where it was scoped out of."
        );
    }
}

#[then(regex = r"^on the sign-in card her password field keeps the width of the card$")]
async fn then_the_card_field_keeps_the_card_width(world: &mut FoundryWorld) {
    let record = recalled(world);
    let card = record["card"].clone();
    let card_width = num(&card, "cardContentWidth");
    let password = card["controls"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .find(|control| text(control, "type") == "password");
    let password = password.unwrap_or_else(|| {
        panic!(
            "WHAT: the card page read carries no password field. WHY: this reading is what proves \
             the cap never reached the signed-out cards, so without the field it proves nothing. \
             Reading: {card}"
        )
    });
    let width = num(&password, "width");
    assert!(
        card_width.is_finite() && card_width > 0.0,
        "WHAT: the card page reports no card box. WHY: the field is compared against the card's \
         own content width, so with no card there is nothing to compare. Reading: {card}"
    );
    assert!(
        (width - card_width).abs() <= SLACK_PX,
        "WHAT: her password field is {width:.0} wide inside a card of {card_width:.0}. WHY: the \
         signed-out cards are out of scope and must not change; a cap applied everywhere narrows \
         every one of them. HOW: scope the unclassed cap to the frame."
    );
}

// ==========================================================================
// L3 — the list, number and file controls are Foundry's own
// ==========================================================================

#[when(regex = r"^she opens every form of the app and both of its dialogs$")]
async fn when_she_opens_every_form_and_dialog(world: &mut FoundryWorld) {
    let palette = text(&recalled(world), "palette");
    let mut readings = Vec::new();
    for address in [
        "/teams",
        "/team/backend/members",
        "/admin/tokens",
        "/workspace/invites",
        "/team/backend/project/sandbox/issues/1",
    ] {
        goto(world, address).await;
        wait_until_on(browser_of(world), address).await;
        let reading = read_with(browser_of(world), CONTROL_READING).await;
        readings.push(serde_json::json!({ "address": address, "reading": reading }));
    }
    open_the_edit_dialog(world).await;
    let edit = read_with(browser_of(world), CONTROL_READING).await;
    readings.push(serde_json::json!({ "address": "the edit dialog", "reading": edit }));
    let edit_dialog = read_with(browser_of(world), DIALOG_READING).await;
    goto(world, BOARD_PATH).await;
    browser_harness::wait_for_board_ready(browser_of(world)).await;
    open_the_new_issue_dialog(world).await;
    let new_issue = read_with(browser_of(world), CONTROL_READING).await;
    readings.push(serde_json::json!({ "address": "the new-issue dialog", "reading": new_issue }));
    open_the_lane_delete_dialog(world).await;
    let lane_delete = read_with(browser_of(world), CONTROL_READING).await;
    readings.push(serde_json::json!({
        "address": "the lane-delete dialog",
        "reading": lane_delete
    }));
    browser_of(world)
        .delete_all_cookies()
        .await
        .expect("give up her session so the sign-in card can be read");
    goto(world, "/sign-in").await;
    wait_until_on(browser_of(world), "/sign-in").await;
    let card = read_with(browser_of(world), CONTROL_READING).await;
    readings.push(serde_json::json!({ "address": "the sign-in card", "reading": card }));
    remember(
        world,
        serde_json::json!({ "palette": palette, "pages": readings, "dialog": edit_dialog }),
    );
}

fn family_controls(record: &serde_json::Value) -> Vec<(String, serde_json::Value)> {
    let mut out = Vec::new();
    for page in record["pages"].as_array().cloned().unwrap_or_default() {
        let address = text(&page, "address");
        for control in page["reading"]["controls"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let tag = text(&control, "tag");
            let kind = text(&control, "type");
            let of_the_family = tag == "select"
                || matches!(
                    kind.as_str(),
                    "number" | "file" | "text" | "email" | "password"
                );
            if of_the_family {
                let mut control = control;
                if let Some(obj) = control.as_object_mut() {
                    obj.insert(
                        "tokenSurface".to_string(),
                        page["reading"]["tokenSurface"].clone(),
                    );
                    obj.insert(
                        "tokenMuted".to_string(),
                        page["reading"]["tokenMuted"].clone(),
                    );
                }
                out.push((address.clone(), control));
            }
        }
    }
    out
}

fn colour_over(raw: &str, behind: &str) -> Option<(u8, u8, u8)> {
    let (ink, alpha) = parse_colour(raw)?;
    let (under, _) = parse_colour(behind)?;
    Some(blend_over(ink, under, alpha))
}

/// Two computed colours name the same paint when their red, green and blue agree.
fn paints_agree(left: &str, right: &str) -> bool {
    match (parse_colour(left), parse_colour(right)) {
        (Some((a, _)), Some((b, _))) => a == b,
        _ => false,
    }
}

#[then(regex = r"^every list, number and file control is drawn as one of Foundry's own$")]
async fn then_every_control_is_of_the_family(world: &mut FoundryWorld) {
    let record = recalled(world);
    let palette = text(&record, "palette");
    let controls = family_controls(&record);
    let kinds: Vec<String> = controls
        .iter()
        .map(|(_, control)| {
            let tag = text(control, "tag");
            let kind = text(control, "type");
            if tag == "select" {
                "select".to_string()
            } else {
                kind
            }
        })
        .collect();
    for wanted in ["select", "number", "file", "text", "password"] {
        assert!(
            kinds.iter().any(|kind| kind == wanted),
            "WHAT: no {wanted} control was read at all, in {kinds:?}. WHY: this law is about each \
             kind of control the operator meets; a kind nobody read is a kind nobody checked. HOW: \
             open a page that carries one."
        );
    }
    let dialog_lists = controls
        .iter()
        .filter(|(address, control)| text(control, "tag") == "select" && address.contains("dialog"))
        .count();
    assert!(
        dialog_lists >= 2,
        "WHAT: {dialog_lists} lists were read inside a dialog. WHY: the edit dialog's status list \
         and the lane-delete dialog's destination list are both part of the family, and a list \
         left on the platform's box is the defect."
    );
    let mut platform: Vec<String> = Vec::new();
    for (address, control) in &controls {
        let tag = text(control, "tag");
        let kind = text(control, "type");
        let name = text(control, "name");
        let border = num(control, "borderWidth");
        let style = text(control, "borderStyle");
        let face = text(control, "face");
        let radius = num(control, "radius");
        let min_height = text(control, "minHeight");
        if kind == "file" {
            let button_face = text(control, "buttonFace");
            let button_border = num(control, "buttonBorder");
            if !button_face.is_empty()
                && button_face != text(&record, "bodyFace")
                && button_face != face
            {
                platform.push(format!(
                    "the file control on {address} draws its button in {button_face} and its box \
                     in {face}"
                ));
            }
            if button_border < FAMILY_BORDER_PX - 0.5 {
                platform.push(format!(
                    "the file control's own button on {address} draws a {button_border:.1}px edge"
                ));
            }
            if !paints_agree(&text(control, "buttonFill"), &text(control, "tokenSurface")) {
                platform.push(format!(
                    "the file control's button on {address} fills with {} where the surface token \
                     is {}",
                    text(control, "buttonFill"),
                    text(control, "tokenSurface")
                ));
            }
            if !paints_agree(&text(control, "buttonEdge"), &text(control, "tokenMuted")) {
                platform.push(format!(
                    "the file control's button on {address} edges with {} where the muted token \
                     is {}",
                    text(control, "buttonEdge"),
                    text(control, "tokenMuted")
                ));
            }
            continue;
        }
        if !paints_agree(&text(control, "fill"), &text(control, "tokenSurface")) {
            platform.push(format!(
                "{tag}/{kind} {name:?} on {address} fills with {} where the surface token is {}",
                text(control, "fill"),
                text(control, "tokenSurface")
            ));
        }
        if !paints_agree(&text(control, "borderColour"), &text(control, "tokenMuted")) {
            platform.push(format!(
                "{tag}/{kind} {name:?} on {address} edges with {} where the muted token is {}",
                text(control, "borderColour"),
                text(control, "tokenMuted")
            ));
        }
        if text(control, "face") != text(control, "parentFace") {
            platform.push(format!(
                "{tag}/{kind} {name:?} on {address} draws {} where the text around it draws {}",
                text(control, "face"),
                text(control, "parentFace")
            ));
        }
        if border < FAMILY_BORDER_PX - 0.5 || style != "solid" {
            platform.push(format!(
                "{tag}/{kind} {name:?} on {address} draws a {border:.1}px {style} edge"
            ));
        }
        if radius <= 0.0 {
            platform.push(format!(
                "{tag}/{kind} {name:?} on {address} draws square corners where the family is \
                 rounded"
            ));
        }
        if min_height != format!("{FAMILY_MIN_HEIGHT_PX:.0}px") {
            platform.push(format!(
                "{tag}/{kind} {name:?} on {address} asks for a minimum height of {min_height} \
                 where the family asks for {FAMILY_MIN_HEIGHT_PX:.0}px"
            ));
        }
    }
    assert!(
        platform.is_empty(),
        "WHAT: {n} controls are drawn by the platform and not by Foundry in the {palette} \
         palette: {platform:?}. WHY: a tiny unstyled list beside a field Foundry drew is the \
         defect the operator reported. HOW: let every one of these kinds join the one field rule \
         group.",
        n = platform.len()
    );
}

#[then(regex = r"^every one of those controls has an edge she can see against the page$")]
async fn then_every_control_edge_is_visible(world: &mut FoundryWorld) {
    let record = recalled(world);
    let palette = text(&record, "palette");
    let controls = family_controls(&record);
    assert!(
        !controls.is_empty(),
        "WHAT: no control was read. WHY: a contrast law over no control holds over nothing."
    );
    let mut faint: Vec<String> = Vec::new();
    for (address, control) in &controls {
        if text(control, "type") == "file" {
            continue;
        }
        let behind = text(control, "behind");
        let edge = text(control, "borderColour");
        let Some(edge) = colour_over(&edge, &behind) else {
            faint.push(format!(
                "the edge of {name:?} on {address} resolves {edge:?}, which is no colour at all",
                name = text(control, "name")
            ));
            continue;
        };
        let Some(surface) = colour_over(&behind, &behind) else {
            continue;
        };
        let ratio = contrast_ratio(edge, surface);
        if ratio < BOUNDARY_RATIO {
            faint.push(format!(
                "{tag}/{kind} {name:?} on {address} draws an edge at {ratio:.2} to 1",
                tag = text(control, "tag"),
                kind = text(control, "type"),
                name = text(control, "name")
            ));
        }
    }
    assert!(
        faint.is_empty(),
        "WHAT: {n} control edges fall under {BOUNDARY_RATIO} to 1 in the {palette} palette: \
         {faint:?}. WHY: a control she cannot find the edge of is a control she cannot tell from \
         the page. HOW: keep the shipped hairline token on every control of the family.",
        n = faint.len()
    );
}

#[then(regex = r"^the words in every one of those controls read clearly$")]
async fn then_the_words_in_every_control_read_clearly(world: &mut FoundryWorld) {
    let record = recalled(world);
    let palette = text(&record, "palette");
    let controls = family_controls(&record);
    let mut faint: Vec<String> = Vec::new();
    for (address, control) in &controls {
        let fill = text(control, "fill");
        let behind = text(control, "behind");
        let ink = text(control, "ink");
        let Some(surface) = colour_over(&fill, &behind) else {
            continue;
        };
        let Some(words) = colour_over(&ink, &fill) else {
            continue;
        };
        let ratio = contrast_ratio(words, surface);
        if ratio < TEXT_RATIO {
            faint.push(format!(
                "{name:?} on {address} reads at {ratio:.2} to 1",
                name = text(control, "name")
            ));
        }
    }
    assert!(
        faint.is_empty(),
        "WHAT: {n} controls hold words under {TEXT_RATIO} to 1 in the {palette} palette: \
         {faint:?}. WHY: a value she cannot read is a value she cannot check before she submits \
         it. HOW: pair the family's fill with the shipped text token.",
        n = faint.len()
    );
}

#[then(regex = r"^a list still shows the arrow her own browser draws$")]
async fn then_a_list_still_shows_its_arrow(world: &mut FoundryWorld) {
    let record = recalled(world);
    let controls = family_controls(&record);
    let lists: Vec<&(String, serde_json::Value)> = controls
        .iter()
        .filter(|(_, control)| text(control, "tag") == "select")
        .collect();
    assert!(
        !lists.is_empty(),
        "WHAT: no list was read. WHY: the arrow law is about the lists, so with none read it holds \
         over nothing."
    );
    let mut stripped: Vec<String> = Vec::new();
    for (address, control) in lists {
        let appearance = text(control, "appearance");
        if appearance == "none" {
            stripped.push(format!(
                "the list {name:?} on {address} asks the browser to draw nothing",
                name = text(control, "name")
            ));
        }
    }
    assert!(
        stripped.is_empty(),
        "WHAT: {stripped:?}. WHY: taking the platform arrow away obliges this stylesheet to draw \
         one, and a drawn arrow is either an asset or a colour named below the token seam. HOW: \
         keep the arrow the platform draws and take the box Foundry draws."
    );
}

// ==========================================================================
// L4 — a stacked form is a section with a heading and its own action
// ==========================================================================

#[when(regex = r"^she opens Teams, the instance dashboard and her account settings$")]
async fn when_she_opens_the_stacked_form_pages(world: &mut FoundryWorld) {
    let mut readings = Vec::new();
    for (name, address) in [
        ("Teams", "/teams"),
        ("the instance dashboard", INSTANCE_PATH),
        ("her account settings", "/account/settings"),
    ] {
        goto(world, address).await;
        wait_until_on(browser_of(world), address).await;
        let sections = read_with(browser_of(world), SECTION_READING).await;
        let frame = read_with(browser_of(world), FRAME_READING).await;
        readings.push(serde_json::json!({
            "name": name,
            "address": address,
            "sections": sections,
            "frame": frame
        }));
    }
    remember(world, serde_json::json!({ "pages": readings }));
}

fn stacked_pages(world: &FoundryWorld) -> Vec<serde_json::Value> {
    let record = recalled(world);
    let pages = record["pages"].as_array().cloned().unwrap_or_default();
    assert_eq!(
        pages.len(),
        3,
        "WHAT: {n} of the three pages of stacked forms were read. WHY: the promise names three \
         pages, so a reading of fewer holds over fewer.",
        n = pages.len()
    );
    pages
}

#[then(regex = r"^every form sits in a section holding exactly one heading and its own action$")]
async fn then_every_form_sits_in_a_section(world: &mut FoundryWorld) {
    let pages = stacked_pages(world);
    let mut orphans: Vec<String> = Vec::new();
    let mut forms_read = 0usize;
    for page in &pages {
        let name = text(page, "name");
        let forms = page["sections"]["forms"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        forms_read += forms.len();
        for form in forms {
            let action = text(&form, "action");
            if form["inSection"].as_bool() != Some(true) {
                orphans.push(format!("the form {action:?} on {name} sits in no section"));
                continue;
            }
            let headings = form["headings"].as_u64().unwrap_or_default();
            if headings != 1 {
                orphans.push(format!(
                    "the section of {action:?} on {name} holds {headings} headings ({words})",
                    words = text(&form, "headingWords")
                ));
            }
            if form["hasSubmit"].as_bool() == Some(true)
                && form["submitInSection"].as_bool() != Some(true)
            {
                orphans.push(format!(
                    "the action of {action:?} on {name} sits outside the section holding its fields"
                ));
            }
        }
    }
    assert!(
        forms_read >= 4,
        "WHAT: {forms_read} forms were read over the three pages. WHY: these pages carry the \
         stacked forms the promise is about; almost none read means a refusal was read instead."
    );
    assert!(
        orphans.is_empty(),
        "WHAT: {n} forms do not read as a section: {orphans:?}. WHY: a Save button at the foot of \
         a page under no heading belongs to nothing she can see. HOW: give each form a section \
         with one heading and keep its action inside it.",
        n = orphans.len()
    );
}

#[then(regex = r"^each form sits under a heading that no other form shares$")]
async fn then_each_form_has_a_heading_of_its_own(world: &mut FoundryWorld) {
    let pages = stacked_pages(world);
    let mut shared: Vec<String> = Vec::new();
    let mut saw_project_rename = false;
    for page in &pages {
        let name = text(page, "name");
        let forms = page["sections"]["forms"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for form in forms {
            let action = text(&form, "action");
            if action.contains("/projects/") && action.contains("rename") {
                saw_project_rename = true;
            }
            let in_section = form["inSection"].as_bool() == Some(true);
            let headings = count_of(&form, "headings");
            let together = count_of(&form, "formsInSection");
            if !in_section || headings != 1 || together != 1 {
                shared.push(format!(
                    "the form {action:?} on {name} shares its place with {together} forms under \
                     {headings} headings ({words})",
                    words = text(&form, "headingWords")
                ));
            }
        }
    }
    assert!(
        saw_project_rename,
        "WHAT: no project rename form was read. WHY: the forms that sit together under one \
         heading are the rename forms on the instance dashboard, so a reading that never opened \
         one proves nothing."
    );
    assert!(
        shared.is_empty(),
        "WHAT: {n} forms do not have a heading of their own: {shared:?}. WHY: a form that shares \
         its heading with another form does not show her where one form ends and the next begins. \
         HOW: give each form a section that holds that form alone and exactly one heading.",
        n = shared.len()
    );
}

#[then(regex = r"^the rename rows show no bullet and no indent of the browser's own$")]
async fn then_the_rename_rows_show_no_bullets(world: &mut FoundryWorld) {
    let pages = stacked_pages(world);
    let mut lists = Vec::new();
    for page in &pages {
        for list in page["sections"]["renameLists"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            lists.push((text(page, "name"), list));
        }
    }
    assert!(
        !lists.is_empty(),
        "WHAT: no list of rename rows was read at all. WHY: the bullets and the indent are the \
         defect, so a reading that found no such list proves nothing. HOW: open the instance \
         dashboard with its workspaces and projects seeded."
    );
    let mut bulleted: Vec<String> = Vec::new();
    for (name, list) in &lists {
        let style = text(list, "listStyle");
        let pad = num(list, "padInlineStart");
        let margin = num(list, "marginInlineStart");
        if style != "none" {
            bulleted.push(format!("a list on {name} draws {style} markers"));
        }
        if pad > SLACK_PX || margin > SLACK_PX {
            bulleted.push(format!(
                "a list on {name} is indented by the browser ({pad:.0} padding, {margin:.0} \
                 margin)"
            ));
        }
    }
    assert!(
        bulleted.is_empty(),
        "WHAT: {bulleted:?}. WHY: nested bullet lists of forms read as an outline and not as the \
         rows of a page. HOW: carry the hierarchy with the heading and a hairline instead of the \
         browser's bullets and indent."
    );
}

#[then(regex = r"^a project row can still be told from its workspace$")]
async fn then_a_project_row_can_be_told_from_its_workspace(world: &mut FoundryWorld) {
    let pages = stacked_pages(world);
    let dashboard = pages
        .iter()
        .find(|page| text(page, "name") == "the instance dashboard");
    let Some(dashboard) = dashboard else {
        panic!(
            "WHAT: the instance dashboard was not among the pages read. WHY: the nested project \
             rows live there."
        );
    };
    let distinction = dashboard["sections"]["rowDistinction"].clone();
    assert!(
        !distinction.is_null(),
        "WHAT: the dashboard showed no project row beside a workspace row. WHY: a flat list is \
         what this reading refuses, so with one of the two missing it proves nothing. Reading: \
         {dashboard}"
    );
    let fills_differ = text(&distinction, "projectFill") != text(&distinction, "workspaceFill");
    let border = num(&distinction, "projectBorder") + num(&distinction, "workspaceBorder");
    assert!(
        fills_differ || border > 0.5,
        "WHAT: the project row and the workspace row share the fill {} and draw {border:.1}px of \
         edge between them. WHY: taking the bullets away and leaving a flat list still hides which \
         projects belong to which workspace. HOW: keep a hairline or a different ground under the \
         nested rows. Reading: {distinction}",
        text(&distinction, "projectFill")
    );
}

#[then(regex = r"^the grouped choices are drawn as Foundry's own groups$")]
async fn then_the_groups_are_foundrys_own(world: &mut FoundryWorld) {
    let pages = stacked_pages(world);
    let mut groups = Vec::new();
    for page in &pages {
        for group in page["sections"]["groups"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            groups.push((text(page, "name"), group));
        }
    }
    assert!(
        !groups.is_empty(),
        "WHAT: no grouped choices were read. WHY: the raw grouping box is the defect, so with no \
         group read the law holds over nothing. HOW: open account settings with its project email \
         choices."
    );
    let mut raw: Vec<String> = Vec::new();
    for (name, group) in &groups {
        let style = text(group, "borderStyle");
        let width = num(group, "borderWidth");
        if matches!(style.as_str(), "groove" | "ridge" | "inset" | "outset")
            || width > 1.0 + SLACK_PX
        {
            raw.push(format!(
                "a group on {name} draws a {width:.0}px {style} box, which is the browser's own"
            ));
        }
        if group["hasLegend"].as_bool() != Some(true) {
            raw.push(format!("a group on {name} carries no heading of its own"));
        }
    }
    assert!(
        raw.is_empty(),
        "WHAT: {raw:?}. WHY: a ridged grey box around a set of choices is the one surface on the \
         page that nothing else of Foundry looks like. HOW: keep the grouping element and draw it \
         as a Foundry group with its legend as the sub-heading."
    );
}

#[then(regex = r"^none of those sections moved the page's content edge$")]
async fn then_no_section_moved_the_content_edge(world: &mut FoundryWorld) {
    let pages = stacked_pages(world);
    let mut moved: Vec<String> = Vec::new();
    for page in &pages {
        let frame = &page["frame"];
        let name = text(page, "name");
        assert_eq!(
            frame["framed"].as_bool(),
            Some(true),
            "WHAT: {name} answered with no frame. WHY: the content edge is read against the \
             frame's own padding box. Reading: {frame}"
        );
        let at = num(frame, "contentInlineStart");
        let edge = num(frame, "frameInlineStart");
        if (at - edge).abs() > SLACK_PX {
            moved.push(format!(
                "{name} now starts at {at:.1} against the frame's {edge:.1}; the boxes on its path \
                 were {chain}",
                chain = frame["chain"]
            ));
        }
    }
    assert!(
        moved.is_empty(),
        "WHAT: {moved:?}. WHY: a section that pads itself undoes the one frame edge one page at a \
         time. HOW: let a section carry rhythm between its own rows and never inline padding on \
         the page's first box."
    );
}

// ==========================================================================
// The swap contract the sectioned rename rows must survive
// ==========================================================================

#[given(regex = r"^she has opened the instance dashboard$")]
async fn given_she_has_opened_the_instance_dashboard(world: &mut FoundryWorld) {
    goto(world, INSTANCE_PATH).await;
    wait_until_on(browser_of(world), INSTANCE_PATH).await;
    browser_harness::wait_for_kb_ready(browser_of(world)).await;
}

#[when(
    regex = r"^she renames a project and a workspace, once with a name each takes and once with one each refuses$"
)]
async fn when_she_renames_a_project_and_a_workspace(world: &mut FoundryWorld) {
    let accepted_project = "Sandbox renamed";
    let accepted_workspace = "Acme renamed";
    let refused_project = "x".repeat(PROJECT_NAME_MAX + 40);
    let refused_workspace = "x".repeat(WORKSPACE_NAME_MAX + 40);
    rename_through(world, "[data-project-row]", accepted_project).await;
    wait_for_truth(
        browser_of(world),
        "return ((document.querySelector('[data-project-row] [data-project-name]') || \
            {textContent: ''}).textContent || '').trim() === 'Sandbox renamed';",
        "the project row carrying the name she gave it",
    )
    .await;
    rename_through(world, "[data-workspace-head]", accepted_workspace).await;
    wait_for_truth(
        browser_of(world),
        "return ((document.querySelector('[data-workspace-head]') || {textContent: ''})\
            .textContent || '').indexOf('Acme renamed') >= 0;",
        "the workspace row carrying the name she gave it",
    )
    .await;
    rename_through(world, "[data-project-row]", &refused_project).await;
    wait_for_truth(
        browser_of(world),
        "return ((document.querySelector('[data-project-row] [data-error-slot]') || \
            {textContent: ''}).textContent || '').trim().length > 0;",
        "the reason the project rename was refused, inside that row",
    )
    .await;
    rename_through(world, "[data-workspace-head]", &refused_workspace).await;
    wait_for_truth(
        browser_of(world),
        "return ((document.querySelector('[data-workspace-head] [data-error-slot]') || \
            {textContent: ''}).textContent || '').trim().length > 0;",
        "the reason the workspace rename was refused, inside that row",
    )
    .await;
    let reading = read_with(
        browser_of(world),
        r#"
function row(selector, nameSelector) {
  const el = document.querySelector(selector);
  if (!el) { return null; }
  const slot = el.querySelector('[data-error-slot]');
  const form = el.querySelector('form');
  const named = nameSelector ? el.querySelector(nameSelector) : null;
  return {
    tag: el.tagName.toLowerCase(),
    name: named ? (named.textContent || '').trim() : '',
    words: (el.textContent || '').trim().slice(0, 120),
    hasErrorSlot: !!slot,
    errorWords: slot ? (slot.textContent || '').trim().slice(0, 120) : '',
    markers: el.hasAttribute('data-project-row')
      ? el.hasAttribute('data-project-id')
      : el.hasAttribute('data-workspace-head'),
    swap: form ? (form.getAttribute('hx-swap') || '') : ''
  };
}
return {
  projectRow: row('[data-project-row]', '[data-project-name]'),
  workspaceHead: row('[data-workspace-head]', null)
};
"#,
    )
    .await;
    remember(
        world,
        serde_json::json!({ "renames": reading, "accepted": accepted_project }),
    );
}

/// Type a name into one rename row and submit that row's own form, the way she
/// does: the field of THAT row, the button of THAT row.
async fn rename_through(world: &FoundryWorld, row: &str, name: &str) {
    let browser = browser_of(world);
    let field = browser
        .find(fantoccini::Locator::Css(&format!(
            "{row} form input[name='name']"
        )))
        .await
        .unwrap_or_else(|err| panic!("the row {row} offers no rename field: {err}"));
    field
        .clear()
        .await
        .unwrap_or_else(|err| panic!("the rename field of {row} could not be emptied: {err}"));
    field
        .send_keys(name)
        .await
        .unwrap_or_else(|err| panic!("the name could not be typed into {row}: {err}"));
    browser
        .find(fantoccini::Locator::Css(&format!(
            "{row} form button[type='submit']"
        )))
        .await
        .unwrap_or_else(|err| panic!("the row {row} offers no Rename control: {err}"))
        .click()
        .await
        .unwrap_or_else(|err| panic!("the Rename control of {row} could not be pressed: {err}"));
}

#[then(regex = r"^each row she renamed shows its new name in place$")]
async fn then_each_renamed_row_shows_its_name(world: &mut FoundryWorld) {
    let record = recalled(world);
    let renames = record["renames"].clone();
    let row = renames["projectRow"].clone();
    assert!(
        !row.is_null(),
        "WHAT: after the renames the project row is gone from the page. WHY: the row is swapped by \
         its own outer element, so a row that disappeared means the swap no longer finds it. \
         Reading: {renames}"
    );
    assert_eq!(
        text(&row, "tag"),
        "li",
        "WHAT: the project row came back as a {tag}. WHY: the rename handler swaps this element by \
         its outer element, so its kind is part of the contract and not a presentation choice. \
         Reading: {renames}",
        tag = text(&row, "tag")
    );
    assert_eq!(
        row["markers"].as_bool(),
        Some(true),
        "WHAT: the project row came back without its markers. WHY: the swap and the error routing \
         both resolve the row by them. Reading: {renames}"
    );
    assert_eq!(
        text(&row, "swap"),
        "outerHTML",
        "WHAT: the project row's form swaps by {swap:?}. WHY: the accepted name replaces the row \
         itself, and a swap of an inner piece leaves the old row around it. Reading: {renames}",
        swap = text(&row, "swap")
    );
    assert_eq!(
        text(&row, "name"),
        "Sandbox renamed",
        "WHAT: the project row reads {name:?} after a name she was allowed to give it. WHY: the \
         row she is looking at is the one the rename replaced. Reading: {renames}",
        name = text(&row, "name")
    );
    let head = renames["workspaceHead"].clone();
    assert_eq!(
        text(&head, "tag"),
        "div",
        "WHAT: the workspace row came back as a {tag}. WHY: that row is the outer element the \
         rename swaps, and its kind is part of the contract. Reading: {renames}",
        tag = text(&head, "tag")
    );
    assert_eq!(
        text(&head, "swap"),
        "outerHTML",
        "WHAT: the workspace row's form swaps by {swap:?}. WHY: the accepted name replaces that \
         row itself. Reading: {renames}",
        swap = text(&head, "swap")
    );
    assert!(
        text(&head, "words").contains("Acme renamed"),
        "WHAT: the workspace row reads {words:?} after a name she was allowed to give it. WHY: \
         the row she is looking at is the one the rename replaced. Reading: {renames}",
        words = text(&head, "words")
    );
}

#[then(regex = r"^each refusal is answered inside the row it was refused in$")]
async fn then_each_refusal_is_answered_in_its_row(world: &mut FoundryWorld) {
    let record = recalled(world);
    let renames = record["renames"].clone();
    for which in ["projectRow", "workspaceHead"] {
        let row = renames[which].clone();
        assert!(
            !row.is_null(),
            "WHAT: the row {which} is gone after its refusal. WHY: a refusal must leave the row \
             standing with the reason inside it. Reading: {renames}"
        );
        assert_eq!(
            row["hasErrorSlot"].as_bool(),
            Some(true),
            "WHAT: the row {which} holds no place for a refusal. WHY: the refusal is routed at the \
             slot's own child, so a row without the slot discards the reason. Reading: {renames}"
        );
        let words = text(&row, "errorWords");
        assert!(
            !words.is_empty(),
            "WHAT: the row {which} shows no reason after a name it refuses. WHY: she is left \
             looking at a row that did nothing and said nothing. Reading: {renames}"
        );
    }
}

// ==========================================================================
// L5 — Delete is the quiet control of the issue dialog
// ==========================================================================

async fn open_the_lane_delete_dialog(world: &mut FoundryWorld) {
    goto(world, BOARD_PATH).await;
    browser_harness::wait_for_board_ready(browser_of(world)).await;
    let browser = browser_of(world);
    let slug = browser
        .execute(
            "const card = document.querySelector('article.issue-card'); \
             const col = card && card.closest('section.column'); \
             const trigger = col && col.querySelector('[data-action=\"toggle-lane-menu\"]'); \
             return trigger ? (trigger.getAttribute('data-lane') || '') : '';",
            Vec::new(),
        )
        .await
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();
    assert!(
        !slug.is_empty(),
        "WHAT: no lane holding a card offered its menu. WHY: the destination list lives in the \
         dialog that deletes a lane which still holds a card."
    );
    browser
        .find(fantoccini::Locator::Css(&format!(
            "button[data-action='toggle-lane-menu'][data-lane='{slug}']"
        )))
        .await
        .unwrap_or_else(|err| panic!("the lane {slug} offers no menu: {err}"))
        .click()
        .await
        .unwrap_or_else(|err| panic!("the lane menu could not be opened: {err}"));
    wait_for_truth(
        browser,
        &format!(
            "return !!document.querySelector(\"[data-lane-menu='{slug}']:not([hidden]) \
             button[hx-get$='/delete']\");"
        ),
        "the open lane menu's delete",
    )
    .await;
    browser
        .find(fantoccini::Locator::Css(&format!(
            "[data-lane-menu='{slug}']:not([hidden]) button[hx-get$='/delete']"
        )))
        .await
        .unwrap_or_else(|err| panic!("the open lane menu offers no delete: {err}"))
        .click()
        .await
        .unwrap_or_else(|err| panic!("the lane delete could not be pressed: {err}"));
    wait_for_truth(
        browser,
        "return !!document.querySelector('[data-modal=\"delete-lane\"] select[name=\"destination\"]');",
        "the lane-delete dialog and its destination list",
    )
    .await;
}

async fn open_the_edit_dialog(world: &mut FoundryWorld) {
    goto(world, BOARD_PATH).await;
    browser_harness::wait_for_board_ready(browser_of(world)).await;
    browser_harness::wait_for_kb_ready(browser_of(world)).await;
    let browser = browser_of(world);
    let card = browser
        .find(fantoccini::Locator::Css("article.issue-card"))
        .await
        .unwrap_or_else(|err| panic!("her board shows no card to open: {err}"));
    card.click()
        .await
        .unwrap_or_else(|err| panic!("the card could not be opened: {err}"));
    wait_for_truth(
        browser,
        "return !!document.querySelector('.modal-dialog input[name=\"title\"]');",
        "the edit dialog of the card",
    )
    .await;
}

async fn open_the_new_issue_dialog(world: &mut FoundryWorld) {
    browser_harness::wait_for_kb_ready(browser_of(world)).await;
    let browser = browser_of(world);
    browser
        .find(fantoccini::Locator::Css("[data-action='new-issue']"))
        .await
        .unwrap_or_else(|err| panic!("her board offers no New issue control: {err}"))
        .click()
        .await
        .unwrap_or_else(|err| panic!("the New issue control could not be pressed: {err}"));
    wait_for_truth(
        browser,
        "return !!document.querySelector('[data-modal=\"new-issue\"] input[name=\"title\"]');",
        "the new-issue dialog",
    )
    .await;
}

#[when(regex = r"^she opens the edit dialog of a card on her board$")]
async fn when_she_opens_the_edit_dialog(world: &mut FoundryWorld) {
    let palette = text(&recalled(world), "palette");
    open_the_edit_dialog(world).await;
    let dialog = read_with(browser_of(world), DIALOG_READING).await;
    remember(
        world,
        serde_json::json!({ "palette": palette, "dialog": dialog }),
    );
}

#[given(regex = r"^she has opened the edit dialog of a card on her board$")]
async fn given_she_has_opened_the_edit_dialog(world: &mut FoundryWorld) {
    open_the_edit_dialog(world).await;
}

fn dialog_of(world: &FoundryWorld) -> serde_json::Value {
    let dialog = recalled(world)["dialog"].clone();
    assert_eq!(
        dialog["opened"].as_bool(),
        Some(true),
        "WHAT: the dialog this scenario is about never opened. WHY: every reading below is of its \
         two controls, so a closed dialog would pass them all on nothing. Reading: {dialog}"
    );
    dialog
}

#[then(regex = r"^Delete is drawn as one of Foundry's quiet controls and Save as the main action$")]
async fn then_delete_is_quiet_and_save_is_primary(world: &mut FoundryWorld) {
    let record = recalled(world);
    let palette = text(&record, "palette");
    let dialog = dialog_of(world);
    let delete = dialog["deleteControl"].clone();
    let save = dialog["save"].clone();
    assert!(
        !delete.is_null() && !save.is_null(),
        "WHAT: the dialog holds {delete:?} for Delete and {save:?} for Save. WHY: the two tiers \
         are compared against each other, so a missing control makes the comparison empty."
    );
    let delete_fill = text(&delete, "fill");
    let save_fill = text(&save, "fill");
    assert_ne!(
        delete_fill, save_fill,
        "WHAT: in the {palette} palette Delete and Save resolve the same fill {delete_fill}. WHY: \
         a Delete drawn like the main action of the dialog invites the press she least wants to \
         make. HOW: give Delete the shipped quiet pairing and leave Save the only filled control."
    );
    assert!(
        paints_agree(&delete_fill, &text(&dialog, "tokenSurface")),
        "WHAT: Delete fills with {delete_fill} where the surface token is {surface}. WHY: a third \
         colour still looks quieter than Save and is not the shipped quiet control. HOW: paint \
         Delete with the surface token.",
        surface = text(&dialog, "tokenSurface")
    );
    assert!(
        paints_agree(&text(&delete, "borderColour"), &text(&dialog, "tokenMuted")),
        "WHAT: Delete edges with {edge} where the muted token is {muted}. WHY: the quiet control \
         keeps the shipped hairline, and a different edge is a different control. HOW: edge Delete \
         with the muted token.",
        edge = text(&delete, "borderColour"),
        muted = text(&dialog, "tokenMuted")
    );
    assert!(
        paints_agree(&save_fill, &text(&dialog, "tokenBlack")),
        "WHAT: Save fills with {save_fill} where the primary token is {primary}. WHY: Save is the \
         one filled action of the dialog. HOW: paint Save with the primary token.",
        primary = text(&dialog, "tokenBlack")
    );
    let delete_border = text(&delete, "borderColour");
    let delete_behind = text(&delete, "behind");
    assert!(
        colour_over(&delete_fill, &delete_behind).is_some(),
        "WHAT: Delete resolves the fill {delete_fill:?}, which is no colour. WHY: a control with \
         nothing to measure cannot be held to the shipped boundary obligation."
    );
    assert!(
        num(&delete, "borderWidth") >= FAMILY_BORDER_PX - 0.5,
        "WHAT: Delete draws a {width:.1}px edge in the {palette} palette. WHY: a quiet control \
         with no fill and no edge has no boundary at all, which the shipped control law refuses. \
         HOW: take the shipped quiet pairing, which keeps its hairline. Border: {delete_border}",
        width = num(&delete, "borderWidth")
    );
    assert_eq!(
        text(&save, "buttonType"),
        "submit",
        "WHAT: the control read as Save is a {kind}. WHY: the main action of the dialog is the one \
         that submits her edit; comparing Delete against anything else compares nothing.",
        kind = text(&save, "buttonType")
    );
    assert_eq!(
        text(&delete, "buttonType"),
        "button",
        "WHAT: Delete came back as a {kind}. WHY: it sits outside the form on purpose, so it can \
         never submit the edit she is looking at.",
        kind = text(&delete, "buttonType")
    );
}

#[then(regex = r"^Delete has an edge she can see against the dialog$")]
async fn then_delete_has_a_visible_edge(world: &mut FoundryWorld) {
    let record = recalled(world);
    let palette = text(&record, "palette");
    let dialog = dialog_of(world);
    let delete = dialog["deleteControl"].clone();
    let behind = text(&delete, "behind");
    let edge = colour_over(&text(&delete, "borderColour"), &behind);
    let surface = colour_over(&behind, &behind);
    let (Some(edge), Some(surface)) = (edge, surface) else {
        panic!(
            "WHAT: Delete's edge or the surface behind it resolves no colour at all in the \
             {palette} palette. WHY: a control with no measurable boundary is the ghost control \
             the design refused. Reading: {delete}"
        )
    };
    let ratio = contrast_ratio(edge, surface);
    assert!(
        ratio >= BOUNDARY_RATIO,
        "WHAT: Delete's edge measures {ratio:.2} to 1 against the dialog in the {palette} \
         palette. WHY: the shipped law holds every control to an identifiable boundary at \
         {BOUNDARY_RATIO} to 1, and a quiet control is where that is easiest to lose. HOW: keep \
         the shipped hairline token on it."
    );
    let words = colour_over(&text(&delete, "ink"), &text(&delete, "fill"));
    let fill = colour_over(&text(&delete, "fill"), &behind);
    if let (Some(words), Some(fill)) = (words, fill) {
        let ratio = contrast_ratio(words, fill);
        assert!(
            ratio >= TEXT_RATIO,
            "WHAT: Delete's own word reads at {ratio:.2} to 1 in the {palette} palette. WHY: a \
             quiet control still has to be readable. HOW: pair the quiet fill with the shipped \
             text token."
        );
    }
}

#[then(regex = r"^the status list and the title field of that dialog are Foundry's own$")]
async fn then_the_dialog_controls_are_foundrys_own(world: &mut FoundryWorld) {
    let dialog = dialog_of(world);
    let status = dialog["status"].clone();
    assert!(
        !status.is_null(),
        "WHAT: the dialog holds no status list. WHY: this is one of the two dialog lists the \
         control family must reach. Reading: {dialog}"
    );
    assert!(
        num(&status, "borderWidth") >= FAMILY_BORDER_PX - 0.5,
        "WHAT: the dialog's status list draws a {width:.1}px edge. WHY: the control family is not \
         scoped to the frame; a list is drawn by the platform wherever it sits. HOW: let select \
         join the one field rule group.",
        width = num(&status, "borderWidth")
    );
    assert_eq!(
        text(&dialog, "statusMinHeight"),
        format!("{FAMILY_MIN_HEIGHT_PX:.0}px"),
        "WHAT: the dialog's status list asks for a minimum height of {asked}. WHY: a list half the \
         height of the field above it is the platform's box beside Foundry's. HOW: let it join the \
         field rule group.",
        asked = text(&dialog, "statusMinHeight")
    );
    assert_eq!(
        text(&dialog, "statusFace"),
        text(&dialog, "bodyFace"),
        "WHAT: the dialog's status list draws its words in {face} where the page draws {body}. \
         WHY: the platform's own face beside Foundry's is what makes the control read as somebody \
         else's. HOW: inherit the face.",
        face = text(&dialog, "statusFace"),
        body = text(&dialog, "bodyFace")
    );
    assert_ne!(
        text(&dialog, "statusAppearance"),
        "none",
        "WHAT: the status list asks the browser to draw nothing. WHY: with the arrow taken away \
         and nothing drawn in its place she cannot tell the list from a field."
    );
}

#[when(regex = r"^she presses Delete in that dialog$")]
async fn when_she_presses_delete(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    browser
        .find(fantoccini::Locator::Css("[data-action='delete-issue']"))
        .await
        .unwrap_or_else(|err| panic!("the dialog offers no Delete: {err}"))
        .click()
        .await
        .unwrap_or_else(|err| panic!("Delete could not be pressed: {err}"));
    wait_for_truth(
        browser,
        "return (document.querySelector('#modal-root') || {textContent: ''})\
            .textContent.toLowerCase().indexOf('delete') >= 0 && \
            !document.querySelector('.modal-dialog input[name=\"title\"]');",
        "the confirmation she is asked for",
    )
    .await;
    let dialog = read_with(browser_of(world), DIALOG_READING).await;
    let words = browser_of(world)
        .execute(
            "return (document.querySelector('#modal-root') || {textContent: ''}).textContent.trim();",
            Vec::new(),
        )
        .await
        .map(|value| value.as_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    remember(
        world,
        serde_json::json!({ "dialog": dialog, "confirm": words }),
    );
}

#[then(regex = r"^she is asked to confirm before the issue in that dialog is deleted$")]
async fn then_she_is_asked_to_confirm(world: &mut FoundryWorld) {
    let record = recalled(world);
    let words = text(&record, "confirm");
    assert!(
        !words.is_empty(),
        "WHAT: pressing Delete left the dialog's own place empty. WHY: the control's whole \
         mechanism is the confirmation it fetches, so an empty answer means the press did nothing \
         and the operator is left unsure whether her issue is gone. Record: {record}"
    );
    let lowered = words.to_lowercase();
    assert!(
        lowered.contains("delete"),
        "WHAT: the answer to Delete reads {words:?}. WHY: she must be asked to confirm before \
         anything is deleted, which is the step the quiet presentation must not cost."
    );
}

// ==========================================================================
// L6 — the board header is one row, the key is a labelled value
// ==========================================================================

#[given(regex = r"^she has opened her board$")]
async fn given_she_has_opened_her_board(world: &mut FoundryWorld) {
    goto(world, BOARD_PATH).await;
    browser_harness::wait_for_board_ready(browser_of(world)).await;
    browser_harness::wait_for_kb_ready(browser_of(world)).await;
}

#[when(regex = r"^she opens her board and its change report$")]
async fn when_she_opens_the_board_and_the_report(world: &mut FoundryWorld) {
    goto(world, BOARD_PATH).await;
    browser_harness::wait_for_board_ready(browser_of(world)).await;
    let board = read_with(browser_of(world), HEADER_READING).await;
    goto(world, REPORT_PATH).await;
    wait_until_on(browser_of(world), REPORT_PATH).await;
    let report = read_with(browser_of(world), HEADER_READING).await;
    remember(
        world,
        serde_json::json!({ "board": board, "report": report }),
    );
}

#[then(regex = r#"^each page names the project key as a labelled value under "([^"]+)"$"#)]
async fn then_each_page_names_the_key_as_a_value(world: &mut FoundryWorld, label: String) {
    let record = recalled(world);
    for which in ["board", "report"] {
        let reading = record[which].clone();
        let element = reading["keyElement"].clone();
        assert!(
            !element.is_null(),
            "WHAT: the {which} holds the key in no element of its own. WHY: a key that is a run of \
             text in a sentence cannot be read as a value — not by her eye and not by anything \
             that reads the page. HOW: put the key in its own element carrying the key. Reading: \
             {reading}"
        );
        let attribute = text(&reading, "keyAttribute");
        assert_eq!(
            attribute, "GEN",
            "WHAT: the {which} carries the key {attribute:?} where the project's stored key is \
             \"GEN\". WHY: a key shortened, padded or re-lettered on the page is a different key \
             to her and to anybody she sends it to."
        );
        assert_eq!(
            text(&reading, "keyWords"),
            attribute,
            "WHAT: the {which} draws {words:?} in the element whose value is {attribute:?}. WHY: \
             the value she reads and the value the page carries must be the one key.",
            words = text(&reading, "keyWords")
        );
        let header = text(&reading, "labelWords");
        assert!(
            header.contains(&label),
            "WHAT: the {which} header reads {header:?}, with no label {label:?} over the key. \
             WHY: the operator chose that label; a bare key beside a project name reads as part of \
             the name."
        );
    }
}

#[then(regex = r"^the two board controls are drawn as quiet controls with a gap between them$")]
async fn then_the_board_controls_are_quiet_with_a_gap(world: &mut FoundryWorld) {
    let record = recalled(world);
    let board = record["board"].clone();
    let report = board["report"].clone();
    let integrations = board["integrations"].clone();
    assert!(
        !report.is_null() && !integrations.is_null(),
        "WHAT: the board offers {report:?} and {integrations:?}. WHY: both controls are drawn for \
         an operator who may manage the project, so a reading with one of them missing proves \
         nothing about the row."
    );
    for (name, control) in [
        ("the change report", &report),
        ("integrations", &integrations),
    ] {
        assert!(
            paints_agree(&text(control, "fill"), &text(&board, "tokenSurface")),
            "WHAT: {name} resolves the fill {fill} where the surface token is {wanted}. WHY: two \
             run-together links are not two controls; she reads a drawn control as something she \
             can press. HOW: give both the shipped quiet pairing.",
            fill = text(control, "fill"),
            wanted = text(&board, "tokenSurface")
        );
        assert!(
            paints_agree(&text(control, "borderColour"), &text(&board, "tokenMuted")),
            "WHAT: {name} draws its edge in {edge} where the muted token is {wanted}. WHY: the \
             edge is what identifies a control with no fill.",
            edge = text(control, "borderColour"),
            wanted = text(&board, "tokenMuted")
        );
        assert!(
            num(control, "borderWidth") >= FAMILY_BORDER_PX - 0.5,
            "WHAT: {name} draws a {width:.1}px edge. WHY: a quiet control with no edge has no \
             boundary. HOW: keep the shipped hairline.",
            width = num(control, "borderWidth")
        );
    }
    let gap = num(&integrations, "left") - num(&report, "right");
    assert!(
        gap > 0.0,
        "WHAT: the two board controls leave {gap:.1}px between their boxes. WHY: run together \
         they read as one long link, which is the header the operator reported. HOW: lay the row \
         out with a gap."
    );
    let same_row = (num(&report, "top") - num(&integrations, "top")).abs() <= SLACK_PX;
    assert!(
        same_row,
        "WHAT: the two board controls sit at {first:.1} and {second:.1} down the page. WHY: the \
         promise is one tidy row, not a stack. HOW: lay them out on one row.",
        first = num(&report, "top"),
        second = num(&integrations, "top")
    );
}

#[then(regex = r"^neither board control is underlined$")]
async fn then_neither_board_control_is_underlined(world: &mut FoundryWorld) {
    let record = recalled(world);
    let board = record["board"].clone();
    for which in ["report", "integrations"] {
        let control = board[which].clone();
        let decoration = text(&control, "decoration");
        assert!(
            !decoration.contains("underline"),
            "WHAT: the board control {which} is drawn with {decoration}. WHY: an underlined run of \
             words beside another one is the pair she reported as run-together links. HOW: draw \
             them as controls, which carry their own boundary instead."
        );
    }
}

#[then(regex = r"^the change report is still reached by the address the live board reads$")]
async fn then_the_report_keeps_the_address_the_live_board_reads(world: &mut FoundryWorld) {
    let record = recalled(world);
    let report = record["board"]["report"].clone();
    let classes = text(&report, "cls");
    assert!(
        classes.split_whitespace().any(|cls| cls == "report-link"),
        "WHAT: the change report control carries the classes {classes:?}. WHY: the live board \
         finds this control by that class and derives its own address from it; renamed, the board \
         stops refreshing and says nothing. HOW: add the control classes beside the shipped one."
    );
    let href = text(&report, "href");
    assert!(
        href.ends_with("/report"),
        "WHAT: the change report control opens {href:?}. WHY: the live board strips that ending to \
         derive the address it listens on. HOW: keep the shipped address."
    );
}

// ==========================================================================
// A board she may not manage
// ==========================================================================

#[when(regex = r"^that teammate opens the board$")]
async fn when_the_teammate_opens_the_board(world: &mut FoundryWorld) {
    let cookie = text(&recalled(world), "cookie");
    let base = harness_of(world).base_url();
    let body = http_of(world)
        .get(format!("{base}{BOARD_PATH}"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("open the board as the teammate")
        .text()
        .await
        .expect("read the board as the teammate");
    remember(world, serde_json::json!({ "body": body }));
}

#[then(regex = r#"^the board names the project key as a labelled value under "([^"]+)"$"#)]
async fn then_the_served_board_names_the_key(world: &mut FoundryWorld, label: String) {
    let record = recalled(world);
    let body = text(&record, "body");
    let doc = html::parse(&body);
    let keyed = html::select_all(&doc, "[data-key-prefix]");
    assert_eq!(
        keyed.len(),
        1,
        "WHAT: the board she may read holds {n} elements carrying the project key. WHY: the key is \
         one value in one element; none leaves it as a run of text and two leave two answers to \
         the same question.",
        n = keyed.len()
    );
    let carried = keyed[0].value().attr("data-key-prefix").unwrap_or_default();
    assert_eq!(
        carried, "GEN",
        "WHAT: the board carries the key {carried:?} where the project's stored key is \"GEN\"."
    );
    assert!(
        body.contains(&label),
        "WHAT: the board she may read shows no label {label:?} over the key. WHY: the label is \
         what makes the key a value she can read."
    );
}

#[then(regex = r"^only the change report is offered to her, in the row of board controls$")]
async fn then_only_the_report_is_offered(world: &mut FoundryWorld) {
    let record = recalled(world);
    let body = text(&record, "body");
    let doc = html::parse(&body);
    let report = html::select_all(&doc, "nav.board-actions a.report-link");
    let integrations = html::select_all(&doc, "a.integrations-link");
    assert_eq!(
        report.len(),
        1,
        "WHAT: the board offers {n} change-report controls in its row of controls. WHY: a reader \
         who may not manage the project still reads its change report, and the row is where she \
         finds it.",
        n = report.len()
    );
    assert!(
        integrations.is_empty(),
        "WHAT: the board offers integrations to a reader who may not manage the project. WHY: this \
         work changes presentation, never who may open a page."
    );
}

// ==========================================================================
// The live board, and the search panel's own place
// ==========================================================================

#[when(regex = r"^the issue on her board is deleted from somewhere else$")]
async fn when_the_issue_is_deleted_elsewhere(world: &mut FoundryWorld) {
    let present = browser_of(world)
        .execute(
            "return Array.from(document.querySelectorAll('article.issue-card'))\
                .map(c => c.getAttribute('data-issue-key') || '');",
            Vec::new(),
        )
        .await
        .ok()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let keys: Vec<String> = present
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect();
    assert!(
        keys.iter().any(|key| key == "GEN-1"),
        "WHAT: the open board shows {keys:?}, with no GEN-1. WHY: the refresh is the card leaving, \
         so a board that never held it would pass with the live connection dead."
    );
    let marked = browser_of(world)
        .execute(
            "const card = Array.from(document.querySelectorAll('article.issue-card'))
                 .find(c => (c.getAttribute('data-issue-key') || '') === 'GEN-1');
             const lane = card && card.parentElement;
             if (!lane) { return false; }
             lane.__sameLane = 'kept';
             return true;",
            Vec::new(),
        )
        .await
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    assert!(
        marked,
        "WHAT: the open board has no column holding GEN-1 to keep watching. WHY: the card leaves \
         from the column she already has open; with no column the reading would be of some other page."
    );
    ensure_harness(world).await;
    let base = harness_of(world).base_url();
    let cookie =
        establish_session(harness_of(world), http_of(world), MEI_EMAIL, MEI_PASSWORD).await;
    /* The anti-forgery token is independent of her session. The sign-in page
    mints it, and the same value travels as the cookie and as the field, which
    is the double-submit the application asks for. */
    let minted = http_of(world)
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("open the sign-in page for the anti-forgery mint")
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|value| value.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .expect("the sign-in page mints the anti-forgery cookie")
        .to_string();
    let cookie = format!("{cookie}; foundry_csrf={minted}");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("_csrf", minted);
    let answer = http_of(world)
        .post(format!("{base}{BOARD_PATH}/issues/1/delete"))
        .header("cookie", &cookie)
        .form(&form)
        .send()
        .await
        .expect("delete the issue from somewhere else");
    let status = answer.status();
    let body = answer.text().await.unwrap_or_default();
    assert!(
        status.is_success() || status.is_redirection(),
        "WHAT: deleting the issue from somewhere else answered {status}: {body}. WHY: with nothing \
         deleted the open board has no change to show and the reading would pass on nothing.",
        body = body.chars().take(240).collect::<String>()
    );
    remember(world, serde_json::json!({ "gone": "GEN-1" }));
}

#[then(regex = r"^that card leaves her open board without her reloading it$")]
async fn then_the_card_leaves_her_open_board(world: &mut FoundryWorld) {
    let script = "const cols = document.querySelector('.board#board-columns');
         const keys = Array.from(document.querySelectorAll('article.issue-card'))
             .map(c => c.getAttribute('data-issue-key') || '');
         const held = cols ? [cols].concat(Array.from(cols.querySelectorAll('*'))) : [];
         const sameLane = held.some(el => el.__sameLane === 'kept');
         return {
             sameLane: sameLane,
             gone: keys.indexOf('GEN-1') < 0,
             here: window.location.pathname === '/team/backend/project/sandbox',
             keys: keys
         };";
    let mut last = serde_json::Value::Null;
    for _ in 0..100 {
        last = browser_of(world)
            .execute(script, Vec::new())
            .await
            .unwrap_or(serde_json::Value::Null);
        let same = last["sameLane"].as_bool() == Some(true);
        let gone = last["gone"].as_bool() == Some(true);
        let here = last["here"].as_bool() == Some(true);
        if same && gone && here {
            return;
        }
        // A reload or a replacement of the columns is already the wrong refresh.
        if !same || !here {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!(
        "WHAT: the open board reads {last}. WHY: a card deleted somewhere else leaves the column \
         she already has open, because the live board takes its address from the change report \
         control and drops that one card. A reload, a replacement of that column, or a new card \
         appearing would be a different board. HOW: keep the change report control's class and \
         its address ending in /report, so the open board still hears the deletion."
    );
}

#[when(regex = r"^she presses the slash key$")]
async fn when_she_presses_slash(world: &mut FoundryWorld) {
    browser_harness::wait_for_kb_ready(browser_of(world)).await;
    browser_harness::press_key(browser_of(world), "/").await;
    wait_for_truth(
        browser_of(world),
        "return !!document.querySelector('[data-search-panel], [data-search-results]');",
        "the search panel after the slash key",
    )
    .await;
    let reading = read_with(
        browser_of(world),
        r#"
function px(v) { const n = parseFloat(v); return isNaN(n) ? 0 : n; }
const trigger = document.querySelector('[data-action="new-issue"]');
const results = document.querySelector('[data-search-results]');
const panel = results ? results.parentElement : null;
if (!trigger || !panel) { return { found: false }; }
const tr = trigger.getBoundingClientRect();
const pr = panel.getBoundingClientRect();
const parentCs = getComputedStyle(trigger.parentElement);
return {
  found: true,
  sameParent: panel.parentElement === trigger.parentElement,
  parentDisplay: parentCs.display,
  panelTop: pr.top, panelLeft: pr.left, panelWidth: pr.width, panelHeight: pr.height,
  triggerTop: tr.top, triggerBottom: tr.bottom, triggerRight: tr.right
};
"#,
    )
    .await;
    remember(world, serde_json::json!({ "panel": reading }));
}

#[then(regex = r"^the search panel is drawn in its own block beside the New issue control$")]
async fn then_the_search_panel_is_in_its_own_block(world: &mut FoundryWorld) {
    let record = recalled(world);
    let panel = record["panel"].clone();
    assert_eq!(
        panel["found"].as_bool(),
        Some(true),
        "WHAT: after the slash key there is no search panel beside the New issue control. WHY: the \
         panel is put in beside that control, inside its own parent; a control moved into a row \
         takes the panel with it. Reading: {panel}"
    );
    assert_eq!(
        panel["sameParent"].as_bool(),
        Some(true),
        "WHAT: the search panel no longer shares the New issue control's own parent. WHY: that is \
         where the panel is inserted, so a different parent means the panel went somewhere nobody \
         laid out. Reading: {panel}"
    );
    let display = text(&panel, "parentDisplay");
    assert!(
        !display.contains("flex") && !display.contains("grid"),
        "WHAT: the New issue control's parent lays its children out as {display}. WHY: the search \
         panel is inserted as that control's next sibling, so a row parent lays the whole panel out \
         as one more item in the row. HOW: keep that parent a plain block."
    );
    assert!(
        num(&panel, "panelHeight") > 0.0 && num(&panel, "panelWidth") > 0.0,
        "WHAT: the search panel is drawn {w:.0} by {h:.0}. WHY: a panel with no box is a panel she \
         cannot type into.",
        w = num(&panel, "panelWidth"),
        h = num(&panel, "panelHeight")
    );
    assert!(
        num(&panel, "panelTop") >= num(&panel, "triggerTop") - SLACK_PX,
        "WHAT: the search panel is drawn above the New issue control it is inserted after. WHY: \
         laid out as a row item it would sit beside the control instead of under it. Reading: \
         {panel}"
    );
}
