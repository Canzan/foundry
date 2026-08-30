//! `BrowserHarness` — the `@needs-browser` lane's driver (ADR-007).
//!
//! ```text
//! BrowserHarness = InProcHarness (UNCHANGED)  +  fantoccini::Client -> InProcHarness::base_url()
//! ```
//!
//! `InProcHarness` already binds a REAL ephemeral TCP socket and `axum::serve`s a
//! real origin (`foundry-app/src/lib.rs:726-746`, exposed as `base_url()`), so a
//! real browser can point at it TODAY. "In-process" means *same OS process as the
//! test binary*, not *no socket*. There is therefore no new serving plumbing here
//! — only a WebDriver session aimed at the origin the port-to-port suite already
//! uses, which is what keeps the two lanes exercising the SAME app.
//!
//! LIFECYCLE (ADR-007 §4)
//! - ONE chromedriver **process** per lane (per test binary), started lazily and
//!   reaped EXPLICITLY by [`shutdown_chromedriver`], which `tests/acceptance.rs`
//!   calls after the cucumber run returns. Nothing reaps it implicitly — see
//!   [`CHROMEDRIVER_PROC`] for why, and for what this still does not cover.
//! - ONE **session** per scenario, `--headless=new`, FIXED window size so a later
//!   `scrollIntoView` assertion (AC-05.3) is deterministic rather than dependent
//!   on the runner's screen.
//!
//! WAITS ARE CONDITIONS, NEVER SLEEPS. The only bounded wait exposed here is
//! `wait_for_kb_ready`, a `wait().at_most()` on the `[data-kb-ready]` readiness
//! marker (ADR-001). The one `sleep` in this file is in `wait_for_driver_ready`,
//! which polls chromedriver's OWN `/status` endpoint — that is a poll interval on
//! an explicit readiness condition for an external PROCESS, not a timing
//! assumption about the app under test.
//!
//! PROBE, THEN REFUSE — NEVER SKIP. A missing or version-skewed chromedriver
//! makes this harness PANIC with an actionable diagnostic; it never `#[ignore]`s
//! and never soft-passes. A lane that silently skips when the driver is absent
//! recreates the exact failure mode this feature exists to close: a green suite
//! over an absent capability. `cargo xtask ci` additionally preflights the
//! driver/browser major-version match before the lane ever starts.

use crate::support::harness::InProcHarness;
use fantoccini::{ClientBuilder, Locator};
use hyper_util::client::legacy::connect::HttpConnector;
use once_cell::sync::OnceCell;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// Fixed viewport for every session — determinism over the runner's screen.
const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 900;

/// Bound on every readiness condition. Generous enough to absorb a cold page
/// load on a loaded CI box, short enough that a genuinely-absent marker fails
/// the lane promptly rather than hanging it.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// The ADR-001 readiness marker: `keyboard.js` sets it at init. It is both this
/// lane's wait condition AND US-02's "the layer is live" precondition, so the
/// anti-vacuity guard has a real hook.
pub const KB_READY_SELECTOR: &str = "[data-kb-ready]";

/// ONE chromedriver process per lane. `OnceCell` so the Nth scenario reuses the
/// 1st scenario's driver; the `Child` is parked in a `Mutex` so the lane can
/// reach it again at teardown.
///
/// NOTHING REAPS THIS CHILD IMPLICITLY, and the comment that used to sit here
/// claimed the opposite ("the OS reaps it on exit — the same contract the
/// shared Postgres testcontainer has"). Both halves were false:
///
///   * `std::process::Child` explicitly does NOT kill on drop, and a `static`'s
///     `Drop` never runs at process exit anyway. On Unix the driver is
///     reparented to init and outlives the run. Measured 2026-08-30 on a dev
///     machine: chromedriver processes with PPID 1, accumulated across runs.
///   * The testcontainer analogy inverted the actual contract. Testcontainers
///     is reaped DELIBERATELY, by `harness::shutdown_postgres()` in
///     `tests/acceptance.rs`, precisely because its `Drop` cannot do the job
///     either. A raw `Command::spawn` has no reaper at all.
///
/// So the lane reaps it the same way: [`shutdown_chromedriver`], called from
/// `tests/acceptance.rs` beside `shutdown_postgres`.
///
/// WHAT THIS DOES NOT COVER — clean exits only. An interrupted run (Ctrl-C, a
/// harness timeout kill, a panic that aborts rather than unwinds) still orphans
/// the driver, because there is no portable way to bind a child's lifetime to
/// its parent on macOS: Linux has `prctl(PR_SET_PDEATHSIG)`, Darwin has no
/// equivalent. Strays from interrupted runs are reaped by hand
/// (`pkill -f chromedriver`). Overstating a fix is what put the wrong comment
/// here in the first place, so this one states its limit.
static CHROMEDRIVER: OnceCell<u16> = OnceCell::new();
static CHROMEDRIVER_PROC: Mutex<Option<Child>> = Mutex::new(None);

/// Reap this lane's chromedriver: kill it and WAIT for it, so no orphan and no
/// zombie survives the run. Returns the status waited for, or `None` when the
/// lane never started a driver — every lane calls this, including those that
/// filter `@needs-browser` out.
///
/// Call it AFTER the cucumber run returns, next to `harness::shutdown_postgres`.
/// See [`CHROMEDRIVER_PROC`] for the interrupted-run case this cannot cover.
///
/// The `wait` is the load-bearing half: `kill` alone leaves a zombie in this
/// process's child table, which is a smaller leak than an orphan but still a
/// leak. Taking the child OUT of the slot first makes a second call a no-op, so
/// a teardown that runs twice cannot wait on an already-reaped pid.
pub fn shutdown_chromedriver() -> Option<ExitStatus> {
    let mut child = CHROMEDRIVER_PROC
        .lock()
        .expect("chromedriver proc lock")
        .take()?;
    let _ = child.kill();
    child.wait().ok()
}

/// Ask the OS for a free port, then release it. chromedriver has no
/// "bind :0 and tell me the port" mode, so this bind-and-drop is the shipped
/// idiom for handing an external process an ephemeral port.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

/// Start chromedriver once for this lane and return its port.
///
/// PANICS with an install hint when chromedriver is absent. This is the refusal
/// half of "probe, then refuse" (ADR-007 §4): a browser lane that skips is
/// indistinguishable from the bug it exists to prevent.
fn ensure_chromedriver() -> u16 {
    *CHROMEDRIVER.get_or_init(|| {
        let port = free_port();
        let child = Command::new("chromedriver")
            .arg(format!("--port={port}"))
            .arg("--silent")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|err| {
                panic!(
                    "the @needs-browser lane requires chromedriver on PATH, and it could not be \
                     started: {err}\n  install it:  macOS -> `brew install --cask chromedriver`;  \
                     Debian/Ubuntu -> `sudo apt-get install -y chromium-driver`\n  the driver's \
                     MAJOR version must MATCH the installed Chrome's (chromedriver refuses a \
                     mismatched browser). `cargo xtask ci` preflights this."
                )
            });
        *CHROMEDRIVER_PROC.lock().expect("chromedriver proc lock") = Some(child);
        wait_for_driver_ready(port);
        port
    })
}

/// Poll chromedriver's own `/status` until it reports ready. A condition on an
/// external process's readiness endpoint — not a sleep-and-hope.
fn wait_for_driver_ready(port: u16) {
    let deadline = std::time::Instant::now() + READY_TIMEOUT;
    let url = format!("http://127.0.0.1:{port}/status");
    while std::time::Instant::now() < deadline {
        let responded = std::process::Command::new("curl")
            .args(["-fsS", "-o", "/dev/null", "--max-time", "1", &url])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if responded {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("chromedriver did not report ready on 127.0.0.1:{port} within {READY_TIMEOUT:?}");
}

/// Open ONE headless session against this lane's chromedriver, sized to a fixed
/// viewport. One call per scenario.
pub async fn new_session() -> fantoccini::Client {
    open_session(
        Scripting::Enabled,
        ColorScheme::Unstated,
        SiteStorage::Permitted,
    )
    .await
}

/// Open ONE headless session whose DEVICE states a dark colour preference, so
/// `@media (prefers-color-scheme: dark)` actually applies (ADR-003's trap, one
/// layer over).
///
/// A "dark mode" scenario that drives dark by stamping an explicit theme choice
/// on the document leaves the media block GREEN WHETHER OR NOT IT EXISTS — the
/// attribute selector alone satisfies the assertion — and the media path is the
/// DEFAULT state most operators get. This constructor is what makes the default
/// path measurable. See [`ColorScheme`] for the flag and why it is that flag.
pub async fn new_dark_session() -> fantoccini::Client {
    open_session(
        Scripting::Enabled,
        ColorScheme::Dark,
        SiteStorage::Permitted,
    )
    .await
}

/// A dark DEVICE with scripting switched off at the browser — the no-JS × dark
/// corner (NFR-4 × the device-driven default). Combines the two mechanisms
/// [`new_dark_session`] and [`new_session_without_scripting`] each establish.
pub async fn new_dark_session_without_scripting() -> fantoccini::Client {
    open_session(
        Scripting::Disabled,
        ColorScheme::Dark,
        SiteStorage::Permitted,
    )
    .await
}

/// A dark DEVICE whose browser REFUSES this origin access to stored state — the
/// storage-refused corner. Scripting stays ON: the whole point is that the theme
/// script RUNS and its stored-choice read THROWS, so the guard's catch is the
/// thing under test. Composes [`ColorScheme::Dark`] with [`SiteStorage::Refused`];
/// see the latter for the measurement.
pub async fn new_dark_session_refusing_site_storage() -> fantoccini::Client {
    open_session(Scripting::Enabled, ColorScheme::Dark, SiteStorage::Refused).await
}

/// THE ANTI-VACUITY PROBE: what the BROWSER says its device prefers, read from
/// `window.matchMedia('(prefers-color-scheme: dark)').matches`.
///
/// The baseline is `false`, so this discriminates: a dark session reports `true`
/// and a session with no stated preference reports `false`. Every dark-by-device
/// `Given` asserts it BEFORE asserting anything about foundry's own rendering, so
/// if the capability ever stops taking effect the lane fails LOUDLY instead of
/// silently measuring the light palette twice.
pub async fn device_prefers_dark(client: &fantoccini::Client) -> bool {
    let matches = client
        .execute(
            "return window.matchMedia('(prefers-color-scheme: dark)').matches;",
            Vec::new(),
        )
        .await
        .expect("read the device colour preference");
    matches
        .as_bool()
        .expect("matchMedia().matches is a boolean")
}

/// Drain everything the browser has recorded and return only the UNHANDLED
/// SCRIPT ERRORS — the entries Chrome files under `source: "javascript"`.
///
/// The recorder itself is the `goog:loggingPrefs` capability set in
/// [`open_session`], so it is armed BEFORE the first navigation and catches an
/// error thrown while a `<head>` script is still being parsed. An in-page
/// `window.onerror` cannot do that: it would have to be installed by a script
/// that runs after the one it is meant to watch, and it would be destroyed by the
/// navigation it is meant to observe.
///
/// FILTERED TO `source == "javascript"` ON PURPOSE, and this is the one judgement
/// call here. The same log also carries `source: "network"` entries — a headless
/// Chrome asking for a favicon the test origin does not serve files a SEVERE
/// network 404 on every single navigation. That is an artefact of the harness's
/// own substrate, not something foundry reports to an operator, and folding it in
/// would make "nothing was reported" unsatisfiable for reasons having nothing to
/// do with the code under test.
///
/// DESTRUCTIVE: chromedriver hands back the entries accumulated since the last
/// call and clears them, so each call reads a window, not a running total.
///
/// Reached by a direct HTTP call rather than through fantoccini because `/log` is
/// a chromedriver endpoint, not a W3C one, and fantoccini wraps only the latter.
/// Plain HTTP on loopback — the same reasoning that keeps a TLS stack out of this
/// file.
pub async fn unhandled_script_errors(client: &fantoccini::Client) -> Vec<String> {
    let port = ensure_chromedriver();
    let session_id = client
        .session_id()
        .await
        .expect("read the WebDriver session id")
        .expect("the session must still be open to read its log");
    let body: serde_json::Value = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/session/{session_id}/log"))
        .json(&serde_json::json!({ "type": "browser" }))
        .send()
        .await
        .expect("ask chromedriver for the browser log")
        .json()
        .await
        .expect("chromedriver's browser log must be JSON");
    let entries = body["value"].as_array().unwrap_or_else(|| {
        panic!(
            "chromedriver returned no browser-log array, so the unhandled-error recorder is not \
             armed and `nothing was reported` would hold vacuously. Response: {body}"
        )
    });
    entries
        .iter()
        .filter(|entry| entry["source"].as_str() == Some("javascript"))
        .map(|entry| entry["message"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// The mobile device metrics every `open_mobile_session` injects — a mid-range
/// phone's logical viewport (iPhone-12-class: 390×844 CSS px at DPR 3). ADR-003
/// (the load-bearing test decision): headless `--headless=new` is DESKTOP Chrome,
/// which lays out at the OS window width regardless of the `<meta name=viewport>`
/// tag — so a narrow-WINDOW test would be GREEN whether or not the viewport meta
/// exists (green over nothing). chromedriver's `goog:chromeOptions.mobileEmulation`
/// makes Chrome apply REAL mobile viewport semantics: the ~980px fallback layout
/// when no viewport meta is declared (the no-viewport DEFECT reproduces → RED), and
/// the device-width layout once the meta is present (the fix is measurable → GREEN).
const MOBILE_WIDTH: u32 = 390;
const MOBILE_HEIGHT: u32 = 844;
const MOBILE_PIXEL_RATIO: u32 = 3;

/// Open ONE headless MOBILE session against this lane's chromedriver, driving a
/// REAL emulated 390×844 phone viewport (ADR-003). One call per scenario, the
/// mobile counterpart to [`open_session`].
///
/// Unlike the desktop path it injects `mobileEmulation.deviceMetrics` into
/// `goog:chromeOptions` and DELIBERATELY does NOT call `set_window_size`: under
/// mobile emulation the emulated `deviceMetrics` (not the OS window) govern the
/// layout viewport, so `window.innerWidth` reflects 390 and resizing the window
/// would neither change it nor be honoured. This is what separates the honest
/// mobile-viewport probe from a desktop resize that proves nothing.
pub async fn open_mobile_session() -> fantoccini::Client {
    let port = ensure_chromedriver();
    let chrome_options = serde_json::json!({
        "args": [
            "--headless=new",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--disable-gpu",
        ],
        "mobileEmulation": {
            "deviceMetrics": {
                "width": MOBILE_WIDTH,
                "height": MOBILE_HEIGHT,
                "pixelRatio": MOBILE_PIXEL_RATIO,
                "mobile": true,
            }
        }
    });
    let mut capabilities = serde_json::Map::new();
    capabilities.insert("goog:chromeOptions".to_string(), chrome_options);
    ClientBuilder::new(HttpConnector::new())
        .capabilities(capabilities)
        .connect(&format!("http://127.0.0.1:{port}"))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "could not open a mobile chromedriver session: {err}\n  the driver's MAJOR version \
                 must MATCH the installed Chrome's — `chromedriver --version` vs `google-chrome \
                 --version`. `cargo xtask ci` preflights this; a `brew upgrade` that moves one and \
                 not the other is the usual cause."
            )
        })
    // NO set_window_size here: the emulated deviceMetrics own the layout viewport.
}

/// Open ONE headless session with JavaScript switched OFF at the BROWSER, for the
/// no-JS path (NFR-4 / ODD-8).
///
/// This is a real content setting, not a simulation: Chrome parses the page and
/// runs no script at all, so `keyboard.js` never initialises and `[data-kb-ready]`
/// never appears. That absence is the anti-vacuity hook the no-JS scenario asserts
/// — without it, a session that quietly kept scripting ON would let the scenario
/// pass while proving nothing about the scripting-off path.
pub async fn new_session_without_scripting() -> fantoccini::Client {
    open_session(
        Scripting::Disabled,
        ColorScheme::Unstated,
        SiteStorage::Permitted,
    )
    .await
}

/// Whether a session's browser runs page scripts. The no-JS path is a first-class
/// surface here (NFR-4), so it gets a name rather than a bare bool at the call site.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scripting {
    Enabled,
    Disabled,
}

/// What a session's DEVICE says it prefers — the peer of [`Scripting`], and the
/// reason the dark-by-device path is testable at all.
///
/// `Unstated` is the shipped baseline: no flag, `matchMedia` reports `false`, the
/// computed palette resolves LIGHT. `Dark` injects `--force-dark-mode` into
/// `goog:chromeOptions.args` at SESSION CREATION — the same idiom
/// `open_mobile_session` establishes for `mobileEmulation.deviceMetrics`.
///
/// EMPIRICALLY MEASURED, twice (raw headless Chrome via `--dump-dom`, and
/// chromedriver 151.0.7922.138 over W3C `POST /session` + `execute/sync`):
///
/// ```text
///   flags: <none>                                  matchMedia=false  cssvar=LIGHT
///   flags: --force-dark-mode                       matchMedia=true   cssvar=DARK
///   flags: --enable-features=WebContentsForceDark  matchMedia=false  cssvar=LIGHT
/// ```
///
/// BOTH `matchMedia` AND the computed custom property flip under
/// `--force-dark-mode`, so the media block genuinely applies — this is not merely
/// the JS API reporting a preference.
///
/// DO NOT "FIX" THIS TO `--enable-features=WebContentsForceDark`. That is Chrome's
/// AUTO-DARKENING feature, a different thing; it measurably flips neither the
/// `matchMedia` result nor the computed custom property, and substituting it would
/// silently return this lane to green-over-nothing.
///
/// NOT CDP. `POST /session/{id}/goog/cdp/execute` was considered and rejected.
/// fantoccini 0.21.5 does expose `Client::issue_cmd` (`session.rs:338`) and
/// `session_id` (`client.rs:110`), so CDP WAS reachable — recorded so nobody
/// reopens this as a discovery. The rejection is on DETERMINISM, not availability:
/// a runtime call can race page load where a session capability cannot, and the
/// capability needs no side-channel HTTP client and no new dependency.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorScheme {
    /// No stated preference — the shipped baseline (`matchMedia` -> false).
    Unstated,
    /// The device prefers dark (`matchMedia` -> true).
    Dark,
}

/// Whether a session's browser lets this origin touch stored state — the third
/// peer of [`Scripting`] and [`ColorScheme`], and the mechanism behind the
/// storage-refused degradation lane.
///
/// `Permitted` is the shipped baseline: no pref, both reading and writing
/// succeed. `Refused` sets Chrome's SITE-DATA content setting
/// (`profile.default_content_setting_values.cookies = 2`) through
/// `goog:chromeOptions.prefs` at SESSION CREATION — the same capability idiom
/// [`ColorScheme::Dark`] and `open_mobile_session` establish.
///
/// EMPIRICALLY MEASURED against a real `http://` origin under chromedriver
/// 151.0.7922.138 over W3C `POST /session` + `execute/sync` (a `file://` origin
/// would not have exercised content settings at all):
///
/// ```text
///   prefs: <none>                                READ=ok             WRITE=ok
///   prefs: cookies=2                             READ=SecurityError  WRITE=SecurityError
///   prefs: cookies=2 + block_third_party_cookies READ=SecurityError  WRITE=SecurityError
/// ```
///
/// BOTH arms throw, not just the write — which is what makes the READ guard in
/// `theme.js` observable: the stored-choice read throws, the catch returns
/// "follow the device", and the page themes from the device. Composing this with
/// `--force-dark-mode` was measured too: `matchMedia` still reports dark, so the
/// two capabilities do not interfere.
///
/// DO NOT REPLACE THIS WITH A SCRIPT-INJECTED THROWING ACCESSOR. Stubbing
/// `localStorage` from the test would assert against the stub rather than the
/// browser, and it would be the only assertion in this lane not exercising a real
/// substrate. Filling storage to its quota was also rejected: a real exception,
/// but quota semantics vary by platform and a short value overwriting an existing
/// short key may not throw at all — a flaky oracle.
///
/// KNOWN, DELIBERATE CONSEQUENCE: blocking site data also blocks the SESSION
/// COOKIE, so no signed-in screen is reachable under this capability. The
/// storage-refused scenario is therefore driven on the sign-in screen by
/// NECESSITY, not preference — and the write guard has no scenario at all,
/// because "storage is refused" and "the theme control exists" are mutually
/// exclusive by construction (the control mounts only inside the rail, and the
/// rail renders only on signed-in screens).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SiteStorage {
    /// The origin may read and write stored state — the shipped baseline.
    Permitted,
    /// The origin may not: both reads and writes throw `SecurityError`.
    Refused,
}

async fn open_session(
    scripting: Scripting,
    color_scheme: ColorScheme,
    site_storage: SiteStorage,
) -> fantoccini::Client {
    let port = ensure_chromedriver();
    let mut args = vec![
        "--headless=new".to_string(),
        "--no-sandbox".to_string(),
        "--disable-dev-shm-usage".to_string(),
        "--disable-gpu".to_string(),
        format!("--window-size={WINDOW_WIDTH},{WINDOW_HEIGHT}"),
    ];
    if color_scheme == ColorScheme::Dark {
        // See ColorScheme::Dark — measured, and NOT interchangeable with
        // --enable-features=WebContentsForceDark.
        args.push("--force-dark-mode".to_string());
    }
    let mut prefs = serde_json::Map::new();
    if scripting == Scripting::Disabled {
        // Chrome's own JavaScript content setting: 2 == block. Applied as a
        // profile preference so it covers the whole session, every origin.
        prefs.insert(
            "profile.managed_default_content_settings.javascript".to_string(),
            serde_json::json!(2),
        );
    }
    if site_storage == SiteStorage::Refused {
        // Chrome's SITE-DATA content setting: 2 == block. See SiteStorage for the
        // measurement — under this pref BOTH reading and writing stored state
        // throw SecurityError against a real http:// origin.
        prefs.insert(
            "profile.default_content_setting_values.cookies".to_string(),
            serde_json::json!(2),
        );
    }
    let mut chrome_options = serde_json::json!({ "args": args });
    if !prefs.is_empty() {
        chrome_options["prefs"] = serde_json::Value::Object(prefs);
    }
    let mut capabilities = serde_json::Map::new();
    capabilities.insert("goog:chromeOptions".to_string(), chrome_options);
    // THE UNHANDLED-ERROR RECORDER, installed as a SESSION CAPABILITY — i.e.
    // before any navigation, so it cannot miss an error thrown while the very
    // first script is being parsed. That timing is the whole point: the failure
    // the storage-refused scenario guards against is `theme.js` dying at parse
    // time, which an in-page `window.onerror` installed after navigation would
    // arrive too late to see (and which no in-page recorder can survive a
    // navigation to observe at all). See `unhandled_script_errors`.
    capabilities.insert(
        "goog:loggingPrefs".to_string(),
        serde_json::json!({ "browser": "ALL" }),
    );
    // A bare HttpConnector: the WebDriver endpoint is plain HTTP on loopback, so
    // there is no TLS to configure — and no second TLS stack to make rustls'
    // process-level CryptoProvider ambiguous beside reqwest's.
    let client = ClientBuilder::new(HttpConnector::new())
        .capabilities(capabilities)
        .connect(&format!("http://127.0.0.1:{port}"))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "could not open a chromedriver session: {err}\n  the driver's MAJOR version must \
                 MATCH the installed Chrome's — `chromedriver --version` vs `google-chrome \
                 --version`. `cargo xtask ci` preflights this; a `brew upgrade` that moves one \
                 and not the other is the usual cause."
            )
        });
    client
        .set_window_size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .await
        .expect("fix the window size");
    client
}

/// Sign in through the REAL sign-in FORM in the browser — the Earned-Trust probe
/// (ADR-007 §4).
///
/// `harness.rs:401-406` sets `session_cookie_secure: true` and its own comment
/// concedes the port-to-port test "only inspects the header text, not whether the
/// browser would send the cookie back over HTTP". reqwest does not care; a real
/// browser does. Driving the actual form means the browser must ACCEPT a `Secure`
/// cookie over plain HTTP and SEND IT BACK on the next navigation. Chrome treats
/// `127.0.0.1` as a trustworthy origin and is expected to accept it — but that is
/// an assumption about a substrate free to change, which is why it is probed and
/// not believed.
pub async fn sign_in_through_browser(
    client: &fantoccini::Client,
    harness: &InProcHarness,
    email: &str,
    password: &str,
) {
    let base = harness.base_url();
    client
        .goto(&format!("{base}/sign-in"))
        .await
        .expect("navigate to /sign-in");
    client
        .find(Locator::Css("input[name='email']"))
        .await
        .expect("sign-in form must carry an email field")
        .send_keys(email)
        .await
        .expect("type the email");
    let password_field = client
        .find(Locator::Css("input[name='password']"))
        .await
        .expect("sign-in form must carry a password field");
    password_field
        .send_keys(password)
        .await
        .expect("type the password");
    password_field
        .find(Locator::XPath("ancestor::form"))
        .await
        .expect("password field must live inside the sign-in form")
        .find(Locator::Css("button[type='submit']"))
        .await
        .expect("sign-in form must carry a submit button")
        .click()
        .await
        .expect("submit the sign-in form");
}

/// Translate a shortcut's HUMAN name — the name the help overlay advertises and
/// the feature file writes — into the character WebDriver must send.
///
/// The named keys live in the WebDriver spec's Unicode private-use block; the
/// printable ones are themselves. Without this, `send_keys("Esc")` types the
/// three characters `E`, `s`, `c` and the scenario asserts nothing about the
/// Escape key at all — a green test over an unbound shortcut, which is the
/// exact failure mode this whole feature exists to close.
pub fn key_chord(key: &str) -> &str {
    match key {
        "Esc" | "Escape" => "\u{E00C}",
        "Enter" => "\u{E007}",
        "Tab" => "\u{E004}",
        "Space" => "\u{E00D}",
        other => other,
    }
}

/// Dispatch `key` as ONE real keystroke to WHATEVER currently holds focus — the
/// W3C Actions API, the same path a human's keypress takes.
///
/// This exists because element `send_keys` on `<body>` CANNOT deliver a key to
/// a focused control: WebDriver's Element Send Keys runs the focusing steps on
/// the target element first, and focusing `<body>` silently BLURS whatever held
/// focus, so the key lands on the body every time. Proven against Chrome 151
/// with a focused `<button>`: send-keys-to-body yields `Enter@BODY` and no
/// activation; a key action yields `Enter@BUTTON`, the native click fires, and
/// focus is untouched. When nothing is focused, `document.activeElement` IS the
/// body, so this dispatch is identical to the old one — which is what keeps
/// every body-targeted shortcut scenario meaning exactly what it always meant.
pub async fn press_key(client: &fantoccini::Client, key: &str) {
    use fantoccini::actions::{InputSource, KeyAction, KeyActions};
    let mut sequence = KeyActions::new("keyboard".to_string());
    for value in key_chord(key).chars() {
        sequence = sequence
            .then(KeyAction::Down { value })
            .then(KeyAction::Up { value });
    }
    client
        .perform_actions(sequence)
        .await
        .unwrap_or_else(|err| panic!("press {key:?}: {err}"));
}

/// Starts counting `focus()` calls the PAGE makes on `selector`'s element, from
/// now on. Answers "did the client layer grab focus AGAIN?" — a question with no
/// natural observable, because `.focus()` on an already-focused element fires no
/// `focus` event and moves no caret. Without this probe the only honest wording
/// of AC-04.5's second arm would be "focus is still on the box", which is true
/// whether or not the handler ran, and would therefore assert nothing.
///
/// It counts, it does not intercept: the native `focus` still runs, so a page
/// under probe behaves exactly as it does un-probed. Install it AFTER the focus
/// that is legitimate (the `/` that opened the panel), so the count starts at
/// zero and any increment means "again".
pub async fn probe_focus_grabs(client: &fantoccini::Client, selector: &str) {
    client
        .execute(
            "var el = document.querySelector(arguments[0]);
             if (!el) { throw new Error('nothing to probe at ' + arguments[0]); }
             var native = el.focus.bind(el);
             el.__kbFocusGrabs = 0;
             // An OWN property shadowing the prototype method. keyboard.js
             // re-queries the DOM for this same node, so the shadow persists.
             el.focus = function () {
               el.__kbFocusGrabs += 1;
               return native();
             };
             window.__kbFocusProbe = el;
             return true;",
            vec![serde_json::Value::String(selector.to_string())],
        )
        .await
        .expect("install the focus-grab probe");
}

/// How many times the page has called `focus()` on the probed element since
/// [`probe_focus_grabs`] was installed.
pub async fn focus_grabs(client: &fantoccini::Client) -> u64 {
    let count = client
        .execute(
            "var el = window.__kbFocusProbe;
             if (!el) { throw new Error('no focus probe was installed'); }
             return el.__kbFocusGrabs;",
            Vec::new(),
        )
        .await
        .expect("read the focus-grab count");
    count
        .as_u64()
        .expect("the focus-grab probe returns a count")
}

/// Bounded wait on the ADR-001 `[data-kb-ready]` marker. The condition that says
/// "the keyboard layer initialised" — pressed keys before this are a race.
pub async fn wait_for_kb_ready(client: &fantoccini::Client) {
    client
        .wait()
        .at_most(READY_TIMEOUT)
        .for_element(Locator::Css(KB_READY_SELECTOR))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "the keyboard layer never reported ready ({KB_READY_SELECTOR} did not appear \
                 within {READY_TIMEOUT:?}): {err}\n  keyboard.js sets \
                 document.documentElement.dataset.kbReady at init — either it is not loaded from \
                 base.html, or it threw before init completed."
            )
        });
}

#[cfg(test)]
mod tests {
    //! Reaping test for the lane's chromedriver lifecycle.
    //!
    //! Drives the SHIPPED entry point against the SHIPPED static, parking a
    //! `sleep` child in it rather than a real chromedriver. The behaviour under
    //! test is process lifecycle — kill the parked child AND wait for it — which
    //! has nothing to do with WebDriver, and requiring chromedriver here would
    //! put a host prerequisite on the DEFAULT `cargo test -p foundry-acceptance`
    //! lane, which is exactly what `@needs-browser` exists to keep out of it.
    //!
    //! This is the only test in the binary that touches `CHROMEDRIVER_PROC`, and
    //! the lib test binary never starts a real driver, so parking a stand-in
    //! there races with nothing.
    //!
    //! Behaviour budget: 1 behaviour (reap the parked driver) x 2 = 2 permitted;
    //! 1 authored, covering the empty slot, the parked child and the repeat call
    //! as states of that one behaviour.

    use super::*;

    /// True while the OS still has an entry for `pid`. `kill -0` succeeds on a
    /// ZOMBIE too, which is precisely why it is the right oracle here: killing
    /// without waiting leaves a zombie and this still reports the pid alive, so
    /// the assertion below cannot be satisfied by a `kill` with no `wait`.
    fn pid_exists(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[test]
    fn shutdown_kills_and_waits_for_the_parked_driver() {
        // A lane that filtered `@needs-browser` out still calls this. Silence,
        // not a panic on an empty slot.
        assert!(
            shutdown_chromedriver().is_none(),
            "a lane that started no driver has nothing to reap"
        );

        let child = Command::new("sleep")
            .arg("600")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn a long-lived stand-in for chromedriver");
        let pid = child.id();
        *CHROMEDRIVER_PROC.lock().expect("chromedriver proc lock") = Some(child);

        assert!(
            pid_exists(pid),
            "precondition: the parked child must be running before teardown, or \
             this test proves nothing"
        );

        assert!(
            shutdown_chromedriver().is_some(),
            "reaping a parked driver reports the status it waited for"
        );
        assert!(
            CHROMEDRIVER_PROC
                .lock()
                .expect("chromedriver proc lock")
                .is_none(),
            "the slot must be EMPTIED, so a second teardown cannot wait on a pid \
             this process has already reaped"
        );
        assert!(
            !pid_exists(pid),
            "the child must be killed AND waited for. Before 2026-08-30 nothing \
             did either — `Child` does not kill on drop and a `static`'s `Drop` \
             never runs at exit, so the driver was reparented to init and \
             survived the run. A `kill` with no `wait` would trade that orphan \
             for a zombie, which `kill -0` still finds, which is what makes this \
             assertion the one that separates reaping from signalling"
        );

        assert!(
            shutdown_chromedriver().is_none(),
            "reaping an empty slot is a no-op: teardown may run twice"
        );
    }
}
