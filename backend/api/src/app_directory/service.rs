use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::{
    AppDirectory, AppRequest, AvailableApp, BotAppRow, ChannelRow, ConnectedAppRow,
    DirectoryChannel, Health, HealthState, HookRow, InstalledApp, InstalledKind, Permission,
    WorkflowRow,
};
use crate::authz;
use crate::bots::models::Scope;
use crate::hooks::models::HookType;
use crate::integrations::models::Provider;
use crate::notifications::models::NotificationType;
use crate::state::AppState;
use crate::workflows::models::{Definition, RunStatus, Step, Trigger};
use crate::workspace::models::WorkspaceRole;

struct Draft {
    app: InstalledApp,
    channel_ids: Vec<Uuid>,
}

pub async fn directory(
    state: &AppState,
    workspace_id: Uuid,
    viewer: Uuid,
) -> AppResult<AppDirectory> {
    let member = authz::require_workspace_member(state, workspace_id, viewer).await?;
    let can_manage = member.role.has_at_least(&WorkspaceRole::Admin);
    let repo = &state.app_directory_repo;

    let connected = repo.connected_apps(workspace_id).await?;
    let installed_providers: HashSet<String> =
        connected.iter().map(|row| row.provider.clone()).collect();

    let mut drafts: Vec<Draft> = connected.into_iter().filter_map(connected_app).collect();
    drafts.extend(repo.bot_apps(workspace_id).await?.into_iter().map(bot_app));
    drafts.extend(repo.hooks(workspace_id).await?.into_iter().map(hook));
    drafts.extend(
        repo.workflows(workspace_id)
            .await?
            .into_iter()
            .map(workflow),
    );

    let wanted: Vec<Uuid> = drafts
        .iter()
        .flat_map(|d| d.channel_ids.iter().copied())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let channels: HashMap<Uuid, ChannelRow> = repo
        .channels(workspace_id, &wanted, viewer)
        .await?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();

    let installed = drafts
        .into_iter()
        .map(|draft| place(draft, &channels))
        .filter(|(app, touches_visible)| {
            can_manage
                || matches!(
                    app.kind,
                    InstalledKind::ConnectedApp | InstalledKind::BotApp
                )
                || app.every_channel
                || app.added_by == Some(viewer)
                || *touches_visible
        })
        .map(|(app, _)| app)
        .collect();

    let available = Provider::ALL
        .into_iter()
        .map(|provider| AvailableApp {
            provider: provider.as_str().to_string(),
            name: provider.name().to_string(),
            summary: provider.summary().to_string(),
            permissions: provider_permissions(provider),
            installed: installed_providers.contains(provider.as_str()),
        })
        .collect();

    let bot_scopes = Scope::ALL
        .into_iter()
        .map(|scope| Permission::new(scope.as_str(), scope.text()))
        .collect();

    Ok(AppDirectory {
        can_manage,
        available,
        installed,
        bot_scopes,
    })
}

pub async fn request(
    state: &AppState,
    workspace_id: Uuid,
    requester: Uuid,
    req: &AppRequest,
) -> AppResult<()> {
    authz::require_workspace_role(state, workspace_id, requester, &WorkspaceRole::Member).await?;
    let who = state
        .auth_service
        .repo()
        .find_by_id(requester)
        .await?
        .and_then(|u| u.display_name)
        .unwrap_or_else(|| "A member".into());

    let title = format!("{who} asked for an app");
    let name = req.name.trim();
    let body = match req
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty())
    {
        Some(reason) => format!("{name}: {reason}"),
        None => name.to_string(),
    };
    let data = serde_json::json!({ "app_request": name, "requested_by": requester });

    for admin in state.app_directory_repo.admins(workspace_id).await? {
        if admin == requester {
            continue;
        }
        state
            .notification_repo
            .create(
                admin,
                workspace_id,
                &NotificationType::System,
                &title,
                Some(&body),
                &data,
            )
            .await?;
        let event = serde_json::json!({
            "user_id": admin.to_string(),
            "workspace_id": workspace_id.to_string(),
            "title": title,
            "body": body,
            "kind": "system",
        });
        let _ = state.publisher.publish("notification.push", event).await;
    }
    Ok(())
}

fn place(draft: Draft, channels: &HashMap<Uuid, ChannelRow>) -> (InstalledApp, bool) {
    let Draft {
        mut app,
        channel_ids,
    } = draft;
    let found: Vec<&ChannelRow> = channel_ids
        .iter()
        .filter_map(|id| channels.get(id))
        .collect();
    app.channels = found
        .iter()
        .filter(|c| c.visible)
        .map(|c| DirectoryChannel {
            id: c.id,
            name: c.name.clone(),
        })
        .collect();
    app.hidden_channels = (found.len() - app.channels.len()) as i64;
    let touches_visible = !app.channels.is_empty();
    (app, touches_visible)
}

fn installed(kind: InstalledKind, id: String, name: String) -> InstalledApp {
    InstalledApp {
        kind,
        id,
        name,
        description: None,
        added_by: None,
        added_by_name: None,
        added_at: chrono::Utc::now(),
        permissions: Vec::new(),
        every_channel: false,
        channels: Vec::new(),
        hidden_channels: 0,
        health: Health::ok(),
        app_id: None,
    }
}

fn provider_permissions(provider: Provider) -> Vec<Permission> {
    provider
        .permissions()
        .iter()
        .enumerate()
        .map(|(i, text)| Permission::new(&format!("{}.{i}", provider.as_str()), *text))
        .collect()
}

fn connected_app(row: ConnectedAppRow) -> Option<Draft> {
    let provider = Provider::parse(&row.provider)?;
    let mut app = installed(
        InstalledKind::ConnectedApp,
        row.provider,
        provider.name().to_string(),
    );
    app.description = Some(provider.summary().to_string());
    app.added_by = row.configured_by;
    app.added_by_name = row.configured_by_name;
    app.added_at = row.configured_at;
    app.permissions = provider_permissions(provider);
    app.every_channel = true;
    app.health = if row.enabled {
        Health::ok()
    } else {
        Health::off()
    };
    Some(Draft {
        app,
        channel_ids: Vec::new(),
    })
}

fn bot_app(row: BotAppRow) -> Draft {
    let mut app = installed(InstalledKind::BotApp, row.id.to_string(), row.name);
    app.description = row.description;
    app.added_by = row.created_by;
    app.added_by_name = row.created_by_name;
    app.added_at = row.created_at;
    app.permissions = row
        .scopes
        .iter()
        .map(|scope| {
            let text = Scope::parse(scope).map_or_else(|| scope.clone(), |s| s.text().to_string());
            Permission::new(scope, text)
        })
        .collect();
    if let Some(since) = row.failing_since {
        app.health = Health {
            state: HealthState::Failing,
            reason: row.last_failure,
            since: Some(since),
        };
    }
    Draft {
        app,
        channel_ids: row.channel_ids,
    }
}

fn host_of(config: &serde_json::Value) -> String {
    config
        .get("url")
        .and_then(|v| v.as_str())
        .and_then(|url| reqwest::Url::parse(url).ok())
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "a service outside this workspace".into())
}

fn ids_in(value: Option<&serde_json::Value>) -> Vec<Uuid> {
    match value {
        Some(serde_json::Value::String(id)) => id.parse().into_iter().collect(),
        Some(serde_json::Value::Array(ids)) => ids
            .iter()
            .filter_map(|v| v.as_str().and_then(|id| id.parse().ok()))
            .collect(),
        _ => Vec::new(),
    }
}

fn hook(row: HookRow) -> Draft {
    let (kind, channel_ids, permissions) = match row.hook_type {
        HookType::IncomingWebhook => (
            InstalledKind::IncomingWebhook,
            ids_in(row.config.get("channel_id")),
            vec![Permission::new(
                "post",
                "Anyone with its URL can post messages into its channel",
            )],
        ),
        HookType::OutgoingWebhook => (
            InstalledKind::OutgoingWebhook,
            ids_in(row.config.get("channel_ids")),
            vec![Permission::new(
                "send",
                format!(
                    "Sends every message posted in its channels to {}",
                    host_of(&row.config)
                ),
            )],
        ),
        HookType::SlashCommand => {
            let command = row
                .config
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            (
                InstalledKind::SlashCommand,
                ids_in(row.config.get("channel_ids")),
                vec![
                    Permission::new(
                        "send",
                        format!(
                            "Sends what people type after /{command} to {}",
                            host_of(&row.config)
                        ),
                    ),
                    Permission::new("answer", "Answers in the channel it was used in"),
                ],
            )
        }
    };
    let mut app = installed(kind, row.id.to_string(), row.name);
    app.description = row.description;
    app.added_by = Some(row.created_by);
    app.added_by_name = row.created_by_name;
    app.added_at = row.created_at;
    app.permissions = permissions;
    app.every_channel = kind == InstalledKind::SlashCommand && channel_ids.is_empty();
    app.app_id = row.app_id;
    app.health = match (row.is_active, row.disabled_reason) {
        (true, _) => Health::ok(),
        (false, Some(reason)) => Health {
            state: HealthState::Disabled,
            reason: Some(reason),
            since: Some(row.updated_at),
        },
        (false, None) => Health::off(),
    };
    Draft { app, channel_ids }
}

fn trigger_text(trigger: &Trigger) -> (&'static str, String) {
    match trigger {
        Trigger::Schedule { .. } => ("schedule", "Runs on a schedule".into()),
        Trigger::Webhook { .. } => ("webhook", "Runs when its webhook URL is called".into()),
        Trigger::ReactionAdded { emoji, .. } => (
            "reaction",
            format!("Runs when someone reacts with :{emoji}: in its channel"),
        ),
        Trigger::MemberJoined { .. } => ("joined", "Runs when someone joins its channel".into()),
        Trigger::Shortcut { .. } => ("shortcut", "Runs from a shortcut in its channel".into()),
    }
}

fn step_text(step: &Step) -> (&'static str, &'static str) {
    match step {
        Step::SendMessage { .. } => ("send_message", "Posts messages into channels"),
        Step::SendDm { .. } => ("send_dm", "Sends direct messages"),
        Step::Form { .. } => ("form", "Asks people to fill in a form"),
        Step::AddListItem { .. } => ("add_list_item", "Adds items to a list"),
        Step::UpdateListItem { .. } => ("update_list_item", "Changes items in a list"),
    }
}

fn trigger_channel(trigger: &Trigger) -> Option<Uuid> {
    match trigger {
        Trigger::ReactionAdded { channel_id, .. }
        | Trigger::MemberJoined { channel_id }
        | Trigger::Shortcut { channel_id } => Some(*channel_id),
        Trigger::Schedule { .. } | Trigger::Webhook { .. } => None,
    }
}

fn workflow(row: WorkflowRow) -> Draft {
    let mut app = installed(InstalledKind::Workflow, row.id.to_string(), row.name);
    app.description = row.description;
    app.added_by = Some(row.created_by);
    app.added_by_name = row.created_by_name;
    app.added_at = row.created_at;

    let mut channel_ids = Vec::new();
    if let Ok(definition) = serde_json::from_value::<Definition>(row.definition) {
        let (key, text) = trigger_text(&definition.trigger);
        app.permissions.push(Permission::new(key, text));
        channel_ids.extend(trigger_channel(&definition.trigger));
        let mut seen = HashSet::new();
        for step in &definition.steps {
            if let Step::SendMessage { channel_id, .. } = step {
                channel_ids.push(*channel_id);
            }
            let (key, text) = step_text(step);
            if seen.insert(key) {
                app.permissions.push(Permission::new(key, text));
            }
        }
    }
    channel_ids.sort();
    channel_ids.dedup();

    app.health = match (row.enabled, row.last_status) {
        (false, _) => Health::off(),
        (true, Some(RunStatus::Failed)) => Health {
            state: HealthState::Failing,
            reason: row.last_error,
            since: row.last_started_at,
        },
        (true, _) => Health::ok(),
    };
    Draft { app, channel_ids }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_come_from_a_single_id_or_a_list() {
        let one = Uuid::new_v4();
        let two = Uuid::new_v4();
        assert_eq!(ids_in(Some(&serde_json::json!(one.to_string()))), vec![one]);
        assert_eq!(
            ids_in(Some(&serde_json::json!([
                one.to_string(),
                "nope",
                two.to_string()
            ]))),
            vec![one, two]
        );
        assert!(ids_in(None).is_empty());
    }

    #[test]
    fn a_hook_names_only_the_host_it_sends_to() {
        let config = serde_json::json!({ "url": "https://hooks.example.com/secret/path?token=x" });
        assert_eq!(host_of(&config), "hooks.example.com");
        assert_eq!(
            host_of(&serde_json::json!({})),
            "a service outside this workspace"
        );
    }

    #[test]
    fn every_bot_scope_has_plain_words() {
        for scope in Scope::ALL {
            assert!(!scope.text().is_empty(), "{} has no text", scope.as_str());
        }
    }
}
