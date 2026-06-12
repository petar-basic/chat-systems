use sqlx::PgPool;
use uuid::Uuid;

use super::models::*;

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
        sqlx::query_as::<_, Workspace>(
            r"
            INSERT INTO workspaces (name, slug, description, owner_id)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            ",
        )
        .bind(name)
        .bind(slug)
        .bind(description)
        .bind(owner_id)
        .fetch_one(&mut **tx)
        .await
    }

    pub async fn find_workspace_by_id(&self, id: Uuid) -> sqlx::Result<Option<Workspace>> {
        sqlx::query_as::<_, Workspace>("SELECT * FROM workspaces WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_user_workspaces(&self, user_id: Uuid) -> sqlx::Result<Vec<Workspace>> {
        sqlx::query_as::<_, Workspace>(
            r"
            SELECT w.* FROM workspaces w
            JOIN workspace_members wm ON wm.workspace_id = w.id
            WHERE wm.user_id = $1 AND w.is_active = true
            ORDER BY w.created_at DESC
            ",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn soft_delete_workspace(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE workspaces SET is_active = false, deleted_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn hard_delete_workspace(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM workspaces WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn restore_workspace(&self, id: Uuid) -> sqlx::Result<Workspace> {
        sqlx::query_as::<_, Workspace>(
            "UPDATE workspaces SET is_active = true, deleted_at = NULL, updated_at = NOW() WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_deleted_workspaces_for_user(
        &self,
        user_id: Uuid,
        is_instance_admin: bool,
    ) -> sqlx::Result<Vec<Workspace>> {
        if is_instance_admin {
            sqlx::query_as::<_, Workspace>(
                "SELECT * FROM workspaces WHERE is_active = false ORDER BY deleted_at DESC",
            )
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, Workspace>(
                r"
                SELECT w.* FROM workspaces w
                JOIN workspace_members wm ON wm.workspace_id = w.id
                WHERE wm.user_id = $1 AND w.is_active = false
                  AND wm.role IN ('admin', 'owner')
                ORDER BY w.deleted_at DESC
                ",
            )
            .bind(user_id)
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
        sqlx::query_as::<_, Workspace>(
            r"
            UPDATE workspaces
            SET name = COALESCE($2, name),
                description = COALESCE($3, description),
                icon_url = COALESCE($4, icon_url),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            ",
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(icon_url)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn add_member_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &WorkspaceRole,
    ) -> sqlx::Result<WorkspaceMember> {
        sqlx::query_as::<_, WorkspaceMember>(
            r"
            INSERT INTO workspace_members (workspace_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = $3
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(role)
        .fetch_one(&mut **tx)
        .await
    }

    pub async fn get_member(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Option<WorkspaceMember>> {
        sqlx::query_as::<_, WorkspaceMember>(
            "SELECT * FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_members_with_users(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<MemberWithUser>> {
        sqlx::query_as::<_, MemberWithUser>(
            r"
            SELECT wm.workspace_id, wm.user_id, wm.role, wm.joined_at,
                   u.email, u.display_name, u.avatar_url
            FROM workspace_members wm
            JOIN users u ON u.id = wm.user_id
            WHERE wm.workspace_id = $1
            ORDER BY wm.joined_at
            ",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_member_role(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &WorkspaceRole,
    ) -> sqlx::Result<WorkspaceMember> {
        sqlx::query_as::<_, WorkspaceMember>(
            "UPDATE workspace_members SET role = $3 WHERE workspace_id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(role)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn remove_member(&self, workspace_id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
            .bind(workspace_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_invite(
        &self,
        workspace_id: Uuid,
        created_by: Uuid,
        email: Option<&str>,
        role: &WorkspaceRole,
        token: &str,
    ) -> sqlx::Result<WorkspaceInvite> {
        sqlx::query_as::<_, WorkspaceInvite>(
            r"
            INSERT INTO workspace_invites (workspace_id, created_by, email, role, token)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(created_by)
        .bind(email)
        .bind(role)
        .bind(token)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_invite_by_token(&self, token: &str) -> sqlx::Result<Option<WorkspaceInvite>> {
        sqlx::query_as::<_, WorkspaceInvite>("SELECT * FROM workspace_invites WHERE token = $1")
            .bind(token)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn find_invite_by_id(&self, id: Uuid) -> sqlx::Result<Option<WorkspaceInvite>> {
        sqlx::query_as::<_, WorkspaceInvite>("SELECT * FROM workspace_invites WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn claim_invite_use_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> sqlx::Result<Option<WorkspaceInvite>> {
        sqlx::query_as::<_, WorkspaceInvite>(
            r"
            UPDATE workspace_invites
            SET use_count = use_count + 1
            WHERE id = $1
              AND (max_uses IS NULL OR use_count < max_uses)
            RETURNING *
            ",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
    }

    pub async fn list_invites(&self, workspace_id: Uuid) -> sqlx::Result<Vec<WorkspaceInvite>> {
        sqlx::query_as::<_, WorkspaceInvite>(
            "SELECT * FROM workspace_invites WHERE workspace_id = $1 ORDER BY created_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete_invite(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM workspace_invites WHERE id = $1")
            .bind(id)
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
        sqlx::query_as::<_, Channel>(
            r"
            INSERT INTO channels (workspace_id, name, channel_type, description, created_by, is_default)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(name)
        .bind(channel_type)
        .bind(description)
        .bind(created_by)
        .bind(is_default)
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
        sqlx::query_as::<_, Channel>(
            r"
            INSERT INTO channels (workspace_id, name, channel_type, description, created_by, is_default)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(name)
        .bind(channel_type)
        .bind(description)
        .bind(created_by)
        .bind(is_default)
        .fetch_one(&mut **tx)
        .await
    }

    pub async fn find_channel_by_id(&self, id: Uuid) -> sqlx::Result<Option<Channel>> {
        sqlx::query_as::<_, Channel>("SELECT * FROM channels WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_default_channels_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<Channel>> {
        sqlx::query_as::<_, Channel>(
            "SELECT * FROM channels WHERE workspace_id = $1 AND is_archived = false AND is_default = true ORDER BY name",
        )
        .bind(workspace_id)
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
        sqlx::query_as::<_, Channel>(
            r"
            UPDATE channels
            SET name = COALESCE($2, name),
                topic = COALESCE($3, topic),
                description = COALESCE($4, description),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            ",
        )
        .bind(id)
        .bind(name)
        .bind(topic)
        .bind(description)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn archive_channel(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("UPDATE channels SET is_archived = true, updated_at = NOW() WHERE id = $1")
            .bind(id)
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
        sqlx::query_as::<_, ChannelMember>(
            r"
            INSERT INTO channel_members (channel_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (channel_id, user_id) DO NOTHING
            RETURNING *
            ",
        )
        .bind(channel_id)
        .bind(user_id)
        .bind(role)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn add_channel_member_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        channel_id: Uuid,
        user_id: Uuid,
        role: &ChannelRole,
    ) -> sqlx::Result<Option<ChannelMember>> {
        sqlx::query_as::<_, ChannelMember>(
            r"
            INSERT INTO channel_members (channel_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (channel_id, user_id) DO NOTHING
            RETURNING *
            ",
        )
        .bind(channel_id)
        .bind(user_id)
        .bind(role)
        .fetch_optional(&mut **tx)
        .await
    }

    pub async fn get_channel_member(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Option<ChannelMember>> {
        sqlx::query_as::<_, ChannelMember>(
            "SELECT * FROM channel_members WHERE channel_id = $1 AND user_id = $2",
        )
        .bind(channel_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_channel_members(&self, channel_id: Uuid) -> sqlx::Result<Vec<ChannelMember>> {
        sqlx::query_as::<_, ChannelMember>(
            "SELECT * FROM channel_members WHERE channel_id = $1 ORDER BY joined_at",
        )
        .bind(channel_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn remove_channel_member(&self, channel_id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2")
            .bind(channel_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_user_channels(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Channel>> {
        sqlx::query_as::<_, Channel>(
            r"
            SELECT c.* FROM channels c
            JOIN channel_members cm ON cm.channel_id = c.id
            WHERE c.workspace_id = $1 AND cm.user_id = $2 AND c.is_archived = false
            ORDER BY c.is_default DESC, c.name
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn set_channel_muted(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        muted: bool,
    ) -> sqlx::Result<()> {
        sqlx::query("UPDATE channel_members SET muted = $3 WHERE channel_id = $1 AND user_id = $2")
            .bind(channel_id)
            .bind(user_id)
            .bind(muted)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn muted_channel_ids(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r"
            SELECT c.id FROM channels c
            JOIN channel_members cm ON cm.channel_id = c.id
            WHERE c.workspace_id = $1 AND cm.user_id = $2 AND cm.muted = true
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn unread_channel_ids(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r"
            SELECT c.id
            FROM channels c
            JOIN channel_members cm ON cm.channel_id = c.id
            WHERE c.workspace_id = $1 AND cm.user_id = $2 AND c.is_archived = false
              AND EXISTS (
                SELECT 1 FROM messages m
                WHERE m.channel_id = c.id
                  AND m.deleted_at IS NULL
                  AND m.user_id <> $2
                  AND (cm.last_read_at IS NULL OR m.created_at > cm.last_read_at)
              )
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}
