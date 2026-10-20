use redis::streams::StreamRangeReply;
use std::sync::Arc;

use tracing::warn;
use uuid::Uuid;

use crate::connection_manager::{Audience, ConnectionManager};
use crate::event_consumer::handle_event_for;

/// Past this many missed events a full refetch is cheaper than replaying them
/// one by one, and the client already has that path — it is what it does on a
/// cold load.
const REPLAY_MAX_EVENTS: usize = 1_000;

pub fn workspace_stream(workspace_id: Uuid) -> String {
    format!("stream:ws:{workspace_id}")
}

/// A ring is in the log so the worker can be handed it at least once; a client
/// reconnecting minutes later must not be rung for a call that is over.
pub fn replays_to_clients(event_type: &str) -> bool {
    event_type != "huddle.ring"
}

#[derive(Debug, PartialEq, Eq)]
pub enum Replay {
    /// Everything the client missed was still in the log and has been delivered.
    Caught {
        events: usize,
        last_id: Option<String>,
    },
    /// The client's position is older than the log goes, or the gap is larger
    /// than replaying is worth. It should refetch.
    RefetchRequired,
}

/// Redis stream ids are `<millis>-<seq>`, which sorts correctly only when both
/// halves are compared numerically.
fn parse_id(id: &str) -> Option<(u64, u64)> {
    let (ms, seq) = id.split_once('-')?;
    Some((ms.parse().ok()?, seq.parse().ok()?))
}

fn is_older(a: &str, b: &str) -> bool {
    match (parse_id(a), parse_id(b)) {
        (Some(x), Some(y)) => x < y,
        _ => false,
    }
}

async fn oldest_id(
    conn: &mut redis::aio::ConnectionManager,
    key: &str,
) -> redis::RedisResult<Option<String>> {
    let entries: StreamRangeReply = redis::cmd("XRANGE")
        .arg(key)
        .arg("-")
        .arg("+")
        .arg("COUNT")
        .arg(1)
        .query_async(conn)
        .await?;
    Ok(entries.ids.into_iter().next().map(|entry| entry.id))
}

/// The newest position in a workspace log.
///
/// A client that has not yet seen an event has nothing to resume from, so it
/// gets the current tail at subscribe time. Without it, a client that connects
/// to a quiet workspace and then drops has no position to ask from and silently
/// falls back to the old at-most-once behaviour.
pub async fn current_tail(cm: &Arc<ConnectionManager>, workspace_id: Uuid) -> Option<String> {
    let mut conn = cm.redis();
    let entries: StreamRangeReply = redis::cmd("XREVRANGE")
        .arg(workspace_stream(workspace_id))
        .arg("+")
        .arg("-")
        .arg("COUNT")
        .arg(1)
        .query_async(&mut conn)
        .await
        .ok()?;
    entries.ids.into_iter().next().map(|entry| entry.id)
}

/// Hands a reconnecting client the events it missed, through the same fan-out
/// predicates the live path uses.
pub async fn replay_workspace(
    cm: &Arc<ConnectionManager>,
    conn_id: Uuid,
    workspace_id: Uuid,
    last_event_id: &str,
) -> Replay {
    let mut conn = cm.redis();
    let key = workspace_stream(workspace_id);

    // A position older than anything still in the log means the gap has been
    // trimmed away — there is no way to know what was in it, so say so rather
    // than silently delivering a partial backlog.
    match oldest_id(&mut conn, &key).await {
        Ok(Some(oldest)) if is_older(last_event_id, &oldest) => return Replay::RefetchRequired,
        Ok(_) => {}
        Err(e) => {
            warn!(workspace_id = %workspace_id, "replay: XRANGE probe failed: {}", e);
            return Replay::RefetchRequired;
        }
    }

    let entries: StreamRangeReply = match redis::cmd("XRANGE")
        .arg(&key)
        .arg(format!("({last_event_id}"))
        .arg("+")
        .arg("COUNT")
        .arg(REPLAY_MAX_EVENTS + 1)
        .query_async(&mut conn)
        .await
    {
        Ok(entries) => entries,
        Err(e) => {
            warn!(workspace_id = %workspace_id, "replay: XRANGE failed: {}", e);
            return Replay::RefetchRequired;
        }
    };

    if entries.ids.len() > REPLAY_MAX_EVENTS {
        return Replay::RefetchRequired;
    }

    let mut delivered = 0usize;
    let mut last_id = None;

    for entry in entries.ids {
        let id = entry.id.clone();
        let Some(body) = entry.get::<String>("event") else {
            continue;
        };

        let Ok(event) = serde_json::from_str::<shared_events::Event>(&body) else {
            warn!(workspace_id = %workspace_id, "replay: undecodable entry {}", id);
            continue;
        };

        if !replays_to_clients(&event.event_type) {
            last_id = Some(id);
            continue;
        }

        handle_event_for(
            Audience::Connection(conn_id),
            &event.event_type,
            &event.payload,
            cm,
            Some(&id),
        )
        .await;

        delivered += 1;
        last_id = Some(id);
    }

    Replay::Caught {
        events: delivered,
        last_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_ids_compare_by_both_halves() {
        assert!(is_older("100-1", "100-2"));
        assert!(is_older("99-9", "100-0"));
        assert!(!is_older("100-2", "100-1"));
        // 9 is not older than 100 just because "9" sorts after "1" as text.
        assert!(is_older("9-0", "100-0"));
    }

    #[test]
    fn a_ring_is_never_replayed_to_a_client() {
        assert!(!replays_to_clients("huddle.ring"));
        assert!(replays_to_clients("message.created"));
        assert!(replays_to_clients("huddle.member_joined"));
    }

    #[test]
    fn an_unparseable_id_is_not_treated_as_older() {
        assert!(!is_older("garbage", "100-0"));
        assert!(!is_older("100-0", "garbage"));
    }
}
