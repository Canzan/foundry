//! external-integration-plugins slice 05 — project emails (US-EIP-10,
//! ADR-EIP-007, DDR-15).
//!
//! - [`ProjectMail`]: the [`EmailTransport`] project emails go through, the
//!   first `smtp` or `email_api` provider the composition root's notifier
//!   holds (decision 4); none means project emails are unavailable (K-13).
//! - [`section`]: the Project emails section of `/account/settings`: only
//!   projects the member can open, with only the choices they made (OD-10),
//!   K-13 and no form when this Foundry cannot send email, and a note per
//!   muted workspace (the mute stays the master switch, D-9).
//! - [`save`]: `POST /account/settings/project-emails` (CSRF-checked): the
//!   member's choices replace theirs; a choice for a project they cannot
//!   open gets the uniform 404 and saves nothing.

use crate::bootstrap::SessionUser;
use crate::email_stop::StopLinks;
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use foundry_integrations::{email_events, EmailScope, EmailTransport, EventKind};
use foundry_services::integrations::{
    EmailChoiceInput, IntegrationError, ProjectEmails, SavedEmailChoice,
};
use foundry_services::Services;
use secrecy::SecretString;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use tower_sessions::Session;
use uuid::Uuid;

/// The address the section's form posts to.
pub const PROJECT_EMAILS_PATH: &str = "/account/settings/project-emails";

/// K-13: this Foundry has no email transport.
pub const NO_EMAIL_TRANSPORT: &str =
    "This Foundry cannot send email yet. An instance operator needs to set up email delivery.";

/// The transport project emails go through, and the secret their stop links
/// are signed with. Cloning shares both; it sends nothing until the
/// composition root hands it the notifier's transport, and its emails carry
/// no stop link until it hands it `SESSION_SECRET`.
#[derive(Clone, Default)]
pub struct ProjectMail {
    transport: Arc<RwLock<Option<Arc<dyn EmailTransport>>>>,
    stop_secret: Arc<RwLock<Option<Arc<SecretString>>>>,
}

impl ProjectMail {
    /// Send project emails through `transport` from now on (`None`: this
    /// Foundry cannot send email).
    pub fn use_transport(&self, transport: Option<Arc<dyn EmailTransport>>) {
        *self.transport.write().unwrap_or_else(|p| p.into_inner()) = transport;
    }

    /// The transport, when this Foundry can send email.
    pub fn transport(&self) -> Option<Arc<dyn EmailTransport>> {
        self.transport
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn can_send(&self) -> bool {
        self.transport().is_some()
    }

    /// Sign every project email's stop link with `secret` (ADR-EIP-007
    /// decision 5: `SESSION_SECRET`, as the stop doors verify it).
    pub fn sign_stop_links_with(&self, secret: Arc<SecretString>) {
        *self.stop_secret.write().unwrap_or_else(|p| p.into_inner()) = Some(secret);
    }

    /// The stop links of the choice `(user_id, project_id, event)` on the
    /// Foundry at `public_url`.
    pub fn stop_links(
        &self,
        public_url: &str,
        user_id: Uuid,
        project_id: Uuid,
        event: &str,
    ) -> Option<StopLinks> {
        let secret = self
            .stop_secret
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()?;
        StopLinks::sign(public_url, &secret, user_id, project_id, event)
    }
}

/// The Project emails section as `settings.html` renders it.
#[derive(Debug, Clone, Default)]
pub struct ProjectEmailsSection {
    pub can_send: bool,
    pub unavailable: &'static str,
    /// The confirmation of a save just made.
    pub saved: Option<String>,
    /// One line per muted workspace among the offered projects'.
    pub muted: Vec<String>,
    pub projects: Vec<ProjectChoices>,
}

/// One project's offered choices.
#[derive(Debug, Clone)]
pub struct ProjectChoices {
    pub name: String,
    pub events: Vec<EventChoices>,
}

/// One event's two scopes.
#[derive(Debug, Clone)]
pub struct EventChoices {
    pub label: &'static str,
    pub boxes: Vec<ChoiceBox>,
}

/// One tick box: `{project_id}:{event}:{scope}`.
#[derive(Debug, Clone)]
pub struct ChoiceBox {
    pub value: String,
    pub checked: bool,
    pub words: &'static str,
}

/// How a tick box names its scope.
fn box_words(scope: EmailScope) -> &'static str {
    match scope {
        EmailScope::All => "All issues",
        EmailScope::Mine => "Only issues I created or commented on",
    }
}

/// The section for a member whose `emails` these are, with `muted` their
/// muted workspaces.
pub fn section_of(
    emails: &ProjectEmails,
    muted: &HashSet<Uuid>,
    can_send: bool,
    saved: Option<String>,
) -> ProjectEmailsSection {
    let chosen = |project_id: Uuid, event: EventKind, scope: EmailScope| {
        emails.choices.iter().any(|c| {
            c.project_id == project_id
                && c.event == event.wire_name()
                && c.scope == scope.wire_name()
        })
    };
    let projects = emails
        .projects
        .iter()
        .map(|project| ProjectChoices {
            name: project.project_name.clone(),
            events: email_events()
                .map(|event| EventChoices {
                    label: event.display_name(),
                    boxes: EmailScope::ALL
                        .into_iter()
                        .map(|scope| ChoiceBox {
                            value: format!(
                                "{}:{}:{}",
                                project.project_id,
                                event.wire_name(),
                                scope.wire_name()
                            ),
                            checked: chosen(project.project_id, event, scope),
                            words: box_words(scope),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();
    ProjectEmailsSection {
        can_send,
        unavailable: NO_EMAIL_TRANSPORT,
        saved,
        muted: muted_lines(emails, muted),
        projects,
    }
}

/// One line per muted workspace among the offered projects', in their order.
fn muted_lines(emails: &ProjectEmails, muted: &HashSet<Uuid>) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for project in emails
        .projects
        .iter()
        .filter(|p| muted.contains(&p.workspace_id))
    {
        let line = format!(
            "{} is muted. These choices apply when you resubscribe.",
            project.workspace_name
        );
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines
}

/// The member's section, read now.
pub async fn section(
    state: &AppState,
    user_id: Uuid,
    muted: &HashSet<Uuid>,
    saved: Option<String>,
) -> Result<ProjectEmailsSection, IntegrationError> {
    let emails = Services::new(state.store.clone())
        .project_emails(user_id)
        .await?;
    let can_send = state.integrations.project_mail().can_send();
    Ok(section_of(&emails, muted, can_send, saved))
}

/// The submitted choices; `None` when any `choice` does not name a project,
/// an email event and a scope.
pub fn parse_choices(fields: &[(String, String)]) -> Option<Vec<EmailChoiceInput>> {
    fields
        .iter()
        .filter(|(name, _)| name == "choice")
        .map(|(_, value)| {
            let mut parts = value.splitn(3, ':');
            let project_id = parts.next()?.parse().ok()?;
            let event = EventKind::from_wire_name(parts.next()?)?;
            let scope = EmailScope::from_wire_name(parts.next()?)?;
            email_events()
                .any(|e| e == event)
                .then_some(EmailChoiceInput {
                    project_id,
                    event,
                    scope,
                })
        })
        .collect()
}

/// Whether every choice names a project the section offers.
fn offers_every(emails: &ProjectEmails, choices: &[EmailChoiceInput]) -> bool {
    choices.iter().all(|choice| {
        emails
            .projects
            .iter()
            .any(|project| project.project_id == choice.project_id)
    })
}

/// "Saved. Foundry will email {address} about: …" (US-EIP-10).
pub fn saved_line(address: &str, saved: &[SavedEmailChoice]) -> String {
    if saved.is_empty() {
        return "Saved. Foundry will not email you about any project.".to_string();
    }
    let about = saved
        .iter()
        .map(|c| {
            format!(
                "{} ({}) in {}",
                c.event.display_name(),
                c.scope.words(),
                c.project_name
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("Saved. Foundry will email {address} about: {about}.")
}

/// `POST /account/settings/project-emails`.
pub async fn save(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let Ok(Some(user)) = session.get::<SessionUser>(SESSION_KEY_USER_ID).await else {
        return crate::bootstrap::resource_not_found_page();
    };
    let Some(choices) = parse_choices(&fields) else {
        return crate::bootstrap::resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let saving = if state.integrations.project_mail().can_send() {
        services.save_project_emails(user.user_id, &choices).await
    } else {
        // K-13: nothing is saved; a choice for a project the member cannot
        // open is still the uniform 404.
        match services.project_emails(user.user_id).await {
            Ok(emails) if offers_every(&emails, &choices) => {
                return crate::unsubscribe::render_settings(&state, &user, &headers, None).await;
            }
            Ok(_) => Err(IntegrationError::NotFound),
            Err(err) => Err(err),
        }
    };
    let saved = match saving {
        Ok(saved) => saved,
        Err(IntegrationError::NotFound) => return crate::bootstrap::resource_not_found_page(),
        Err(IntegrationError::Store(err)) => {
            tracing::error!(%err, "project emails: save failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    let address = match state.store.find_user_email_lower_by_id(user.user_id).await {
        Ok(Some(address)) => address,
        _ => String::new(),
    };
    let line = saved_line(&address, &saved);
    crate::unsubscribe::render_settings(&state, &user, &headers, Some(line)).await
}
