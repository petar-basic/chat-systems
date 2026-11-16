use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use uuid::Uuid;

use shared_common::errors::{is_unique_violation, AppError, AppResult};

use super::models::{
    MembersQuery, NewProfileField, ProfileField, ProfileFieldKind, ProfileFieldOption,
    ProfileFieldOptionInput, ProfileValues, SetProfileValues, UpdateProfileField, MAX_FIELDS,
    MAX_HINT, MAX_LABEL, MAX_LINK, MAX_ON_CARD, MAX_OPTIONS, MAX_OPTION_LABEL, MAX_TEXT,
};
use super::repo::{FieldDraft, ProfileFieldRepo};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::state::AppState;
use crate::workspace::models::{MemberWithUser, WorkspaceRole};

pub async fn list(state: &AppState, workspace_id: Uuid) -> AppResult<Vec<ProfileField>> {
    Ok(state.profile_field_repo.list(workspace_id).await?)
}

pub async fn create(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    req: &NewProfileField,
    ip: &ClientIp,
) -> AppResult<ProfileField> {
    authz::require_workspace_role(state, workspace_id, actor, &WorkspaceRole::Admin).await?;
    let label = label_of(&req.label)?;
    let hint = hint_of(req.hint.as_deref())?;
    let options = options_of(req.kind, &label, &req.options, &[])?;
    let draft = FieldDraft {
        label: &label,
        hint: hint.as_deref(),
        kind: req.kind,
        options: &options,
        admins_only: req.admins_only,
        on_card: req.on_card,
    };

    let mut tx = state.pool.begin().await?;
    ProfileFieldRepo::lock_workspace_in(&mut tx, workspace_id).await?;
    if ProfileFieldRepo::count_in(&mut tx, workspace_id).await? >= MAX_FIELDS {
        return Err(AppError::Conflict(format!(
            "A workspace has at most {MAX_FIELDS} profile fields"
        )));
    }
    if req.on_card
        && ProfileFieldRepo::on_card_count_in(&mut tx, workspace_id, None).await? >= MAX_ON_CARD
    {
        return Err(card_full());
    }
    let id = ProfileFieldRepo::create_in(&mut tx, workspace_id, &draft)
        .await
        .map_err(|e| taken(e, &label))?;
    tx.commit().await?;

    audit::record(
        state,
        AuditEntry::new(AuditAction::ProfileFieldCreated, actor)
            .workspace(workspace_id)
            .resource(id)
            .ip(ip)
            .details(serde_json::json!({ "label": label, "kind": req.kind })),
    )
    .await;
    found(state, workspace_id, id).await
}

pub async fn update(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    field_id: Uuid,
    req: &UpdateProfileField,
    ip: &ClientIp,
) -> AppResult<ProfileField> {
    authz::require_workspace_role(state, workspace_id, actor, &WorkspaceRole::Admin).await?;
    let field = found(state, workspace_id, field_id).await?;
    let label = match req.label.as_deref() {
        Some(raw) => label_of(raw)?,
        None => field.label.clone(),
    };
    let hint = match req.hint.as_deref() {
        Some(raw) => hint_of(Some(raw))?,
        None => field.hint.clone(),
    };
    let options = match &req.options {
        Some(input) => options_of(field.kind, &label, input, &field.options)?,
        None => field.options.0.clone(),
    };
    let on_card = req.on_card.unwrap_or(field.on_card);
    let draft = FieldDraft {
        label: &label,
        hint: hint.as_deref(),
        kind: field.kind,
        options: &options,
        admins_only: req.admins_only.unwrap_or(field.admins_only),
        on_card,
    };

    let mut tx = state.pool.begin().await?;
    ProfileFieldRepo::lock_workspace_in(&mut tx, workspace_id).await?;
    if on_card
        && !field.on_card
        && ProfileFieldRepo::on_card_count_in(&mut tx, workspace_id, Some(field_id)).await?
            >= MAX_ON_CARD
    {
        return Err(card_full());
    }
    ProfileFieldRepo::update_in(&mut tx, workspace_id, field_id, &draft)
        .await
        .map_err(|e| taken(e, &label))?;
    let dropped = if field.kind == ProfileFieldKind::Options && req.options.is_some() {
        let kept: Vec<String> = options.iter().map(|o| o.id.to_string()).collect();
        ProfileFieldRepo::drop_values_outside_in(&mut tx, field_id, &kept).await?
    } else {
        0
    };
    tx.commit().await?;

    audit::record(
        state,
        AuditEntry::new(AuditAction::ProfileFieldUpdated, actor)
            .workspace(workspace_id)
            .resource(field_id)
            .ip(ip)
            .details(serde_json::json!({ "label": label, "values_dropped": dropped })),
    )
    .await;
    found(state, workspace_id, field_id).await
}

pub async fn delete(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    field_id: Uuid,
    ip: &ClientIp,
) -> AppResult<()> {
    authz::require_workspace_role(state, workspace_id, actor, &WorkspaceRole::Admin).await?;
    let field = found(state, workspace_id, field_id).await?;
    state
        .profile_field_repo
        .delete(workspace_id, field_id)
        .await?;
    audit::record(
        state,
        AuditEntry::new(AuditAction::ProfileFieldDeleted, actor)
            .workspace(workspace_id)
            .resource(field_id)
            .ip(ip)
            .details(serde_json::json!({ "label": field.label, "values": field.value_count })),
    )
    .await;
    Ok(())
}

pub async fn reorder(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    field_ids: &[Uuid],
) -> AppResult<Vec<ProfileField>> {
    authz::require_workspace_role(state, workspace_id, actor, &WorkspaceRole::Admin).await?;
    let mut tx = state.pool.begin().await?;
    ProfileFieldRepo::lock_workspace_in(&mut tx, workspace_id).await?;
    let current: HashSet<Uuid> = ProfileFieldRepo::ids_in(&mut tx, workspace_id)
        .await?
        .into_iter()
        .collect();
    let asked: HashSet<Uuid> = field_ids.iter().copied().collect();
    if asked.len() != field_ids.len() || asked != current {
        return Err(AppError::Validation(
            "List every profile field of the workspace once".into(),
        ));
    }
    ProfileFieldRepo::reorder_in(&mut tx, workspace_id, field_ids).await?;
    tx.commit().await?;
    list(state, workspace_id).await
}

pub async fn values(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<ProfileValues> {
    Ok(ProfileValues {
        user_id,
        values: state
            .profile_field_repo
            .values_of(workspace_id, user_id)
            .await?,
    })
}

pub async fn set_values(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    target: Uuid,
    req: &SetProfileValues,
) -> AppResult<ProfileValues> {
    let member = authz::require_workspace_member(state, workspace_id, actor).await?;
    let admin = member.role.has_at_least(&WorkspaceRole::Admin);
    if target != actor && !admin {
        return Err(AppError::Forbidden(
            "Only an admin edits someone else's profile fields".into(),
        ));
    }
    state
        .workspace_service
        .repo
        .get_member(workspace_id, target)
        .await?
        .ok_or_else(|| AppError::NotFound("Not a member of this workspace".into()))?;

    let fields: HashMap<Uuid, ProfileField> = list(state, workspace_id)
        .await?
        .into_iter()
        .map(|f| (f.id, f))
        .collect();
    let mut written = Vec::new();
    let mut cleared = Vec::new();
    let mut people: Vec<(Uuid, &ProfileField)> = Vec::new();
    for (field_id, raw) in &req.values {
        let field = fields
            .get(field_id)
            .ok_or_else(|| AppError::Validation("Unknown profile field".into()))?;
        if field.admins_only && !admin {
            return Err(AppError::Forbidden(format!(
                "Only an admin sets {}",
                field.label
            )));
        }
        match raw.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            None => cleared.push(*field_id),
            Some(value) => {
                let value = value_of(field, value)?;
                if field.kind == ProfileFieldKind::Person {
                    people.push((Uuid::parse_str(&value).unwrap_or_default(), field));
                }
                written.push((*field_id, value));
            }
        }
    }
    if !people.is_empty() {
        let ids: Vec<Uuid> = people.iter().map(|(id, _)| *id).collect();
        let present: HashSet<Uuid> = state
            .profile_field_repo
            .people_among(workspace_id, &ids)
            .await?
            .into_iter()
            .collect();
        if let Some((_, field)) = people.iter().find(|(id, _)| !present.contains(id)) {
            return Err(AppError::Validation(format!(
                "{}: that person is not in this workspace",
                field.label
            )));
        }
    }

    state
        .profile_field_repo
        .set_values(workspace_id, target, &written, &cleared)
        .await?;
    values(state, workspace_id, target).await
}

pub fn filter_members(
    members: Vec<MemberWithUser>,
    fields: &[ProfileField],
    query: &MembersQuery,
) -> AppResult<Vec<MemberWithUser>> {
    let wanted = wanted_values(query.filter.as_deref(), fields)?;
    let needle = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_lowercase);
    let texts: Vec<Uuid> = fields
        .iter()
        .filter(|f| f.kind == ProfileFieldKind::Text)
        .map(|f| f.id)
        .collect();
    Ok(members
        .into_iter()
        .filter(|m| {
            wanted
                .iter()
                .all(|(field, value)| m.profile_fields.get(field) == Some(value))
        })
        .filter(|m| needle.as_deref().is_none_or(|n| mentions(m, n, &texts)))
        .collect())
}

fn mentions(member: &MemberWithUser, needle: &str, texts: &[Uuid]) -> bool {
    let has = |text: &str| text.to_lowercase().contains(needle);
    member.display_name.as_deref().is_some_and(has)
        || member.title.as_deref().is_some_and(has)
        || has(&member.email)
        || texts
            .iter()
            .filter_map(|id| member.profile_fields.get(id))
            .any(|v| has(v))
}

fn wanted_values(filter: Option<&str>, fields: &[ProfileField]) -> AppResult<Vec<(Uuid, String)>> {
    let Some(filter) = filter.map(str::trim).filter(|f| !f.is_empty()) else {
        return Ok(Vec::new());
    };
    filter
        .split(',')
        .map(|pair| {
            let malformed =
                || AppError::Validation("A filter is field_id:value, separated by commas".into());
            let (field_id, value) = pair.trim().split_once(':').ok_or_else(malformed)?;
            let field_id = Uuid::parse_str(field_id).map_err(|_| malformed())?;
            let value = Uuid::parse_str(value).map_err(|_| malformed())?;
            let field = fields
                .iter()
                .find(|f| f.id == field_id)
                .ok_or_else(|| AppError::Validation("Unknown profile field".into()))?;
            if !field.kind.filterable() {
                return Err(AppError::Validation(format!(
                    "{} cannot be filtered on",
                    field.label
                )));
            }
            Ok((field_id, value.to_string()))
        })
        .collect()
}

fn value_of(field: &ProfileField, value: &str) -> AppResult<String> {
    let refuse = |why: &str| AppError::Validation(format!("{}: {why}", field.label));
    match field.kind {
        ProfileFieldKind::Text => {
            if value.chars().count() > MAX_TEXT {
                return Err(refuse(&format!("at most {MAX_TEXT} characters")));
            }
            if value.chars().any(char::is_control) {
                return Err(refuse("one line of text, without control characters"));
            }
            Ok(value.to_string())
        }
        ProfileFieldKind::Link => {
            let host = value
                .strip_prefix("https://")
                .or_else(|| value.strip_prefix("http://"))
                .unwrap_or_default();
            if host.is_empty()
                || value.len() > MAX_LINK
                || value.chars().any(|c| c.is_control() || c.is_whitespace())
            {
                return Err(refuse("a link starting with http:// or https://"));
            }
            Ok(value.to_string())
        }
        ProfileFieldKind::Date => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map(|d| d.format("%Y-%m-%d").to_string())
            .map_err(|_| refuse("a date written as YYYY-MM-DD")),
        ProfileFieldKind::Options => Uuid::parse_str(value)
            .ok()
            .filter(|id| field.has_option(*id))
            .map(|id| id.to_string())
            .ok_or_else(|| refuse("not one of its options")),
        ProfileFieldKind::Person => Uuid::parse_str(value)
            .map(|id| id.to_string())
            .map_err(|_| refuse("that person is not in this workspace")),
    }
}

fn label_of(raw: &str) -> AppResult<String> {
    let label = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if label.is_empty() || label.chars().count() > MAX_LABEL {
        return Err(AppError::Validation(format!(
            "A field's label is 1 to {MAX_LABEL} characters"
        )));
    }
    if label.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "A field's label cannot contain control characters".into(),
        ));
    }
    Ok(label)
}

fn hint_of(raw: Option<&str>) -> AppResult<Option<String>> {
    let Some(hint) = raw.map(str::trim).filter(|h| !h.is_empty()) else {
        return Ok(None);
    };
    if hint.chars().count() > MAX_HINT || hint.chars().any(char::is_control) {
        return Err(AppError::Validation(format!(
            "A field's hint is one line of at most {MAX_HINT} characters"
        )));
    }
    Ok(Some(hint.to_string()))
}

fn options_of(
    kind: ProfileFieldKind,
    label: &str,
    input: &[ProfileFieldOptionInput],
    current: &[ProfileFieldOption],
) -> AppResult<Vec<ProfileFieldOption>> {
    if kind != ProfileFieldKind::Options {
        if input.is_empty() {
            return Ok(Vec::new());
        }
        return Err(AppError::Validation(
            "Only an options field has options".into(),
        ));
    }
    if input.is_empty() {
        return Err(AppError::Validation(format!(
            "{label} needs at least one option"
        )));
    }
    if input.len() > MAX_OPTIONS {
        return Err(AppError::Validation(format!(
            "{label} can have at most {MAX_OPTIONS} options"
        )));
    }
    let mut seen = HashSet::new();
    let mut kept = HashSet::new();
    input
        .iter()
        .map(|option| {
            let text = option
                .label
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if text.is_empty() || text.chars().count() > MAX_OPTION_LABEL {
                return Err(AppError::Validation(format!(
                    "An option is 1 to {MAX_OPTION_LABEL} characters"
                )));
            }
            if !seen.insert(text.to_lowercase()) {
                return Err(AppError::Validation(format!("{label} lists {text} twice")));
            }
            let id = match option.id {
                Some(id) if current.iter().any(|o| o.id == id) && kept.insert(id) => id,
                Some(_) => {
                    return Err(AppError::Validation(format!(
                        "{text} is not an option of {label}"
                    )))
                }
                None => Uuid::new_v4(),
            };
            Ok(ProfileFieldOption { id, label: text })
        })
        .collect()
}

async fn found(state: &AppState, workspace_id: Uuid, field_id: Uuid) -> AppResult<ProfileField> {
    state
        .profile_field_repo
        .find(workspace_id, field_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile field not found".into()))
}

fn card_full() -> AppError {
    AppError::Conflict(format!(
        "At most {MAX_ON_CARD} fields show on the hover card"
    ))
}

fn taken(err: sqlx::Error, label: &str) -> AppError {
    if is_unique_violation(&err) {
        AppError::Conflict(format!("There is already a field called {label}"))
    } else {
        err.into()
    }
}
