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
//!   reaped when the process exits.
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
use std::process::{Child, Command, Stdio};
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
/// 1st scenario's driver; the `Child` is parked in a `Mutex` for the lifetime of
/// the test binary (the OS reaps it on exit — the same contract the shared
/// Postgres testcontainer has).
static CHROMEDRIVER: OnceCell<u16> = OnceCell::new();
static CHROMEDRIVER_PROC: Mutex<Option<Child>> = Mutex::new(None);

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
    let port = ensure_chromedriver();
    let mut capabilities = serde_json::Map::new();
    capabilities.insert(
        "goog:chromeOptions".to_string(),
        serde_json::json!({
            "args": [
                "--headless=new",
                "--no-sandbox",
                "--disable-dev-shm-usage",
                "--disable-gpu",
                format!("--window-size={WINDOW_WIDTH},{WINDOW_HEIGHT}"),
            ]
        }),
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
        other => other,
    }
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
