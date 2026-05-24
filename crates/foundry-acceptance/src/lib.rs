//! foundry-acceptance — cucumber-rs world + step modules.
//!
//! Slice 1: only the US-01 docker-compose harness is wired. US-05..08
//! step modules land as those slices begin.

#![forbid(unsafe_code)]
#![allow(clippy::needless_return)]

pub mod support;
pub mod world;

pub mod steps {
    pub mod us_01_install;
    pub mod us_05_bootstrap;
    pub mod us_06_signin;
    pub mod us_07_project_create;
    pub mod us_08_file_issue;
    pub mod us_09_realtime_sse;
    pub mod us_10_comments;
    pub mod us_12_keyboard_nav;
}
