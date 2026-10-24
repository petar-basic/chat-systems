use serde_json::{json, Map, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::cells::{self, Cell};
use super::models::{
    clean_column_name, clean_description, clean_title, ChoiceInput, ColumnChangePreview,
    ColumnKind, ColumnOptions, ColumnOptionsInput, ConversionCounts, CreateItemRequest,
    CreateListRequest, List, ListColumn, ListDetail, ListItem, NewColumn, UpdateColumnRequest,
    UpdateListRequest, MAX_COLUMNS, MAX_ITEMS,
};
use super::rank;
use super::repo::Visibility;
use super::views::{
    clean_view_name, CreateViewRequest, ListView, UpdateViewRequest, ViewConfig, ViewKind,
    MAX_VIEWS,
};
use crate::authz;
use crate::messaging::publisher::Staged;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

fn not_found() -> AppError {
    AppError::NotFound("List not found".into())
}

async fn authorize(state: &AppState, user_id: Uuid, list: &List, write: bool) -> AppResult<()> {
    match list.channel_id {
        Some(channel_id) if write => {
            authz::require_channel_post(state, channel_id, user_id).await?;
        }
        Some(channel_id) => {
            authz::require_channel_access(state, channel_id, user_id).await?;
        }
        None => {
            authz::require_workspace_member(state, list.workspace_id, user_id).await?;
            if list.created_by != user_id {
                return Err(not_found());
            }
        }
    }
    Ok(())
}

async fn find(state: &AppState, list_id: Uuid) -> AppResult<List> {
    state.list_repo.find(list_id).await?.ok_or_else(not_found)
}

pub async fn readable(state: &AppState, user_id: Uuid, list_id: Uuid) -> AppResult<List> {
    let list = find(state, list_id).await?;
    authorize(state, user_id, &list, false).await?;
    Ok(list)
}

pub async fn writable(state: &AppState, user_id: Uuid, list_id: Uuid) -> AppResult<List> {
    let list = find(state, list_id).await?;
    authorize(state, user_id, &list, true).await?;
    Ok(list)
}

async fn lock(state: &AppState, conn: &mut PgConnection, list_id: Uuid) -> AppResult<List> {
    state
        .list_repo
        .lock_in(conn, list_id)
        .await?
        .ok_or_else(not_found)
}

async fn stage(
    state: &AppState,
    conn: &mut PgConnection,
    list: &List,
    event_type: &str,
    fields: Value,
) -> AppResult<Staged> {
    let mut payload = json!({
        "list_id": list.id,
        "channel_id": list.channel_id,
        "owner_id": list.created_by,
    });
    if let (Some(payload), Value::Object(fields)) = (payload.as_object_mut(), fields) {
        payload.extend(fields);
    }
    state
        .publisher
        .stage(conn, event_type, list.workspace_id, payload)
        .await
}

pub async fn visible(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
    channel_id: Option<Uuid>,
) -> AppResult<Vec<List>> {
    let member = authz::require_workspace_member(state, workspace_id, user_id).await?;
    if let Some(channel_id) = channel_id {
        let access = authz::require_channel_access(state, channel_id, user_id).await?;
        if access.channel.workspace_id != workspace_id {
            return Err(AppError::NotFound("Channel not found".into()));
        }
    }
    Ok(state
        .list_repo
        .visible(&Visibility {
            workspace_id,
            user_id,
            is_guest: member.role == WorkspaceRole::Guest,
            channel_id,
        })
        .await?)
}

struct NewColumnResolved {
    kind: ColumnKind,
    options: ColumnOptions,
}

fn resolve_columns(columns: &[NewColumn]) -> AppResult<Vec<(String, NewColumnResolved)>> {
    if columns.len() > MAX_COLUMNS {
        return Err(AppError::Validation(format!(
            "A list can have at most {MAX_COLUMNS} columns"
        )));
    }
    columns
        .iter()
        .map(|column| {
            Ok((
                clean_column_name(&column.name)?,
                NewColumnResolved {
                    kind: column.kind,
                    options: column.options.resolve(column.kind)?,
                },
            ))
        })
        .collect()
}

pub async fn create(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
    req: &CreateListRequest,
) -> AppResult<ListDetail> {
    let title = clean_title(&req.title)?;
    let description = clean_description(&req.description)?;
    let columns = resolve_columns(&req.columns)?;
    match req.channel_id {
        Some(channel_id) => {
            let access = authz::require_channel_post(state, channel_id, user_id).await?;
            if access.channel.workspace_id != workspace_id {
                return Err(AppError::NotFound("Channel not found".into()));
            }
        }
        None => {
            authz::require_workspace_member(state, workspace_id, user_id).await?;
        }
    }

    let mut tx = state.pool.begin().await?;
    let list = state
        .list_repo
        .create_in(
            &mut tx,
            workspace_id,
            req.channel_id,
            &title,
            &description,
            user_id,
        )
        .await?;
    for (name, column) in &columns {
        state
            .list_repo
            .insert_column_in(&mut tx, list.id, name, column.kind, &column.options)
            .await?;
    }
    let columns = state.list_repo.columns_in(&mut tx, list.id).await?;
    let view = state
        .list_repo
        .insert_view_in(
            &mut tx,
            list.id,
            "Table",
            ViewKind::Table,
            &ViewConfig::default(),
            user_id,
        )
        .await?;
    let staged = stage(
        state,
        &mut tx,
        &list,
        "list.upserted",
        json!({ "list": list }),
    )
    .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(ListDetail {
        list,
        columns,
        views: vec![view],
    })
}

pub async fn update(
    state: &AppState,
    user_id: Uuid,
    list_id: Uuid,
    req: &UpdateListRequest,
) -> AppResult<List> {
    writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    let current = lock(state, &mut tx, list_id).await?;
    let title = match &req.title {
        Some(title) => clean_title(title)?,
        None => current.title.clone(),
    };
    let description = match &req.description {
        Some(description) => clean_description(description)?,
        None => current.description.clone(),
    };
    let list = state
        .list_repo
        .update_in(&mut tx, list_id, &title, &description)
        .await?;
    let staged = stage(
        state,
        &mut tx,
        &list,
        "list.upserted",
        json!({ "list": list }),
    )
    .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(list)
}

pub async fn delete(state: &AppState, user_id: Uuid, list_id: Uuid) -> AppResult<()> {
    let list = readable(state, user_id, list_id).await?;
    if let Some(channel_id) = list.channel_id {
        if list.created_by == user_id {
            authz::require_channel_post(state, channel_id, user_id).await?;
        } else {
            let channel = authz::find_channel(state, channel_id).await?;
            authz::require_channel_moderator(state, &channel, user_id).await?;
        }
    }
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    state.list_repo.soft_delete_in(&mut tx, list_id).await?;
    let staged = stage(state, &mut tx, &list, "list.deleted", json!({})).await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(())
}

pub async fn detail(state: &AppState, user_id: Uuid, list_id: Uuid) -> AppResult<ListDetail> {
    let list = readable(state, user_id, list_id).await?;
    let columns = state.list_repo.columns(list.id).await?;
    let views = state.list_repo.views(list.id).await?;
    Ok(ListDetail {
        list,
        columns,
        views,
    })
}

async fn columns_changed(
    state: &AppState,
    conn: &mut PgConnection,
    list: &List,
    items_rewritten: bool,
) -> AppResult<(Vec<ListColumn>, Staged)> {
    let columns = state.list_repo.columns_in(conn, list.id).await?;
    let staged = stage(
        state,
        conn,
        list,
        "list.columns_changed",
        json!({ "columns": columns, "items_rewritten": items_rewritten }),
    )
    .await?;
    Ok((columns, staged))
}

async fn views_changed(
    state: &AppState,
    conn: &mut PgConnection,
    list: &List,
) -> AppResult<(Vec<ListView>, Staged)> {
    let views = state.list_repo.views_in(conn, list.id).await?;
    let staged = stage(
        state,
        conn,
        list,
        "list.views_changed",
        json!({ "views": views }),
    )
    .await?;
    Ok((views, staged))
}

async fn prune_views(
    state: &AppState,
    conn: &mut PgConnection,
    list: &List,
    columns: &[ListColumn],
) -> AppResult<Option<Staged>> {
    let mut pruned = false;
    for view in state.list_repo.views_in(conn, list.id).await? {
        let config = view.config.pruned(view.kind, columns);
        if config != view.config.0 {
            state
                .list_repo
                .update_view_in(conn, view.id, &view.name, view.kind, &config)
                .await?;
            pruned = true;
        }
    }
    if !pruned {
        return Ok(None);
    }
    let (_, staged) = views_changed(state, conn, list).await?;
    Ok(Some(staged))
}

fn names_each_once(order: &[Uuid], existing: impl Iterator<Item = Uuid>) -> bool {
    let mut given = order.to_vec();
    given.sort();
    given.dedup();
    let mut existing: Vec<Uuid> = existing.collect();
    existing.sort();
    given.len() == order.len() && given == existing
}

pub async fn add_column(
    state: &AppState,
    user_id: Uuid,
    list_id: Uuid,
    req: &NewColumn,
) -> AppResult<ListColumn> {
    writable(state, user_id, list_id).await?;
    let name = clean_column_name(&req.name)?;
    let options = req.options.resolve(req.kind)?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    if state.list_repo.columns_in(&mut tx, list_id).await?.len() >= MAX_COLUMNS {
        return Err(AppError::Validation(format!(
            "A list can have at most {MAX_COLUMNS} columns"
        )));
    }
    let column = state
        .list_repo
        .insert_column_in(&mut tx, list_id, &name, req.kind, &options)
        .await?;
    let (_, staged) = columns_changed(state, &mut tx, &list, false).await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(column)
}

async fn column_place(state: &AppState, column_id: Uuid) -> AppResult<Uuid> {
    state
        .list_repo
        .column_list(column_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Column not found".into()))
}

fn column_of(columns: &[ListColumn], column_id: Uuid) -> AppResult<&ListColumn> {
    columns
        .iter()
        .find(|column| column.id == column_id)
        .ok_or_else(|| AppError::NotFound("Column not found".into()))
}

fn options_input(options: &ColumnOptions) -> ColumnOptionsInput {
    ColumnOptionsInput {
        choices: options
            .choices
            .iter()
            .map(|choice| ChoiceInput {
                id: Some(choice.id),
                label: choice.label.clone(),
                color: choice.color,
            })
            .collect(),
        number: options.number.clone(),
    }
}

struct Conversion {
    cells: Map<String, Value>,
    converted: i64,
    cleared: i64,
}

async fn convert_cells(
    state: &AppState,
    conn: &mut PgConnection,
    column: &ListColumn,
    kind: ColumnKind,
    options: &ColumnOptions,
) -> AppResult<Conversion> {
    let mut conversion = Conversion {
        cells: Map::new(),
        converted: 0,
        cleared: 0,
    };
    if kind == column.kind && *options == column.options.0 {
        return Ok(conversion);
    }
    for (item_id, value) in state
        .list_repo
        .cells_in(conn, column.list_id, column.id)
        .await?
    {
        match cells::convert((column.kind, &column.options), (kind, options), &value) {
            Some(next) => {
                conversion.converted += 1;
                if next != value {
                    conversion.cells.insert(item_id.to_string(), next);
                }
            }
            None => {
                conversion.cleared += 1;
                conversion.cells.insert(item_id.to_string(), Value::Null);
            }
        }
    }
    Ok(conversion)
}

pub async fn preview_column(
    state: &AppState,
    user_id: Uuid,
    column_id: Uuid,
    req: &ColumnChangePreview,
) -> AppResult<ConversionCounts> {
    let list_id = column_place(state, column_id).await?;
    writable(state, user_id, list_id).await?;
    let options = req.options.resolve(req.kind)?;
    let mut conn = state.pool.acquire().await?;
    let columns = state.list_repo.columns_in(&mut conn, list_id).await?;
    let column = column_of(&columns, column_id)?;
    let conversion = convert_cells(state, &mut conn, column, req.kind, &options).await?;
    Ok(ConversionCounts {
        converted: conversion.converted,
        cleared: conversion.cleared,
    })
}

pub async fn update_column(
    state: &AppState,
    user_id: Uuid,
    column_id: Uuid,
    req: &UpdateColumnRequest,
) -> AppResult<ListColumn> {
    let list_id = column_place(state, column_id).await?;
    writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    let columns = state.list_repo.columns_in(&mut tx, list_id).await?;
    let column = column_of(&columns, column_id)?;
    let name = match &req.name {
        Some(name) => clean_column_name(name)?,
        None => column.name.clone(),
    };
    let kind = req.kind.unwrap_or(column.kind);
    let options = match &req.options {
        Some(input) => input.resolve(kind)?,
        None => options_input(&column.options).resolve(kind)?,
    };
    let conversion = convert_cells(state, &mut tx, column, kind, &options).await?;
    if conversion.cleared > 0 && req.confirm_cleared != Some(conversion.cleared) {
        return Err(AppError::Conflict(format!(
            "This change clears {} {}",
            conversion.cleared,
            if conversion.cleared == 1 {
                "cell"
            } else {
                "cells"
            }
        )));
    }
    state
        .list_repo
        .update_column_in(
            &mut tx,
            column_id,
            &name,
            kind,
            &options,
            req.hidden.unwrap_or(column.hidden),
        )
        .await?;
    let rewritten = !conversion.cells.is_empty();
    if rewritten {
        state
            .list_repo
            .rewrite_cells_in(
                &mut tx,
                list_id,
                column_id,
                &Value::Object(conversion.cells),
            )
            .await?;
    }
    let (columns, staged) = columns_changed(state, &mut tx, &list, rewritten).await?;
    let pruned = prune_views(state, &mut tx, &list, &columns).await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    if let Some(pruned) = pruned {
        state.publisher.dispatch(pruned).await;
    }
    Ok(column_of(&columns, column_id)?.clone())
}

pub async fn delete_column(state: &AppState, user_id: Uuid, column_id: Uuid) -> AppResult<()> {
    let list_id = column_place(state, column_id).await?;
    writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    state
        .list_repo
        .delete_column_in(&mut tx, list_id, column_id)
        .await?;
    let (columns, staged) = columns_changed(state, &mut tx, &list, true).await?;
    let pruned = prune_views(state, &mut tx, &list, &columns).await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    if let Some(pruned) = pruned {
        state.publisher.dispatch(pruned).await;
    }
    Ok(())
}

pub async fn reorder_columns(
    state: &AppState,
    user_id: Uuid,
    list_id: Uuid,
    order: &[Uuid],
) -> AppResult<Vec<ListColumn>> {
    writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    let current = state.list_repo.columns_in(&mut tx, list_id).await?;
    if !names_each_once(order, current.iter().map(|column| column.id)) {
        return Err(AppError::Validation(
            "Name every column of the list exactly once".into(),
        ));
    }
    state
        .list_repo
        .reorder_columns_in(&mut tx, list_id, order)
        .await?;
    let (columns, staged) = columns_changed(state, &mut tx, &list, false).await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(columns)
}

async fn view_place(state: &AppState, view_id: Uuid) -> AppResult<Uuid> {
    state
        .list_repo
        .view_list(view_id)
        .await?
        .ok_or_else(|| AppError::NotFound("View not found".into()))
}

pub async fn create_view(
    state: &AppState,
    user_id: Uuid,
    list_id: Uuid,
    req: &CreateViewRequest,
) -> AppResult<ListView> {
    writable(state, user_id, list_id).await?;
    let name = clean_view_name(&req.name)?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    if state.list_repo.views_in(&mut tx, list_id).await?.len() >= MAX_VIEWS {
        return Err(AppError::Validation(format!(
            "A list can have at most {MAX_VIEWS} views"
        )));
    }
    let columns = state.list_repo.columns_in(&mut tx, list_id).await?;
    let config = req.config.check(req.kind, &columns)?;
    let view = state
        .list_repo
        .insert_view_in(&mut tx, list_id, &name, req.kind, &config, user_id)
        .await?;
    let (_, staged) = views_changed(state, &mut tx, &list).await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(view)
}

fn view_of(views: &[ListView], view_id: Uuid) -> AppResult<&ListView> {
    views
        .iter()
        .find(|view| view.id == view_id)
        .ok_or_else(|| AppError::NotFound("View not found".into()))
}

pub async fn update_view(
    state: &AppState,
    user_id: Uuid,
    view_id: Uuid,
    req: &UpdateViewRequest,
) -> AppResult<ListView> {
    let list_id = view_place(state, view_id).await?;
    writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    let views = state.list_repo.views_in(&mut tx, list_id).await?;
    let view = view_of(&views, view_id)?;
    let name = match &req.name {
        Some(name) => clean_view_name(name)?,
        None => view.name.clone(),
    };
    let kind = req.kind.unwrap_or(view.kind);
    let columns = state.list_repo.columns_in(&mut tx, list_id).await?;
    let config = match &req.config {
        Some(config) => config.check(kind, &columns)?,
        None => view.config.pruned(kind, &columns),
    };
    state
        .list_repo
        .update_view_in(&mut tx, view_id, &name, kind, &config)
        .await?;
    let (views, staged) = views_changed(state, &mut tx, &list).await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(view_of(&views, view_id)?.clone())
}

pub async fn delete_view(state: &AppState, user_id: Uuid, view_id: Uuid) -> AppResult<()> {
    let list_id = view_place(state, view_id).await?;
    writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    let views = state.list_repo.views_in(&mut tx, list_id).await?;
    view_of(&views, view_id)?;
    if views.len() == 1 {
        return Err(AppError::Validation(
            "A list keeps at least one view".into(),
        ));
    }
    state.list_repo.delete_view_in(&mut tx, view_id).await?;
    let (_, staged) = views_changed(state, &mut tx, &list).await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(())
}

pub async fn reorder_views(
    state: &AppState,
    user_id: Uuid,
    list_id: Uuid,
    order: &[Uuid],
) -> AppResult<Vec<ListView>> {
    writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    let current = state.list_repo.views_in(&mut tx, list_id).await?;
    if !names_each_once(order, current.iter().map(|view| view.id)) {
        return Err(AppError::Validation(
            "Name every view of the list exactly once".into(),
        ));
    }
    state
        .list_repo
        .reorder_views_in(&mut tx, list_id, order)
        .await?;
    let (views, staged) = views_changed(state, &mut tx, &list).await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(views)
}

pub async fn items(state: &AppState, user_id: Uuid, list_id: Uuid) -> AppResult<Vec<ListItem>> {
    readable(state, user_id, list_id).await?;
    Ok(state.list_repo.items(list_id).await?)
}

struct Edit {
    set: Map<String, Value>,
    clear: Vec<String>,
    people: Vec<Uuid>,
}

fn validate(columns: &[ListColumn], values: &Map<String, Value>) -> AppResult<Edit> {
    let mut edit = Edit {
        set: Map::new(),
        clear: Vec::new(),
        people: Vec::new(),
    };
    for (key, value) in values {
        let column = key
            .parse::<Uuid>()
            .ok()
            .and_then(|id| columns.iter().find(|column| column.id == id))
            .ok_or_else(|| AppError::Validation("That column is not in this list".into()))?;
        match cells::normalize(column.kind, &column.options, value) {
            Ok(Cell::Set(value)) => {
                if let Some(person) = cells::person_of(column.kind, &value) {
                    edit.people.push(person);
                }
                edit.set.insert(column.id.to_string(), value);
            }
            Ok(Cell::Clear) => edit.clear.push(column.id.to_string()),
            Err(problem) => {
                return Err(AppError::Validation(format!("{} {problem}", column.name)));
            }
        }
    }
    Ok(edit)
}

async fn check_people(state: &AppState, list: &List, people: &[Uuid]) -> AppResult<()> {
    if people.is_empty() {
        return Ok(());
    }
    let present = match list.channel_id {
        Some(channel_id) => {
            state
                .list_repo
                .channel_members_among(channel_id, people)
                .await?
        }
        None => {
            state
                .list_repo
                .workspace_members_among(list.workspace_id, people)
                .await?
        }
    };
    if people.iter().any(|person| !present.contains(person)) {
        return Err(AppError::Validation(
            "A person must be a member of the list's conversation".into(),
        ));
    }
    Ok(())
}

pub async fn create_item(
    state: &AppState,
    user_id: Uuid,
    list_id: Uuid,
    req: &CreateItemRequest,
) -> AppResult<ListItem> {
    writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    let columns = state.list_repo.columns_in(&mut tx, list_id).await?;
    let edit = validate(&columns, &req.values)?;
    check_people(state, &list, &edit.people).await?;
    if state.list_repo.item_count_in(&mut tx, list_id).await? >= MAX_ITEMS {
        return Err(AppError::Validation(format!(
            "A list can hold at most {MAX_ITEMS} items"
        )));
    }
    let rank = match req.after_id {
        Some(after) => {
            let (low, high) = state
                .list_repo
                .neighbours_in(&mut tx, list_id, Some(after), None)
                .await?
                .ok_or_else(|| AppError::Validation("That item is not in this list".into()))?;
            rank::between(low.as_deref(), high.as_deref())
        }
        None => {
            let last = state.list_repo.last_rank_in(&mut tx, list_id).await?;
            rank::between(last.as_deref(), None)
        }
    };
    let item = state
        .list_repo
        .insert_item_in(&mut tx, list_id, &rank, &Value::Object(edit.set), user_id)
        .await?;
    let staged = stage(
        state,
        &mut tx,
        &list,
        "list.item_upserted",
        json!({ "item": item }),
    )
    .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(item)
}

async fn item_place(state: &AppState, item_id: Uuid) -> AppResult<Uuid> {
    state
        .list_repo
        .item_list(item_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Item not found".into()))
}

pub async fn update_item(
    state: &AppState,
    user_id: Uuid,
    item_id: Uuid,
    values: &Map<String, Value>,
) -> AppResult<ListItem> {
    let list_id = item_place(state, item_id).await?;
    let list = writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    if !state.list_repo.share_lock_in(&mut tx, list_id).await? {
        return Err(not_found());
    }
    let columns = state.list_repo.columns_in(&mut tx, list_id).await?;
    let edit = validate(&columns, values)?;
    check_people(state, &list, &edit.people).await?;
    let item = state
        .list_repo
        .merge_item_in(&mut tx, item_id, &Value::Object(edit.set), &edit.clear)
        .await?
        .ok_or_else(|| AppError::NotFound("Item not found".into()))?;
    let staged = stage(
        state,
        &mut tx,
        &list,
        "list.item_upserted",
        json!({ "item": item }),
    )
    .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(item)
}

pub async fn move_item(
    state: &AppState,
    user_id: Uuid,
    item_id: Uuid,
    after_id: Option<Uuid>,
) -> AppResult<ListItem> {
    if after_id == Some(item_id) {
        return Err(AppError::Validation("An item cannot follow itself".into()));
    }
    let list_id = item_place(state, item_id).await?;
    writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    let list = lock(state, &mut tx, list_id).await?;
    let (low, high) = state
        .list_repo
        .neighbours_in(&mut tx, list_id, after_id, Some(item_id))
        .await?
        .ok_or_else(|| AppError::Validation("That item is not in this list".into()))?;
    let rank = rank::between(low.as_deref(), high.as_deref());
    let item = state
        .list_repo
        .move_item_in(&mut tx, item_id, &rank)
        .await?;
    let staged = stage(
        state,
        &mut tx,
        &list,
        "list.item_upserted",
        json!({ "item": item }),
    )
    .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(item)
}

pub async fn delete_item(state: &AppState, user_id: Uuid, item_id: Uuid) -> AppResult<()> {
    let list_id = item_place(state, item_id).await?;
    let list = writable(state, user_id, list_id).await?;
    let mut tx = state.pool.begin().await?;
    if !state.list_repo.share_lock_in(&mut tx, list_id).await? {
        return Err(not_found());
    }
    if !state.list_repo.delete_item_in(&mut tx, item_id).await? {
        return Err(AppError::NotFound("Item not found".into()));
    }
    let staged = stage(
        state,
        &mut tx,
        &list,
        "list.item_deleted",
        json!({ "item_id": item_id }),
    )
    .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(())
}
