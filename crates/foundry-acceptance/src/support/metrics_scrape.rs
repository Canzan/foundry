//! Prometheus text-exposition consumer for slice-6 acceptance scenarios.
//!
//! Analogous to slice-2's `sse_client.rs`. Does `reqwest::get` against
//! the subprocess's bound metrics URL and parses the response into
//! typed structs the step bodies assert against.
//!
//! Per `distill/driver.md` §2a and `distill/proposals.md` § "Decision-
//! driven invented detail #5": intentionally NOT taking a dependency
//! on `prometheus-parse` — the parsing surface needed is small enough
//! to fit in this file, and avoiding the new crate dep matches the
//! slice-2 justification for rolling our own SSE parser.
//!
//! Public surface:
//! - [`scrape_metrics`] — happy-path scrape returning a parsed snapshot.
//! - [`scrape_metrics_raw`] — returns (status, body) without parsing.
//! - [`ScrapeSnapshot`] — typed view over the scrape.
//! - [`MetricSample`] — one parsed sample line.
//!
//! Prometheus text-exposition subset handled:
//! - Lines beginning `#` are HELP / TYPE comments — skipped.
//! - Blank lines — skipped.
//! - Other lines parse as `{name}{labels?} {value}` where `{labels?}`
//!   is an optional `{k1="v1",k2="v2"}` block.
//! - Values parse as `f64`; `NaN`, `+Inf`, `-Inf` supported.
//! - Histogram bucket lines (e.g.
//!   `http_request_duration_seconds_bucket{le="0.005"}`) and summary
//!   quantile lines (e.g.
//!   `http_request_duration_seconds{quantile="0.99"}`) are parsed
//!   generically; the caller uses `samples_for` / `samples_with_prefix`
//!   to slice them.

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// One metric sample as exposed in the Prometheus text format.
///
/// Example line:
/// ```text
/// http_requests_total{path="/healthz",method="GET",status="200"} 5
/// ```
/// parses to:
/// ```text
/// MetricSample {
///     name: "http_requests_total",
///     labels: {"path": "/healthz", "method": "GET", "status": "200"},
///     value: 5.0,
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct MetricSample {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub value: f64,
}

/// Parsed view over a single `/metrics` scrape.
#[derive(Debug)]
pub struct ScrapeSnapshot {
    #[allow(dead_code)]
    pub raw_body: String,
    pub samples: Vec<MetricSample>,
}

impl ScrapeSnapshot {
    /// Return all samples whose name matches `name` exactly.
    pub fn samples_for(&self, name: &str) -> Vec<&MetricSample> {
        self.samples.iter().filter(|s| s.name == name).collect()
    }

    /// Return all samples whose name starts with `prefix` (e.g.
    /// `"http_request_duration_seconds_bucket"` to collect histogram
    /// bucket lines).
    #[allow(dead_code)]
    pub fn samples_with_prefix(&self, prefix: &str) -> Vec<&MetricSample> {
        self.samples
            .iter()
            .filter(|s| s.name.starts_with(prefix))
            .collect()
    }

    /// Sum of `value` across all samples named `name`. The semantic for
    /// counters is "the family's total" — counters never decrement so
    /// the sum across label combinations IS the family total.
    pub fn sum_for(&self, name: &str) -> f64 {
        self.samples_for(name).into_iter().map(|s| s.value).sum()
    }

    /// Return true if the body contains a line for `name` OR for a
    /// derived series with that prefix (e.g. `_count`, `_sum`, etc.).
    /// Used as a cheap pre-check before drilling into samples.
    pub fn contains_metric_line(&self, name: &str) -> bool {
        self.samples
            .iter()
            .any(|s| s.name == name || s.name.starts_with(&format!("{name}_")))
    }

    /// Collect the set of label KEYS used across all samples whose
    /// metric NAME matches `name`. Used by the cardinality safety
    /// scenario to assert no forbidden keys appear.
    pub fn label_keys_for(&self, name: &str) -> BTreeSet<String> {
        self.samples_for(name)
            .into_iter()
            .flat_map(|s| s.labels.keys().cloned())
            .collect()
    }

    /// Sum of `_count` series derived from a histogram NAME (e.g.
    /// `"http_request_duration_seconds"` -> looks at the
    /// `http_request_duration_seconds_count` lines). Used by the
    /// "histogram has at least one bucket with count >= N" Then step
    /// — the `metrics-exporter-prometheus` recorder default renders
    /// histograms as Prometheus summaries (with `_count` + `_sum` +
    /// `quantile=...` quantile lines), so the slice-6 scrape check
    /// inspects `_count` for the observation count instead of the
    /// `_bucket{le=...}` shape we'd get from a native histogram.
    pub fn histogram_observation_count(&self, name: &str) -> u64 {
        let count_name = format!("{name}_count");
        self.samples
            .iter()
            .filter(|s| s.name == count_name)
            .map(|s| s.value as u64)
            .sum()
    }
}

/// Scrape `http://{addr}/metrics` and parse the response. Panics on
/// HTTP error, non-200, or parse error — these are test failures, not
/// recoverable conditions.
pub async fn scrape_metrics(addr: SocketAddr) -> ScrapeSnapshot {
    let (status, body) = scrape_metrics_raw(addr).await;
    assert_eq!(
        status,
        reqwest::StatusCode::OK,
        "expected /metrics to return 200, got {status} (body: {body})"
    );
    let samples = parse_exposition(&body);
    ScrapeSnapshot {
        raw_body: body,
        samples,
    }
}

/// As [`scrape_metrics`] but returns the raw HTTP status + body so the
/// caller can assert on them. Used by scenario #9 which explicitly
/// asserts HTTP 200 on the startup-probe success path.
pub async fn scrape_metrics_raw(addr: SocketAddr) -> (reqwest::StatusCode, String) {
    let url = format!("http://127.0.0.1:{}/metrics", addr.port());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("build scrape http client");
    let resp = client
        .get(&url)
        .send()
        .await
        .unwrap_or_else(|err| panic!("scrape GET {url} failed: {err}"));
    let status = resp.status();
    let body = resp
        .text()
        .await
        .unwrap_or_else(|err| panic!("read scrape body from {url} failed: {err}"));
    (status, body)
}

/// Inner-loop poll cadence used by [`poll_until_sample`].
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Per-scrape HTTP timeout used by [`poll_until_sample`]. Deliberately
/// short — the local sidecar listener returns in sub-millisecond under
/// idle load, so a 750ms ceiling fails-fast under contention rather
/// than absorbing the entire poll deadline in a single hung scrape.
/// (The shared [`scrape_metrics_raw`] uses a 10s timeout suitable for
/// one-shot startup-probe scenarios but pathological for a poll loop.)
const POLL_SCRAPE_TIMEOUT: Duration = Duration::from_millis(750);

/// Poll the `/metrics` endpoint up to `timeout`, returning the first
/// [`MetricSample`] for `metric_name` that satisfies `predicate`. Panics
/// with the full sample history on timeout — flake-debuggable.
///
/// Used by step contracts that need "eventually" semantics on an
/// asynchronously-updated metric (a gauge updated by a background poll
/// task, or a counter incremented by a scheduled sweep tick). A single-
/// instant scrape can sample the value before the background task has
/// run; this helper retries until the value satisfies the predicate or
/// the deadline elapses.
///
/// Introduced for slice-6 (`db_connections_in_use` gauge); promoted to
/// this support module when slice-7's tombstone-GC counter became the
/// second caller. See
/// `docs/feature/slice-6-scenario-hardening/distill/wave-decisions.md` § D2
/// and `docs/feature/slice-7-gc-counter-race/distill/wave-decisions.md`.
pub async fn poll_until_sample<P>(
    addr: SocketAddr,
    metric_name: &str,
    predicate: P,
    timeout: Duration,
) -> MetricSample
where
    P: Fn(&MetricSample) -> bool,
{
    // Build a single client and reuse it for every scrape — connection
    // pooling avoids a fresh TCP handshake per poll iteration. The
    // 750ms per-scrape timeout caps each request so a slow scrape can't
    // monopolise the outer deadline.
    let client = reqwest::Client::builder()
        .timeout(POLL_SCRAPE_TIMEOUT)
        .build()
        .expect("build poll_until_sample http client");
    let url = format!("http://127.0.0.1:{}/metrics", addr.port());

    let started_at = Instant::now();
    let deadline = started_at + timeout;
    // Full sample history across the deadline window — the timeout
    // panic dumps this so a flake-investigator sees the temporal shape
    // of what the subprocess actually emitted, including scrape errors
    // (which often ARE the signal when the subprocess is unhealthy).
    let mut history: Vec<(Duration, Result<Vec<MetricSample>, String>)> = Vec::new();
    loop {
        let now = Instant::now();
        let result = match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status == reqwest::StatusCode::OK {
                    match resp.text().await {
                        Ok(body) => {
                            let parsed = parse_exposition(&body);
                            let matching: Vec<MetricSample> = parsed
                                .iter()
                                .filter(|s| s.name == metric_name)
                                .cloned()
                                .collect();
                            if let Some(hit) = matching.iter().find(|s| predicate(s)) {
                                return hit.clone();
                            }
                            Ok(matching)
                        }
                        Err(err) => Err(format!("body read: {err}")),
                    }
                } else {
                    Err(format!("status {status}"))
                }
            }
            Err(err) => Err(format!("send: {err}")),
        };
        history.push((now.duration_since(started_at), result));
        if Instant::now() >= deadline {
            let mut lines = String::new();
            for (t, outcome) in &history {
                match outcome {
                    Ok(samples) if samples.is_empty() => lines.push_str(&format!(
                        "  [t+{:.2}s] no `{metric_name}` samples\n",
                        t.as_secs_f64()
                    )),
                    Ok(samples) => lines.push_str(&format!(
                        "  [t+{:.2}s] samples={samples:?}\n",
                        t.as_secs_f64()
                    )),
                    Err(err) => lines.push_str(&format!(
                        "  [t+{:.2}s] scrape error: {err}\n",
                        t.as_secs_f64()
                    )),
                }
            }
            panic!(
                "poll_until_sample for `{metric_name}` timed out after {:?} \
                 ({} scrapes observed).\n{lines}",
                timeout,
                history.len(),
            );
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Parse a Prometheus text-exposition body into `MetricSample`s.
///
/// Returns an empty Vec on empty input (no panic). Comment + blank
/// lines are skipped. Malformed lines are skipped with a warning to
/// stderr (defensive — the recorder we're parsing should never emit
/// malformed lines, but a single bad line shouldn't fail the entire
/// scrape).
pub fn parse_exposition(body: &str) -> Vec<MetricSample> {
    let mut samples = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        match parse_line(trimmed) {
            Some(sample) => samples.push(sample),
            None => {
                eprintln!("metrics_scrape: skipping malformed line: {trimmed:?}");
            }
        }
    }
    samples
}

/// Parse a single exposition line of the form
/// `name[{labels}] value [timestamp]`. Returns `None` on parse error.
fn parse_line(line: &str) -> Option<MetricSample> {
    // Split into "<name>[{labels}]" and "value [ts]" at the first
    // top-level whitespace AFTER the closing `}` of the optional
    // label block. Without a label block the split is at the first
    // whitespace.
    let (head, tail) = split_head_tail(line)?;
    let (name, labels) = parse_head(head)?;
    let value_str = tail.split_ascii_whitespace().next()?;
    let value: f64 = parse_value(value_str)?;
    Some(MetricSample {
        name,
        labels,
        value,
    })
}

/// Split `name[{labels}] value` into (head, tail) at the first
/// whitespace OUTSIDE the optional label block.
fn split_head_tail(line: &str) -> Option<(&str, &str)> {
    let mut depth = 0u32;
    let mut in_quotes = false;
    let mut escape = false;
    for (i, ch) in line.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_quotes {
            match ch {
                '\\' => escape = true,
                '"' => in_quotes = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_quotes = true,
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            c if c.is_ascii_whitespace() && depth == 0 => {
                let head = &line[..i];
                let tail = line[i..].trim_start();
                return Some((head, tail));
            }
            _ => {}
        }
    }
    None
}

/// Parse `name` or `name{k1="v1",k2="v2"}` into (name, labels).
fn parse_head(head: &str) -> Option<(String, BTreeMap<String, String>)> {
    let open = head.find('{');
    match open {
        None => Some((head.to_string(), BTreeMap::new())),
        Some(idx) => {
            let name = head[..idx].to_string();
            if !head.ends_with('}') {
                return None;
            }
            let label_block = &head[idx + 1..head.len() - 1];
            let labels = parse_label_block(label_block);
            Some((name, labels))
        }
    }
}

/// Parse `k1="v1",k2="v2"` into a sorted map.
fn parse_label_block(block: &str) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    let mut state = LabelState::Key(String::new());
    let mut current_key = String::new();
    let mut escape = false;
    for ch in block.chars() {
        match &mut state {
            LabelState::Key(buf) => match ch {
                '=' => {
                    current_key = std::mem::take(buf);
                    state = LabelState::ExpectQuote;
                }
                ',' => {
                    // Stray separator — skip.
                    buf.clear();
                }
                c if c.is_whitespace() => { /* ignore */ }
                c => buf.push(c),
            },
            LabelState::ExpectQuote => match ch {
                '"' => state = LabelState::Value(String::new()),
                _ => {
                    // Malformed — abandon current pair.
                    state = LabelState::Key(String::new());
                }
            },
            LabelState::Value(buf) => {
                if escape {
                    match ch {
                        'n' => buf.push('\n'),
                        't' => buf.push('\t'),
                        '\\' => buf.push('\\'),
                        '"' => buf.push('"'),
                        other => buf.push(other),
                    }
                    escape = false;
                    continue;
                }
                match ch {
                    '\\' => escape = true,
                    '"' => {
                        let value = std::mem::take(buf);
                        labels.insert(current_key.clone(), value);
                        state = LabelState::ExpectSeparator;
                    }
                    c => buf.push(c),
                }
            }
            LabelState::ExpectSeparator => match ch {
                ',' => state = LabelState::Key(String::new()),
                c if c.is_whitespace() => { /* ignore */ }
                _ => {
                    // Malformed — start a fresh key.
                    state = LabelState::Key(String::new());
                }
            },
        }
    }
    labels
}

enum LabelState {
    Key(String),
    ExpectQuote,
    Value(String),
    ExpectSeparator,
}

/// Parse a Prometheus value token (`f64`, `NaN`, `+Inf`, `-Inf`).
fn parse_value(token: &str) -> Option<f64> {
    match token {
        "NaN" => Some(f64::NAN),
        "+Inf" => Some(f64::INFINITY),
        "-Inf" => Some(f64::NEG_INFINITY),
        other => other.parse::<f64>().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_counter_line() {
        let body = r#"# HELP http_requests_total The total number of HTTP requests
# TYPE http_requests_total counter
http_requests_total{path="/healthz",method="GET",status="200"} 5
"#;
        let samples = parse_exposition(body);
        assert_eq!(samples.len(), 1);
        let s = &samples[0];
        assert_eq!(s.name, "http_requests_total");
        assert_eq!(s.value, 5.0);
        assert_eq!(s.labels.get("path"), Some(&"/healthz".to_string()));
        assert_eq!(s.labels.get("method"), Some(&"GET".to_string()));
        assert_eq!(s.labels.get("status"), Some(&"200".to_string()));
    }

    #[test]
    fn parses_unlabeled_gauge_line() {
        let body = "db_connections_in_use 3\n";
        let samples = parse_exposition(body);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].name, "db_connections_in_use");
        assert_eq!(samples[0].value, 3.0);
        assert!(samples[0].labels.is_empty());
    }

    #[test]
    fn parses_route_template_with_braces_in_label_value() {
        let body = r#"http_requests_total{path="/team/{team_slug}/project/{project_slug}",method="POST",status="201"} 1
"#;
        let samples = parse_exposition(body);
        assert_eq!(samples.len(), 1);
        assert_eq!(
            samples[0].labels.get("path"),
            Some(&"/team/{team_slug}/project/{project_slug}".to_string())
        );
    }

    #[test]
    fn sums_counter_across_label_combinations() {
        let body = "http_requests_total{path=\"/a\",method=\"GET\",status=\"200\"} 3\n\
                    http_requests_total{path=\"/b\",method=\"GET\",status=\"200\"} 2\n\
                    http_requests_total{path=\"/a\",method=\"POST\",status=\"201\"} 1\n";
        let snap = ScrapeSnapshot {
            raw_body: body.to_string(),
            samples: parse_exposition(body),
        };
        assert_eq!(snap.sum_for("http_requests_total"), 6.0);
    }

    #[test]
    fn label_keys_collected_across_samples() {
        let body = "http_requests_total{path=\"/a\",method=\"GET\",status=\"200\"} 1\n";
        let snap = ScrapeSnapshot {
            raw_body: body.to_string(),
            samples: parse_exposition(body),
        };
        let keys = snap.label_keys_for("http_requests_total");
        let expected: BTreeSet<String> = ["path", "method", "status"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn histogram_observation_count_sums_count_lines() {
        let body = "http_request_duration_seconds_count{path=\"/a\",method=\"GET\",status=\"200\"} 3\n\
                    http_request_duration_seconds_count{path=\"/b\",method=\"GET\",status=\"200\"} 5\n\
                    http_request_duration_seconds_sum{path=\"/a\",method=\"GET\",status=\"200\"} 0.12\n";
        let snap = ScrapeSnapshot {
            raw_body: body.to_string(),
            samples: parse_exposition(body),
        };
        assert_eq!(
            snap.histogram_observation_count("http_request_duration_seconds"),
            8
        );
    }

    #[test]
    fn skips_comment_and_blank_lines() {
        let body = "# HELP foo bar\n\
                    \n\
                    # TYPE foo counter\n\
                    foo 1\n";
        assert_eq!(parse_exposition(body).len(), 1);
    }

    #[test]
    fn contains_metric_line_matches_base_or_derived() {
        let body =
            "http_request_duration_seconds_count{path=\"/a\",method=\"GET\",status=\"200\"} 3\n";
        let snap = ScrapeSnapshot {
            raw_body: body.to_string(),
            samples: parse_exposition(body),
        };
        assert!(snap.contains_metric_line("http_request_duration_seconds"));
        assert!(snap.contains_metric_line("http_request_duration_seconds_count"));
        assert!(!snap.contains_metric_line("nope"));
    }
}
