pub mod consumer;
pub mod email;
pub mod models;
pub mod repo;
pub mod routes;
pub mod text;

use uuid::Uuid;

use models::NotificationData;

pub fn push_event(
    user_id: Uuid,
    workspace_id: Uuid,
    kind: &str,
    body: Option<&str>,
    data: &NotificationData,
) -> serde_json::Value {
    serde_json::json!({
        "user_id": user_id,
        "workspace_id": workspace_id,
        "kind": kind,
        "body": body,
        "data": data,
    })
}
