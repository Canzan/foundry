//! fix-comment-delete-csrf — the `@needs-browser` step definitions for the
//! comment-Delete CSRF gap.
//!
//! THE DEFECT (RCA docs/feature/fix-comment-delete-csrf/rca/root-cause-analysis.md):
//! the comment Delete button (comment_card.html:4) is a bare `hx-delete` with NO
//! CSRF token. csrf_middleware requires a token for DELETE (is_safe_method,
//! csrf.rs:125, allows only GET/HEAD/OPTIONS). A body-less DELETE cannot carry
//! `_csrf` the way a form POST/PATCH does, the button has no `hx-headers` echo, and
//! there is no global htmx CSRF injection — so in a REAL browser the DELETE hits
//! csrf_middleware with no token → 403 BEFORE the handler runs. htmx 2.0.4 does not
//! swap a 4xx body, so the `#comment-{id}` card STAYS. Comment deletion is broken
//! for real users.
//!
//! WHY A BROWSER AND NOT reqwest (RCA Root Cause B): the shipped HTTP-lane
//! comment-delete tests (us_10_comment_edit_delete) set the CSRF header/field
//! MANUALLY on the reqwest client, so they pass and can never see the button's
//! missing token. Only a real browser submits the button AS SHIPPED. This scenario
//! ADDS the DOM-level oracle the HTTP lane structurally cannot provide: click the
//! shipped Delete button, assert the card is REMOVED from the DOM.
//!
//! FALSIFIABILITY: against the CURRENT tree (Delete button with no `hx-headers`),
//! the DELETE 403s, htmx discards the 4xx, and the `.comment` card STAYS — so
//! `the comment card is removed from the page` FAILS. That RED, seen before the
//! `hx-headers` fix, is the direct reproduction of the defect. After the fix the
//! DELETE carries `x-csrf-token` (the cookie→header double-submit csrf_middleware
//! accepts at csrf.rs:181-186), the handler soft-deletes, returns the
//! `.comment-deleted` fragment, and htmx swaps the card away → GREEN.
//!
//! REUSE: the seed (`the "Sandbox" project has an issue "GEN-1" with a comment by
//! Mei`) and the issue-page browser Given (`Mei is viewing the "GEN-1" issue page
//! in a real browser`) are the shipped S6 steps in feature_form_error_display.rs —
//! globally registered, so this feature drives the identical seeded comment card.
//! The Background (workspace/project/sign-in) is the shipped HTTP-lane seed. Only
//! the When (click Delete) and the two Then oracles below are new; their phrases
//! are globally unique per cucumber-rs.

use crate::world::FoundryWorld;
use cucumber::{then, when};
use fantoccini::Locator;
use std::time::{Duration, Instant};

/// The seeded project's key prefix (matches the shipped S6 seed in
/// feature_form_error_display.rs, which inserts the comment under key prefix GEN).
const PROJECT_KEY_PREFIX: &str = "GEN";

/// The seeded comment's body — mirrors ORIGINAL_COMMENT_BODY in the shipped S6
/// seed. Used only to make the removal oracle's diagnostic concrete.
const SEEDED_COMMENT_BODY: &str = "The gateway needs a circuit breaker";

/// The comment card's Delete button on the issue page (comment_card.html:4).
/// Rendered because Mei authored the comment (`can_delete` true).
const COMMENT_DELETE_BUTTON_SELECTOR: &str = ".comment .comment-delete-button";

/// The live comment card article (comment_card.html:1 — `article.comment`). After
/// a successful delete htmx swaps its outerHTML for the server's
/// `<div class="comment-deleted">` fragment (comments.rs:494), whose class token is
/// NOT `comment`, so `.comment` no longer matches anything on the page.
const COMMENT_CARD_SELECTOR: &str = ".comment";

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

// --- When -------------------------------------------------------------------

/// Click the shipped Delete button on the seeded comment card. On the current
/// tree the resulting DELETE carries no CSRF token → csrf_middleware 403s it and
/// htmx discards the 4xx; after the fix the button's `hx-headers` supplies the
/// `x-csrf-token` cookie→header token and the DELETE is accepted.
#[when(regex = r"^Mei clicks the comment's Delete button$")]
async fn clicks_delete_button(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(COMMENT_DELETE_BUTTON_SELECTOR))
        .await
        .expect("the issue page must render the seeded comment's Delete button")
        .click()
        .await
        .expect("click the comment Delete button");
}

// --- Then -------------------------------------------------------------------

/// THE ORACLE. Bounded-poll until the `.comment` card is GONE from the DOM (htmx
/// swapped its outerHTML for the `.comment-deleted` fragment on a 200). Against the
/// current tree the DELETE 403s (no CSRF token), htmx 2.0.4 discards the 4xx body,
/// and the card STAYS — so this never holds and fails with the reproduction
/// diagnostic. This is the DOM-level oracle the HTTP lane cannot provide.
#[then(regex = r"^the comment card is removed from the page$")]
async fn comment_card_removed(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let cards = browser
            .find_all(Locator::Css(COMMENT_CARD_SELECTOR))
            .await
            .unwrap_or_default();
        if cards.is_empty() {
            return;
        }
        if Instant::now() >= deadline {
            let count = cards.len();
            panic!(
                "the comment card is still on the page ({count} `.comment` element(s) remain) after \
                 clicking Delete. On the current tree the Delete button carries no CSRF token, so \
                 csrf_middleware returns 403 BEFORE the handler runs; htmx 2.0.4 does not swap a 4xx \
                 body, so the card containing {SEEDED_COMMENT_BODY:?} is never removed. THIS is the \
                 defect the browser oracle exists to catch — the HTTP lane injects the token manually \
                 and cannot see the missing one."
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Backs the DOM oracle at the driven-store boundary: the delete actually
/// persisted a soft-delete tombstone (`deleted_at IS NOT NULL`, migration 0006).
/// Guards against a DOM-only mis-swap that removed the card without deleting the
/// row — the card removal must reflect a real soft delete, not a cosmetic swap.
#[then(regex = r"^the comment is recorded as deleted in the store$")]
async fn comment_recorded_deleted(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let deleted: (bool,) = sqlx::query_as(
        "SELECT c.deleted_at IS NOT NULL FROM comments c \
           JOIN issues i ON i.id = c.issue_id \
           JOIN projects p ON p.id = i.project_id \
          WHERE p.key_prefix = $1",
    )
    .bind(PROJECT_KEY_PREFIX)
    .fetch_one(pool)
    .await
    .expect("read the seeded comment from the store");
    assert!(
        deleted.0,
        "the comment card left the DOM but the store shows deleted_at IS NULL — the DELETE never \
         reached the soft-delete handler (the CSRF 403 would produce exactly this), so the removal \
         was not a real delete."
    );
}
