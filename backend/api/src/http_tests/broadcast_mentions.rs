use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::messaging::routes::expand_mentions;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

struct Team {
    app: axum::Router,
    state: std::sync::Arc<AppState>,
    ws: Uuid,
    channel: Uuid,
    owner: String,
    lead: String,
    lead_id: Uuid,
    member: String,
    member_id: Uuid,
    other_id: Uuid,
}

async fn team(pool: PgPool) -> Team {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "bc-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Broadcast WS").await;
    let channel = seed_channel(&state, ws, owner_id, "all-hands", false).await;
    let mut people = Vec::new();
    for (prefix, role) in [
        ("bc-lead", ChannelRole::Admin),
        ("bc-member", ChannelRole::Member),
        ("bc-other", ChannelRole::Member),
    ] {
        let (id, _, token) = seed_and_login(&app, &state, prefix, false).await;
        add_ws_member(&state, ws, id, "member").await;
        state
            .workspace_service
            .repo
            .add_channel_member(channel, id, &role)
            .await
            .unwrap();
        people.push((id, token));
    }
    let [(lead_id, lead), (member_id, member), (other_id, _)] = people.try_into().unwrap();
    Team {
        app,
        state,
        ws,
        channel,
        owner,
        lead,
        lead_id,
        member,
        member_id,
        other_id,
    }
}

async fn set_rules(t: &Team, rules: Value) {
    let (status, body) = send(
        &t.app,
        "PATCH",
        &format!("/api/workspaces/{}", t.ws),
        Some(&t.owner),
        Some(rules),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

async fn say(t: &Team, token: &str, content: &str) -> (StatusCode, Value) {
    send(
        &t.app,
        "POST",
        &format!("/api/channels/{}/messages", t.channel),
        Some(token),
        Some(json!({ "content": content })),
    )
    .await
}

async fn mention_count(t: &Team, user: Uuid) -> i32 {
    sqlx::query_scalar(
        "SELECT mention_count FROM channel_members WHERE channel_id = $1 AND user_id = $2",
    )
    .bind(t.channel)
    .bind(user)
    .fetch_one(&t.state.pool)
    .await
    .unwrap()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_rules_round_trip_and_only_an_admin_sets_them(pool: PgPool) {
    let t = team(pool).await;
    let rules = json!({
        "broadcast_mentions": "admins",
        "public_channel_creators": "admins",
        "private_channel_creators": "members",
    });
    set_rules(&t, rules.clone()).await;

    let (_, workspace) = send(
        &t.app,
        "GET",
        &format!("/api/workspaces/{}", t.ws),
        Some(&t.member),
        None,
    )
    .await;
    for (key, value) in rules.as_object().unwrap() {
        assert_eq!(&workspace["settings"][key], value, "{key}");
    }

    let (status, _) = send(
        &t.app,
        "PATCH",
        &format!("/api/workspaces/{}", t.ws),
        Some(&t.member),
        Some(json!({ "broadcast_mentions": "members" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn by_default_anyone_who_may_post_notifies_the_channel(pool: PgPool) {
    let t = team(pool).await;
    let (status, body) = say(&t, &t.member, "@[channel](channel) lunch").await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(mention_count(&t, t.other_id).await, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_admins_only_rule_refuses_a_members_broadcast_and_says_why(pool: PgPool) {
    let t = team(pool).await;
    set_rules(&t, json!({ "broadcast_mentions": "admins" })).await;

    for content in [
        "@[channel](channel) lunch",
        "@here anyone?",
        "@everyone hello",
    ] {
        let (status, body) = say(&t, &t.member, content).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{content}");
        assert!(
            body.to_string()
                .contains("Only workspace admins and this channel's admins"),
            "{body:?}"
        );
    }
    let (status, _) = say(&t, &t.member, "no one in particular").await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &t.app,
        "POST",
        &format!("/api/channels/{}/polls", t.channel),
        Some(&t.member),
        Some(json!({ "question": "@channel pizza?", "options": ["yes", "no"] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a poll: {body:?}");

    let (status, body) = send(
        &t.app,
        "POST",
        &format!("/api/workspaces/{}/scheduled-messages", t.ws),
        Some(&t.member),
        Some(json!({
            "channel_id": t.channel,
            "content": "@[here](here) standup",
            "send_at": Utc::now() + Duration::hours(1),
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a scheduled message: {body:?}"
    );

    let (status, body) = say(&t, &t.lead, "@[channel](channel) from the channel admin").await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = say(&t, &t.owner, "@[channel](channel) from the owner").await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(mention_count(&t, t.other_id).await, 2);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_can_turn_broadcasts_off_for_everyone_in_it(pool: PgPool) {
    let t = team(pool).await;
    let path = format!("/api/channels/{}", t.channel);

    let (status, _) = send(
        &t.app,
        "PATCH",
        &path,
        Some(&t.member),
        Some(json!({ "allow_broadcast_mentions": false })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a plain member does not decide"
    );

    let (status, body) = send(
        &t.app,
        "PATCH",
        &path,
        Some(&t.lead),
        Some(json!({ "allow_broadcast_mentions": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["settings"]["allow_broadcast_mentions"], false);

    for token in [&t.owner, &t.lead, &t.member] {
        let (status, body) = say(&t, token, "@[channel](channel) hello").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            body.to_string().contains("turned off in this channel"),
            "{body:?}"
        );
    }

    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'channel.broadcast_mentions_changed' AND resource_id = $1 AND user_id = $2",
    )
    .bind(t.channel)
    .bind(t.lead_id)
    .fetch_one(&t.state.pool)
    .await
    .unwrap();
    assert_eq!(audited, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn what_was_queued_before_the_right_was_lost_posts_without_notifying(pool: PgPool) {
    let t = team(pool).await;
    let queued = t
        .state
        .scheduled_repo
        .create(crate::scheduled::repo::NewScheduledMessage {
            workspace_id: t.ws,
            user_id: t.member_id,
            channel_id: t.channel,
            content: "@[channel](channel) reminder",
            send_at: Utc::now() - Duration::seconds(5),
            attachment_ids: &[],
            thread_parent_id: None,
            also_in_channel: false,
        })
        .await
        .unwrap();
    set_rules(&t, json!({ "broadcast_mentions": "admins" })).await;

    let claimed = t.state.scheduled_repo.claim_due().await.unwrap();
    let row = claimed.into_iter().find(|c| c.id == queued.id).unwrap();
    crate::scheduled::executor::deliver_for_test(&t.state, &row)
        .await
        .expect("it still goes out");
    let posted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE channel_id = $1 AND content = '@[channel](channel) reminder'",
    )
    .bind(t.channel)
    .fetch_one(&t.state.pool)
    .await
    .unwrap();
    assert_eq!(posted, 1);
    assert_eq!(mention_count(&t, t.other_id).await, 0);

    assert!(
        expand_mentions(
            &t.state,
            t.channel,
            t.member_id,
            "@[here](here) from a workflow"
        )
        .await
        .is_empty(),
        "a workflow run passes its builder, who may not broadcast"
    );
    assert!(
        expand_mentions(
            &t.state,
            t.channel,
            t.lead_id,
            "@[channel](channel) from a workflow"
        )
        .await
        .contains(&t.other_id),
        "one built by a channel admin still may"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_workflow_that_would_broadcast_without_the_right_is_refused_at_save(pool: PgPool) {
    let t = team(pool).await;
    set_rules(&t, json!({ "broadcast_mentions": "admins" })).await;
    let (status, body) = send(
        &t.app,
        "POST",
        &format!("/api/workspaces/{}/workflows", t.ws),
        Some(&t.member),
        Some(json!({
            "name": "Standup",
            "definition": {
                "trigger": { "type": "schedule", "days": ["mon"], "time": "09:00", "timezone": "UTC" },
                "steps": [{ "type": "send_message", "channel_id": t.channel, "text": "@channel standup" }],
            },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    assert!(
        body.to_string().contains("Step 1: you can't use @channel"),
        "{body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn creating_a_channel_follows_the_rule_for_its_type(pool: PgPool) {
    let t = team(pool).await;
    let (guest_id, _, guest) = seed_and_login(&t.app, &t.state, "bc-guest", false).await;
    add_ws_member(&t.state, t.ws, guest_id, "guest").await;
    let create = |token: String, name: &'static str, channel_type: &'static str| {
        let app = t.app.clone();
        let path = format!("/api/workspaces/{}/channels", t.ws);
        async move {
            send(
                &app,
                "POST",
                &path,
                Some(&token),
                Some(json!({ "name": name, "channel_type": channel_type })),
            )
            .await
        }
    };

    let (status, _) = create(t.member.clone(), "open-default", "public").await;
    assert_eq!(status, StatusCode::OK, "members create by default");
    let (status, _) = create(guest.clone(), "guest-default", "public").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "guests never create");

    set_rules(&t, json!({ "public_channel_creators": "admins" })).await;
    let (status, body) = create(t.member.clone(), "open-limited", "public").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        body.to_string().contains("create public channels"),
        "{body:?}"
    );
    let (status, _) = create(t.member.clone(), "closed-allowed", "private").await;
    assert_eq!(status, StatusCode::OK, "the private rule is separate");

    set_rules(&t, json!({ "private_channel_creators": "admins" })).await;
    let (status, body) = create(t.member.clone(), "closed-limited", "private").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        body.to_string().contains("create private channels"),
        "{body:?}"
    );
    for (name, channel_type) in [("admin-open", "public"), ("admin-closed", "private")] {
        let (status, body) = create(t.owner.clone(), name, channel_type).await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }
}
