use rand::distributions::Alphanumeric;
use rand::Rng;
use tracing::info;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::repo::WorkspaceRepo;
use crate::auth::service::AuthService;
use crate::config::AppConfig;

pub struct WorkspaceService {
    pub repo: WorkspaceRepo,
    config: AppConfig,
}

impl WorkspaceService {
    pub fn new(repo: WorkspaceRepo, config: AppConfig) -> Self {
        Self { repo, config }
    }

    pub async fn is_workspace_member(&self, workspace_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        self.repo
            .get_member(workspace_id, user_id)
            .await
            .map(|m| m.is_some())
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub async fn create_workspace(
        &self,
        name: &str,
        description: Option<&str>,
        owner_id: Uuid,
    ) -> AppResult<Workspace> {
        let slug = slug_from_name(name);

        let mut tx = self.repo.begin().await?;

        let workspace =
            WorkspaceRepo::create_workspace_tx(&mut tx, name, &slug, description, owner_id).await?;

        WorkspaceRepo::add_member_tx(&mut tx, workspace.id, owner_id, &WorkspaceRole::Owner)
            .await?;

        let channel = WorkspaceRepo::create_channel_tx(
            &mut tx,
            workspace.id,
            "general",
            &ChannelType::Public,
            Some("General discussion"),
            owner_id,
            true,
        )
        .await?;

        WorkspaceRepo::add_channel_member_tx(&mut tx, channel.id, owner_id, &ChannelRole::Admin)
            .await?;

        tx.commit().await?;

        info!("Workspace created: {} ({})", workspace.name, workspace.id);
        Ok(workspace)
    }

    pub async fn create_invite(
        &self,
        workspace_id: Uuid,
        created_by: Uuid,
        email: Option<&str>,
        role: Option<WorkspaceRole>,
        auth_service: &AuthService,
    ) -> AppResult<WorkspaceInvite> {
        let role = role.unwrap_or(WorkspaceRole::Member);

        let token: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let provisioned = if let Some(email_addr) = email {
            Some(auth_service.provision_user(email_addr).await?)
        } else {
            None
        };

        let invite = self
            .repo
            .create_invite(workspace_id, created_by, email, &role, &token)
            .await?;

        if let (Some(email_addr), Some(user)) = (email, provisioned.as_ref()) {
            let workspace = self.repo.find_workspace_by_id(workspace_id).await?;
            if let Some(ws) = workspace {
                let role_str = serde_json::to_value(&role)
                    .ok()
                    .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                    .unwrap_or_else(|| "member".to_string());

                let reg_token = auth_service.generate_registration_token(
                    user.id,
                    email_addr,
                    workspace_id,
                    &role_str,
                )?;
                let invite_url = format!("{}/invite/{}", self.config.public_url, reg_token);
                if let Err(e) = auth_service
                    .send_invite_email(email_addr, &ws.name, &invite_url)
                    .await
                {
                    tracing::warn!("Failed to send invite email: {}", e);
                }
            }
        }

        Ok(invite)
    }

    pub async fn accept_invite(&self, token: &str, user_id: Uuid) -> AppResult<WorkspaceMember> {
        let invite = self
            .repo
            .find_invite_by_token(token)
            .await?
            .ok_or_else(|| AppError::NotFound("Invite not found or expired".into()))?;

        if let Some(expires) = invite.expires_at {
            if expires < chrono::Utc::now() {
                return Err(AppError::BadRequest("Invite has expired".into()));
            }
        }

        let mut tx = self.repo.begin().await?;

        if WorkspaceRepo::claim_invite_use_tx(&mut tx, invite.id)
            .await?
            .is_none()
        {
            return Err(AppError::BadRequest("Invite has reached max uses".into()));
        }

        let member =
            WorkspaceRepo::add_member_tx(&mut tx, invite.workspace_id, user_id, &invite.role)
                .await?;

        let channels =
            WorkspaceRepo::list_default_channels_tx(&mut tx, invite.workspace_id).await?;

        for ch in channels {
            WorkspaceRepo::add_channel_member_tx(&mut tx, ch.id, user_id, &ChannelRole::Member)
                .await?;
        }

        tx.commit().await?;

        info!(
            "User {} accepted invite to workspace {}",
            user_id, invite.workspace_id
        );
        Ok(member)
    }
}

fn slug_from_name(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    let slug = slug.trim_matches('-').to_string();
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();
    format!("{}-{}", slug, suffix.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, StorageBackend};
    use shared_common::errors::AppError;
    use sqlx::PgPool;
    use uuid::Uuid;

    fn test_config() -> AppConfig {
        AppConfig {
            port: 3000,
            database_url: String::new(),
            redis_url: String::new(),
            jwt_secret: "test-jwt-secret-0123456789-abcdefghij".to_string(),
            access_token_expiry: 3600,
            refresh_token_expiry: 604_800,
            admin_email: None,
            admin_password: None,
            smtp_host: "localhost".to_string(),
            smtp_port: 1025,
            smtp_user: String::new(),
            smtp_password: String::new(),
            smtp_from_address: "noreply@test.local".to_string(),
            smtp_from_name: "Test".to_string(),
            smtp_use_tls: false,
            public_url: "http://localhost:3000".to_string(),
            instance_name: "Test".to_string(),
            instance_icon_url: None,
            cors_origins: String::new(),
            storage_backend: StorageBackend::Local,
            local_storage_path: "./data/files".to_string(),
            s3_endpoint: String::new(),
            s3_region: String::new(),
            s3_bucket: String::new(),
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
            turn_secret: String::new(),
            turn_urls: String::new(),
            stun_urls: String::new(),
            turn_ttl_secs: 43200,
            pg_pool_max: 5,
        }
    }

    async fn insert_user(pool: &PgPool, email: &str) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO users (email, status) VALUES ($1, 'active') RETURNING id",
        )
        .bind(email)
        .fetch_one(pool)
        .await
        .expect("insert user")
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn create_workspace_seeds_owner_member_and_default_channel(pool: PgPool) {
        let owner_id = insert_user(&pool, "owner@test.local").await;
        let service = WorkspaceService::new(WorkspaceRepo::new(pool), test_config());

        let workspace = service
            .create_workspace("Acme Corp", Some("the place"), owner_id)
            .await
            .expect("create_workspace should succeed");

        let fetched = service
            .repo
            .find_workspace_by_id(workspace.id)
            .await
            .expect("query workspace")
            .expect("workspace row must exist");
        assert_eq!(fetched.owner_id, owner_id, "owner_id must be the caller");
        assert_eq!(fetched.name, "Acme Corp");

        let member = service
            .repo
            .get_member(workspace.id, owner_id)
            .await
            .expect("query member")
            .expect("owner must be a workspace member");
        assert_eq!(
            member.role,
            WorkspaceRole::Owner,
            "owner's membership role must be Owner"
        );

        let mut tx = service.repo.begin().await.expect("begin");
        let defaults = WorkspaceRepo::list_default_channels_tx(&mut tx, workspace.id)
            .await
            .expect("list default channels");
        tx.commit().await.expect("commit");
        assert_eq!(defaults.len(), 1, "exactly one default channel expected");
        let general = &defaults[0];
        assert_eq!(
            general.name.as_deref(),
            Some("general"),
            "the default channel must be named #general"
        );
        assert!(
            general.is_default,
            "the #general channel must be is_default"
        );
        assert_eq!(general.channel_type, ChannelType::Public);

        let channel_member = service
            .repo
            .get_channel_member(general.id, owner_id)
            .await
            .expect("query channel member")
            .expect("owner must be a member of #general");
        assert_eq!(
            channel_member.role,
            ChannelRole::Admin,
            "owner must be a channel Admin of #general"
        );
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn accept_invite_enforces_single_use_and_increments_count(pool: PgPool) {
        let owner_id = insert_user(&pool, "owner2@test.local").await;
        let first_user = insert_user(&pool, "first@test.local").await;
        let second_user = insert_user(&pool, "second@test.local").await;

        let service = WorkspaceService::new(WorkspaceRepo::new(pool.clone()), test_config());

        let workspace = service
            .create_workspace("Invite WS", None, owner_id)
            .await
            .expect("create_workspace");

        let token = "single-use-token-abc123";
        let invite_id = sqlx::query_scalar::<_, Uuid>(
            r"
            INSERT INTO workspace_invites (workspace_id, created_by, role, token, max_uses)
            VALUES ($1, $2, 'member', $3, 1)
            RETURNING id
            ",
        )
        .bind(workspace.id)
        .bind(owner_id)
        .bind(token)
        .fetch_one(&pool)
        .await
        .expect("insert single-use invite");

        let member = service
            .accept_invite(token, first_user)
            .await
            .expect("first accept_invite should succeed");
        assert_eq!(member.user_id, first_user);
        assert_eq!(member.workspace_id, workspace.id);
        assert_eq!(member.role, WorkspaceRole::Member);

        assert!(
            service
                .repo
                .get_member(workspace.id, first_user)
                .await
                .expect("query member")
                .is_some(),
            "first user must be persisted as a workspace member"
        );

        let second = service.accept_invite(token, second_user).await;
        match second {
            Err(AppError::BadRequest(msg)) => {
                assert!(
                    msg.to_lowercase().contains("max use")
                        || msg.to_lowercase().contains("exhaust")
                        || msg.to_lowercase().contains("reached"),
                    "expected a max-uses/exhausted BadRequest, got: {msg}"
                );
            }
            other => panic!("expected Err(BadRequest), got {other:?}"),
        }

        assert!(
            service
                .repo
                .get_member(workspace.id, second_user)
                .await
                .expect("query member")
                .is_none(),
            "rejected acceptance must not add the second user"
        );

        let invite = service
            .repo
            .find_invite_by_id(invite_id)
            .await
            .expect("find invite")
            .expect("invite row must exist");
        assert_eq!(
            invite.use_count, 1,
            "use_count must record exactly one consumption"
        );
        assert_eq!(invite.max_uses, Some(1));
    }

    #[test]
    fn workspace_role_has_at_least_respects_ordering() {
        use WorkspaceRole::*;

        for r in [Guest, Member, Admin, Owner] {
            assert!(r.has_at_least(&r), "{r:?} must satisfy at-least itself");
        }

        assert!(Owner.has_at_least(&Admin));
        assert!(Owner.has_at_least(&Member));
        assert!(Owner.has_at_least(&Guest));
        assert!(Admin.has_at_least(&Member));
        assert!(Admin.has_at_least(&Guest));
        assert!(Member.has_at_least(&Guest));

        assert!(!Member.has_at_least(&Admin), "Member must not be >= Admin");
        assert!(!Member.has_at_least(&Owner), "Member must not be >= Owner");
        assert!(!Guest.has_at_least(&Member), "Guest must not be >= Member");
        assert!(!Guest.has_at_least(&Admin));
        assert!(!Guest.has_at_least(&Owner));
        assert!(!Admin.has_at_least(&Owner), "Admin must not be >= Owner");

        assert!(Owner.level() > Admin.level());
        assert!(Admin.level() > Member.level());
        assert!(Member.level() > Guest.level());
    }
}
