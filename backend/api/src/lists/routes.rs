use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::{
    ColumnChangePreview, ColumnOrder, ConversionCounts, CreateItemRequest, CreateListRequest, List,
    ListColumn, ListDetail, ListItem, ListsQuery, MoveItemRequest, NewColumn, UpdateColumnRequest,
    UpdateItemRequest, UpdateListRequest,
};
use super::service;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_lists, create_list))
        .routes(routes!(get_list, update_list, delete_list))
        .routes(routes!(list_items, create_list_item))
        .routes(routes!(add_list_column))
        .routes(routes!(reorder_list_columns))
        .routes(routes!(update_list_column, delete_list_column))
        .routes(routes!(preview_list_column))
        .routes(routes!(update_list_item, delete_list_item))
        .routes(routes!(move_list_item))
}

#[utoipa::path(
    get, path = "/workspaces/{ws_id}/lists", tag = "lists",
    params(ListsQuery),
    responses((status = 200, body = DataList<List>)))]
async fn list_lists(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<ListsQuery>,
) -> AppResult<Json<DataList<List>>> {
    let lists = service::visible(&state, auth.user_id, ws_id, query.channel_id).await?;
    Ok(Json(lists.into()))
}

#[utoipa::path(
    post, path = "/workspaces/{ws_id}/lists", tag = "lists",
    request_body = CreateListRequest,
    responses((status = 200, body = ListDetail)))]
async fn create_list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateListRequest>,
) -> AppResult<Json<ListDetail>> {
    Ok(Json(
        service::create(&state, auth.user_id, ws_id, &req).await?,
    ))
}

#[utoipa::path(
    get, path = "/lists/{list_id}", tag = "lists",
    responses((status = 200, body = ListDetail)))]
async fn get_list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(list_id): Path<Uuid>,
) -> AppResult<Json<ListDetail>> {
    Ok(Json(service::detail(&state, auth.user_id, list_id).await?))
}

#[utoipa::path(
    patch, path = "/lists/{list_id}", tag = "lists",
    request_body = UpdateListRequest,
    responses((status = 200, body = List)))]
async fn update_list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(list_id): Path<Uuid>,
    Json(req): Json<UpdateListRequest>,
) -> AppResult<Json<List>> {
    Ok(Json(
        service::update(&state, auth.user_id, list_id, &req).await?,
    ))
}

#[utoipa::path(
    delete, path = "/lists/{list_id}", tag = "lists",
    responses((status = 200, body = StatusResponse)))]
async fn delete_list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(list_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    service::delete(&state, auth.user_id, list_id).await?;
    Ok(Json(StatusResponse::new("deleted")))
}

#[utoipa::path(
    get, path = "/lists/{list_id}/items", tag = "lists",
    responses((status = 200, body = DataList<ListItem>)))]
async fn list_items(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(list_id): Path<Uuid>,
) -> AppResult<Json<DataList<ListItem>>> {
    let items = service::items(&state, auth.user_id, list_id).await?;
    Ok(Json(items.into()))
}

#[utoipa::path(
    post, path = "/lists/{list_id}/items", tag = "lists",
    request_body = CreateItemRequest,
    responses((status = 200, body = ListItem)))]
async fn create_list_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(list_id): Path<Uuid>,
    Json(req): Json<CreateItemRequest>,
) -> AppResult<Json<ListItem>> {
    Ok(Json(
        service::create_item(&state, auth.user_id, list_id, &req).await?,
    ))
}

#[utoipa::path(
    post, path = "/lists/{list_id}/columns", tag = "lists",
    request_body = NewColumn,
    responses((status = 200, body = ListColumn)))]
async fn add_list_column(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(list_id): Path<Uuid>,
    Json(req): Json<NewColumn>,
) -> AppResult<Json<ListColumn>> {
    Ok(Json(
        service::add_column(&state, auth.user_id, list_id, &req).await?,
    ))
}

#[utoipa::path(
    put, path = "/lists/{list_id}/column-order", tag = "lists",
    request_body = ColumnOrder,
    responses((status = 200, body = DataList<ListColumn>)))]
async fn reorder_list_columns(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(list_id): Path<Uuid>,
    Json(req): Json<ColumnOrder>,
) -> AppResult<Json<DataList<ListColumn>>> {
    let columns = service::reorder_columns(&state, auth.user_id, list_id, &req.column_ids).await?;
    Ok(Json(columns.into()))
}

#[utoipa::path(
    patch, path = "/list-columns/{column_id}", tag = "lists",
    request_body = UpdateColumnRequest,
    responses((status = 200, body = ListColumn)))]
async fn update_list_column(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(column_id): Path<Uuid>,
    Json(req): Json<UpdateColumnRequest>,
) -> AppResult<Json<ListColumn>> {
    Ok(Json(
        service::update_column(&state, auth.user_id, column_id, &req).await?,
    ))
}

#[utoipa::path(
    delete, path = "/list-columns/{column_id}", tag = "lists",
    responses((status = 200, body = StatusResponse)))]
async fn delete_list_column(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(column_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    service::delete_column(&state, auth.user_id, column_id).await?;
    Ok(Json(StatusResponse::new("deleted")))
}

#[utoipa::path(
    post, path = "/list-columns/{column_id}/preview", tag = "lists",
    request_body = ColumnChangePreview,
    responses((status = 200, body = ConversionCounts)))]
async fn preview_list_column(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(column_id): Path<Uuid>,
    Json(req): Json<ColumnChangePreview>,
) -> AppResult<Json<ConversionCounts>> {
    Ok(Json(
        service::preview_column(&state, auth.user_id, column_id, &req).await?,
    ))
}

#[utoipa::path(
    patch, path = "/list-items/{item_id}", tag = "lists",
    request_body = UpdateItemRequest,
    responses((status = 200, body = ListItem)))]
async fn update_list_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(item_id): Path<Uuid>,
    Json(req): Json<UpdateItemRequest>,
) -> AppResult<Json<ListItem>> {
    Ok(Json(
        service::update_item(&state, auth.user_id, item_id, &req.values).await?,
    ))
}

#[utoipa::path(
    delete, path = "/list-items/{item_id}", tag = "lists",
    responses((status = 200, body = StatusResponse)))]
async fn delete_list_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(item_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    service::delete_item(&state, auth.user_id, item_id).await?;
    Ok(Json(StatusResponse::new("deleted")))
}

#[utoipa::path(
    post, path = "/list-items/{item_id}/move", tag = "lists",
    request_body = MoveItemRequest,
    responses((status = 200, body = ListItem)))]
async fn move_list_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(item_id): Path<Uuid>,
    Json(req): Json<MoveItemRequest>,
) -> AppResult<Json<ListItem>> {
    Ok(Json(
        service::move_item(&state, auth.user_id, item_id, req.after_id).await?,
    ))
}
