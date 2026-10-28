use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{FileKind, FileRecord, FileTextStatus, TextJob};
use super::text::{snippet_parts, SnippetPart};

pub struct NewFile<'a> {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub message_id: Option<Uuid>,
    pub filename: &'a str,
    pub storage_key: &'a str,
    pub mime_type: &'a str,
    pub size_bytes: i64,
    pub draft_until: Option<DateTime<Utc>>,
    pub thumbnail_key: Option<&'a str>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

pub struct FileSearch<'a> {
    pub query: &'a str,
    pub workspace_id: Uuid,
    pub requester_id: Uuid,
    pub requester_is_guest: bool,
    pub channel_id: Option<Uuid>,
    pub uploader_id: Option<Uuid>,
    pub kind: Option<FileKind>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub limit: i64,
    pub offset: i64,
}

pub struct FileMatch {
    pub record: FileRecord,
    pub message_id: Uuid,
    pub channel_id: Uuid,
    pub kind: FileKind,
    pub snippet: Option<Vec<SnippetPart>>,
}

pub struct FileRepo {
    pool: PgPool,
}

impl FileRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, file: NewFile<'_>) -> sqlx::Result<FileRecord> {
        let mut conn = self.pool.acquire().await?;
        self.create_in(&mut conn, file).await
    }

    pub async fn create_in(
        &self,
        conn: &mut PgConnection,
        file: NewFile<'_>,
    ) -> sqlx::Result<FileRecord> {
        sqlx::query_as!(
            FileRecord,
            r"
            INSERT INTO files (user_id, workspace_id, message_id, filename, storage_key, mime_type, size_bytes, draft_until, thumbnail_key, width, height)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at
            ",
            file.user_id,
            file.workspace_id,
            file.message_id,
            file.filename,
            file.storage_key,
            file.mime_type,
            file.size_bytes,
            file.draft_until,
            file.thumbnail_key,
            file.width,
            file.height
        )
        .fetch_one(conn)
        .await
    }

    pub async fn for_message(&self, message_id: Uuid) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as!(
            FileRecord,
            "SELECT id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at
               FROM files WHERE message_id = $1 ORDER BY created_at, id",
            message_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn claim_drafts_in(
        &self,
        conn: &mut PgConnection,
        ids: &[Uuid],
        message_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as!(
            FileRecord,
            "UPDATE files SET message_id = $2, draft_until = NULL
              WHERE id = ANY($1) AND workspace_id = $3 AND user_id = $4
                AND message_id IS NULL AND draft_until IS NOT NULL
              RETURNING id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at",
            ids,
            message_id,
            workspace_id,
            user_id
        )
        .fetch_all(conn)
        .await
    }

    pub async fn hold_drafts(
        &self,
        ids: &[Uuid],
        workspace_id: Uuid,
        user_id: Uuid,
        until: DateTime<Utc>,
    ) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            "UPDATE files SET draft_until = $4
              WHERE id = ANY($1) AND workspace_id = $2 AND user_id = $3
                AND message_id IS NULL AND draft_until IS NOT NULL",
            ids,
            workspace_id,
            user_id,
            until
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_from_message_except_in(
        &self,
        conn: &mut PgConnection,
        message_id: Uuid,
        keep: &[Uuid],
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as!(
            FileRecord,
            "DELETE FROM files WHERE message_id = $1 AND NOT (id = ANY($2))
              RETURNING id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at",
            message_id,
            keep
        )
        .fetch_all(conn)
        .await
    }

    pub async fn delete_expired_drafts(&self, limit: i64) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as!(
            FileRecord,
            "DELETE FROM files WHERE id IN (
                 SELECT id FROM files
                  WHERE draft_until < NOW() AND message_id IS NULL
                  ORDER BY draft_until
                  LIMIT $1
             )
             RETURNING id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at",
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<FileRecord>> {
        sqlx::query_as!(FileRecord, "SELECT id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at FROM files WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn find_by_storage_key(&self, key: &str) -> sqlx::Result<Option<FileRecord>> {
        sqlx::query_as!(FileRecord, "SELECT id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at FROM files WHERE storage_key = $1 OR thumbnail_key = $1", key)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_by_workspace_for_user(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as!(
            FileRecord,
            "SELECT id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at FROM files WHERE workspace_id = $1 AND user_id = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4",
            workspace_id,
            user_id,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn for_channel(
        &self,
        channel_id: Uuid,
        kind: Option<&str>,
        cursor: Option<Uuid>,
        limit: i64,
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as!(
            FileRecord,
            r"
            SELECT f.id, f.message_id, f.user_id, f.workspace_id, f.filename, f.storage_key,
                   f.mime_type, f.size_bytes, f.thumbnail_key, f.width, f.height, f.created_at
              FROM files f
              JOIN messages m ON m.id = f.message_id
             WHERE m.channel_id = $1
               AND m.deleted_at IS NULL
               AND CASE $2::text
                       WHEN 'image' THEN f.mime_type LIKE 'image/%'
                       WHEN 'video' THEN f.mime_type LIKE 'video/%'
                       WHEN 'audio' THEN f.mime_type LIKE 'audio/%'
                       WHEN 'document' THEN NOT (f.mime_type LIKE 'image/%'
                                                 OR f.mime_type LIKE 'video/%'
                                                 OR f.mime_type LIKE 'audio/%')
                       ELSE TRUE
                   END
               AND ($3::uuid IS NULL
                    OR (f.created_at, f.id) < (SELECT c.created_at, c.id FROM files c WHERE c.id = $3))
             ORDER BY f.created_at DESC, f.id DESC
             LIMIT $4
            ",
            channel_id,
            kind,
            cursor,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn search(&self, params: FileSearch<'_>) -> sqlx::Result<Vec<FileMatch>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query_scalar!(
            r#"SELECT set_config('plan_cache_mode', 'force_custom_plan', true) AS "mode!""#
        )
        .fetch_one(&mut *tx)
        .await?;
        let rows = sqlx::query!(
            r#"
            WITH visible AS MATERIALIZED (
              SELECT id FROM public.searchable_channels($2, $3, $4, $5, TRUE, TRUE) AS id
            ),
            wanted AS (
              SELECT public.search_normalize($1) AS term,
                     '%' || replace(replace(replace(public.search_normalize($1), '\', '\\'), '%', '\%'), '_', '\_') || '%' AS pattern,
                     plainto_tsquery(public.search_text_config(), public.search_normalize($1)) AS terms
            ),
            candidates AS (
              SELECT f.id FROM files f CROSS JOIN wanted w
               WHERE f.workspace_id = $2
                 AND ($1 = '' OR f.search_name ILIKE w.pattern OR w.term <% f.search_name)
              UNION
              SELECT t.file_id FROM file_texts t CROSS JOIN wanted w
               WHERE $1 <> '' AND t.status = 'done' AND t.search_vector @@ w.terms
            ),
            ranked AS (
              SELECT f.id, f.user_id, f.workspace_id, f.filename, f.storage_key, f.mime_type,
                     f.size_bytes, f.thumbnail_key, f.width, f.height, f.created_at,
                     m.id AS message_id, m.channel_id,
                     public.file_kind_of(f.mime_type, f.filename) AS kind,
                     ($1 <> '' AND f.search_name ILIKE w.pattern) AS near,
                     ($1 <> '' AND (f.search_name ILIKE w.pattern OR w.term <% f.search_name)) AS named,
                     word_similarity(w.term, f.search_name) AS similarity,
                     COALESCE(t.search_vector @@ w.terms, FALSE) AS inside,
                     COALESCE(ts_rank(t.search_vector, w.terms), 0) AS relevance,
                     t.content, w.terms
                FROM candidates c
                JOIN files f ON f.id = c.id
                JOIN messages m ON m.id = f.message_id
                CROSS JOIN wanted w
                LEFT JOIN file_texts t ON t.file_id = f.id AND t.status = 'done' AND $1 <> ''
               WHERE f.workspace_id = $2
                 AND m.deleted_at IS NULL
                 AND m.channel_id IN (SELECT id FROM visible)
                 AND ($6::uuid IS NULL OR f.user_id = $6)
                 AND ($7::file_kind IS NULL OR public.file_kind_of(f.mime_type, f.filename) = $7)
                 AND ($8::timestamptz IS NULL OR f.created_at >= $8)
                 AND ($9::timestamptz IS NULL OR f.created_at < $9)
               ORDER BY near DESC, named DESC, similarity DESC, relevance DESC,
                        f.created_at DESC, f.id DESC
               LIMIT $10 OFFSET $11
            )
            SELECT id AS "id!", user_id AS "user_id!", workspace_id AS "workspace_id!",
                   filename AS "filename!", storage_key AS "storage_key!",
                   mime_type AS "mime_type!", size_bytes AS "size_bytes!",
                   thumbnail_key, width, height, created_at AS "created_at!",
                   message_id AS "message_id!", channel_id AS "channel_id!",
                   kind AS "kind!: FileKind",
                   CASE WHEN inside THEN ts_headline(
                     'public.search_headline', content, terms,
                     'StartSel=' || chr(2) || ', StopSel=' || chr(3)
                       || ', MaxWords=24, MinWords=8, MaxFragments=2, FragmentDelimiter=" … "'
                   ) END AS headline
              FROM ranked
             ORDER BY near DESC, named DESC, similarity DESC, relevance DESC,
                      created_at DESC, id DESC
            "#,
            params.query,
            params.workspace_id,
            params.requester_id,
            params.channel_id,
            params.requester_is_guest,
            params.uploader_id,
            params.kind as Option<FileKind>,
            params.after,
            params.before,
            params.limit,
            params.offset
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows
            .into_iter()
            .map(|row| FileMatch {
                message_id: row.message_id,
                channel_id: row.channel_id,
                kind: row.kind,
                snippet: row.headline.as_deref().map(snippet_parts),
                record: FileRecord {
                    id: row.id,
                    message_id: Some(row.message_id),
                    user_id: row.user_id,
                    workspace_id: row.workspace_id,
                    filename: row.filename,
                    storage_key: row.storage_key,
                    mime_type: row.mime_type,
                    size_bytes: row.size_bytes,
                    thumbnail_key: row.thumbnail_key,
                    width: row.width,
                    height: row.height,
                    created_at: row.created_at,
                },
            })
            .collect())
    }

    pub async fn give_up_stale_texts(
        &self,
        stale_after_secs: f64,
        max_attempts: i32,
    ) -> sqlx::Result<u64> {
        let done = sqlx::query!(
            "UPDATE file_texts SET status = 'failed', extracted_at = NOW()
              WHERE status = 'pending'
                AND claimed_at < NOW() - make_interval(secs => $1)
                AND attempts >= $2",
            stale_after_secs,
            max_attempts
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected())
    }

    pub async fn reclaim_stale_texts(
        &self,
        limit: i64,
        stale_after_secs: f64,
        max_attempts: i32,
    ) -> sqlx::Result<Vec<TextJob>> {
        sqlx::query_as!(
            TextJob,
            r#"
            WITH claimed AS (
              UPDATE file_texts t SET attempts = t.attempts + 1, claimed_at = NOW()
               WHERE t.file_id IN (
                 SELECT file_id FROM file_texts
                  WHERE status = 'pending' AND claimed_at < NOW() - make_interval(secs => $2)
                    AND attempts < $3
                  ORDER BY claimed_at
                  LIMIT $1
                  FOR UPDATE SKIP LOCKED
               )
              RETURNING t.file_id
            )
            SELECT f.id AS file_id, f.storage_key, f.filename, f.mime_type, f.size_bytes,
                   public.file_kind_of(f.mime_type, f.filename) AS "kind!: FileKind"
              FROM claimed c JOIN files f ON f.id = c.file_id
            "#,
            limit,
            stale_after_secs,
            max_attempts
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn claim_new_texts(&self, limit: i64) -> sqlx::Result<Vec<TextJob>> {
        sqlx::query_as!(
            TextJob,
            r#"
            WITH claimed AS (
              INSERT INTO file_texts (file_id)
              SELECT f.id FROM files f
               WHERE f.message_id IS NOT NULL
                 AND NOT EXISTS (SELECT 1 FROM file_texts t WHERE t.file_id = f.id)
               ORDER BY f.created_at DESC
               LIMIT $1
               FOR UPDATE OF f SKIP LOCKED
              ON CONFLICT (file_id) DO NOTHING
              RETURNING file_id
            )
            SELECT f.id AS file_id, f.storage_key, f.filename, f.mime_type, f.size_bytes,
                   public.file_kind_of(f.mime_type, f.filename) AS "kind!: FileKind"
              FROM claimed c JOIN files f ON f.id = c.file_id
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn finish_text(
        &self,
        file_id: Uuid,
        status: FileTextStatus,
        content: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE file_texts SET status = $2, content = $3, extracted_at = NOW()
              WHERE file_id = $1 AND status = 'pending'",
            file_id,
            status as FileTextStatus,
            content
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn text_backlog(&self) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT (SELECT COUNT(*) FROM files f
                        WHERE f.message_id IS NOT NULL
                          AND NOT EXISTS (SELECT 1 FROM file_texts t WHERE t.file_id = f.id))
                    + (SELECT COUNT(*) FROM file_texts WHERE status = 'pending') AS "backlog!""#
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn channel_id_for_message(&self, message_id: Uuid) -> sqlx::Result<Option<Uuid>> {
        sqlx::query_scalar!("SELECT channel_id FROM messages WHERE id = $1", message_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Workspace photos are uploaded like any other file and never posted to a
    /// message, so they land in the "nothing owns it" bucket. They are meant to
    /// be seen.
    pub async fn is_avatar(&self, storage_key: &str) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM workspace_members
              WHERE avatar_url IS NOT NULL
                AND position($1 in avatar_url) > 0
            ) AS "exists!"
            "#,
            storage_key
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete(&self, id: Uuid) -> sqlx::Result<Option<FileRecord>> {
        sqlx::query_as!(FileRecord, "DELETE FROM files WHERE id = $1 RETURNING id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at", id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Deleting the rows and getting the storage keys back has to be one
    /// statement: a read followed by a delete can hand the same key to two
    /// concurrent callers, and the second one deletes an object the first has
    /// already replaced.
    pub async fn delete_for_channel_message(
        &self,
        message_id: Uuid,
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as!(
            FileRecord,
            "DELETE FROM files WHERE message_id = $1 RETURNING id, message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, thumbnail_key, width, height, created_at",
            message_id
        )
        .fetch_all(&self.pool)
        .await
    }
}
