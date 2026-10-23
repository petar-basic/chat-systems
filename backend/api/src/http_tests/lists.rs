use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn create_list(
    app: &axum::Router,
    token: &str,
    ws: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/lists"),
        Some(token),
        Some(body),
    )
    .await
}

async fn list_in(
    app: &axum::Router,
    token: &str,
    ws: Uuid,
    channel: Option<Uuid>,
    columns: Value,
) -> Value {
    let (status, body) = create_list(
        app,
        token,
        ws,
        json!({ "title": "Launch plan", "channel_id": channel, "columns": columns }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body
}

fn list_id(detail: &Value) -> &str {
    detail["list"]["id"].as_str().unwrap()
}

fn column_id(detail: &Value, index: usize) -> String {
    detail["columns"][index]["id"].as_str().unwrap().to_string()
}

async fn add_item(app: &axum::Router, token: &str, list: &str, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/lists/{list}/items"),
        Some(token),
        Some(body),
    )
    .await
}

async fn edit_item(
    app: &axum::Router,
    token: &str,
    item: &str,
    values: Value,
) -> (StatusCode, Value) {
    send(
        app,
        "PATCH",
        &format!("/api/list-items/{item}"),
        Some(token),
        Some(json!({ "values": values })),
    )
    .await
}

async fn items(app: &axum::Router, token: &str, list: &str) -> Vec<Value> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/lists/{list}/items"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"].as_array().unwrap().clone()
}

fn text_column(name: &str) -> Value {
    json!({ "name": name, "kind": "text" })
}

async fn announcement(state: &crate::state::AppState, ws: Uuid, owner: Uuid) -> Uuid {
    let channel = seed_channel(state, ws, owner, "news", false).await;
    sqlx::query("UPDATE channels SET settings = settings || '{\"post_policy\": \"moderators\"}' WHERE id = $1")
        .bind(channel)
        .execute(&state.pool)
        .await
        .unwrap();
    channel
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_list_is_seen_and_edited_exactly_as_its_conversation(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "list-authz-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "List Authz WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "list-authz-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "list-authz-guest", false).await;
    add_ws_member(&state, ws, guest_id, "guest").await;
    let (_, _, stranger) = seed_and_login(&app, &state, "list-authz-stranger", false).await;

    let public = seed_channel(&state, ws, owner_id, "lobby", false).await;
    let private = seed_channel(&state, ws, owner_id, "board", true).await;
    let news = announcement(&state, ws, owner_id).await;

    let columns = json!([text_column("Title")]);
    let open = list_in(&app, &owner, ws, Some(public), columns.clone()).await;
    let closed = list_in(&app, &owner, ws, Some(private), columns.clone()).await;
    let announced = list_in(&app, &owner, ws, Some(news), columns.clone()).await;
    let personal = list_in(&app, &owner, ws, None, columns.clone()).await;

    let cases = [
        (
            &member,
            &open,
            StatusCode::OK,
            StatusCode::OK,
            "a member, public channel",
        ),
        (
            &member,
            &closed,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            "a member outside a private channel",
        ),
        (
            &member,
            &announced,
            StatusCode::OK,
            StatusCode::FORBIDDEN,
            "a member who cannot post in an announcement channel",
        ),
        (
            &member,
            &personal,
            StatusCode::NOT_FOUND,
            StatusCode::NOT_FOUND,
            "someone else's personal list",
        ),
        (
            &guest,
            &open,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            "a guest outside the channel",
        ),
        (
            &stranger,
            &open,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            "not in the workspace",
        ),
        (
            &stranger,
            &personal,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            "not in the workspace, personal list",
        ),
        (
            &owner,
            &announced,
            StatusCode::OK,
            StatusCode::OK,
            "the announcement channel's moderator",
        ),
        (
            &owner,
            &personal,
            StatusCode::OK,
            StatusCode::OK,
            "a personal list's creator",
        ),
    ];
    for (token, detail, read, write, why) in cases {
        let list = list_id(detail);
        let (status, _) = send(
            &app,
            "GET",
            &format!("/api/lists/{list}"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, read, "read: {why}");
        let (status, _) = send(
            &app,
            "GET",
            &format!("/api/lists/{list}/items"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, read, "read items: {why}");
        let (status, _) = add_item(&app, token, list, json!({})).await;
        assert_eq!(status, write, "add an item: {why}");
        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/lists/{list}/columns"),
            Some(token),
            Some(text_column("Notes")),
        )
        .await;
        assert_eq!(status, write, "add a column: {why}");
    }

    state
        .workspace_service
        .repo
        .add_channel_member(
            public,
            guest_id,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .unwrap();
    let (status, _) = add_item(&app, &guest, list_id(&open), json!({})).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a guest in the channel edits its list"
    );

    for (token, channel, expected, why) in [
        (
            &member,
            Some(private),
            StatusCode::FORBIDDEN,
            "a list in a channel the member is not in",
        ),
        (
            &member,
            Some(news),
            StatusCode::FORBIDDEN,
            "a list where only moderators post",
        ),
        (
            &stranger,
            None,
            StatusCode::FORBIDDEN,
            "a personal list outside the workspace",
        ),
        (
            &member,
            None,
            StatusCode::OK,
            "a personal list of one's own",
        ),
    ] {
        let (status, _) = create_list(
            &app,
            token,
            ws,
            json!({ "title": "Mine", "channel_id": channel }),
        )
        .await;
        assert_eq!(status, expected, "create: {why}");
    }

    let titles = |body: &Value| -> Vec<String> {
        let mut ids: Vec<String> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|list| list["id"].as_str().unwrap().to_string())
            .collect();
        ids.sort();
        ids
    };
    let (_, seen) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/lists"),
        Some(&member),
        None,
    )
    .await;
    let seen = titles(&seen);
    assert!(seen.contains(&list_id(&open).to_string()));
    assert!(seen.contains(&list_id(&announced).to_string()));
    assert!(
        !seen.contains(&list_id(&closed).to_string()),
        "a private channel's list stays hidden"
    );
    assert!(
        !seen.contains(&list_id(&personal).to_string()),
        "someone else's personal list stays hidden"
    );
    let (_, in_lobby) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/lists?channel_id={public}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(titles(&in_lobby), vec![list_id(&open).to_string()]);
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/lists?channel_id={private}"),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/lists/{}", list_id(&open)),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "only its creator or a moderator deletes a list"
    );
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/lists/{}", list_id(&open)),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/lists/{}", list_id(&open)),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a deleted list is gone");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn every_kind_of_cell_is_checked_by_the_server_and_kept(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "list-kinds", false).await;
    let ws = seed_workspace(&state, owner_id, "List Kinds WS").await;
    let (outsider_id, _, _) = seed_and_login(&app, &state, "list-kinds-outsider", false).await;
    add_ws_member(&state, ws, outsider_id, "member").await;
    let channel = seed_channel(&state, ws, owner_id, "team", true).await;

    let detail = list_in(
        &app,
        &owner,
        ws,
        Some(channel),
        json!([
            text_column("Title"),
            { "name": "Points", "kind": "number", "options": { "number": { "style": "currency", "decimals": 2, "currency": "eur" } } },
            { "name": "Status", "kind": "select", "options": { "choices": [{ "label": "To do" }, { "label": "Done", "color": "green" }] } },
            { "name": "Tags", "kind": "multi_select", "options": { "choices": [{ "label": "web" }, { "label": "api" }] } },
            { "name": "Owner", "kind": "person" },
            { "name": "Due", "kind": "date" },
            { "name": "Shipped", "kind": "checkbox" },
            { "name": "Link", "kind": "link" },
        ]),
    )
    .await;
    let columns = &detail["columns"];
    assert_eq!(columns[1]["options"]["number"]["currency"], "EUR");
    assert_eq!(columns[2]["options"]["choices"][1]["color"], "green");
    let col = |i: usize| column_id(&detail, i);
    let choice =
        |column: usize, index: usize| columns[column]["options"]["choices"][index]["id"].clone();
    let list = list_id(&detail);

    let values = json!({
        col(0): "Fix login",
        col(1): 12.5,
        col(2): choice(2, 1),
        col(3): [choice(3, 1), choice(3, 0)],
        col(4): owner_id,
        col(5): "2026-10-30",
        col(6): true,
        col(7): "https://example.com/issue/1",
    });
    let (status, item) = add_item(&app, &owner, list, json!({ "values": values })).await;
    assert_eq!(status, StatusCode::OK, "{item:?}");

    let stored = &items(&app, &owner, list).await[0]["values"];
    assert_eq!(stored[col(0)], "Fix login");
    assert_eq!(stored[col(1)], 12.5);
    assert_eq!(stored[col(2)], choice(2, 1));
    assert_eq!(
        stored[col(3)],
        json!([choice(3, 0), choice(3, 1)]),
        "choices keep the column's order"
    );
    assert_eq!(stored[col(4)], owner_id.to_string());
    assert_eq!(stored[col(5)], "2026-10-30");
    assert_eq!(stored[col(6)], true);
    assert_eq!(stored[col(7)], "https://example.com/issue/1");

    let item_id = item["id"].as_str().unwrap();
    for (column, value, why) in [
        (col(0), json!(5), "text takes text"),
        (col(1), json!("12"), "a number takes a number"),
        (
            col(2),
            json!(Uuid::new_v4()),
            "a select takes one of its choices",
        ),
        (col(3), json!(["web"]), "a multi-select takes choice ids"),
        (
            col(4),
            json!(outsider_id),
            "a person must be in the conversation",
        ),
        (col(4), json!("ana"), "a person is an id"),
        (col(5), json!("30/10/2026"), "a date is YYYY-MM-DD"),
        (col(6), json!("yes"), "a checkbox takes true or false"),
        (
            col(7),
            json!("ftp://example.com"),
            "a link is http or https",
        ),
        (
            Uuid::new_v4().to_string(),
            json!("x"),
            "a column of the list",
        ),
    ] {
        let (status, body) = edit_item(&app, &owner, item_id, json!({ column: value })).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}: {body:?}");
    }
    let (status, body) = add_item(
        &app,
        &owner,
        list,
        json!({ "values": { col(4): outsider_id } }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");

    let (status, body) =
        edit_item(&app, &owner, item_id, json!({ col(0): null, col(3): [] })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["values"].get(col(0)).is_none(), "null clears a cell");
    assert!(
        body["values"].get(col(3)).is_none(),
        "an empty choice list clears a cell"
    );
    assert_eq!(
        body["values"][col(1)],
        12.5,
        "cells not named keep their value"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn two_people_editing_one_row_both_keep_their_edit(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ana_id, _, ana) = seed_and_login(&app, &state, "list-merge-ana", false).await;
    let ws = seed_workspace(&state, ana_id, "List Merge WS").await;
    let (bo_id, _, bo) = seed_and_login(&app, &state, "list-merge-bo", false).await;
    add_ws_member(&state, ws, bo_id, "member").await;
    let channel = seed_channel(&state, ws, ana_id, "work", false).await;
    let detail = list_in(
        &app,
        &ana,
        ws,
        Some(channel),
        json!([text_column("Title"), text_column("Notes")]),
    )
    .await;
    let (title, notes) = (column_id(&detail, 0), column_id(&detail, 1));
    let (_, item) = add_item(&app, &ana, list_id(&detail), json!({})).await;
    let item = item["id"].as_str().unwrap();

    for round in 0..5 {
        let first = json!({ title.clone(): format!("title {round}") });
        let second = json!({ notes.clone(): format!("notes {round}") });
        let (a, b) = tokio::join!(
            edit_item(&app, &ana, item, first),
            edit_item(&app, &bo, item, second),
        );
        assert_eq!((a.0, b.0), (StatusCode::OK, StatusCode::OK));
        let row = &items(&app, &ana, list_id(&detail)).await[0]["values"];
        assert_eq!(row[&title], format!("title {round}"), "round {round}");
        assert_eq!(row[&notes], format!("notes {round}"), "round {round}");
    }

    edit_item(&app, &ana, item, json!({ title.clone(): "Ana's" })).await;
    let (_, last) = edit_item(&app, &bo, item, json!({ title.clone(): "Bo's" })).await;
    assert_eq!(
        last["values"][&title], "Bo's",
        "the same cell is last write wins"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn items_move_by_writing_one_rank(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "list-rank", false).await;
    let ws = seed_workspace(&state, owner_id, "List Rank WS").await;
    let detail = list_in(&app, &owner, ws, None, json!([text_column("Title")])).await;
    let (list, title) = (list_id(&detail).to_string(), column_id(&detail, 0));

    let mut ids = Vec::new();
    for name in ["a", "b", "c"] {
        let (_, item) = add_item(
            &app,
            &owner,
            &list,
            json!({ "values": { title.clone(): name } }),
        )
        .await;
        ids.push(item["id"].as_str().unwrap().to_string());
    }
    let order = |rows: Vec<Value>| -> Vec<String> {
        rows.iter()
            .map(|row| row["values"][&title].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(order(items(&app, &owner, &list).await), ["a", "b", "c"]);

    let mv = |item: String, after: Option<String>| {
        let app = app.clone();
        let owner = owner.clone();
        async move {
            send(
                &app,
                "POST",
                &format!("/api/list-items/{item}/move"),
                Some(&owner),
                Some(json!({ "after_id": after })),
            )
            .await
        }
    };
    let before = items(&app, &owner, &list).await;
    let (status, moved) = mv(ids[2].clone(), None).await;
    assert_eq!(status, StatusCode::OK, "{moved:?}");
    let after = items(&app, &owner, &list).await;
    assert_eq!(order(after.clone()), ["c", "a", "b"]);
    for (old, new) in before.iter().zip(after.iter().skip(1)) {
        assert_eq!(old["rank"], new["rank"], "only the moved item changes rank");
    }

    assert_eq!(
        mv(ids[0].clone(), Some(ids[1].clone())).await.0,
        StatusCode::OK
    );
    assert_eq!(order(items(&app, &owner, &list).await), ["c", "b", "a"]);

    let (_, inserted) = add_item(
        &app,
        &owner,
        &list,
        json!({ "values": { title.clone(): "between" }, "after_id": ids[2] }),
    )
    .await;
    assert!(inserted["id"].is_string());
    assert_eq!(
        order(items(&app, &owner, &list).await),
        ["c", "between", "b", "a"]
    );

    assert_eq!(
        mv(ids[0].clone(), Some(ids[0].clone())).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        mv(ids[0].clone(), Some(Uuid::new_v4().to_string())).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/list-items/{}", ids[1]),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        order(items(&app, &owner, &list).await),
        ["c", "between", "a"]
    );
    assert_eq!(
        mv(ids[1].clone(), None).await.0,
        StatusCode::NOT_FOUND,
        "a deleted item does not move"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn changing_a_column_kind_converts_what_it_can_after_naming_the_loss(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "list-convert", false).await;
    let ws = seed_workspace(&state, owner_id, "List Convert WS").await;
    let detail = list_in(&app, &owner, ws, None, json!([text_column("Estimate")])).await;
    let list = list_id(&detail).to_string();
    let column = column_id(&detail, 0);
    for text in ["3", "lots", "4.5"] {
        add_item(
            &app,
            &owner,
            &list,
            json!({ "values": { column.clone(): text } }),
        )
        .await;
    }
    add_item(&app, &owner, &list, json!({})).await;

    let patch = |body: Value| {
        let (app, owner, column) = (app.clone(), owner.clone(), column.clone());
        async move {
            send(
                &app,
                "PATCH",
                &format!("/api/list-columns/{column}"),
                Some(&owner),
                Some(body),
            )
            .await
        }
    };

    let (status, preview) = send(
        &app,
        "POST",
        &format!("/api/list-columns/{column}/preview"),
        Some(&owner),
        Some(json!({ "kind": "number" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview, json!({ "converted": 2, "cleared": 1 }));

    let (status, refused) = patch(json!({ "kind": "number" })).await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused:?}");
    assert!(refused.to_string().contains("clears 1 cell"), "{refused:?}");
    assert_eq!(
        patch(json!({ "kind": "number", "confirm_cleared": 2 }))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (status, changed) = patch(json!({ "kind": "number", "confirm_cleared": 1 })).await;
    assert_eq!(status, StatusCode::OK, "{changed:?}");
    assert_eq!(changed["kind"], "number");
    let numbers: Vec<Value> = items(&app, &owner, &list)
        .await
        .iter()
        .map(|row| row["values"].get(&column).cloned().unwrap_or(Value::Null))
        .collect();
    assert_eq!(
        numbers,
        vec![json!(3.0), Value::Null, json!(4.5), Value::Null]
    );

    let (status, _) = patch(json!({ "kind": "text", "name": "Estimate (days)" })).await;
    assert_eq!(status, StatusCode::OK, "number to text loses nothing");
    let first = &items(&app, &owner, &list).await[0]["values"][&column];
    assert_eq!(first, "3");

    let (status, select) = patch(json!({
        "kind": "select",
        "options": { "choices": [{ "label": "3" }, { "label": "Later" }] },
        "confirm_cleared": 1,
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{select:?}");
    let three = select["options"]["choices"][0]["id"].clone();
    let later = select["options"]["choices"][1]["id"].clone();
    assert_eq!(
        items(&app, &owner, &list).await[0]["values"][&column],
        three
    );

    let (status, _) =
        patch(json!({ "options": { "choices": [{ "id": later, "label": "Later" }] } })).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "dropping a choice in use clears its cells"
    );
    let (status, _) = patch(json!({
        "options": { "choices": [{ "id": three, "label": "Three" }, { "id": later, "label": "Later" }] },
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "renaming a choice keeps its cells");

    let (status, _) = patch(json!({ "kind": "text" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        items(&app, &owner, &list).await[0]["values"][&column],
        "Three"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn columns_and_items_have_limits_and_columns_keep_an_order(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "list-limits", false).await;
    let ws = seed_workspace(&state, owner_id, "List Limits WS").await;

    let many: Vec<Value> = (0..31).map(|i| text_column(&format!("c{i}"))).collect();
    let (status, _) = create_list(
        &app,
        &owner,
        ws,
        json!({ "title": "Wide", "columns": many }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "31 columns is one too many"
    );
    let thirty: Vec<Value> = (0..30).map(|i| text_column(&format!("c{i}"))).collect();
    let detail = list_in(&app, &owner, ws, None, json!(thirty)).await;
    let list = list_id(&detail).to_string();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/lists/{list}/columns"),
        Some(&owner),
        Some(text_column("c30")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let first = column_id(&detail, 0);
    let (_, item) = add_item(
        &app,
        &owner,
        &list,
        json!({ "values": { first.clone(): "kept?" } }),
    )
    .await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/list-columns/{first}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let row = &items(&app, &owner, &list).await[0];
    assert_eq!(row["id"], item["id"]);
    assert!(
        row["values"].get(&first).is_none(),
        "a deleted column leaves no cells behind"
    );

    let (_, detail) = send(
        &app,
        "GET",
        &format!("/api/lists/{list}"),
        Some(&owner),
        None,
    )
    .await;
    let mut order: Vec<String> = detail["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    order.reverse();
    let (status, reordered) = send(
        &app,
        "PUT",
        &format!("/api/lists/{list}/column-order"),
        Some(&owner),
        Some(json!({ "column_ids": order })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reordered:?}");
    assert_eq!(reordered["data"][0]["id"], json!(order[0]));
    let (status, hidden) = send(
        &app,
        "PATCH",
        &format!("/api/list-columns/{}", order[0]),
        Some(&owner),
        Some(json!({ "hidden": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{hidden:?}");
    assert_eq!(hidden["hidden"], json!(true));
    let (_, renamed) = send(
        &app,
        "PATCH",
        &format!("/api/list-columns/{}", order[0]),
        Some(&owner),
        Some(json!({ "name": "Renamed" })),
    )
    .await;
    assert_eq!(
        renamed["hidden"],
        json!(true),
        "a rename leaves a hidden column hidden"
    );
    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/lists/{list}/column-order"),
        Some(&owner),
        Some(json!({ "column_ids": &order[1..] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "every column, exactly once"
    );

    sqlx::query(
        "INSERT INTO list_items (list_id, rank, created_by)
         SELECT $1, lpad(g::text, 6, '1'), $2 FROM generate_series(2, 10000) g",
    )
    .bind(list.parse::<Uuid>().unwrap())
    .bind(owner_id)
    .execute(&state.pool)
    .await
    .unwrap();
    let (status, body) = add_item(&app, &owner, &list, json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn every_change_is_staged_for_the_lists_audience(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "list-events", false).await;
    let ws = seed_workspace(&state, owner_id, "List Events WS").await;
    let channel = seed_channel(&state, ws, owner_id, "ops", false).await;

    let shared = list_in(
        &app,
        &owner,
        ws,
        Some(channel),
        json!([text_column("Title")]),
    )
    .await;
    let personal = list_in(&app, &owner, ws, None, json!([])).await;
    let (_, item) = add_item(&app, &owner, list_id(&shared), json!({})).await;
    let item = item["id"].as_str().unwrap();
    edit_item(&app, &owner, item, json!({ column_id(&shared, 0): "x" })).await;
    send(
        &app,
        "DELETE",
        &format!("/api/list-items/{item}"),
        Some(&owner),
        None,
    )
    .await;
    send(
        &app,
        "POST",
        &format!("/api/lists/{}/columns", list_id(&personal)),
        Some(&owner),
        Some(text_column("Notes")),
    )
    .await;
    send(
        &app,
        "DELETE",
        &format!("/api/lists/{}", list_id(&personal)),
        Some(&owner),
        None,
    )
    .await;

    let rows: Vec<(String, Value)> = sqlx::query_as(
        "SELECT event_type, payload FROM event_outbox
          WHERE event_type LIKE 'list.%' AND workspace_id = $1 ORDER BY id",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    let types: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(
        types,
        [
            "list.upserted",
            "list.upserted",
            "list.item_upserted",
            "list.item_upserted",
            "list.item_deleted",
            "list.columns_changed",
            "list.deleted",
        ]
    );
    for (event_type, payload) in &rows {
        let frame = shared_events::frames::ServerFrame::from_event(event_type, payload);
        assert!(
            frame.is_ok(),
            "{event_type} reads back as its frame: {frame:?}"
        );
    }
    assert_eq!(
        rows[2].1["channel_id"],
        json!(channel),
        "a channel list's events go to its channel"
    );
    assert_eq!(
        rows[3].1["item"]["values"][column_id(&shared, 0)],
        "x",
        "the row as written"
    );
    assert_eq!(rows[6].1["channel_id"], Value::Null);
    assert_eq!(
        rows[6].1["owner_id"],
        json!(owner_id),
        "a personal list's events go to its owner"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn exports_retention_and_channel_deletion_cover_lists(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "list-lifecycle", false).await;
    let ws = seed_workspace(&state, owner_id, "List Lifecycle WS").await;
    let (peer_id, _, _) = seed_and_login(&app, &state, "list-lifecycle-peer", false).await;
    add_ws_member(&state, ws, peer_id, "member").await;
    let channel = seed_channel(&state, ws, owner_id, "plans", false).await;
    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/conversations"),
        Some(&owner),
        Some(json!({ "participant_ids": [peer_id] })),
    )
    .await;
    let dm: Uuid = conv["id"].as_str().unwrap().parse().unwrap();

    let shared = list_in(
        &app,
        &owner,
        ws,
        Some(channel),
        json!([text_column("Title")]),
    )
    .await;
    let title = column_id(&shared, 0);
    for name in ["old", "new"] {
        add_item(
            &app,
            &owner,
            list_id(&shared),
            json!({ "values": { title.clone(): name } }),
        )
        .await;
    }
    list_in(&app, &owner, ws, Some(dm), json!([])).await;
    list_in(&app, &owner, ws, None, json!([])).await;

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
    assert_eq!(
        manifest["files"]["lists.jsonl"]["rows"], 1,
        "without DMs the export leaves out the DM's list and the personal one"
    );
    let csv = format!("lists/{}.csv", list_id(&shared));
    assert_eq!(manifest["files"][&csv]["rows"], 2, "{manifest:#}");

    let shared_id: Uuid = list_id(&shared).parse().unwrap();
    sqlx::query("UPDATE list_items SET updated_at = NOW() - INTERVAL '40 days' WHERE \"values\"->>$2 = 'old' AND list_id = $1")
        .bind(shared_id)
        .bind(&title)
        .execute(&state.pool)
        .await
        .unwrap();
    let gone = list_in(&app, &owner, ws, Some(channel), json!([])).await;
    send(
        &app,
        "DELETE",
        &format!("/api/lists/{}", list_id(&gone)),
        Some(&owner),
        None,
    )
    .await;
    sqlx::query("UPDATE lists SET deleted_at = NOW() - INTERVAL '40 days' WHERE id = $1")
        .bind(list_id(&gone).parse::<Uuid>().unwrap())
        .execute(&state.pool)
        .await
        .unwrap();
    state
        .retention_repo
        .upsert(
            ws,
            &crate::retention::repo::UpdateRetentionRequest {
                message_days: Some(30),
                file_days: None,
                audit_days: Some(730),
                notification_days: Some(90),
            },
            owner_id,
        )
        .await
        .unwrap();
    let counts = crate::retention::job::run_once(&state).await;
    assert_eq!((counts.list_items, counts.lists), (1, 1));
    let left: Vec<String> = items(&app, &owner, list_id(&shared))
        .await
        .iter()
        .map(|row| row["values"][&title].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        left,
        ["new"],
        "an item untouched past the window goes, as a message would"
    );

    sqlx::query("DELETE FROM channels WHERE id = $1")
        .bind(channel)
        .execute(&state.pool)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lists WHERE channel_id = $1")
        .bind(channel)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0, "a deleted channel takes its lists with it");
}
