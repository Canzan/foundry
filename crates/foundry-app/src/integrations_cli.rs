//! `foundry integrations` operator subcommands, Slice 1.
//!
//! Configures what the web integrations pages configure through the same
//! services the pages call: workspace status and disconnect, each member's
//! email choices, and a project's routes (list, history, pause, resume,
//! edit, delete). The CLI acts as a named user (`--as <email>`), the same
//! identity the `set-role` command uses, and reads the same instance keys
//! and destination catalogues the server boots with.
//!
//! Not in this slice: creating a route and connecting Matrix (both need the
//! destination lookups the web handlers run), Slack install (browser login),
//! and the instance Slack app and secrets key.
//!
//! Output: `key: value` lines and a trailing `status: OK` on success. Exit
//! codes: 0 done; 2 usage or no match; 3 database or integration settings
//! unavailable; 4 a connection cannot be read (check the secrets key).

use crate::admin_cli::{resolve_workspace_selector, run_with_store};
use crate::integrations::config::IntegrationSettings;
use crate::integrations::key_file::InstanceKeyFile;
use crate::integrations::store_capabilities::StoreCapabilities;
use crate::integrations::Integrations;
use foundry_integrations::{EmailScope, EventKind};
use foundry_services::integrations::{
    DestinationCatalogs, EmailChoiceInput, IntegrationError, RouteAction, RouteEditDraft,
};
use foundry_services::{Principal, Services};
use foundry_store::Store;
use std::sync::Arc;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

const USAGE: &str = "usage:
  foundry integrations workspace status --workspace <id|name> --as <email>
  foundry integrations disconnect <slack|matrix> --workspace <id|name> --as <email> [--yes]
  foundry integrations email list --as <email>
  foundry integrations email save --as <email> --project <uuid> --event <event> --scope <all|mine>
  foundry integrations email stop --as <email> --project <uuid> --event <event>
  foundry integrations route list --workspace <id|name> --team <slug> --project <slug> --as <email>
  foundry integrations route history --workspace <id|name> --team <slug> --project <slug> --route <uuid> --as <email>
  foundry integrations route <pause|resume|delete> --workspace <id|name> --team <slug> --project <slug> --route <uuid> --as <email>
  foundry integrations route edit --workspace <id|name> --team <slug> --project <slug> --route <uuid> --events <a,b> --content-level <level> [--lanes <a,b>] --as <email>";

/// Entry point for `foundry integrations …`. `args` are the words after
/// `integrations`.
pub fn run(args: &[String]) -> i32 {
    match plan(args) {
        Ok(command) => command.execute(),
        Err(message) => {
            eprintln!("foundry integrations: {message}\n{USAGE}");
            2
        }
    }
}

/// A parsed command, ready to run against the live database.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    WorkspaceStatus {
        workspace: String,
        email: String,
    },
    Disconnect {
        kind: String,
        workspace: String,
        email: String,
        confirmed: bool,
    },
    EmailList {
        email: String,
    },
    EmailSave {
        email: String,
        project: Uuid,
        event: EventKind,
        scope: EmailScope,
    },
    EmailStop {
        email: String,
        project: Uuid,
        event: String,
    },
    RouteList {
        scope: ProjectScope,
        email: String,
    },
    RouteHistory {
        scope: ProjectScope,
        route: Uuid,
        email: String,
    },
    RouteChange {
        scope: ProjectScope,
        route: Uuid,
        action: ChangeKind,
        email: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
struct ProjectScope {
    workspace: String,
    team: String,
    project: String,
}

#[derive(Debug, PartialEq, Eq)]
enum ChangeKind {
    Pause,
    Resume,
    Delete,
    Edit {
        events: Vec<String>,
        content_level: String,
        lanes: Vec<String>,
    },
}

impl Command {
    /// The command's name as its messages print it.
    fn name(&self) -> &'static str {
        match self {
            Command::WorkspaceStatus { .. } => "foundry integrations workspace status",
            Command::Disconnect { .. } => "foundry integrations disconnect",
            Command::EmailList { .. } => "foundry integrations email list",
            Command::EmailSave { .. } => "foundry integrations email save",
            Command::EmailStop { .. } => "foundry integrations email stop",
            Command::RouteList { .. } => "foundry integrations route list",
            Command::RouteHistory { .. } => "foundry integrations route history",
            Command::RouteChange { action, .. } => match action {
                ChangeKind::Pause => "foundry integrations route pause",
                ChangeKind::Resume => "foundry integrations route resume",
                ChangeKind::Delete => "foundry integrations route delete",
                ChangeKind::Edit { .. } => "foundry integrations route edit",
            },
        }
    }

    fn execute(self) -> i32 {
        let name = self.name();
        run_with_store(name, move |store| async move { perform(self, store).await })
    }
}

/// Parse the words after `integrations` into a [`Command`].
fn plan(args: &[String]) -> Result<Command, String> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["workspace", "status", rest @ ..] => {
            let flags = Flags::parse(rest)?;
            Ok(Command::WorkspaceStatus {
                workspace: flags.required("--workspace")?,
                email: flags.required("--as")?,
            })
        }
        ["disconnect", kind, rest @ ..] => {
            if *kind != "slack" && *kind != "matrix" {
                return Err(format!("disconnect takes slack or matrix, not {kind:?}"));
            }
            let flags = Flags::parse(rest)?;
            Ok(Command::Disconnect {
                kind: kind.to_string(),
                workspace: flags.required("--workspace")?,
                email: flags.required("--as")?,
                confirmed: flags.present("--yes"),
            })
        }
        ["email", "list", rest @ ..] => {
            let flags = Flags::parse(rest)?;
            Ok(Command::EmailList {
                email: flags.required("--as")?,
            })
        }
        ["email", "save", rest @ ..] => {
            let flags = Flags::parse(rest)?;
            let event = flags.required("--event")?;
            let scope = flags.required("--scope")?;
            Ok(Command::EmailSave {
                email: flags.required("--as")?,
                project: uuid_flag(&flags, "--project")?,
                event: EventKind::from_wire_name(&event)
                    .ok_or_else(|| format!("unknown event {event:?}"))?,
                scope: EmailScope::from_wire_name(&scope)
                    .ok_or_else(|| format!("scope must be all or mine, not {scope:?}"))?,
            })
        }
        ["email", "stop", rest @ ..] => {
            let flags = Flags::parse(rest)?;
            Ok(Command::EmailStop {
                email: flags.required("--as")?,
                project: uuid_flag(&flags, "--project")?,
                event: flags.required("--event")?,
            })
        }
        ["route", verb @ ("list" | "history" | "pause" | "resume" | "delete" | "edit"), rest @ ..] =>
        {
            let flags = Flags::parse(rest)?;
            let scope = ProjectScope {
                workspace: flags.required("--workspace")?,
                team: flags.required("--team")?,
                project: flags.required("--project")?,
            };
            let email = flags.required("--as")?;
            match *verb {
                "list" => Ok(Command::RouteList { scope, email }),
                "history" => Ok(Command::RouteHistory {
                    route: uuid_flag(&flags, "--route")?,
                    scope,
                    email,
                }),
                "pause" | "resume" | "delete" => Ok(Command::RouteChange {
                    route: uuid_flag(&flags, "--route")?,
                    action: match *verb {
                        "pause" => ChangeKind::Pause,
                        "resume" => ChangeKind::Resume,
                        _ => ChangeKind::Delete,
                    },
                    scope,
                    email,
                }),
                _ => Ok(Command::RouteChange {
                    route: uuid_flag(&flags, "--route")?,
                    action: ChangeKind::Edit {
                        events: list_flag(&flags, "--events")?,
                        content_level: flags.required("--content-level")?,
                        lanes: flags
                            .optional("--lanes")
                            .map(split_list)
                            .unwrap_or_default(),
                    },
                    scope,
                    email,
                }),
            }
        }
        _ => Err("unknown command".to_string()),
    }
}

/// `--name value` pairs and bare `--name` switches, in any order.
struct Flags<'a> {
    pairs: Vec<(&'a str, Option<&'a str>)>,
}

impl<'a> Flags<'a> {
    fn parse(words: &[&'a str]) -> Result<Self, String> {
        let mut pairs = Vec::new();
        let mut index = 0;
        while index < words.len() {
            let word = words[index];
            if !word.starts_with("--") {
                return Err(format!("unexpected argument {word:?}"));
            }
            // A switch (`--yes`) takes no value; any other flag takes the next word.
            let value = match words.get(index + 1) {
                Some(next) if !next.starts_with("--") => {
                    index += 1;
                    Some(*next)
                }
                _ => None,
            };
            pairs.push((word, value));
            index += 1;
        }
        Ok(Flags { pairs })
    }

    fn optional(&self, name: &str) -> Option<&'a str> {
        self.pairs
            .iter()
            .find(|(flag, _)| *flag == name)
            .and_then(|(_, value)| *value)
    }

    fn required(&self, name: &str) -> Result<String, String> {
        self.optional(name)
            .map(str::to_string)
            .ok_or_else(|| format!("{name} is required"))
    }

    fn present(&self, name: &str) -> bool {
        self.pairs.iter().any(|(flag, _)| *flag == name)
    }
}

fn uuid_flag(flags: &Flags<'_>, name: &str) -> Result<Uuid, String> {
    let raw = flags.required(name)?;
    Uuid::parse_str(&raw).map_err(|_| format!("{name} must be a uuid, not {raw:?}"))
}

fn list_flag(flags: &Flags<'_>, name: &str) -> Result<Vec<String>, String> {
    let items = split_list(&flags.required(name)?);
    if items.is_empty() {
        return Err(format!("{name} needs at least one value"));
    }
    Ok(items)
}

fn split_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Run one parsed command against the live store. Prints `key: value`
/// lines and the closing `status: OK`.
async fn perform(command: Command, store: Store) -> i32 {
    let name = command.name();
    let store = Arc::new(store);
    let services = Services::new(store.clone());
    match command {
        Command::EmailList { email } => match user_id(&store, &email).await {
            Ok(user) => email_list(&services, user, name).await,
            Err(code) => code,
        },
        Command::EmailSave {
            email,
            project,
            event,
            scope,
        } => match user_id(&store, &email).await {
            Ok(user) => {
                let input = [EmailChoiceInput {
                    project_id: project,
                    event,
                    scope,
                }];
                match services.save_project_emails(user, &input).await {
                    Ok(saved) => {
                        for choice in saved {
                            println!(
                                "saved: {} · {} · {}",
                                choice.project_name,
                                choice.event.wire_name(),
                                choice.scope.wire_name()
                            );
                        }
                        println!("status: OK");
                        0
                    }
                    Err(err) => fail(name, err),
                }
            }
            Err(code) => code,
        },
        Command::EmailStop {
            email,
            project,
            event,
        } => match user_id(&store, &email).await {
            Ok(user) => match services.stop_project_email(user, project, &event).await {
                Ok(stop) => {
                    println!(
                        "stopped: {} · {}",
                        stop.project_name,
                        stop.event.wire_name()
                    );
                    println!("status: OK");
                    0
                }
                Err(err) => fail(name, err),
            },
            Err(code) => code,
        },
        Command::WorkspaceStatus { workspace, email } => {
            match principal(&store, &workspace, &email).await {
                Ok(principal) => match services.workspace_integrations(&principal).await {
                    Ok(view) => {
                        println!(
                            "slack: {}",
                            view.slack.as_deref().unwrap_or("not connected")
                        );
                        println!(
                            "slack-lost: {}",
                            if view.slack_lost.is_some() {
                                "yes"
                            } else {
                                "no"
                            }
                        );
                        match view.matrix {
                            Some(matrix) => {
                                println!("matrix: {}", matrix.connected);
                                println!("matrix-token: {}", matrix.token_saved);
                            }
                            None => println!("matrix: not connected"),
                        }
                        println!("status: OK");
                        0
                    }
                    Err(err) => fail(name, err),
                },
                Err(code) => code,
            }
        }
        Command::Disconnect {
            kind,
            workspace,
            email,
            confirmed,
        } => match principal(&store, &workspace, &email).await {
            Ok(principal) => disconnect(&services, &principal, &kind, confirmed, name).await,
            Err(code) => code,
        },
        Command::RouteList { scope, email } => {
            match principal(&store, &scope.workspace, &email).await {
                Ok(principal) => match services
                    .project_integrations(&principal, &scope.team, &scope.project)
                    .await
                {
                    Ok(view) => {
                        println!("project: {}", view.project_name);
                        for route in &view.routes {
                            println!(
                                "route: {} · {} · {} · {}",
                                route.id,
                                route.kind,
                                route.destination_label,
                                if route.paused { "paused" } else { "live" }
                            );
                        }
                        println!("status: OK");
                        0
                    }
                    Err(err) => fail(name, err),
                },
                Err(code) => code,
            }
        }
        Command::RouteHistory {
            scope,
            route,
            email,
        } => match principal(&store, &scope.workspace, &email).await {
            Ok(principal) => match services
                .route_history(&principal, &scope.team, &scope.project, route)
                .await
            {
                Ok(history) => {
                    println!("project: {}", history.project_name);
                    println!("route: {} · {}", history.kind, history.destination_label);
                    for row in &history.deliveries {
                        let at = row.occurred_at.format(&Rfc3339).unwrap_or_default();
                        println!(
                            "delivery: {at} · {} · {} · attempts {}",
                            row.event_kind, row.status, row.attempts
                        );
                    }
                    println!("status: OK");
                    0
                }
                Err(err) => fail(name, err),
            },
            Err(code) => code,
        },
        Command::RouteChange {
            scope,
            route,
            action,
            email,
        } => match principal(&store, &scope.workspace, &email).await {
            Ok(principal) => {
                let integrations = match boot(store.clone()).await {
                    Ok(integrations) => integrations,
                    Err(code) => return code,
                };
                let action = match action {
                    ChangeKind::Pause => RouteAction::Pause,
                    ChangeKind::Resume => RouteAction::Resume,
                    ChangeKind::Delete => RouteAction::Delete,
                    ChangeKind::Edit {
                        events,
                        content_level,
                        lanes,
                    } => RouteAction::Edit(RouteEditDraft {
                        events,
                        content_level,
                        lanes,
                    }),
                };
                let catalogs: &dyn DestinationCatalogs = &integrations;
                match services
                    .change_route(
                        &principal,
                        &scope.team,
                        &scope.project,
                        route,
                        action,
                        catalogs,
                        OffsetDateTime::now_utc(),
                    )
                    .await
                {
                    Ok(outcome) => {
                        println!("route: {route}");
                        println!("outcome: {}", describe_route_outcome(&outcome));
                        println!("status: OK");
                        0
                    }
                    Err(err) => fail(name, err),
                }
            }
            Err(code) => code,
        },
    }
}

fn describe_route_outcome(outcome: &foundry_store::integrations::RouteChangeOutcome) -> String {
    use foundry_store::integrations::RouteChangeOutcome::*;
    match outcome {
        Changed { cancelled } => format!("changed, {cancelled} pending deliveries cancelled"),
        Unchanged => "unchanged".to_string(),
        ProjectDeleted => "project deleted".to_string(),
    }
}

async fn email_list(services: &Services, user: Uuid, name: &str) -> i32 {
    match services.project_emails(user).await {
        Ok(emails) => {
            for project in &emails.projects {
                println!(
                    "project: {} · {} · {}",
                    project.project_id, project.workspace_name, project.project_name
                );
            }
            for choice in &emails.choices {
                println!(
                    "choice: {} · {} · {}",
                    choice.project_id, choice.event, choice.scope
                );
            }
            println!("status: OK");
            0
        }
        Err(err) => fail(name, err),
    }
}

async fn disconnect(
    services: &Services,
    principal: &Principal,
    kind: &str,
    confirmed: bool,
    name: &str,
) -> i32 {
    let preview = match services.disconnect_preview(principal, kind).await {
        Ok(Some(preview)) => preview,
        Ok(None) => {
            println!("kind: {kind}");
            println!("outcome: not connected");
            println!("status: OK");
            return 0;
        }
        Err(err) => return fail(name, err),
    };
    println!("kind: {kind} · {}", preview.label);
    println!("routes: {}", preview.routes);
    println!("projects: {}", preview.projects);
    if !confirmed {
        println!("outcome: preview only, pass --yes to disconnect");
        println!("status: OK");
        return 0;
    }
    match services
        .disconnect(principal, kind, OffsetDateTime::now_utc())
        .await
    {
        Ok(foundry_store::integrations::DisconnectOutcome::Disconnected { cancelled, .. }) => {
            println!("outcome: disconnected, {cancelled} pending deliveries cancelled");
            println!(
                "note: the vendor-side token is not revoked here; revoke it in {}",
                if kind == "slack" { "Slack" } else { "Matrix" }
            );
            println!("status: OK");
            0
        }
        Ok(_) => {
            println!("outcome: not connected");
            println!("status: OK");
            0
        }
        Err(err) => fail(name, err),
    }
}

/// The acting user's id, by email. Exit 2 when no such user exists.
async fn user_id(store: &Store, email: &str) -> Result<Uuid, i32> {
    match store.user_id_by_email(&email.to_ascii_lowercase()).await {
        Ok(Some(id)) => Ok(id),
        Ok(None) => {
            eprintln!("foundry integrations: no user matches {email:?}");
            Err(2)
        }
        Err(err) => {
            eprintln!("foundry integrations: could not resolve {email:?}: {err}");
            Err(3)
        }
    }
}

/// The acting user acting within a workspace: both selectors resolve here.
async fn principal(store: &Store, workspace: &str, email: &str) -> Result<Principal, i32> {
    let workspaces = store.list_workspaces().await.map_err(|err| {
        eprintln!("foundry integrations: could not list workspaces: {err}");
        3
    })?;
    let Some((workspace_id, _)) = resolve_workspace_selector(&workspaces, workspace) else {
        eprintln!(
            "foundry integrations: no workspace matches {workspace:?}; \
             run `foundry doctor list-workspaces` to see each workspace's id and name."
        );
        return Err(2);
    };
    let user_id = user_id(store, email).await?;
    Ok(Principal::Human {
        user_id,
        workspace_id: *workspace_id,
    })
}

/// The integrations as the server boots them: the same settings, instance
/// keys and key file, so catalogues can open saved credentials.
async fn boot(store: Arc<Store>) -> Result<Integrations, i32> {
    let public_url = crate::integrations::config::public_url_or(
        |name| std::env::var(name).ok(),
        "http://localhost:8080",
    );
    let settings = IntegrationSettings::from_env(&public_url).map_err(|err| {
        eprintln!("foundry integrations: integration settings refused: {err}");
        3
    })?;
    let probe = StoreCapabilities::new(store);
    let key_file = InstanceKeyFile::of(&settings);
    Integrations::boot(settings, &*probe, &key_file, OffsetDateTime::now_utc())
        .await
        .map_err(|err| {
            eprintln!("foundry integrations: the Slack app's client could not be built: {err}");
            3
        })
}

/// Print a service failure and return its exit code.
fn fail(name: &str, err: IntegrationError) -> i32 {
    match err {
        IntegrationError::NotFound => {
            eprintln!("{name}: not found, or you are not an admin of this workspace");
            2
        }
        IntegrationError::Unavailable(kind) => {
            eprintln!(
                "{name}: the {kind} connection cannot be read; \
                 check the secrets key (see foundry doctor rotate-secrets)"
            );
            4
        }
        IntegrationError::Store(err) => {
            eprintln!("{name}: {err}");
            3
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn a_switch_takes_no_value_and_a_flag_takes_the_next_word() {
        let command = plan(&words(
            "disconnect slack --workspace Canzan --as a@b.c --yes",
        ))
        .unwrap();
        assert_eq!(
            command,
            Command::Disconnect {
                kind: "slack".into(),
                workspace: "Canzan".into(),
                email: "a@b.c".into(),
                confirmed: true,
            }
        );
    }

    #[test]
    fn disconnect_takes_only_slack_or_matrix() {
        let err = plan(&words("disconnect teams --workspace W --as a@b.c")).unwrap_err();
        assert!(err.contains("slack or matrix"), "{err}");
    }

    #[test]
    fn a_missing_required_flag_names_it() {
        let err = plan(&words("route list --team t --project p --as a@b.c")).unwrap_err();
        assert_eq!(err, "--workspace is required");
    }

    #[test]
    fn route_edit_reads_events_and_optional_lanes() {
        let command = plan(&words(
            "route edit --workspace W --team t --project p --route 0195a1b2-0000-7000-8000-000000000001 --events issue.created,issue.moved --content-level title --as a@b.c",
        ))
        .unwrap();
        match command {
            Command::RouteChange {
                action:
                    ChangeKind::Edit {
                        events,
                        content_level,
                        lanes,
                    },
                ..
            } => {
                assert_eq!(events, vec!["issue.created", "issue.moved"]);
                assert_eq!(content_level, "title");
                assert!(lanes.is_empty());
            }
            other => panic!("expected an edit, got {other:?}"),
        }
    }

    #[test]
    fn email_save_rejects_an_unknown_scope() {
        let err = plan(&words(
            "email save --as a@b.c --project 0195a1b2-0000-7000-8000-000000000001 --event issue.created --scope everyone",
        ))
        .unwrap_err();
        assert!(err.contains("scope must be all or mine"), "{err}");
    }
}
