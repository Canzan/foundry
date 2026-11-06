//! SCAFFOLD: true — instance-integration-setup DISTILL (2026-11-06), RED
//! scaffold for the generated-key test seam (DISCUSS D3, DESIGN D8, brief.md
//! "For Acceptance Designer": the generated secrets key is read only through
//! a `test-support` reader).
//!
//! Acceptance needs the secrets key Foundry generated as a planted string for
//! the "never shown" sweep and the database-dump check (KPI-IIS-G1). Nothing
//! but a test-support build may name this module: DELIVER lists it in
//! check-arch `integrations-test-seam` and gives it the marker string the
//! Dockerfile refuses (DEVOPS task 1).

// SCAFFOLD: true
/// The secrets key stored in the database `pool` reaches, unwrapped with `instance_key` (the
/// operator's setting value, or the Foundry-made file's contents), as its
/// base64url text. `None` when no secrets key is stored or `instance_key`
/// does not unwrap it.
pub async fn generated_secrets_key(pool: &sqlx::PgPool, instance_key: &str) -> Option<String> {
    let _ = (pool, instance_key);
    panic!("Not yet implemented -- RED scaffold")
}
