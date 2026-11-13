use rand::distr::Alphanumeric;
use rand::RngExt;
use tracing::info;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::repo::WorkspaceRepo;
use crate::auth::service::{AuthService, InviteEmail};
use crate::config::AppConfig;
use crate::guests::service::InviteTerms;
use crate::messaging::publisher::{EventPublisher, Staged};
use crate::profile::models::Language;

pub struct WorkspaceService {
    pub repo: WorkspaceRepo,
    config: AppConfig,
}

impl WorkspaceService {
    pub fn new(repo: WorkspaceRepo, config: AppConfig) -> Self {
        Self { repo, config }
    }

    pub async fn is_workspace_member(&self, workspace_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        Ok(self.repo.get_member(workspace_id, user_id).await?.is_some())
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
        inviter: Inviter,
        email: Option<&str>,
        lifetime: InviteLifetime,
        terms: InviteTerms,
        auth_service: &AuthService,
    ) -> AppResult<CreatedInvite> {
        let role = terms.role;

        let token: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        if let Some(email_addr) = email {
            auth_service.provision_user(email_addr).await?;
        }

        let expires_at = chrono::Utc::now() + chrono::Duration::hours(lifetime.expires_in_hours);
        let mut tx = self.repo.begin().await?;
        let mut invite = WorkspaceRepo::create_invite_in(
            &mut tx,
            crate::workspace::repo::NewInvite {
                workspace_id,
                created_by: inviter.user_id,
                email,
                role: &role,
                token: &token,
                max_uses: lifetime.max_uses,
                expires_at,
                guest: terms.guest,
            },
        )
        .await?;
        WorkspaceRepo::add_invite_channels_in(&mut tx, invite.id, &terms.channel_ids).await?;
        tx.commit().await?;
        invite.channel_ids = terms.channel_ids;

        if invite.email.is_none() {
            return Ok(CreatedInvite {
                invite,
                invite_url: None,
                email_status: None,
            });
        }

        let invite_url = self.invite_url(&invite, auth_service).await?;
        let email_status = self
            .email_invite(&invite, &invite_url, auth_service, inviter.language)
            .await;
        Ok(CreatedInvite {
            invite,
            invite_url: Some(invite_url),
            email_status: Some(email_status),
        })
    }

    pub async fn invite_url(
        &self,
        invite: &WorkspaceInvite,
        auth_service: &AuthService,
    ) -> AppResult<String> {
        let email = invite.email.as_deref().ok_or_else(|| {
            AppError::BadRequest("Only an email invite has a link of its own".into())
        })?;
        if !invite.expires_at.is_some_and(|e| e >= chrono::Utc::now()) {
            return Err(AppError::BadRequest("This invite has expired".into()));
        }
        if invite.max_uses.is_some_and(|max| invite.use_count >= max) {
            return Err(AppError::BadRequest("This invite was already used".into()));
        }
        let user = auth_service
            .repo()
            .find_by_email(email)
            .await?
            .ok_or_else(|| AppError::NotFound("Nobody holds this invite any more".into()))?;
        let token = auth_service.issue_invite_link(user.id, invite.id).await?;
        Ok(format!("{}/invite/{}", self.config.public_url, token))
    }

    async fn email_invite(
        &self,
        invite: &WorkspaceInvite,
        invite_url: &str,
        auth_service: &AuthService,
        language: Language,
    ) -> InviteEmailStatus {
        let (Some(email), true) = (invite.email.as_deref(), auth_service.can_send_email()) else {
            return InviteEmailStatus::NotConfigured;
        };
        let (Ok(Some(workspace)), Some(expires_at)) = (
            self.repo.find_workspace_by_id(invite.workspace_id).await,
            invite.expires_at,
        ) else {
            return InviteEmailStatus::Failed;
        };
        let inviter = self
            .repo
            .inviter_name(invite.workspace_id, invite.created_by)
            .await
            .ok()
            .flatten();
        match auth_service
            .send_invite_email(
                email,
                InviteEmail {
                    workspace_name: &workspace.name,
                    inviter: inviter.as_deref(),
                    url: invite_url,
                    expires_at,
                },
                language,
            )
            .await
        {
            Ok(()) => InviteEmailStatus::Sent,
            Err(e) => {
                tracing::warn!("Failed to send invite email: {}", e);
                InviteEmailStatus::Failed
            }
        }
    }

    pub async fn resend_invite(
        &self,
        email: &str,
        language: Language,
        auth_service: &AuthService,
    ) -> AppResult<()> {
        let Some(invite) = self.repo.find_live_email_invite(email).await? else {
            let workspace = self.repo.workspace_that_invited(email).await?;
            return auth_service
                .send_invite_expired(email, workspace.as_deref(), language)
                .await;
        };
        let invite_url = self.invite_url(&invite, auth_service).await?;
        self.email_invite(&invite, &invite_url, auth_service, language)
            .await;
        Ok(())
    }

    pub async fn join_workspace(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &WorkspaceRole,
    ) -> AppResult<Vec<Staged>> {
        let mut tx = self.repo.begin().await?;

        WorkspaceRepo::add_member_if_absent_tx(&mut tx, workspace_id, user_id, role, None).await?;
        let staged = settle_in(&mut tx, workspace_id, user_id, None).await?;

        tx.commit().await?;

        info!("User {} joined workspace {}", user_id, workspace_id);
        Ok(staged)
    }

    pub async fn open_email_invite(
        &self,
        invite_id: Uuid,
        email: &str,
    ) -> AppResult<WorkspaceInvite> {
        let invite = self
            .repo
            .find_invite_by_id(invite_id)
            .await?
            .ok_or_else(invalid_invite)?;
        ensure_live(&invite, email)?;
        Ok(invite)
    }

    pub async fn redeem_email_invite(
        &self,
        invite_id: Uuid,
        user_id: Uuid,
        email: &str,
    ) -> AppResult<(WorkspaceInvite, Vec<Staged>)> {
        let invite = self
            .repo
            .find_invite_by_id(invite_id)
            .await?
            .ok_or_else(invalid_invite)?;
        if self
            .repo
            .get_member(invite.workspace_id, user_id)
            .await?
            .is_some()
        {
            return Ok((invite, Vec::new()));
        }
        ensure_live(&invite, email)?;

        let mut tx = self.repo.begin().await?;
        WorkspaceRepo::claim_invite_use_tx(&mut tx, invite.id)
            .await?
            .ok_or_else(invalid_invite)?;
        WorkspaceRepo::add_member_if_absent_tx(
            &mut tx,
            invite.workspace_id,
            user_id,
            &invite.role,
            invite.guest_terms(),
        )
        .await?;
        let staged = settle_in(&mut tx, invite.workspace_id, user_id, Some(&invite)).await?;
        tx.commit().await?;

        info!(
            "User {} redeemed email invite to workspace {}",
            user_id, invite.workspace_id
        );
        Ok((invite, staged))
    }

    pub async fn accept_invite(
        &self,
        token: &str,
        user_id: Uuid,
        user_email: &str,
    ) -> AppResult<(WorkspaceMember, Vec<Staged>)> {
        let invite = self
            .repo
            .find_invite_by_token(token)
            .await?
            .ok_or_else(|| AppError::NotFound("Invite not found or expired".into()))?;
        if let Some(member) = self.repo.get_member(invite.workspace_id, user_id).await? {
            return Ok((member, Vec::new()));
        }

        // A row that escaped the expiry default is treated as expired, not as
        // eternal: fail closed on the one field that bounds this invite.
        match invite.expires_at {
            Some(expires) if expires >= chrono::Utc::now() && !guest_access_over(&invite) => {}
            _ => return Err(AppError::BadRequest("Invite has expired".into())),
        }

        if let Some(invited) = invite.email.as_deref() {
            if !invited.eq_ignore_ascii_case(user_email) {
                return Err(AppError::Forbidden(
                    "This invite was issued to a different address".into(),
                ));
            }
        }

        let mut tx = self.repo.begin().await?;

        if WorkspaceRepo::claim_invite_use_tx(&mut tx, invite.id)
            .await?
            .is_none()
        {
            return Err(AppError::BadRequest("Invite has reached max uses".into()));
        }

        let Some(member) = WorkspaceRepo::add_member_if_absent_tx(
            &mut tx,
            invite.workspace_id,
            user_id,
            &invite.role,
            invite.guest_terms(),
        )
        .await?
        else {
            tx.rollback().await?;
            let member = self
                .repo
                .get_member(invite.workspace_id, user_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Invite not found or expired".into()))?;
            return Ok((member, Vec::new()));
        };

        let staged = settle_in(&mut tx, invite.workspace_id, user_id, Some(&invite)).await?;

        tx.commit().await?;

        info!(
            "User {} accepted invite to workspace {}",
            user_id, invite.workspace_id
        );
        Ok((member, staged))
    }
}

fn invalid_invite() -> AppError {
    AppError::Unauthorized("Invalid or expired invite".into())
}

fn ensure_live(invite: &WorkspaceInvite, email: &str) -> AppResult<()> {
    let live = invite.expires_at.is_some_and(|e| e >= chrono::Utc::now())
        && !guest_access_over(invite)
        && invite.max_uses.is_none_or(|max| invite.use_count < max)
        && invite
            .email
            .as_deref()
            .is_some_and(|invited| invited.eq_ignore_ascii_case(email));
    if live {
        Ok(())
    } else {
        Err(invalid_invite())
    }
}

fn guest_access_over(invite: &WorkspaceInvite) -> bool {
    invite
        .guest_until
        .is_some_and(|until| until <= chrono::Utc::now())
}

async fn settle_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    user_id: Uuid,
    invite: Option<&WorkspaceInvite>,
) -> AppResult<Vec<Staged>> {
    let mut staged =
        vec![EventPublisher::stage_workspace_member_joined(tx, workspace_id, user_id).await?];
    let channels = match invite.filter(|i| i.guest_kind.is_some()) {
        Some(invite) => {
            WorkspaceRepo::open_channels_among_tx(tx, workspace_id, &invite.channel_ids).await?
        }
        None => WorkspaceRepo::list_default_channels_tx(tx, workspace_id)
            .await?
            .into_iter()
            .map(|ch| ch.id)
            .collect(),
    };
    staged.extend(join_channels(tx, workspace_id, user_id, &channels).await?);
    WorkspaceRepo::restore_profile_in(tx, workspace_id, user_id).await?;
    for channel_id in WorkspaceRepo::restore_conversations_in(tx, workspace_id, user_id).await? {
        staged.push(
            EventPublisher::stage_channel_member_joined(tx, channel_id, workspace_id, user_id)
                .await?,
        );
    }
    Ok(staged)
}

async fn join_channels(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    user_id: Uuid,
    channel_ids: &[Uuid],
) -> AppResult<Vec<Staged>> {
    let mut staged = Vec::new();
    for channel_id in channel_ids {
        if WorkspaceRepo::add_channel_member_tx(tx, *channel_id, user_id, &ChannelRole::Member)
            .await?
            .is_some()
        {
            staged.push(
                EventPublisher::stage_channel_member_joined(tx, *channel_id, workspace_id, user_id)
                    .await?,
            );
        }
    }
    Ok(staged)
}

fn slug_from_name(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    let slug = slug.trim_matches('-').to_string();
    let suffix: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();
    format!("{}-{}", slug, suffix.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use shared_common::errors::AppError;
    use sqlx::PgPool;
    use uuid::Uuid;

    fn test_config() -> AppConfig {
        AppConfig::for_tests()
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

    #[test_macros::db_test(migrations = "../migrations")]
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

    #[test_macros::db_test(migrations = "../migrations")]
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
            INSERT INTO workspace_invites (workspace_id, created_by, role, token, max_uses, expires_at)
            VALUES ($1, $2, 'member', $3, 1, NOW() + INTERVAL '1 day')
            RETURNING id
            ",
        )
        .bind(workspace.id)
        .bind(owner_id)
        .bind(token)
        .fetch_one(&pool)
        .await
        .expect("insert single-use invite");

        let (member, _) = service
            .accept_invite(token, first_user, "first@test.local")
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

        let second = service
            .accept_invite(token, second_user, "second@test.local")
            .await;
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
