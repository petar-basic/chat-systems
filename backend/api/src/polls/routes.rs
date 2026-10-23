use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{
    CreatePollRequest, MyVote, OptionVoters, PollSummary, VoteRequest, VoteResponse,
};
use super::service;
use crate::dto::DataList;
use crate::messaging::models::Message;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(create_poll))
        .routes(routes!(my_vote, vote))
        .routes(routes!(list_voters))
        .routes(routes!(close_poll))
}

#[utoipa::path(
    post, path = "/channels/{ch_id}/polls", tag = "polls",
    request_body = CreatePollRequest,
    responses((status = 200, body = Message)))]
async fn create_poll(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<CreatePollRequest>,
) -> AppResult<Json<Message>> {
    Ok(Json(
        service::create(&state, auth.user_id, ch_id, &req).await?,
    ))
}

#[utoipa::path(
    get, path = "/polls/{poll_id}/vote", tag = "polls",
    responses((status = 200, body = MyVote)))]
async fn my_vote(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(poll_id): Path<Uuid>,
) -> AppResult<Json<MyVote>> {
    service::find_readable(&state, poll_id, auth.user_id).await?;
    let option_ids = state.poll_repo.my_vote(poll_id, auth.user_id).await?;
    Ok(Json(MyVote { option_ids }))
}

#[utoipa::path(
    put, path = "/polls/{poll_id}/vote", tag = "polls",
    request_body = VoteRequest,
    responses((status = 200, body = VoteResponse)))]
async fn vote(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(poll_id): Path<Uuid>,
    Json(req): Json<VoteRequest>,
) -> AppResult<Json<VoteResponse>> {
    let (option_ids, poll) = service::vote(&state, auth.user_id, poll_id, &req.option_ids).await?;
    Ok(Json(VoteResponse { option_ids, poll }))
}

#[utoipa::path(
    get, path = "/polls/{poll_id}/votes", tag = "polls",
    responses((status = 200, body = DataList<OptionVoters>)))]
async fn list_voters(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(poll_id): Path<Uuid>,
) -> AppResult<Json<DataList<OptionVoters>>> {
    let place = service::find_readable(&state, poll_id, auth.user_id).await?;
    if place.poll.anonymous {
        return Err(AppError::Forbidden("This poll is anonymous".into()));
    }
    Ok(Json(state.poll_repo.voters(poll_id).await?.into()))
}

#[utoipa::path(
    post, path = "/polls/{poll_id}/close", tag = "polls",
    responses((status = 200, body = PollSummary)))]
async fn close_poll(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(poll_id): Path<Uuid>,
) -> AppResult<Json<PollSummary>> {
    Ok(Json(
        service::close_by(&state, auth.user_id, poll_id).await?,
    ))
}
