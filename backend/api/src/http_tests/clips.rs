use super::common::*;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const BOUNDARY: &str = "----clipboundary";
const WEBM: &[u8] = &[
    0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 0x01, 0x42, 0xF7, 0x81, 0x01, 0x42, 0xF2, 0x81,
    0x04, 0x42,
];
const MP4: &[u8] = b"\0\0\0\x20ftypM4A \0\0\0\0isomM4A mp42";

enum Part<'a> {
    Text(&'a str, &'a str),
    File(&'a str, &'a str, &'a str, &'a [u8]),
}

fn body(parts: &[Part<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    for part in parts {
        match part {
            Part::Text(name, text) => {
                out.extend_from_slice(
                    format!(
                        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n"
                    )
                    .as_bytes(),
                );
                out.extend_from_slice(text.as_bytes());
            }
            Part::File(name, filename, ctype, data) => {
                out.extend_from_slice(
                    format!(
                        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {ctype}\r\n\r\n"
                    )
                    .as_bytes(),
                );
                out.extend_from_slice(data);
            }
        }
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    out
}

async fn upload(
    app: &axum::Router,
    ws_id: Uuid,
    token: &str,
    parts: &[Part<'_>],
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/files/clip/{ws_id}?draft=true"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body(parts)))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn poster(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(width, height, image::Rgb([30, 41, 59]));
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("encode");
    out.into_inner()
}

fn waveform(peaks: usize, top: u8) -> Vec<u8> {
    (0..peaks)
        .map(|i| (i % usize::from(top + 1)) as u8)
        .collect()
}

async fn files_of(pool: &PgPool, user_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("count")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_screen_clip_keeps_its_poster_and_length_into_the_message(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "clip-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Clip WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "main", false).await;
    let picture = poster(1280, 720);

    let (status, clip) = upload(
        &app,
        ws_id,
        &owner,
        &[
            Part::Text("clip", r#"{"kind":"video","duration_ms":20000}"#),
            Part::File("poster", "poster.png", "image/png", &picture),
            Part::File("media", "Clip.webm", "video/webm;codecs=vp8,opus", WEBM),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{clip:?}");
    assert_eq!(
        clip["clip"],
        json!({ "kind": "video", "duration_ms": 20000 })
    );
    assert_eq!(clip["mime_type"], "video/webm", "stored without its codecs");
    assert_eq!(
        (clip["width"].as_i64(), clip["height"].as_i64()),
        (Some(1280), Some(720))
    );
    let poster_url = clip["thumbnail_url"].as_str().expect("a poster");
    assert!(poster_url.contains("poster"), "{poster_url}");

    let (status, message) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "the bug", "attachment_ids": [clip["id"]] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{message:?}");
    let attachment = &message["metadata"]["attachments"][0];
    assert_eq!(attachment["clip"]["duration_ms"], 20000);
    assert_eq!(attachment["thumbnail_url"], poster_url);

    let media = clip["url"].as_str().expect("url");
    let path = &media[media.find("/api/").expect("api path")..];
    let request = Request::builder()
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {owner}"))
        .header(header::RANGE, "bytes=4-7")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::PARTIAL_CONTENT,
        "seeking asks for a range"
    );
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(bytes.as_ref(), &WEBM[4..8]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_audio_clip_carries_its_waveform_and_no_poster(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "audio-clip-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Audio Clip WS").await;
    let peaks = waveform(100, 100);
    let meta = json!({ "kind": "audio", "duration_ms": 4200, "waveform": peaks }).to_string();

    let (status, clip) = upload(
        &app,
        ws_id,
        &owner,
        &[
            Part::Text("clip", &meta),
            Part::File("media", "Clip.m4a", "audio/mp4", MP4),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{clip:?}");
    assert_eq!(clip["clip"]["waveform"], json!(peaks));
    assert_eq!(clip["thumbnail_url"], Value::Null);
    assert_eq!(clip["mime_type"], "audio/mp4");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_clip_whose_metadata_is_wrong_is_refused_and_nothing_is_kept(pool: PgPool) {
    let (app, state) = app_and_state(pool.clone()).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "bad-clip-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Bad Clip WS").await;
    let picture = poster(64, 36);
    let short =
        json!({ "kind": "audio", "duration_ms": 1000, "waveform": waveform(99, 100) }).to_string();
    let loud =
        json!({ "kind": "audio", "duration_ms": 1000, "waveform": vec![101; 100] }).to_string();
    let drawn =
        json!({ "kind": "video", "duration_ms": 1000, "waveform": waveform(100, 100) }).to_string();
    let cases = [
        ("empty", r#"{"kind":"video","duration_ms":0}"#.to_string()),
        (
            "over five minutes",
            r#"{"kind":"video","duration_ms":300001}"#.to_string(),
        ),
        (
            "audio without a waveform",
            r#"{"kind":"audio","duration_ms":1000}"#.to_string(),
        ),
        ("99 peaks", short),
        ("a peak over 100", loud),
        ("a video with a waveform", drawn),
        (
            "a field nobody knows",
            r#"{"kind":"video","duration_ms":1000,"transcript":"hi"}"#.to_string(),
        ),
        (
            "a screen kind",
            r#"{"kind":"screen","duration_ms":1000}"#.to_string(),
        ),
        ("not JSON", "twenty seconds".to_string()),
    ];
    for (what, meta) in cases {
        let (status, body) = upload(
            &app,
            ws_id,
            &owner,
            &[
                Part::Text("clip", &meta),
                Part::File("poster", "poster.png", "image/png", &picture),
                Part::File("media", "Clip.webm", "video/webm", WEBM),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{what}: {body:?}");
    }
    assert_eq!(files_of(&pool, owner_id).await, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_poster_that_is_not_an_image_is_refused(pool: PgPool) {
    let (app, state) = app_and_state(pool.clone()).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "poster-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Poster WS").await;
    let meta = r#"{"kind":"video","duration_ms":1000}"#;
    let posters: [(&str, &[u8]); 3] = [
        ("a PDF", b"%PDF-1.7 not a picture"),
        (
            "a PNG header and nothing else",
            b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR",
        ),
        ("nothing", b""),
    ];
    for (what, bytes) in posters {
        let (status, body) = upload(
            &app,
            ws_id,
            &owner,
            &[
                Part::Text("clip", meta),
                Part::File("poster", "poster.png", "image/png", bytes),
                Part::File("media", "Clip.webm", "video/webm", WEBM),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{what}: {body:?}");
    }
    let (status, body) = upload(
        &app,
        ws_id,
        &owner,
        &[
            Part::Text("clip", meta),
            Part::File("media", "Clip.webm", "video/webm", WEBM),
        ],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a video clip without a poster: {body:?}"
    );
    assert_eq!(files_of(&pool, owner_id).await, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_recording_is_the_container_and_kind_it_says(pool: PgPool) {
    let (app, state) = app_and_state(pool.clone()).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "media-owner", false).await;
    let (_, _, outsider) = seed_and_login(&app, &state, "media-outsider", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Media WS").await;
    let picture = poster(64, 36);
    let video = r#"{"kind":"video","duration_ms":1000}"#;
    let audio =
        json!({ "kind": "audio", "duration_ms": 1000, "waveform": waveform(100, 100) }).to_string();

    let recordings: [(&str, &str, &str, &[u8]); 4] = [
        ("WebM bytes named MP4", video, "video/mp4", WEBM),
        ("a QuickTime movie", video, "video/quicktime", MP4),
        ("audio in a video clip", video, "audio/webm", WEBM),
        ("a PDF", video, "video/webm", b"%PDF-1.7 not a recording"),
    ];
    for (what, meta, ctype, bytes) in recordings {
        let (status, body) = upload(
            &app,
            ws_id,
            &owner,
            &[
                Part::Text("clip", meta),
                Part::File("poster", "poster.png", "image/png", &picture),
                Part::File("media", "Clip", ctype, bytes),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{what}: {body:?}");
    }

    let (status, body) = upload(
        &app,
        ws_id,
        &owner,
        &[
            Part::Text("clip", &audio),
            Part::File("poster", "poster.png", "image/png", &picture),
            Part::File("media", "Clip.m4a", "audio/mp4", MP4),
        ],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "an audio clip has no poster: {body:?}"
    );

    let (status, body) = upload(
        &app,
        ws_id,
        &owner,
        &[
            Part::Text("clip", &audio),
            Part::File("media", "Clip.m4a", "audio/mp4", MP4),
            Part::Text("extra", "more"),
        ],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "nothing after the recording: {body:?}"
    );
    assert_eq!(files_of(&pool, owner_id).await, 0);

    let (status, _) = upload(
        &app,
        ws_id,
        &outsider,
        &[
            Part::Text("clip", &audio),
            Part::File("media", "Clip.m4a", "audio/mp4", MP4),
        ],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "only a member records into a workspace"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn erasing_the_person_removes_their_clips_and_posters(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "clip-erase-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Clip Erase WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "main", false).await;
    let (subject_id, subject_email) = seed(&state, "clip-erase-subject", false).await;
    add_ws_member(&state, ws_id, subject_id, "member").await;
    let subject = login(&app, &subject_email, PASSWORD).await;
    let picture = poster(640, 360);

    let (status, clip) = upload(
        &app,
        ws_id,
        &subject,
        &[
            Part::Text("clip", r#"{"kind":"video","duration_ms":20000}"#),
            Part::File("poster", "poster.png", "image/png", &picture),
            Part::File("media", "Clip.mp4", "video/mp4", MP4),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{clip:?}");
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&subject),
        Some(json!({ "content": "a clip", "attachment_ids": [clip["id"]] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (storage_key, thumbnail_key): (String, Option<String>) =
        sqlx::query_as("SELECT storage_key, thumbnail_key FROM files WHERE user_id = $1")
            .bind(subject_id)
            .fetch_one(&state.pool)
            .await
            .expect("the clip");
    let thumbnail_key = thumbnail_key.expect("the poster is the clip's thumbnail");

    let (_, _, instance_admin) = seed_and_login(&app, &state, "clip-erase-admin", true).await;
    let (status, erased) = send(
        &app,
        "DELETE",
        &format!("/api/admin/users/{subject_id}/data"),
        Some(&instance_admin),
        Some(json!({ "hard_delete": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{erased:?}");
    assert_eq!(erased["removed"]["files"], 1);
    assert_eq!(files_of(&state.pool, subject_id).await, 0);
    for key in [storage_key, thumbnail_key] {
        assert!(
            state.file_storage.download(&key).await.is_err(),
            "{key} goes with the row"
        );
    }
}
