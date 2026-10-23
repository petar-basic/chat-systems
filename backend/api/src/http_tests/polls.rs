use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn create(
    app: &axum::Router,
    token: &str,
    channel: impl std::fmt::Display,
    body: Value,
) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/channels/{channel}/polls"),
        Some(token),
        Some(body),
    )
    .await
}

async fn lunch(
    app: &axum::Router,
    token: &str,
    channel: impl std::fmt::Display,
    extra: Value,
) -> Value {
    let mut body = json!({ "question": "Lunch?", "options": ["Thursday", "Friday", "Never"] });
    body.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let (status, msg) = create(app, token, channel, body).await;
    assert_eq!(status, StatusCode::OK, "{msg:?}");
    msg
}

fn poll_of(msg: &Value) -> &Value {
    &msg["metadata"]["poll"]
}

fn option_id(msg: &Value, index: usize) -> String {
    poll_of(msg)["options"][index]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn counts(poll: &Value) -> Vec<i64> {
    poll["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option["votes"].as_i64().unwrap())
        .collect()
}

async fn vote(
    app: &axum::Router,
    token: &str,
    poll_id: &str,
    options: &[&str],
) -> (StatusCode, Value) {
    send(
        app,
        "PUT",
        &format!("/api/polls/{poll_id}/vote"),
        Some(token),
        Some(json!({ "option_ids": options })),
    )
    .await
}

async fn message(state: &crate::state::AppState, id: &str) -> Value {
    let msg = state
        .message_repo
        .find_by_id(Uuid::parse_str(id).unwrap())
        .await
        .unwrap()
        .unwrap();
    serde_json::to_value(msg).unwrap()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_poll_is_a_message_carrying_its_counts(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poll-create", false).await;
    let ws = seed_workspace(&state, owner_id, "Poll WS").await;
    let ch = seed_channel(&state, ws, owner_id, "food", false).await;

    let msg = lunch(
        &app,
        &owner,
        ch,
        json!({ "options": [" Thursday ", "Friday", ""] }),
    )
    .await;
    assert_eq!(
        msg["content"], "Lunch?",
        "the question is the text, so search finds it"
    );
    let poll = poll_of(&msg);
    assert_eq!(poll["question"], "Lunch?");
    assert_eq!(poll["multiple_choice"], false);
    assert_eq!(poll["anonymous"], false);
    assert_eq!(poll["voters"], 0);
    assert_eq!(poll["created_by"], json!(owner_id));
    let labels: Vec<&str> = poll["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["label"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        vec!["Thursday", "Friday"],
        "trimmed, blanks dropped, order kept"
    );

    for (body, why) in [
        (
            json!({ "question": " ", "options": ["a", "b"] }),
            "no question",
        ),
        (
            json!({ "question": "Q", "options": ["only"] }),
            "one option",
        ),
        (
            json!({ "question": "Q", "options": (0..11).map(|i| i.to_string()).collect::<Vec<_>>() }),
            "eleven options",
        ),
        (
            json!({ "question": "Q", "options": ["Yes", "yes"] }),
            "the same option twice",
        ),
        (
            json!({ "question": "Q", "options": ["a", "b"], "closes_at": "2020-01-01T00:00:00Z" }),
            "closing in the past",
        ),
    ] {
        let (status, _) = create(&app, &owner, ch, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    }

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/messages/{}", msg["id"].as_str().unwrap()),
        Some(&owner),
        Some(json!({ "content": "Dinner?" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a poll is not edited"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn single_choice_keeps_one_answer_and_it_can_change_or_go(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poll-single", false).await;
    let ws = seed_workspace(&state, owner_id, "Poll Single WS").await;
    let ch = seed_channel(&state, ws, owner_id, "food", false).await;
    let msg = lunch(&app, &owner, ch, json!({})).await;
    let poll_id = poll_of(&msg)["id"].as_str().unwrap().to_string();
    let (thu, fri) = (option_id(&msg, 0), option_id(&msg, 1));

    let (status, body) = vote(&app, &owner, &poll_id, &[&thu]).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["option_ids"], json!([thu]));
    assert_eq!(counts(&body["poll"]), vec![1, 0, 0]);

    let (_, body) = vote(&app, &owner, &poll_id, &[&fri]).await;
    assert_eq!(
        counts(&body["poll"]),
        vec![0, 1, 0],
        "a new answer replaces the old one"
    );
    assert_eq!(body["poll"]["voters"], 1);
    assert_eq!(
        counts(poll_of(&message(&state, msg["id"].as_str().unwrap()).await)),
        vec![0, 1, 0],
        "the message itself carries the counts"
    );

    let (status, _) = vote(&app, &owner, &poll_id, &[&thu, &fri]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "one answer only");
    let (status, _) = vote(&app, &owner, &poll_id, &[&Uuid::new_v4().to_string()]).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "an option of another poll"
    );

    let (_, mine) = send(
        &app,
        "GET",
        &format!("/api/polls/{poll_id}/vote"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(mine["option_ids"], json!([fri]));

    let (_, body) = vote(&app, &owner, &poll_id, &[]).await;
    assert_eq!(
        counts(&body["poll"]),
        vec![0, 0, 0],
        "an empty answer withdraws the vote"
    );
    assert_eq!(body["poll"]["voters"], 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn two_clicks_at_once_still_leave_one_answer(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poll-race", false).await;
    let ws = seed_workspace(&state, owner_id, "Poll Race WS").await;
    let ch = seed_channel(&state, ws, owner_id, "food", false).await;
    let msg = lunch(&app, &owner, ch, json!({})).await;
    let poll_id = poll_of(&msg)["id"].as_str().unwrap().to_string();
    let (thu, fri) = (option_id(&msg, 0), option_id(&msg, 1));

    for _ in 0..5 {
        let (first, second) = ([thu.as_str()], [fri.as_str()]);
        let (a, b) = tokio::join!(
            vote(&app, &owner, &poll_id, &first),
            vote(&app, &owner, &poll_id, &second)
        );
        assert_eq!((a.0, b.0), (StatusCode::OK, StatusCode::OK));
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM poll_votes WHERE poll_id = $1")
            .bind(Uuid::parse_str(&poll_id).unwrap())
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(rows, 1);
        let stored = message(&state, msg["id"].as_str().unwrap()).await;
        assert_eq!(
            counts(poll_of(&stored)).iter().sum::<i64>(),
            1,
            "the counts agree with the rows"
        );
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn multiple_choice_keeps_any_subset(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poll-multi", false).await;
    let ws = seed_workspace(&state, owner_id, "Poll Multi WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "poll-multi-m", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let ch = seed_channel(&state, ws, owner_id, "food", false).await;
    let msg = lunch(&app, &owner, ch, json!({ "multiple_choice": true })).await;
    let poll_id = poll_of(&msg)["id"].as_str().unwrap().to_string();
    let ids: Vec<String> = (0..3).map(|i| option_id(&msg, i)).collect();

    vote(&app, &owner, &poll_id, &[&ids[0], &ids[2]]).await;
    let (status, body) = vote(&app, &member, &poll_id, &[&ids[2], &ids[1], &ids[2]]).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body["option_ids"],
        json!([ids[1], ids[2]]),
        "in the poll's order, once each"
    );
    assert_eq!(counts(&body["poll"]), vec![1, 1, 2]);
    assert_eq!(body["poll"]["voters"], 2);

    let (_, body) = vote(&app, &owner, &poll_id, &[&ids[1]]).await;
    assert_eq!(counts(&body["poll"]), vec![0, 2, 1]);

    let (status, voters) = send(
        &app,
        "GET",
        &format!("/api/polls/{poll_id}/votes"),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut second: Vec<String> = voters["data"][1]["user_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect();
    second.sort();
    let mut expected = vec![owner_id.to_string(), member_id.to_string()];
    expected.sort();
    assert_eq!(second, expected);
    assert_eq!(voters["data"][0]["user_ids"], json!([]));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn who_may_create_vote_and_read_a_poll(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poll-authz-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Poll Authz WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "poll-authz-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "poll-authz-guest", false).await;
    add_ws_member(&state, ws, guest_id, "guest").await;
    let (_, _, stranger) = seed_and_login(&app, &state, "poll-authz-stranger", false).await;

    let public = seed_channel(&state, ws, owner_id, "lobby", false).await;
    let private = seed_channel(&state, ws, owner_id, "board", true).await;
    let announce = seed_channel(&state, ws, owner_id, "news", false).await;
    sqlx::query("UPDATE channels SET settings = settings || '{\"post_policy\": \"moderators\"}' WHERE id = $1")
        .bind(announce)
        .execute(&state.pool)
        .await
        .unwrap();

    let open = lunch(&app, &owner, public, json!({})).await;
    let closed_room = lunch(&app, &owner, private, json!({})).await;
    let news = lunch(&app, &owner, announce, json!({})).await;

    let body = json!({ "question": "Q", "options": ["a", "b"] });
    for (token, channel, expected, why) in [
        (
            &member,
            public,
            StatusCode::OK,
            "a member in a public channel",
        ),
        (
            &member,
            private,
            StatusCode::FORBIDDEN,
            "a member outside a private channel",
        ),
        (
            &guest,
            public,
            StatusCode::FORBIDDEN,
            "a guest outside the channel",
        ),
        (
            &stranger,
            public,
            StatusCode::FORBIDDEN,
            "not in the workspace",
        ),
        (
            &member,
            announce,
            StatusCode::FORBIDDEN,
            "only moderators post in an announcement channel",
        ),
    ] {
        assert_eq!(
            create(&app, token, channel, body.clone()).await.0,
            expected,
            "create: {why}"
        );
    }

    for (token, msg, expected, why) in [
        (
            &member,
            &open,
            StatusCode::OK,
            "a member in a public channel",
        ),
        (
            &member,
            &news,
            StatusCode::OK,
            "everybody votes in an announcement channel",
        ),
        (
            &member,
            &closed_room,
            StatusCode::FORBIDDEN,
            "a private channel they are not in",
        ),
        (
            &guest,
            &open,
            StatusCode::FORBIDDEN,
            "a guest guessing the id",
        ),
        (
            &stranger,
            &open,
            StatusCode::FORBIDDEN,
            "not in the workspace",
        ),
    ] {
        let poll_id = poll_of(msg)["id"].as_str().unwrap();
        let option = option_id(msg, 0);
        assert_eq!(
            vote(&app, token, poll_id, &[&option]).await.0,
            expected,
            "vote: {why}"
        );
        for path in ["vote", "votes"] {
            let (status, _) = send(
                &app,
                "GET",
                &format!("/api/polls/{poll_id}/{path}"),
                Some(token),
                None,
            )
            .await;
            assert_eq!(status, expected, "{path}: {why}");
        }
    }

    let (status, _) = vote(&app, &owner, &Uuid::new_v4().to_string(), &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{}", open["id"].as_str().unwrap()),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let poll_id = poll_of(&open)["id"].as_str().unwrap();
    assert_eq!(
        vote(&app, &member, poll_id, &[]).await.0,
        StatusCode::NOT_FOUND,
        "a deleted message's poll takes no votes"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_the_creator_or_a_moderator_closes_and_a_closed_poll_refuses_votes(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poll-close-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Poll Close WS").await;
    let (author_id, _, author) = seed_and_login(&app, &state, "poll-close-author", false).await;
    add_ws_member(&state, ws, author_id, "member").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "poll-close-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let ch = seed_channel(&state, ws, owner_id, "food", false).await;

    let first = lunch(&app, &author, ch, json!({})).await;
    let second = lunch(&app, &author, ch, json!({})).await;
    let close = |msg: &Value| format!("/api/polls/{}/close", poll_of(msg)["id"].as_str().unwrap());

    let (status, _) = send(&app, "POST", &close(&first), Some(&member), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "another member");
    let (status, body) = send(&app, "POST", &close(&first), Some(&author), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["closed_at"].is_string());
    let (status, _) = send(&app, "POST", &close(&first), Some(&author), None).await;
    assert_eq!(status, StatusCode::CONFLICT, "already closed");
    let (status, _) = send(&app, "POST", &close(&second), Some(&owner), None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a workspace admin moderates the channel"
    );

    assert!(
        poll_of(&message(&state, first["id"].as_str().unwrap()).await)["closed_at"].is_string()
    );
    let option = option_id(&first, 0);
    let (status, _) = vote(
        &app,
        &member,
        poll_of(&first)["id"].as_str().unwrap(),
        &[&option],
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_poll_closes_itself_at_its_closing_time(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poll-due", false).await;
    let ws = seed_workspace(&state, owner_id, "Poll Due WS").await;
    let ch = seed_channel(&state, ws, owner_id, "food", false).await;
    let later = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let due = lunch(&app, &owner, ch, json!({ "closes_at": later })).await;
    let not_yet = lunch(&app, &owner, ch, json!({ "closes_at": later })).await;
    let poll_id = poll_of(&due)["id"].as_str().unwrap().to_string();

    sqlx::query("UPDATE polls SET closes_at = NOW() - INTERVAL '1 minute' WHERE id = $1")
        .bind(Uuid::parse_str(&poll_id).unwrap())
        .execute(&state.pool)
        .await
        .unwrap();
    let option = option_id(&due, 0);
    let (status, _) = vote(&app, &owner, &poll_id, &[&option]).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "past its time it refuses votes before the worker runs"
    );

    assert_eq!(crate::polls::service::close_due(&state).await, 1);
    assert_eq!(
        crate::polls::service::close_due(&state).await,
        0,
        "closed once"
    );
    let stored = message(&state, due["id"].as_str().unwrap()).await;
    assert!(
        poll_of(&stored)["closed_at"].is_string(),
        "every reader learns it closed"
    );
    let untouched = message(&state, not_yet["id"].as_str().unwrap()).await;
    assert!(poll_of(&untouched)["closed_at"].is_null());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn nobody_learns_who_voted_on_an_anonymous_poll(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poll-anon", false).await;
    let ws = seed_workspace(&state, owner_id, "Poll Anon WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "poll-anon-m", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let ch = seed_channel(&state, ws, owner_id, "food", false).await;
    let secret = lunch(&app, &owner, ch, json!({ "anonymous": true })).await;
    let open = lunch(&app, &owner, ch, json!({})).await;
    let secret_id = poll_of(&secret)["id"].as_str().unwrap().to_string();

    let option = option_id(&secret, 1);
    let (_, body) = vote(&app, &member, &secret_id, &[&option]).await;
    assert_eq!(counts(&body["poll"]), vec![0, 1, 0], "counts are public");
    let option = option_id(&secret, 0);
    vote(&app, &owner, &secret_id, &[&option]).await;
    let option = option_id(&open, 0);
    vote(
        &app,
        &member,
        poll_of(&open)["id"].as_str().unwrap(),
        &[&option],
    )
    .await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/polls/{secret_id}/votes"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "not even its creator, who is the workspace owner"
    );
    let (_, mine) = send(
        &app,
        "GET",
        &format!("/api/polls/{secret_id}/vote"),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(
        mine["option_ids"],
        json!([option_id(&secret, 1)]),
        "a voter still sees their own answer"
    );
    let stored = message(&state, secret["id"].as_str().unwrap()).await;
    assert!(
        !stored.to_string().contains(&member_id.to_string()),
        "the message carries counts, never voters"
    );

    send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&owner),
        Some(json!({})),
    )
    .await;
    let job = state
        .export_repo
        .claim_next()
        .await
        .unwrap()
        .expect("a pending export");
    let id = job.id;
    crate::export::job::run_for_test(&state, job)
        .await
        .expect("export runs");
    let manifest = state
        .export_repo
        .find(id)
        .await
        .unwrap()
        .unwrap()
        .manifest
        .unwrap();
    assert_eq!(manifest["files"]["polls.jsonl"]["rows"], 2);
    assert_eq!(
        manifest["files"]["poll_votes.jsonl"]["rows"], 1,
        "the export holds the open poll's vote and only counts for the anonymous one"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn polls_work_in_threads_and_direct_messages(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poll-places", false).await;
    let ws = seed_workspace(&state, owner_id, "Poll Places WS").await;
    let (peer_id, _, peer) = seed_and_login(&app, &state, "poll-places-peer", false).await;
    add_ws_member(&state, ws, peer_id, "member").await;
    let ch = seed_channel(&state, ws, owner_id, "food", false).await;

    let (_, root) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "Planning lunch" })),
    )
    .await;
    let in_thread = lunch(&app, &owner, ch, json!({ "thread_parent_id": root["id"] })).await;
    assert_eq!(in_thread["thread_parent_id"], root["id"]);

    let (status, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/conversations"),
        Some(&owner),
        Some(json!({ "participant_ids": [peer_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{conv:?}");
    let dm = conv["id"].as_str().unwrap();
    let in_dm = lunch(&app, &owner, dm, json!({})).await;
    let option = option_id(&in_dm, 2);
    let (status, _) = vote(
        &app,
        &peer,
        poll_of(&in_dm)["id"].as_str().unwrap(),
        &[&option],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
