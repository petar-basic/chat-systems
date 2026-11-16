use std::collections::HashMap;

use serde::Serialize;
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{ProfileField, ProfileFieldKind, ProfileFieldOption};

pub struct FieldDraft<'a> {
    pub label: &'a str,
    pub hint: Option<&'a str>,
    pub kind: ProfileFieldKind,
    pub options: &'a [ProfileFieldOption],
    pub admins_only: bool,
    pub on_card: bool,
}

#[derive(Debug, Serialize)]
pub struct ValueOwner {
    pub workspace_id: Uuid,
    pub field_id: Uuid,
    pub label: String,
    pub kind: ProfileFieldKind,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_label: Option<String>,
}

#[derive(Clone)]
pub struct ProfileFieldRepo {
    pool: PgPool,
}

impl ProfileFieldRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn lock_workspace_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "SELECT pg_advisory_xact_lock(hashtextextended('profile_fields:' || $1::uuid::text, 0))",
            workspace_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn list(&self, workspace_id: Uuid) -> sqlx::Result<Vec<ProfileField>> {
        sqlx::query_as!(
            ProfileField,
            r#"
            SELECT f.id, f.workspace_id, f.label, f.hint, f.kind AS "kind: ProfileFieldKind",
                   f.options AS "options: Json<Vec<ProfileFieldOption>>",
                   f.admins_only, f.on_card, f.position,
                   (SELECT count(*) FROM profile_field_values v WHERE v.field_id = f.id) AS "value_count!"
              FROM profile_fields f
             WHERE f.workspace_id = $1
             ORDER BY f.position, f.created_at
            "#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find(
        &self,
        workspace_id: Uuid,
        field_id: Uuid,
    ) -> sqlx::Result<Option<ProfileField>> {
        Self::find_in(&mut *self.pool.acquire().await?, workspace_id, field_id).await
    }

    pub async fn find_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        field_id: Uuid,
    ) -> sqlx::Result<Option<ProfileField>> {
        sqlx::query_as!(
            ProfileField,
            r#"
            SELECT f.id, f.workspace_id, f.label, f.hint, f.kind AS "kind: ProfileFieldKind",
                   f.options AS "options: Json<Vec<ProfileFieldOption>>",
                   f.admins_only, f.on_card, f.position,
                   (SELECT count(*) FROM profile_field_values v WHERE v.field_id = f.id) AS "value_count!"
              FROM profile_fields f
             WHERE f.workspace_id = $1 AND f.id = $2
            "#,
            workspace_id,
            field_id
        )
        .fetch_optional(&mut *conn)
        .await
    }

    pub async fn count_in(conn: &mut PgConnection, workspace_id: Uuid) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM profile_fields WHERE workspace_id = $1"#,
            workspace_id
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn on_card_count_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        except: Option<Uuid>,
    ) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"
            SELECT count(*) AS "count!"
              FROM profile_fields
             WHERE workspace_id = $1 AND on_card AND id IS DISTINCT FROM $2
            "#,
            workspace_id,
            except
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn create_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        draft: &FieldDraft<'_>,
    ) -> sqlx::Result<Uuid> {
        sqlx::query_scalar!(
            r#"
            INSERT INTO profile_fields (workspace_id, label, hint, kind, options, admins_only, on_card, position)
            VALUES ($1, $2, $3, $4, $5, $6, $7,
                    (SELECT COALESCE(max(position) + 1, 0) FROM profile_fields WHERE workspace_id = $1))
            RETURNING id
            "#,
            workspace_id,
            draft.label,
            draft.hint,
            draft.kind as ProfileFieldKind,
            Json(draft.options) as _,
            draft.admins_only,
            draft.on_card
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn update_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        field_id: Uuid,
        draft: &FieldDraft<'_>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            UPDATE profile_fields
               SET label = $3, hint = $4, options = $5, admins_only = $6, on_card = $7
             WHERE workspace_id = $1 AND id = $2
            "#,
            workspace_id,
            field_id,
            draft.label,
            draft.hint,
            Json(draft.options) as _,
            draft.admins_only,
            draft.on_card
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn drop_values_outside_in(
        conn: &mut PgConnection,
        field_id: Uuid,
        kept: &[String],
    ) -> sqlx::Result<u64> {
        Ok(sqlx::query!(
            "DELETE FROM profile_field_values WHERE field_id = $1 AND NOT (value = ANY($2))",
            field_id,
            kept
        )
        .execute(&mut *conn)
        .await?
        .rows_affected())
    }

    pub async fn delete(&self, workspace_id: Uuid, field_id: Uuid) -> sqlx::Result<bool> {
        Ok(sqlx::query!(
            "DELETE FROM profile_fields WHERE workspace_id = $1 AND id = $2",
            workspace_id,
            field_id
        )
        .execute(&self.pool)
        .await?
        .rows_affected()
            > 0)
    }

    pub async fn reorder_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        field_ids: &[Uuid],
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            UPDATE profile_fields f
               SET position = o.ord - 1
              FROM unnest($2::uuid[]) WITH ORDINALITY AS o(id, ord)
             WHERE f.workspace_id = $1 AND f.id = o.id
            "#,
            workspace_id,
            field_ids
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn ids_in(conn: &mut PgConnection, workspace_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT id FROM profile_fields WHERE workspace_id = $1",
            workspace_id
        )
        .fetch_all(&mut *conn)
        .await
    }

    pub async fn values_of(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<HashMap<Uuid, String>> {
        let rows = sqlx::query!(
            "SELECT field_id, value FROM profile_field_values WHERE workspace_id = $1 AND user_id = $2",
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.field_id, r.value)).collect())
    }

    pub async fn people_among(
        &self,
        workspace_id: Uuid,
        user_ids: &[Uuid],
    ) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r#"
            SELECT wm.user_id
              FROM workspace_members wm
              JOIN users u ON u.id = wm.user_id
             WHERE wm.workspace_id = $1 AND wm.user_id = ANY($2) AND NOT u.is_bot
            "#,
            workspace_id,
            user_ids
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn set_values(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        written: &[(Uuid, String)],
        cleared: &[Uuid],
    ) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        let (fields, values): (Vec<Uuid>, Vec<String>) = written.iter().cloned().unzip();
        sqlx::query!(
            r#"
            INSERT INTO profile_field_values (workspace_id, user_id, field_id, value)
            SELECT $1, $2, w.field_id, w.value
              FROM unnest($3::uuid[], $4::text[]) AS w(field_id, value)
            ON CONFLICT (workspace_id, user_id, field_id)
            DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
            "#,
            workspace_id,
            user_id,
            &fields,
            &values
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM profile_field_values WHERE workspace_id = $1 AND user_id = $2 AND field_id = ANY($3)",
            workspace_id,
            user_id,
            cleared
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }

    pub async fn owned_by(&self, user_id: Uuid) -> sqlx::Result<Vec<ValueOwner>> {
        sqlx::query_as!(
            ValueOwner,
            r#"
            SELECT v.workspace_id, v.field_id, f.label, f.kind AS "kind: ProfileFieldKind", v.value,
                   (SELECT o->>'label' FROM jsonb_array_elements(f.options) o
                     WHERE o->>'id' = v.value) AS option_label
              FROM profile_field_values v
              JOIN profile_fields f ON f.id = v.field_id
             WHERE v.user_id = $1
             ORDER BY v.workspace_id, f.position
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }
}
