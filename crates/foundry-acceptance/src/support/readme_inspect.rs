//! Helpers for inspecting the workspace's README + toolchain pin.
//!
//! US-13's assertions pin structural contracts (the README has a
//! Quickstart, the MSRV is pinned in `rust-toolchain.toml` and stated
//! in the README, and the hot-reload watch command + local URL are
//! documented). These helpers locate the workspace root via
//! `CARGO_MANIFEST_DIR` and return small semantic structs the step
//! bodies match against.
//!
//! The helpers are pure (Mandate 4): they take strings, return data.
//! The only impure bits (`std::fs::read_to_string`) live in
//! `read_readme` / `read_rust_toolchain` and are stable, fast, and
//! trivially reliable. Adapters around these would be ceremony for no
//! payoff.

use std::path::PathBuf;

/// One Quickstart heading + its fenced command blocks and the prose
/// paragraphs preceding the first fence. Returned by
/// [`find_quickstart`]; consumed by the US-13 Then steps.
pub struct QuickstartSection {
    pub heading_text: String,
    pub fenced_command_blocks: Vec<String>,
    pub prose_paragraphs: Vec<String>,
}

/// Resolve the workspace root. `CARGO_MANIFEST_DIR` for
/// `foundry-acceptance` is `<workspace>/crates/foundry-acceptance`, so
/// the workspace root is two parents up.
pub fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .expect("workspace root resolves from CARGO_MANIFEST_DIR")
}

/// Read the workspace `README.md`. Panics with a clear message if the
/// file is missing — the contributor-onboarding contract requires it.
pub fn read_readme() -> String {
    let path = workspace_root().join("README.md");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("workspace README.md missing at {}: {e}", path.display()))
}

/// Read the workspace `rust-toolchain.toml`. Panics if missing — the
/// MSRV pin is a structural contract per ADR-04 of the US-13 distill.
pub fn read_rust_toolchain() -> String {
    let path = workspace_root().join("rust-toolchain.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "workspace rust-toolchain.toml missing at {}: {e}",
            path.display()
        )
    })
}

/// Locate the README's Quickstart section. Recognised by a level-2 or
/// level-3 heading whose text contains "Quickstart" (case-insensitive),
/// up to but not including the next heading at the same-or-higher
/// level. Returns the heading text, every fenced block inside the
/// section (regardless of code-fence language tag), and every prose
/// paragraph the contributor scans before reaching the first fence.
pub fn find_quickstart(readme: &str) -> Option<QuickstartSection> {
    let lines: Vec<&str> = readme.lines().collect();
    let mut start: Option<(usize, usize, String)> = None; // (line_idx, heading_level, text)
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level == 0 || level > 6 {
            continue;
        }
        let text = trimmed[level..].trim();
        if text.to_lowercase().contains("quickstart") && (level == 2 || level == 3) {
            start = Some((i, level, text.to_string()));
            break;
        }
    }
    let (start_idx, heading_level, heading_text) = start?;

    // Find the end: next heading at level <= heading_level.
    let mut end_idx = lines.len();
    for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
        let trimmed = line.trim_start();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level > 0 && level <= heading_level {
            end_idx = i;
            break;
        }
    }

    let section_lines = &lines[start_idx + 1..end_idx];

    // Extract fenced code blocks.
    let mut fenced_command_blocks = Vec::new();
    let mut prose_paragraphs = Vec::new();
    let mut in_fence = false;
    let mut current_fence = String::new();
    let mut current_para = String::new();
    let mut first_fence_seen = false;

    for line in section_lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_fence {
                fenced_command_blocks.push(std::mem::take(&mut current_fence));
                in_fence = false;
                first_fence_seen = true;
            } else {
                if !current_para.trim().is_empty() && !first_fence_seen {
                    prose_paragraphs.push(current_para.trim().to_string());
                }
                current_para.clear();
                in_fence = true;
            }
            continue;
        }
        if in_fence {
            current_fence.push_str(line);
            current_fence.push('\n');
        } else if !first_fence_seen {
            // Blank line terminates a prose paragraph.
            if line.trim().is_empty() {
                if !current_para.trim().is_empty() {
                    prose_paragraphs.push(current_para.trim().to_string());
                    current_para.clear();
                }
            } else {
                current_para.push_str(line);
                current_para.push('\n');
            }
        }
    }
    if !current_para.trim().is_empty() && !first_fence_seen {
        prose_paragraphs.push(current_para.trim().to_string());
    }

    // A subsection heading (e.g. "### Prerequisites") inside the
    // Quickstart is part of the prereq prose for our purposes — the
    // contributor reads it as the answer to "what do I need before
    // running the first command". Pull subsection-heading text into
    // the prose pile so the prereq assertion can scan a single body.
    let mut subheading_prose = String::new();
    for line in section_lines {
        let trimmed = line.trim_start();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level > 0 && level > heading_level {
            subheading_prose.push_str(trimmed[level..].trim());
            subheading_prose.push('\n');
        }
    }
    if !subheading_prose.trim().is_empty() {
        prose_paragraphs.push(subheading_prose.trim().to_string());
    }

    Some(QuickstartSection {
        heading_text,
        fenced_command_blocks,
        prose_paragraphs,
    })
}

/// Extract the channel pinned by `rust-toolchain.toml`. Returns
/// `Some("1.85")` for `channel = "1.85"`, `Some("stable")` for the
/// generic channel, and `None` if the field is absent.
pub fn extract_pinned_msrv(toolchain_toml: &str) -> Option<String> {
    for line in toolchain_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        // Match `channel = "..."` (with arbitrary whitespace).
        if let Some(rest) = trimmed.strip_prefix("channel") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('=')?.trim_start();
            let rest = rest.strip_prefix('"')?;
            let end = rest.find('"')?;
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// Find the specific Rust version the README's prerequisites name.
/// Recognises patterns like `Rust 1.85`, `Rust 1.85.0`, `rust 1.85`,
/// `Rust 1.85+`. Returns the bare version string (e.g. `"1.85"`)
/// without the surrounding `Rust ` prefix or trailing `+`.
pub fn find_readme_msrv_mention(readme: &str) -> Option<String> {
    // Walk character-by-character: locate "Rust " (case-insensitive)
    // followed by a digit-dot-digit version.
    let lower = readme.to_lowercase();
    let mut search_from = 0usize;
    while let Some(rel_idx) = lower[search_from..].find("rust") {
        let idx = search_from + rel_idx;
        let after = &readme[idx + 4..];
        // Require a separator (space or non-breaking space) then digits.
        let after_trim = after.trim_start_matches([' ', '\t', '\u{a0}']);
        if after_trim.starts_with(|c: char| c.is_ascii_digit()) {
            // Extract the contiguous version: digits, dots, digits.
            let mut end = 0usize;
            let bytes = after_trim.as_bytes();
            let mut seen_dot = false;
            while end < bytes.len() {
                let c = bytes[end] as char;
                if c.is_ascii_digit() {
                    end += 1;
                } else if c == '.' && !seen_dot {
                    seen_dot = true;
                    end += 1;
                } else if c == '.' && seen_dot {
                    // Allow 1.85.0 — keep walking digits.
                    end += 1;
                } else {
                    break;
                }
            }
            if end > 0 && seen_dot {
                return Some(after_trim[..end].trim_end_matches('.').to_string());
            }
        }
        search_from = idx + 4;
    }
    None
}

/// Find the documented hot-reload command. Returns the first line in
/// the README that contains `cargo watch` (the contract).
pub fn find_watch_command(readme: &str) -> Option<String> {
    readme
        .lines()
        .find(|l| l.contains("cargo watch"))
        .map(|l| l.trim().to_string())
}

/// Find the local app URL the README documents (e.g.
/// `http://localhost:3000`). Returns the first `http://localhost...`
/// substring found.
pub fn find_local_app_url(readme: &str) -> Option<String> {
    let needle = "http://localhost";
    let start = readme.find(needle)?;
    let tail = &readme[start..];
    let end = tail
        .find(|c: char| c.is_whitespace() || c == ')' || c == '`' || c == '<' || c == '>')
        .unwrap_or(tail.len());
    Some(tail[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_pinned_msrv_recognises_specific_version() {
        let toml = r#"[toolchain]
channel = "1.85"
components = ["rustfmt"]
"#;
        assert_eq!(extract_pinned_msrv(toml).as_deref(), Some("1.85"));
    }

    #[test]
    fn extract_pinned_msrv_recognises_generic_channel() {
        let toml = r#"[toolchain]
channel = "stable"
"#;
        assert_eq!(extract_pinned_msrv(toml).as_deref(), Some("stable"));
    }

    #[test]
    fn extract_pinned_msrv_returns_none_when_absent() {
        let toml = r#"[toolchain]
components = ["clippy"]
"#;
        assert_eq!(extract_pinned_msrv(toml), None);
    }

    #[test]
    fn find_readme_msrv_mention_picks_first_version() {
        let readme = "Prerequisites: **Rust 1.85** or newer, plus Docker.";
        assert_eq!(find_readme_msrv_mention(readme).as_deref(), Some("1.85"));
    }

    #[test]
    fn find_readme_msrv_mention_ignores_bare_word() {
        let readme = "Foundry is written in Rust.";
        assert_eq!(find_readme_msrv_mention(readme), None);
    }

    #[test]
    fn find_watch_command_locates_cargo_watch_line() {
        let readme = "Run: `cargo watch -x 'run --bin foundry'`\n";
        assert!(find_watch_command(readme).unwrap().contains("cargo watch"));
    }

    #[test]
    fn find_local_app_url_extracts_localhost_url() {
        let readme = "Open http://localhost:3000 in a browser.";
        assert_eq!(
            find_local_app_url(readme).as_deref(),
            Some("http://localhost:3000")
        );
    }

    #[test]
    fn find_quickstart_collects_fenced_blocks_and_prose() {
        let readme = "\
# Foundry

## Quickstart

### Prerequisites

You need **Rust 1.85** and **Docker**.

### Five commands

```sh
git clone https://github.com/foundry-project/foundry.git
cd foundry
cp .env.example .env
docker compose up -d postgres
cargo test -p foundry-acceptance --release
```

## Other section
";
        let qs = find_quickstart(readme).expect("Quickstart section");
        assert!(qs.heading_text.contains("Quickstart"));
        assert_eq!(qs.fenced_command_blocks.len(), 1);
        assert!(qs.fenced_command_blocks[0].contains("cargo test"));
        let prose = qs.prose_paragraphs.join("\n");
        assert!(
            prose.contains("Rust"),
            "prose missing Rust prereq:\n{prose}"
        );
        assert!(
            prose.contains("Docker"),
            "prose missing Docker prereq:\n{prose}"
        );
    }

    #[test]
    fn find_quickstart_returns_none_when_absent() {
        let readme = "# Just a title\n\nNo quickstart here.\n";
        assert!(find_quickstart(readme).is_none());
    }
}
