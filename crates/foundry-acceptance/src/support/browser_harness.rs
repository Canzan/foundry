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
//! WAITS ARE CONDITIONS, NEVER SLEEPS — AND THE CONDITION MUST MEAN WHAT THE
//! CALLER NEEDS. Two families of bounded wait live here:
//!
//!   * `wait_for_kb_ready` — the `[data-kb-ready]` marker (ADR-001). It proves
//!     THE KEYBOARD LAYER IS LIVE, and nothing else. `keyboard.js` loads from
//!     `base.html`, which every template extends, so this marker is true on the
//!     sign-in page too. Use it before pressing a key; never as a stand-in for
//!     "we are on page X".
//!   * `wait_for_page` / `wait_for_board_ready` — a marker that identifies ONE
//!     page, panicking through `describe_wrong_page` with the URL, the document
//!     title and whether a sign-in form is showing.
//!
//! The split is the fix for a real defect (2026-08-30). Step definitions had been
//! using `wait_for_kb_ready` as a "the board has loaded" precondition. Because it
//! passes everywhere, a sign-in whose trailing redirect clobbered the board
//! navigation surfaced pages later as "the board must render the GEN-1 card" — a
//! message naming a cause nobody had measured. Every failure mode told the same
//! wrong story, so the browser lane was written off as an environmental flake for
//! two features running. A test that cannot say why it failed costs more than it
//! saves.
//!
//! The one `sleep` in this file is in `wait_for_driver_ready`, which polls
//! chromedriver's OWN `/status` endpoint — that is a poll interval on an explicit
//! readiness condition for an external PROCESS, not a timing assumption about the
//! app under test.
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

/// Bound on the DRIVER's boot, which is a different order of magnitude from the
/// in-page conditions [`READY_TIMEOUT`] covers and must not share its budget.
///
/// 10s was right when this lane ran a bare `chromedriver` binary on the host: it
/// listens in milliseconds. The lane now boots `selenium/standalone-chrome`, a
/// container starting a JVM and a browser — measured at 8s on an idle machine
/// here, i.e. inside the old budget with no margin at all, and over it the moment
/// the suite's own startup competes for the box. That is not a slow machine; it
/// is a constant that was not revisited when the thing it bounds changed.
///
/// Generous on purpose: this costs nothing when the driver is quick (the poll
/// returns the instant `/status` answers) and the alternative is the cascade
/// described on `CHROMEDRIVER`.
const DRIVER_READY_TIMEOUT: Duration = Duration::from_secs(90);

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
/// Name of this lane's browser container, so teardown can `docker rm -f` it.
/// Killing the `docker run` client alone does NOT stop the container.
static BROWSER_CONTAINER: Mutex<Option<String>> = Mutex::new(None);

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
    let status = child.wait().ok();
    // `--rm` only fires when the container STOPS, and killing the `docker run`
    // client does not stop it. Remove it by name, or every interrupted run
    // leaves a 2GB-shm Chrome container behind.
    if let Some(name) = BROWSER_CONTAINER
        .lock()
        .expect("browser container lock")
        .take()
    {
        let _ = Command::new("docker")
            .args(["rm", "-f", &name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    status
}

/// Ask the OS for a free port, then release it. chromedriver has no
/// "bind :0 and tell me the port" mode, so this bind-and-drop is the shipped
/// idiom for handing an external process an ephemeral port.
/// The host port Docker published for the container's 4444, read back FROM DOCKER.
///
/// Replaces a bind-then-release `free_port()` helper, which was a race with a
/// visible failure. It bound `127.0.0.1:0`, took the port the OS offered, dropped
/// the listener, and only then passed that number to `docker run -p PORT:4444`.
/// Between the drop and the publish the port is free, and this suite asks Docker
/// for other ephemeral ports constantly (every Postgres testcontainer). When the
/// two collided, a `sqlx` pool aimed at "its" Postgres reached the BROWSER
/// container instead and died on `unexpected response from SSLRequest: 0x48`.
///
/// `0x48` is `H` — the first byte of Selenium's `HTTP/1.1` reply. The failure
/// surfaced in `foundry-services`, a crate with no idea a browser exists.
///
/// Letting Docker allocate and then asking it what it chose removes the window
/// rather than narrowing it: the port is never unclaimed between decision and use.
fn published_port(name: &str) -> u16 {
    let deadline = std::time::Instant::now() + DRIVER_READY_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if let Ok(out) = Command::new("docker").args(["port", name, "4444"]).output() {
            // `docker port` prints one line per published binding, e.g.
            // `0.0.0.0:53569` and possibly a second `[::]:53569`. Any of them
            // names the same host port; take the first that parses.
            if let Some(port) = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|line| line.rsplit(':').next())
                .filter_map(|port| port.trim().parse::<u16>().ok())
                .find(|port| *port != 0)
            {
                return port;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "the browser container `{name}` never reported a published port for 4444 within \
         {DRIVER_READY_TIMEOUT:?} — `docker ps -a` and `docker logs {name}` will say why it did \
         not start"
    );
}

/// The browser image. It bundles a chromedriver and a Chrome of the SAME
/// build, so the version skew this lane used to preflight for cannot happen:
/// there is no host chromedriver to drift from a host Chrome. Same reasoning
/// as pinning the Postgres client image to the server's tag.
const BROWSER_IMAGE: &str = "selenium/standalone-chrome:latest";

/// Concurrent browser sessions the node will serve. Derived from
/// `MAX_CONCURRENT_SCENARIOS` so the two cannot drift; the headroom absorbs a
/// scenario whose session has not been dropped yet when the next one asks.
const BROWSER_MAX_SESSIONS: usize = crate::support::MAX_CONCURRENT_SCENARIOS + 2;

/// Chrome inside the container must reach the app, which listens on the HOST.
/// `TestApp::spawn_app` binds `0.0.0.0` and reports `127.0.0.1:<port>`, and
/// this rule makes the container's Chrome resolve that same literal to the
/// host gateway — so all 216 `base_url()` callers and 46 navigations keep
/// working unchanged. Rewriting them instead would have been a large edit to
/// a lane that currently passes 100%.
pub(crate) const HOST_RESOLVER_RULE: &str =
    "--host-resolver-rules=MAP 127.0.0.1 host.docker.internal";

/// Where the browser container's `host.docker.internal` points: Docker's
/// `host-gateway` (the default), the machine the suite runs on. When the suite
/// itself runs in a Linux container on the Docker host's network (`--network
/// host`, the route around a macOS host that cannot execute freshly built
/// binaries), the app listens inside the Docker VM, and the bridge gateway
/// (`FOUNDRY_BROWSER_HOST_GATEWAY=172.17.0.1`) is what reaches it. Unset, the
/// lane behaves exactly as before.
fn browser_host_gateway() -> String {
    std::env::var("FOUNDRY_BROWSER_HOST_GATEWAY").unwrap_or_else(|_| "host-gateway".to_string())
}

/// Where this process reaches the browser container's published WebDriver
/// port: `127.0.0.1` (the default). From inside a container on Docker
/// Desktop's host network a published port is reached through the bridge
/// gateway instead (`FOUNDRY_BROWSER_DRIVER_HOST=172.17.0.1`), the same address
/// testcontainers picks for Postgres when it finds itself in a container.
fn driver_host() -> String {
    std::env::var("FOUNDRY_BROWSER_DRIVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
}

/// Start the browser container once for this lane and return the host port its
/// WebDriver endpoint is published on.
///
/// PANICS with an actionable message when Docker is unreachable. This is the
/// refusal half of "probe, then refuse" (ADR-007 §4), unchanged in spirit: a
/// browser lane that skips is indistinguishable from the bug it exists to
/// prevent. Only the prerequisite moved — from "a chromedriver whose major
/// matches your Chrome" to "the Docker daemon this suite already requires".
/// Distinguishes the container of one `ensure_chromedriver` attempt from the next;
/// see the naming comment inside it.
static BROWSER_CONTAINER_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn ensure_chromedriver() -> u16 {
    *CHROMEDRIVER.get_or_init(|| {
        let daemon = Command::new("docker")
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !matches!(daemon, Ok(st) if st.success()) {
            panic!(
                "the @needs-browser lane drives Chrome from the `{BROWSER_IMAGE}` container, \
                 so a reachable Docker daemon is required. Start Colima/OrbStack/Docker \
                 Desktop. Neither chromedriver nor Chrome is needed on the host — that is \
                 the point: the image bundles a MATCHED pair, so the version skew this lane \
                 used to refuse on cannot occur."
            );
        }
        // Unique per ATTEMPT, not merely per process. A panic in `wait_for_driver_ready`
        // leaves the `OnceCell` empty, so the next scenario runs this block again — and
        // with a fixed per-PID name every one of those `docker run --name` calls collided
        // with the container the first attempt had already created. The retries could
        // therefore never succeed, which is how one slow boot became a whole red lane.
        let name = format!(
            "foundry-acceptance-chrome-{}-{}",
            std::process::id(),
            BROWSER_CONTAINER_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        );
        let child = Command::new("docker")
            .args(["run", "--rm", "--name", &name])
            // Docker picks the host port; `published_port` reads it back. See there
            // for why we no longer pick one ourselves.
            .args(["-p", "4444"])
            // Reach the host's ephemeral app ports from inside the container.
            .arg(format!(
                "--add-host=host.docker.internal:{}",
                browser_host_gateway()
            ))
            // Chrome will exhaust the default 64MB /dev/shm and crash tabs
            // mid-scenario; the Selenium images document this as required.
            .arg("--shm-size=2g")
            // Match the node's session capacity to the lane's concurrency.
            // A Selenium node offers ONE slot by default and clamps concurrent
            // sessions of the SAME browser to that, so the lane's six scenarios
            // queued behind a single slot and the router failed them with
            // "New session request timed out" — a limit invisible in the old
            // design, where one host `chromedriver` served every session. The
            // OVERRIDE flag is not optional: without it the node silently
            // re-clamps to its own computed maximum and MAX_SESSIONS is ignored.
            .args([
                "-e",
                &format!("SE_NODE_MAX_SESSIONS={BROWSER_MAX_SESSIONS}"),
            ])
            .args(["-e", "SE_NODE_OVERRIDE_MAX_SESSIONS=true"])
            // Queue a little longer than a cold Chrome start, so a brief burst
            // waits instead of failing.
            .args(["-e", "SE_SESSION_REQUEST_TIMEOUT=120"])
            .arg(BROWSER_IMAGE)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|err| {
                panic!("could not start the `{BROWSER_IMAGE}` browser container: {err}")
            });
        *CHROMEDRIVER_PROC.lock().expect("chromedriver proc lock") = Some(child);
        *BROWSER_CONTAINER.lock().expect("browser container lock") = Some(name.clone());
        let port = published_port(&name);
        wait_for_driver_ready(port);
        port
    })
}

/// Poll chromedriver's own `/status` until it reports ready. A condition on an
/// external process's readiness endpoint — not a sleep-and-hope.
fn wait_for_driver_ready(port: u16) {
    let deadline = std::time::Instant::now() + DRIVER_READY_TIMEOUT;
    let url = format!("http://{}:{port}/status", driver_host());
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
    panic!(
        "the browser container did not report ready on {}:{port} within \
         {DRIVER_READY_TIMEOUT:?}.\n  NOTE: this failure repeats PER SCENARIO — a panic inside \
         `OnceCell::get_or_init` leaves the cell empty, so every later scenario starts its own \
         container and fails the same way. A lane reporting N of these is reporting ONE problem \
         N times, not N problems.\n  `docker logs` the container, or run the image by hand, \
         before believing the app is involved.",
        driver_host()
    );
}

/// Open a WebDriver session against this lane's driver, retrying the OPEN.
///
/// ONE writer for both session shapes (desktop and mobile-emulation): they had
/// the same unguarded `.connect()` and the same panic text, so a fix applied to
/// one silently left the other alone.
///
/// The retry is on the OPEN and nothing else. Chrome is a process launch, and
/// the lane starts up to `max_concurrent_scenarios` of them at once: a Chrome
/// that loses that race dies before writing its DevToolsActivePort file, and the
/// driver reports `session not created: DevToolsActivePort file doesn't exist` —
/// a startup failure that says nothing about foundry, and that the next attempt
/// a moment later does not hit. This weakens no assertion: a scenario still gets
/// a real browser or the lane still fails, but it now fails for a reason that is
/// actually about the app.
///
/// A version-skewed driver fails IDENTICALLY on every attempt, so the retry costs
/// a bounded pause and the panic still names that cause — and, unlike the text it
/// replaces, tells the two apart instead of blaming skew for both.
async fn connect_session(
    port: u16,
    capabilities: serde_json::Map<String, serde_json::Value>,
    what: &str,
) -> fantoccini::Client {
    const SESSION_ATTEMPTS: u32 = 3;
    let mut last_err = None;
    for attempt in 1..=SESSION_ATTEMPTS {
        match ClientBuilder::new(HttpConnector::new())
            .capabilities(capabilities.clone())
            .connect(&format!("http://{}:{port}", driver_host()))
            .await
        {
            Ok(client) => return client,
            Err(err) => {
                last_err = Some(err);
                if attempt < SESSION_ATTEMPTS {
                    tokio::time::sleep(Duration::from_millis(500 * u64::from(attempt))).await;
                }
            }
        }
    }
    let err = last_err.expect("a failed open records its error");
    panic!(
        "could not open a {what} session after {SESSION_ATTEMPTS} attempts: {err}\n  `session \
         not created: DevToolsActivePort file doesn\'t exist` means Chrome itself failed to \
         launch — usually too many concurrent launches, or too little memory; lower \
         `max_concurrent_scenarios` or tag the scenario `@serial`.\n  Any other error is usually \
         version skew: the driver\'s MAJOR version must MATCH the installed Chrome\'s — \
         `chromedriver --version` vs `google-chrome --version`. `cargo xtask ci` preflights this; \
         a `brew upgrade` that moves one and not the other is the usual cause."
    )
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
        .post(format!(
            "http://{}:{port}/session/{session_id}/log",
            driver_host()
        ))
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
            // Chrome runs in a container; the app listens on the host. Without
            // this every navigation to 127.0.0.1 would hit the container.
            HOST_RESOLVER_RULE,
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
    // REQUIRED once the driver is the Selenium standalone router rather than a
    // bare chromedriver: chromedriver defaults to Chrome when `browserName` is
    // absent, the router does not — it has to pick a node, and an absent
    // `browserName` gives it nothing to match, so session creation fails with a
    // bare "session not created". Bisected against a live container.
    capabilities.insert(
        "browserName".to_string(),
        serde_json::Value::String("chrome".to_string()),
    );
    capabilities.insert("goog:chromeOptions".to_string(), chrome_options);
    connect_session(port, capabilities, "mobile chromedriver").await
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
        // Chrome runs in a container; the app listens on the host. Without
        // this every navigation to 127.0.0.1 would hit the container itself.
        HOST_RESOLVER_RULE.to_string(),
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
    // REQUIRED once the driver is the Selenium standalone router rather than a
    // bare chromedriver: chromedriver defaults to Chrome when `browserName` is
    // absent, the router does not — it has to pick a node, and an absent
    // `browserName` gives it nothing to match, so session creation fails with a
    // bare "session not created". Bisected against a live container.
    capabilities.insert(
        "browserName".to_string(),
        serde_json::Value::String("chrome".to_string()),
    );
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
    let client = connect_session(port, capabilities, "chromedriver").await;
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
    // WAIT FOR THE SIGN-IN TO LAND — do not assume the click completed it.
    //
    // `submit_signin` answers 303 → `/`, and `dashboard_root` renders the
    // authenticated shell. WebDriver's Element Click does NOT reliably block
    // until that chain finishes: chromedriver returns once the click is
    // dispatched and its navigation tracking has expired, so this function used
    // to return with the POST still in flight. Every caller then immediately
    // `goto`s a board — and the sign-in's own trailing redirect to `/` LANDS ON
    // TOP of that board navigation, leaving the browser parked on the dashboard.
    //
    // Measured 2026-08-30 on `us-close-modal` with a page-specific oracle in
    // place: 6 of 10 scenarios stranded at `http://127.0.0.1:PORT/`, title
    // "Foundry", sign-in form ABSENT. The session was authenticated the whole
    // time — the failure was never a bounce to /sign-in, it was this race, which
    // reported itself as "the board must render the GEN-1 card" because the
    // readiness wait in between (`[data-kb-ready]`) is true on every page.
    //
    // Waiting for `.app-shell` is the honest post-condition: only templates that
    // require a session extend `app_shell.html` (`signin.html` extends
    // `base.html` directly), so it appears exactly when the redirect chain has
    // finished AND the app accepted the credentials. A refused sign-in re-renders
    // the form, this wait expires, and `describe_wrong_page` says so outright
    // instead of leaving a later step to misreport it.
    wait_for_page(
        client,
        "the signed-in application shell",
        Locator::Css(APP_SHELL_SELECTOR),
    )
    .await;
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

/// Where the browser ACTUALLY was when a page-specific wait expired — the three
/// facts that separate "the page loaded but the feature is broken" from "we are
/// not on that page at all".
#[derive(Debug, Clone, PartialEq, Eq)]
struct WrongPage {
    url: String,
    title: String,
    shows_sign_in_form: bool,
}

/// The panic text a page-specific wait carries. Kept a PURE function so the
/// wording is under test without a browser: an oracle that cannot say WHY it
/// failed is what let `[data-kb-ready]` mask a redirect-to-sign-in for two
/// features running, and a wording regression would silently restore that.
fn describe_wrong_page(
    page_name: &str,
    selector: &str,
    timeout: Duration,
    actual: &WrongPage,
) -> String {
    let cause = if actual.shows_sign_in_form {
        "the page IS the sign-in form, so this session was NOT authenticated when the navigation \
         ran and the app bounced it to /sign-in. Suspect the sign-in helper returning before the \
         session cookie was set — not the page under test."
    } else {
        "no sign-in form is present, so the session WAS authenticated and the browser is on some \
         other authenticated page than the expected one. Suspect the URL the step built, or a \
         render that failed after auth."
    };
    format!(
        "{page_name} never rendered: {selector} did not appear within {timeout:?}.\n  \
         actually at url: {url}\n  \
         actually showing title: {title}\n  \
         sign-in form present: {shows_sign_in_form}\n  \
         {cause}",
        url = actual.url,
        title = actual.title,
        shows_sign_in_form = actual.shows_sign_in_form,
    )
}

/// Read back the three facts [`describe_wrong_page`] reports. Best-effort by
/// design: this only ever runs on a path that is already failing, so a WebDriver
/// error here must not replace the diagnostic with a second, worse one.
async fn capture_wrong_page(client: &fantoccini::Client) -> WrongPage {
    WrongPage {
        url: client
            .current_url()
            .await
            .map(|u| u.to_string())
            .unwrap_or_else(|err| format!("<could not read the current url: {err}>")),
        title: client
            .title()
            .await
            .unwrap_or_else(|err| format!("<could not read the document title: {err}>")),
        // The sign-in form, not merely the path: a URL can be rewritten, and the
        // rendered form is what actually proves the app refused the session.
        shows_sign_in_form: client
            .find(Locator::Css(SIGN_IN_FORM_SELECTOR))
            .await
            .is_ok(),
    }
}

/// The shipped sign-in form (`signin.html:6`). The one DOM fact that separates
/// "this session was never authenticated" from every other failure.
const SIGN_IN_FORM_SELECTOR: &str = "form[action='/sign-in'] input[name='password']";

/// The BOARD's own render-contract container: `board.html:11` emits
/// `<div class="board" id="board-columns">`.
///
/// WHY NOT `[data-kb-ready]`, which the step definitions used as a "the board
/// loaded" precondition until 2026-08-30: `keyboard.js` is loaded from
/// `base.html:51`, which EVERY template extends — sign-in, forgot-password,
/// invite-accept, the error pages — and it sets `dataset.kbReady`
/// unconditionally at init. The marker is therefore true on every page in the
/// app. Its ADR-001 contract, "the keyboard layer is live", is exactly right and
/// stays; it simply never said anything about WHICH page, so a navigation that
/// landed on the sign-in form satisfied it and the failure surfaced pages later
/// as "the board must render the GEN-1 card".
///
/// WHY NOT `[data-column]`: a column is a LANE, emitted once per row of
/// `columns` by `partials/board_columns.html`. board-lane-management ships lane
/// deletion, so a board can legitimately render zero lanes — `[data-column]`
/// would then time out on a board that loaded perfectly. `#board-columns` is
/// emitted unconditionally by `board.html` and by no other template in the
/// repository, so it is present exactly when the browser is on a board.
pub const BOARD_READY_SELECTOR: &str = ".board#board-columns";

/// The signed-in application shell (`app_shell.html:3`). Only templates that
/// require a session extend it — `signin.html` extends `base.html` directly — so
/// its presence is the app's own answer to "is this browser authenticated?".
pub const APP_SHELL_SELECTOR: &str = ".app-shell";

/// Bounded wait for a marker that identifies ONE page, panicking with
/// [`describe_wrong_page`] — which names the URL, the title and whether a
/// sign-in form is showing — when it never appears.
///
/// The diagnostic is the point. A wait whose failure message asserts a cause it
/// never measured costs more than it saves.
///
/// Takes a `Locator` rather than a CSS string because not every page HAS a
/// structural marker of its own: `bootstrap_dashboard.html` extends `base.html`
/// with nothing but an `<h1>`, so its only honest marker is that heading's TEXT,
/// which is XPath's job and CSS cannot express.
pub async fn wait_for_page(client: &fantoccini::Client, page_name: &str, marker: Locator<'_>) {
    if client
        .wait()
        .at_most(READY_TIMEOUT)
        .for_element(marker)
        .await
        .is_ok()
    {
        return;
    }
    let described = match marker {
        Locator::Css(selector) => format!("css {selector}"),
        Locator::Id(id) => format!("id {id}"),
        Locator::LinkText(text) => format!("link text {text:?}"),
        Locator::XPath(path) => format!("xpath {path}"),
    };
    let actual = capture_wrong_page(client).await;
    panic!(
        "{}",
        describe_wrong_page(page_name, &described, READY_TIMEOUT, &actual)
    );
}

/// "We are on a project board, and it has rendered." The precondition every step
/// that then looks for a card actually needs. See [`BOARD_READY_SELECTOR`].
pub async fn wait_for_board_ready(client: &fantoccini::Client) {
    wait_for_page(
        client,
        "the project board",
        Locator::Css(BOARD_READY_SELECTOR),
    )
    .await;
}

/// Bounded wait on the ADR-001 `[data-kb-ready]` marker. The condition that says
/// "the keyboard layer initialised" — pressed keys before this are a race.
///
/// NOTE what this does and does not prove. It proves the keyboard layer is live,
/// which is its ADR-001 contract and is true on EVERY page in the app, sign-in
/// included. It does NOT prove which page the browser is on. Steps that need
/// "the board has loaded" want [`wait_for_board_ready`]; steps that are about to
/// press a key want this.
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

// =========================================================================
// SYNTHETIC CARD DRAG (card-drag-drop-feedback DDD-8b/c/d/e)
// =========================================================================
//
// When this kit was written the card drag was native HTML5 DnD and it was
// driven by DISPATCH. Correction (card-pointer-drag DISTILL, measured on Chrome
// 151): W3C mouse actions on a `draggable` card DO start a real HTML5 drag
// (trusted dragstart, pointercancel, dragover, drop, dragend). The kit stays
// for FOREIGN drags; card drags move to the trusted pointer driver below. It is
// DISPATCHED: real `DragEvent`s carrying one real
// `DataTransfer`, on the real elements, into `board-dnd.js`'s own listeners.
// This generalises the `fire()` idiom at `keyboard_shortcut_bindings.rs:3028`
// and the script in `feature_board_lane_reorder.rs::drag_a_card` into ONE kit,
// so no third copy is written. Those two shipped steps are NOT changed (KPI 8).
//
// The kit follows the browser's protocol, which is what keeps it honest:
//   * every point is resolved from LIVE geometry at dispatch time, and the
//     event goes to the element actually under that point
//     (`elementFromPoint`), exactly as a real drag's would;
//   * `drop` is dispatched ONLY if the last `dragover` was `defaultPrevented`.
//     A real browser never fires `drop` on a target that did not claim the
//     drag, and a kit that did would let a lane with no listener "accept" a
//     drop it never claimed. That is precisely the RCA bug, so a kit that
//     skipped this rule could not see it;
//   * `Escape` and "release outside any lane" are `dragend` on the dragged card
//     with no `drop`, which is exactly what a real browser delivers (DDD-8c,
//     DDD-12: the page receives no key events during a native drag);
//   * a FOREIGN drag (a file, a text selection, a card from another tab) has no
//     `dragstart` on this page at all. It only swaps in a `DataTransfer` whose
//     payload a real one would carry.
//
// Every phase returns the event's `defaultPrevented`, the only observable a
// synthetic lane has for "the board claimed this drag".
//
// The kit lives on `window`. A board REPLACE (htmx OOB, `applyBoard`) keeps it;
// a RELOAD discards it, which is fine, because the next phase re-installs it.

/// Where in a lane, or on the page, a synthetic drag points. Resolved from live
/// geometry at dispatch time, never from coordinates a step invented.
#[derive(Debug, Clone, Copy)]
pub enum DragSpot<'a> {
    /// Inside the lane `data-column = slug`, just below its last card: the END
    /// slot. For an empty lane, its vertical middle.
    LaneEnd(&'a str),
    /// Inside the lane, just below the top edge of its first card, which is
    /// above that card's middle and therefore the TOP slot.
    LaneTop(&'a str),
    /// In the gap between two adjacent cards of one lane (their gap midpoint).
    Between(&'a str, &'a str),
    /// One pixel below the top edge of this card, above its middle.
    AboveMiddleOf(&'a str),
    /// Four pixels below this card's bottom edge, clamped inside its lane.
    Below(&'a str),
    /// The middle of this card: the pointer is over the card itself.
    OnCard(&'a str),
    /// The board page's own `<header>`, outside `#board-columns`.
    PageHeader,
    /// `#board-columns` itself, at the seam between two adjacent lanes: the
    /// board, but no lane.
    BoardGap(&'a str, &'a str),
    /// Exactly the point the previous `dragover` used: a still pointer.
    SamePoint,
}

impl DragSpot<'_> {
    fn to_json(self) -> serde_json::Value {
        use serde_json::json;
        match self {
            DragSpot::LaneEnd(lane) => json!({"kind": "laneEnd", "lane": lane}),
            DragSpot::LaneTop(lane) => json!({"kind": "laneTop", "lane": lane}),
            DragSpot::Between(above, below) => {
                json!({"kind": "between", "above": above, "below": below})
            }
            DragSpot::AboveMiddleOf(key) => json!({"kind": "aboveMiddle", "key": key}),
            DragSpot::Below(key) => json!({"kind": "below", "key": key}),
            DragSpot::OnCard(key) => json!({"kind": "onCard", "key": key}),
            DragSpot::PageHeader => json!({"kind": "header"}),
            DragSpot::BoardGap(left, right) => {
                json!({"kind": "gap", "left": left, "right": right})
            }
            DragSpot::SamePoint => json!({"kind": "same"}),
        }
    }
}

/// What a foreign drag carries. Neither has a `dragstart` on this page.
#[derive(Debug, Clone, Copy)]
pub enum ForeignPayload<'a> {
    /// A file from the desktop, carried as a real `File` in the transfer.
    File(&'a str),
    /// Plain text: a selection from another app, or a card dragged from another
    /// foundry tab (which carries its key as `text/plain`, exactly like ours).
    Text(&'a str),
}

const SYNTHETIC_DRAG_KIT: &str = r#"
var kit = window.__synthDrag;
if (!kit) {
  kit = window.__synthDrag = { transfer: null, lastOverPrevented: false, lastPoint: null };
  var need = function (el, what) {
    if (!el) { throw new Error('synthetic drag: ' + what + ' is not on the board'); }
    return el;
  };
  var board = function () { return need(document.getElementById('board-columns'), '#board-columns'); };
  var lane = function (slug) { return need(board().querySelector('[data-column="' + slug + '"]'), 'lane ' + slug); };
  var card = function (key) { return need(board().querySelector('[data-issue-key="' + key + '"]'), 'card ' + key); };
  var rect = function (el) { return el.getBoundingClientRect(); };
  var clampIn = function (y, r) { return Math.max(r.top + 1, Math.min(y, r.bottom - 2)); };
  var under = function (x, y, within, fallback) {
    var el = document.elementFromPoint(x, y);
    return el && within.contains(el) ? el : fallback;
  };
  var laneOf = function (key) { return need(card(key).closest('[data-column]'), 'the lane of ' + key); };
  kit.resolve = function (spot) {
    var l, r, x, y, cards, a, b;
    switch (spot.kind) {
      case 'laneEnd':
        l = lane(spot.lane); r = rect(l); x = r.left + r.width / 2;
        cards = l.querySelectorAll('.issue-card');
        y = cards.length ? clampIn(rect(cards[cards.length - 1]).bottom + 4, r) : r.top + r.height / 2;
        return { target: under(x, y, l, l), x: x, y: y };
      case 'laneTop':
        l = lane(spot.lane); r = rect(l); x = r.left + r.width / 2;
        cards = l.querySelectorAll('.issue-card');
        y = cards.length ? rect(cards[0]).top + 1 : r.top + r.height / 2;
        return { target: under(x, y, l, l), x: x, y: y };
      case 'between':
        l = laneOf(spot.above); a = rect(card(spot.above)); b = rect(card(spot.below));
        r = rect(l); x = r.left + r.width / 2; y = (a.bottom + b.top) / 2;
        return { target: under(x, y, l, l), x: x, y: y };
      case 'aboveMiddle':
        l = laneOf(spot.key); a = rect(card(spot.key)); r = rect(l);
        x = r.left + r.width / 2; y = a.top + 1;
        return { target: under(x, y, l, l), x: x, y: y };
      case 'below':
        l = laneOf(spot.key); a = rect(card(spot.key)); r = rect(l);
        x = r.left + r.width / 2; y = clampIn(a.bottom + 4, r);
        return { target: under(x, y, l, l), x: x, y: y };
      case 'onCard':
        a = card(spot.key); r = rect(a);
        return { target: a, x: r.left + r.width / 2, y: r.top + r.height / 2 };
      case 'header':
        a = need(document.querySelector('.app-shell__content > header') || document.querySelector('header'), 'the page header');
        r = rect(a);
        return { target: a, x: r.left + r.width / 2, y: r.top + r.height / 2 };
      case 'gap':
        a = rect(lane(spot.left)); b = rect(lane(spot.right));
        return { target: board(), x: (a.right + b.left) / 2, y: a.top + 24 };
      case 'same':
        need(kit.lastPoint, 'a previous drag point');
        return { target: document.elementFromPoint(kit.lastPoint.x, kit.lastPoint.y) || board(), x: kit.lastPoint.x, y: kit.lastPoint.y };
    }
    throw new Error('synthetic drag: unknown spot ' + JSON.stringify(spot));
  };
  kit.fire = function (target, type, x, y, extra) {
    var init = { bubbles: true, cancelable: true, composed: true, dataTransfer: kit.transfer, clientX: x, clientY: y };
    if (extra) { for (var k in extra) { init[k] = extra[k]; } }
    var event = new DragEvent(type, init);
    target.dispatchEvent(event);
    return event.defaultPrevented;
  };
  kit.start = function (a) {
    var c = card(a.key), r = rect(c);
    kit.transfer = new DataTransfer();
    kit.lastOverPrevented = false;
    return kit.fire(c, 'dragstart', r.left + r.width / 2, r.top + r.height / 2);
  };
  kit.foreign = function (a) {
    var dt = new DataTransfer();
    if (a.file) { dt.items.add(new File(['foundry acceptance'], a.file, { type: 'image/png' })); }
    else { dt.setData('text/plain', a.text); }
    // Chrome ignores `dropEffect` writes on a script-made DataTransfer (it is
    // not a real drag data store) and always reads back `none`, so give this
    // one a live, writable `dropEffect` as a real drag's store has. A browser
    // offers an outside drag as a copy before the page answers, so a `none`
    // read back after dragover can only be the board's own answer.
    var effect = 'copy';
    Object.defineProperty(dt, 'dropEffect', {
      configurable: true,
      get: function () { return effect; },
      set: function (v) { if (['none', 'copy', 'link', 'move'].indexOf(v) >= 0) { effect = v; } }
    });
    kit.transfer = dt;
    kit.lastOverPrevented = false;
    kit.lastOverEffect = null;
    return true;
  };
  kit.enter = function (a) { var p = kit.resolve(a.spot); return kit.fire(p.target, 'dragenter', p.x, p.y); };
  kit.over = function (a) {
    var p = kit.resolve(a.spot);
    kit.lastPoint = { x: p.x, y: p.y };
    kit.lastOverPrevented = kit.fire(p.target, 'dragover', p.x, p.y);
    kit.lastOverEffect = kit.transfer ? kit.transfer.dropEffect : null;
    return kit.lastOverPrevented;
  };
  kit.leave = function (a) {
    var p = kit.resolve(a.spot);
    var related = a.related ? kit.resolve(a.related).target : null;
    return kit.fire(p.target, 'dragleave', p.x, p.y, { relatedTarget: related });
  };
  kit.drop = function (a) {
    if (!kit.lastOverPrevented) { return false; }
    var p = kit.resolve(a.spot);
    return kit.fire(p.target, 'drop', p.x, p.y);
  };
  kit.end = function (a) {
    var c = document.querySelector('[data-issue-key="' + a.key + '"]');
    var prevented = false;
    if (c) { var r = rect(c); prevented = kit.fire(c, 'dragend', r.left + r.width / 2, r.top + r.height / 2); }
    kit.transfer = null;
    kit.lastOverPrevented = false;
    return prevented;
  };
}
"#;

async fn run_kit(client: &fantoccini::Client, phase: &str, arg: serde_json::Value) -> bool {
    let script = format!("{SYNTHETIC_DRAG_KIT}\nreturn window.__synthDrag.{phase}(arguments[0]);");
    client
        .execute(&script, vec![arg])
        .await
        .unwrap_or_else(|err| panic!("synthetic drag phase {phase:?} could not run: {err}"))
        .as_bool()
        .unwrap_or(false)
}

/// `dragstart` on the card `key`, with a fresh `DataTransfer` for this drag.
pub async fn drag_start(client: &fantoccini::Client, key: &str) -> bool {
    run_kit(client, "start", serde_json::json!({ "key": key })).await
}

/// Begin a FOREIGN drag: no `dragstart` on this page, only the transfer a real
/// one would carry. Follow with [`drag_over`] / [`drag_drop`].
pub async fn drag_start_foreign(client: &fantoccini::Client, payload: ForeignPayload<'_>) {
    let arg = match payload {
        ForeignPayload::File(name) => serde_json::json!({ "file": name }),
        ForeignPayload::Text(text) => serde_json::json!({ "text": text }),
    };
    run_kit(client, "foreign", arg).await;
}

/// Run the kit `phase` that points at `spot`.
async fn run_kit_at(client: &fantoccini::Client, phase: &str, spot: DragSpot<'_>) -> bool {
    run_kit(client, phase, serde_json::json!({ "spot": spot.to_json() })).await
}

/// `dragenter` at `spot`.
pub async fn drag_enter(client: &fantoccini::Client, spot: DragSpot<'_>) -> bool {
    run_kit_at(client, "enter", spot).await
}

/// `dragover` at `spot`; returns whether the board claimed it.
pub async fn drag_over(client: &fantoccini::Client, spot: DragSpot<'_>) -> bool {
    run_kit_at(client, "over", spot).await
}

/// The `dropEffect` the page left on the transfer during the last `dragover`,
/// or `None` if no `dragover` has run since the drag began. A foreign drag
/// starts at `copy`, so `none` here can only be the board's own answer.
pub async fn last_drop_effect(client: &fantoccini::Client) -> Option<String> {
    let script = format!("{SYNTHETIC_DRAG_KIT}\nreturn window.__synthDrag.lastOverEffect || null;");
    client
        .execute(&script, vec![])
        .await
        .unwrap_or_else(|err| panic!("reading the last dragover's dropEffect failed: {err}"))
        .as_str()
        .map(str::to_owned)
}

/// `dragleave` at `spot`, with `related` as the `relatedTarget` (`None` is the
/// pointer leaving the window).
pub async fn drag_leave(
    client: &fantoccini::Client,
    spot: DragSpot<'_>,
    related: Option<DragSpot<'_>>,
) -> bool {
    let related = related.map(DragSpot::to_json);
    run_kit(
        client,
        "leave",
        serde_json::json!({ "spot": spot.to_json(), "related": related }),
    )
    .await
}

/// `drop` at `spot`, dispatched ONLY if the last `dragover` was claimed, as a
/// real browser would. Returns `false` without dispatching otherwise.
pub async fn drag_drop(client: &fantoccini::Client, spot: DragSpot<'_>) -> bool {
    run_kit_at(client, "drop", spot).await
}

/// `dragend` on the dragged card `key` (a drop, `Escape`, or a release outside
/// any lane all end this way) and forget the transfer.
pub async fn drag_end(client: &fantoccini::Client, key: &str) -> bool {
    run_kit(client, "end", serde_json::json!({ "key": key })).await
}

// ---- The move-request spy and the no-reload mark (DDD-8d, DDD-8e) --------

/// Install a page-side `fetch` spy and the `window.__cdfMark` no-reload mark.
/// Idempotent within one document. WebDriver-executed scripts are not bound by
/// the page's CSP, so this needs no production seam. A reload discards BOTH,
/// which is exactly what makes the mark an oracle: [`assert_not_reloaded`]
/// fails if a step reloaded between the board replace and the drag (D2).
pub async fn install_drag_observers(client: &fantoccini::Client) -> String {
    client
        .execute(
            r#"if (!window.__cdfRequests) {
                 window.__cdfRequests = [];
                 var original = window.fetch;
                 window.fetch = function (input, init) {
                   var headers = (init && init.headers) || {};
                   var entry = {
                     url: typeof input === 'string' ? input : ((input && input.url) || ''),
                     method: (init && init.method) || 'GET',
                     body: init && typeof init.body === 'string' ? init.body : null,
                     csrf: headers['x-csrf-token'] || headers['X-CSRF-Token'] || '',
                     status: null
                   };
                   window.__cdfRequests.push(entry);
                   var pending = original.apply(this, arguments);
                   pending.then(function (r) { entry.status = r.status; },
                                function () { entry.status = 0; });
                   return pending;
                 };
               }
               if (!window.__cdfMark) { window.__cdfMark = 'cdf-' + Date.now() + '-' + Math.random(); }
               return window.__cdfMark;"#,
            vec![],
        )
        .await
        .expect("install the fetch spy and the no-reload mark")
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Panic unless the no-reload mark `mark` is still on this document, i.e. no
/// step navigated or reloaded since [`install_drag_observers`] returned it.
pub async fn assert_not_reloaded(client: &fantoccini::Client, mark: &str) {
    let now = client
        .execute("return window.__cdfMark || '';", vec![])
        .await
        .expect("read the no-reload mark")
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        now, mark,
        "the board was RELOADED between the in-place refresh and the drag (window.__cdfMark was \
         {mark:?}, is now {now:?}). A reload re-binds every listener and hides the bug this \
         scenario exists to catch (D2, DDD-8e); the step sequence is wrong, not the board"
    );
}

/// One request the page sent through `fetch`, as the spy recorded it.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SpiedRequest {
    pub url: String,
    pub method: String,
    pub body: Option<String>,
    pub csrf: String,
    /// `None` while in flight; `Some(0)` for a network error.
    pub status: Option<u16>,
}

/// Every request the spy has recorded on the current document.
pub async fn spied_requests(client: &fantoccini::Client) -> Vec<SpiedRequest> {
    let raw = client
        .execute("return window.__cdfRequests || [];", vec![])
        .await
        .expect("read the fetch spy");
    serde_json::from_value(raw).expect("the fetch spy's entries deserialize")
}

// =========================================================================
// TRUSTED POINTER INPUT (card-pointer-drag DDD-12)
// =========================================================================
//
// The card drag moves onto Pointer Events (ADR-BOARD-CARD-004), so it can be
// driven the way a person drives it: W3C WebDriver Actions, which chromedriver
// turns into TRUSTED browser input (`Input.dispatchMouseEvent` /
// `Input.dispatchTouchEvent`). That is stronger evidence than the synthetic
// `DragEvent` kit above could give, and the kit stays for FOREIGN drags only
// (a file, a text selection, another tab's card), which have no pointer on this
// page at all (D3, DDD-1).
//
// Three rules keep these helpers honest:
//   * THE RECORDER IS ARMED FIRST. Automation drags can deliver zero events,
//     and a drag that delivered nothing looks exactly like a feature that did
//     nothing. [`install_pointer_recorder`] counts every pointer, touch, click
//     and native-drag event the page receives (capture phase on `window`, so no
//     page listener can hide one), and every "nothing lifted" oracle reads it
//     first: trusted input must have ARRIVED before its absence of effect means
//     anything.
//   * COORDINATES COME FROM LIVE GEOMETRY. [`spot_point`] reuses the kit's
//     `DragSpot` resolver, so a pointer goes where the board actually is at that
//     moment, never where a step guessed it would be.
//   * ONE GESTURE MAY SPAN SEVERAL CALLS, so "press and lift" can be one
//     call, the oracle a read, and the release a later call. For the MOUSE and
//     the PEN that is W3C Actions: WebDriver keeps each input source's state
//     (button down, last position) between `perform_actions` calls. For TOUCH it
//     is NOT: measured against this lane's chromedriver 151 (card-pointer-drag
//     DISTILL, OQ-9 probe), a W3C touch source that is still down at the end of
//     one `perform_actions` call has every later move and release SILENTLY
//     DROPPED, and it then poisons the next touch call too. Touch therefore goes
//     through the very dispatch chromedriver uses underneath its touch actions,
//     Chrome's `Input.dispatchTouchEvent`, reached through the driver's CDP
//     passthrough (`/session/{id}/goog/cdp/execute`). The events are just as
//     trusted, a gesture spans calls, and a second finger is a second touch
//     point beside the first, as on a phone.

/// Which kind of pointer a gesture uses. Each maps to one W3C input source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Touch,
    /// A second finger, down while the first one is.
    SecondTouch,
    /// A third finger: a fresh contact for a new gesture while the first
    /// finger's bookkeeping is in any state.
    RetryTouch,
    Pen,
}

impl PointerKind {
    fn source(self) -> &'static str {
        match self {
            PointerKind::Mouse => "cpd-mouse",
            PointerKind::Touch => "cpd-touch",
            PointerKind::SecondTouch => "cpd-touch-2",
            PointerKind::RetryTouch => "cpd-touch-3",
            PointerKind::Pen => "cpd-pen",
        }
    }
}

/// One tick of a pointer gesture. Points are viewport CSS pixels, rounded and
/// clamped into the viewport when the actions are built.
#[derive(Debug, Clone, Copy)]
pub enum PointerStep {
    /// Jump to a point (one move event).
    To(f64, f64),
    /// Travel from the current point to this one in `n` equal moves.
    Glide(f64, f64, u32),
    /// Primary button / contact down.
    Down,
    /// Secondary (right) mouse button down.
    DownSecondary,
    /// Primary button / contact up.
    Up,
    /// Secondary (right) mouse button up.
    UpSecondary,
    /// Stay still for this many milliseconds (a W3C `pause`: the hold).
    Hold(u64),
    /// Small back-and-forth moves at the current point for about this many
    /// milliseconds: a finger or hand holding "still" at an edge, which real
    /// hardware reports as a trickle of 1 px moves.
    Jitter(u64),
}

/// How long one move tick is allowed to take. CDP touch dispatch is vsync-paced
/// at ~33 ms per event (spike Q7), so a longer gesture must budget for it.
const MOVE_TICK: Duration = Duration::from_millis(16);

/// Perform `steps` with the pointer `kind`, starting from `from` (where the
/// pointer is now, or where the first step puts it). Returns where the pointer
/// ended. Mouse and pen: one W3C `perform_actions` call for all the steps.
/// Touch: one CDP touch event per step (see the header of this section).
pub async fn perform_pointer(
    client: &fantoccini::Client,
    kind: PointerKind,
    from: (f64, f64),
    steps: &[PointerStep],
) -> (f64, f64) {
    let (vw, vh) = viewport(client).await;
    let clamp = |x: f64, y: f64| -> (f64, f64) {
        (
            x.round().clamp(0.0, (vw - 1.0).max(0.0)),
            y.round().clamp(0.0, (vh - 1.0).max(0.0)),
        )
    };
    // Expand into single ticks: a point to move to, a press, a release, a pause.
    let mut ticks: Vec<Tick> = Vec::new();
    let mut at = from;
    for step in steps {
        match *step {
            PointerStep::To(x, y) => {
                ticks.push(Tick::Move(clamp(x, y)));
                at = (x, y);
            }
            PointerStep::Glide(x, y, n) => {
                let n = n.max(1);
                for i in 1..=n {
                    let t = f64::from(i) / f64::from(n);
                    ticks.push(Tick::Move(clamp(
                        at.0 + (x - at.0) * t,
                        at.1 + (y - at.1) * t,
                    )));
                }
                at = (x, y);
            }
            PointerStep::Down => ticks.push(Tick::Down(false)),
            PointerStep::DownSecondary => ticks.push(Tick::Down(true)),
            PointerStep::Up => ticks.push(Tick::Up(false)),
            PointerStep::UpSecondary => ticks.push(Tick::Up(true)),
            PointerStep::Hold(ms) => ticks.push(Tick::Pause(ms)),
            PointerStep::Jitter(ms) => {
                let n = (ms / 40).max(1);
                for i in 0..n {
                    let dx = if i % 2 == 0 { -1.0 } else { 0.0 };
                    ticks.push(Tick::Move(clamp(at.0 + dx, at.1)));
                    ticks.push(Tick::Pause(24));
                }
                ticks.push(Tick::Move(clamp(at.0, at.1)));
            }
        }
    }
    match kind {
        PointerKind::Mouse | PointerKind::Pen => w3c_pointer(client, kind, &ticks).await,
        _ => cdp_touch(client, kind, &ticks).await,
    }
    at
}

/// One tick of an expanded gesture. `Down(true)` / `Up(true)` is the
/// secondary mouse button.
#[derive(Debug, Clone, Copy)]
enum Tick {
    Move((f64, f64)),
    Down(bool),
    Up(bool),
    Pause(u64),
}

async fn w3c_pointer(client: &fantoccini::Client, kind: PointerKind, ticks: &[Tick]) {
    use fantoccini::actions::{
        InputSource, MouseActions, PenActions, PointerAction, MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT,
    };
    let actions: Vec<PointerAction> = ticks
        .iter()
        .map(|tick| match *tick {
            Tick::Move((x, y)) => PointerAction::MoveTo {
                duration: Some(MOVE_TICK),
                x: x as i64,
                y: y as i64,
            },
            Tick::Down(secondary) => PointerAction::Down {
                button: if secondary {
                    MOUSE_BUTTON_RIGHT
                } else {
                    MOUSE_BUTTON_LEFT
                },
            },
            Tick::Up(secondary) => PointerAction::Up {
                button: if secondary {
                    MOUSE_BUTTON_RIGHT
                } else {
                    MOUSE_BUTTON_LEFT
                },
            },
            Tick::Pause(ms) => PointerAction::Pause {
                duration: Duration::from_millis(ms),
            },
        })
        .collect();
    let source = kind.source().to_string();
    let result = if kind == PointerKind::Pen {
        let seq = actions
            .into_iter()
            .fold(PenActions::new(source), InputSource::then);
        client.perform_actions(seq).await
    } else {
        let seq = actions
            .into_iter()
            .fold(MouseActions::new(source), InputSource::then);
        client.perform_actions(seq).await
    };
    result.unwrap_or_else(|err| {
        panic!(
            "BROKEN(driver): the {kind:?} pointer actions were refused by the browser driver: \
             {err}. This is the harness, not the board: W3C Actions must reach the page before \
             any card-drag oracle means anything (DDD-12)"
        )
    });
}

/// Touch points currently down, per WebDriver session: CDP touch id -> point.
/// CDP needs every touch event to carry the full set of points still down.
type TouchSet = std::collections::BTreeMap<u32, (f64, f64)>;
static TOUCH_POINTS: Mutex<Option<std::collections::HashMap<String, TouchSet>>> = Mutex::new(None);

fn touch_id(kind: PointerKind) -> u32 {
    match kind {
        PointerKind::SecondTouch => 2,
        PointerKind::RetryTouch => 3,
        _ => 1,
    }
}

async fn session_of(client: &fantoccini::Client) -> String {
    client
        .session_id()
        .await
        .expect("read the WebDriver session id")
        .expect("an open WebDriver session")
}

/// One Chrome DevTools command through the driver's CDP passthrough.
async fn cdp(client: &fantoccini::Client, cmd: &str, params: serde_json::Value) {
    let session = session_of(client).await;
    let port = ensure_chromedriver();
    let url = format!(
        "http://{}:{port}/session/{session}/goog/cdp/execute",
        driver_host()
    );
    let response = reqwest::Client::new()
        .post(&url)
        .json(&serde_json::json!({ "cmd": cmd, "params": params }))
        .send()
        .await
        .unwrap_or_else(|err| panic!("BROKEN(driver): the {cmd} request failed: {err}"));
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "BROKEN(driver): the browser driver refused {cmd} ({status}): {body}"
    );
}

fn touch_points_json(points: &TouchSet) -> serde_json::Value {
    serde_json::Value::Array(
        points
            .iter()
            .map(|(id, (x, y))| serde_json::json!({ "x": x, "y": y, "id": id }))
            .collect(),
    )
}

async fn cdp_touch(client: &fantoccini::Client, kind: PointerKind, ticks: &[Tick]) {
    let session = session_of(client).await;
    let id = touch_id(kind);
    // Where this finger is when it is not down yet (the next press lands there).
    let mut hover: Option<(f64, f64)> = None;
    for tick in ticks {
        let points = {
            let mut guard = TOUCH_POINTS.lock().expect("touch points lock");
            guard
                .get_or_insert_with(Default::default)
                .entry(session.clone())
                .or_default()
                .clone()
        };
        let set = |points: TouchSet| {
            let mut guard = TOUCH_POINTS.lock().expect("touch points lock");
            guard
                .get_or_insert_with(Default::default)
                .insert(session.clone(), points);
        };
        match *tick {
            Tick::Pause(ms) => tokio::time::sleep(Duration::from_millis(ms)).await,
            Tick::Move(p) => {
                if points.contains_key(&id) {
                    let mut moved = points.clone();
                    moved.insert(id, p);
                    cdp(
                        client,
                        "Input.dispatchTouchEvent",
                        serde_json::json!({ "type": "touchMove", "touchPoints": touch_points_json(&moved) }),
                    )
                    .await;
                    set(moved);
                } else {
                    hover = Some(p);
                }
            }
            Tick::Down(secondary) => {
                assert!(!secondary, "a touch has no secondary button");
                let p = hover.expect("a touch press needs a point: start the gesture with To");
                let mut pressed = points.clone();
                pressed.insert(id, p);
                cdp(
                    client,
                    "Input.dispatchTouchEvent",
                    serde_json::json!({ "type": "touchStart", "touchPoints": touch_points_json(&pressed) }),
                )
                .await;
                set(pressed);
            }
            Tick::Up(_) => {
                let p = *points
                    .get(&id)
                    .expect("a touch release needs that finger to be down");
                // CDP releases exactly the points listed on touchEnd.
                let mut released = TouchSet::new();
                released.insert(id, p);
                cdp(
                    client,
                    "Input.dispatchTouchEvent",
                    serde_json::json!({ "type": "touchEnd", "touchPoints": touch_points_json(&released) }),
                )
                .await;
                let mut rest = points.clone();
                rest.remove(&id);
                hover = Some(p);
                set(rest);
            }
        }
    }
}

/// The layout viewport, `(innerWidth, innerHeight)`.
pub async fn viewport(client: &fantoccini::Client) -> (f64, f64) {
    let raw = client
        .execute("return [window.innerWidth, window.innerHeight];", vec![])
        .await
        .expect("read the viewport size");
    serde_json::from_value(raw).expect("viewport shape")
}

/// The viewport point of a [`DragSpot`], resolved from live geometry by the
/// same resolver the synthetic kit uses, so both drivers aim at the same place.
pub async fn spot_point(client: &fantoccini::Client, spot: DragSpot<'_>) -> (f64, f64) {
    let script = format!(
        "{SYNTHETIC_DRAG_KIT}\nvar p = window.__synthDrag.resolve(arguments[0]); return [p.x, p.y];"
    );
    let raw = client
        .execute(&script, vec![spot.to_json()])
        .await
        .unwrap_or_else(|err| panic!("resolve the point of {spot:?}: {err}"));
    serde_json::from_value(raw).expect("spot point shape")
}

/// The middle of the part of card `key` that is inside the viewport: where a
/// finger or cursor would actually press it. Panics if none of it is visible.
pub async fn card_press_point(client: &fantoccini::Client, key: &str) -> (f64, f64) {
    let raw = client
        .execute(
            "var c = document.querySelector('#board-columns [data-issue-key=\"' + arguments[0] + '\"]');
             if (!c) { return null; }
             var r = c.getBoundingClientRect();
             var l = Math.max(r.left, 0), t = Math.max(r.top, 0);
             var rr = Math.min(r.right, window.innerWidth), b = Math.min(r.bottom, window.innerHeight);
             if (rr - l < 8 || b - t < 8) { return [-1, -1]; }
             return [(l + rr) / 2, (t + b) / 2];",
            vec![serde_json::json!(key)],
        )
        .await
        .unwrap_or_else(|err| panic!("locate {key} to press it: {err}"));
    let point: Option<(f64, f64)> = serde_json::from_value(raw).expect("press point shape");
    let point = point.unwrap_or_else(|| panic!("{key} is not on the board to be pressed"));
    assert!(
        point.0 >= 0.0,
        "{key} is on the board but not on screen, so no finger could press it"
    );
    point
}

/// Arm the page event recorder (idempotent within one document; a reload
/// discards it, so re-arm after every navigation). Capture phase on `window`:
/// it sees every event before any page listener can stop it. The native-drag
/// entry is read in the bubble phase so it can report whether the page
/// cancelled the drag.
pub async fn install_pointer_recorder(client: &fantoccini::Client) {
    client
        .execute(
            r#"if (!window.__cpdRec) {
                 var rec = window.__cpdRec = { counts: {}, trusted: {}, types: {}, log: [], lastDown: null, nativeDrags: [] };
                 var keyOf = function (t) {
                   var el = t && t.closest ? t.closest('[data-issue-key]') : null;
                   return el ? el.getAttribute('data-issue-key') : (t && t.tagName ? t.tagName.toLowerCase() : '?');
                 };
                 var note = function (e) {
                   rec.counts[e.type] = (rec.counts[e.type] || 0) + 1;
                   if (e.isTrusted) { rec.trusted[e.type] = (rec.trusted[e.type] || 0) + 1; }
                   if (e.pointerType) { rec.types[e.pointerType] = true; }
                   if (e.type === 'pointerdown') { rec.lastDown = { id: e.pointerId, type: e.pointerType, key: keyOf(e.target) }; }
                   if (rec.log.length > 400) { rec.log.shift(); }
                   rec.log.push(e.type + (e.pointerType ? '/' + e.pointerType + '#' + e.pointerId : '')
                     + '@' + keyOf(e.target) + (e.isTrusted ? '' : '(synthetic)'));
                 };
                 ['pointerdown','pointermove','pointerup','pointercancel','touchstart','touchmove','touchend',
                  'touchcancel','click','contextmenu','dragstart'].forEach(function (t) {
                   window.addEventListener(t, note, { capture: true, passive: true });
                 });
                 window.addEventListener('dragstart', function (e) {
                   rec.nativeDrags.push({ key: keyOf(e.target), cancelled: e.defaultPrevented, trusted: e.isTrusted });
                 });
               }
               return true;"#,
            vec![],
        )
        .await
        .expect("arm the pointer recorder");
}

/// What the recorder has seen on the current document.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PointerRecord {
    pub counts: std::collections::HashMap<String, u64>,
    pub trusted: std::collections::HashMap<String, u64>,
    pub types: std::collections::HashMap<String, bool>,
    pub log: Vec<String>,
    #[serde(rename = "lastDown")]
    pub last_down: Option<serde_json::Value>,
    #[serde(rename = "nativeDrags")]
    pub native_drags: Vec<serde_json::Value>,
}

impl PointerRecord {
    /// Trusted events of `event_type` the page received.
    pub fn trusted(&self, event_type: &str) -> u64 {
        self.trusted.get(event_type).copied().unwrap_or(0)
    }

    /// Every event of `event_type` the page received, trusted or not.
    pub fn count(&self, event_type: &str) -> u64 {
        self.counts.get(event_type).copied().unwrap_or(0)
    }

    /// One line for a failure message: trusted counts and the last events.
    pub fn describe(&self) -> String {
        let mut kinds: Vec<&String> = self.counts.keys().collect();
        kinds.sort();
        let counts = kinds
            .iter()
            .map(|k| format!("{k}={}/{}", self.trusted(k), self.count(k)))
            .collect::<Vec<_>>()
            .join(" ");
        let tail = self
            .log
            .iter()
            .rev()
            .take(10)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "page recorder (trusted/all): [{counts}]; pointer types seen: {:?}; native drags: \
             {:?}; last events: [{tail}]",
            self.types.keys().collect::<Vec<_>>(),
            self.native_drags
        )
    }
}

/// Read the recorder. An unarmed document reads as empty.
pub async fn pointer_record(client: &fantoccini::Client) -> PointerRecord {
    let raw = client
        .execute("return window.__cpdRec || null;", vec![])
        .await
        .expect("read the pointer recorder");
    if raw.is_null() {
        return PointerRecord::default();
    }
    serde_json::from_value(raw).expect("pointer recorder shape")
}

/// The system takes the touch away from the page mid-gesture: an incoming call,
/// the notification shade, the OS back gesture. Chrome reports it as a TRUSTED
/// `touchcancel` + `pointercancel`. Driven through Chrome's own input domain
/// (`Input.dispatchTouchEvent` `touchCancel`, reached through the driver's CDP
/// passthrough) because W3C Actions have no portable "the OS took it" step:
/// measured, chromedriver 151 ACCEPTS the W3C `pointerCancel` action and
/// dispatches nothing at all.
pub async fn system_cancels_touch(client: &fantoccini::Client) {
    cdp(
        client,
        "Input.dispatchTouchEvent",
        serde_json::json!({ "type": "touchCancel", "touchPoints": [] }),
    )
    .await;
    let session = session_of(client).await;
    if let Some(map) = TOUCH_POINTS.lock().expect("touch points lock").as_mut() {
        map.remove(&session);
    }
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
    //!
    //! Plus the WRONG-PAGE DIAGNOSTIC (2026-08-30). `describe_wrong_page` is a
    //! pure function and therefore its own driving port, so these are
    //! port-to-port at domain scope with no browser in sight. Behaviour budget:
    //! 2 behaviours (report where the browser actually was; attribute the cause
    //! from the sign-in-form state) x 2 = 4 permitted; 2 authored, each
    //! table-driven across BOTH sign-in states so neither arm is an untested
    //! branch.

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

    /// Both sign-in states, so the observable is the REPORTING, not one branch.
    fn wrong_pages() -> [WrongPage; 2] {
        [
            WrongPage {
                url: "http://127.0.0.1:53511/sign-in".to_string(),
                title: "Sign in to Foundry".to_string(),
                shows_sign_in_form: true,
            },
            WrongPage {
                url: "http://127.0.0.1:53511/team/backend/project/sandbox".to_string(),
                title: "Sandbox - Backend".to_string(),
                shows_sign_in_form: false,
            },
        ]
    }

    /// BEHAVIOUR 1 — the diagnostic names WHERE the browser actually was.
    ///
    /// This is the whole defect. The wait that expired said only "the board must
    /// render the GEN-1 card", which names a cause the evidence never supported
    /// and reads identically whichever page the browser was stranded on. A
    /// message that cannot be told apart between "the card is missing from the
    /// board" and "we are looking at the sign-in form" is why this was filed as
    /// an environmental flake for two features running.
    #[test]
    fn the_diagnostic_names_the_page_the_browser_was_actually_on() {
        for actual in wrong_pages() {
            let message = describe_wrong_page(
                "the project board",
                ".board#board-columns",
                Duration::from_secs(10),
                &actual,
            );
            assert!(
                message.contains(&actual.url),
                "the diagnostic must quote the URL the browser was actually at, \
                 or the reader cannot tell a stranded navigation from a broken \
                 render. got: {message}"
            );
            assert!(
                message.contains(&actual.title),
                "the diagnostic must quote the document title actually showing — \
                 the one fact that names the page in the reader's own words. \
                 got: {message}"
            );
            assert!(
                message.contains("the project board") && message.contains(".board#board-columns"),
                "the diagnostic must say what it was waiting FOR as well as what \
                 it got. got: {message}"
            );
        }
    }

    /// BEHAVIOUR 2 — the diagnostic attributes the CAUSE from the sign-in state,
    /// and the two attributions are different.
    ///
    /// A sign-in form on the page means the session was not authenticated when
    /// the navigation ran; no sign-in form means it was, and the fault lies
    /// elsewhere. Reporting the same sentence for both would be the original bug
    /// with more words.
    #[test]
    fn the_diagnostic_attributes_the_cause_from_the_sign_in_state() {
        let [stranded_on_sign_in, on_another_authenticated_page] = wrong_pages();
        let bounced = describe_wrong_page(
            "the project board",
            ".board#board-columns",
            Duration::from_secs(10),
            &stranded_on_sign_in,
        );
        let authenticated = describe_wrong_page(
            "the project board",
            ".board#board-columns",
            Duration::from_secs(10),
            &on_another_authenticated_page,
        );

        assert!(
            bounced.contains("sign-in form present: true"),
            "the sign-in probe's answer must be stated outright, not left to be \
             inferred from the URL. got: {bounced}"
        );
        assert!(
            authenticated.contains("sign-in form present: false"),
            "and stated in the negative case too, so its absence is evidence \
             rather than an omission. got: {authenticated}"
        );
        assert_ne!(
            bounced, authenticated,
            "the two states must produce DIFFERENT readings. One message for \
             every failure mode is exactly the vacuity being fixed"
        );
        assert!(
            bounced.contains("NOT authenticated"),
            "a sign-in form on the page has one meaning — the session was not \
             authenticated — and the diagnostic must commit to it. got: {bounced}"
        );
        assert!(
            !authenticated.contains("NOT authenticated"),
            "and must NOT claim it when no sign-in form is present, or the \
             attribution is noise. got: {authenticated}"
        );
    }
}
