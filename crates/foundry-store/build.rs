// Tell cargo to rebuild `foundry-store` whenever any migration file
// changes. Without this, the `sqlx::migrate!()` macro embeds whatever
// the migrations directory contained at the time of the FIRST build,
// and subsequent file additions / edits silently use the stale embed.
//
// This was discovered the hard way during the slice-3 end-to-end
// walkthrough: a docker-compose run produced a foundry-app image with
// migration 0004 baked in, but 0005 (added later in the same session)
// was missing. Operators who add a new migration and run
// `docker compose up -d` without `--build --no-cache` would see the
// app boot against a stale schema.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
