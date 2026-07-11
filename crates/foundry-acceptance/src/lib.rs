//! foundry-acceptance — cucumber-rs world + step modules.
//!
//! Slice 1: only the US-01 docker-compose harness is wired. US-05..08
//! step modules land as those slices begin.

#![forbid(unsafe_code)]
#![allow(clippy::needless_return)]

pub mod support;
pub mod world;

pub mod steps {
    pub mod feature_a_programmatic;
    pub mod feature_b_web_tier;
    pub mod feature_board_new_issue;
    pub mod feature_card_ranking_within_status;
    pub mod feature_dashboard_enhancements;
    pub mod feature_invite_accept;
    pub mod feature_issue_change_history;
    pub mod feature_issue_edit_dialog;
    pub mod feature_issue_status_move;
    pub mod feature_machine_token_admin;
    pub mod feature_member_invites;
    pub mod feature_mwt_slice_01_coexist;
    pub mod feature_mwt_slice_02_web_boundary;
    pub mod feature_mwt_slice_03_api_auth_boundary;
    pub mod feature_mwt_slice_04_non_enumerability;
    pub mod feature_mwt_slice_05_migration_guarantee;
    pub mod feature_mwt_slice_06_provision_and_prove;
    pub mod feature_navigation_bar;
    pub mod feature_notification_delivery_providers;
    pub mod feature_per_workspace_backup;
    pub mod feature_recipient_notification_preferences;
    pub mod feature_remaining_surfaces;
    pub mod feature_token_management_api;
    pub mod feature_web_provisioning_flow;
    pub mod handler_instrumentation;
    pub mod keyboard_fragments_templating;
    pub mod slice_8_deferred_metrics;
    pub mod token_mutations_metric_export;
    pub mod us_01_install;
    pub mod us_02_multi_replica;
    pub mod us_03_backup_restore;
    pub mod us_04_rolling_upgrade;
    pub mod us_05_bootstrap;
    pub mod us_06_signin;
    pub mod us_07_project_create;
    pub mod us_08_file_issue;
    pub mod us_09_realtime_sse;
    pub mod us_10_comment_edit_delete;
    pub mod us_10_comments;
    pub mod us_10_tombstone_gc;
    pub mod us_11_attachments;
    pub mod us_12_keyboard_nav;
    pub mod us_13_contributor_onboarding;
}
