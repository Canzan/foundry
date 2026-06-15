//! Email port + a slice-1 implementation backed by `lettre`.
//!
//! For acceptance tests we use [`FakeEmailSender`] which records every
//! send in memory. Production wiring (slice-1 binary) keeps the real
//! lettre SMTP transport behind an env-config gate.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SentEmail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[async_trait]
pub trait EmailSender: Send + Sync + Debug + 'static {
    async fn send(&self, to: &str, subject: &str, body: &str) -> anyhow::Result<()>;
}

/// Disabled sender. Returns Ok without doing anything. Slice-1 binary
/// uses this when SMTP env vars are unset.
#[derive(Debug, Default, Clone)]
pub struct NoopEmailSender;

#[async_trait]
impl EmailSender for NoopEmailSender {
    async fn send(&self, _to: &str, _subject: &str, _body: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Test-only recorder. The acceptance harness reads `sent()` to assert
/// "exactly one email was sent to mei@acme.com".
#[derive(Debug, Default)]
pub struct FakeEmailSender {
    inner: Mutex<Vec<SentEmail>>,
    /// When set, every `send` returns `Err` WITHOUT recording — models a mail
    /// service outage so a best-effort sender's non-fatal failure path can be
    /// exercised (workspace-member-invites US-01, AC-01.4: the shareable link is
    /// still shown when the email fails to send).
    failing: AtomicBool,
}

impl FakeEmailSender {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Put the sender into failure mode: every subsequent `send` returns `Err`
    /// and records nothing (the real SMTP transport would likewise record no
    /// send on an outage).
    pub fn set_failing(&self) {
        self.failing.store(true, Ordering::SeqCst);
    }

    pub fn sent(&self) -> Vec<SentEmail> {
        self.inner.lock().expect("FakeEmailSender mutex").clone()
    }

    pub fn count_to(&self, addr: &str) -> usize {
        self.sent().iter().filter(|e| e.to == addr).count()
    }

    pub fn last_to(&self, addr: &str) -> Option<SentEmail> {
        self.sent().into_iter().rev().find(|e| e.to == addr)
    }
}

#[async_trait]
impl EmailSender for FakeEmailSender {
    async fn send(&self, to: &str, subject: &str, body: &str) -> anyhow::Result<()> {
        if self.failing.load(Ordering::SeqCst) {
            anyhow::bail!("FakeEmailSender: mail service unavailable (test-induced)");
        }
        self.inner
            .lock()
            .expect("FakeEmailSender mutex")
            .push(SentEmail {
                to: to.to_string(),
                subject: subject.to_string(),
                body: body.to_string(),
            });
        Ok(())
    }
}
