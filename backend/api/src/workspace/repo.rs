use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::*;

pub struct NewInvite<'a> {
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub email: Option<&'a str>,
    pub role: &'a WorkspaceRole,
    pub token: &'a str,
    pub max_uses: i32,
    pub expires_at: DateTime<Utc>,
}

pub struct WorkspaceRepo {
    pool: PgPool,
}

impl WorkspaceRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn begin(&self) -> sqlx::Result<sqlx::Transaction<'_, sqlx::Postgres>> {
        self.pool.begin().await
    }

    pub async fn create_workspace_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        name: &str,
        slug: &str,
        description: Option<&str>,
        owner_id: Uuid,
    ) -> sqlx::Result<Workspace> {
        sqlx::query_as!(
            Workspace,
            r#"
            INSERT INTO workspaces (name, slug, description, owner_id)
            VALUES ($1, $2, $3, $4)
            RETURNING id, name, slug, description, icon_url, owner_id, settings AS "settings!",
                   is_active AS "is_active!", deleted_at, created_at, updated_at
            "#,
            name,
            slug,
            description,
            owner_id
        )
        .fetch_one(&mut **tx)
        .await
    }

    pub async fn find_workspace_by_id(&self, id: Uuid) -> sqlx::Result<Option<Workspace>> {
        sqlx::query_as!(
            Workspace,
            r#"SELECT id, name, slug, description, icon_url, owner_id, settings AS "settings!",
                   is_active AS "is_active!", deleted_at, created_at, updated_at
                 FROM workspaces WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_user_workspaces(&self, user_id: Uuid) -> sqlx::Result<Vec<Workspace>> {
        sqlx::query_as!(
            Workspace,
            r#"
            SELECT w.id, w.name, w.slug, w.description, w.icon_url, w.owner_id, w.settings AS "settings!",
                   w.is_active AS "is_active!", w.deleted_at, w.created_at, w.updated_at
              FROM workspaces w
              JOIN workspace_members wm ON wm.workspace_id = w.id
             WHERE wm.user_id = $1 AND w.is_active = true
             ORDER BY w.created_at DESC
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn soft_delete_workspace_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE workspaces SET is_active = false, deleted_at = NOW(), updated_at = NOW() WHERE id = $1",
            id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn soft_delete_workspace(&self, id: Uuid) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        self.soft_delete_workspace_in(&mut tx, id).await?;
        tx.commit().await
    }

    pub async fn hard_delete_workspace_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM workspaces WHERE id = $1", id)
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    pub async fn hard_delete_workspace(&self, id: Uuid) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        self.hard_delete_workspace_in(&mut tx, id).await?;
        tx.commit().await
    }

    pub async fn restore_workspace_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
    ) -> sqlx::Result<Workspace> {
        sqlx::query_as!(
            Workspace,
            r#"UPDATE workspaces SET is_active = true, deleted_at = NULL, updated_at = NOW() WHERE id = $1
               RETURNING id, name, slug, description, icon_url, owner_id, settings AS "settings!",
                   is_active AS "is_active!", deleted_at, created_at, updated_at"#,
            id
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn restore_workspace(&self, id: Uuid) -> sqlx::Result<Workspace> {
        let mut tx = self.pool.begin().await?;
        let workspace = self.restore_workspace_in(&mut tx, id).await?;
        tx.commit().await?;
        Ok(workspace)
    }

    pub async fn list_deleted_workspaces_for_user(
        &self,
        user_id: Uuid,
        is_instance_admin: bool,
    ) -> sqlx::Result<Vec<Workspace>> {
        if is_instance_admin {
            sqlx::query_as!(
                Workspace,
                r#"SELECT id, name, slug, description, icon_url, owner_id, settings AS "settings!",
                   is_active AS "is_active!", deleted_at, created_at, updated_at
                     FROM workspaces WHERE is_active = false ORDER BY deleted_at DESC"#
            )
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as!(
                Workspace,
                r#"
                SELECT w.id, w.name, w.slug, w.description, w.icon_url, w.owner_id, w.settings AS "settings!",
                   w.is_active AS "is_active!", w.deleted_at, w.created_at, w.updated_at
                  FROM workspaces w
                  JOIN workspace_members wm ON wm.workspace_id = w.id
                 WHERE wm.user_id = $1 AND w.is_active = false
                   AND wm.role IN ('admin', 'owner')
                 ORDER BY w.deleted_at DESC
                "#,
                user_id
            )
            .fetch_all(&self.pool)
            .await
        }
    }

    pub async fn update_workspace(
        &self,
        id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
        icon_url: Option<&str>,
    ) -> sqlx::Result<Workspace> {
        sqlx::query_as!(
            Workspace,
            r#"
            UPDATE workspaces
            SET name = COALESCE($2, name),
                description = COALESCE($3, description),
                -- An empty string clears it. Plain COALESCE could only ever set
                -- an icon, never take one off, so the remove button had nothing
                -- to say.
                icon_url = CASE WHEN $4::text = '' THEN NULL ELSE COALESCE($4, icon_url) END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, slug, description, icon_url, owner_id, settings AS "settings!",
                   is_active AS "is_active!", deleted_at, created_at, updated_at
            "#,
            id,
            name,
            description,
            icon_url
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn add_member_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &WorkspaceRole,
    ) -> sqlx::Result<WorkspaceMember> {
        sqlx::query_as!(
            WorkspaceMember,
            r#"
            INSERT INTO workspace_members (workspace_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = $3
            RETURNING workspace_id, user_id, role AS "role: WorkspaceRole", joined_at
            "#,
            workspace_id,
            user_id,
            role.clone() as WorkspaceRole
        )
        .fetch_one(&mut **tx)
        .await
    }

    pub async fn add_member_if_absent_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &WorkspaceRole,
    ) -> sqlx::Result<Option<WorkspaceMember>> {
        sqlx::query_as!(
            WorkspaceMember,
            r#"
            INSERT INTO workspace_members (workspace_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, user_id) DO NOTHING
            RETURNING workspace_id, user_id, role AS "role: WorkspaceRole", joined_at
            "#,
            workspace_id,
            user_id,
            role.clone() as WorkspaceRole
        )
        .fetch_optional(&mut **tx)
        .await
    }

    pub async fn add_member(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &WorkspaceRole,
    ) -> sqlx::Result<WorkspaceMember> {
        sqlx::query_as!(
            WorkspaceMember,
            r#"
            INSERT INTO workspace_members (workspace_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = workspace_members.role
            RETURNING workspace_id, user_id, role AS "role: WorkspaceRole", joined_at
            "#,
            workspace_id,
            user_id,
            role.clone() as WorkspaceRole
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_channel_by_name(
        &self,
        workspace_id: Uuid,
        name: &str,
    ) -> sqlx::Result<Option<Channel>> {
        sqlx::query_as!(
            Channel,
            r#"SELECT id, workspace_id, name, channel_type AS "channel_type: ChannelType", topic, description,
                   created_by, is_default AS "is_default!", is_archived AS "is_archived!",
                   settings AS "settings!", created_at, updated_at
                 FROM channels WHERE workspace_id = $1 AND name = $2"#,
            workspace_id,
            name
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_member(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Option<WorkspaceMember>> {
        sqlx::query_as!(
            WorkspaceMember,
            r#"SELECT workspace_id, user_id, role AS "role: WorkspaceRole", joined_at
                 FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
            workspace_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Whether these two people are in any channel together. The predicate a
    /// guest's reach is measured by, on both the reading side (the directory)
    /// and the writing side (opening a conversation) -- one rule, asked twice.
    pub async fn share_a_channel(&self, a: Uuid, b: Uuid) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
              SELECT 1
                FROM channel_members mine
                JOIN channel_members theirs ON theirs.channel_id = mine.channel_id
               WHERE mine.user_id = $1 AND theirs.user_id = $2
            ) AS "exists!"
            "#,
            a,
            b
        )
        .fetch_one(&self.pool)
        .await
    }

    /// What a guest is allowed to know: the people they share a channel with,
    /// without their addresses. The redaction happens here rather than in the
    /// handler, because a projection a caller has to remember to apply is a
    /// projection the next caller forgets.
    ///
    /// A display name and an avatar are what the UI needs to render a message.
    /// An email is what somebody outside the company needs to phish it.
    pub async fn list_members_visible_to_guest(
        &self,
        workspace_id: Uuid,
        guest_id: Uuid,
    ) -> sqlx::Result<Vec<MemberWithUser>> {
        sqlx::query_as!(
            MemberWithUser,
            r#"
            SELECT wm.workspace_id, wm.user_id, wm.role AS "role: WorkspaceRole", wm.joined_at,
                   '' AS "email!", u.display_name, u.avatar_url,
                   CASE WHEN u.status_expires_at IS NULL OR u.status_expires_at > NOW()
                        THEN u.status_emoji END AS "status_emoji?",
                   CASE WHEN u.status_expires_at IS NULL OR u.status_expires_at > NOW()
                        THEN u.status_text END AS "status_text?"
            FROM workspace_members wm
            JOIN users u ON u.id = wm.user_id
            WHERE wm.workspace_id = $1
              AND (
                wm.user_id = $2
                OR EXISTS (
                  SELECT 1
                    FROM channel_members mine
                    JOIN channel_members theirs ON theirs.channel_id = mine.channel_id
                   WHERE mine.user_id = $2
                     AND theirs.user_id = wm.user_id
                )
              )
            ORDER BY wm.joined_at
            "#,
            workspace_id,
            guest_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_members_with_users(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<MemberWithUser>> {
        sqlx::query_as!(
            MemberWithUser,
            r#"
            SELECT wm.workspace_id, wm.user_id, wm.role AS "role: WorkspaceRole", wm.joined_at,
                   u.email, u.display_name, u.avatar_url,
                   CASE WHEN u.status_expires_at IS NULL OR u.status_expires_at > NOW()
                        THEN u.status_emoji END AS "status_emoji?",
                   CASE WHEN u.status_expires_at IS NULL OR u.status_expires_at > NOW()
                        THEN u.status_text END AS "status_text?"
            FROM workspace_members wm
            JOIN users u ON u.id = wm.user_id
            WHERE wm.workspace_id = $1
            ORDER BY wm.joined_at
            "#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_member_role(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &WorkspaceRole,
    ) -> sqlx::Result<WorkspaceMember> {
        sqlx::query_as!(
            WorkspaceMember,
            r#"UPDATE workspace_members SET role = $3 WHERE workspace_id = $1 AND user_id = $2
               RETURNING workspace_id, user_id, role AS "role: WorkspaceRole", joined_at"#,
            workspace_id,
            user_id,
            role.clone() as WorkspaceRole
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Removal also drops the channel memberships. Leaving them behind meant the
    /// realtime gateway — which only checks `channel_members` — kept delivering,
    /// and re-adding the person silently restored every private channel they had
    /// ever been in.
    pub async fn remove_member_in(
        &self,
        conn: &mut PgConnection,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Uuid>> {
        let channel_ids: Vec<Uuid> = sqlx::query_scalar!(
            r"
            DELETE FROM channel_members cm
             USING channels c
             WHERE cm.channel_id = c.id
               AND c.workspace_id = $1
               AND cm.user_id = $2
            RETURNING cm.channel_id
            ",
            workspace_id,
            user_id
        )
        .fetch_all(&mut *conn)
        .await?;

        sqlx::query!(
            "DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
            workspace_id,
            user_id
        )
        .execute(&mut *conn)
        .await?;

        Ok(channel_ids)
    }

    pub async fn remove_member(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Uuid>> {
        let mut tx = self.pool.begin().await?;
        let channel_ids = self
            .remove_member_in(&mut tx, workspace_id, user_id)
            .await?;
        tx.commit().await?;
        Ok(channel_ids)
    }

    pub async fn create_invite(&self, invite: NewInvite<'_>) -> sqlx::Result<WorkspaceInvite> {
        sqlx::query_as!(
            WorkspaceInvite,
            r#"
            INSERT INTO workspace_invites
                (workspace_id, created_by, email, role, token, max_uses, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, workspace_id, created_by, email, role AS "role: WorkspaceRole", token, max_uses,
                   use_count AS "use_count!", expires_at, created_at
            "#,
            invite.workspace_id,
            invite.created_by,
            invite.email,
            invite.role.clone() as WorkspaceRole,
            invite.token,
            invite.max_uses,
            invite.expires_at
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_invite_by_token(&self, token: &str) -> sqlx::Result<Option<WorkspaceInvite>> {
        sqlx::query_as!(
            WorkspaceInvite,
            r#"SELECT id, workspace_id, created_by, email, role AS "role: WorkspaceRole", token, max_uses,
                   use_count AS "use_count!", expires_at, created_at
                 FROM workspace_invites WHERE token = $1"#,
            token
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_invite_by_id(&self, id: Uuid) -> sqlx::Result<Option<WorkspaceInvite>> {
        sqlx::query_as!(
            WorkspaceInvite,
            r#"SELECT id, workspace_id, created_by, email, role AS "role: WorkspaceRole", token, max_uses,
                   use_count AS "use_count!", expires_at, created_at
                 FROM workspace_invites WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn claim_invite_use_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> sqlx::Result<Option<WorkspaceInvite>> {
        sqlx::query_as!(
            WorkspaceInvite,
            r#"
            UPDATE workspace_invites
            SET use_count = use_count + 1
            WHERE id = $1
              AND (max_uses IS NULL OR use_count < max_uses)
            RETURNING id, workspace_id, created_by, email, role AS "role: WorkspaceRole", token, max_uses,
                   use_count AS "use_count!", expires_at, created_at
            "#,
            id
        )
        .fetch_optional(&mut **tx)
        .await
    }

    pub async fn list_invites(&self, workspace_id: Uuid) -> sqlx::Result<Vec<WorkspaceInvite>> {
        sqlx::query_as!(
            WorkspaceInvite,
            r#"SELECT id, workspace_id, created_by, email, role AS "role: WorkspaceRole", token, max_uses,
                   use_count AS "use_count!", expires_at, created_at
                 FROM workspace_invites WHERE workspace_id = $1 ORDER BY created_at DESC"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete_invite(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM workspace_invites WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_channel(
        &self,
        workspace_id: Uuid,
        name: &str,
        channel_type: &ChannelType,
        description: Option<&str>,
        created_by: Uuid,
        is_default: bool,
    ) -> sqlx::Result<Channel> {
        sqlx::query_as!(
            Channel,
            r#"
            INSERT INTO channels (workspace_id, name, channel_type, description, created_by, is_default)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, workspace_id, name, channel_type AS "channel_type: ChannelType", topic, description,
                   created_by, is_default AS "is_default!", is_archived AS "is_archived!",
                   settings AS "settings!", created_at, updated_at
            "#,
            workspace_id,
            name,
            channel_type.clone() as ChannelType,
            description,
            created_by,
            is_default
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn create_channel_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        workspace_id: Uuid,
        name: &str,
        channel_type: &ChannelType,
        description: Option<&str>,
        created_by: Uuid,
        is_default: bool,
    ) -> sqlx::Result<Channel> {
        sqlx::query_as!(
            Channel,
            r#"
            INSERT INTO channels (workspace_id, name, channel_type, description, created_by, is_default)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, workspace_id, name, channel_type AS "channel_type: ChannelType", topic, description,
                   created_by, is_default AS "is_default!", is_archived AS "is_archived!",
                   settings AS "settings!", created_at, updated_at
            "#,
            workspace_id,
            name,
            channel_type.clone() as ChannelType,
            description,
            created_by,
            is_default
        )
        .fetch_one(&mut **tx)
        .await
    }

    pub async fn find_channel_by_id(&self, id: Uuid) -> sqlx::Result<Option<Channel>> {
        sqlx::query_as!(
            Channel,
            r#"SELECT id, workspace_id, name, channel_type AS "channel_type: ChannelType", topic, description,
                   created_by, is_default AS "is_default!", is_archived AS "is_archived!",
                   settings AS "settings!", created_at, updated_at
                 FROM channels WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_default_channels_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<Channel>> {
        sqlx::query_as!(
            Channel,
            r#"SELECT id, workspace_id, name, channel_type AS "channel_type: ChannelType", topic, description,
                   created_by, is_default AS "is_default!", is_archived AS "is_archived!",
                   settings AS "settings!", created_at, updated_at
                 FROM channels WHERE workspace_id = $1 AND is_archived = false AND is_default = true ORDER BY name"#,
            workspace_id
        )
        .fetch_all(&mut **tx)
        .await
    }

    pub async fn update_channel(
        &self,
        id: Uuid,
        name: Option<&str>,
        topic: Option<&str>,
        description: Option<&str>,
    ) -> sqlx::Result<Channel> {
        sqlx::query_as!(
            Channel,
            r#"
            UPDATE channels
            SET name = COALESCE($2, name),
                topic = COALESCE($3, topic),
                description = COALESCE($4, description),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, workspace_id, name, channel_type AS "channel_type: ChannelType", topic, description,
                   created_by, is_default AS "is_default!", is_archived AS "is_archived!",
                   settings AS "settings!", created_at, updated_at
            "#,
            id,
            name,
            topic,
            description
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Merged into `settings` rather than replacing it: the column is a bag
    /// that later features will also want, and a whole-object write would
    /// silently drop whatever they put there.
    pub async fn set_channel_post_policy(&self, id: Uuid, policy: &str) -> sqlx::Result<Channel> {
        sqlx::query_as!(
            Channel,
            r#"
            UPDATE channels
               SET settings = COALESCE(settings, '{}'::jsonb)
                              || jsonb_build_object('post_policy', $2::text),
                   updated_at = NOW()
             WHERE id = $1
            RETURNING id, workspace_id, name, channel_type AS "channel_type: ChannelType", topic, description,
                   created_by, is_default AS "is_default!", is_archived AS "is_archived!",
                   settings AS "settings!", created_at, updated_at
            "#,
            id,
            policy
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn archive_channel(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE channels SET is_archived = true, updated_at = NOW() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn add_channel_member(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        role: &ChannelRole,
    ) -> sqlx::Result<ChannelMember> {
        sqlx::query_as!(
            ChannelMember,
            r#"
            INSERT INTO channel_members (channel_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (channel_id, user_id) DO UPDATE SET channel_id = EXCLUDED.channel_id
            RETURNING channel_id, user_id, role AS "role: ChannelRole", last_read_at, last_read_msg,
                   notifications AS "notifications!", is_muted AS "is_muted!",
                   is_starred AS "is_starred!", joined_at
            "#,
            channel_id,
            user_id,
            role.clone() as ChannelRole
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn add_channel_member_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        channel_id: Uuid,
        user_id: Uuid,
        role: &ChannelRole,
    ) -> sqlx::Result<Option<ChannelMember>> {
        sqlx::query_as!(
            ChannelMember,
            r#"
            INSERT INTO channel_members (channel_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (channel_id, user_id) DO NOTHING
            RETURNING channel_id, user_id, role AS "role: ChannelRole", last_read_at, last_read_msg,
                   notifications AS "notifications!", is_muted AS "is_muted!",
                   is_starred AS "is_starred!", joined_at
            "#,
            channel_id,
            user_id,
            role.clone() as ChannelRole
        )
        .fetch_optional(&mut **tx)
        .await
    }

    pub async fn update_channel_member_role(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        role: &ChannelRole,
    ) -> sqlx::Result<ChannelMember> {
        sqlx::query_as!(
            ChannelMember,
            r#"UPDATE channel_members SET role = $3 WHERE channel_id = $1 AND user_id = $2
               RETURNING channel_id, user_id, role AS "role: ChannelRole", last_read_at, last_read_msg,
                   notifications AS "notifications!", is_muted AS "is_muted!",
                   is_starred AS "is_starred!", joined_at"#,
            channel_id,
            user_id,
            role.clone() as ChannelRole
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_channel_bookmarks(
        &self,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<ChannelBookmark>> {
        sqlx::query_as!(
            ChannelBookmark,
            "SELECT id, channel_id, created_by, label, url, emoji, position, created_at FROM channel_bookmarks WHERE channel_id = $1 ORDER BY position, created_at",
            channel_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn create_channel_bookmark(
        &self,
        channel_id: Uuid,
        created_by: Uuid,
        label: &str,
        url: &str,
        emoji: Option<&str>,
    ) -> sqlx::Result<ChannelBookmark> {
        sqlx::query_as!(
            ChannelBookmark,
            r"
            INSERT INTO channel_bookmarks (channel_id, created_by, label, url, emoji, position)
            VALUES ($1, $2, $3, $4, $5,
                    COALESCE((SELECT MAX(position) + 1 FROM channel_bookmarks WHERE channel_id = $1), 0))
            RETURNING id, channel_id, created_by, label, url, emoji, position, created_at
            ",
            channel_id,
            created_by,
            label,
            url,
            emoji
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_channel_bookmark(&self, id: Uuid) -> sqlx::Result<Option<ChannelBookmark>> {
        sqlx::query_as!(
            ChannelBookmark,
            "SELECT id, channel_id, created_by, label, url, emoji, position, created_at FROM channel_bookmarks WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete_channel_bookmark(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM channel_bookmarks WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_channel_member(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Option<ChannelMember>> {
        sqlx::query_as!(
            ChannelMember,
            r#"SELECT channel_id, user_id, role AS "role: ChannelRole", last_read_at, last_read_msg,
                   notifications AS "notifications!", is_muted AS "is_muted!",
                   is_starred AS "is_starred!", joined_at
                 FROM channel_members WHERE channel_id = $1 AND user_id = $2"#,
            channel_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_channel_members(&self, channel_id: Uuid) -> sqlx::Result<Vec<ChannelMember>> {
        sqlx::query_as!(
            ChannelMember,
            r#"SELECT channel_id, user_id, role AS "role: ChannelRole", last_read_at, last_read_msg,
                   notifications AS "notifications!", is_muted AS "is_muted!",
                   is_starred AS "is_starred!", joined_at
                 FROM channel_members WHERE channel_id = $1 ORDER BY joined_at"#,
            channel_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn remove_channel_member_in(
        &self,
        conn: &mut PgConnection,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2",
            channel_id,
            user_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn remove_channel_member(&self, channel_id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        self.remove_channel_member_in(&mut tx, channel_id, user_id)
            .await?;
        tx.commit().await
    }

    pub async fn list_user_channels(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Channel>> {
        sqlx::query_as!(
            Channel,
            r#"
            SELECT c.id, c.workspace_id, c.name, c.channel_type AS "channel_type: ChannelType", c.topic, c.description,
                   c.created_by, c.is_default AS "is_default!", c.is_archived AS "is_archived!",
                   c.settings AS "settings!", c.created_at, c.updated_at
              FROM channels c
              JOIN channel_members cm ON cm.channel_id = c.id
             WHERE c.workspace_id = $1 AND cm.user_id = $2 AND c.is_archived = false
               AND c.channel_type IN ('public', 'private')
             ORDER BY c.is_default DESC, c.name
            "#,
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_browsable_channels(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<BrowsableChannel>> {
        sqlx::query_as!(
            BrowsableChannel,
            r#"
            SELECT c.id,
                   c.workspace_id,
                   c.name,
                   c.channel_type AS "channel_type: ChannelType",
                   c.topic,
                   c.description,
                   c.is_default AS "is_default!",
                   c.created_at,
                   COUNT(cm.user_id) AS "member_count!",
                   COALESCE(BOOL_OR(cm.user_id = $2), false) AS "is_member!"
            FROM channels c
            LEFT JOIN channel_members cm ON cm.channel_id = c.id
            WHERE c.workspace_id = $1
              AND c.channel_type = 'public'
              AND c.is_archived = false
            GROUP BY c.id
            ORDER BY c.name
            "#,
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn set_channel_muted(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        muted: bool,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE channel_members SET muted = $3 WHERE channel_id = $1 AND user_id = $2",
            channel_id,
            user_id,
            muted
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn muted_channel_ids(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r"
            SELECT c.id FROM channels c
            JOIN channel_members cm ON cm.channel_id = c.id
            WHERE c.workspace_id = $1 AND cm.user_id = $2 AND cm.muted = true
            ",
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reads the denormalised counter rather than asking, once per channel,
    /// whether any message is newer than the read mark. The old shape grew with
    /// message volume; this one is bounded by how many channels the user is in.
    pub async fn unread_channel_ids(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r"
            SELECT c.id
            FROM channels c
            JOIN channel_members cm ON cm.channel_id = c.id
            WHERE c.workspace_id = $1 AND cm.user_id = $2 AND c.is_archived = false
              AND cm.unread_count > 0
            ",
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }
}
