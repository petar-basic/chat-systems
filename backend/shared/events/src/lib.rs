pub mod auth_events;
pub mod huddle_events;
pub mod messaging_events;
pub mod workspace_events;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// The position this event occupies in its workspace's replay log. Absent
    /// for events that are not replayable — typing, presence, signalling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
}

impl Event {
    pub fn new(event_type: &str, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            payload,
            timestamp: chrono::Utc::now(),
            stream_id: None,
        }
    }
}
