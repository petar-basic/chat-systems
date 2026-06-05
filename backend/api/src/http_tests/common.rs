#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use crate::auth::service::AuthService;
use crate::config::{AppConfig, StorageBackend};
use crate::state::AppState;
use crate::workspace::models::{ChannelRole, ChannelType};
use crate::{build_app, build_state};

pub const PASSWORD: &str = "password123";

pub fn test_config() -> AppConfig {
    AppConfig {
        port: 0,
        database_url: "postgres://unused/in/tests".into(),
        redis_url: std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into()),
        jwt_secret: "integration-test-secret-key-0123456789-abcdef".into(),
        access_token_expiry: 3600,
        refresh_token_expiry: 604_800,
        admin_email: None,
        admin_password: None,
        smtp_host: "localhost".into(),
        smtp_port: 1025,
        smtp_user: String::new(),
        smtp_password: String::new(),
        smtp_from_address: "noreply@test.local".into(),
        smtp_from_name: "Test".into(),
        smtp_use_tls: false,
        public_url: "http://localhost".into(),
        instance_name: "Test".into(),
        instance_icon_url: None,
        cors_origins: "http://localhost".into(),
        storage_backend: StorageBackend::Local,
        local_storage_path: std::env::temp_dir()
            .join(format!("chatsys-test-{}", Uuid::new_v4()))
            .to_string_lossy()
            .into_owned(),
        s3_endpoint: "http://localhost:9000".into(),
        s3_region: "us-east-1".into(),
        s3_bucket: "test".into(),
        s3_access_key: "test".into(),
        s3_secret_key: "test".into(),
    }
}

pub async fn app_and_state(pool: sqlx::PgPool) -> (Router, Arc<AppState>) {
    let state = build_state(pool, test_config()).await.expect("build_state");
    let app = build_app(state.clone());
    (app, state)
}

pub fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@test.local", Uuid::new_v4())
}

pub async fn seed_user(state: &AppState, email: &str, is_admin: bool) -> Uuid {
    let hash = AuthService::hash_password(PASSWORD).expect("hash");
    let user = state
        .auth_service
        .repo()
        .create(email, Some(&hash), Some("Test User"), is_admin)
        .await
        .expect("create user");
    state
        .auth_service
        .repo()
        .activate(user.id, &hash, "Test User")
        .await
        .expect("activate user");
    user.id
}

pub async fn seed(state: &AppState, prefix: &str, is_admin: bool) -> (Uuid, String) {
    let email = unique_email(prefix);
    let id = seed_user(state, &email, is_admin).await;
    (id, email)
}

pub async fn seed_and_login(
    app: &Router,
    state: &AppState,
    prefix: &str,
    is_admin: bool,
) -> (Uuid, String, String) {
    let (id, email) = seed(state, prefix, is_admin).await;
    let token = login(app, &email, PASSWORD).await;
    (id, email, token)
}

pub async fn seed_workspace(state: &AppState, owner_id: Uuid, name: &str) -> Uuid {
    state
        .workspace_service
        .create_workspace(name, None, owner_id)
        .await
        .expect("create workspace")
        .id
}

pub async fn add_ws_member(state: &AppState, ws_id: Uuid, user_id: Uuid, role: &str) {
    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) \
         VALUES ($1, $2, $3::workspace_role) ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(ws_id)
    .bind(user_id)
    .bind(role)
    .execute(&state.pool)
    .await
    .expect("add ws member");
}

pub async fn seed_channel(
    state: &AppState,
    ws_id: Uuid,
    creator: Uuid,
    name: &str,
    private: bool,
) -> Uuid {
    let ty = if private {
        ChannelType::Private
    } else {
        ChannelType::Public
    };
    let ch = state
        .workspace_service
        .repo
        .create_channel(ws_id, name, &ty, None, creator, false)
        .await
        .expect("create channel");
    let _ = state
        .workspace_service
        .repo
        .add_channel_member(ch.id, creator, &ChannelRole::Admin)
        .await;
    ch.id
}

pub async fn login(app: &Router, email: &str, password: &str) -> String {
    let (status, body) = send(
        app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login should succeed: {body:?}");
    body["access_token"]
        .as_str()
        .expect("access_token in login response")
        .to_string()
}

pub async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let request = match body {
        Some(b) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}
