---
name: backend-conventions
description: Enforce chat-systems Rust/Axum backend feature architecture and layer boundaries. Use when creating, refactoring, or reviewing backend code under backend/api/src or backend/realtime/src, especially for responsibilities of routes, service, repo, models, and publisher/consumer/executor files.
---

# Backend Conventions

## Overview

Apply chat-systems Rust/Axum backend conventions consistently when implementing or reviewing feature code.
Keep strict separation between HTTP handling, business logic, and data access.

## Load Source of Truth

- Read `docs/backend.md` first for API contracts and route reference.
- Treat `docs/backend.md` as canonical for endpoint signatures.
- Read `backend/api/src/messaging/` as the reference implementation for a complete feature.
- Read `backend/api/src/workspace/` as the reference for a feature with a service layer.

## Choose Task Flow

- Use **Create Flow** for new features.
- Use **Modify Flow** for updates in existing features.
- Use **Review Flow** for pull request or diff reviews.

## Create Flow (New Feature)

1. Create feature directory under `backend/api/src/<feature_name>/`.
2. Add required files: `mod.rs`, `models.rs`, `repo.rs`, `routes.rs`.
3. Add optional files only when required: `service.rs` (multi-step business logic), `publisher.rs` (Redis event publishing), `consumer.rs` (Redis event consumption), `executor.rs` (background task execution), `storage.rs` (external storage abstraction).
4. Implement in this order: models → repo → service (if needed) → routes.
5. Add the repo/service to `AppState` in `backend/api/src/state.rs`.
6. Register the router in `backend/api/src/main.rs` by merging `<feature>::routes::router(state.clone())`.
7. Add migrations to `backend/migrations/` with sqlx migrate.
8. Spawn background tasks in `main.rs` via `tokio::spawn(...)` if needed.

## Modify Flow (Existing Feature)

1. Keep existing API contracts stable unless requirements explicitly change.
2. Preserve layer boundaries:
   - Keep request parsing and response shaping in routes.
   - Keep business rules and permission checks in service or route helpers.
   - Keep SQL queries in repo.
3. Move misplaced logic to the correct layer before adding new logic.
4. Update or add migrations if model fields change.

## Review Flow (PR / Diff)

Use this checklist:

- Confirm routes do not contain raw `sqlx` queries.
- Confirm routes do not implement business rules — they call repo/service methods.
- Confirm repo functions contain only database operations (no permissions, no HTTP types).
- Confirm permission checks run at the start of the handler or in a helper called first.
- Confirm repo returns `sqlx::Result<Option<T>>` for single-row lookups; routes convert `None` to `AppError::NotFound`.
- Confirm errors use `AppError` variants, never raw `anyhow` leaking into response.
- Confirm `AppResult<Json<T>>` is the return type for all route handlers.
- Confirm the new router is merged in `main.rs`.
- Confirm new state fields are added to `AppState` and initialized in `main.rs`.
- Confirm N+1 queries are avoided — batch-fetch related data and group by key.
- Confirm soft deletes set `deleted_at` and queries filter `WHERE deleted_at IS NULL`.

## Layer Responsibilities

### Routes (`routes.rs`)

- Parse path params (`Path<Uuid>`), query params (`Query<T>`), and request bodies (`Json<T>`).
- Extract `AuthUser` from request via the `FromRequestParts` extractor.
- Call repo or service methods.
- Return `AppResult<Json<serde_json::Value>>` or `AppResult<Json<ConcreteType>>`.
- Perform permission checks via helper functions (`require_member`, `require_role`) at the top of the handler.
- Avoid business rules and raw SQL.

### Service (`service.rs`) — add when needed

- Orchestrate multi-step operations that span multiple repo calls.
- Contain complex business rules (invite flow, workspace creation with owner setup, etc.).
- Raise `AppError` variants, never `axum::response::IntoResponse` types.
- Convert between domain types when necessary.
- If a feature has no multi-step logic, put the logic directly in routes with helper functions.

### Repo (`repo.rs`)

- Issue all `sqlx` queries and mutations.
- Return `sqlx::Result<T>` or `sqlx::Result<Option<T>>`.
- Avoid permissions, business workflows, and HTTP concerns.
- Only query tables owned by the current feature; join to other tables via SQL joins (not by calling another feature's repo).

### Models (`models.rs`)

- Define `sqlx::FromRow` database structs.
- Define request DTOs (structs with `Deserialize`).
- Define response DTOs or use `serde_json::Value` for ad-hoc shapes.
- Keep role enums and their helper methods here (e.g., `WorkspaceRole::has_at_least`).

### Publisher (`publisher.rs`) — messaging pattern

- Wrap an `EventPublisher` with typed convenience methods.
- Serialize events to JSON and publish to Redis channels (`events:<domain>`).
- Never call the publisher from repo.

### Consumer / Executor (`consumer.rs`, `executor.rs`) — background tasks

- Run as `tokio::spawn`-ed tasks in `main.rs`.
- Subscribe to Redis channels and react to events.
- Call repo methods to persist side effects.

## Coding Conventions

- Use `Arc<AppState>` as `State(state): State<Arc<AppState>>` in every handler.
- Use `AppResult<T>` as the return type alias for `Result<T, AppError>`.
- Convert sqlx errors with `.map_err(|e| AppError::Database(e.to_string()))?`.
- Convert `Option::None` to errors with `.ok_or_else(|| AppError::NotFound("...".into()))`.
- Use `snake_case` for all files, functions, and variables; `PascalCase` for types.
- Keep `async fn` throughout the call chain; never block async paths with sync I/O.
- Use `Uuid` for all primary keys; `DateTime<Utc>` for timestamps.
- Use `#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]` for database models.

## AppState Pattern

```rust
// state.rs — all shared resources live here
pub struct AppState {
    pub config: AppConfig,
    pub pool: sqlx::PgPool,
    pub auth_service: AuthService,
    pub workspace_service: WorkspaceService,
    pub message_repo: MessageRepo,
    pub publisher: EventPublisher,
    pub file_repo: FileRepo,
    pub file_storage: Box<dyn FileStorage + Send + Sync>,
    pub hook_repo: HookRepo,
    pub notification_repo: NotificationRepo,
}
```

Add new repos/services to `AppState` and initialize them in `main.rs` before building the router.

## Error Handling Pattern

```rust
// shared/common/src/errors.rs — use these variants
AppError::Unauthorized(msg)   // 401 — missing or invalid token
AppError::Forbidden(msg)      // 403 — authenticated but not allowed
AppError::NotFound(msg)       // 404 — resource does not exist
AppError::BadRequest(msg)     // 400 — malformed request
AppError::Conflict(msg)       // 409 — uniqueness violation
AppError::Internal(msg)       // 500 — unexpected server error
AppError::Database(msg)       // 500 — sqlx error forwarded as string
```

Standard conversion chain in route handlers:
```rust
// sqlx error → AppError
let msg = state.message_repo.find_by_id(id).await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
```

## Permission Helper Pattern

```rust
// Declare near the top of routes.rs or in a helpers module
async fn require_member(state: &AppState, workspace_id: Uuid, user_id: Uuid)
    -> AppResult<WorkspaceMember> {
    state
        .workspace_service
        .repo
        .get_member(workspace_id, user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::Forbidden("Not a member of this workspace".into()))
}

async fn require_role(state: &AppState, workspace_id: Uuid, user_id: Uuid,
    minimum: &WorkspaceRole) -> AppResult<WorkspaceMember> {
    let member = require_member(state, workspace_id, user_id).await?;
    if !member.role.has_at_least(minimum) {
        return Err(AppError::Forbidden(format!("Requires at least {:?} role", minimum)));
    }
    Ok(member)
}
```

Call permission helpers at the very top of each handler before any mutation.

## sqlx Patterns

**Simple insert with RETURNING:**
```rust
pub async fn create_message(&self, channel_id: Uuid, user_id: Uuid, content: &str)
    -> sqlx::Result<Message> {
    sqlx::query_as::<_, Message>(
        "INSERT INTO messages (channel_id, user_id, content)
         VALUES ($1, $2, $3)
         RETURNING *",
    )
    .bind(channel_id)
    .bind(user_id)
    .bind(content)
    .fetch_one(&self.pool)
    .await
}
```

**Cursor-based pagination (prefer over offset):**
```rust
if let Some(before_id) = cursor {
    sqlx::query_as::<_, Message>(
        "SELECT * FROM messages
         WHERE channel_id = $1
           AND deleted_at IS NULL
           AND created_at < (SELECT created_at FROM messages WHERE id = $3)
         ORDER BY created_at DESC LIMIT $2",
    )
    .bind(channel_id).bind(limit).bind(before_id)
    .fetch_all(&self.pool).await
} else {
    // ... without cursor
}
```

**Batch fetch to avoid N+1:**
```rust
// Fetch all reactions for a set of messages in one query
pub async fn list_reactions_for_messages(&self, message_ids: &[Uuid])
    -> sqlx::Result<Vec<Reaction>> {
    sqlx::query_as::<_, Reaction>(
        "SELECT * FROM reactions WHERE message_id = ANY($1)",
    )
    .bind(message_ids)
    .fetch_all(&self.pool)
    .await
}
```

Group results by key after fetching:
```rust
let mut reactions_map: HashMap<Uuid, Vec<Reaction>> = HashMap::new();
for r in reactions {
    reactions_map.entry(r.message_id).or_default().push(r);
}
```

**Soft delete:**
```rust
pub async fn soft_delete(&self, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE messages SET deleted_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&self.pool)
        .await?;
    Ok(())
}
```

## Route Registration Pattern

```rust
// routes.rs — every feature exports a router function
pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/resource", get(list).post(create))
        .route("/resource/:id", get(get_one).patch(update).delete(remove))
        .layer(middleware::from_fn(auth_middleware));
    Router::new().merge(routes).with_state(state)
}

// main.rs — merge all feature routers
let api = Router::new()
    .merge(feature_a::routes::router(state.clone()))
    .merge(feature_b::routes::router(state.clone()));
let app = Router::new().nest("/api", api);
```

## Quick Anti-Patterns

- Do not write raw sqlx queries inside route handlers.
- Do not return `sqlx::Result` or `anyhow::Error` directly from handlers — convert to `AppError`.
- Do not skip permission checks before mutations.
- Do not issue a SELECT to look up a value and then INSERT separately when a scalar subquery works atomically.
- Do not call another feature's repo from your repo — use SQL joins instead.
- Do not use `.unwrap()` or `.expect()` in production paths — propagate with `?`.
- Do not block the async executor with synchronous I/O — use async equivalents.
- Do not put background task logic (consumers, executors) inline in route handlers.
- Do not forget to spawn background tasks in `main.rs` and add their deps to `AppState`.
- Do not add a new feature without registering its router in `main.rs`.
