use chrono::{DateTime, Utc};
use sqlx::PgPool;
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

    pub async fn add_member(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &WorkspaceRole,
    ) -> sqlx::Result<WorkspaceMember> {
        sqlx::query_as::<_, WorkspaceMember>(
            r"
            INSERT INTO workspace_members (workspace_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = workspace_members.role
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(role)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_channel_by_name(
        &self,
        workspace_id: Uuid,
        name: &str,
    ) -> sqlx::Result<Option<Channel>> {
        sqlx::query_as::<_, Channel>("SELECT * FROM channels WHERE workspace_id = $1 AND name = $2")
            .bind(workspace_id)
            .bind(name)
            .fetch_optional(&self.pool)
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

    /// Whether these two people are in any channel together. The predicate a
    /// guest's reach is measured by, on both the reading side (the directory)
    /// and the writing side (opening a conversation) -- one rule, asked twice.
    pub async fn share_a_channel(&self, a: Uuid, b: Uuid) -> sqlx::Result<bool> {
        sqlx::query_scalar::<_, bool>(
            r"
            SELECT EXISTS (
              SELECT 1
                FROM channel_members mine
                JOIN channel_members theirs ON theirs.channel_id = mine.channel_id
               WHERE mine.user_id = $1 AND theirs.user_id = $2
            )
            ",
        )
        .bind(a)
        .bind(b)
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
        sqlx::query_as::<_, MemberWithUser>(
            r"
            SELECT wm.workspace_id, wm.user_id, wm.role, wm.joined_at,
                   '' AS email, u.display_name, u.avatar_url,
                   CASE WHEN u.status_expires_at IS NULL OR u.status_expires_at > NOW()
                        THEN u.status_emoji END AS status_emoji,
                   CASE WHEN u.status_expires_at IS NULL OR u.status_expires_at > NOW()
                        THEN u.status_text END AS status_text
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
            ",
        )
        .bind(workspace_id)
        .bind(guest_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_members_with_users(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<MemberWithUser>> {
        sqlx::query_as::<_, MemberWithUser>(
            r"
            SELECT wm.workspace_id, wm.user_id, wm.role, wm.joined_at,
                   u.email, u.display_name, u.avatar_url,
                   CASE WHEN u.status_expires_at IS NULL OR u.status_expires_at > NOW()
                        THEN u.status_emoji END AS status_emoji,
                   CASE WHEN u.status_expires_at IS NULL OR u.status_expires_at > NOW()
                        THEN u.status_text END AS status_text
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

    /// Removal also drops the channel memberships. Leaving them behind meant the
    /// realtime gateway — which only checks `channel_members` — kept delivering,
    /// and re-adding the person silently restored every private channel they had
    /// ever been in.
    pub async fn remove_member(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Uuid>> {
        let mut tx = self.pool.begin().await?;

        let channel_ids: Vec<Uuid> = sqlx::query_scalar(
            r"
            DELETE FROM channel_members cm
             USING channels c
             WHERE cm.channel_id = c.id
               AND c.workspace_id = $1
               AND cm.user_id = $2
            RETURNING cm.channel_id
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
            .bind(workspace_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(channel_ids)
    }

    pub async fn create_invite(&self, invite: NewInvite<'_>) -> sqlx::Result<WorkspaceInvite> {
        sqlx::query_as::<_, WorkspaceInvite>(
            r"
            INSERT INTO workspace_invites
                (workspace_id, created_by, email, role, token, max_uses, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            ",
        )
        .bind(invite.workspace_id)
        .bind(invite.created_by)
        .bind(invite.email)
        .bind(invite.role)
        .bind(invite.token)
        .bind(invite.max_uses)
        .bind(invite.expires_at)
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

    /// Merged into `settings` rather than replacing it: the column is a bag
    /// that later features will also want, and a whole-object write would
    /// silently drop whatever they put there.
    pub async fn set_channel_post_policy(&self, id: Uuid, policy: &str) -> sqlx::Result<Channel> {
        sqlx::query_as::<_, Channel>(
            r"
            UPDATE channels
               SET settings = COALESCE(settings, '{}'::jsonb)
                              || jsonb_build_object('post_policy', $2::text),
                   updated_at = NOW()
             WHERE id = $1
            RETURNING *
            ",
        )
        .bind(id)
        .bind(policy)
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
            ON CONFLICT (channel_id, user_id) DO UPDATE SET channel_id = EXCLUDED.channel_id
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

    pub async fn update_channel_member_role(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        role: &ChannelRole,
    ) -> sqlx::Result<ChannelMember> {
        sqlx::query_as::<_, ChannelMember>(
            "UPDATE channel_members SET role = $3 WHERE channel_id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(channel_id)
        .bind(user_id)
        .bind(role)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_channel_bookmarks(
        &self,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<ChannelBookmark>> {
        sqlx::query_as::<_, ChannelBookmark>(
            "SELECT * FROM channel_bookmarks WHERE channel_id = $1 ORDER BY position, created_at",
        )
        .bind(channel_id)
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
        sqlx::query_as::<_, ChannelBookmark>(
            r"
            INSERT INTO channel_bookmarks (channel_id, created_by, label, url, emoji, position)
            VALUES ($1, $2, $3, $4, $5,
                    COALESCE((SELECT MAX(position) + 1 FROM channel_bookmarks WHERE channel_id = $1), 0))
            RETURNING *
            ",
        )
        .bind(channel_id)
        .bind(created_by)
        .bind(label)
        .bind(url)
        .bind(emoji)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_channel_bookmark(&self, id: Uuid) -> sqlx::Result<Option<ChannelBookmark>> {
        sqlx::query_as::<_, ChannelBookmark>("SELECT * FROM channel_bookmarks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn delete_channel_bookmark(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM channel_bookmarks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
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

    pub async fn list_browsable_channels(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<BrowsableChannel>> {
        sqlx::query_as::<_, BrowsableChannel>(
            r"
            SELECT c.id,
                   c.workspace_id,
                   c.name,
                   c.channel_type,
                   c.topic,
                   c.description,
                   c.is_default,
                   c.created_at,
                   COUNT(cm.user_id) AS member_count,
                   COALESCE(BOOL_OR(cm.user_id = $2), false) AS is_member
            FROM channels c
            LEFT JOIN channel_members cm ON cm.channel_id = c.id
            WHERE c.workspace_id = $1
              AND c.channel_type = 'public'
              AND c.is_archived = false
            GROUP BY c.id
            ORDER BY c.name
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

    /// Reads the denormalised counter rather than asking, once per channel,
    /// whether any message is newer than the read mark. The old shape grew with
    /// message volume; this one is bounded by how many channels the user is in.
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
              AND cm.unread_count > 0
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}
