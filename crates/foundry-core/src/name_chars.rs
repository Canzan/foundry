//! The ONE refused-character predicate shared by every name rule
//! (project-name-rule DDD-1, ADR-PROJECT-NAME-001; originally
//! instance-workspace-name-rule D4). `WorkspaceName` and `ProjectName` both
//! call it, so the two rules cannot drift apart.

/// The D3/D4 refused set: Cc (`char::is_control`), the bidi embeddings and
/// overrides (U+202A-202E), the bidi isolates (U+2066-2069), and the line and
/// paragraph separators (U+2028, U+2029). Every other format character (ZWJ,
/// ZWNJ, ZWSP, LRM, RLM, soft hyphen, U+FEFF, ...) is allowed.
///
/// Mirrored in SQL, range for range, by `foundry_name_rule_violation` in
/// `crates/foundry-store/migrations/0019_workspace_name_rule.sql`
/// (name-db-checks DDD-3); change both together.
pub(crate) fn is_refused_name_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{2028}' | '\u{2029}'
        )
}
