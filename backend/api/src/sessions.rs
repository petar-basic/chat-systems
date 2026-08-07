use chrono::Utc;
use uuid::Uuid;

use shared_common::errors::AppResult;

use crate::middleware::{RevocationRecord, RevocationStore};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionScope {
    All,
    AllExcept(Uuid),
}

impl SessionScope {
    fn survivor(self) -> Option<Uuid> {
        match self {
            SessionScope::All => None,
            SessionScope::AllExcept(jti) => Some(jti),
        }
    }
}

/// Ends a user's sessions: refresh tokens are deleted, outstanding access
/// tokens are marked invalid, and live WebSocket connections are told to close.
/// Every caller that needs "this access must stop now" goes through here so the
/// three steps cannot drift apart.
pub async fn revoke(
    state: &AppState,
    user_id: Uuid,
    scope: SessionScope,
    reason: &str,
) -> AppResult<()> {
    let survivor = scope.survivor();

    let purged = match survivor {
        None => state
            .auth_service
            .repo()
            .delete_user_refresh_tokens(user_id)
            .await
            .err(),
        Some(jti) => state
            .auth_service
            .repo()
            .delete_user_refresh_tokens_except(user_id, &jti.to_string())
            .await
            .err(),
    };
    if let Some(e) = purged {
        tracing::warn!(
            "failed to delete refresh tokens for user {}: {}",
            user_id,
            e
        );
    }

    let record = RevocationRecord {
        at: Utc::now().timestamp(),
        except_jti: survivor,
    };
    // The flag has to outlive every access token issued before `at`. Deriving it
    // from the *current* access expiry breaks if an operator lowers that value:
    // tokens minted under the old, longer expiry would outlive their own
    // revocation. Refresh expiry is the safe ceiling and costs one small key.
    let ttl = state
        .config
        .access_token_expiry
        .max(state.config.refresh_token_expiry)
        .max(0) as u64;
    RevocationStore(state.redis.clone())
        .revoke(user_id, &record, ttl)
        .await;

    if let Err(e) = state
        .publisher
        .publish(
            "session.revoked",
            serde_json::json!({
                "user_id": user_id,
                "except_jti": survivor,
                "reason": reason,
            }),
        )
        .await
    {
        tracing::warn!("failed to publish session.revoked for {}: {}", user_id, e);
    }

    Ok(())
}

pub async fn restore(state: &AppState, user_id: Uuid) {
    RevocationStore(state.redis.clone()).restore(user_id).await;
}
